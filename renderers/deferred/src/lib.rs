use crate::instance::{DeviceInstances, InstanceList};
use crate::light::DeferredLights;
use crate::mesh::DeferredAnimMesh;
use crate::skin::DeferredSkin;
use futures::executor::block_on;
use glam::*;
use mesh::DeferredMesh;
use rtbvh::AABB;
use scene::graph::Skin;
use scene::renderers::{RenderMode, Renderer, Setting, SettingValue};
use scene::{
    raw_window_handle::HasRawWindowHandle, AnimatedMesh, BitVec, Camera, DeviceMaterial, Instance,
    ObjectRef, Texture, TrackedStorage,
};
use std::error::Error;
use std::fmt::{Display, Formatter};

mod instance;
mod light;
mod mesh;
mod output;
mod pass;
mod pipeline;
mod skin;

pub struct Deferred {
    device: wgpu::Device,
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
            bindings: &[
                wgpu::BindGroupLayoutEntry {
                    // Albedo texture
                    binding: 0,
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
            bindings: &[
                wgpu::BindGroupLayoutEntry {
                    // Matrix buffer
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX
                        | wgpu::ShaderStage::FRAGMENT
                        | wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                },
                wgpu::BindGroupLayoutEntry {
                    // Material buffer
                    binding: 1,
                    visibility: wgpu::ShaderStage::FRAGMENT | wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::StorageBuffer {
                        readonly: true,
                        dynamic: false,
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    // Texture sampler
                    binding: 2,
                    visibility: wgpu::ShaderStage::FRAGMENT | wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Sampler { comparison: false },
                },
            ],
            label: Some("uniform-layout"),
        })
    }
}

impl Renderer for Deferred {
    fn init<T: HasRawWindowHandle>(
        window: &T,
        width: usize,
        height: usize,
    ) -> Result<Box<Self>, Box<dyn Error>> {
        let surface = wgpu::Surface::create(window);
        let adapter = match block_on(wgpu::Adapter::request(
            &wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::HighPerformance,
            },
            wgpu::BackendBit::PRIMARY,
        )) {
            None => return Err(Box::new(DeferredError::RequestDeviceError)),
            Some(adapter) => adapter,
        };

