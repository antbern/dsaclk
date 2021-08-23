use core::{
    fmt::Debug,
    ops::{Add, AddAssign, Div, DivAssign, Sub, SubAssign},
};

#[derive(Default, Clone, Copy, Debug)]
pub struct Vec3f(pub f32, pub f32, pub f32);

impl Vec3f {
    pub fn len2(&self) -> f32 {
        self.0 * self.0 + self.1 * self.1 + self.2 * self.2
    }
}

impl AddAssign for Vec3f {
    fn add_assign(&mut self, other: Self) {
        *self = Vec3f(self.0 + other.0, self.1 + other.1, self.2 + other.2)
    }
}

impl Add for Vec3f {
    type Output = Self;

    fn add(self, other: Self) -> Vec3f {
        Vec3f(self.0 + other.0, self.1 + other.1, self.2 + other.2)
    }
}

impl Sub for Vec3f {
    type Output = Self;

    fn sub(self, other: Vec3f) -> Vec3f {
        Vec3f(self.0 - other.0, self.1 - other.1, self.2 - other.2)
    }
}

impl SubAssign for Vec3f {
    fn sub_assign(&mut self, other: Self) {
        *self = Vec3f(self.0 - other.0, self.1 - other.1, self.2 - other.2)
    }
}

impl Div<f32> for Vec3f {
    type Output = Self;

    fn div(self, rhs: f32) -> Vec3f {
        Vec3f(self.0 / rhs, self.1 / rhs, self.2 / rhs)
    }
}

impl DivAssign<f32> for Vec3f {
    fn div_assign(&mut self, rhs: f32) {
        *self = Vec3f(self.0 / rhs, self.1 / rhs, self.2 / rhs)
    }
}

impl From<nalgebra::Vector3<f32>> for Vec3f {
    fn from(d: nalgebra::Vector3<f32>) -> Self {
        Vec3f(d.x, d.y, d.z)
    }
}
