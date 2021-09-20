#![no_std]
#![no_main]

mod clock;
mod defmt_uart;
mod dialog;
mod display;
mod encoder;
mod event;
mod logger;
mod mpu;
mod panel;
mod player;
mod sdcard;
mod util;
mod vec;

use clock::{Clock, ClockState};
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

use crate::{
    clock::AlarmState,
    display::{Display, I2CDisplayDriver},
    logger::{LogContents, Logger},
    mpu::MPU,
};
use crate::{dialog::Dialog, panel::CursorState};
use crate::{panel::Panel, sdcard::SdCard};

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

#[derive(Debug)]
pub struct SharedState {
    clock: ClockState,
    alarm: AlarmState,
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

    // TEMP: Setup RTC clock
    peripherals.RCC.apb1enr.modify(|_, w| w.pwren().enabled());
    peripherals.PWR.cr.modify(|_, w| w.dbp().set_bit());
    peripherals
        .RCC
        .bdcr
        .modify(|_, w| w.lseon().on().rtcsel().lse());

    while peripherals.RCC.bdcr.read().lserdy().is_not_ready() {}
    peripherals.RCC.bdcr.modify(|_, w| w.rtcen().enabled());

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

    // split all GPIO pins needed
    let gpioa = peripherals.GPIOA.split();
    let gpiob = peripherals.GPIOB.split();
    let gpioc = peripherals.GPIOC.split();
    let gpiod = peripherals.GPIOD.split();

    // setup UART communication through the ST-LINK/V2-1 debugger
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

    #[allow(unused_variables)]
    let player = Player::new(peripherals.TIM4, gpiob.pb7.into_alternate_af2(), &clocks);
    // player.play(&mut delay);

    // setup Timer 3 as an encoder and put it in the mutex as a global variable
    ENCODER.put(Encoder::new(
        peripherals.TIM3,
        gpiob.pb4.into_alternate_af2().internal_pull_up(true),
        gpiob.pb5.into_alternate_af2().internal_pull_up(true),
        gpiob.pb14.into_pull_down_input(),
    ));

    // setup the timer used for polling (started later)
    let mut timer = Timer::tim5(peripherals.TIM5, POLL_FREQ.hz(), clocks);
    timer.listen(Event::TimeOut);
    TIMER_TIM5.put(timer);

    // setup sdio interface for SD card
    let d0 = gpioc.pc8.into_alternate_af12().internal_pull_up(true);
    let clk = gpioc.pc12.into_alternate_af12().internal_pull_up(false);
    let cmd = gpiod.pd2.into_alternate_af12().internal_pull_up(true);
    let sdio = stm32f4xx_hal::sdio::Sdio::new(peripherals.SDIO, (clk, cmd, d0), clocks);

    let mut card = SdCard::init(sdio, &mut delay, 2000).unwrap();

    //card.store_settings(Settings { logger_block: 10 }).unwrap();

    let mut settings = card.load_settings().unwrap();
    defmt::debug!("Loaded settings: {}", defmt::Debug2Format(&settings));

    // experiment with RTC
    let mut c = Clock::new(peripherals.RTC);

    if !c.is_set() {
        info!("RTC is not initialized! Setting up...");

        c.init();
    }
    c.enable_alarm_interrupt(&peripherals.EXTI);

    // setup and try MPU6050 I2C3

    let i2c3 = I2c::new(
        peripherals.I2C3,
        (
            gpioa.pa8.into_alternate_af4().set_open_drain(),
            gpioc.pc9.into_alternate_af4().set_open_drain(),
        ),
        400.khz(),
        clocks,
    );

    let mut mpu = MPU::new(i2c3, &mut delay, 10).unwrap();

    // setup display I2C
    let i2c = I2c::new(
        peripherals.I2C1,
        (
            gpiob.pb8.into_alternate_af4().set_open_drain(),
            gpiob.pb9.into_alternate_af4().set_open_drain(),
        ),
        100.khz(),
        clocks,
    );

    let mut display = I2CDisplayDriver::new(i2c, 0x63, 4, 20);
    display.set_type(4, &mut delay).unwrap();
    display.set_cursor_mode(display::CursorMode::Off).unwrap();

    display.set_backlight_brightness(64).unwrap();
    display.set_backlight_enabled(true).unwrap();

    // write stuff to the screen
    let mut disp: display::BufferedDisplay<4, 20> = display::BufferedDisplay::new();
    disp.clear().unwrap();
    disp.apply(&mut display).unwrap();

    defmt::info!("Initializing!");

    let calib = mpu.calibrate(&mut delay, 100, 1000 / 100 * 4);
    defmt::debug!("Calibrated: {}", defmt::Debug2Format(&calib));
    mpu.set_calibration(calib);

