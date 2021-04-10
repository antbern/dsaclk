#![no_std]
#![no_main]

mod clock;

// pick a panicking behavior
use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
// use panic_abort as _; // requires nightly
// use panic_itm as _; // logs messages over ITM; requires ITM support
// use panic_semihosting as _; // logs messages to the host stderr; requires a debugger


// use cortex_m::asm;
use cortex_m_rt::entry;
use cortex_m::iprintln;
use cortex_m::prelude::*;
use cortex_m::delay::Delay;

use stm32f4xx_hal::prelude::*;

#[entry]
fn main() -> ! {
    
    
    let peripherals = stm32f4xx_hal::stm32::Peripherals::take().unwrap();
    let mut peripherals_m = cortex_m::Peripherals::take().unwrap();

    let rcc = peripherals.RCC.constrain();

    let clocks = rcc.cfgr
        .use_hse(8.mhz())
        .pclk1(42.mhz())
        .pclk2(84.mhz())
        .sysclk(84.mhz())
        .hclk(84.mhz())
        .require_pll48clk()
        .freeze();

    // setup_clock(peripherals.RCC, peripherals.FLASH, peripherals.PWR);

    let stim = &mut peripherals_m.ITM.stim[0];
    
    let gpioa = peripherals.GPIOA.split();
    let mut led = gpioa.pa5.into_push_pull_output();

    led.set_high().unwrap();
    
    
    // setup sdio interface for sc card
    // let sdio = &peripherals.SDIO;

    let gpioc = peripherals.GPIOC.split();
    let gpiod = peripherals.GPIOD.split();

    let mut sdio = stm32f4xx_hal::sdio::Sdio::new(peripherals.SDIO, (
        gpioc.pc12.into_alternate_af12(), 
        gpiod.pd2.into_alternate_af12(),
        gpioc.pc8.into_alternate_af12(),
    ));


    // peripherals_m.SYST.set_clock_source()
    let mut delay = Delay::new(peripherals_m.SYST, clocks.sysclk().0);
    

    // delay.delay_ms(10000);
    led.set_low().unwrap();
    

    if let Err(e) = sdio.init_card(stm32f4xx_hal::sdio::ClockFreq::F400Khz) {
        iprintln!(stim, "Error initializing SD-card: {:?}", e);
        loop {
            led.set_high().unwrap();
            delay.delay_ms(100);
            led.set_low().unwrap();
            delay.delay_ms(100);

        }
    }
    // everything went fine, get a reference to the card

    if let Ok(card) = sdio.card() {
        iprintln!(stim, "Card successfully initialized! Info: {:?}", card.cid());
    } else {
        iprintln!(stim, "Card could not be initialized!");
    }

    



    
    // println!("Hello, World!");   


    // let mut flash = peripherals.FLASH;
    // let clocks = rcc.cfgr.freeze(&mut flash.acr);
    // let cp = cortex_m::Peripherals::take().unwrap();
    // let mut delay = embedded_hal::blocking::delay::DelayMs::delay_ms(cp.SYST, clocks);

    // let mut stdout = match hio::hstdout() {
    //     Ok(fd) => fd,
    //     Err(()) => return Err(core::fmt::Error),
    // };
    

    loop { 
        led.set_high().unwrap();

        iprintln!(stim, "On!");

        delay.delay_ms(1000);

        led.set_low().unwrap();

        iprintln!(stim, "Off!");
        delay.delay_ms(1000);

    }
}
