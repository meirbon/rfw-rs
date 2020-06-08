use futures::executor::block_on;
use glam::*;
use rtbvh::AABB;
use scene::renderers::{RenderMode, Renderer, Setting, SettingValue};
use scene::{BitVec, DeviceMaterial, HasRawWindowHandle, Instance};
use shared::*;
use std::error::Error;
use std::fmt::{Display, Formatter};

mod instance;
mod light;
mod mesh;
mod output;
mod pass;
mod pipeline;

pub struct CopyCommand<'a> {
    destination_buffer: &'a wgpu::Buffer,
    copy_size: wgpu::BufferAddress,
    staging_buffer: wgpu::Buffer,
}

impl<'a> CopyCommand<'a> {
    pub fn record(&self, encoder: &mut wgpu::CommandEncoder) {
        assert!(self.copy_size > 0);
        encoder.copy_buffer_to_buffer(
            &self.staging_buffer,
            0,
            self.destination_buffer,
            0,
            self.copy_size,
        )
    }
}

pub struct Deferred<'a> {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter: wgpu::Adapter,
    surface: wgpu::Surface,
    swap_chain: wgpu::SwapChain,
    meshes: Vec<mesh::DeferredMesh>,
    mesh_changed: BitVec,
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
    camera_staging_buffer: wgpu::Buffer,
    output: output::DeferredOutput,
    compiler: Compiler<'a>,
    pipeline: pipeline::RenderPipeline,
    scene_bounds: AABB,

    ssao_pass: pass::SSAOPass,
    radiance_pass: pass::RadiancePass,
    blit_pass: pass::BlitPass,

    debug_view: output::DeferredView,
    debug_enabled: bool,
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