    // setup stuff for the menu system
    let manager: &mut dyn Panel<display::BufferedDisplay<4, 20>> =
        &mut panel::time::TimePanel::new();

    // setup the shared state
    let mut panel_state = SharedState {
        clock: c.get_state(),
        alarm: c.get_alarm(),
    };

    let mut last_cursor_state = CursorState::Off;
    let mut last_edit_state = false;
    let mut dialog: Option<dialog::Dialog> = None;

    // create logger with one minute timeout
    let mut logger = Logger::new();

    // enable TIM5 interrupt in the NVIC before starting the loop
    stm32::NVIC::unpend(stm32f4xx_hal::interrupt::TIM5);
    unsafe {
        stm32::NVIC::unmask(stm32f4xx_hal::interrupt::TIM5);
    };

    loop {
        // wait for any interrupts to happen ("sleep")
        // NOTE: makes the debugger having a hard time connecting sometimes
        // cortex_m::asm::wfi();

        // free(|cs| logf_cs!(cs, "Queue size: {:?}\n", EVENT_QUEUE.count(cs)));

        // handle all pending events
        while let Some(evt) = free(|cs| EVENT_QUEUE.take(cs)) {
            use InterruptEvent::*;

            if let Some(d) = dialog {
                match evt {
                    LongPress | ShortPress => {
                        dialog = None;
                        // post the event that is supposed to happen after the dialog
                        if let Some(e) = d.event {
                            free(|cs| EVENT_QUEUE.put(cs, e));
                        }
                    }
                    _ => (),
                }
            } else {
                match evt {
                    Tick => {
                        led.toggle().unwrap();

                        // fetch the current date and time from the clock if the panel is not editing
                        if last_edit_state && !manager.is_editing() {
                            c.set_state(panel_state.clock);
                            c.set_alarm(panel_state.alarm);
                        }

                        if !manager.is_editing() {
                            panel_state.clock = c.get_state();
                            panel_state.alarm = c.get_alarm();
                        }

                        last_edit_state = manager.is_editing();

                        if let Some(m) = mpu.tick() {
                            defmt::debug!("measurement: {:?}", defmt::Debug2Format(&m));
                            logger
                                .append(
                                    &c.get_state(),
                                    LogContents::Measurement(m),
                                    &mut card,
                                    &mut settings,
                                )
                                .expect("Error appending to log");
                        }
                    }
                    Encoder(change) => {
                        let mut c = change;
                        while c > 0 {
                            manager.next(&mut panel_state);
                            c -= 1;
                        }

                        while c < 0 {
                            manager.previous(&mut panel_state);
                            c += 1;
                        }

                        // logf!("Encoder: {:?}\n", change);
                        defmt::info!("Encoder: {=i8}", change);
                    }
                    LongPress => {
                        manager.leave(&mut panel_state);
                        logger
                            .flush(&mut card, &mut settings)
                            .expect("Error flushing log");
                    }
                    ShortPress => manager.enter(&mut panel_state),
                    Alarm => {
                        defmt::info!("Alarm Interrupt! {}", defmt::Debug2Format(&c.get_alarm()));

                        // free(|cs| {
                        //     let diag = crate::Dialog::new("Alarm triggered", None);
                        //     EVENT_QUEUE.put(cs, InterruptEvent::Dialog(diag));
                        // });
                        c.alarm_reset();
                        dialog = Some(crate::Dialog::new("Alarm triggered", None));
                    } // Dialog(d) => dialog = Some(d),
                }
            }
        }

        disp.clear().unwrap();

        let cursor_state = if let Some(d) = dialog {
            d.display(&mut disp).unwrap();
            CursorState::Off
        } else {
            manager.display(&mut disp, &mut panel_state).unwrap();
            manager.get_cursor_state(&panel_state)
        };

        // update the display after processing all events
        let changed = disp.apply(&mut display).unwrap();

        // set the cursor mode if the screen was modified of the cursor state changed
        if changed || cursor_state != last_cursor_state {
            match cursor_state {
                panel::CursorState::Off => {
                    display.set_cursor_mode(display::CursorMode::Off).unwrap()
                }
                panel::CursorState::Underline(r, c) => {
                    display.set_cursor_position(r, c).unwrap();
                    display
                        .set_cursor_mode(display::CursorMode::Underline)
                        .unwrap();
                }
                panel::CursorState::Blinking(r, c) => {
                    display.set_cursor_position(r, c).unwrap();
                    display
                        .set_cursor_mode(display::CursorMode::Blinking)
                        .unwrap();
                }
            }
        }
        last_cursor_state = cursor_state;
    }
}
