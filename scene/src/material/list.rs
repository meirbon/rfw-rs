use crate::{material::Material, DeviceMaterial};
use image::GenericImageView;
use rfw_utils::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::ops::{Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Sub, SubAssign};
use std::path::{Path, PathBuf};

#[cfg(feature = "object_caching")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct MaterialList {
    light_flags: BitVec,
    materials: TrackedStorage<Material>,
    device_materials: TrackedStorage<DeviceMaterial>,
    tex_path_mapping: HashMap<PathBuf, usize>,
    textures: TrackedStorage<Texture>,
    tex_material_mapping: FlaggedStorage<HashSet<u32>>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Flip {
    None,
    FlipU,
    FlipV,
    FlipUV,
}

impl Default for Flip {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone)]
pub enum TextureSource {
    Loaded(Texture),
    Filesystem(PathBuf, Flip),
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

// TODO: Support other formats than BGRA8
#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Texture {
    pub data: Vec<u32>,
    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
}

impl Default for Texture {
    fn default() -> Self {
        let mut data = Vec::with_capacity(64 * 64);
        for y in 0..64 {
            for x in 0..64 {
                let r = x as f32 / 64.0_f32;
                let g = y as f32 / 64.0_f32;
                let b = 0.2;

                let r = (r * 255.0) as u32;
                let g = (g * 255.0) as u32;
                let b = (b * 255.0) as u32;
                let a = 255_u32;

                data.push((a << 24) + ((r >> 2) << 16) + ((g >> 2) << 8) + (b >> 2));
            }
        }

        assert_eq!(data.len(), 64 * 64);

        Self {
            data,
            width: 64,
            height: 64,
            mip_levels: 1,
        }
    }
}

impl Display for Texture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Texture {{ data: {} bytes, width: {}, height: {}, mip_levels: {} }}",
            self.data.len() * std::mem::size_of::<u32>(),
            self.width,
            self.height,
            self.mip_levels
        )
    }
}

impl Texture {
    pub const MIP_LEVELS: usize = 5;

