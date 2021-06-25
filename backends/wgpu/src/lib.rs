use crate::mesh::WgpuSkin;
pub use crate::output::WgpuView;
use bitflags::bitflags;
use futures::executor::block_on;
use rfw::backend::RenderMode;
use rfw::prelude::*;
use std::error::Error;
use std::num::{NonZeroU32, NonZeroU64, NonZeroU8};
use std::sync::Arc;
use std::{
    fmt::{Display, Formatter},
    rc::Rc,
};

mod d2;
mod light;
mod list;
mod mat;
mod mem;
mod mesh;
mod output;
mod pass;
mod pipeline;

use crate::mem::ManagedBuffer;
use list::*;
use mat::*;

pub use output::WgpuOutput;

#[derive(Debug)]
pub struct WgpuSettings {
    pub view: WgpuView,
    pub enable_skinning: bool,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    scale_factor: f64,
}

#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct UniformCamera {
    pub view: Mat4,
    pub proj: Mat4,
    pub matrix_2d: Mat4,
    pub light_count: [u32; 4],
    pub position: Vec4,
}

bitflags! {
    #[derive(Default)]
    pub struct UpdateFlags: u32 {
        const UPDATE_3D_MESHES = 1;
        const UPDATE_3D_INSTANCES = 2;
        const UPDATE_2D_MESHES = 4;
        const UPDATE_2D_INSTANCES = 8;
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub struct InstanceMatrices {
    pub matrix: Mat4,
    pub normal: Mat4,
}

#[derive(Default, Debug, Clone)]
pub struct InstanceExtra {
    flags: Vec<InstanceFlags3D>,
    skin_ids: Vec<Option<u16>>,
    local_aabb: Aabb,
}

pub struct WgpuBackend {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface,
    swap_chain: wgpu::SwapChain,

    mesh_flags: Vec<Mesh3dFlags>,
    vertices_3d: VertexList<Vertex3D, JointData>,
    instances_3d_storage: Vec<Rc<Vec<InstanceMatrices>>>,
    instances_3d: InstanceList<InstanceMatrices, InstanceExtra>,
    vertices_2d: VertexList<Vertex2D, u32>,
    instances_2d: InstanceList<Mat4>,

    update_flags: UpdateFlags,

    material_buffer: ManagedBuffer<DeviceMaterial>,
    texture_sampler: wgpu::Sampler,
    textures: FlaggedStorage<WgpuTexture>,
    texture_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    lights: light::WgpuLights,

    uniform_bind_group_layout: wgpu::BindGroupLayout,
    uniform_bind_group: wgpu::BindGroup,

    camera_buffer: ManagedBuffer<UniformCamera>,
    output: output::WgpuOutput,
    pipeline: pipeline::RenderPipeline,
    scene_bounds: Aabb,

    ssao_pass: pass::SsaoPass,
    radiance_pass: pass::RadiancePass,
    blit_pass: pass::BlitPass,
    output_pass: pass::QuadPass,

    skin_layout: wgpu::BindGroupLayout,
    skins: TrackedStorage<WgpuSkin>,

    lights_changed: bool,
    instances_changed: bool,

    d2_renderer: d2::Renderer,

    settings: WgpuSettings,
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
    const TEXTURE_CAPACITY: usize = 128;
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
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                count: NonZeroU32::new(Self::TEXTURE_CAPACITY as _),
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Uint,
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
            }],
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
                    ty: wgpu::BindingType::Buffer {
                        has_dynamic_offset: false,
                        ty: wgpu::BufferBindingType::Uniform,
                        min_binding_size: NonZeroU64::new(Self::UNIFORM_CAMERA_SIZE as _),
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    // Material mem
                    binding: 1,
                    count: None,
                    visibility: wgpu::ShaderStage::FRAGMENT | wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        has_dynamic_offset: false,
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        min_binding_size: None,
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    // Texture sampler
                    binding: 2,
                    count: None,
                    visibility: wgpu::ShaderStage::FRAGMENT | wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Sampler {
                        filtering: true,
                        comparison: false,
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    // 2D Instance buffer
                    binding: 3,
                    count: None,
                    visibility: wgpu::ShaderStage::VERTEX
                        | wgpu::ShaderStage::FRAGMENT
                        | wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    // 3D Instance buffer
                    binding: 4,
                    count: None,
                    visibility: wgpu::ShaderStage::VERTEX
                        | wgpu::ShaderStage::FRAGMENT
                        | wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                },
            ],
        })
    }
}

