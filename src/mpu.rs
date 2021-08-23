use core::fmt::Debug;
use embedded_hal::{
    blocking::delay::DelayMs,
    blocking::i2c::{Write, WriteRead},
};
use mpu6050::Mpu6050;

use crate::vec::Vec3f;

#[derive(Debug, Default, Clone)]
pub struct Measurement {
    acc_mean: Vec3f,
    temp_mean: f32,
    gyro_mag_max: f32,
}

#[derive(Default, Debug)]
pub struct CalibrationOffset {
    acc: Vec3f,
    gyro: Vec3f,
}
pub struct MPU<I> {
    mpu: Mpu6050<I>,
    measurement: Measurement,
    count: u32,
    samples: u32,
    calib: CalibrationOffset,
}

impl<I, E> MPU<I>
where
    I: Write<Error = E> + WriteRead<Error = E>,
    E: Debug,
{
    pub fn new<D: DelayMs<u8>>(
        i2c: I,
        delay: &mut D,
        samples: u32,
    ) -> Result<MPU<I>, mpu6050::Mpu6050Error<E>> {
        let mut mpu = Mpu6050::new(i2c);
        mpu.init(delay)?;

        mpu.set_dlpf(mpu6050::device::DLPF_CFG::_10_10)?;

        Ok(MPU {
            mpu,
            measurement: Measurement::default(),
            count: 0,
            samples,
            calib: CalibrationOffset::default(),
        })
    }
    pub fn tick(&mut self) -> Option<Measurement> {
        use micromath::F32Ext;

        // get temp
        self.measurement.temp_mean += self.mpu.get_temp().unwrap();

        // get gyro data, scaled with sensitivity
        let gyro: Vec3f = Vec3f::from(self.mpu.get_gyro().unwrap()) - self.calib.gyro;
        self.measurement.gyro_mag_max = f32::max(self.measurement.gyro_mag_max, gyro.len2());

        // get accelerometer data, scaled with sensitivity
        let acc: Vec3f = Vec3f::from(self.mpu.get_acc().unwrap()) - self.calib.acc;
        self.measurement.acc_mean += acc;

        self.count += 1;

        if self.count > self.samples {
            self.measurement.temp_mean /= self.count as f32;
            self.measurement.acc_mean /= self.count as f32;
            self.measurement.gyro_mag_max = self.measurement.gyro_mag_max.sqrt();

            let meas = self.measurement.clone();
            self.measurement = Measurement::default();
            self.count = 0;

            Some(meas)
        } else {
            None
        }
    }

    pub fn set_calibration(&mut self, c: CalibrationOffset) {
        self.calib = c
    }

    pub fn calibrate<D: DelayMs<u8>>(
        &mut self,
        delay: &mut D,
        interval: u8,
        count: u32,
    ) -> CalibrationOffset {
        let mut o = CalibrationOffset::default();

        for _ in 0..count {
            let gyro: Vec3f = self.mpu.get_gyro().unwrap().into();
            let acc: Vec3f = self.mpu.get_acc().unwrap().into();

            o.gyro += gyro;
            o.acc += acc;

            delay.delay_ms(interval);
        }

        o.gyro /= count as f32;
        o.acc /= count as f32;

        // correct for gravity (z-axis)
        o.acc.2 -= 1.0;

        o
    }
}
