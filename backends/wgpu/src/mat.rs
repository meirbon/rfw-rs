use rfw::{
    prelude::{DeviceMaterial, TextureData},
    utils::collections::FlaggedStorage,
};
use std::num::NonZeroU32;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Debug)]
pub struct WgpuTexture {
    dims: (u32, u32),
    texture: Arc<Option<wgpu::Texture>>,
    pub(crate) view: Arc<Option<wgpu::TextureView>>,
}

impl Default for WgpuTexture {
    fn default() -> Self {
        Self {
            dims: (0, 0),
            texture: Arc::new(None),
            view: Arc::new(None),
        }
    }
}

impl WgpuTexture {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, tex: TextureData) -> Self {
        let mut texture = Self::default();
        texture.init(device, queue, tex);
        texture
    }

    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, tex: TextureData) {
        if self.texture.is_none() || tex.width != self.dims.0 || tex.height != self.dims.1 {
            self.init(device, queue, tex);
            return;
        }

        let texture: &Option<wgpu::Texture> = &self.texture;
        let texture: &wgpu::Texture = texture.as_ref().unwrap();

        let mut width = tex.width;
        let mut height = tex.height;
        let mut local_offset = 0_u64;
        for i in 0..tex.mip_levels {
            let offset = local_offset * std::mem::size_of::<u32>() as u64;

            let end = (width as usize * height as usize * std::mem::size_of::<u32>()) as u64;

            queue.write_texture(
                wgpu::TextureCopyView {
                    mip_level: i as u32,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                    texture: &texture,
                },
                &tex.bytes[(offset as usize)..(offset + end) as usize],
                wgpu::TextureDataLayout {
                    offset: 0,
                    bytes_per_row: ((width as usize * std::mem::size_of::<u32>()) as u32),
                    rows_per_image: tex.height,
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth: 1,
                },
            );

            local_offset += (width * height) as wgpu::BufferAddress;
            width >>= 1;
            height >>= 1;
        }
    }

    fn init(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, tex: TextureData) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: tex.width,
                height: tex.height,
                depth: 1,
            },
            mip_level_count: tex.mip_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
        });

        let mut width = tex.width;
        let mut height = tex.height;
        let mut local_offset = 0 as wgpu::BufferAddress;
        for i in 0..tex.mip_levels {
            let offset = local_offset * std::mem::size_of::<u32>() as u64;

            let end = (width as usize * height as usize * std::mem::size_of::<u32>()) as u64;

            queue.write_texture(
                wgpu::TextureCopyView {
                    mip_level: i as u32,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                    texture: &texture,
                },
                &tex.bytes[(offset as usize)..(offset + end) as usize],
                wgpu::TextureDataLayout {
                    offset: 0,
                    bytes_per_row: ((width as usize * std::mem::size_of::<u32>()) as u32),
                    rows_per_image: tex.height,
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth: 1,
                },
            );

            local_offset += (width * height) as wgpu::BufferAddress;
            width >>= 1;
            height >>= 1;
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(wgpu::TextureFormat::Bgra8Unorm),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: Default::default(),
            base_mip_level: 0,
            level_count: NonZeroU32::new(tex.mip_levels),
            base_array_layer: 0,
            array_layer_count: None,
        });

        self.dims = (tex.width, tex.height);
        self.texture = Arc::new(Some(texture));
        self.view = Arc::new(Some(view));
    }
}

impl Clone for WgpuTexture {
    fn clone(&self) -> Self {
        Self {
            dims: self.dims,
            texture: self.texture.clone(),
            view: self.view.clone(),
        }
    }
}

#[derive(Debug)]
pub struct WgpuBindGroup {
    pub group: Arc<Option<wgpu::BindGroup>>,
}

impl WgpuBindGroup {
    pub fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        material: &DeviceMaterial,
        textures: &FlaggedStorage<WgpuTexture>,
    ) -> Self {
        let albedo_tex = material.diffuse_map.max(0) as usize;
        let normal_tex = material.normal_map.max(0) as usize;
        let roughness_tex = material.metallic_roughness_map.max(0) as usize;
        let emissive_tex = material.emissive_map.max(0) as usize;
        let sheen_tex = material.sheen_map.max(0) as usize;

        let albedo_view = textures[albedo_tex].view.deref().as_ref().unwrap();
        let normal_view = textures[normal_tex].view.deref().as_ref().unwrap();
        let roughness_view = textures[roughness_tex].view.deref().as_ref().unwrap();
        let emissive_view = textures[emissive_tex].view.deref().as_ref().unwrap();
        let sheen_view = textures[sheen_tex].view.deref().as_ref().unwrap();

        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(albedo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(roughness_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(emissive_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(sheen_view),
                },
            ],
            layout,
        });

        Self {
            group: Arc::new(Some(group)),
        }
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        material: &DeviceMaterial,
        textures: &FlaggedStorage<WgpuTexture>,
    ) {
        let albedo_tex = material.diffuse_map.max(0) as usize;
        let normal_tex = material.normal_map.max(0) as usize;
        let roughness_tex = material.metallic_roughness_map.max(0) as usize;
        let emissive_tex = material.emissive_map.max(0) as usize;
        let sheen_tex = material.sheen_map.max(0) as usize;

        let albedo_view = textures[albedo_tex].view.deref().as_ref().unwrap();
        let normal_view = textures[normal_tex].view.deref().as_ref().unwrap();
        let roughness_view = textures[roughness_tex].view.deref().as_ref().unwrap();
        let emissive_view = textures[emissive_tex].view.deref().as_ref().unwrap();
        let sheen_view = textures[sheen_tex].view.deref().as_ref().unwrap();

        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(albedo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(normal_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(roughness_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(emissive_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(sheen_view),
                },
            ],
            layout,
        });

        self.group = Arc::new(Some(group));
    }
}

impl Default for WgpuBindGroup {
    fn default() -> Self {
        Self {
            group: Arc::new(None),
        }
    }
}

impl Clone for WgpuBindGroup {
    fn clone(&self) -> Self {
        Self {
            group: self.group.clone(),
        }
    }
}
