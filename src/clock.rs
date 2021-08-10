use cortex_m::interrupt::free;
use stm32f4xx_hal::{interrupt, stm32 as stm32f401};

use crate::event::InterruptEvent;
use crate::EVENT_QUEUE;

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

#[derive(Debug, Clone, Copy)]
pub struct AlarmState {
    pub hour: u8,
    pub minute: u8,
    pub enabled: bool,
}

pub struct Clock {
    rtc: stm32f401::RTC,
}

impl Clock {
    pub fn new(reg: stm32f401::RTC) -> Clock {
        Clock { rtc: reg }
    }

    // disables the write protection of the RTC registers for manipulation and then enables it again
    fn protected<F>(&mut self, f: F)
    where
        F: FnOnce(&mut stm32f401::RTC),
    {
        // Disable RTC write protection
        self.rtc.wpr.write(|w| w.key().bits(0xCA));
        self.rtc.wpr.write(|w| w.key().bits(0x53));

        f(&mut self.rtc);

        // enable RTC write protection
        self.rtc.wpr.write(|w| w.key().bits(0xFF));
    }

    fn initialization_mode<F>(&mut self, f: F)
    where
        F: FnOnce(&mut stm32f401::RTC),
    {
        self.protected(|rtc| {
            // Enter initialization mode
            rtc.isr.modify(|_, w| w.init().init_mode());

            // wait for confirmation
            while rtc.isr.read().initf().is_not_allowed() {}

            // we can now safely initialize the RTC
            f(rtc);

            // exit initialization mode
            rtc.isr.modify(|_, w| w.init().free_running_mode());
        });
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

    pub fn get_alarm(&self) -> AlarmState {
        let cr = self.rtc.cr.read();
        let alrmar = self.rtc.alrmar.read();
        AlarmState {
            hour: alrmar.ht().bits() * 10 + alrmar.hu().bits(),
            minute: alrmar.mnt().bits() * 10 + alrmar.mnu().bits(),
            enabled: cr.alrae().is_enabled(),
        }
    }

    pub fn set_alarm(&mut self, alarm: AlarmState) {
        self.protected(|rtc| {
            // disable alarm A
            rtc.cr.modify(|_, w| w.alrae().disabled());

            // wait for confirmation
            while rtc.isr.read().alrawf().is_update_not_allowed() {}

            // configure alarm A
            rtc.alrmar.modify(|_, w| {
                w.msk1()
                    .mask() // care about seconds
                    .msk2()
                    .mask() // care about minutes
                    .msk3()
                    .mask() // care about hours
                    .msk4()
                    .not_mask() // do not care about date/week day
                    .pm()
                    .am() // AM/24 hour format
                    .ht()
                    .bits(alarm.hour / 10) // set hour
                    .hu()
                    .bits(alarm.hour % 10)
                    .mnt()
                    .bits(alarm.minute / 10) // set minute
                    .mnu()
                    .bits(alarm.minute % 10)
                    .st()
                    .bits(0) // set seconds to zero to match on new minute
                    .su()
                    .bits(0)
            });

            // re-enable alarm A (if enabled)
            rtc.cr.modify(|_, w| {
                w.alrae().variant(match alarm.enabled {
                    false => stm32f401::rtc::cr::ALRAE_A::DISABLED,
                    true => stm32f401::rtc::cr::ALRAE_A::ENABLED,
                })
            });
        });
    }

    pub fn alarm_triggered(&self) -> bool {
        self.rtc.isr.read().alraf().is_match_()
    }

    pub fn alarm_reset(&mut self) {
        self.rtc.isr.modify(|_, w| w.alraf().clear())
    }

    pub fn enable_alarm_interrupt(&mut self, exti: &stm32f401::EXTI) {
        // According to page 449 of the STM32F401xDE reference manual

        // enable EXTI Line 17 in interrupt mode and select rising edge sensitivity
        exti.imr.modify(|_, w| w.mr17().unmasked());
        exti.rtsr.modify(|_, w| w.tr17().enabled());

        // enable RTC alarm A interrupt
        self.protected(|rtc| rtc.cr.modify(|_, w| w.alraie().enabled()));

        // enable RTC Alarm interrupt in the NVIC
        stm32f401::NVIC::unpend(stm32f4xx_hal::interrupt::RTC_ALARM);
        unsafe {
            stm32f401::NVIC::unmask(stm32f4xx_hal::interrupt::RTC_ALARM);
        };
    }
}

#[interrupt]
fn RTC_ALARM() {
    free(|cs| {
        // SAFETY only used to reset the interrupt pending bit atomically with no side effects
        unsafe {
            (*stm32f401::EXTI::ptr()).pr.write(|w| w.pr17().set_bit());
        }

        EVENT_QUEUE.put(cs, InterruptEvent::Alarm);
    });
}
