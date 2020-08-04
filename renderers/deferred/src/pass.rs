use super::{
    light::{DeferredLights, ShadowMapArray},
    output::{DeferredOutput, DeferredView},
};
use shared::*;

pub struct BlitPass {
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline_layout: wgpu::PipelineLayout,
    pipeline: wgpu::RenderPipeline,
}

impl BlitPass {
    pub fn new(device: &wgpu::Device, output: &DeferredOutput) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit-bind-group-layout"),
            bindings: &[
                output.as_storage_entry(0, wgpu::ShaderStage::FRAGMENT, DeferredView::Albedo, true),
                output.as_storage_entry(
                    1,
                    wgpu::ShaderStage::FRAGMENT,
                    DeferredView::Radiance,
                    true,
                ),
                output.as_storage_entry(2, wgpu::ShaderStage::FRAGMENT, DeferredView::SSAO, true),
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit-bind-group"),
            layout: &bind_group_layout,
            bindings: &[
                output.as_binding(0, DeferredView::Albedo),
                output.as_binding(1, DeferredView::Radiance),
                output.as_binding(2, DeferredView::SSAO),
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&bind_group_layout],
        });
        let vert_shader = include_bytes!("../shaders/quad.vert.spv");
        let frag_shader = include_bytes!("../shaders/deferred_blit.frag.spv");

        let vert_module = device.create_shader_module(vert_shader.to_quad_bytes());
        let frag_module = device.create_shader_module(frag_shader.to_quad_bytes());

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            /// The layout of bind groups for this pipeline.
            layout: &pipeline_layout,
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &vert_module,
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &frag_module,
            }),
            rasterization_state: None,
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: DeferredOutput::OUTPUT_FORMAT,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: None,
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint32,
                vertex_buffers: &[],
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        Self {
            bind_group_layout,
            bind_group,
            pipeline_layout,
            pipeline,
        }
    }

    pub fn update_bind_groups(&mut self, device: &wgpu::Device, output: &DeferredOutput) {
        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit-bind-group"),
            layout: &self.bind_group_layout,
            bindings: &[
                output.as_binding(0, DeferredView::Albedo),
                output.as_binding(1, DeferredView::Radiance),
                output.as_binding(2, DeferredView::SSAO),
            ],
        });
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, output: &wgpu::TextureView) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: output,
                clear_color: wgpu::Color::BLACK,
                load_op: wgpu::LoadOp::Clear,
                store_op: wgpu::StoreOp::Store,
                resolve_target: None,
            }],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}

pub struct SSAOPass {
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline_layout: wgpu::PipelineLayout,
    pipeline: wgpu::ComputePipeline,

    filter_uniform_direction_buffer: wgpu::Buffer,
    filter_direction_x: wgpu::Buffer,
    filter_direction_y: wgpu::Buffer,
    filter_bind_group_layout: wgpu::BindGroupLayout,
    filter_bind_group1: wgpu::BindGroup,
    filter_bind_group2: wgpu::BindGroup,
    filter_pipeline_layout: wgpu::PipelineLayout,
    filter_pipeline: wgpu::ComputePipeline,
}

