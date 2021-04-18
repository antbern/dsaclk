#![no_std]
#![no_main]

mod clock;
mod encoder;
mod event;
mod player;
mod util;

use clock::Clock;
use encoder::Encoder;
use event::{EventQueue, InterruptEvent};
// pick a panicking behavior
use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
                     // use panic_abort as _; // requires nightly
                     // use panic_itm as _; // logs messages over ITM; requires ITM support
                     // use panic_semihosting as _; // logs messages to the host stderr; requires a debugger

// use cortex_m::asm;
use cortex_m::delay::Delay;
use cortex_m::prelude::*;
use cortex_m::{interrupt::free, iprint, iprintln};
use cortex_m_rt::entry;

use player::Player;
use stm32f4xx_hal::{
    interrupt,
    prelude::*,
    stm32,
    timer::{Event, Timer},
};
use util::GlobalCell;

// global variables to be shared with ISRs
static ENCODER: GlobalCell<Encoder> = GlobalCell::new(None);
static TIMER_TIM5: GlobalCell<Timer<stm32::TIM5>> = GlobalCell::new(None);
// static EVENT_QUEUE: GlobalCell<EventQueue> = GlobalCell::new(Some(EventQueue::new()));

static EVENT_QUEUE: EventQueue<32> = EventQueue::new();

#[interrupt]
fn TIM5() {
    free(|cs| {
        TIMER_TIM5.try_borrow_mut::<_, ()>(cs, |tim| {
            tim.clear_interrupt(Event::TimeOut);
            Some(())
        });

        // put a tich event in the queue as a status indicator for now
        EVENT_QUEUE.put(cs, InterruptEvent::Tick);

        // // poll the quadrature encoder to see if any change was made
        ENCODER.try_borrow_mut(cs, |enc| {
            if let Some(change) = enc.check() {
                // yes, post an event to the main loop

                // EVT_QUEUE.borrow(cs).put(InterruptEvent::Encoder(change));
                EVENT_QUEUE.put(cs, InterruptEvent::Encoder(change));
            }
            Some(())
        });
    })
}

