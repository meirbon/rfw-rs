use crate::light::WgpuLights;
use crate::mesh::{SkinningPipeline, WgpuSkin};
pub use crate::output::WgpuView;
use futures::executor::block_on;
use mesh::WgpuMesh;
use rfw::prelude::*;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::num::{NonZeroU64, NonZeroU8};
use std::ops::Deref;
use std::sync::Arc;

mod d2;
mod instance;
mod light;
mod mat;
mod mem;
mod mesh;
mod output;
mod pass;
mod pipeline;

use crate::mem::ManagedBuffer;
use mat::*;

#[cfg(feature = "imgui-winit")]
mod gui;

use crate::instance::InstanceList;
#[cfg(feature = "imgui-winit")]
pub use gui::*;
pub use output::WgpuOutput;
use rfw::scene::FrustrumResult;

#[derive(Debug, Clone)]
pub enum TaskResult {
    Mesh(usize, WgpuMesh),
}

#[derive(Debug)]
pub struct WgpuSettings {
    pub view: WgpuView,
    pub enable_skinning: bool,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    scale_factor: f64,
    #[cfg(feature = "imgui-winit")]
    pub imgui: Option<WgpuImGuiContext>,
}

#[cfg(feature = "imgui-winit")]
impl WgpuSettings {
    pub fn setup_imgui(&mut self, window: &winit::window::Window) {
        if self.imgui.is_some() {
            return;
        }

        self.imgui = Some(gui::WgpuImGuiContext::from_winit(
            window,
            &self.device,
            &self.queue,
        ));
    }

    pub fn update_ui<T: 'static>(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::Event<T>,
    ) {
        if let Some(gui) = self.imgui.as_mut() {
            gui.update_ui(window, event);
        }
    }

    pub fn draw_ui<CB>(&mut self, window: &winit::window::Window, draw: CB)
    where
        CB: FnMut(&mut imgui::Ui<'_>),
    {
        if let Some(gui) = self.imgui.as_mut() {
            gui.draw_ui(window, draw);
        }
    }
}

#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct UniformCamera {
    pub view: Mat4,
    pub proj: Mat4,
    pub light_count: [u32; 4],
    pub position: Vec4,
}

pub struct WgpuBackend {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface,
    swap_chain: wgpu::SwapChain,
    meshes: TrackedStorage<mesh::WgpuMesh>,
    instances: TrackedStorage<instance::InstanceList>,
    instance_bind_group_layout: Arc<wgpu::BindGroupLayout>,
    material_buffer: ManagedBuffer<DeviceMaterial>,
    material_bind_groups: FlaggedStorage<WgpuBindGroup>,
    texture_sampler: wgpu::Sampler,
    textures: FlaggedStorage<WgpuTexture>,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    lights: light::WgpuLights,
    skinning_pipeline: SkinningPipeline,

    uniform_bind_group_layout: wgpu::BindGroupLayout,
    uniform_bind_group: wgpu::BindGroup,

    camera_buffer: ManagedBuffer<UniformCamera>,
    output: output::WgpuOutput,
    pipeline: pipeline::RenderPipeline,
    scene_bounds: AABB,

    ssao_pass: pass::SSAOPass,
    radiance_pass: pass::RadiancePass,
    blit_pass: pass::BlitPass,
    output_pass: pass::QuadPass,

    skins: TrackedStorage<WgpuSkin>,

    lights_changed: bool,
    materials_changed: bool,
    instances_changed: bool,

    mesh_bounds: FlaggedStorage<(AABB, Vec<VertexMesh>)>,
    task_pool: ManagedTaskPool<TaskResult>,
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

    fn sync_pool(&mut self) {
        let meshes = &mut self.meshes;

        for result in self
            .task_pool
            .sync()
            .filter(|t| t.is_some())
            .map(|t| t.unwrap())
        {
            match result {
                TaskResult::Mesh(id, mesh) => {
                    if self.instances.get(id).is_none() {
                        self.instances
                            .overwrite(id, instance::InstanceList::default());
                    }

                    meshes.overwrite(id, mesh);
                }
            }
        }
    }
}

impl Backend for WgpuBackend {
    type Settings = WgpuSettings;

