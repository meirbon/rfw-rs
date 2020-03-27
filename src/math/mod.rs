pub mod mat3;
pub mod mat4;
pub mod vec2;
pub mod vec3;
pub mod vec4;

pub use mat3::Mat3;
pub use mat4::Mat4;
pub use vec2::Vec2;
pub use vec3::Vec3;
pub use vec4::Vec4;

pub fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

pub fn vec3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

pub fn vec4(x: f32, y: f32, z: f32, w: f32) -> Vec4 {
    Vec4::new(x, y, z, w)
}

pub trait All {
    fn all(a: f32) -> Self;
}

pub fn all<T>(a: f32) -> T where T: All {
    T::all(a)
}

pub fn zero<T>() -> T where T: All {
    T::all(0.0)
}

pub trait DotProduct {
    fn dot(&self, other: &Self) -> f32;
}

pub fn dot<T>(a: T, b: T) -> f32 where T: DotProduct {
    a.dot(&b)
}

pub trait CrossProduct {
    fn cross(&self, other: &Self) -> Self;
}

pub fn cross<T>(a: T, b: T) -> T where T: CrossProduct {
    a.cross(&b)
}

pub trait Normalize {
    fn normalize(&self) -> Self;
}

pub fn normalize<T>(a: T) -> T where T: Normalize {
    a.normalize()
}

fn length<T>(a: T) -> f32 where T: DotProduct {
    a.dot(&a).sqrt()
}

fn squared_length<T>(a: T) -> f32 where T: DotProduct {
    a.dot(&a)
}

pub trait Sqrt {
    fn sqrt(&self) -> Self;
}

pub fn sqrt<T>(a: T) -> T where T: Sqrt {
    a.sqrt()
}

pub fn clamp<T>(a: T, min: f32, max: f32) -> T where T: Min + Max + All {
    a.max(&T::all(min)).min(&T::all(max))
}

pub trait Min {
    fn min(&self, other: &Self) -> Self;
}

pub trait Max {
    fn max(&self, other: &Self) -> Self;
}

pub fn min<T>(a: T, b: T) -> T where T: Min {
    a.min(&b)
}

pub fn max<T>(a: T, b: T) -> T where T: Max {
    a.max(&b)
}

