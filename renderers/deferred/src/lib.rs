use crate::instance::{DeviceInstances, InstanceList};
use crate::light::DeferredLights;
use crate::mesh::DeferredAnimMesh;
use crate::skin::DeferredSkin;
use futures::executor::block_on;
use glam::*;
use mesh::DeferredMesh;
use rtbvh::AABB;

use rfw_scene::r2d::{D2Instance, D2Mesh};
use rfw_scene::{
    graph::Skin,
    raw_window_handle::HasRawWindowHandle,
    renderers::{RenderMode, Renderer, Setting, SettingValue},
    AnimatedMesh, Camera, ChangedIterator, DeviceMaterial, FlaggedStorage, Instance, Mesh,
    ObjectRef, Texture, TrackedStorage, VertexMesh,
};
use rfw_utils::*;
use shared::BytesConversion;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::num::{NonZeroU64, NonZeroU8};
use std::sync::Arc;
use wgpu::util::DeviceExt;

mod d2;
mod instance;
mod light;
mod mesh;
mod output;
mod pass;
mod pipeline;
mod skin;

#[derive(Debug, Clone)]
pub enum TaskResult {
    Mesh(usize, DeferredMesh),
    AnimMesh(usize, DeferredAnimMesh),
}

pub struct Deferred {
    device: Arc<wgpu::Device>,
    queue: wgpu::Queue,
    surface: wgpu::Surface,
    swap_chain: wgpu::SwapChain,
    meshes: TrackedStorage<mesh::DeferredMesh>,
    anim_meshes: TrackedStorage<mesh::DeferredAnimMesh>,
    instances: instance::InstanceList,
    material_buffer: wgpu::Buffer,
    material_buffer_size: wgpu::BufferAddress,
    material_bind_groups: Vec<wgpu::BindGroup>,
    texture_sampler: wgpu::Sampler,
    textures: Vec<wgpu::Texture>,
    texture_views: Vec<wgpu::TextureView>,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    lights: light::DeferredLights,

    uniform_bind_group_layout: wgpu::BindGroupLayout,
    uniform_bind_group: wgpu::BindGroup,

    uniform_camera_buffer: wgpu::Buffer,
    output: output::DeferredOutput,
    pipeline: pipeline::RenderPipeline,
    scene_bounds: AABB,

    ssao_pass: pass::SSAOPass,
    radiance_pass: pass::RadiancePass,
    blit_pass: pass::BlitPass,

    skins: TrackedStorage<DeferredSkin>,
    skin_bind_group_layout: wgpu::BindGroupLayout,

    debug_view: output::DeferredView,
    lights_changed: bool,
    materials_changed: bool,

    mesh_bounds: FlaggedStorage<(AABB, Vec<VertexMesh>)>,
    anim_mesh_bounds: FlaggedStorage<(AABB, Vec<VertexMesh>)>,
    task_pool: ManagedTaskPool<TaskResult>,
    d2_renderer: d2::Renderer,
}

#[derive(Debug, Copy, Clone)]
enum DeferredError {
    RequestDeviceError,
}

impl std::error::Error for DeferredError {}

impl Display for DeferredError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not retrieve valid device.")
    }
}

