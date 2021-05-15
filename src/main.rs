#![no_std]
#![no_main]

mod clock;
mod defmt_uart;
mod display;
mod encoder;
mod event;
mod player;
mod util;

use clock::Clock;
use defmt::{debug, error, info};
use encoder::Encoder;
use event::{EventQueue, InterruptEvent};

// use panic-probe as panicking behavior which will print panic
// messages through defmt
use panic_probe as _;

// use cortex_m::asm;
use cortex_m::delay::Delay;
use cortex_m::interrupt::free;
use cortex_m::prelude::*;
use cortex_m_rt::entry;
use player::Player;
use stm32f4xx_hal::{
    i2c::I2c,
    interrupt,
    pac::USART2,
    prelude::*,
    serial::{config::Config, Serial, Tx},
    stm32,
    timer::{Event, Timer},
};
use util::GlobalCell;

use crate::display::I2CLCDDisplay;

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

        // put a tick event in the queue as a status indicator for now
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

    // setup I2C
    let i2c = I2c::new(
        peripherals.I2C1,
        (
            gpiob.pb8.into_alternate_af4().set_open_drain(),
            gpiob.pb9.into_alternate_af4().set_open_drain(),
        ),
        100.khz(),
        clocks,
    );

    let mut display = I2CLCDDisplay::new(i2c, 0x63, 4, 20);

    display.set_backlight_enabled(true).unwrap();
    for i in (0..=255).step_by(8) {
        display.set_backlight_brightness(i).unwrap();
        delay.delay_ms(100);
    }

    for i in (0..=255).rev().step_by(8) {
        display.set_backlight_brightness(i).unwrap();
        delay.delay_ms(100);
    }

    display.set_backlight_enabled(false).unwrap();

    defmt::info!("Initializing!");

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
                    // logf!("Encoder: {:?}\n", change);
                    defmt::info!("Encoder: {=i8}", change);
                    if change == 2 {
                        // for development
                        break 'outer;
                    }
                }
                e => defmt::error!("Unhandled event: {}", e),
            }
        }
    }

    // experiment with RTC
    let c = Clock::new(peripherals.RTC);

    if !c.is_set() {
        info!("RTC is not initialized! Setting up...");

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
                defmt::error!("Error initializing SD-card: {}", defmt::Debug2Format(&e));
            }
        }

        led.set_high().unwrap();
        delay.delay_ms(100);
        led.set_low().unwrap();
        delay.delay_ms(100);
    }

    // if everything went fine, get a reference to the card and print some debug data
    if let Ok(card) = sdio.card() {
        info!(
            "Card successfully initialized! Info: {}",
            defmt::Debug2Format(&card.cid)
        );

        // read block of data and print it

        let nblocks = sdio.card().map(|c| c.block_count()).unwrap_or(0);
        info!("Card detected: nbr of blocks: {=u32}", nblocks);

        let mut block = [0u8; 512];
        match sdio.read_block(1, &mut block) {
            Ok(_) => (),
            Err(err) => {
                error!("Failed to read block: {}", defmt::Debug2Format(&err));
            }
        }

        for i in (0..512).step_by(16) {
            debug!("{=[u8]:X} ", block[i..i + 16]);

            // if (i + 1) % 16 == 0 {
            //     logf!("\n");
            // }
        }

        // logf!("\n");

        // try to write something to the second block
        for (i, b) in block.iter_mut().enumerate() {
            *b = (i % 255) as u8;
        }

        // if let Err(e) = sdio.write_block(1, &block) {
        //     logf!("Failed to write block: {:?}\n", e);
        //     loop{}
        // }
    } else {
        error!("Card could not be initialized!");
    }

    loop {
        led.set_high().unwrap();
        delay.delay_ms(1000);

        led.set_low().unwrap();
        delay.delay_ms(1000);
    }
}