#[entry]
fn main() -> ! {
    let peripherals = stm32f4xx_hal::stm32::Peripherals::take().unwrap();
    let mut peripherals_m = cortex_m::Peripherals::take().unwrap();

    // enable the timer 4 peripheral in RCC before constraining the clock
    peripherals.RCC.apb1enr.modify(|_, w| w.tim4en().enabled());

    cortex_m::asm::dsb(); // needed to prevent some errors due to optimizations
                          // reset timer 4 peripheral
    peripherals
        .RCC
        .apb1rstr
        .modify(|_, w| w.tim4rst().set_bit());
    peripherals
        .RCC
        .apb1rstr
        .modify(|_, w| w.tim4rst().clear_bit());

    let rcc = peripherals.RCC.constrain();

    let clocks = rcc
        .cfgr
        .use_hse(8.mhz())
        .pclk1(42.mhz())
        .pclk2(84.mhz())
        .sysclk(84.mhz())
        .hclk(84.mhz())
        .require_pll48clk()
        .freeze();

    assert!(clocks.is_pll48clk_valid());

    let stim = &mut peripherals_m.ITM.stim[0];

    let mut delay = Delay::new(peripherals_m.SYST, clocks.sysclk().0);

    let gpioa = peripherals.GPIOA.split();
    let mut led = gpioa.pa5.into_push_pull_output();

    led.set_high().unwrap();

    // experiment with tone generator
    let gpiob = peripherals.GPIOB.split();

    let player = Player::new(peripherals.TIM4, gpiob.pb7.into_alternate_af2(), &clocks);
    player.play(&mut delay);

    // setup Timer 3 as an encoder and put it in the mutex as a global variable
    ENCODER.put(Encoder::new(
        peripherals.TIM3,
        gpiob.pb4.into_alternate_af2().internal_pull_up(true),
        gpiob.pb5.into_alternate_af2().internal_pull_up(true),
    ));

    let mut timer = Timer::tim5(peripherals.TIM5, 1.hz(), clocks);
    timer.listen(Event::TimeOut);
    TIMER_TIM5.put(timer);
    // enable TIM5 interrupt in the NVIC
    stm32::NVIC::unpend(stm32f4xx_hal::interrupt::TIM5);
    unsafe {
        stm32::NVIC::unmask(stm32f4xx_hal::interrupt::TIM5);
    };

    loop {
        // wait for any interrupts to happen ("sleep")
        cortex_m::asm::wfi();
        free(|cs| iprintln!(stim, "Queue size: {:?}", EVENT_QUEUE.count(cs)));

        // handle all pending events
        while let Some(evt) = free(|cs| EVENT_QUEUE.take(cs)) {
            use InterruptEvent::*;
            match evt {
                Tick => {
                    led.toggle().unwrap();
                }
                Encoder(change) => {
                    iprintln!(stim, "Encoder: {:?}", change);
                }
                _ => (),
            }
        }
        // led.toggle().unwrap();
    }

    // let prescaler: u16 = (clocks.sysclk().0 / 32_000_000).try_into().unwrap();
    // tim.psc.modify(|_, w|w.psc().bits(prescaler));

    // experiment with RTC
    let mut c = Clock::new(peripherals.RTC);

    if !c.is_set() {
        iprintln!(stim, "RTC is not initialized! Setting up...");

        c.init();
    }

    // setup sdio interface for sc card
    let gpioc = peripherals.GPIOC.split();
    let gpiod = peripherals.GPIOD.split();

    let d0 = gpioc.pc8.into_alternate_af12().internal_pull_up(true);
    let clk = gpioc.pc12.into_alternate_af12().internal_pull_up(false);
    let cmd = gpiod.pd2.into_alternate_af12().internal_pull_up(true);
    let mut sdio = stm32f4xx_hal::sdio::Sdio::new(peripherals.SDIO, (clk, cmd, d0), clocks);

    led.set_low().unwrap();

    loop {
        match sdio.init_card(stm32f4xx_hal::sdio::ClockFreq::F12Mhz) {
            Ok(_) => break,
            Err(e) => {
                iprintln!(stim, "Error initializing SD-card: {:?}", e);
            }
        }

        led.set_high().unwrap();
        delay.delay_ms(100);
        led.set_low().unwrap();
        delay.delay_ms(100);
    }

    // if everything went fine, get a reference to the card and print some debug data
    if let Ok(card) = sdio.card() {
        iprintln!(stim, "Card successfully initialized! Info: {:?}", card.cid);

        // read block of data and print it

        let nblocks = sdio.card().map(|c| c.block_count()).unwrap_or(0);
        iprintln!(stim, "Card detected: nbr of blocks: {:?}", nblocks);

        let mut block = [0u8; 512];
        match sdio.read_block(1, &mut block) {
            Ok(_) => (),
            Err(err) => {
                iprintln!(stim, "Failed to read block: {:?}", err);
            }
        }

        let mut i = 0;
        for b in block.iter() {
            iprint!(stim, "{:02X} ", b);
            // iprint!(stim, "X ");

            // delay to allow ITM to send stuff
            delay.delay_ms(10);

            if (i + 1) % 16 == 0 {
                iprintln!(stim);
            }
            i += 1;
        }

        iprintln!(stim);

        // try to write something to the second block
        for (i, b) in block.iter_mut().enumerate() {
            *b = (i % 255) as u8;
        }

        // if let Err(e) = sdio.write_block(1, &block) {
        //     iprintln!(stim, "Failed to write block: {:?}", e);
        //     loop{}
        // }
    } else {
        iprintln!(stim, "Card could not be initialized!");
    }

    loop {
        led.set_high().unwrap();

        iprintln!(stim, "On!");

        delay.delay_ms(1000);

        led.set_low().unwrap();

        iprintln!(stim, "Off!");
        delay.delay_ms(1000);
    }
}
