use crate::{MaterialFlags, MaterialProps};
use bitvec::prelude::*;
use l3d::mat::{Flip, Material, Texture, TextureSource};
use rfw_backend::DeviceMaterial;
use rfw_math::*;
use rfw_utils::collections::{
    ChangedIterator, FlaggedIterator, FlaggedIteratorMut, FlaggedStorage, TrackedStorage,
};
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::ops::{Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Sub, SubAssign};
use std::path::{Path, PathBuf};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct MaterialList {
    light_flags: bitvec::prelude::BitVec,
    materials: TrackedStorage<Material>,
    device_materials: TrackedStorage<DeviceMaterial>,
    tex_path_mapping: HashMap<PathBuf, usize>,
    textures: TrackedStorage<Texture>,
    tex_material_mapping: FlaggedStorage<HashSet<u32>>,
}

impl Display for MaterialList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MaterialList: {{ lights_materials: {}, materials: {}, textures: {} }}",
            self.light_flags.count_ones(),
            self.materials.len(),
            self.textures.len()
        )
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TextureFormat {
    R,
    RG,
    RGB,
    RGBA,
    BGR,
    BGRA,
    R16,
    RG16,
    RGB16,
    RGBA16,
}

#[derive(Debug, Copy, Clone)]
pub struct Pixel {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for Pixel {
    fn default() -> Self {
        Self {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        }
    }
}

impl From<[f32; 4]> for Pixel {
    fn from(c: [f32; 4]) -> Self {
        Self {
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
        }
    }
}

impl Into<[f32; 4]> for Pixel {
    fn into(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

impl Pixel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn zero() -> Self {
        Self::from([0.0; 4])
    }

    pub fn one() -> Self {
        Self::from([1.0; 4])
    }

    pub fn from_bgra(pixel: u32) -> Self {
        let r = pixel.overflowing_shr(16).0 & 255;
        let g = pixel.overflowing_shr(8).0 & 255;
        let b = pixel & 255;
        let a = pixel.overflowing_shr(24).0 & 255;

        let r = r as f32 / 255.0;
        let g = g as f32 / 255.0;
        let b = b as f32 / 255.0;
        let a = a as f32 / 255.0;

        Self { r, g, b, a }
    }

    pub fn from_rgba(pixel: u32) -> Self {
        let r = pixel.overflowing_shr(0).0 & 255;
        let g = pixel.overflowing_shr(8).0 & 255;
        let b = pixel.overflowing_shr(16).0 & 255;
        let a = pixel.overflowing_shr(24).0 & 255;

        let r = r as f32 / 255.0;
        let g = g as f32 / 255.0;
        let b = b as f32 / 255.0;
        let a = a as f32 / 255.0;

        Self { r, g, b, a }
    }

    pub fn scaled(self, factor: f32) -> Self {
        Self {
            r: self.r * factor,
            g: self.g * factor,
            b: self.b * factor,
            a: self.a * factor,
        }
    }

    pub fn sqrt(self) -> Self {
        Self {
            r: if self.r <= 0.0 { 0.0 } else { self.r.sqrt() },
            g: if self.g <= 0.0 { 0.0 } else { self.g.sqrt() },
            b: if self.b <= 0.0 { 0.0 } else { self.b.sqrt() },
            a: if self.a <= 0.0 { 0.0 } else { self.a.sqrt() },
        }
    }

    pub fn pow(self, exp: f32) -> Self {
        Self {
            r: if self.r <= 0.0 { 0.0 } else { self.r.powf(exp) },
            g: if self.g <= 0.0 { 0.0 } else { self.g.powf(exp) },
            b: if self.b <= 0.0 { 0.0 } else { self.b.powf(exp) },
            a: if self.a <= 0.0 { 0.0 } else { self.a.powf(exp) },
        }
    }
}

impl Add<Pixel> for Pixel {
    type Output = Self;

    fn add(self, rhs: Pixel) -> Self::Output {
        Self {
            r: self.r + rhs.r,
            g: self.g + rhs.g,
            b: self.b + rhs.b,
            a: self.a + rhs.a,
        }
    }
}

impl Sub<Pixel> for Pixel {
    type Output = Self;

    fn sub(self, rhs: Pixel) -> Self::Output {
        Self {
            r: self.r - rhs.r,
            g: self.g - rhs.g,
            b: self.b - rhs.b,
            a: self.a - rhs.a,
        }
    }
}

impl Div<Pixel> for Pixel {
    type Output = Self;

    fn div(self, rhs: Pixel) -> Self::Output {
        Self {
            r: self.r / rhs.r,
            g: self.g / rhs.g,
            b: self.b / rhs.b,
            a: self.a / rhs.a,
        }
    }
}

impl Mul<Pixel> for Pixel {
    type Output = Self;

