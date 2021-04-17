use cortex_m::{delay::Delay, prelude::_embedded_hal_blocking_delay_DelayMs};
use stm32f4xx_hal::{
    gpio::{gpiob::PB7, Alternate, AF2},
    rcc::Clocks,
    stm32 as stm32f401,
};

pub struct Player {
    timer: stm32f401::TIM4,
    pin: PB7<Alternate<AF2>>,
    psc_freq: u32,
}

impl Player {
    pub fn new(timer: stm32f401::TIM4, pin: PB7<Alternate<AF2>>, clocks: &Clocks) -> Player {
        // setup the clock source to be the internal clock
        timer.smcr.modify(
            |_, w| w.ece().disabled().sms().disabled(), // clock directly from the internal clock
        );
        // write prescaler to generate 10kHz clock
        let psc_freq = 10000;
        timer
            .psc
            .write(|w| w.psc().bits((clocks.pclk2().0 / psc_freq) as u16 - 1));

        // 3a) setup CH2 as toggling output (= PB7 AF02)  and
        // 3c) disable preload feature
        timer
            .ccmr1_output_mut()
            .modify(|_, w| w.oc2m().toggle().oc2pe().disabled());

        // 3b) select output polarity
        timer.ccer.modify(|_, w| w.cc2p().clear_bit());

        // 3d) enable capture/compare 2 output
        timer.ccer.modify(|_, w| w.cc2e().set_bit());

        Player {
            timer,
            pin,
            psc_freq,
        }
    }

    pub fn play(&self, delay: &mut Delay) {
        // write ARR to decide the output frequency
        self.set_frequency(440);
        self.start_tone();
        delay.delay_ms(1000);

        self.set_frequency(1000);

        delay.delay_ms(100);

        self.set_frequency(2000);

        delay.delay_ms(1000);
        self.stop_tone();
    }

    pub fn set_frequency(&self, freq: u32) {
        self.timer
            .arr
            .write(|w| w.arr().bits((self.psc_freq / freq) as u16));
    }

    pub fn start_tone(&self) {
        // set up toggle function and enable the timer
        self.timer
            .ccmr1_output_mut()
            .modify(|_, w| w.oc2m().toggle());
        self.timer.cr1.modify(|_, w| w.cen().enabled().dir().down());
    }

    pub fn stop_tone(&self) {
        // disable the timer
        self.timer.cr1.modify(|_, w| w.cen().disabled());
        //force output low
        self.timer
            .ccmr1_output_mut()
            .modify(|_, w| w.oc2m().force_inactive());
    }
}
