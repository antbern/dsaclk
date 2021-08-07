use stm32f4xx_hal::stm32 as stm32f401;

#[derive(Debug, Clone, Copy)]
pub struct ClockState {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub weekday: u8,
    pub day: u8,
    pub month: u8,
    pub year: u8,
}

impl Default for ClockState {
    fn default() -> Self {
        ClockState {
            hour: 0,
            minute: 0,
            second: 0,
            weekday: 1,
            day: 1,
            month: 1,
            year: 0,
        }
    }
}

pub struct Clock {
    rtc: stm32f401::RTC,
}

impl Clock {
    pub fn new(reg: stm32f401::RTC) -> Clock {
        Clock { rtc: reg }
    }

    fn initialization_mode<F>(&mut self, f: F)
    where
        F: FnOnce(&mut stm32f401::RTC),
    {
        // Disable RTC write protection
        self.rtc.wpr.write(|w| w.key().bits(0xCA));
        self.rtc.wpr.write(|w| w.key().bits(0x53));

        // Enter initialization mode
        self.rtc.isr.modify(|_, w| w.init().init_mode());

        // wait for confirmation
        while self.rtc.isr.read().initf().is_not_allowed() {}

        // we can now safely initialize the RTC
        f(&mut self.rtc);

        // exit initialization mode
        self.rtc.isr.modify(|_, w| w.init().free_running_mode());

        // enable RTC write protection
        self.rtc.wpr.write(|w| w.key().bits(0xFF));
    }

    /// Initialize the RTC clock
    pub fn init(&mut self) {
        self.initialization_mode(|rtc| {
            // program prescaler (if needed)
            // since the default is okay if using a 32.768 kHz crystal we do not need to do anything

            // configure time format
            rtc.cr.modify(|_, w| w.fmt().twenty_four_hour());
        });
    }

    pub fn is_set(&self) -> bool {
        self.rtc.isr.read().inits().bit_is_set()
    }

    pub fn get_state(&self) -> ClockState {
        let tr = self.rtc.tr.read();
        let dr = self.rtc.dr.read();

        ClockState {
            hour: tr.ht().bits() * 10 + tr.hu().bits(),
            minute: tr.mnt().bits() * 10 + tr.mnu().bits(),
            second: tr.st().bits() * 10 + tr.su().bits(),
            weekday: dr.wdu().bits(),
            day: dr.dt().bits() * 10 + dr.du().bits(),
            month: if dr.mt().bits() { 10 } else { 0 } + dr.mu().bits(),
            year: dr.yt().bits() * 10 + dr.yu().bits(),
        }
    }

    pub fn set_state(&mut self, state: ClockState) {
        self.initialization_mode(|rtc| {
            rtc.tr.modify(|_, w| {
                w.ht()
                    .bits(state.hour / 10)
                    .hu()
                    .bits(state.hour % 10)
                    .mnt()
                    .bits(state.minute / 10)
                    .mnu()
                    .bits(state.minute % 10)
                    .st()
                    .bits(state.second / 10)
                    .su()
                    .bits(state.second % 10)
            });
            rtc.dr.modify(|_, w| {
                w.dt()
                    .bits(state.day / 10)
                    .du()
                    .bits(state.day % 10)
                    .mt()
                    .bit(state.month >= 10)
                    .mu()
                    .bits(state.month % 10)
                    .yt()
                    .bits(state.year / 10)
                    .yu()
                    .bits(state.year % 10)
            });

            rtc.cr.modify(|_, w| w.fmt().twenty_four_hour());
        })
    }
}
