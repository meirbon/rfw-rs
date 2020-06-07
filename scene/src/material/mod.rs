pub mod list;

pub use list::*;

use glam::*;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct MaterialFlags {
    bits: u32,
}

impl Into<u32> for MaterialFlags {
    fn into(self) -> u32 {
        self.bits
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
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

#[repr(C)]
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Material {
    pub color: [f32; 4],    // 16
    pub specular: [f32; 4], // 32
    pub metallic: f32,
    pub subsurface: f32,
    pub specular_f: f32,
    pub roughness: f32,
    pub specular_tint: f32,
    pub anisotropic: f32,
    pub sheen: f32,
    pub sheen_tint: f32,
    pub clearcoat: f32,
    pub clearcoat_gloss: f32,
    pub transmission: f32,
    pub eta: f32,
    pub custom0: f32,
    pub custom1: f32,
    pub custom2: f32,
    pub custom3: f32,
    pub diffuse_tex: i16,
    pub normal_tex: i16,
    pub roughness_tex: i16,
    pub metallic_tex: i16,
    pub emissive_tex: i16,
    pub sheen_tex: i16,
}

impl Display for Material {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Material {{ color: {}, specular: {}, metallic: {}, subsurface: {}, specular_f: {}, roughness: {}, specular_tint: {}, anisotropic: {}, sheen: {}, sheen_tint: {}, clearcoat: {}, clearcoat_gloss: {}, transmission: {}, eta: {}, custom0: {}, custom1: {}, custom2: {}, custom3: {}, diffuse_tex: {}, normal_tex: {}, roughness_tex: {}, metallic_tex: {}, emissive_tex: {}, sheen_tex: {} }}",
            Vec4::from(self.color),
            Vec4::from(self.specular),
            self.metallic,
            self.subsurface,
            self.specular_f,
            self.roughness,
            self.specular_tint,
            self.anisotropic,
            self.sheen,
            self.sheen_tint,
            self.clearcoat,
            self.clearcoat_gloss,
            self.transmission,
            self.eta,
            self.custom0,
            self.custom1,
            self.custom2,
            self.custom3,
            self.diffuse_tex,
            self.normal_tex,
            self.roughness_tex,
            self.metallic_tex,
            self.emissive_tex,
            self.sheen_tex,
        )
    }
}

#[derive(Debug, Clone)]
pub struct DeviceMaterial {
    pub color: [f32; 4],      // 16
    pub specular: [f32; 4],   // 32
    pub parameters: [u32; 4], // 48
    pub flags: u32,           // 52
    _padding: [u32; 3],       // 64
}

impl Into<DeviceMaterial> for &Material {
    fn into(self) -> DeviceMaterial {
        let to_char = |f: f32| -> u8 { (f.min(1.0).max(0.0) * 255.0) as u8 };
        let to_u32 = |a: f32, b: f32, c: f32, d: f32| -> u32 {
            let a = to_char(a) as u32;
            let b = to_char(b) as u32;
            let c = to_char(c) as u32;
            let d = to_char(d) as u32;

            a + (b << 8) + (c << 16) + (d << 24)
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
        if self.roughness_tex >= 0 {
            flags.set(MaterialProps::HasRoughnessMap, true);
        }
        if self.metallic_tex >= 0 {
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
            specular: self.specular,
            parameters,
            flags: flags.into(),
            _padding: [0; 3],
        }
    }
}

impl Default for Material {
    fn default() -> Self {
        Self {
            color: [1.0; 4],
            specular: [1.0; 4],
            metallic: 0.0,
            subsurface: 0.0,
            specular_f: 0.5,
            roughness: 0.0,

            specular_tint: 0.0,
            anisotropic: 0.0,
            sheen: 0.0,
            sheen_tint: 0.0,

            clearcoat: 0.0,
            clearcoat_gloss: 1.0,
            transmission: 0.0,
            eta: 1.0,

            custom0: 0.0,
            custom1: 0.0,
            custom2: 0.0,
            custom3: 0.0,

            diffuse_tex: -1,
            normal_tex: -1,
            roughness_tex: -1,
            metallic_tex: -1,
            emissive_tex: -1,
            sheen_tex: -1,
        }
    }
}

impl Material {
    pub fn is_emissive(&self) -> bool {
        let color: Vec3 = Vec4::from(self.color).truncate();
        color.cmpgt(Vec3::one()).any()
    }
}