impl FromWindowHandle for WgpuBackend {
    fn init<W: HasRawWindowHandle>(
        window: &W,
        width: u32,
        height: u32,
        scale: f64,
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

        let (render_width, render_height) = (
            (width as f64 * scale) as u32,
            (height as f64 * scale) as u32,
        );
        let width = width as u32;
        let height = height as u32;

        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::PUSH_CONSTANTS
                    | wgpu::Features::SAMPLED_TEXTURE_BINDING_ARRAY
                    | wgpu::Features::MAPPABLE_PRIMARY_BUFFERS
                    | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
                limits: wgpu::Limits {
                    max_sampled_textures_per_shader_stage: Self::TEXTURE_CAPACITY as _,
                    ..Default::default()
                },
                label: Some("rfw-device"),
            },
            None,
        ))
        .unwrap();

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let swap_chain = device.create_swap_chain(
            &surface,
            &wgpu::SwapChainDescriptor {
                width,
                height,
                usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
                format: output::WgpuOutput::OUTPUT_FORMAT,
                present_mode: Self::PRESENT_MODE,
            },
        );

        let material_buffer: ManagedBuffer<DeviceMaterial> = ManagedBuffer::new(
            device.clone(),
            queue.clone(),
            wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
            1,
        );

        let texture_bind_group_layout = Self::create_texture_bind_group_layout(&device);

        let mut textures = FlaggedStorage::new();
        let mut dummy_tex = Texture::default();
        dummy_tex.generate_mipmaps(Texture::MIP_LEVELS);

        textures.push(WgpuTexture::new(
            &device,
            &queue,
            TextureData {
                width: dummy_tex.width,
                height: dummy_tex.height,
                mip_levels: dummy_tex.mip_levels,
                bytes: dummy_tex.data.as_bytes(),
                format: DataFormat::BGRA8,
            },
        ));

        let dummy_ref: &wgpu::TextureView = &textures[0].view.as_ref().as_ref().unwrap();
        let references = vec![dummy_ref; Self::TEXTURE_CAPACITY];
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("textures-bind-group"),
            layout: &texture_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureViewArray(references.as_slice()),
            }],
        });

        let camera_buffer = ManagedBuffer::new(
            device.clone(),
            queue.clone(),
            wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            1,
        );

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
            border_color: None,
        });

        let vertices_3d = VertexList::new(&device, &queue);
        let instances_3d_storage = Default::default();
        let instances_3d = InstanceList::new(&device, &queue);

        let vertices_2d = VertexList::new(&device, &queue);
        let instances_2d = InstanceList::new(&device, &queue);

        let uniform_bind_group_layout = Self::create_uniform_bind_group_layout(&device);
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform-bind-group"),
            layout: &uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.binding_resource(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: material_buffer.binding_resource(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&texture_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: instances_2d.get_buffer().binding_resource(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: instances_3d.get_buffer().binding_resource(),
                },
            ],
        });

        let skin_layout = WgpuSkin::create_layout(&device);
        let lights = light::WgpuLights::new(10, &device, &uniform_bind_group_layout, &skin_layout);
        let output = output::WgpuOutput::new(&device, render_width, render_height);

        let pipeline = pipeline::RenderPipeline::new(
            &device,
            &uniform_bind_group_layout,
            &skin_layout,
            &texture_bind_group_layout,
        );

        let ssao_pass = pass::SsaoPass::new(&device, &uniform_bind_group_layout, &output);
        let radiance_pass = pass::RadiancePass::new(
            &device,
            camera_buffer.buffer(),
            material_buffer.buffer(),
            &output,
            &lights,
        );
        let blit_pass = pass::BlitPass::new(&device, &output);
        let output_pass = pass::QuadPass::new(&device, &output);

        let d2_renderer = d2::Renderer::new(
            &device,
            &uniform_bind_group_layout,
            &texture_bind_group_layout,
        );

        let settings = WgpuSettings {
            view: WgpuView::Output,
            enable_skinning: true,
            device: device.clone(),
            queue: queue.clone(),
            scale_factor: scale,
        };

        Ok(Box::new(Self {
            device,
            queue,
            surface,
            swap_chain,

            mesh_flags: Vec::new(),
            vertices_3d,
            instances_3d_storage,
            instances_3d,
            vertices_2d,
            instances_2d,
            update_flags: Default::default(),

            material_buffer,
            texture_sampler,
            textures,
            texture_bind_group,
            texture_bind_group_layout,
            lights,

            uniform_bind_group_layout,
            uniform_bind_group,
            camera_buffer,
            output,
            pipeline,
            ssao_pass,
            radiance_pass,
            blit_pass,
            output_pass,
            scene_bounds: Aabb::empty(),

            skin_layout,
            skins: TrackedStorage::new(),

            lights_changed: true,
            instances_changed: true,

            d2_renderer,
            settings,
        }))
    }
}