impl Deferred {
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

impl Renderer for Deferred {
    fn init<T: HasRawWindowHandle>(
        window: &T,
        width: usize,
        height: usize,
    ) -> Result<Box<Self>, Box<dyn Error>> {
        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
        let surface = unsafe { instance.create_surface(window) };
        let adapter = match block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: wgpu::PowerPreference::HighPerformance,
        })) {
            None => return Err(Box::new(DeferredError::RequestDeviceError)),
            Some(adapter) => adapter,
        };

        println!("Picked device: {}", adapter.get_info().name);

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
                width: width as u32,
                height: height as u32,
                usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
                format: output::DeferredOutput::OUTPUT_FORMAT,
                present_mode: wgpu::PresentMode::Mailbox,
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

        let skin_bind_group_layout = DeferredSkin::create_bind_group_layout(&device);

        let lights = light::DeferredLights::new(
            10,
            &device,
            &instances.bind_group_layout,
            &skin_bind_group_layout,
        );

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

        let output = output::DeferredOutput::new(&device, width, height);

        let pipeline = pipeline::RenderPipeline::new(
            &device,
            &uniform_bind_group_layout,
            &instances.bind_group_layout,
            &texture_bind_group_layout,
            &skin_bind_group_layout,
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

        let d2_renderer = d2::Renderer::new(&device);

        Ok(Box::new(Self {
            device: Arc::new(device),
            queue,
            surface,
            swap_chain,
            meshes: TrackedStorage::new(),
            anim_meshes: TrackedStorage::new(),
            instances,
            material_buffer,
            material_buffer_size,
            material_bind_groups: Vec::new(),
            texture_sampler,
            textures: Vec::new(),
            texture_views: Vec::new(),
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
            scene_bounds: AABB::new(),

            skins: TrackedStorage::new(),
            skin_bind_group_layout,

            debug_view: output::DeferredView::Output,
            lights_changed: true,
            materials_changed: true,

            mesh_bounds: FlaggedStorage::new(),
            anim_mesh_bounds: FlaggedStorage::new(),
            task_pool: ManagedTaskPool::default(),
            d2_renderer,
        }))
    }

    fn set_2d_meshes(&mut self, meshes: ChangedIterator<'_, D2Mesh>) {
        self.d2_renderer.update_meshes(&self.device, meshes);
    }

    fn set_2d_instances(&mut self, instances: ChangedIterator<'_, D2Instance>) {
        self.d2_renderer.update_instances(&self.queue, instances);
    }

    fn set_meshes(&mut self, meshes: ChangedIterator<'_, Mesh>) {
        for (id, mesh) in meshes {
            self.mesh_bounds
                .overwrite_val(id, (mesh.bounds.clone(), mesh.meshes.clone()));
            let device = self.device.clone();
            let mesh = mesh.clone();
            self.task_pool.push(move |finish| {
                let mesh = mesh::DeferredMesh::new(&device, &mesh);
                finish.send(TaskResult::Mesh(id, mesh));
            });
        }
    }

    fn set_animated_meshes(&mut self, meshes: ChangedIterator<'_, AnimatedMesh>) {
        for (id, mesh) in meshes {
            self.anim_mesh_bounds
                .overwrite_val(id, (mesh.bounds.clone(), mesh.meshes.clone()));
            let device = self.device.clone();
            let mesh = mesh.clone();
            self.task_pool.push(move |finish| {
                let mesh = mesh::DeferredAnimMesh::new(&device, &mesh);
                finish.send(TaskResult::AnimMesh(id, mesh));
            });
        }
    }

    fn set_instances(&mut self, instances: ChangedIterator<'_, Instance>) {
        for (id, instance) in instances {
            match instance.object_id {
                ObjectRef::None => {
                    self.instances.set(
                        &self.device,
                        id,
                        instance.clone(),
                        &(AABB::empty(), Vec::new()),
                    );
                }
                ObjectRef::Static(mesh_id) => {
                    self.instances.set(
                        &self.device,
                        id,
                        instance.clone(),
                        &self.mesh_bounds[mesh_id as usize],
                    );
                }
                ObjectRef::Animated(mesh_id) => {
                    self.instances.set(
                        &self.device,
                        id,
                        instance.clone(),
                        &self.anim_mesh_bounds[mesh_id as usize],
                    );
                }
            }

            self.scene_bounds.grow_bb(
                &instance
                    .local_bounds()
                    .transformed(instance.get_transform().to_cols_array()),
            );
        }
    }

    fn set_materials(&mut self, materials: ChangedIterator<'_, rfw_scene::DeviceMaterial>) {
        let materials = materials.as_slice();
        let size = (materials.len() * std::mem::size_of::<DeviceMaterial>()) as wgpu::BufferAddress;

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

        self.material_bind_groups = (0..materials.len())
            .map(|i| {
                let material = &materials[i];
                let albedo_tex = material.diffuse_map.max(0) as usize;
                let normal_tex = material.normal_map.max(0) as usize;
                let roughness_tex = material.metallic_roughness_map.max(0) as usize;
                let emissive_tex = material.emissive_map.max(0) as usize;
                let sheen_tex = material.sheen_map.max(0) as usize;

                let albedo_view = &self.texture_views[albedo_tex];
                let normal_view = &self.texture_views[normal_tex];
                let roughness_view = &self.texture_views[roughness_tex];
                let emissive_view = &self.texture_views[emissive_tex];
                let sheen_view = &self.texture_views[sheen_tex];

                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
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
                    layout: &self.texture_bind_group_layout,
                })
            })
            .collect();

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

    fn set_textures(&mut self, textures: ChangedIterator<'_, rfw_scene::Texture>) {
        // TODO: We should only update changed textures, not all
        self.textures.clear();
        self.texture_views.clear();
        self.material_bind_groups.clear();

        let textures = textures.as_slice();
        let staging_size =
            textures.iter().map(|t| t.data.len()).sum::<usize>() * std::mem::size_of::<u32>();
        let mut data = vec![0_u8; staging_size];
        let mut data_ptr = data.as_mut_ptr() as *mut u32;

        {
            for tex in textures.iter() {
                unsafe {
                    std::ptr::copy(tex.data.as_ptr(), data_ptr, tex.data.len());
                    data_ptr = data_ptr.add(tex.data.len());
                }
            }
        }

        let mut offset = 0 as wgpu::BufferAddress;
        for (i, tex) in textures.iter().enumerate() {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(format!("texture-{}", i).as_str()),
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
                let offset = offset + local_offset * std::mem::size_of::<u32>() as u64;

                let end = (width as usize * height as usize * std::mem::size_of::<u32>()) as u64;

                self.queue.write_texture(
                    wgpu::TextureCopyView {
                        mip_level: i as u32,
                        origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                        texture: &texture,
                    },
                    &data[(offset as usize)..(offset + end) as usize],
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

            offset += (tex.data.len() * std::mem::size_of::<u32>()) as wgpu::BufferAddress;
            self.textures.push(texture);
        }

        self.texture_views = self
            .textures
            .iter()
            .map(|t| {
                t.create_view(&wgpu::TextureViewDescriptor {
                    label: None,
                    format: Some(wgpu::TextureFormat::Bgra8Unorm),
                    array_layer_count: None,
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    level_count: None,
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: 0,
                })
            })
            .collect();

        self.d2_renderer
            .update_bind_groups(&self.device, self.texture_views.as_slice());
    }

    fn synchronize(&mut self) {
        {
            let meshes = &mut self.meshes;
            let anim_meshes = &mut self.anim_meshes;

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
                    TaskResult::AnimMesh(id, mesh) => {
                        anim_meshes.overwrite(id, mesh);
                    }
                }
            }
        }

        Self::record_update(
            &self.device,
            &self.queue,
            &mut self.instances,
            &self.meshes,
            &self.anim_meshes,
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
        self.anim_meshes.reset_changed();
    }

    fn render(&mut self, camera: &rfw_scene::Camera, _mode: RenderMode) {
        let output = match self.swap_chain.get_current_frame() {
            Ok(output) => output,
            Err(_) => return,
        };

        let light_counts = self.lights.counts();
        let light_pass = Self::render_lights(
            &self.device,
            &mut self.lights,
            &self.instances,
            &self.meshes,
            &self.anim_meshes,
            &self.skins,
        );

        let render_pass = Self::render_scene(
            &self.device,
            camera,
            light_counts,
            &self.pipeline,
            &self.instances,
            &self.meshes,
            &self.anim_meshes,
            &self.skins,
            &self.output,
            &self.uniform_camera_buffer,
            &self.uniform_bind_group,
            self.material_bind_groups.as_slice(),
            &self.ssao_pass,
            &self.radiance_pass,
        );

        self.queue.submit(std::iter::once(light_pass));
        let mut output_pass = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("output-pass"),
            });

        if self.debug_view == output::DeferredView::Output {
            self.blit_pass.render(&mut output_pass, &output.output.view);
        } else {
            self.output
                .blit_debug(&output.output.view, &mut output_pass, self.debug_view);
        }

        let mut d2_encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        self.d2_renderer.render(
            &mut d2_encoder,
            &output.output.view,
            &self.output.depth_texture_view,
        );

        self.queue
            .submit(vec![render_pass, output_pass.finish(), d2_encoder.finish()]);

        self.instances.reset_changed();
        self.lights_changed = false;
    }

    fn resize<T: HasRawWindowHandle>(&mut self, _window: &T, width: usize, height: usize) {
        self.swap_chain = self.device.create_swap_chain(
            &self.surface,
            &wgpu::SwapChainDescriptor {
                width: width as u32,
                height: height as u32,
                present_mode: wgpu::PresentMode::Mailbox,
                format: Self::OUTPUT_FORMAT,
                usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            },
        );

        self.output.resize(&self.device, width, height);
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
    }

    fn set_point_lights(&mut self, _lights: ChangedIterator<'_, rfw_scene::PointLight>) {
        self.lights_changed = true;
    }

    fn set_spot_lights(&mut self, lights: ChangedIterator<'_, rfw_scene::SpotLight>) {
        self.lights
            .set_spot_lights(lights.changed(), lights.as_slice(), &self.scene_bounds);
        self.lights_changed = true;
    }

    fn set_area_lights(&mut self, lights: ChangedIterator<'_, rfw_scene::AreaLight>) {
        self.lights
            .set_area_lights(lights.changed(), lights.as_slice(), &self.scene_bounds);
        self.lights_changed = true;
    }

    fn set_directional_lights(&mut self, lights: ChangedIterator<'_, rfw_scene::DirectionalLight>) {
        self.lights
            .set_directional_lights(lights.changed(), lights.as_slice(), &self.scene_bounds);
        self.lights_changed = true;
    }

    fn set_skybox(&mut self, _skybox: Texture) {
        unimplemented!()
    }

    fn set_skins(&mut self, skins: ChangedIterator<'_, Skin>) {
        for (i, skin) in skins {
            self.skins
                .overwrite(i, DeferredSkin::new(&self.device, skin.clone()));
            self.skins[i].create_bind_group(&self.device, &self.skin_bind_group_layout);
        }
    }

    fn get_settings(&self) -> Vec<Setting> {
        vec![Setting::new(
            String::from("debug-view"),
            SettingValue::Int(0),
            Some(0..8),
        )]
    }

    fn set_setting(&mut self, setting: rfw_scene::renderers::Setting) {
        if setting.key() == "debug-view" {
            let debug_view = match setting.value() {
                SettingValue::Int(i) => output::DeferredView::from(*i),
                _ => output::DeferredView::Output,
            };

            self.debug_view = debug_view;
        }
    }
}

