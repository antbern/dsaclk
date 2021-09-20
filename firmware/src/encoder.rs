use hal::{
    gpio::{
        gpiob::{PB14, PB4, PB5},
        Alternate, AF2,
    },
    pac::TIM3,
    qei::Qei,
};
use hal::{
    gpio::{Input, PullDown},
    prelude::*,
};
use stm32f4xx_hal as hal;

pub enum Button {
    ShortPress,
    LongPress,
}

pub type Pb4Af2 = PB4<Alternate<AF2>>;
pub type Pb5Af2 = PB5<Alternate<AF2>>;

pub struct Encoder {
    qei: Qei<TIM3, (Pb4Af2, Pb5Af2)>,
    last_count: u16,
    btn_pin: PB14<Input<PullDown>>,
    btn_count: u32,
}

impl Encoder {
    pub fn new(tim3: TIM3, pb4: Pb4Af2, pb5: Pb5Af2, btn_pin: PB14<Input<PullDown>>) -> Self {
        let qei = hal::qei::Qei::new(tim3, (pb4, pb5));
        let last_count = qei.count();
        Encoder {
            qei,
            last_count,
            btn_pin,
            btn_count: 0,
        }
    }

    pub fn check(&mut self) -> Option<i8> {
        let count = self.qei.count();
        let diff = count.wrapping_sub(self.last_count) as i16;

        if diff.abs() >= 4 {
            self.last_count = count;
            Some((diff / 4) as i8)
        } else {
            None
        }
    }

    pub fn check_btn(&mut self, timeout: u32) -> Option<Button> {
        let pressed = self.btn_pin.is_high().unwrap();

        let evt = match (pressed, self.btn_count) {
            (false, cnt) if 0 < cnt && cnt < timeout => Some(Button::ShortPress),
            (true, cnt) if cnt < timeout => None,
            (true, cnt) if cnt == timeout => Some(Button::LongPress),
            (..) => None,
        };

        self.btn_count = match pressed {
            true => self.btn_count.saturating_add(1), // avoid overflow
            false => 0,
        };

        evt
    }
}