impl Backend for WgpuBackend {
    fn set_2d_mesh(&mut self, id: usize, mesh: MeshData2D) {
        if self.vertices_2d.has(id) {
            self.vertices_2d
                .update_pointer(id, mesh.vertices.to_vec(), Vec::new());
        } else {
            self.vertices_2d
                .add_pointer(id, mesh.vertices.to_vec(), Vec::new());
        }

        self.update_flags |= UpdateFlags::UPDATE_2D_MESHES;
    }

    fn set_2d_instances(&mut self, id: usize, instances: InstancesData2D<'_>) {
        if self.instances_2d.has(id) {
            self.instances_2d
                .update_instances_list(id, instances.matrices, ());
        } else {
            self.instances_2d
                .add_instances_list(id, instances.matrices.to_vec(), ());
        }

        self.instances_changed = true;
        self.update_flags |= UpdateFlags::UPDATE_2D_INSTANCES;
    }

    fn set_3d_mesh(&mut self, id: usize, mesh: MeshData3D) {
        if self.mesh_flags.len() <= id {
            self.mesh_flags.resize(
                (id + 1).max(self.mesh_flags.len() * 2),
                Mesh3dFlags::default(),
            );
        }

        self.mesh_flags[id] = mesh.flags;
        if self.vertices_3d.has(id) {
            self.vertices_3d
                .update_pointer(id, mesh.vertices.to_vec(), mesh.skin_data.to_vec());
        } else {
            self.vertices_3d
                .add_pointer(id, mesh.vertices.to_vec(), mesh.skin_data.to_vec());
        }

        self.update_flags |= UpdateFlags::UPDATE_3D_MESHES;
    }

    fn unload_3d_meshes(&mut self, ids: &[usize]) {
        for id in ids.iter().copied() {
            self.instances_3d.remove_instances_list(id);
            self.vertices_3d.remove_pointer(id);
        }
    }

    fn set_3d_instances(&mut self, mesh: usize, instances: InstancesData3D<'_>) {
        if mesh >= self.instances_3d_storage.len() {
            self.instances_3d_storage
                .resize(mesh + 1, Default::default());
        }

        let vec: Vec<InstanceMatrices> = instances
            .matrices
            .iter()
            .copied()
            .map(|m| InstanceMatrices {
                matrix: m,
                normal: m.inverse().transpose(),
            })
            .collect();

        let extra = InstanceExtra {
            flags: instances.flags.to_vec(),
            skin_ids: instances
                .skin_ids
                .iter()
                .map(|i| if i.0 >= 0 { Some(i.0 as u16) } else { None })
                .collect(),
            local_aabb: instances.local_aabb,
        };

        if self.instances_3d.has(mesh) {
            self.instances_3d.update_instances_list(mesh, &vec, extra);
        } else {
            self.instances_3d.add_instances_list(mesh, vec, extra);
        }

        self.update_flags.insert(UpdateFlags::UPDATE_3D_INSTANCES);
    }

