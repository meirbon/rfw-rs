use rfw::prelude::TextureData;
use std::num::NonZeroU32;
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
                wgpu::ImageCopyTexture {
                    mip_level: i as u32,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                    texture: &texture,
                },
                &tex.bytes[(offset as usize)..(offset + end) as usize],
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: NonZeroU32::new((width as usize * std::mem::size_of::<u32>()) as u32),
                    rows_per_image: NonZeroU32::new(tex.height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
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
                depth_or_array_layers: 1,
            },
            mip_level_count: tex.mip_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
        });

        let mut width = tex.width;
        let mut height = tex.height;
        let mut local_offset = 0_u64;
        for i in 0..tex.mip_levels {
            let offset = local_offset * std::mem::size_of::<u32>() as u64;

            let end = (width as usize * height as usize * std::mem::size_of::<u32>()) as u64;

            queue.write_texture(
                wgpu::ImageCopyTexture {
                    mip_level: i as u32,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                    texture: &texture,
                },
                &tex.bytes[(offset as usize)..(offset + end) as usize],
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: NonZeroU32::new((width as usize * std::mem::size_of::<u32>()) as u32),
                    rows_per_image: NonZeroU32::new(tex.height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
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
            mip_level_count: NonZeroU32::new(tex.mip_levels),
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