    fn init<T: HasRawWindowHandle>(
        window: &T,
        window_size: (u32, u32),
        scale_factor: f64,
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
        let (render_width, render_height) = (
            (width as f64 * scale_factor) as u32,
            (height as f64 * scale_factor) as u32,
        );
        let width = width as u32;
        let height = height as u32;

        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::PUSH_CONSTANTS
                    | wgpu::Features::SAMPLED_TEXTURE_BINDING_ARRAY
                    | wgpu::Features::MAPPABLE_PRIMARY_BUFFERS,
                limits: wgpu::Limits::default(),
                shader_validation: true,
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
                usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
                format: output::WgpuOutput::OUTPUT_FORMAT,
                present_mode: Self::PRESENT_MODE,
            },
        );

        let material_buffer: ManagedBuffer<DeviceMaterial> = ManagedBuffer::new(
            device.clone(),
            queue.clone(),
            wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
            10,
        );

        let texture_bind_group_layout = Self::create_texture_bind_group_layout(&device);
        let instance_bind_group_layout = Arc::new(device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    // Instance matrices
                    binding: 0,
                    count: None,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::StorageBuffer {
                        readonly: true,
                        min_binding_size: None,
                        dynamic: false,
                    },
                }],
                label: Some("mesh-bind-group-descriptor-layout"),
            },
        ));

        let lights = light::WgpuLights::new(10, &device, &instance_bind_group_layout);

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
        });

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
            ],
        });

        let output = output::WgpuOutput::new(&device, render_width, render_height);

        let pipeline = pipeline::RenderPipeline::new(
            &device,
            &uniform_bind_group_layout,
            &instance_bind_group_layout,
            &texture_bind_group_layout,
        );

        let ssao_pass = pass::SSAOPass::new(&device, &uniform_bind_group_layout, &output);
        let radiance_pass = pass::RadiancePass::new(
            &device,
            camera_buffer.buffer(),
            material_buffer.buffer(),
            &output,
            &lights,
        );
        let blit_pass = pass::BlitPass::new(&device, &output);
        let output_pass = pass::QuadPass::new(&device, &output);

        let d2_renderer = d2::Renderer::new(&device, &instance_bind_group_layout);

        let settings = WgpuSettings {
            view: WgpuView::Output,
            enable_skinning: true,
            device: device.clone(),
            queue: queue.clone(),
            scale_factor,
            #[cfg(feature = "imgui-winit")]
            imgui: None,
        };

        let skinning_pipeline = SkinningPipeline::new(&device);

        Ok(Box::new(Self {
            device,
            queue,
            surface,
            swap_chain,
            meshes: TrackedStorage::new(),
            instances: TrackedStorage::new(),
            instance_bind_group_layout,
            material_buffer,
            material_bind_groups: Default::default(),
            texture_sampler,
            textures: FlaggedStorage::new(),
            texture_bind_group_layout,
            lights,
            skinning_pipeline,

            uniform_bind_group_layout,
            uniform_bind_group,
            camera_buffer,
            output,
            pipeline,
            ssao_pass,
            radiance_pass,
            blit_pass,
            output_pass,
            scene_bounds: AABB::empty(),

            skins: TrackedStorage::new(),

            lights_changed: true,
            materials_changed: true,
            instances_changed: true,

            mesh_bounds: FlaggedStorage::new(),
            task_pool: ManagedTaskPool::default(),
            d2_renderer,
            settings,
        }))
    }

    fn set_2d_mesh(&mut self, id: usize, mesh: MeshData2D) {
        self.d2_renderer
            .set_mesh(&self.device, &self.queue, id, mesh);
    }

    fn set_2d_instances(&mut self, id: usize, instances: InstancesData2D<'_>) {
        self.d2_renderer.set_instances(
            id,
            &self.device,
            &self.queue,
            instances,
            &self.instance_bind_group_layout,
        );
    }

    fn set_3d_mesh(&mut self, id: usize, mesh: MeshData3D) {
        let ranges = mesh.ranges.to_vec();
        self.mesh_bounds.overwrite_val(id, (mesh.bounds, ranges));
        let device = self.device.clone();

        let name = mesh.name.to_string();
        let vertices = mesh.vertices.to_vec();
        let ranges = mesh.ranges.to_vec();
        let skin_data = mesh.skin_data.to_vec();
        let bounds = mesh.bounds;

        self.task_pool.push(move |finish| {
            let mesh = mesh::WgpuMesh::new(&device, name, vertices, ranges, skin_data, bounds);
            finish.send(TaskResult::Mesh(id, mesh));
        });
    }

    fn unload_3d_meshes(&mut self, ids: Vec<usize>) {
        for id in ids {
            match self.meshes.erase(id) {
                Ok(_) => {}
                Err(_) => panic!("mesh id {} did not exist", id),
            }

            match self.instances.erase(id) {
                Ok(_) => {}
                Err(_) => panic!("mesh id {} did not exist", id),
            }
        }
    }

    fn set_3d_instances(&mut self, mesh: usize, instances: InstancesData3D<'_>) {
        self.sync_pool();
        if let Some(i) = self.instances.get_mut(mesh) {
            let m = &self.meshes[mesh];
            for matrix in instances.matrices {
                self.scene_bounds
                    .grow_bb(&m.bounds.transformed(matrix.to_cols_array()));
            }

            i.update(
                &self.device,
                &self.queue,
                m,
                instances,
                &self.instance_bind_group_layout,
                self.skins.as_slice(),
                &self.skinning_pipeline,
            );
        }

        self.instances_changed = true;
    }

    fn set_materials(&mut self, materials: &[DeviceMaterial], changed: &BitSlice) {
        {
            if materials.len() > self.material_buffer.len() {
                self.material_buffer.resize(materials.len() * 2);
                self.material_buffer.as_mut_slice()[0..materials.len()].clone_from_slice(materials);
                self.material_buffer.copy_to_device();
            }
        }

        for i in 0..materials.len() {
            if !changed[i] {
                continue;
            }

            let material = &materials[i];
            if let Some(bind_group) = self.material_bind_groups.get_mut(i) {
                bind_group.update(
                    &self.device,
                    &self.texture_bind_group_layout,
                    material,
                    &self.textures,
                );
            } else {
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
            ],
        });

        self.materials_changed = true;
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

        self.d2_renderer
            .update_bind_groups(&self.device, self.textures.as_slice());
    }

    fn synchronize(&mut self) {
        self.sync_pool();

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
        self.meshes.reset_changed();
    }

    fn render(&mut self, camera: CameraView3D, _mode: RenderMode) {
        let output = match self.swap_chain.get_current_frame() {
            Ok(output) => output,
            Err(_) => return,
        };

        {
            let cam = &mut self.camera_buffer.as_mut_slice()[0];
            cam.view = camera.get_rh_view_matrix();
            cam.proj = camera.get_rh_projection();
            cam.light_count = self.lights.counts();
            cam.position = Vec3::from(camera.pos).extend(1.0);
        }
        self.camera_buffer.copy_to_device();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render"),
            });

        if self.instances_changed {
            Self::render_lights(
                &mut encoder,
                &mut self.lights,
                &self.instances,
                &self.meshes,
            );
            self.instances_changed = false;
        }

        Self::render_scene(
            &mut encoder,
            FrustrumG::from_matrix(camera.get_rh_matrix()),
            &self.pipeline,
            &self.instances,
            &self.meshes,
            &self.output,
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

        if self.settings.view == WgpuView::Output {
            self.blit_pass
                .render(&mut output_pass, &self.output.output_texture_view);
        } else {
            self.output.blit_debug(
                &self.output.output_texture_view,
                &mut output_pass,
                self.settings.view,
            );
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

        self.queue
            .submit(vec![encoder.finish(), output_pass.finish()]);

        #[cfg(feature = "imgui-winit")]
        if let Some(imgui) = self.settings.imgui.as_mut() {
            let mut render_pass = d2_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                    attachment: &output.output.view,
                    resolve_target: None,
                }],
                depth_stencil_attachment: None,
            });

            let render = imgui.draw_data.take();
            if let Some(ui) = render {
                let draw_data = unsafe { ui.as_ref() }.unwrap();
                imgui
                    .renderer
                    .render(draw_data, &self.queue, &self.device, &mut render_pass)
                    .expect("Could not render imgui.");
            }
        }

        self.queue.submit(Some(d2_encoder.finish()));
        self.lights_changed = false;
    }

    fn resize<T: HasRawWindowHandle>(
        &mut self,
        _window: &T,
        window_size: (u32, u32),
        scale_factor: f64,
    ) {
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
                usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
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
                s.update(&self.device, &self.queue, skins[i]);
            } else {
                self.skins
                    .overwrite(i, WgpuSkin::new(&self.device, skins[i]));
            }
        }
    }

    fn settings(&mut self) -> &mut Self::Settings {
        &mut self.settings
    }
}

