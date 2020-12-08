pub mod list;

pub use list::*;
use std::fmt::Display;

use rfw_utils::prelude::{l3d::mat::Material, Vec3A, Vec4};
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

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
#[repr(C)]
pub struct DeviceMaterial {
    pub color: [f32; 4],
    // 16
    pub absorption: [f32; 4],
    // 32
    pub specular: [f32; 4],
    // 48
    pub parameters: [u32; 4], // 64

    pub flags: u32,
    // 68
    pub diffuse_map: i32,
    // 72
    pub normal_map: i32,
    // 76
    pub metallic_roughness_map: i32, // 80

    pub emissive_map: i32,
    // 84
    pub sheen_map: i32,
    // 88
    pub _dummy: [i32; 2], // 96
}

impl Default for DeviceMaterial {
    fn default() -> Self {
        Self {
            color: [0.0; 4],
            absorption: [0.0; 4],
            specular: [0.0; 4],
            parameters: [0; 4],
            flags: 0,
            diffuse_map: -1,
            normal_map: -1,
            metallic_roughness_map: -1,
            emissive_map: -1,
            sheen_map: -1,
            _dummy: [0; 2],
        }
    }
}

impl DeviceMaterial {
    pub fn get_metallic(&self) -> f32 {
        (self.parameters[0] & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_subsurface(&self) -> f32 {
        ((self.parameters[0].overflowing_shr(8)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_specular_f(&self) -> f32 {
        ((self.parameters[0].overflowing_shr(16)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_roughness(&self) -> f32 {
        ((self.parameters[0].overflowing_shr(24)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_specular_tint(&self) -> f32 {
        (self.parameters[1] & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_anisotropic(&self) -> f32 {
        ((self.parameters[1].overflowing_shr(8)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_sheen(&self) -> f32 {
        ((self.parameters[1].overflowing_shr(16)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_sheen_tint(&self) -> f32 {
        ((self.parameters[1].overflowing_shr(24)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_clearcoat(&self) -> f32 {
        (self.parameters[2] & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_clearcoat_gloss(&self) -> f32 {
        ((self.parameters[2].overflowing_shr(8)).0 & 255) as f32 * 1.0 / 255.0
    }
    pub fn get_transmission(&self) -> f32 {
        ((self.parameters[2].overflowing_shr(8)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_eta(&self) -> f32 {
        ((self.parameters[2].overflowing_shr(24)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_custom0(&self) -> f32 {
        (self.parameters[3] & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_custom1(&self) -> f32 {
        ((self.parameters[3].overflowing_shr(8)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_custom2(&self) -> f32 {
        ((self.parameters[3].overflowing_shr(8)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_custom3(&self) -> f32 {
        ((self.parameters[3].overflowing_shr(24)).0 & 255) as f32 * 1.0 / 255.0
    }
}

impl Into<DeviceMaterial> for &Material {
    fn into(self) -> DeviceMaterial {
        let to_char = |f: f32| -> u8 { (f * 255.0).min(255.0) as u8 };
        let to_u32 = |a: f32, b: f32, c: f32, d: f32| -> u32 {
            let a = to_char(a) as u32;
            let b = to_char(b) as u32;
            let c = to_char(c) as u32;
            let d = to_char(d) as u32;

            a | (b << 8) | (c << 16) | (d << 24)
        };

        let parameters: [u32; 4] = [
            to_u32(
                self.metallic,
                self.subsurface,
                self.specular_f,
                self.roughness,
            ),
            to_u32(
                self.specular_tint,
                self.anisotropic,
                self.sheen,
                self.sheen_tint,
            ),
            to_u32(
                self.clearcoat,
                self.clearcoat_gloss,
                self.transmission,
                self.eta,
            ),
            to_u32(self.custom0, self.custom1, self.custom2, self.custom3),
        ];

        let mut flags = MaterialFlags::default();
        if self.diffuse_tex >= 0 {
            flags.set(MaterialProps::HasDiffuseMap, true);
        }
        if self.normal_tex >= 0 {
            flags.set(MaterialProps::HasNormalMap, true);
        }
        if self.metallic_roughness_tex >= 0 {
            flags.set(MaterialProps::HasRoughnessMap, true);
            flags.set(MaterialProps::HasMetallicMap, true);
        }
        if self.emissive_tex >= 0 {
            flags.set(MaterialProps::HasEmissiveMap, true);
        }
        if self.sheen_tex >= 0 {
            flags.set(MaterialProps::HasSheenMap, true);
        }

        DeviceMaterial {
            color: self.color,
            absorption: self.absorption,
            specular: self.specular,
            parameters,
            flags: flags.into(),
            diffuse_map: self.diffuse_tex as i32,
            normal_map: self.normal_tex as i32,
            metallic_roughness_map: self.metallic_roughness_tex as i32,
            emissive_map: self.emissive_tex as i32,
            sheen_map: self.sheen_tex as i32,
            ..Default::default()
        }
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