impl Deferred {
    fn record_update(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &mut InstanceList,
        meshes: &TrackedStorage<DeferredMesh>,
        anim_meshes: &TrackedStorage<DeferredAnimMesh>,
        skins: &TrackedStorage<DeferredSkin>,
    ) {
        skins.iter_changed().for_each(|(_, s)| s.update(queue));

        instances.update(device, &meshes, &anim_meshes, &queue);

        meshes.iter_changed().for_each(|(_, m)| m.copy_data(queue));

        anim_meshes
            .iter_changed()
            .for_each(|(_, m)| m.copy_data(queue));
    }

    fn render_lights(
        device: &wgpu::Device,
        lights: &mut DeferredLights,
        instances: &InstanceList,
        meshes: &TrackedStorage<DeferredMesh>,
        anim_meshes: &TrackedStorage<DeferredAnimMesh>,
        skins: &TrackedStorage<DeferredSkin>,
    ) -> wgpu::CommandBuffer {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render-lights"),
        });
        lights.render(&mut encoder, instances, meshes, anim_meshes, skins);
        encoder.finish()
    }

    fn render_scene(
        device: &wgpu::Device,
        camera: &Camera,
        light_counts: [u32; 4],
        pipeline: &pipeline::RenderPipeline,
        instances: &InstanceList,
        meshes: &TrackedStorage<DeferredMesh>,
        anim_meshes: &TrackedStorage<DeferredAnimMesh>,
        skins: &TrackedStorage<DeferredSkin>,
        d_output: &output::DeferredOutput,
        uniform_camera_buffer: &wgpu::Buffer,
        uniform_bind_group: &wgpu::BindGroup,
        material_bind_groups: &[wgpu::BindGroup],
        ssao_pass: &pass::SSAOPass,
        radiance_pass: &pass::RadiancePass,
    ) -> wgpu::CommandBuffer {
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

        let mut rasterize_pass = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render"),
        });

        rasterize_pass.copy_buffer_to_buffer(
            &camera_staging_buffer,
            0,
            uniform_camera_buffer,
            0,
            Self::UNIFORM_CAMERA_SIZE,
        );

        use output::*;

        {
            let mut render_pass = rasterize_pass.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[
                    d_output.as_descriptor(DeferredView::Albedo),
                    d_output.as_descriptor(DeferredView::Normal),
                    d_output.as_descriptor(DeferredView::WorldPos),
                    d_output.as_descriptor(DeferredView::ScreenSpace),
                    d_output.as_descriptor(DeferredView::MatParams),
                ],
                depth_stencil_attachment: Some(d_output.as_depth_descriptor()),
            });

            let matrix = camera.get_rh_matrix();
            let frustrum = rfw_scene::FrustrumG::from_matrix(matrix);

            let device_instance = &instances.device_instances;

            instances
                .iter()
                .filter(|(_, _, bounds)| {
                    frustrum
                        .aabb_in_frustrum(&bounds.root_bounds)
                        .should_render()
                })
                .for_each(|(i, instance, bounds)| match instance.object_id {
                    ObjectRef::None => {}
                    ObjectRef::Static(mesh_id) => {
                        let mesh = &meshes[mesh_id as usize];
                        if let Some(buffer) = mesh.buffer.as_ref() {
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

                            mesh.sub_meshes
                                .iter()
                                .enumerate()
                                .filter(|(j, _)| {
                                    frustrum
                                        .aabb_in_frustrum(&bounds.mesh_bounds[*j])
                                        .should_render()
                                })
                                .for_each(|(_, sub_mesh)| {
                                    let bind_group =
                                        &material_bind_groups[sub_mesh.mat_id as usize];
                                    render_pass.set_bind_group(2, bind_group, &[]);
                                    render_pass.draw(sub_mesh.first..sub_mesh.last, 0..1);
                                });
                        }
                    }
                    ObjectRef::Animated(mesh_id) => {
                        let mesh = &anim_meshes[mesh_id as usize];
                        if let Some(buffer) = mesh.buffer.as_ref() {
                            if let Some(skin_id) = instance.skin_id {
                                render_pass.set_pipeline(&pipeline.anim_pipeline);
                                render_pass.set_bind_group(0, &uniform_bind_group, &[]);
                                render_pass.set_bind_group(
                                    1,
                                    &device_instance.bind_group,
                                    &[DeviceInstances::dynamic_offset_for(i) as u32],
                                );

                                let buffer_slice = buffer.slice(mesh.buffer_start..mesh.buffer_end);
                                let anim_buffer_slice =
                                    buffer.slice(mesh.anim_start..mesh.anim_end);
                                render_pass.set_vertex_buffer(0, buffer_slice);
                                render_pass.set_vertex_buffer(1, buffer_slice);
                                render_pass.set_vertex_buffer(2, buffer_slice);
                                render_pass.set_vertex_buffer(3, buffer_slice);
                                render_pass.set_vertex_buffer(4, buffer_slice);
                                render_pass.set_vertex_buffer(5, anim_buffer_slice);
                                render_pass.set_vertex_buffer(6, anim_buffer_slice);

                                mesh.sub_meshes
                                    .iter()
                                    .enumerate()
                                    .filter(|(j, _)| {
                                        frustrum
                                            .aabb_in_frustrum(&bounds.mesh_bounds[*j])
                                            .should_render()
                                    })
                                    .for_each(|(_, sub_mesh)| {
                                        let bind_group =
                                            &material_bind_groups[sub_mesh.mat_id as usize];
                                        render_pass.set_bind_group(2, bind_group, &[]);
                                        render_pass.set_bind_group(
                                            3,
                                            match skins[skin_id as usize].bind_group.as_ref() {
                                                None => panic!(
                                                    "Skin {} does not have a bind group (yet)",
                                                    skin_id
                                                ),
                                                Some(b) => b,
                                            },
                                            &[],
                                        );
                                        render_pass.draw(sub_mesh.first..sub_mesh.last, 0..1);
                                    });
                            } else {
                                render_pass.set_pipeline(&pipeline.pipeline);
                                render_pass.set_bind_group(0, &uniform_bind_group, &[]);
                                render_pass.set_bind_group(
                                    1,
                                    &device_instance.bind_group,
                                    &[DeviceInstances::dynamic_offset_for(i) as u32],
                                );

                                let buffer_slice = buffer.slice(mesh.buffer_start..mesh.buffer_end);
                                render_pass.set_vertex_buffer(0, buffer_slice);
                                render_pass.set_vertex_buffer(1, buffer_slice);
                                render_pass.set_vertex_buffer(2, buffer_slice);
                                render_pass.set_vertex_buffer(3, buffer_slice);
                                render_pass.set_vertex_buffer(4, buffer_slice);

                                mesh.sub_meshes
                                    .iter()
                                    .enumerate()
                                    .filter(|(j, _)| {
                                        frustrum
                                            .aabb_in_frustrum(&bounds.mesh_bounds[*j])
                                            .should_render()
                                    })
                                    .for_each(|(_, sub_mesh)| {
                                        let bind_group =
                                            &material_bind_groups[sub_mesh.mat_id as usize];
                                        render_pass.set_bind_group(2, bind_group, &[]);
                                        render_pass.draw(sub_mesh.first..sub_mesh.last, 0..1);
                                    });
                            }
                        }
                    }
                });
        }

        ssao_pass.launch(
            &mut rasterize_pass,
            d_output.width,
            d_output.height,
            &uniform_bind_group,
        );

        radiance_pass.launch(&mut rasterize_pass, d_output.width, d_output.height);

        rasterize_pass.finish()
    }
}