    fn mul(self, rhs: Pixel) -> Self::Output {
        Self {
            r: self.r * rhs.r,
            g: self.g * rhs.g,
            b: self.b * rhs.b,
            a: self.a * rhs.a,
        }
    }
}

impl AddAssign<Pixel> for Pixel {
    fn add_assign(&mut self, rhs: Pixel) {
        self.r += rhs.r;
        self.g += rhs.g;
        self.b += rhs.b;
        self.a += rhs.a;
    }
}

impl SubAssign<Pixel> for Pixel {
    fn sub_assign(&mut self, rhs: Pixel) {
        self.r -= rhs.r;
        self.g -= rhs.g;
        self.b -= rhs.b;
        self.a -= rhs.a;
    }
}

impl DivAssign<Pixel> for Pixel {
    fn div_assign(&mut self, rhs: Pixel) {
        self.r /= rhs.r;
        self.g /= rhs.g;
        self.b /= rhs.b;
        self.a /= rhs.a;
    }
}

impl MulAssign<Pixel> for Pixel {
    fn mul_assign(&mut self, rhs: Pixel) {
        self.r *= rhs.r;
        self.g *= rhs.g;
        self.b *= rhs.b;
        self.a *= rhs.a;
    }
}

impl Index<usize> for Pixel {
    type Output = f32;

    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.r,
            1 => &self.g,
            2 => &self.b,
            3 => &self.a,
            _ => panic!("invalid index {}", index),
        }
    }
}

impl IndexMut<usize> for Pixel {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match index {
            0 => &mut self.r,
            1 => &mut self.g,
            2 => &mut self.b,
            3 => &mut self.a,
            _ => panic!("invalid index {}", index),
        }
    }
}

impl Default for MaterialList {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct TextureDescriptor {
    pub albedo: Option<TextureSource>,
    pub normal: Option<TextureSource>,
    pub metallic_roughness_map: Option<TextureSource>,
    pub emissive_map: Option<TextureSource>,
    pub sheen_map: Option<TextureSource>,
}

impl TextureDescriptor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_albedo(mut self, tex: TextureSource) -> Self {
        self.albedo = Some(tex);
        self
    }

    pub fn with_normal(mut self, tex: TextureSource) -> Self {
        self.normal = Some(tex);
        self
    }

    pub fn with_metallic_roughness(mut self, tex: TextureSource) -> Self {
        self.metallic_roughness_map = Some(tex);
        self
    }

    pub fn with_metallic_roughness_maps(
        mut self,
        metallic: TextureSource,
        roughness: TextureSource,
    ) -> Self {
        let metallic = match metallic {
            TextureSource::Loaded(t) => t,
            TextureSource::Filesystem(path, flip) => Texture::load(path, flip).unwrap(),
        };

        let roughness = match roughness {
            TextureSource::Loaded(t) => t,
            TextureSource::Filesystem(path, flip) => Texture::load(path, flip).unwrap(),
        };

        let tex = Texture::merge(Some(&metallic), Some(&roughness), None, None);

        self.metallic_roughness_map = Some(TextureSource::Loaded(tex));
        self
    }

    pub fn with_emissive(mut self, tex: TextureSource) -> Self {
        self.emissive_map = Some(tex);
        self
    }

    pub fn with_sheen(mut self, tex: TextureSource) -> Self {
        self.sheen_map = Some(tex);
        self
    }
}

impl Default for TextureDescriptor {
    fn default() -> Self {
        Self {
            albedo: None,
            normal: None,
            metallic_roughness_map: None,
            emissive_map: None,
            sheen_map: None,
        }
    }
}

#[allow(dead_code)]
impl MaterialList {
    // Creates an empty material list
    pub fn empty() -> MaterialList {
        MaterialList {
            light_flags: bitvec::prelude::BitVec::new(),
            materials: TrackedStorage::new(),
            device_materials: TrackedStorage::new(),
            tex_path_mapping: HashMap::new(),
            textures: TrackedStorage::new(),
            tex_material_mapping: FlaggedStorage::new(),
        }
    }

    /// Creates a material list with at least a single (empty) texture and (empty) material
    pub fn new() -> MaterialList {
        let mut materials = TrackedStorage::new();
        materials.push(Material::default());

        // Make sure always a single texture exists (as fallback)
        let mut textures = TrackedStorage::new();
        let mut default = Texture::default();
        default.generate_mipmaps(Texture::MIP_LEVELS);
        textures.push(default);

        let mut light_flags = bitvec::prelude::BitVec::new();
        light_flags.push(false);

        MaterialList {
            light_flags,
            materials,
            device_materials: TrackedStorage::new(),
            tex_path_mapping: HashMap::new(),
            textures,
            tex_material_mapping: FlaggedStorage::new(),
        }
    }