impl SSAOPass {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        uniform_bind_group_layout: &wgpu::BindGroupLayout,
        output: &DeferredOutput,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            compare: wgpu::CompareFunction::Never,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ssao-bind-group-layout"),
            bindings: &[
                output.as_storage_entry(0, wgpu::ShaderStage::COMPUTE, DeferredView::SSAO, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Sampler { comparison: false },
                },
                output.as_sampled_entry(2, wgpu::ShaderStage::COMPUTE, DeferredView::ScreenSpace),
                output.as_sampled_entry(3, wgpu::ShaderStage::COMPUTE, DeferredView::Normal),
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ssao-staging-command"),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ssao-bind-group"),
            layout: &bind_group_layout,
            bindings: &[
                output.as_binding(0, DeferredView::SSAO),
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                output.as_binding(2, DeferredView::ScreenSpace),
                output.as_binding(3, DeferredView::Normal),
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[uniform_bind_group_layout, &bind_group_layout],
        });

        let shader = include_bytes!("../shaders/ssao.comp.spv");
        let shader_module = device.create_shader_module(shader.to_quad_bytes());
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            layout: &pipeline_layout,
            compute_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &shader_module,
            },
        });

        let filter_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("filter-bind-group-layout"),
                bindings: &[
                    output.as_storage_entry(
                        0,
                        wgpu::ShaderStage::COMPUTE,
                        DeferredView::SSAO,
                        false,
                    ),
                    output.as_storage_entry(
                        1,
                        wgpu::ShaderStage::COMPUTE,
                        DeferredView::SSAO,
                        true,
                    ),
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                    },
                ],
            });
        let filter_uniform_weight_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("filter-uniform-weight-buffer"),
            usage: wgpu::BufferUsage::COPY_DST | wgpu::BufferUsage::UNIFORM,
            size: (std::mem::size_of::<u32>() * 2 + std::mem::size_of::<f32>() * 128)
                as wgpu::BufferAddress,
        });
        let weights = Self::calc_blur_data(32, 5.0);
        assert_eq!(weights.len(), 128);
        let staging_buffer = device.create_buffer_mapped(&wgpu::BufferDescriptor {
            label: Some("staging-uniform-weight-buffer"),
            usage: wgpu::BufferUsage::COPY_SRC,
            size: (std::mem::size_of::<u32>() * 2 + std::mem::size_of::<f32>() * 128)
                as wgpu::BufferAddress,
        });

        unsafe {
            let width: [u32; 1] = [32];
            let width2: [u32; 1] = [64];
            let ptr = staging_buffer.data.as_mut_ptr();
            ptr.copy_from(width.as_ptr() as *const u8, 4);
            ptr.add(4).copy_from(width2.as_ptr() as *const u8, 4);
            ptr.add(8).copy_from(weights.as_ptr() as *const u8, 4 * 128);
        }
        let staging_buffer = staging_buffer.finish();

        encoder.copy_buffer_to_buffer(
            &staging_buffer,
            0,
            &filter_uniform_weight_buffer,
            0,
            (std::mem::size_of::<u32>() * 2 + std::mem::size_of::<f32>() * 128)
                as wgpu::BufferAddress,
        );

        let direction_x: [u32; 2] = [1, 0];
        let direction_y: [u32; 2] = [0, 1];
        let dir_x = unsafe { std::slice::from_raw_parts(direction_x.as_ptr() as *const u8, 8) };
        let dir_y = unsafe { std::slice::from_raw_parts(direction_y.as_ptr() as *const u8, 8) };
        let filter_direction_x = device.create_buffer_with_data(dir_x, wgpu::BufferUsage::COPY_SRC);
        let filter_direction_y = device.create_buffer_with_data(dir_y, wgpu::BufferUsage::COPY_SRC);

        let filter_uniform_direction_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("filter-uniform-direction-buffer"),
            size: 8,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        });

        let filter_bind_group1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("filter-bind-group"),
            layout: &filter_bind_group_layout,
            bindings: &[
                output.as_binding(0, DeferredView::FilteredSSAO),
                output.as_binding(1, DeferredView::SSAO),
                wgpu::Binding {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &filter_uniform_direction_buffer,
                        range: 0..8,
                    },
                },
            ],
        });

        let filter_bind_group2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("filter-bind-group"),
            layout: &filter_bind_group_layout,
            bindings: &[
                output.as_binding(0, DeferredView::SSAO),
                output.as_binding(1, DeferredView::FilteredSSAO),
                wgpu::Binding {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &filter_uniform_direction_buffer,
                        range: 0..8,
                    },
                },
            ],
        });

        let filter_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[&filter_bind_group_layout],
            });
        let shader = include_bytes!("../shaders/ssao_filter.comp.spv");
        let shader_module = device.create_shader_module(shader.to_quad_bytes());
        let filter_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            layout: &filter_pipeline_layout,
            compute_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &shader_module,
            },
        });

        queue.submit(&[encoder.finish()]);

        Self {
            sampler,
            bind_group_layout,
            bind_group,
            pipeline_layout,
            pipeline,
            filter_uniform_direction_buffer,
            filter_direction_x,
            filter_direction_y,
            filter_bind_group_layout,
            filter_bind_group1,
            filter_bind_group2,
            filter_pipeline_layout,
            filter_pipeline,
        }
    }

    fn normal_dist(value: f32, mean: f32, deviation: f32) -> f32 {
        let value = value - mean;
        let value_sq = value * value;
        let variance = deviation * deviation;
        (-value_sq / (2.0 * variance)).exp() / ((2.0 * std::f32::consts::PI).sqrt() * deviation)
    }

    pub fn calc_blur_data(width: usize, deviation: f32) -> Vec<f32> {
        let mut weights = vec![0.0 as f32; width * 4];
        let mut total = 0.0;
        let width2 = width * 2;

        assert!(width >= 1);
        assert!(width <= 32);

        // Calculate normal distribution
        for i in 0..width {
            let current = Self::normal_dist((width - i) as f32, 0.0, deviation);
            weights[width2 - i] = current;
            weights[i] = weights[width2 - i];
            total += 2.0 * current;
        }
        weights[width] = Self::normal_dist(0.0, 0.0, deviation);
        total += weights[width];

        // Normalize values such that together they sum to 1
        for i in 0..width2 {
            weights[i] /= total;
        }

        weights
    }

    pub fn update_bind_groups(&mut self, device: &wgpu::Device, output: &DeferredOutput) {
        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ssao-bind-group"),
            layout: &self.bind_group_layout,
            bindings: &[
                output.as_binding(0, DeferredView::SSAO),
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                output.as_binding(2, DeferredView::ScreenSpace),
                output.as_binding(3, DeferredView::Normal),
            ],
        });

        self.filter_bind_group1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("filter-bind-group"),
            layout: &self.filter_bind_group_layout,
            bindings: &[
                output.as_binding(0, DeferredView::FilteredSSAO),
                output.as_binding(1, DeferredView::SSAO),
                wgpu::Binding {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &self.filter_uniform_direction_buffer,
                        range: 0..8,
                    },
                },
            ],
        });

        self.filter_bind_group2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("filter-bind-group"),
            layout: &self.filter_bind_group_layout,
            bindings: &[
                output.as_binding(0, DeferredView::SSAO),
                output.as_binding(1, DeferredView::FilteredSSAO),
                wgpu::Binding {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &self.filter_uniform_direction_buffer,
                        range: 0..8,
                    },
                },
            ],
        });
    }

    pub fn launch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        width: usize,
        height: usize,
        uniform_bind_group: &wgpu::BindGroup,
    ) {
        encoder.copy_buffer_to_buffer(
            &self.filter_direction_x,
            0,
            &self.filter_uniform_direction_buffer,
            0,
            8,
        );

        {
            let mut ssao_pass = encoder.begin_compute_pass();
            ssao_pass.set_pipeline(&self.pipeline);
            ssao_pass.set_bind_group(0, uniform_bind_group, &[]);
            ssao_pass.set_bind_group(1, &self.bind_group, &[]);
            ssao_pass.dispatch(((width * height) as f32 / 64.0).ceil() as u32, 1, 1);
        }

        {
            let mut ssao_pass = encoder.begin_compute_pass();
            ssao_pass.set_pipeline(&self.filter_pipeline);
            ssao_pass.set_bind_group(0, &self.filter_bind_group1, &[]);
            ssao_pass.dispatch(
                (width as f32 / 8.0).ceil() as u32,
                (height as f32 / 8.0).ceil() as u32,
                1,
            );
        }

        encoder.copy_buffer_to_buffer(
            &self.filter_direction_y,
            0,
            &self.filter_uniform_direction_buffer,
            0,
            8,
        );

        {
            let mut ssao_pass = encoder.begin_compute_pass();
            ssao_pass.set_pipeline(&self.filter_pipeline);
            ssao_pass.set_bind_group(0, &self.filter_bind_group2, &[]);
            ssao_pass.dispatch(
                (width as f32 / 8.0).ceil() as u32,
                (height as f32 / 8.0).ceil() as u32,
                1,
            );
        }
    }
}

