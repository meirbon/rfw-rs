use crate::instance::{DeviceInstances, InstanceList};
use crate::light::WgpuLights;
use futures::executor::block_on;
use mesh::WgpuMesh;
use rayon::prelude::*;
use rfw::prelude::*;
use rfw::scene::mesh::VertexMesh;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::num::{NonZeroU32, NonZeroU64, NonZeroU8};
use std::ops::Deref;
use std::sync::Arc;
use wgpu::util::DeviceExt;

mod d2;
mod instance;
mod light;
mod mesh;
mod output;
mod pass;
mod pipeline;

#[derive(Debug, Clone)]
pub enum TaskResult {
    Mesh(usize, WgpuMesh),
}

#[derive(Debug)]
pub struct WgpuTexture {
    dims: (u32, u32),
    texture: Arc<Option<wgpu::Texture>>,
    view: Arc<Option<wgpu::TextureView>>,
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
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, tex: &Texture) -> Self {
        let mut texture = Self::default();
        texture.init(device, queue, tex);
        texture
    }

    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, tex: &Texture) {
        if self.texture.is_none() || tex.width != self.dims.0 || tex.height != self.dims.1 {
            self.init(device, queue, tex);
            return;
        }

        let texture: &Option<wgpu::Texture> = &self.texture;
        let texture: &wgpu::Texture = texture.as_ref().unwrap();

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
                &tex.data.as_bytes()[(offset as usize)..(offset + end) as usize],
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

    fn init(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, tex: &Texture) {
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
                &tex.data.as_bytes()[(offset as usize)..(offset + end) as usize],
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

pub struct WgpuBackend {
    device: Arc<wgpu::Device>,
    queue: wgpu::Queue,
    surface: wgpu::Surface,
    swap_chain: wgpu::SwapChain,
    meshes: TrackedStorage<mesh::WgpuMesh>,
    instances: instance::InstanceList,
    material_buffer: wgpu::Buffer,
    material_buffer_size: wgpu::BufferAddress,
    material_bind_groups: FlaggedStorage<WgpuBindGroup>,
    texture_sampler: wgpu::Sampler,
    textures: FlaggedStorage<WgpuTexture>,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    lights: light::WgpuLights,

    uniform_bind_group_layout: wgpu::BindGroupLayout,
    uniform_bind_group: wgpu::BindGroup,

    uniform_camera_buffer: wgpu::Buffer,
    output: output::WgpuOutput,
    pipeline: pipeline::RenderPipeline,
    scene_bounds: AABB,

    ssao_pass: pass::SSAOPass,
    radiance_pass: pass::RadiancePass,
    blit_pass: pass::BlitPass,
    output_pass: pass::QuadPass,

    skins: TrackedStorage<Skin>,

    debug_view: output::WgpuView,
    lights_changed: bool,
    materials_changed: bool,

    mesh_bounds: FlaggedStorage<(AABB, Vec<VertexMesh>)>,
    task_pool: ManagedTaskPool<TaskResult>,
    d2_renderer: d2::Renderer,
}

#[derive(Debug, Copy, Clone)]
enum WgpuError {
    RequestDeviceError,
}

impl std::error::Error for WgpuError {}

impl Display for WgpuError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not retrieve valid device.")
    }
}

impl WgpuBackend {
    const PRESENT_MODE: wgpu::PresentMode = wgpu::PresentMode::Immediate;
    const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
    const UNIFORM_CAMERA_SIZE: wgpu::BufferAddress = (std::mem::size_of::<Mat4>()
        + std::mem::size_of::<Mat4>()
        + std::mem::size_of::<[u32; 4]>()
        + std::mem::size_of::<Vec4>())
        as wgpu::BufferAddress;