    pub fn add<B: Into<[f32; 3]>>(
        &mut self,
        color: B,
        roughness: f32,
        specular: B,
        transmission: f32,
    ) -> usize {
        let material = Material {
            color: Vec3::from(color.into()).extend(1.0).into(),
            roughness,
            specular: Vec3::from(specular.into()).extend(1.0).into(),
            transmission,
            ..Material::default()
        };

        self.push(material)
    }

    pub fn add_with_maps(
        &mut self,
        color: Vec3,
        roughness: f32,
        specular: Vec3,
        transmission: f32,
        textures: TextureDescriptor,
    ) -> usize {
        let albedo = textures.albedo;
        let normal = textures.normal;
        let metallic_roughness_map = textures.metallic_roughness_map;
        let emissive_map = textures.emissive_map;
        let sheen_map = textures.sheen_map;

        let mut material = Material::default();
        material.color = color.extend(1.0).into();
        material.specular = specular.extend(1.0).into();
        material.roughness = roughness;
        material.transmission = transmission;

        let diffuse_tex = if let Some(albedo) = albedo {
            match albedo {
                TextureSource::Loaded(tex) => self.push_texture(tex) as i32,
                TextureSource::Filesystem(path, flip) => {
                    self.get_texture_index(&path, flip).unwrap_or_else(|_| -1)
                }
            }
        } else {
            -1
        };

        let normal_tex = if let Some(normal) = normal {
            match normal {
                TextureSource::Loaded(tex) => self.push_texture(tex) as i32,
                TextureSource::Filesystem(path, flip) => {
                    self.get_texture_index(&path, flip).unwrap_or_else(|_| -1)
                }
            }
        } else {
            -1
        };

        let metallic_roughness_tex = if let Some(metallic_roughness_map) = metallic_roughness_map {
            match metallic_roughness_map {
                TextureSource::Loaded(tex) => self.push_texture(tex) as i32,
                TextureSource::Filesystem(path, flip) => {
                    self.get_texture_index(&path, flip).unwrap_or_else(|_| -1)
                }
            }
        } else {
            -1
        };

        let emissive_tex = if let Some(emissive_map) = emissive_map {
            match emissive_map {
                TextureSource::Loaded(tex) => self.push_texture(tex) as i32,
                TextureSource::Filesystem(path, flip) => {
                    self.get_texture_index(&path, flip).unwrap_or_else(|_| -1)
                }
            }
        } else {
            -1
        };

        let sheen_tex = if let Some(sheen_map) = sheen_map {
            match sheen_map {
                TextureSource::Loaded(tex) => self.push_texture(tex) as i32,
                TextureSource::Filesystem(path, flip) => {
                    self.get_texture_index(&path, flip).unwrap_or_else(|_| -1)
                }
            }
        } else {
            -1
        };

        material.diffuse_tex = diffuse_tex as i16;
        material.normal_tex = normal_tex as i16;
        material.metallic_roughness_tex = metallic_roughness_tex as i16;
        material.emissive_tex = emissive_tex as i16;
        material.sheen_tex = sheen_tex as i16;

        self.push(material)
    }

    pub fn push(&mut self, mat: Material) -> usize {
        let i = self.materials.len();
        let is_light = Vec4::from(mat.color).truncate().cmpgt(Vec3::ONE).any();

        if mat.diffuse_tex >= 0 {
            self.tex_material_mapping[mat.diffuse_tex as usize].insert(i as u32);
        }
        if mat.normal_tex >= 0 {
            self.tex_material_mapping[mat.normal_tex as usize].insert(i as u32);
        }
        if mat.metallic_roughness_tex >= 0 {
            self.tex_material_mapping[mat.metallic_roughness_tex as usize].insert(i as u32);
        }
        if mat.emissive_tex >= 0 {
            self.tex_material_mapping[mat.emissive_tex as usize].insert(i as u32);
        }
        if mat.sheen_tex >= 0 {
            self.tex_material_mapping[mat.sheen_tex as usize].insert(i as u32);
        }

        self.light_flags.push(is_light);
        self.materials.push(mat);
        i
    }

    pub fn push_texture(&mut self, mut texture: Texture) -> usize {
        if texture.width < 64 || texture.height < 64 {
            texture = texture.resized(64.max(texture.width), 64.max(texture.height));
        }

        texture.generate_mipmaps(Texture::MIP_LEVELS);
        let i = self.textures.len();
        self.textures.push(texture);
        self.tex_material_mapping.overwrite_val(i, HashSet::new());
        i
    }

