use hal::prelude::*;
use hal::{
    gpio::{
        gpiob::{PB4, PB5},
        Alternate, AF2,
    },
    pac::TIM3,
    qei::Qei,
};
use stm32f4xx_hal as hal;

pub struct Encoder {
    qei: Qei<TIM3, (PB4<Alternate<AF2>>, PB5<Alternate<AF2>>)>,
    last_count: u16,
}

impl Encoder {
    pub fn new(tim3: TIM3, pb4: PB4<Alternate<AF2>>, pb5: PB5<Alternate<AF2>>) -> Self {
        let qei = hal::qei::Qei::new(tim3, (pb4, pb5));
        let last_count = qei.count();
        Encoder { qei, last_count }
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
}