    fn create_texture_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture-bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    // Albedo texture
                    binding: 0,
                    count: None,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::SampledTexture {
                        component_type: wgpu::TextureComponentType::Uint,
                        multisampled: false,
                        dimension: wgpu::TextureViewDimension::D2,
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    // Normal texture
                    binding: 1,
                    count: None,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::SampledTexture {
                        component_type: wgpu::TextureComponentType::Uint,
                        multisampled: false,
                        dimension: wgpu::TextureViewDimension::D2,
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    // Roughness texture
                    binding: 2,
                    count: None,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::SampledTexture {
                        component_type: wgpu::TextureComponentType::Uint,
                        multisampled: false,
                        dimension: wgpu::TextureViewDimension::D2,
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    // Emissive texture
                    binding: 3,
                    count: None,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::SampledTexture {
                        component_type: wgpu::TextureComponentType::Uint,
                        multisampled: false,
                        dimension: wgpu::TextureViewDimension::D2,
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    // Sheen texture
                    binding: 4,
                    count: None,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::SampledTexture {
                        component_type: wgpu::TextureComponentType::Uint,
                        multisampled: false,
                        dimension: wgpu::TextureViewDimension::D2,
                    },
                },
            ],
        })
    }

    fn create_uniform_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniform-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    // Matrix mem
                    binding: 0,
                    count: None,
                    visibility: wgpu::ShaderStage::VERTEX
                        | wgpu::ShaderStage::FRAGMENT
                        | wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::UniformBuffer {
                        dynamic: false,
                        min_binding_size: NonZeroU64::new(Self::UNIFORM_CAMERA_SIZE as _),
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    // Material mem
                    binding: 1,
                    count: None,
                    visibility: wgpu::ShaderStage::FRAGMENT | wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::StorageBuffer {
                        min_binding_size: wgpu::BufferSize::new(256),
                        readonly: true,
                        dynamic: false,
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    // Texture sampler
                    binding: 2,
                    count: None,
                    visibility: wgpu::ShaderStage::FRAGMENT | wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Sampler { comparison: false },
                },
            ],
        })
    }
}