    pub fn get(&self, index: usize) -> Option<&Material> {
        self.materials.get(index)
    }

    pub fn get_mut<T: FnMut(Option<&mut Material>)>(&mut self, index: usize, mut cb: T) {
        if let Some(mat) = self.materials.get_mut(index) {
            if mat.diffuse_tex >= 0 {
                self.tex_material_mapping[mat.diffuse_tex as usize].remove(&(index as u32));
            }
            if mat.normal_tex >= 0 {
                self.tex_material_mapping[mat.normal_tex as usize].remove(&(index as u32));
            }
            if mat.metallic_roughness_tex >= 0 {
                self.tex_material_mapping[mat.metallic_roughness_tex as usize]
                    .remove(&(index as u32));
            }
            if mat.emissive_tex >= 0 {
                self.tex_material_mapping[mat.emissive_tex as usize].remove(&(index as u32));
            }
            if mat.sheen_tex >= 0 {
                self.tex_material_mapping[mat.sheen_tex as usize].remove(&(index as u32));
            }
        }

        cb(self.materials.get_mut(index));

        if let Some(mat) = self.materials.get_mut(index) {
            self.light_flags.set(
                index,
                Vec4::from(mat.color).truncate().cmpgt(Vec3::ONE).any(),
            );

            if mat.diffuse_tex >= 0 {
                self.tex_material_mapping[mat.diffuse_tex as usize].insert(index as u32);
            }
            if mat.normal_tex >= 0 {
                self.tex_material_mapping[mat.normal_tex as usize].insert(index as u32);
            }
            if mat.metallic_roughness_tex >= 0 {
                self.tex_material_mapping[mat.metallic_roughness_tex as usize].insert(index as u32);
            }
            if mat.emissive_tex >= 0 {
                self.tex_material_mapping[mat.emissive_tex as usize].insert(index as u32);
            }
            if mat.sheen_tex >= 0 {
                self.tex_material_mapping[mat.sheen_tex as usize].insert(index as u32);
            }
        }
    }

    pub unsafe fn get_unchecked(&self, index: usize) -> &Material {
        self.materials.get_unchecked(index)
    }

    pub unsafe fn get_unchecked_mut<T: FnMut(&mut Material)>(&mut self, index: usize, mut cb: T) {
        cb(self.materials.get_unchecked_mut(index));
        self.light_flags.set(
            index,
            Vec4::from(self.materials[index].color)
                .truncate()
                .cmpgt(Vec3::ONE)
                .any(),
        );
    }

    pub fn get_texture(&self, index: usize) -> Option<&Texture> {
        self.textures.get(index)
    }

    pub fn get_texture_mut(&mut self, index: usize) -> Option<&mut Texture> {
        for id in self.tex_material_mapping[index].iter() {
            let id = *id as usize;
            self.materials.trigger_changed(id);
        }

        self.textures.get_mut(index)
    }

    pub fn get_texture_index<T: AsRef<Path> + Copy>(
        &mut self,
        path: T,
        flip: Flip,
    ) -> Result<i32, i32> {
        // First see if we have already loaded the texture before
        if let Some(id) = self.tex_path_mapping.get(path.as_ref()) {
            return Ok((*id) as i32);
        }

        return match Texture::load(path, flip) {
            Ok(mut tex) => {
                if tex.width < 64 || tex.height < 64 {
                    tex = tex.resized(64.max(tex.width), 64.max(tex.height));
                }

                tex.generate_mipmaps(Texture::MIP_LEVELS);

                let index = self.textures.push(tex);
                self.tex_material_mapping
                    .overwrite_val(index, HashSet::new());

                // Add to mapping to prevent loading the same image multiple times
                self.tex_path_mapping
                    .insert(path.as_ref().to_path_buf(), index);

                Ok(index as i32)
            }
            Err(_) => Err(-1),
        };
    }

    pub fn get_default(&self) -> usize {
        0
    }

    pub fn is_empty(&self) -> bool {
        self.materials.is_empty()
    }

    pub fn len(&self) -> usize {
        self.materials.len()
    }

    pub fn len_textures(&self) -> usize {
        self.textures.len()
    }

    pub fn changed(&self) -> bool {
        self.materials.any_changed()
    }

    pub fn light_flags(&self) -> &bitvec::prelude::BitVec {
        &self.light_flags
    }

    pub unsafe fn as_slice(&self) -> &[Material] {
        self.materials.as_slice()
    }

