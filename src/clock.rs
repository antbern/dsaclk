use stm32f4xx_hal::stm32 as stm32f401;

pub struct Clock {
    rtc: stm32f401::RTC,
}

impl Clock {
    pub fn new(reg: stm32f401::RTC) -> Clock {
        Clock { rtc: reg }
    }

    /// Initialize the RTC clock
    pub fn init(&mut self) {
        // Disable RTC write protection
        self.rtc.wpr.write(|w| w.key().bits(0xCA));
        self.rtc.wpr.write(|w| w.key().bits(0x53));

        // Enter initialization mode
        self.rtc.isr.modify(|_, w| w.init().init_mode());

        // wait for confirmation
        while self.rtc.isr.read().initf().is_not_allowed() {}

        // program prescaler (if needed)
        // since the default is okay if using a 32.768 kHz crystal we do not need to do anything

        // load time and date values
        self.rtc.tr.modify(|_, w| w.su().bits(3));

        // configure time format
        self.rtc.cr.modify(|_, w| w.fmt().twenty_four_hour());

        // exit initialization mode
        self.rtc.isr.modify(|_, w| w.init().free_running_mode());

        // enable RTC write protection
        self.rtc.wpr.write(|w| w.key().bits(0xFF));
    }

    pub fn is_set(&self) -> bool {
        self.rtc.isr.read().inits().bit_is_set()
    }

    pub fn get_su(&self) -> u8 {
        self.rtc.tr.read().su().bits()
    }
}