        println!("Picked device: {}", adapter.get_info().name);

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            extensions: wgpu::Extensions {
                anisotropic_filtering: true,
            },
            limits: wgpu::Limits::default(),
        }));

        let swap_chain = device.create_swap_chain(
            &surface,
            &wgpu::SwapChainDescriptor {
                width: width as u32,
                height: height as u32,
                usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
                format: output::DeferredOutput::OUTPUT_FORMAT,
                present_mode: wgpu::PresentMode::Immediate,
            },
        );

        let instances = instance::InstanceList::new(&device);
        let material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("material-buffer"),
            size: std::mem::size_of::<DeviceMaterial>() as wgpu::BufferAddress * 10,
            usage: wgpu::BufferUsage::STORAGE_READ | wgpu::BufferUsage::COPY_DST,
        });

        let texture_bind_group_layout = Self::create_texture_bind_group_layout(&device);

        let skin_bind_group_layout = DeferredSkin::create_bind_group_layout(&device);

        let lights = light::DeferredLights::new(
            10,
            &device,
            &queue,
            &instances.bind_group_layout,
            &skin_bind_group_layout,
        );

        let uniform_camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform-buffer"),
            size: Self::UNIFORM_CAMERA_SIZE,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        });

        let texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 5.0,
            compare: wgpu::CompareFunction::Never,
        });

        let uniform_bind_group_layout = Self::create_uniform_bind_group_layout(&device);
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform-bind-group"),
            layout: &uniform_bind_group_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &uniform_camera_buffer,
                        range: 0..Self::UNIFORM_CAMERA_SIZE,
                    },
                },
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &material_buffer,
                        range: 0..std::mem::size_of::<DeviceMaterial>() as wgpu::BufferAddress * 10,
                    },
                },
                wgpu::Binding {
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

        let ssao_pass = pass::SSAOPass::new(&device, &queue, &uniform_bind_group_layout, &output);
        let radiance_pass =
            pass::RadiancePass::new(&device, &uniform_bind_group_layout, &output, &lights);
        let blit_pass = pass::BlitPass::new(&device, &output);

        Ok(Box::new(Self {
            device,
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
        }))
    }

    fn set_mesh(&mut self, id: usize, mesh: &scene::Mesh) {
        self.meshes
            .overwrite(id, mesh::DeferredMesh::new(&self.device, mesh));
    }

    fn set_animated_mesh(&mut self, id: usize, mesh: &AnimatedMesh) {
        self.anim_meshes
            .overwrite(id, mesh::DeferredAnimMesh::new(&self.device, mesh));
    }

    fn set_instance(&mut self, id: usize, instance: &Instance) {
        match instance.object_id {
            ObjectRef::None => {
                self.instances
                    .set(&self.device, id, instance.clone(), &DeferredMesh::default());
            }
            ObjectRef::Static(mesh_id) => {
                self.instances.set(
                    &self.device,
                    id,
                    instance.clone(),
                    &self.meshes[mesh_id as usize],
                );
            }
            ObjectRef::Animated(mesh_id) => {
                self.instances.set_animated(
                    &self.device,
                    id,
                    instance.clone(),
                    &self.anim_meshes[mesh_id as usize],
                );
            }
        }

        self.scene_bounds.grow_bb(
            &instance
                .local_bounds()
                .transformed(instance.get_transform().to_cols_array()),
        );
    }

    fn set_materials(
        &mut self,
        materials: &[scene::Material],
        device_materials: &[scene::DeviceMaterial],
    ) {
        assert!(materials.len() > 0);
        assert_eq!(materials.len(), device_materials.len());
        let size =
            (device_materials.len() * std::mem::size_of::<DeviceMaterial>()) as wgpu::BufferAddress;
        let staging_buffer = self.device.create_buffer_with_data(
            unsafe {
                std::slice::from_raw_parts(device_materials.as_ptr() as *const u8, size as usize)
            },
            wgpu::BufferUsage::COPY_SRC,
        );

        if size > self.material_buffer_size {
            self.material_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("material-buffer"),
                size,
                usage: wgpu::BufferUsage::COPY_DST | wgpu::BufferUsage::STORAGE_READ,
            });
            self.material_buffer_size = size;
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("create-material-cmd-buffer"),
            });

        encoder.copy_buffer_to_buffer(&staging_buffer, 0, &self.material_buffer, 0, size);
        self.queue.submit(&[encoder.finish()]);

        self.material_bind_groups = (0..materials.len())
            .map(|i| {
                let material = &materials[i];
                let albedo_tex = material.diffuse_tex.max(0) as usize;
                let normal_tex = material.normal_tex.max(0) as usize;

                let albedo_view = &self.texture_views[albedo_tex];
                let normal_view = &self.texture_views[normal_tex];

                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    bindings: &[
                        wgpu::Binding {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(albedo_view),
                        },
                        wgpu::Binding {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(normal_view),
                        },
                    ],
                    layout: &self.texture_bind_group_layout,
                })
            })
            .collect();

        self.uniform_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform-bind-group"),
            layout: &self.uniform_bind_group_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &self.uniform_camera_buffer,
                        range: 0..Self::UNIFORM_CAMERA_SIZE,
                    },
                },
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &self.material_buffer,
                        range: 0..self.material_buffer_size,
                    },
                },
                wgpu::Binding {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.texture_sampler),
                },
            ],
        });

        self.materials_changed = true;
    }

    fn set_textures(&mut self, textures: &[scene::Texture]) {
        let staging_size =
            textures.iter().map(|t| t.data.len()).sum::<usize>() * std::mem::size_of::<u32>();
        let staging_buffer = self.device.create_buffer_mapped(&wgpu::BufferDescriptor {
            label: Some("material-staging-buffer"),
            size: staging_size as wgpu::BufferAddress,
            usage: wgpu::BufferUsage::MAP_WRITE | wgpu::BufferUsage::COPY_SRC,
        });

        let mut data_ptr = staging_buffer.data.as_mut_ptr() as *mut u32;
        for tex in textures.iter() {
            unsafe {
                std::ptr::copy(tex.data.as_ptr(), data_ptr, tex.data.len());
                data_ptr = data_ptr.add(tex.data.len());
            }
        }
        let staging_buffer = staging_buffer.finish();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("texture-staging-cmd-buffer"),
            });

        let mut offset = 0 as wgpu::BufferAddress;
        for (i, tex) in textures.iter().enumerate() {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(format!("texture-{}", i).as_str()),
                size: wgpu::Extent3d {
                    width: tex.width,
                    height: tex.height,
                    depth: 1,
                },
                array_layer_count: 1,
                mip_level_count: scene::Texture::MIP_LEVELS as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8Unorm,
                usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
            });

            let mut width = tex.width;
            let mut height = tex.height;
            let mut local_offset = 0 as wgpu::BufferAddress;
            for i in 0..scene::Texture::MIP_LEVELS {
                encoder.copy_buffer_to_texture(
                    wgpu::BufferCopyView {
                        buffer: &staging_buffer,
                        offset: offset
                            + local_offset * std::mem::size_of::<u32>() as wgpu::BufferAddress,
                        bytes_per_row: (width as usize * std::mem::size_of::<u32>()) as u32,
                        rows_per_image: tex.height,
                    },
                    wgpu::TextureCopyView {
                        origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                        array_layer: 0,
                        mip_level: i as u32,
                        texture: &texture,
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
            .map(|t| t.create_default_view())
            .collect();

        self.queue.submit(&[encoder.finish()]);
    }

    fn synchronize(&mut self) {
        let update = Self::record_update(
            &self.device,
            &self.queue,
            &mut self.instances,
            &self.meshes,
            &self.anim_meshes,
            &self.skins,
        );

        self.lights_changed |= self.lights.synchronize(&self.device, &self.queue);

        if self.lights_changed {
            self.radiance_pass
                .update_bind_groups(&self.device, &self.output, &self.lights);
        }

        block_on(update);

        self.skins.reset_changed();
        self.meshes.reset_changed();
        self.anim_meshes.reset_changed();
    }

    fn render(&mut self, camera: &scene::Camera, _mode: RenderMode) {
        let output = match self.swap_chain.get_next_texture() {
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

        let light_pass = futures::executor::block_on(light_pass);
        self.queue.submit(&[light_pass]);

        let mut output_pass = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("output-pass"),
            });

        if self.debug_view == output::DeferredView::Output {
            self.blit_pass.render(&mut output_pass, &output.view);
        } else {
            self.output
                .blit_debug(&output.view, &mut output_pass, self.debug_view);
        }

        let render_pass = futures::executor::block_on(render_pass);

        self.queue.submit(&[render_pass, output_pass.finish()]);

        self.instances.reset_changed();
        self.lights_changed = false;
    }

    fn resize<T: HasRawWindowHandle>(&mut self, _window: &T, width: usize, height: usize) {
        self.swap_chain = self.device.create_swap_chain(
            &self.surface,
            &wgpu::SwapChainDescriptor {
                width: width as u32,
                height: height as u32,
                present_mode: wgpu::PresentMode::Immediate,
                format: Self::OUTPUT_FORMAT,
                usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            },
        );

        self.output.resize(&self.device, width, height);
        self.radiance_pass
            .update_bind_groups(&self.device, &self.output, &self.lights);
        self.ssao_pass
            .update_bind_groups(&self.device, &self.output);
        self.blit_pass
            .update_bind_groups(&self.device, &self.output);
    }

    fn set_point_lights(&mut self, _changed: &BitVec, _lights: &[scene::PointLight]) {
        self.lights_changed = true;
    }

    fn set_spot_lights(&mut self, changed: &BitVec, lights: &[scene::SpotLight]) {
        self.lights
            .set_spot_lights(changed, lights, &self.scene_bounds);
        self.lights_changed = true;
    }

    fn set_area_lights(&mut self, changed: &BitVec, lights: &[scene::AreaLight]) {
        self.lights
            .set_area_lights(changed, lights, &self.scene_bounds);
        self.lights_changed = true;
    }

    fn set_directional_lights(&mut self, changed: &BitVec, lights: &[scene::DirectionalLight]) {
        self.lights
            .set_directional_lights(changed, lights, &self.scene_bounds);
        self.lights_changed = true;
    }

    fn set_skybox(&mut self, _skybox: Texture) {
        unimplemented!()
    }

    fn set_skin(&mut self, id: usize, skin: &Skin) {
        self.skins
            .overwrite(id, DeferredSkin::new(&self.device, skin.clone()));
        self.skins[id].create_bind_group(&self.device, &self.skin_bind_group_layout);
    }

    fn get_settings(&self) -> Vec<Setting> {
        vec![Setting::new(
            String::from("debug-view"),
            SettingValue::Int(0),
            Some(0..8),
        )]
    }

    fn set_setting(&mut self, setting: scene::renderers::Setting) {
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
    async fn record_update(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &mut InstanceList,
        meshes: &TrackedStorage<DeferredMesh>,
        anim_meshes: &TrackedStorage<DeferredAnimMesh>,
        skins: &TrackedStorage<DeferredSkin>,
    ) {
        let s = skins
            .iter_changed()
            .map(|(_, s)| s.update(device, queue))
            .collect::<Vec<_>>();

        let instances_update = instances.update(device, &meshes, &anim_meshes, &queue);

        let mesh_updates = meshes
            .iter_changed()
            .map(|(_, m)| m.copy_data(device, queue))
            .collect::<Vec<_>>();

        let anim_mesh_updates = anim_meshes
            .iter_changed()
            .map(|(_, m)| m.copy_data(device, queue))
            .collect::<Vec<_>>();

        for s in s.into_iter() {
            s.await
        }
        for m in mesh_updates.into_iter() {
            m.await
        }
        for m in anim_mesh_updates.into_iter() {
            m.await
        }

        instances_update.await;
    }

    async fn render_lights(
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
        lights
            .render(&mut encoder, instances, meshes, anim_meshes, skins)
            .await;
        encoder.finish()
    }

    async fn render_scene(
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
            let view = camera.get_view_matrix();
            let projection = camera.get_projection();

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

        let camera_staging_buffer =
            device.create_buffer_with_data(&camera_data, wgpu::BufferUsage::COPY_SRC);

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
                ],
                depth_stencil_attachment: Some(d_output.as_depth_descriptor()),
            });

            let matrix = camera.get_rh_matrix();
            let frustrum = scene::FrustrumG::from_matrix(matrix);

            let device_instance = &instances.device_instances;

            instances
                .iter()
                .filter(|(_, _, bounds)| {
                    frustrum
                        .aabb_in_frustrum(&bounds.root_bounds)
                        .should_render()
                })
                .for_each(|(i, instance, bounds)| match instance.object_id {
                    ObjectRef::None => panic!("Invalid"),
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

                            render_pass.set_vertex_buffer(0, buffer, 0, mesh.buffer_size);
                            render_pass.set_vertex_buffer(1, buffer, 0, mesh.buffer_size);
                            render_pass.set_vertex_buffer(2, buffer, 0, mesh.buffer_size);
                            render_pass.set_vertex_buffer(3, buffer, 0, mesh.buffer_size);
                            render_pass.set_vertex_buffer(4, buffer, 0, mesh.buffer_size);

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
                        if let (Some(buffer), Some(anim_buffer)) =
                        (mesh.buffer.as_ref(), mesh.anim_buffer.as_ref())
                        {
                            if let Some(skin_id) = instance.skin_id {
                                render_pass.set_pipeline(&pipeline.anim_pipeline);
                                render_pass.set_bind_group(0, &uniform_bind_group, &[]);
                                render_pass.set_bind_group(
                                    1,
                                    &device_instance.bind_group,
                                    &[DeviceInstances::dynamic_offset_for(i) as u32],
                                );

                                render_pass.set_vertex_buffer(0, buffer, 0, mesh.buffer_size);
                                render_pass.set_vertex_buffer(1, buffer, 0, mesh.buffer_size);
                                render_pass.set_vertex_buffer(2, buffer, 0, mesh.buffer_size);
                                render_pass.set_vertex_buffer(3, buffer, 0, mesh.buffer_size);
                                render_pass.set_vertex_buffer(4, buffer, 0, mesh.buffer_size);
                                render_pass.set_vertex_buffer(
                                    5,
                                    anim_buffer,
                                    0,
                                    mesh.anim_buffer_size,
                                );
                                render_pass.set_vertex_buffer(
                                    6,
                                    anim_buffer,
                                    0,
                                    mesh.anim_buffer_size,
                                );

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

                                render_pass.set_vertex_buffer(0, buffer, 0, mesh.buffer_size);
                                render_pass.set_vertex_buffer(1, buffer, 0, mesh.buffer_size);
                                render_pass.set_vertex_buffer(2, buffer, 0, mesh.buffer_size);
                                render_pass.set_vertex_buffer(3, buffer, 0, mesh.buffer_size);
                                render_pass.set_vertex_buffer(4, buffer, 0, mesh.buffer_size);

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

        radiance_pass.launch(
            &mut rasterize_pass,
            d_output.width,
            d_output.height,
            &uniform_bind_group,
        );

        rasterize_pass.finish()
    }
}