    pub fn iter_changed_materials(&self) -> ChangedIterator<'_, Material> {
        self.materials.iter_changed()
    }

    pub unsafe fn textures_slice(&self) -> &[Texture] {
        self.textures.as_slice()
    }

    pub fn iter_changed_textures(&self) -> ChangedIterator<'_, Texture> {
        self.textures.iter_changed()
    }

    pub fn get_materials_changed(&self) -> &BitSlice {
        self.materials.changed()
    }

    pub fn update_device_materials(&mut self) {
        for (i, m) in self.materials.iter_changed() {
            self.device_materials.overwrite(i, into_device_material(m));
        }
    }

    pub fn get_device_materials(&self) -> &[DeviceMaterial] {
        self.device_materials.as_slice()
    }

    pub fn get_textures(&self) -> &[Texture] {
        self.textures.as_slice()
    }

    pub fn get_textures_changed(&self) -> &BitSlice {
        self.textures.changed()
    }

    pub fn textures_changed(&self) -> bool {
        self.textures.any_changed()
    }

    pub fn reset_changed(&mut self) {
        self.materials.reset_changed();
        self.textures.reset_changed();
    }

    pub fn set_changed(&mut self) {
        self.materials.trigger_changed_all();
        self.device_materials.trigger_changed_all();
        self.textures.trigger_changed_all();
    }

    // Returns an iterator that goes over all materials
    pub fn iter(&self) -> FlaggedIterator<'_, Material> {
        self.materials.iter()
    }

    /// Returns an iterator that goes over all materials
    /// Note: calling this function sets the changed flag to true for all materials,
    /// this can have a major impact on performance if there are many materials in the scene.
    pub fn iter_mut(&mut self) -> FlaggedIteratorMut<'_, Material> {
        self.materials.iter_mut()
    }

    pub fn tex_iter(&self) -> FlaggedIterator<'_, Texture> {
        self.textures.iter()
    }

    /// Returns an iterator that goes over all materials
    /// Note: calling this function sets the changed flag to true for all textures,
    /// this can have a major impact on performance as all textures will have to be reuploaded
    /// to the rendering device.
    pub fn tex_iter_mut(&mut self) -> FlaggedIteratorMut<'_, Texture> {
        self.textures.iter_mut()
    }
}

impl Index<usize> for MaterialList {
    type Output = Material;

    fn index(&self, index: usize) -> &Self::Output {
        &self.materials[index]
    }
}

impl IndexMut<usize> for MaterialList {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.materials[index]
    }
}

pub(crate) fn into_device_material(mat: &Material) -> DeviceMaterial {
    let to_char = |f: f32| -> u8 { (f * 255.0).min(255.0) as u8 };
    let to_u32 = |a: f32, b: f32, c: f32, d: f32| -> u32 {
        let a = to_char(a) as u32;
        let b = to_char(b) as u32;
        let c = to_char(c) as u32;
        let d = to_char(d) as u32;

        a | (b << 8) | (c << 16) | (d << 24)
    };

    let parameters: [u32; 4] = [
        to_u32(mat.metallic, mat.subsurface, mat.specular_f, mat.roughness),
        to_u32(
            mat.specular_tint,
            mat.anisotropic,
            mat.sheen,
            mat.sheen_tint,
        ),
        to_u32(
            mat.clearcoat,
            mat.clearcoat_gloss,
            mat.transmission,
            mat.eta,
        ),
        to_u32(mat.custom0, mat.custom1, mat.custom2, mat.custom3),
    ];

    let mut flags = MaterialFlags::default();
    if mat.diffuse_tex >= 0 {
        flags.set(MaterialProps::HasDiffuseMap, true);
    }
    if mat.normal_tex >= 0 {
        flags.set(MaterialProps::HasNormalMap, true);
    }
    if mat.metallic_roughness_tex >= 0 {
        flags.set(MaterialProps::HasRoughnessMap, true);
        flags.set(MaterialProps::HasMetallicMap, true);
    }
    if mat.emissive_tex >= 0 {
        flags.set(MaterialProps::HasEmissiveMap, true);
    }
    if mat.sheen_tex >= 0 {
        flags.set(MaterialProps::HasSheenMap, true);
    }

    DeviceMaterial {
        color: mat.color,
        absorption: mat.absorption,
        specular: mat.specular,
        parameters,
        flags: flags.into(),
        diffuse_map: mat.diffuse_tex as i32,
        normal_map: mat.normal_tex as i32,
        metallic_roughness_map: mat.metallic_roughness_tex as i32,
        emissive_map: mat.emissive_tex as i32,
        sheen_map: mat.sheen_tex as i32,
        ..Default::default()
    }
}
