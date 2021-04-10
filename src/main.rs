#![no_std]
#![no_main]

mod clock;

use core::{cell::RefCell, hint::spin_loop, ops::DerefMut};

use clock::Clock;
// pick a panicking behavior
use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
// use panic_abort as _; // requires nightly
// use panic_itm as _; // logs messages over ITM; requires ITM support
// use panic_semihosting as _; // logs messages to the host stderr; requires a debugger


// use cortex_m::asm;
use cortex_m_rt::entry;
use cortex_m::{interrupt::{Mutex, free}, iprintln, iprint};
use cortex_m::prelude::*;
use cortex_m::delay::Delay;

use stm32f4xx_hal::{gpio::{Output, PushPull, gpiob::PB7}, interrupt, prelude::*, stm32, timer::{Event, PinC2, Timer}};

static TIMER_TIM4: Mutex<RefCell<Option<Timer<stm32::TIM4>>>> = Mutex::new(RefCell::new(None));

static BUZZER: Mutex<RefCell<Option<PB7<Output<PushPull>>>>> = Mutex::new(RefCell::new(None));



#[interrupt]
fn TIM4() {
    free(|cs| {
        if let Some(ref mut tim4) = TIMER_TIM4.borrow(cs).borrow_mut().deref_mut(){
            tim4.clear_interrupt(Event::TimeOut);
        }


        if let Some(ref mut buzzer) = BUZZER.borrow(cs).borrow_mut().deref_mut() {

            for _ in 0..100 {
                buzzer.set_high().unwrap();
                cortex_m::asm::delay(84_000);
                buzzer.set_low().unwrap();
                cortex_m::asm::delay(84_000);
            }

        }
    
    })

    
}

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

    assert!(clocks.is_pll48clk_valid());

    // setup_clock(peripherals.RCC, peripherals.FLASH, peripherals.PWR);

    let stim = &mut peripherals_m.ITM.stim[0];
    
    // peripherals_m.SYST.set_clock_source()
    let mut delay = Delay::new(peripherals_m.SYST, clocks.sysclk().0);
    
    
    let gpioa = peripherals.GPIOA.split();
    let mut led = gpioa.pa5.into_push_pull_output();

    led.set_high().unwrap();


    // experiment with tone generator
    let gpiob = peripherals.GPIOB.split();
    
    let mut buzzer  = gpiob.pb7.into_push_pull_output(); // into_alternate_af2();
    free(|cs| *BUZZER.borrow(cs).borrow_mut() = Some(buzzer));
   
    let mut timer = Timer::tim4(peripherals.TIM4, 1.hz(), clocks);
    timer.listen(Event::TimeOut);
    free(|cs| *TIMER_TIM4.borrow(cs).borrow_mut() = Some(timer));
    
    stm32::NVIC::unpend(stm32f4xx_hal::interrupt::TIM4);

    unsafe { stm32::NVIC::unmask(stm32f4xx_hal::interrupt::TIM4); };

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
            },
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

            if (i+1) % 16 == 0 {
                iprintln!(stim);
            }
            i += 1;
        }

        iprintln!(stim);

        // try to write something to the second block
        for (i, b) in block.iter_mut().enumerate(){
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
