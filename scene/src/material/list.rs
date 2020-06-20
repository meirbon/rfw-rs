use crate::{material::Material, DeviceMaterial};

use bitvec::prelude::*;
use glam::*;
use image::GenericImageView;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::ops::{Index, IndexMut};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialList {
    changed: BitVec,
    changed_textures: BitVec,
    light_flags: BitVec,
    materials: Vec<Material>,
    tex_path_mapping: HashMap<PathBuf, usize>,
    textures: Vec<Texture>,
}

impl Display for MaterialList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MaterialList: {{ changed: {}, changed_textures: {}, lights_materials: {}, materials: {}, textures: {} }}",
            self.changed.count_ones(),
            self.changed_textures.count_ones(),
            self.light_flags.count_ones(),
            self.materials.len(),
            self.textures.len()
        )
    }
}

// TODO: Support other formats than BGRA8
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Texture {
    pub data: Vec<u32>,
    pub width: u32,
    pub height: u32,
}

impl Display for Texture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Texture {{ data: {} bytes, width: {}, height: {} }}",
            self.data.len() * std::mem::size_of::<u32>(),
            self.width,
            self.height
        )
    }
}

impl Texture {
    pub const MIP_LEVELS: usize = 5;

    pub fn generate_mipmaps(&mut self, levels: usize) {
        self.data.resize(self.required_texel_count(levels), 0);

        let mut src_offset = 0;
        let mut dst_offset = src_offset + self.width as usize * self.height as usize;

        let mut pw = self.width as usize;
        let mut w = self.width as usize >> 1;
        let mut h = self.height as usize >> 1;

        for _ in 1..levels {
            let max_dst_offset = dst_offset + (w * h);
            debug_assert!(max_dst_offset <= self.data.len());

            for y in 0..h {
                for x in 0..w {
                    let src0 = self.data[x * 2 + (y * 2) * pw + src_offset];
                    let src1 = self.data[x * 2 + 1 + (y * 2) * pw + src_offset];
                    let src2 = self.data[x * 2 + (y * 2 + 1) * pw + src_offset];
                    let src3 = self.data[x * 2 + 1 + (y * 2 + 1) * pw + src_offset];
                    let a = ((src0 >> 24) & 255)
                        .min((src1 >> 24) & 255)
                        .min(((src2 >> 24) & 255).min((src3 >> 24) & 255));
                    let r = ((src0 >> 16) & 255)
                        + ((src1 >> 16) & 255)
                        + ((src2 >> 16) & 255)
                        + ((src3 >> 16) & 255);
                    let g = ((src0 >> 8) & 255)
                        + ((src1 >> 8) & 255)
                        + ((src2 >> 8) & 255)
                        + ((src3 >> 8) & 255);
                    let b = (src0 & 255) + (src1 & 255) + (src2 & 255) + (src3 & 255);
                    self.data[dst_offset + x + y * w] =
                        (a << 24) + ((r >> 2) << 16) + ((g >> 2) << 8) + (b >> 2);
                }
            }

            src_offset = dst_offset;
            dst_offset += w * h;
            pw = w;
            w >>= 1;
            h >>= 1;
        }
    }

    fn required_texel_count(&self, levels: usize) -> usize {
        let mut w = self.width;
        let mut h = self.height;
        let mut needed = 0;

        for _ in 0..levels {
            needed += w * h;
            w >>= 1;
            h >>= 1;
        }

        needed as usize
    }

    pub fn sample_uv_rgba(&self, uv: [f32; 2]) -> [f32; 4] {
        let x = (self.width as f32 * uv[0]) as i32;
        let y = (self.height as f32 * uv[1]) as i32;

        let x = x.min(self.width as i32 - 1).max(0) as usize;
        let y = y.min(self.height as i32).max(0) as usize;

        // BGRA texel
        let texel = self.data[y * self.width as usize + x];
        let blue = (texel & 0xFF) as f32 / 255.0;
        let green = ((texel.overflowing_shr(8).0) & 0xFF) as f32 / 255.0;
        let red = ((texel.overflowing_shr(16).0) & 0xFF) as f32 / 255.0;
        let alpha = ((texel.overflowing_shr(24).0) & 0xFF) as f32 / 255.0;

        [red, green, blue, alpha]
    }

