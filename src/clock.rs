use stm32f4xx_hal::stm32 as stm32f401;

pub struct Clock {
    rtc: stm32f401::RTC,
}

impl Clock {
    pub fn new(reg: stm32f401::RTC) -> Clock {
        Clock { rtc: reg }
    }

    pub fn init(&self) -> () {
        // Initialize the RTC clock
    }

    pub fn is_set(&self) -> bool {
        self.rtc.isr.read().inits().bit_is_set()
    }
}