pub struct RadiancePass {
    pipeline_layout: wgpu::PipelineLayout,
    pipeline: wgpu::ComputePipeline,
    deferred_sampler: wgpu::Sampler,
    shadow_sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    lights_bind_group_layout: wgpu::BindGroupLayout,
    lights_bind_group: wgpu::BindGroup,
}

impl RadiancePass {
    pub fn new(
        device: &wgpu::Device,
        uniform_bind_group_layout: &wgpu::BindGroupLayout,
        output: &DeferredOutput,
        lights: &DeferredLights,
    ) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("radiance-bind-group-layout"),
            bindings: &[
                output.as_storage_entry(
                    0,
                    wgpu::ShaderStage::COMPUTE,
                    DeferredView::Radiance,
                    false,
                ),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Sampler { comparison: false },
                },
                output.as_storage_entry(2, wgpu::ShaderStage::COMPUTE, DeferredView::Albedo, true),
                output.as_storage_entry(3, wgpu::ShaderStage::COMPUTE, DeferredView::Normal, true),
                output.as_storage_entry(
                    4,
                    wgpu::ShaderStage::COMPUTE,
                    DeferredView::WorldPos,
                    true,
                ),
            ],
        });

        let lights_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("lights-bind-group-layout"),
                bindings: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::Sampler { comparison: false },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::SampledTexture {
                            component_type: wgpu::TextureComponentType::Float,
                            dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::SampledTexture {
                            component_type: wgpu::TextureComponentType::Float,
                            dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 8,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::SampledTexture {
                            component_type: wgpu::TextureComponentType::Float,
                            dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 10,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 11,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 12,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                    },
                ],
            });
        let shadow_sampler = ShadowMapArray::create_sampler(device);

        let deferred_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            compare: wgpu::CompareFunction::Never,
            lod_max_clamp: 0.0,
            lod_min_clamp: 0.0,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            label: Some("output-bind-group"),
            bindings: &[
                output.as_binding(0, DeferredView::Radiance),
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&deferred_sampler),
                },
                output.as_binding(2, DeferredView::Albedo),
                output.as_binding(3, DeferredView::Normal),
                output.as_binding(4, DeferredView::WorldPos),
            ],
        });

        let lights_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lights-bind-group"),
            layout: &lights_bind_group_layout,
            bindings: &[
                lights.area_lights.uniform_binding(1),
                lights.spot_lights.uniform_binding(2),
                lights.directional_lights.uniform_binding(3),
                wgpu::Binding {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
                // lights.point_lights.shadow_map_binding(5),
                lights.area_lights.shadow_map_binding(6),
                lights.spot_lights.shadow_map_binding(7),
                lights.directional_lights.shadow_map_binding(8),
                // lights.point_lights.infos_binding(9),
                lights.area_lights.infos_binding(10),
                lights.spot_lights.infos_binding(11),
                lights.directional_lights.infos_binding(12),
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[
                uniform_bind_group_layout,
                &bind_group_layout,
                &lights_bind_group_layout,
            ],
        });

        let spirv = include_bytes!("../shaders/lighting.comp.spv");
        let module = device.create_shader_module(spirv.to_quad_bytes());

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            layout: &pipeline_layout,
            compute_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &module,
            },
        });

        Self {
            pipeline_layout,
            pipeline,
            deferred_sampler,
            shadow_sampler,
            bind_group_layout,
            bind_group,
            lights_bind_group_layout,
            lights_bind_group,
        }
    }

    pub fn update_bind_groups(
        &mut self,
        device: &wgpu::Device,
        output: &DeferredOutput,
        lights: &DeferredLights,
    ) {
        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.bind_group_layout,
            label: Some("output-bind-group"),
            bindings: &[
                output.as_binding(0, DeferredView::Radiance),
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.deferred_sampler),
                },
                output.as_binding(2, DeferredView::Albedo),
                output.as_binding(3, DeferredView::Normal),
                output.as_binding(4, DeferredView::WorldPos),
            ],
        });

        self.lights_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lights-bind-group"),
            layout: &self.lights_bind_group_layout,
            bindings: &[
                lights.area_lights.uniform_binding(1),
                lights.spot_lights.uniform_binding(2),
                lights.directional_lights.uniform_binding(3),
                wgpu::Binding {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.shadow_sampler),
                },
                // lights.point_lights.shadow_map_binding(5),
                lights.area_lights.shadow_map_binding(6),
                lights.spot_lights.shadow_map_binding(7),
                lights.directional_lights.shadow_map_binding(8),
                // lights.point_lights.infos_binding(9),
                lights.area_lights.infos_binding(10),
                lights.spot_lights.infos_binding(11),
                lights.directional_lights.infos_binding(12),
            ],
        });
    }

    pub fn launch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        width: usize,
        height: usize,
        uniform_bind_group: &wgpu::BindGroup,
    ) {
        let mut compute_pass = encoder.begin_compute_pass();
        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, uniform_bind_group, &[]);
        compute_pass.set_bind_group(1, &self.bind_group, &[]);
        compute_pass.set_bind_group(2, &self.lights_bind_group, &[]);
        compute_pass.dispatch(
            (width as f32 / 8.0).ceil() as u32,
            (height as f32 / 8.0).ceil() as u32,
            1,
        );
    }
}