    pub fn sample_uv_bgra(&self, uv: [f32; 2]) -> [f32; 4] {
        let x = (self.width as f32 * uv[0]) as i32;
        let y = (self.height as f32 * uv[1]) as i32;

        let x = x.min(self.width as i32 - 1).max(0) as usize;
        let y = y.min(self.height as i32).max(0) as usize;

        // BGRA texel
        let texel = self.data[y * self.width as usize + x];
        let blue = (texel & 0xFF) as f32 / 255.0;
        let green = ((texel.overflowing_shr(8).0) & 0xFF) as f32 / 255.0;
        let red = ((texel.overflowing_shr(16).0) & 0xFF) as f32 / 255.0;
        let alpha = ((texel.overflowing_shr(24).0) & 0xFF) as f32 / 255.0;

        [blue, green, red, alpha]
    }

    pub fn sample_uv_rgb(&self, uv: [f32; 2]) -> [f32; 3] {
        let x = (self.width as f32 * uv[0]) as i32;
        let y = (self.height as f32 * uv[1]) as i32;

        let x = x.min(self.width as i32 - 1).max(0) as usize;
        let y = y.min(self.height as i32).max(0) as usize;

        // BGRA texel
        let texel = self.data[y * self.width as usize + x];
        let blue = (texel & 0xFF) as f32 / 255.0;
        let green = ((texel.overflowing_shr(8).0) & 0xFF) as f32 / 255.0;
        let red = ((texel.overflowing_shr(16).0) & 0xFF) as f32 / 255.0;

        [red, green, blue]
    }

    pub fn sample_uv_bgr(&self, uv: [f32; 2]) -> [f32; 3] {
        let x = (self.width as f32 * uv[0]) as i32;
        let y = (self.height as f32 * uv[1]) as i32;

        let x = x.min(self.width as i32 - 1).max(0) as usize;
        let y = y.min(self.height as i32).max(0) as usize;

        // BGRA texel
        let texel = self.data[y * self.width as usize + x];
        let blue = (texel & 0xFF) as f32 / 255.0;
        let green = ((texel.overflowing_shr(8).0) & 0xFF) as f32 / 255.0;
        let red = ((texel.overflowing_shr(16).0) & 0xFF) as f32 / 255.0;

        [blue, green, red]
    }

    pub fn get_texel(&self, x: usize, y: usize) -> u32 {
        let x = (x as i32).min(self.width as i32 - 1).max(0) as usize;
        let y = (y as i32).min(self.height as i32).max(0) as usize;
        self.data[y * self.width as usize + x]
    }
}

#[allow(dead_code)]
impl MaterialList {
    // Creates an empty material list
    pub fn empty() -> MaterialList {
        MaterialList {
            changed: BitVec::new(),
            changed_textures: BitVec::new(),
            light_flags: BitVec::new(),
            materials: Vec::new(),
            tex_path_mapping: HashMap::new(),
            textures: Vec::new(),
        }
    }

