pub mod list;

use l3d::mat::Material;
pub use list::*;
use rfw_backend::DeviceMaterial;
use rfw_math::*;
use std::fmt::Display;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct MaterialFlags {
    bits: u32,
}

impl Into<u32> for MaterialFlags {
    fn into(self) -> u32 {
        self.bits
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub enum MaterialProps {
    HasDiffuseMap = 0,
    HasNormalMap = 1,
    HasRoughnessMap = 2,
    HasMetallicMap = 3,
    HasEmissiveMap = 4,
    HasSheenMap = 5,
}

impl Default for MaterialFlags {
    fn default() -> Self {
        Self { bits: 0 }
    }
}

impl Display for MaterialFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MaterialFlags {{ HasDiffuseMap: {}, HasNormalMap: {}, HasRoughnessMap: {}, HasMetallicMap: {}, HasEmissiveMap: {}, HasSheenMap: {} }}",
            self.get(MaterialProps::HasDiffuseMap),
            self.get(MaterialProps::HasNormalMap),
            self.get(MaterialProps::HasRoughnessMap),
            self.get(MaterialProps::HasMetallicMap),
            self.get(MaterialProps::HasEmissiveMap),
            self.get(MaterialProps::HasSheenMap),
        )
    }
}

#[allow(dead_code)]
impl MaterialFlags {
    pub fn set(&mut self, prop: MaterialProps, value: bool) {
        let offset = prop as u32;
        if value {
            let mask = 1 << offset;
            self.bits |= mask;
        } else {
            let mask = !(1 << offset);
            self.bits &= mask;
        }
    }

    pub fn get(&self, prop: MaterialProps) -> bool {
        (self.bits.overflowing_shr(prop as u32).0 & 1) == 1
    }
}

pub trait Emissive {
    fn is_emissive(&self) -> bool;
}

impl Emissive for Material {
    fn is_emissive(&self) -> bool {
        let color: Vec3A = Vec4::from(self.color).into();
        color.cmpgt(Vec3A::one()).any()
    }
}
