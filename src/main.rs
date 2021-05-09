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
use cortex_m::interrupt::free;
use cortex_m::prelude::*;
use cortex_m_rt::entry;

use core::fmt::Write;
use core::format_args;
use player::Player;
use stm32f4xx_hal::{
    interrupt,
    pac::USART2,
    prelude::*,
    serial::{config::Config, Serial, Tx},
    stm32,
    timer::{Event, Timer},
};
use util::GlobalCell;

const POLL_FREQ: u32 = 10;
const LONG_PRESS_DURATION: u32 = 2;

// global variables to be shared with ISRs
static ENCODER: GlobalCell<Encoder> = GlobalCell::new(None);
static TIMER_TIM5: GlobalCell<Timer<stm32::TIM5>> = GlobalCell::new(None);

/// UART port used for debug
static DEBUG_UART_TX: GlobalCell<Tx<USART2>> = GlobalCell::new(None);

static EVENT_QUEUE: EventQueue<32> = EventQueue::new();

/// Logs the formatted string to the debug USART port.
/// Note: do _not_ use this inside a CriticalSection. Instead, use `logf_cs` and pass in the CriticalSection object
#[allow(unused_macros)]
macro_rules! logf {
    ($($arg:tt)*) => (
        free(|cs| DEBUG_UART_TX.try_borrow_mut(cs, |uart|Some(uart.write_fmt(format_args!($($arg)*)))).unwrap()).unwrap()
    );
}

/// Logs the formatted string to the debug USART port if already in a CriticalSection
#[allow(unused_macros)]
macro_rules! logf_cs {
    ($cs:ident, $($arg:tt)*) => (
        DEBUG_UART_TX.try_borrow_mut($cs, |uart|Some(uart.write_fmt(format_args!($($arg)*)))).unwrap().unwrap()
    );
}

#[interrupt]
fn TIM5() {
    free(|cs| {
        TIMER_TIM5.try_borrow_mut::<_, ()>(cs, |tim| {
            tim.clear_interrupt(Event::TimeOut);
            Some(())
        });

        // logf_cs!(cs, "Tick!\n");

        // put a tich event in the queue as a status indicator for now
        EVENT_QUEUE.put(cs, InterruptEvent::Tick);

        // // poll the quadrature encoder to see if any change was made
        ENCODER.try_borrow_mut(cs, |enc| {
            if let Some(change) = enc.check() {
                EVENT_QUEUE.put(cs, InterruptEvent::Encoder(change));
            }

            if let Some(evt) = enc.check_btn(LONG_PRESS_DURATION * POLL_FREQ) {
                EVENT_QUEUE.put(
                    cs,
                    match evt {
                        encoder::Button::ShortPress => InterruptEvent::ShortPress,
                        encoder::Button::LongPress => InterruptEvent::LongPress,
                    },
                );
            }

            Some(())
        });
    })
}

#[entry]
fn main() -> ! {
    let peripherals = stm32f4xx_hal::stm32::Peripherals::take().unwrap();
    let peripherals_m = cortex_m::Peripherals::take().unwrap();

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

    // setup UART communication through the ST-LINK/V2-1 debugger
    let gpioa = peripherals.GPIOA.split();

    let serial = Serial::new(
        peripherals.USART2,
        (
            gpioa.pa2.into_alternate_af7(),
            gpioa.pa3.into_alternate_af7(),
        ),
        Config::default().baudrate(115200.bps()),
        clocks,
    )
    .unwrap();
    let (tx, _rx) = serial.split();
    DEBUG_UART_TX.put(tx);

    let mut delay = Delay::new(peripherals_m.SYST, clocks.sysclk().0);

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
        gpiob.pb14.into_pull_down_input(),
    ));

    let mut timer = Timer::tim5(peripherals.TIM5, POLL_FREQ.hz(), clocks);
    timer.listen(Event::TimeOut);
    TIMER_TIM5.put(timer);
    // enable TIM5 interrupt in the NVIC
    stm32::NVIC::unpend(stm32f4xx_hal::interrupt::TIM5);
    unsafe {
        stm32::NVIC::unmask(stm32f4xx_hal::interrupt::TIM5);
    };

    'outer: loop {
        // wait for any interrupts to happen ("sleep")
        // NOTE: makes the debugger having a hard time connecting sometimes
        cortex_m::asm::wfi();

        // free(|cs| logf_cs!(cs, "Queue size: {:?}\n", EVENT_QUEUE.count(cs)));

        // handle all pending events
        while let Some(evt) = free(|cs| EVENT_QUEUE.take(cs)) {
            use InterruptEvent::*;
            match evt {
                Tick => {
                    led.toggle().unwrap();
                }
                Encoder(change) => {
                    logf!("Encoder: {:?}\n", change);
                    if change == 2 {
                        // for development
                        break 'outer;
                    }
                }
                e => logf!("Unhandled event: {:?}\n", e),
            }
        }
    }

    // experiment with RTC
    let c = Clock::new(peripherals.RTC);

    if !c.is_set() {
        logf!("RTC is not initialized! Setting up...\n");

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
                logf!("Error initializing SD-card: {:?}\n", e);
            }
        }

        led.set_high().unwrap();
        delay.delay_ms(100);
        led.set_low().unwrap();
        delay.delay_ms(100);
    }

    // if everything went fine, get a reference to the card and print some debug data
    if let Ok(card) = sdio.card() {
        logf!("Card successfully initialized! Info: {:?}\n", card.cid);

        // read block of data and print it

        let nblocks = sdio.card().map(|c| c.block_count()).unwrap_or(0);
        logf!("Card detected: nbr of blocks: {:?}\n", nblocks);

        let mut block = [0u8; 512];
        match sdio.read_block(1, &mut block) {
            Ok(_) => (),
            Err(err) => {
                logf!("Failed to read block: {:?}\n", err);
            }
        }

        for (i, b) in block.iter().enumerate() {
            logf!("{:02X} ", b);

            if (i + 1) % 16 == 0 {
                logf!("\n");
            }
        }

        logf!("\n");

        // try to write something to the second block
        for (i, b) in block.iter_mut().enumerate() {
            *b = (i % 255) as u8;
        }

        // if let Err(e) = sdio.write_block(1, &block) {
        //     logf!("Failed to write block: {:?}\n", e);
        //     loop{}
        // }
    } else {
        logf!("Card could not be initialized!\n");
    }

    loop {
        led.set_high().unwrap();

        // logf!("On!\n");

        delay.delay_ms(1000);

        led.set_low().unwrap();

        // logf!("Off!\n");
        delay.delay_ms(1000);
    }
}