    fn set_materials(&mut self, materials: &[DeviceMaterial], _changed: &BitSlice) {
        if materials.len() > self.material_buffer.len() {
            self.material_buffer.resize(materials.len() * 2);
            self.material_buffer.as_mut_slice()[0..materials.len()].clone_from_slice(materials);
            self.material_buffer.copy_to_device();
        }

        self.uniform_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform-bind-group"),
            layout: &self.uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.binding_resource(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.material_buffer.binding_resource(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.texture_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.instances_2d.get_buffer().binding_resource(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.instances_3d.get_buffer().binding_resource(),
                },
            ],
        });
    }

    fn set_textures(&mut self, textures: &[TextureData<'_>], changed: &BitSlice) {
        for i in 0..textures.len() {
            if !changed[i] {
                continue;
            }

            let tex = textures[i];

            if let Some(t) = self.textures.get_mut(i) {
                t.update(&self.device, &self.queue, tex);
            } else {
                self.textures
                    .overwrite_val(i, WgpuTexture::new(&self.device, &self.queue, tex));
            }
        }

        let mut texture_views = Vec::with_capacity(Self::TEXTURE_CAPACITY);
        let dummy_ref = &self.textures[0].view.as_ref().as_ref().unwrap();
        for (_, t) in self.textures.iter() {
            texture_views.push(if let Some(view) = t.view.as_ref() {
                view
            } else {
                dummy_ref
            });
        }

        while texture_views.len() < Self::TEXTURE_CAPACITY {
            texture_views.push(dummy_ref);
        }

        self.texture_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("textures-bind-group"),
            layout: &self.texture_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureViewArray(texture_views.as_slice()),
            }],
        });
    }

    fn synchronize(&mut self) {
        self.lights_changed |= self.lights.synchronize(&self.device, &self.queue);

        if self.lights_changed {
            self.radiance_pass.update_bind_groups(
                &self.device,
                &self.output,
                &self.lights,
                self.camera_buffer.buffer(),
                self.material_buffer.buffer(),
            );
        }

        self.skins.reset_changed();

        self.vertices_3d.update();
        self.vertices_2d.update();

        if self.update_flags.contains(UpdateFlags::UPDATE_3D_INSTANCES) {
            self.instances_3d.update();
        }

        if self.update_flags.contains(UpdateFlags::UPDATE_2D_INSTANCES) {
            self.instances_2d.update();
        }

        self.update_flags = UpdateFlags::empty();

        self.uniform_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform-bind-group"),
            layout: &self.uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.binding_resource(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.material_buffer.binding_resource(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.texture_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.instances_2d.get_buffer().binding_resource(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.instances_3d.get_buffer().binding_resource(),
                },
            ],
        });
    }

    fn render(&mut self, camera_2d: CameraView2D, camera_3d: CameraView3D, mode: RenderMode) {
        let output = match self.swap_chain.get_current_frame() {
            Ok(output) => output,
            Err(_) => return,
        };

        {
            let cam = &mut self.camera_buffer.as_mut_slice()[0];
            cam.view = camera_3d.get_rh_view_matrix();
            cam.proj = camera_3d.get_rh_projection();
            cam.matrix_2d = camera_2d.matrix;
            cam.light_count = self.lights.counts();
            cam.position = camera_3d.pos.extend(1.0);
        }
        self.camera_buffer.copy_to_device();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render"),
            });

        if self.instances_changed {
            encoder.insert_debug_marker("lights");
            self.render_lights(&mut encoder);
            self.instances_changed = false;
        }

        encoder.insert_debug_marker("render");
        self.render_scene(
            &mut encoder,
            FrustrumG::from_matrix(camera_3d.get_rh_matrix()),
        );
        self.queue.submit(Some(encoder.finish()));

        let mut output_encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("output-encoder"),
                });
        output_encoder.insert_debug_marker("output");
        if mode == RenderMode::Default {
            self.blit_pass
                .render(&mut output_encoder, &self.output.output_texture_view);
        } else {
            self.output.blit_debug(
                &self.output.output_texture_view,
                &mut output_encoder,
                match mode {
                    RenderMode::Default => WgpuView::Output,
                    RenderMode::Normal => WgpuView::Normal,
                    RenderMode::Albedo => WgpuView::Albedo,
                    RenderMode::GBuffer => WgpuView::GBuffer,
                    RenderMode::ScreenSpace => WgpuView::ScreenSpace,
                    RenderMode::Ssao => WgpuView::Ssao,
                    RenderMode::FilteredSsao => WgpuView::FilteredSsao,
                },
            );
        }

        self.d2_renderer.render_list(
            &mut output_encoder,
            &self.uniform_bind_group,
            &self.texture_bind_group,
            &self.vertices_2d,
            &self.instances_2d,
            &self.output.output_texture_view,
            &self.output.depth_texture_view,
        );

        self.output_pass
            .render(&mut output_encoder, &output.output.view);
        self.queue.submit(Some(output_encoder.finish()));
        self.lights_changed = false;
    }

    fn resize(&mut self, window_size: (u32, u32), scale_factor: f64) {
        self.device.poll(wgpu::Maintain::Wait);
        self.settings.scale_factor = scale_factor;
        let (width, height) = window_size;
        let (render_width, render_height) = (
            (width as f64 * scale_factor) as u32,
            (height as f64 * scale_factor) as u32,
        );

        self.swap_chain = self.device.create_swap_chain(
            &self.surface,
            &wgpu::SwapChainDescriptor {
                width: width as u32,
                height: height as u32,
                present_mode: Self::PRESENT_MODE,
                format: Self::OUTPUT_FORMAT,
                usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
            },
        );

        self.output
            .resize(&self.device, render_width, render_height);
        self.radiance_pass.update_bind_groups(
            &self.device,
            &self.output,
            &self.lights,
            self.camera_buffer.buffer(),
            self.material_buffer.buffer(),
        );
        self.ssao_pass
            .update_bind_groups(&self.device, &self.output);
        self.blit_pass
            .update_bind_groups(&self.device, &self.output);
        self.output_pass
            .update_bind_groups(&self.device, &self.output);
    }

    fn set_point_lights(&mut self, _lights: &[PointLight], _changed: &BitSlice) {
        self.lights_changed = true;
    }

    fn set_spot_lights(&mut self, lights: &[SpotLight], changed: &BitSlice) {
        self.lights
            .set_spot_lights(changed, lights, &self.scene_bounds);
        self.lights_changed = true;
    }

    fn set_area_lights(&mut self, lights: &[AreaLight], changed: &BitSlice) {
        self.lights
            .set_area_lights(changed, lights, &self.scene_bounds);
        self.lights_changed = true;
    }

    fn set_directional_lights(&mut self, lights: &[DirectionalLight], changed: &BitSlice) {
        self.lights
            .set_directional_lights(changed, lights, &self.scene_bounds);
        self.lights_changed = true;
    }

    fn set_skybox(&mut self, _skybox: TextureData) {
        unimplemented!()
    }

    fn set_skins(&mut self, skins: &[SkinData], changed: &BitSlice) {
        for i in 0..skins.len() {
            if !changed[i] {
                continue;
            }

            if let Some(s) = self.skins.get_mut(i) {
                s.update(&self.device, &self.queue, &self.skin_layout, skins[i]);
            } else {
                self.skins
                    .overwrite(i, WgpuSkin::new(&self.device, &self.skin_layout, skins[i]));
            }
        }

        self.instances_changed = true;
    }
}

