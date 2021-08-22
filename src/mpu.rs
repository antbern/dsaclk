use core::fmt::Debug;

use heapless::pool::Init;
use mpu6050::Mpu6050;

use embedded_hal::{
    blocking::delay::DelayMs,
    blocking::i2c::{Write, WriteRead},
};

#[derive(Debug, Default, Clone)]
pub struct Measurement {
    acc_mean: (f32, f32, f32),
    temp_mean: f32,
    gyro_mag_max: f32,
}
pub struct MPU<I> {
    mpu: Mpu6050<I>,
    measurement: Measurement,
    count: u32,
    samples: u32,
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

        Ok(MPU {
            mpu,
            measurement: Measurement::default(),
            count: 0,
            samples,
        })
    }
    pub fn tick(&mut self) -> Option<Measurement> {
        use micromath::F32Ext;

        // get temp
        self.measurement.temp_mean += self.mpu.get_temp().unwrap();

        // get gyro data, scaled with sensitivity
        let gyro = self.mpu.get_gyro().unwrap();
        self.measurement.gyro_mag_max = f32::max(
            self.measurement.gyro_mag_max,
            gyro.x * gyro.x + gyro.y * gyro.y + gyro.x * gyro.z,
        );

        // get accelerometer data, scaled with sensitivity
        let acc = self.mpu.get_acc().unwrap();
        self.measurement.acc_mean = (
            self.measurement.acc_mean.0 + acc.x,
            self.measurement.acc_mean.1 + acc.y,
            self.measurement.acc_mean.2 + acc.z,
        );

        self.count += 1;

        if self.count > self.samples {
            self.measurement.temp_mean /= self.count as f32;
            self.measurement.acc_mean = (
                self.measurement.acc_mean.0 / self.count as f32,
                self.measurement.acc_mean.1 / self.count as f32,
                self.measurement.acc_mean.2 / self.count as f32,
            );
            self.measurement.gyro_mag_max = self.measurement.gyro_mag_max.sqrt();

            let meas = self.measurement.clone();
            self.measurement = Measurement::default();
            self.count = 0;

            Some(meas)
        } else {
            None
        }
    }
}
