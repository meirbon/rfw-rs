use std::ops::{Add, AddAssign, Sub, SubAssign, Mul, MulAssign, Div, DivAssign, Index, IndexMut};
use std::convert::Into;
use crate::math::*;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl All for Vec4 {
    fn all(a: f32) -> Self {
        Self {
            x: a,
            y: a,
            z: a,
            w: a,
        }
    }
}

impl DotProduct for Vec4 {
    fn dot(&self, other: &Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w
    }
}

impl Normalize for Vec4 {
    fn normalize(&self) -> Self {
        let r = 1.0 / self.length();
        Self {
            x: self.x * r,
            y: self.y * r,
            z: self.z * r,
            w: self.w * r,
        }
    }
}

impl Min for Vec4 {
    fn min(&self, other: &Self) -> Self {
        Self {
            x: self.x.min(other.x),
            y: self.y.min(other.y),
            z: self.z.min(other.z),
            w: self.w.min(other.w),
        }
    }
}

impl Max for Vec4 {
    fn max(&self, other: &Self) -> Self {
        Self {
            x: self.x.max(other.x),
            y: self.y.max(other.y),
            z: self.z.max(other.z),
            w: self.w.max(other.w),
        }
    }
}

impl Vec4 {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Vec4 {
        Vec4 { x, y, z, w }
    }

    pub fn new_single(f: f32) -> Vec4 {
        Vec4::new(f, f, f, f)
    }

    pub fn single(a: f32) -> Vec4 {
        Vec4 { x: a, y: a, z: a, w: a }
    }

    pub fn add(&self, other: &Vec4) -> Vec4 {
        Vec4 {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
            w: self.w + other.w,
        }
    }

    pub fn sub(&self, other: &Vec4) -> Vec4 {
        Vec4 {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
            w: self.w - other.w,
        }
    }

    pub fn mul(&self, other: &Vec4) -> Vec4 {
        Vec4 {
            x: self.x * other.x,
            y: self.y * other.y,
            z: self.z * other.z,
            w: self.w * other.w,
        }
    }

    pub fn div(&self, other: &Vec4) -> Vec4 {
        Vec4 {
            x: self.x / other.x,
            y: self.y / other.y,
            z: self.z / other.z,
            w: self.w / other.w,
        }
    }

    pub fn mul_f(&self, f: f32) -> Vec4 {
        Vec4 {
            x: self.x * f,
            y: self.y * f,
            z: self.z * f,
            w: self.w * f,
        }
    }

    pub fn div_f(&self, f: f32) -> Vec4 {
        Vec4 {
            x: self.x / f,
            y: self.y / f,
            z: self.z / f,
            w: self.w / f,
        }
    }

    pub fn dot(&self, other: &Vec4) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w
    }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w).sqrt()
    }

    pub fn squared_length(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }

    pub fn unit_vector(&self) -> Vec4 {
        let k = 1.0 / self.length();

        Vec4 {
            x: self.x * k,
            y: self.y * k,
            z: self.z * k,
            w: self.w * k,
        }
    }

    pub fn normalize(&self) -> Vec4 {
        let r = 1.0 / self.length();
        Vec4::new(self.x * r, self.y * r, self.z * r, self.w * r)
    }

    pub fn clamp(&self, min: f32, max: f32) -> Vec4 {
        Vec4 {
            x: self.x.min(max).max(min),
            y: self.y.min(max).max(min),
            z: self.z.min(max).max(min),
            w: self.w.min(max).max(min),
        }
    }

    pub fn sqrt(&self) -> Self {
        Self {
            x: self.x.sqrt(),
            y: self.y.sqrt(),
            z: self.z.sqrt(),
            w: self.w.sqrt(),
        }
    }
}

impl Add<Vec4> for Vec4 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Vec4 {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
            w: self.w + rhs.w,
        }
    }
}

impl Add<f32> for Vec4 {
    type Output = Self;

    fn add(self, rhs: f32) -> Self {
        Vec4 {
            x: self.x + rhs,
            y: self.y + rhs,
            z: self.z + rhs,
            w: self.w + rhs,
        }
    }
}

impl AddAssign<Vec4> for Vec4 {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
        self.w += rhs.w;
    }
}

impl AddAssign<f32> for Vec4 {
    fn add_assign(&mut self, rhs: f32) {
        self.x += rhs;
        self.y += rhs;
        self.z += rhs;
        self.w += rhs;
    }
}

impl Sub<Vec4> for Vec4 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Vec4 {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
            w: self.w - rhs.w,
        }
    }
}

impl Sub<f32> for Vec4 {
    type Output = Self;

    fn sub(self, rhs: f32) -> Self {
        Vec4 {
            x: self.x - rhs,
            y: self.y - rhs,
            z: self.z - rhs,
            w: self.w - rhs,
        }
    }
}

impl SubAssign<Vec4> for Vec4 {
    fn sub_assign(&mut self, rhs: Vec4) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
        self.w -= rhs.w;
    }
}

impl SubAssign<f32> for Vec4 {
    fn sub_assign(&mut self, rhs: f32) {
        self.x -= rhs;
        self.y -= rhs;
        self.z -= rhs;
        self.w -= rhs;
    }
}

impl Mul<Vec4> for Vec4 {
    type Output = Self;

    fn mul(self, rhs: Vec4) -> Self {
        Vec4 {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
            z: self.z * rhs.z,
            w: self.w * rhs.w,
        }
    }
}

impl Mul<f32> for Vec4 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Vec4 {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
            w: self.w * rhs,
        }
    }
}

impl MulAssign<Vec4> for Vec4 {
    fn mul_assign(&mut self, rhs: Vec4) {
        self.x *= rhs.x;
        self.y *= rhs.y;
        self.z *= rhs.z;
        self.w *= rhs.w
    }
}

impl MulAssign<f32> for Vec4 {
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
        self.w *= rhs;
    }
}

impl Div<Vec4> for Vec4 {
    type Output = Self;

    fn div(self, rhs: Vec4) -> Self {
        Vec4 {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
            z: self.z / rhs.z,
            w: self.w / rhs.w,
        }
    }
}

impl Div<f32> for Vec4 {
    type Output = Self;

    fn div(self, rhs: f32) -> Self {
        Vec4 {
            x: self.x / rhs,
            y: self.y / rhs,
            z: self.z / rhs,
            w: self.w / rhs,
        }
    }
}

impl DivAssign<Vec4> for Vec4 {
    fn div_assign(&mut self, rhs: Vec4) {
        self.x /= rhs.x;
        self.y /= rhs.y;
        self.z /= rhs.z;
        self.w /= rhs.w;
    }
}

impl DivAssign<f32> for Vec4 {
    fn div_assign(&mut self, rhs: f32) {
        self.x /= rhs;
        self.y /= rhs;
        self.z /= rhs;
        self.w /= rhs;
    }
}

impl Index<usize> for Vec4 {
    type Output = f32;

    fn index(&self, index: usize) -> &<Self as Index<usize>>::Output {
        match index {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            3 => &self.w,
            _ => panic!("Index out of bounds"),
        }
    }
}

impl IndexMut<usize> for Vec4 {
    fn index_mut(&mut self, index: usize) -> &mut <Self as Index<usize>>::Output {
        match index {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            3 => &mut self.w,
            _ => panic!("Index out of bounds"),
        }
    }
}

impl Into<Vec3> for Vec4 {
    fn into(self) -> Vec3 {
        Vec3 {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }
}

impl Into<Vec2> for Vec4 {
    fn into(self) -> Vec2 {
        Vec2 {
            x: self.x,
            y: self.y,
        }
    }
}