impl WgpuBackend {
    fn render_lights(
        encoder: &mut wgpu::CommandEncoder,
        lights: &mut WgpuLights,
        instances: &TrackedStorage<InstanceList>,
        meshes: &TrackedStorage<WgpuMesh>,
    ) {
        lights.render(encoder, instances, meshes);
    }

    fn render_scene(
        encoder: &mut wgpu::CommandEncoder,
        frustrum: FrustrumG,
        pipeline: &pipeline::RenderPipeline,
        instances: &TrackedStorage<InstanceList>,
        meshes: &TrackedStorage<WgpuMesh>,
        d_output: &output::WgpuOutput,
        uniform_bind_group: &wgpu::BindGroup,
        material_bind_groups: &[WgpuBindGroup],
        ssao_pass: &pass::SSAOPass,
        radiance_pass: &pass::RadiancePass,
    ) {
        use output::*;

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

        render_pass.set_pipeline(&pipeline.pipeline);
        render_pass.set_bind_group(0, uniform_bind_group, &[]);

        for (id, i) in instances.iter() {
            let m = &meshes[id];
            let instances = i.len() as usize;
            if instances == 0 {
                continue;
            }

            render_pass.set_bind_group(1, (*i.instances_bg).as_ref().unwrap(), &[]);
            if i.supports_skinning {
                for instance in 0..instances {
                    if frustrum.aabb_in_frustrum(&i.instances_bounds[instance])
                        == FrustrumResult::Outside
                    {
                        continue;
                    }

                    let buffer = if let Some(b) = i.buffer_for(instance) {
                        b
                    } else {
                        continue;
                    };

                    let buffer_slice = buffer.slice(..);
                    render_pass.set_vertex_buffer(0, buffer_slice);
                    render_pass.set_vertex_buffer(1, buffer_slice);
                    render_pass.set_vertex_buffer(2, buffer_slice);
                    render_pass.set_vertex_buffer(3, buffer_slice);
                    render_pass.set_vertex_buffer(4, buffer_slice);

                    for r in m.ranges.iter() {
                        let bind_group = &material_bind_groups[r.mat_id as usize];
                        render_pass.set_bind_group(
                            2,
                            bind_group.group.deref().as_ref().unwrap(),
                            &[],
                        );
                        render_pass.draw(r.first..r.last, (instance as u32)..(instance as u32 + 1));
                    }
                }
            } else {
                let buffer = if let Some(buffer) = m.buffer() {
                    buffer
                } else {
                    continue;
                };

                let buffer_slice = buffer.slice(..);
                render_pass.set_vertex_buffer(0, buffer_slice);
                render_pass.set_vertex_buffer(1, buffer_slice);
                render_pass.set_vertex_buffer(2, buffer_slice);
                render_pass.set_vertex_buffer(3, buffer_slice);
                render_pass.set_vertex_buffer(4, buffer_slice);

                // TODO: We should probably do some GPU based culling for huge numbers of instances
                if instances > 20000 {
                    for r in m.ranges.iter() {
                        let bind_group = &material_bind_groups[r.mat_id as usize];
                        render_pass.set_bind_group(
                            2,
                            bind_group.group.deref().as_ref().unwrap(),
                            &[],
                        );
                        render_pass.draw(r.first..r.last, 0..(instances as u32));
                    }
                } else {
                    for instance in 0..(instances as u32) {
                        if frustrum.aabb_in_frustrum(&i.instances_bounds[instance as usize])
                            != FrustrumResult::Outside
                        {
                            for r in m.ranges.iter() {
                                let bind_group = &material_bind_groups[r.mat_id as usize];
                                render_pass.set_bind_group(
                                    2,
                                    bind_group.group.deref().as_ref().unwrap(),
                                    &[],
                                );
                                render_pass.draw(r.first..r.last, instance..(instance + 1));
                            }
                        }
                    }
                }
            }
        }

        drop(render_pass);

        ssao_pass.launch(
            encoder,
            d_output.width,
            d_output.height,
            &uniform_bind_group,
        );

        radiance_pass.launch(encoder, d_output.width, d_output.height);
    }
}