impl WgpuBackend {
    fn render_lights(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.lights.render(
            encoder,
            &self.uniform_bind_group,
            &self.vertices_3d,
            &self.instances_3d,
            self.mesh_flags.as_slice(),
            self.skins.as_slice(),
        );
    }

    fn render_scene(&self, encoder: &mut wgpu::CommandEncoder, _frustrum: FrustrumG) {
        use output::*;

        if self.vertices_3d.requires_update() {
            return;
        }

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[
                self.output.as_descriptor(WgpuView::Albedo),
                self.output.as_descriptor(WgpuView::Normal),
                self.output.as_descriptor(WgpuView::GBuffer),
                self.output.as_descriptor(WgpuView::ScreenSpace),
                self.output.as_descriptor(WgpuView::MatParams),
            ],
            depth_stencil_attachment: Some(self.output.as_depth_descriptor()),
        });

        let jw_buffer = self.vertices_3d.get_jw_buffer().buffer();
        let vertex_buffer = self.vertices_3d.get_vertex_buffer().buffer();

        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        render_pass.set_bind_group(1, &self.texture_bind_group, &[]);

        let v_ranges = self.vertices_3d.get_ranges();
        let i_ranges = self.instances_3d.get_ranges();

        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));

        for (i, r) in i_ranges.iter() {
            if r.count == 0 {
                continue;
            }

            let v = v_ranges.get(i).unwrap();
            let skins = r.extra.skin_ids.as_slice();

            if self.settings.enable_skinning
                && (v.jw_end - v.jw_start) > 0
                && skins.len() == (r.count as usize)
            {
                for i in 0..r.count {
                    if let Some(skin) = skins.get(i as usize).and_then(|i| {
                        i.and_then(|i| {
                            self.skins
                                .get(i as usize)
                                .and_then(|s| s.bind_group.as_ref())
                        })
                    }) {
                        // animated mesh
                        render_pass.set_pipeline(&self.pipeline.anim_pipeline);
                        render_pass.set_vertex_buffer(
                            0,
                            vertex_buffer.slice(
                                ((v.start as usize * std::mem::size_of::<Vertex3D>())
                                    as wgpu::BufferAddress)..,
                            ),
                        );
                        render_pass.set_vertex_buffer(
                            1,
                            jw_buffer.slice(
                                ((v.jw_start as usize * std::mem::size_of::<JointData>())
                                    as wgpu::BufferAddress)..,
                            ),
                        );
                        render_pass.set_bind_group(2, skin, &[]);
                        render_pass.draw(0..(v.end - v.start), (r.start + i)..(r.start + i + 1));
                    } else {
                        render_pass.set_pipeline(&self.pipeline.pipeline);
                        render_pass.draw(0..(v.end - v.start), (r.start + i)..(r.start + i + 1));
                    }
                }

                render_pass.set_pipeline(&self.pipeline.pipeline);
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            } else {
                // static mesh
                render_pass.set_pipeline(&self.pipeline.pipeline);
                render_pass.draw(v.start..v.end, r.start..r.end);
            }
        }

        drop(render_pass);

        self.ssao_pass.launch(
            encoder,
            self.output.width,
            self.output.height,
            &self.uniform_bind_group,
        );

        self.radiance_pass
            .launch(encoder, self.output.width, self.output.height);
    }
}