    // Creates a material list with at least a single (empty) texture and (empty) material
    pub fn new() -> MaterialList {
        let materials = vec![Material::default()];

        let mut textures = Vec::new();

        textures.push(Texture {
            // Make sure always a single texture exists (as fallback)
            width: 64,
            height: 64,
            data: vec![0; 4096],
        });

        let mut changed = BitVec::new();
        let mut changed_textures = BitVec::new();
        let mut light_flags = BitVec::new();
        changed.push(true);
        changed_textures.push(true);
        light_flags.push(false);

        MaterialList {
            changed,
            changed_textures,
            light_flags,
            materials,
            tex_path_mapping: HashMap::new(),
            textures,
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
        albedo: Option<PathBuf>,
        normal: Option<PathBuf>,
        roughness_map: Option<PathBuf>,
        metallic_map: Option<PathBuf>,
        emissive_map: Option<PathBuf>,
        sheen_map: Option<PathBuf>,
    ) -> usize {
        let mut material = Material::default();
        material.color = color.extend(1.0).into();
        material.specular = specular.extend(1.0).into();
        material.roughness = roughness;
        material.transmission = transmission;

        let diffuse_tex = if let Some(albedo) = albedo {
            self.get_texture_index(&albedo).unwrap_or_else(|_| -1)
        } else {
            -1
        };

        let normal_tex = if let Some(normal) = normal {
            self.get_texture_index(&normal).unwrap_or_else(|_| -1)
        } else {
            -1
        };

        let roughness_tex = if let Some(roughness_map) = roughness_map {
            self.get_texture_index(&roughness_map)
                .unwrap_or_else(|_| -1)
        } else {
            -1
        };

        let metallic_tex = if let Some(metallic_map) = metallic_map {
            self.get_texture_index(&metallic_map).unwrap_or_else(|_| -1)
        } else {
            -1
        };

        let emissive_tex = if let Some(emissive_map) = emissive_map {
            self.get_texture_index(&emissive_map).unwrap_or_else(|_| -1)
        } else {
            -1
        };

        let sheen_tex = if let Some(sheen_map) = sheen_map {
            self.get_texture_index(&sheen_map).unwrap_or_else(|_| -1)
        } else {
            -1
        };

        material.diffuse_tex = diffuse_tex as i16;
        material.normal_tex = normal_tex as i16;
        material.roughness_tex = roughness_tex as i16;
        material.metallic_tex = metallic_tex as i16;
        material.emissive_tex = emissive_tex as i16;
        material.sheen_tex = sheen_tex as i16;

        self.push(material)
    }

    pub fn push(&mut self, mat: Material) -> usize {
        let i = self.materials.len();
        self.changed.push(true);
        let is_light = Vec4::from(mat.color).truncate().cmpgt(Vec3::one()).any();

        self.light_flags.push(is_light);
        self.materials.push(mat);
        i
    }

    pub fn push_texture(&mut self, texture: Texture) -> usize {
        let i = self.textures.len();
        self.textures.push(texture);
        i
    }

    pub fn get(&self, index: usize) -> Option<&Material> {
        self.materials.get(index)
    }

    pub fn get_mut<T: FnMut(Option<&mut Material>)>(&mut self, index: usize, mut cb: T) {
        cb(self.materials.get_mut(index));
        self.changed.set(index, true);
        self.light_flags.set(
            index,
            Vec4::from(self.materials[index].color)
                .truncate()
                .cmpgt(Vec3::one())
                .any(),
        );
    }

    pub unsafe fn get_unchecked(&self, index: usize) -> &Material {
        self.materials.get_unchecked(index)
    }

    pub unsafe fn get_unchecked_mut<T: FnMut(&mut Material)>(&mut self, index: usize, mut cb: T) {
        cb(self.materials.get_unchecked_mut(index));
        self.changed.set(index, true);
        self.light_flags.set(
            index,
            Vec4::from(self.materials[index].color)
                .truncate()
                .cmpgt(Vec3::one())
                .any(),
        );
    }

    pub fn get_texture(&self, index: usize) -> Option<&Texture> {
        self.textures.get(index)
    }

    pub fn get_texture_mut(&mut self, index: usize) -> Option<&mut Texture> {
        self.changed_textures.set(index, true);
        self.textures.get_mut(index)
    }

    pub fn get_texture_index<T: AsRef<Path> + Copy>(&mut self, path: T) -> Result<i32, i32> {
        // First see if we have already loaded the texture before
        if let Some(id) = self.tex_path_mapping.get(path.as_ref()) {
            return Ok((*id) as i32);
        }

        // See if file exists
        if !path.as_ref().exists() {
            return Err(-1);
        }

        // Attempt to load image
        let img = image::open(path);
        if let Err(_) = img {
            return Err(-1);
        }

        // Loading was successful
        let img = img.unwrap().flipv();

        let (width, height) = (img.width(), img.height());
        let mut data = vec![0 as u32; (width * height) as usize];

        let bgra_image = img.to_bgra();
        data.copy_from_slice(unsafe {
            std::slice::from_raw_parts(bgra_image.as_ptr() as *const u32, (width * height) as usize)
        });

        let mut tex = Texture {
            width,
            height,
            data,
        };

        tex.generate_mipmaps(Texture::MIP_LEVELS);

        self.changed_textures.push(true);
        self.textures.push(tex);
        let index = self.textures.len() - 1;

        // Add to mapping to prevent loading the same image multiple times
        self.tex_path_mapping
            .insert(path.as_ref().to_path_buf(), index);

        Ok(index as i32)
    }

    pub fn get_default(&self) -> usize {
        0
    }

    pub fn len(&self) -> usize {
        self.materials.len()
    }

    pub fn len_textures(&self) -> usize {
        self.textures.len()
    }

    pub fn changed(&self) -> bool {
        self.changed.any()
    }

    pub fn light_flags(&self) -> &BitVec {
        &self.light_flags
    }

    pub fn as_slice(&self) -> &[Material] {
        self.materials.as_slice()
    }

    pub fn textures_slice(&self) -> &[Texture] {
        self.textures.as_slice()
    }

    pub fn into_device_materials(&self) -> Vec<DeviceMaterial> {
        self.materials.par_iter().map(|m| m.into()).collect()
    }

    pub fn textures_changed(&self) -> bool {
        self.changed_textures.any()
    }

    pub fn reset_changed(&mut self) {
        self.changed.set_all(false);
        self.changed_textures.set_all(false);
    }

    pub fn set_changed(&mut self) {
        self.changed.set_all(true);
        self.changed_textures.set_all(true);
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