impl<'a> Deferred<'a> {
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

impl Renderer for Deferred<'_> {
    fn init<T: HasRawWindowHandle>(
        window: &T,
        width: usize,
        height: usize,
    ) -> Result<Box<Self>, Box<dyn Error>> {
        let surface = wgpu::Surface::create(window);
        let adapter = block_on(wgpu::Adapter::request(
            &wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::HighPerformance,
            },
            wgpu::BackendBit::PRIMARY,
        ))
        .unwrap();

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
                present_mode: wgpu::PresentMode::Mailbox,
            },
        );

        let instances = instance::InstanceList::new(&device);
        let material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("material-buffer"),
            size: std::mem::size_of::<DeviceMaterial>() as wgpu::BufferAddress * 10,
            usage: wgpu::BufferUsage::STORAGE_READ | wgpu::BufferUsage::COPY_DST,
        });

        let texture_bind_group_layout = Self::create_texture_bind_group_layout(&device);

        let lights = light::DeferredLights::new(10, &device, &instances.bind_group_layout);

        let uniform_camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform-buffer"),
            size: Self::UNIFORM_CAMERA_SIZE,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        });

        let camera_staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform-staging-buffer"),
            size: Self::UNIFORM_CAMERA_SIZE,
            usage: wgpu::BufferUsage::MAP_WRITE | wgpu::BufferUsage::COPY_SRC,
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

        let mut compiler = CompilerBuilder::new().build().unwrap();
        let output = output::DeferredOutput::new(&device, width, height, &mut compiler);

        let pipeline = pipeline::RenderPipeline::new(
            &device,
            &uniform_bind_group_layout,
            &instances.bind_group_layout,
            &texture_bind_group_layout,
            &mut compiler,
        );

        let material_buffer_size =
            std::mem::size_of::<DeviceMaterial>() as wgpu::BufferAddress * 10;

        let ssao_pass = pass::SSAOPass::new(
            &device,
            &queue,
            &mut compiler,
            &uniform_bind_group_layout,
            &output,
        );
        let radiance_pass = pass::RadiancePass::new(
            &device,
            &mut compiler,
            &uniform_bind_group_layout,
            &output,
            &lights,
        );
        let blit_pass = pass::BlitPass::new(&device, &mut compiler, &output);

        Ok(Box::new(Self {
            device,
            queue,
            adapter,
            surface,
            swap_chain,
            meshes: Vec::new(),
            mesh_changed: BitVec::new(),
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
            camera_staging_buffer,
            output,
            compiler,
            pipeline,
            ssao_pass,
            radiance_pass,
            blit_pass,
            scene_bounds: AABB::new(),

            debug_view: output::DeferredView::Output,
            debug_enabled: false,
            lights_changed: true,
            materials_changed: true,
        }))
    }

    fn set_mesh(&mut self, id: usize, mesh: &scene::Mesh) {
        if id >= self.meshes.len() {
            self.meshes
                .push(mesh::DeferredMesh::new(&self.device, mesh));
            self.mesh_changed.push(true);
        } else {
            self.meshes[id] = mesh::DeferredMesh::new(&self.device, mesh);
            self.mesh_changed.set(id, true);
        }
    }

    fn set_instance(&mut self, id: usize, instance: &Instance) {
        self.instances.set(
            &self.device,
            id,
            instance.clone(),
            &self.meshes[instance.get_hit_id()],
        );

        self.scene_bounds.grow_bb(
            &instance
                .local_bounds()
                .transformed(instance.get_transform()),
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
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("synchronize-command"),
            });

        let mut commands = Vec::with_capacity(self.meshes.len() + self.instances.len());
        for i in 0..self.meshes.len() {
            if !self.mesh_changed.get(i).unwrap() {
                continue;
            }
            commands.push(self.meshes[i].get_copy_command(&self.device));
        }

        for command in self.instances.update(&self.device, self.meshes.as_slice()) {
            commands.push(command);
        }

        for command in commands.iter() {
            command.record(&mut encoder);
        }

        self.queue.submit(&[encoder.finish()]);
        self.lights_changed |= self.lights.synchronize(&self.device, &self.queue);
        if self.lights_changed {
            self.radiance_pass
                .update_bind_groups(&self.device, &self.output, &self.lights);
        }

        self.mesh_changed.set_all(false);
    }

    fn render(&mut self, camera: &scene::Camera, _mode: RenderMode) {
        let mapping = self
            .camera_staging_buffer
            .map_write(0, Self::UNIFORM_CAMERA_SIZE);

        let mut rasterize_pass =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render"),
                });

        rasterize_pass.copy_buffer_to_buffer(
            &self.camera_staging_buffer,
            0,
            &self.uniform_camera_buffer,
            0,
            Self::UNIFORM_CAMERA_SIZE,
        );

        let output = self.swap_chain.get_next_texture();
        if output.is_err() {
            return;
        }
        let output = output.unwrap();
        use output::*;

        if self.lights_changed || self.instances.changed() {
            self.lights
                .render(&mut rasterize_pass, &self.instances, self.meshes.as_slice());
        }

        {
            let mut render_pass = rasterize_pass.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[
                    self.output.as_descriptor(DeferredView::Albedo),
                    self.output.as_descriptor(DeferredView::Normal),
                    self.output.as_descriptor(DeferredView::WorldPos),
                    self.output.as_descriptor(DeferredView::ScreenSpace),
                ],
                depth_stencil_attachment: Some(self.output.as_depth_descriptor()),
            });

            let matrix = camera.get_rh_matrix();
            let frustrum = scene::FrustrumG::from_matrix(matrix);

            render_pass.set_pipeline(&self.pipeline.pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            for i in 0..self.instances.len() {
                let instance = &self.instances.instances[i];
                let device_instance = &self.instances.device_instances[i];
                let bounds = &self.instances.bounds[i];

                if !frustrum
                    .aabb_in_frustrum(&bounds.root_bounds)
                    .should_render()
                {
                    continue;
                }

                let mesh: &mesh::DeferredMesh = &self.meshes[instance.get_hit_id()];
                render_pass.set_vertex_buffer(0, &mesh.buffer, 0, mesh.buffer_size);
                render_pass.set_vertex_buffer(1, &mesh.buffer, 0, mesh.buffer_size);
                render_pass.set_vertex_buffer(2, &mesh.buffer, 0, mesh.buffer_size);
                render_pass.set_vertex_buffer(3, &mesh.buffer, 0, mesh.buffer_size);
                render_pass.set_vertex_buffer(4, &mesh.buffer, 0, mesh.buffer_size);

                render_pass.set_bind_group(1, &device_instance.bind_group, &[]);

                for j in 0..mesh.sub_meshes.len() {
                    if !frustrum
                        .aabb_in_frustrum(&bounds.mesh_bounds[j])
                        .should_render()
                    {
                        continue;
                    }

                    let sub_mesh = &mesh.sub_meshes[j];
                    let bind_group = &self.material_bind_groups[sub_mesh.mat_id as usize];
                    render_pass.set_bind_group(2, bind_group, &[]);
                    render_pass.draw(sub_mesh.first..sub_mesh.last, 0..1);
                }
            }
        }

        self.ssao_pass.launch(
            &mut rasterize_pass,
            self.output.width,
            self.output.height,
            &self.uniform_bind_group,
        );

        self.radiance_pass.launch(
            &mut rasterize_pass,
            self.output.width,
            self.output.height,
            &self.uniform_bind_group,
        );
        self.device.poll(wgpu::Maintain::Wait);
        if let Ok(mut mapping) = block_on(mapping) {
            let slice = mapping.as_slice();
            let view = camera.get_view_matrix();
            let projection = camera.get_projection();
            let light_counts: [u32; 4] = self.lights.counts();

            unsafe {
                let ptr = slice.as_mut_ptr();
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
        }
        self.queue.submit(&[rasterize_pass.finish()]);

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

        self.queue.submit(&[output_pass.finish()]);

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