    pub fn generate_mipmaps(&mut self, levels: usize) {
        if self.mip_levels == levels as u32 {
            return;
        }

        self.mip_levels = levels as u32;
        self.data.resize(self.required_texel_count(levels), 0);

        let mut src_offset = 0;
        let mut dst_offset = src_offset + self.width as usize * self.height as usize;

        let mut pw: usize = self.width as usize;
        let mut w: usize = self.width as usize >> 1;
        let mut h: usize = self.height as usize >> 1;

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

    pub fn texel_count(width: u32, height: u32, levels: usize) -> usize {
        let mut w = width;
        let mut h = height;
        let mut needed = 0;

        for _ in 0..levels {
            needed += w * h;
            w >>= 1;
            h >>= 1;
        }

        needed as usize
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

    /// Texel count
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn offset_for_level(&self, mip_level: usize) -> usize {
        assert!(mip_level <= self.mip_levels as usize);
        let mut offset = 0;
        for i in 0..mip_level {
            let (w, h) = self.mip_level_width_height(i);
            offset += w * h;
        }
        offset
    }

    pub fn mip_level_width(&self, mip_level: usize) -> usize {
        let mut w = self.width as usize;
        for _ in 0..mip_level {
            w >>= 1;
        }
        w
    }

    pub fn mip_level_height(&self, mip_level: usize) -> usize {
        let mut h = self.height as usize;
        for _ in 0..mip_level {
            h >>= 1;
        }
        h
    }

    pub fn mip_level_width_height(&self, mip_level: usize) -> (usize, usize) {
        let mut w = self.width as usize;
        let mut h = self.height as usize;

        if mip_level == 0 {
            return (w, h);
        }

        for _ in 0..mip_level {
            w >>= 1;
            h >>= 1
        }

        (w, h)
    }

    /// Resizes texture into given dimensions, this is an expensive operation.
    pub fn resized(&self, width: u32, height: u32) -> Texture {
        let mut image: image::ImageBuffer<image::Bgra<u8>, Vec<u8>> =
            image::ImageBuffer::new(self.width, self.height);
        image.copy_from_slice(unsafe {
            std::slice::from_raw_parts(
                self.data.as_ptr() as *const u8,
                (self.width * self.height) as usize * std::mem::size_of::<u32>(),
            )
        });
        let image =
            image::imageops::resize(&image, width, height, image::imageops::FilterType::Nearest);

        let mut data = vec![0; (width * height) as usize];
        data.copy_from_slice(unsafe {
            std::slice::from_raw_parts(image.as_ptr() as *const u32, (width * height) as usize)
        });

        Texture {
            data,
            width: width as u32,
            height: height as u32,
            mip_levels: 1,
        }
    }

    pub fn load<T: AsRef<Path>>(path: T, flip: Flip) -> Result<Self, ()> {
        // See if file exists
        if !path.as_ref().exists() {
            return Err(());
        }

        // Attempt to load image
        let img = match image::open(path) {
            Ok(img) => img,
            Err(_) => return Err(()),
        };

        // Loading was successful
        let img = match flip {
            Flip::None => img,
            Flip::FlipU => img.fliph(),
            Flip::FlipV => img.flipv(),
            Flip::FlipUV => img.fliph().flipv(),
        };

        let (width, height) = (img.width(), img.height());
        let mut data = vec![0 as u32; (width * height) as usize];

        let bgra_image = img.to_bgra8();
        data.copy_from_slice(unsafe {
            std::slice::from_raw_parts(bgra_image.as_ptr() as *const u32, (width * height) as usize)
        });

        Ok(Texture {
            width,
            height,
            data,
            mip_levels: 1,
        })
    }

    pub fn flipped(&mut self, flip: Flip) -> Self {
        let mut img: image::ImageBuffer<image::Bgra<u8>, Vec<u8>> =
            image::ImageBuffer::new(self.width, self.height);
        img.copy_from_slice(unsafe {
            std::slice::from_raw_parts(
                self.data.as_ptr() as *const u8,
                (self.width * self.height) as usize * std::mem::size_of::<u32>(),
            )
        });

        let img = image::DynamicImage::ImageBgra8(img);
        let img = match flip {
            Flip::None => img,
            Flip::FlipU => img.fliph(),
            Flip::FlipV => img.flipv(),
            Flip::FlipUV => img.fliph().flipv(),
        };

        let mut data = vec![0; (self.width * self.height) as usize];
        data.copy_from_slice(unsafe {
            std::slice::from_raw_parts(
                img.as_bgra8().unwrap().as_ptr() as *const u32,
                (self.width * self.height) as usize,
            )
        });

        Texture {
            data,
            width: self.width,
            height: self.height,
            mip_levels: 1,
        }
    }

    pub fn merge(r: Option<&Self>, g: Option<&Self>, b: Option<&Self>, a: Option<&Self>) -> Self {
        let mut wh = None;

        let mut assert_tex_wh = |t: Option<&Self>| {
            if let Some(t) = t {
                if let Some((width, height)) = wh {
                    assert_eq!(width, t.width);
                    assert_eq!(height, t.height);
                } else {
                    wh = Some((t.width, t.height));
                }
            }
        };

        assert_tex_wh(r);
        assert_tex_wh(g);
        assert_tex_wh(b);
        assert_tex_wh(a);

        let (width, height) = wh.unwrap();

        let sample = |t: Option<&Self>, index: usize| {
            if let Some(t) = t {
                t.data[index] & 255
            } else {
                0
            }
        };

        let mut data = vec![0_u32; (width * height) as usize];
        for y in 0..height {
            for x in 0..width {
                let index = (y * height + x) as usize;
                let r = sample(r, index);
                let g = sample(g, index);
                let b = sample(b, index);
                let a = sample(a, index);
                data[index] = (a << 24) + (r << 16) + (g << 8) + b;
            }
        }

        Texture {
            data,
            width,
            height,
            mip_levels: 1,
        }
    }

    /// Parses texture from bytes. Stride is size of texel in bytes
    pub fn from_bytes(
        bytes: &[u8],
        width: u32,
        height: u32,
        format: TextureFormat,
        stride: usize,
    ) -> Self {
        if format == TextureFormat::BGRA && stride == 4 {
            // Same format as internal format
            return Texture {
                data: unsafe {
                    std::slice::from_raw_parts(
                        bytes.as_ptr() as *mut u32,
                        (width * height) as usize,
                    )
                }
                .to_vec(),
                width,
                height,
                mip_levels: 1,
            };
        }

        let mut data = vec![0 as u32; (width * height) as usize];
        for y in 0..height {
            for x in 0..width {
                let index = (x + y * width) as usize;
                let orig_index = index * stride;
                let (r, g, b, a) = match format {
                    TextureFormat::R => (bytes[orig_index] as u32, 0, 0, 0),
                    TextureFormat::RG => (bytes[orig_index] as u32, 0, 0, 0),
                    TextureFormat::BGR => (
                        bytes[orig_index + 2] as u32,
                        bytes[orig_index + 1] as u32,
                        bytes[orig_index] as u32,
                        0,
                    ),
                    TextureFormat::RGB => (
                        bytes[orig_index] as u32,
                        bytes[orig_index + 1] as u32,
                        bytes[orig_index + 2] as u32,
                        0,
                    ),
                    TextureFormat::RGBA => (
                        bytes[orig_index] as u32,
                        bytes[orig_index + 1] as u32,
                        bytes[orig_index + 2] as u32,
                        bytes[orig_index + 3] as u32,
                    ),
                    TextureFormat::BGRA => (
                        bytes[orig_index + 2] as u32,
                        bytes[orig_index + 1] as u32,
                        bytes[orig_index] as u32,
                        bytes[orig_index + 3] as u32,
                    ),
                    TextureFormat::R16 => (
                        (bytes[orig_index] as u32) + ((bytes[orig_index + 1] as u32) << 16),
                        0,
                        0,
                        0,
                    ),
                    TextureFormat::RG16 => (
                        (bytes[orig_index] as u32) + ((bytes[orig_index + 1] as u32) << 16),
                        (bytes[orig_index + 2] as u32) + ((bytes[orig_index + 3] as u32) << 16),
                        0,
                        0,
                    ),
                    TextureFormat::RGB16 => (
                        (bytes[orig_index] as u32) + ((bytes[orig_index + 1] as u32) << 16),
                        (bytes[orig_index + 2] as u32) + ((bytes[orig_index + 3] as u32) << 16),
                        (bytes[orig_index + 4] as u32) + ((bytes[orig_index + 5] as u32) << 16),
                        0,
                    ),
                    TextureFormat::RGBA16 => (
                        (bytes[orig_index] as u32) + ((bytes[orig_index + 1] as u32) << 16),
                        (bytes[orig_index + 2] as u32) + ((bytes[orig_index + 3] as u32) << 16),
                        (bytes[orig_index + 4] as u32) + ((bytes[orig_index + 5] as u32) << 16),
                        (bytes[orig_index + 6] as u32) + ((bytes[orig_index + 7] as u32) << 16),
                    ),
                };

                data[index] = (a << 24) + (r << 16) + (g << 8) + b;
            }
        }

        Texture {
            data,
            width,
            height,
            mip_levels: 1,
        }
    }

    pub fn transformed<C>(mut self, cb: C) -> Texture
    where
        C: Fn(Pixel) -> Pixel,
    {
        for pixel in self.data.iter_mut() {
            let cb = &cb;
            let p: u32 = *pixel;

            let r: u32 = p.overflowing_shr(16).0 & 255;
            let g: u32 = p.overflowing_shr(8).0 & 255;
            let b: u32 = p & 255;
            let a: u32 = p.overflowing_shr(24).0 & 255;

            let r: f32 = r as f32 / 255.0;
            let g: f32 = g as f32 / 255.0;
            let b: f32 = b as f32 / 255.0;
            let a: f32 = a as f32 / 255.0;

            let colors: Pixel = cb(Pixel::from([r, g, b, a]));
            let r: u32 = (colors.r.min(1.0).max(0.0) * 255.0) as u32;
            let g: u32 = (colors.g.min(1.0).max(0.0) * 255.0) as u32;
            let b: u32 = (colors.b.min(1.0).max(0.0) * 255.0) as u32;
            let a: u32 = (colors.a.min(1.0).max(0.0) * 255.0) as u32;

            *pixel = (a << 24) + (r << 16) + (g << 8) + b;
        }

        self
    }
}

impl<T: AsRef<Path>> From<T> for Texture {
    fn from(path: T) -> Self {
        Self::load(path, Flip::default()).unwrap()
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
            light_flags: BitVec::new(),
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

        let mut light_flags = BitVec::new();
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
        let is_light = Vec4::from(mat.color).truncate().cmpgt(Vec3::one()).any();

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
                Vec4::from(mat.color).truncate().cmpgt(Vec3::one()).any(),
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
                .cmpgt(Vec3::one())
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

    pub fn light_flags(&self) -> &BitVec {
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

    pub fn get_device_materials(&mut self) -> ChangedIterator<'_, DeviceMaterial> {
        for (i, m) in self.materials.iter_changed() {
            self.device_materials.overwrite(i, m.into());
        }
        self.device_materials.iter_changed()
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