impl Backend for WgpuBackend {
    fn init<T: HasRawWindowHandle>(
        window: &T,
        window_size: (usize, usize),
        render_size: (usize, usize),
    ) -> Result<Box<Self>, Box<dyn Error>> {
        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
        let surface = unsafe { instance.create_surface(window) };

        let adapter = match block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: wgpu::PowerPreference::HighPerformance,
        })) {
            None => return Err(Box::new(WgpuError::RequestDeviceError)),
            Some(adapter) => adapter,
        };

        let (width, height) = window_size;
        let (render_width, render_height) = render_size;
        let width = width as u32;
        let height = height as u32;

        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::PUSH_CONSTANTS
                    | wgpu::Features::SAMPLED_TEXTURE_BINDING_ARRAY,
                limits: wgpu::Limits::default(),
                shader_validation: true,
            },
            None,
        ))
        .unwrap();

        let swap_chain = device.create_swap_chain(
            &surface,
            &wgpu::SwapChainDescriptor {
                width,
                height,
                usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
                format: output::WgpuOutput::OUTPUT_FORMAT,
                present_mode: Self::PRESENT_MODE,
            },
        );

        let instances = instance::InstanceList::new(&device);
        let material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("material-mem"),
            mapped_at_creation: false,
            size: std::mem::size_of::<DeviceMaterial>() as wgpu::BufferAddress * 10,
            usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
        });

        let texture_bind_group_layout = Self::create_texture_bind_group_layout(&device);

        let lights = light::WgpuLights::new(10, &device, &instances.bind_group_layout);

        let uniform_camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform-mem"),
            mapped_at_creation: false,
            size: Self::UNIFORM_CAMERA_SIZE,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        });

        let texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("texture-sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 5.0,
            compare: None,
            anisotropy_clamp: NonZeroU8::new(8),
        });

        let uniform_bind_group_layout = Self::create_uniform_bind_group_layout(&device);
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform-bind-group"),
            layout: &uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(
                        uniform_camera_buffer.slice(0..Self::UNIFORM_CAMERA_SIZE as _),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(material_buffer.slice(
                        0..std::mem::size_of::<DeviceMaterial>() as wgpu::BufferAddress * 10,
                    )),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&texture_sampler),
                },
            ],
        });

        let output = output::WgpuOutput::new(&device, render_width, render_height);

        let pipeline = pipeline::RenderPipeline::new(
            &device,
            &uniform_bind_group_layout,
            &instances.bind_group_layout,
            &texture_bind_group_layout,
        );

        let material_buffer_size =
            std::mem::size_of::<DeviceMaterial>() as wgpu::BufferAddress * 10;

        let ssao_pass = pass::SSAOPass::new(&device, &uniform_bind_group_layout, &output);
        let radiance_pass = pass::RadiancePass::new(
            &device,
            &uniform_camera_buffer,
            &material_buffer,
            &output,
            &lights,
        );
        let blit_pass = pass::BlitPass::new(&device, &output);
        let output_pass = pass::QuadPass::new(&device, &output);

        let d2_renderer = d2::Renderer::new(&device);

        Ok(Box::new(Self {
            device: Arc::new(device),
            queue,
            surface,
            swap_chain,
            meshes: TrackedStorage::new(),
            instances,
            material_buffer,
            material_buffer_size,
            material_bind_groups: Default::default(),
            texture_sampler,
            textures: FlaggedStorage::new(),
            texture_bind_group_layout,
            lights,
            uniform_bind_group_layout,
            uniform_bind_group,
            uniform_camera_buffer,
            output,
            pipeline,
            ssao_pass,
            radiance_pass,
            blit_pass,
            output_pass,
            scene_bounds: AABB::new(),

            skins: TrackedStorage::new(),

            debug_view: output::WgpuView::Output,
            lights_changed: true,
            materials_changed: true,

            mesh_bounds: FlaggedStorage::new(),
            task_pool: ManagedTaskPool::default(),
            d2_renderer,
        }))
    }

    fn set_2d_meshes(&mut self, meshes: ChangedIterator<'_, Mesh2D>) {
        self.d2_renderer.update_meshes(&self.device, meshes);
    }

    fn set_2d_instances(&mut self, instances: ChangedIterator<'_, Instance2D>) {
        self.d2_renderer.update_instances(&self.queue, instances);
    }

    fn set_3d_meshes(&mut self, meshes: ChangedIterator<'_, Mesh3D>) {
        for (id, mesh) in meshes {
            self.mesh_bounds
                .overwrite_val(id, (mesh.bounds.clone(), mesh.meshes.clone()));
            let device = self.device.clone();
            let mesh = mesh.clone();
            self.task_pool.push(move |finish| {
                let mesh = mesh::WgpuMesh::new(&device, &mesh);
                finish.send(TaskResult::Mesh(id, mesh));
            });
        }
    }

    fn unload_3d_meshes(&mut self, ids: Vec<usize>) {
        for id in ids {
            match self.meshes.erase(id) {
                Ok(_) => {}
                Err(_) => panic!("mesh id {} did not exist", id),
            }
        }
    }

    fn set_instances(&mut self, instances: ChangedIterator<'_, Instance3D>) {
        for (id, instance) in instances {
            if let Some(mesh_id) = instance.object_id {
                self.instances.set(
                    &self.device,
                    id,
                    instance.clone(),
                    &self.mesh_bounds[mesh_id as usize],
                );
            } else {
                self.instances.set(
                    &self.device,
                    id,
                    instance.clone(),
                    &(AABB::empty(), Vec::new()),
                );
            }

            self.scene_bounds.grow_bb(
                &instance
                    .local_bounds()
                    .transformed(instance.get_transform().to_cols_array()),
            );
        }
    }

    fn unload_instances(&mut self, ids: Vec<usize>) {
        for id in ids {
            self.instances.set(
                &self.device,
                id,
                Instance3D::default(),
                &(AABB::empty(), Vec::new()),
            );
        }
    }

    fn set_materials(&mut self, materials: ChangedIterator<'_, DeviceMaterial>) {
        {
            let materials = materials.as_slice();
            let size =
                (materials.len() * std::mem::size_of::<DeviceMaterial>()) as wgpu::BufferAddress;

            if size > self.material_buffer_size {
                self.material_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    mapped_at_creation: false,
                    label: Some("material-mem"),
                    size,
                    usage: wgpu::BufferUsage::COPY_DST | wgpu::BufferUsage::STORAGE,
                });
                self.material_buffer_size = size;
            }

            self.queue
                .write_buffer(&self.material_buffer, 0, materials.as_bytes());
        }

        for (i, material) in materials {
            self.material_bind_groups.overwrite_val(
                i,
                WgpuBindGroup::new(
                    &self.device,
                    &self.texture_bind_group_layout,
                    material,
                    &self.textures,
                ),
            );
        }

        self.uniform_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform-bind-group"),
            layout: &self.uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(
                        self.uniform_camera_buffer
                            .slice(0..Self::UNIFORM_CAMERA_SIZE),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(
                        self.material_buffer.slice(0..self.material_buffer_size),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.texture_sampler),
                },
            ],
        });

        self.materials_changed = true;
    }

    fn set_textures(&mut self, textures: ChangedIterator<'_, rfw::prelude::Texture>) {
        for (i, tex) in textures {
            if let Some(t) = self.textures.get_mut(i) {
                t.update(&self.device, &self.queue, tex);
            } else {
                self.textures
                    .overwrite_val(i, WgpuTexture::new(&self.device, &self.queue, tex));
            }
        }

        self.d2_renderer
            .update_bind_groups(&self.device, &self.textures);
    }

    fn synchronize(&mut self) {
        {
            let meshes = &mut self.meshes;

            for result in self
                .task_pool
                .sync()
                .filter(|t| t.is_some())
                .map(|t| t.unwrap())
            {
                match result {
                    TaskResult::Mesh(id, mesh) => {
                        meshes.overwrite(id, mesh);
                    }
                }
            }
        }

        Self::record_update(
            &self.device,
            &self.queue,
            &mut self.instances,
            &self.meshes,
            &self.skins,
        );

        self.lights_changed |= self.lights.synchronize(&self.device, &self.queue);

        if self.lights_changed {
            self.radiance_pass.update_bind_groups(
                &self.device,
                &self.output,
                &self.lights,
                &self.uniform_camera_buffer,
                &self.material_buffer,
            );
        }

        self.skins.reset_changed();
        self.meshes.reset_changed();
    }

    fn render(&mut self, camera: &Camera, _mode: RenderMode) {
        let output = match self.swap_chain.get_current_frame() {
            Ok(output) => output,
            Err(_) => return,
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render"),
            });
        let light_counts = self.lights.counts();
        Self::render_lights(
            &mut encoder,
            &mut self.lights,
            &self.instances,
            &self.meshes,
        );

        Self::render_scene(
            &self.device,
            &mut encoder,
            camera,
            light_counts,
            &self.pipeline,
            &self.instances,
            &self.meshes,
            &self.output,
            &self.uniform_camera_buffer,
            &self.uniform_bind_group,
            self.material_bind_groups.as_slice(),
            &self.ssao_pass,
            &self.radiance_pass,
        );

        let mut output_pass = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("output-pass"),
            });

        if self.debug_view == output::WgpuView::Output {
            // self.blit_pass.render(&mut output_pass, &output.output.view);
            self.blit_pass
                .render(&mut output_pass, &self.output.output_texture_view);
        } else {
            self.output
                .blit_debug(&output.output.view, &mut output_pass, self.debug_view);
        }

        let mut d2_encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        self.d2_renderer.render(
            &mut d2_encoder,
            &self.output.output_texture_view,
            &self.output.depth_texture_view,
        );

        self.output_pass
            .render(&mut d2_encoder, &output.output.view);

        self.queue.submit(vec![
            encoder.finish(),
            output_pass.finish(),
            d2_encoder.finish(),
        ]);

        self.instances.reset_changed();
        self.lights_changed = false;
    }

    fn resize<T: HasRawWindowHandle>(
        &mut self,
        _window: &T,
        window_size: (usize, usize),
        render_size: (usize, usize),
    ) {
        let (width, height) = window_size;
        let (render_width, render_height) = render_size;

        self.swap_chain = self.device.create_swap_chain(
            &self.surface,
            &wgpu::SwapChainDescriptor {
                width: width as u32,
                height: height as u32,
                present_mode: Self::PRESENT_MODE,
                format: Self::OUTPUT_FORMAT,
                usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            },
        );

        self.output
            .resize(&self.device, render_width, render_height);
        self.radiance_pass.update_bind_groups(
            &self.device,
            &self.output,
            &self.lights,
            &self.uniform_camera_buffer,
            &self.material_buffer,
        );
        self.ssao_pass
            .update_bind_groups(&self.device, &self.output);
        self.blit_pass
            .update_bind_groups(&self.device, &self.output);
        self.output_pass
            .update_bind_groups(&self.device, &self.output);
    }

    fn set_point_lights(&mut self, _lights: ChangedIterator<'_, PointLight>) {
        self.lights_changed = true;
    }

    fn set_spot_lights(&mut self, lights: ChangedIterator<'_, SpotLight>) {
        self.lights
            .set_spot_lights(lights.changed(), lights.as_slice(), &self.scene_bounds);
        self.lights_changed = true;
    }

    fn set_area_lights(&mut self, lights: ChangedIterator<'_, AreaLight>) {
        self.lights
            .set_area_lights(lights.changed(), lights.as_slice(), &self.scene_bounds);
        self.lights_changed = true;
    }

    fn set_directional_lights(&mut self, lights: ChangedIterator<'_, DirectionalLight>) {
        self.lights
            .set_directional_lights(lights.changed(), lights.as_slice(), &self.scene_bounds);
        self.lights_changed = true;
    }

    fn set_skybox(&mut self, _skybox: Texture) {
        unimplemented!()
    }

    fn set_skins(&mut self, skins: ChangedIterator<'_, Skin>) {
        for (i, skin) in skins {
            self.skins.overwrite(i, skin.clone());
        }
    }

    fn get_settings(&self) -> Vec<Setting> {
        vec![Setting::new(
            String::from("debug-view"),
            SettingValue::Int(0),
            Some(0..8),
        )]
    }

    fn set_setting(&mut self, setting: rfw::prelude::Setting) {
        if setting.key() == "debug-view" {
            let debug_view = match setting.value() {
                SettingValue::Int(i) => output::WgpuView::from(*i),
                _ => output::WgpuView::Output,
            };

            self.debug_view = debug_view;
        }
    }
}

impl WgpuBackend {
    fn record_update(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &mut InstanceList,
        meshes: &TrackedStorage<WgpuMesh>,
        skins: &TrackedStorage<Skin>,
    ) {
        meshes
            .iter_changed()
            .par_bridge()
            .for_each(|(_, m)| m.copy_data(queue));
        instances.update(device, &meshes, skins, &queue);
    }

    fn render_lights(
        encoder: &mut wgpu::CommandEncoder,
        lights: &mut WgpuLights,
        instances: &InstanceList,
        meshes: &TrackedStorage<WgpuMesh>,
    ) {
        lights.render(encoder, instances, meshes);
    }

    fn render_scene(
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        camera: &Camera,
        light_counts: [u32; 4],
        pipeline: &pipeline::RenderPipeline,
        instances: &InstanceList,
        meshes: &TrackedStorage<WgpuMesh>,
        d_output: &output::WgpuOutput,
        uniform_camera_buffer: &wgpu::Buffer,
        uniform_bind_group: &wgpu::BindGroup,
        material_bind_groups: &[WgpuBindGroup],
        ssao_pass: &pass::SSAOPass,
        radiance_pass: &pass::RadiancePass,
    ) {
        let camera_data = {
            let mut data = [0 as u8; Self::UNIFORM_CAMERA_SIZE as usize];
            let view = camera.get_rh_view_matrix();
            let projection = camera.get_rh_projection();

            unsafe {
                let ptr = data.as_mut_ptr();
                // View matrix
                ptr.copy_from(view.as_ref().as_ptr() as *const u8, 64);
                // Projection matrix
                ptr.add(64)
                    .copy_from(projection.as_ref().as_ptr() as *const u8, 64);

                // Light counts
                ptr.add(128)
                    .copy_from(light_counts.as_ptr() as *const u8, 16);

                // Camera position
                ptr.add(144).copy_from(
                    Vec3::from(camera.pos).extend(1.0).as_ref().as_ptr() as *const u8,
                    16,
                );
            }

            data
        };

        let camera_staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: &camera_data,
            usage: wgpu::BufferUsage::COPY_SRC,
        });

        encoder.copy_buffer_to_buffer(
            &camera_staging_buffer,
            0,
            uniform_camera_buffer,
            0,
            Self::UNIFORM_CAMERA_SIZE,
        );

        use output::*;

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[
                    d_output.as_descriptor(WgpuView::Albedo),
                    d_output.as_descriptor(WgpuView::Normal),
                    d_output.as_descriptor(WgpuView::WorldPos),
                    d_output.as_descriptor(WgpuView::ScreenSpace),
                    d_output.as_descriptor(WgpuView::MatParams),
                ],
                depth_stencil_attachment: Some(d_output.as_depth_descriptor()),
            });

            let matrix = camera.get_rh_matrix();
            let frustrum = rfw::scene::FrustrumG::from_matrix(matrix);

            let device_instance = &instances.device_instances;

            let instance_ids = (0..instances.len())
                .into_iter()
                .filter(|i| instances.instances.get(*i).is_some())
                .collect::<Vec<usize>>();

            // Render all instances
            for i in instance_ids.into_iter() {
                // Retrieve instance info
                let (instance, bounds) = match instances.get(i) {
                    Some((i, b)) => {
                        // Check whether instance is valid and in frustrum
                        if i.object_id == ObjectRef::None
                            || !frustrum.aabb_in_frustrum(&b.root_bounds).should_render()
                        {
                            continue;
                        }

                        (i, b)
                    }
                    _ => continue,
                };

                let mesh_id = if let Some(id) = instance.object_id {
                    id as usize
                } else {
                    continue;
                };

                let mesh = &meshes[mesh_id as usize];
                if let Some(buffer) = instances.vertex_buffers[i].as_ref() {
                    render_pass.set_pipeline(&pipeline.pipeline);
                    render_pass.set_bind_group(0, uniform_bind_group, &[]);
                    render_pass.set_bind_group(
                        1,
                        &device_instance.bind_group,
                        &[DeviceInstances::dynamic_offset_for(i) as u32],
                    );

                    let buffer_slice = buffer.slice(0..mesh.buffer_size);
                    render_pass.set_vertex_buffer(0, buffer_slice);
                    render_pass.set_vertex_buffer(1, buffer_slice);
                    render_pass.set_vertex_buffer(2, buffer_slice);
                    render_pass.set_vertex_buffer(3, buffer_slice);
                    render_pass.set_vertex_buffer(4, buffer_slice);

                    mesh.desc
                        .meshes
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| {
                            frustrum
                                .aabb_in_frustrum(&bounds.mesh_bounds[*j])
                                .should_render()
                        })
                        .for_each(|(_, sub_mesh)| {
                            let bind_group = &material_bind_groups[sub_mesh.mat_id as usize];
                            render_pass.set_bind_group(
                                2,
                                bind_group.group.deref().as_ref().unwrap(),
                                &[],
                            );
                            render_pass.draw(sub_mesh.first..sub_mesh.last, 0..1);
                        });
                }
            }
        }

        ssao_pass.launch(
            encoder,
            d_output.width,
            d_output.height,
            &uniform_bind_group,
        );

        radiance_pass.launch(encoder, d_output.width, d_output.height);
    }
}
