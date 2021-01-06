use super::{
    light::{ShadowMapArray, WgpuLights},
    output::{WgpuOutput, WgpuView},
};
use rfw::prelude::*;
use std::borrow::Cow;
use std::num::NonZeroU64;
use wgpu::util::DeviceExt;

pub struct QuadPass {
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

impl QuadPass {
    pub fn new(device: &wgpu::Device, output: &WgpuOutput) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit-bind-group-layout"),
            entries: &[
                output.as_sampled_entry(0, wgpu::ShaderStage::FRAGMENT, WgpuView::Output),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Sampler { comparison: false },
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("quad-pass-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            compare: None,
            anisotropy_clamp: None,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("quad-bind-group"),
            layout: &bind_group_layout,
            entries: &[
                output.as_binding(0, WgpuView::Output),
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let vert_shader: &[u8] = include_bytes!("../shaders/quad.vert.spv");
        let frag_shader: &[u8] = include_bytes!("../shaders/quad.frag.spv");

        let vert_module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(Cow::from(
            vert_shader.as_quad_bytes(),
        )));
        let frag_module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(Cow::from(
            frag_shader.as_quad_bytes(),
        )));

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("deferred-quad-pipeline"),
            layout: Some(&pipeline_layout),
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
                format: WgpuOutput::OUTPUT_FORMAT,
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
            pipeline,
            sampler,
        }
    }

    pub fn update_bind_groups(&mut self, device: &wgpu::Device, output: &WgpuOutput) {
        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit-bind-group"),
            layout: &self.bind_group_layout,
            entries: &[
                output.as_binding(0, WgpuView::Output),
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, output: &wgpu::TextureView) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: output,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
                resolve_target: None,
            }],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}

pub struct BlitPass {
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl BlitPass {
    pub fn new(device: &wgpu::Device, output: &WgpuOutput) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit-bind-group-layout"),
            entries: &[
                output.as_storage_entry(0, wgpu::ShaderStage::FRAGMENT, WgpuView::Albedo, true),
                output.as_storage_entry(1, wgpu::ShaderStage::FRAGMENT, WgpuView::Radiance, true),
                output.as_storage_entry(2, wgpu::ShaderStage::FRAGMENT, WgpuView::SSAO, true),
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit-bind-group"),
            layout: &bind_group_layout,
            entries: &[
                output.as_binding(0, WgpuView::Albedo),
                output.as_binding(1, WgpuView::Radiance),
                output.as_binding(2, WgpuView::SSAO),
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let vert_shader: &[u8] = include_bytes!("../shaders/quad.vert.spv");
        let frag_shader: &[u8] = include_bytes!("../shaders/deferred_blit.frag.spv");

        let vert_module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(Cow::from(
            vert_shader.as_quad_bytes(),
        )));
        let frag_module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(Cow::from(
            frag_shader.as_quad_bytes(),
        )));

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("deferred-blit-pipeline"),
            layout: Some(&pipeline_layout),
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
                format: WgpuOutput::OUTPUT_FORMAT,
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
            pipeline,
        }
    }

    pub fn update_bind_groups(&mut self, device: &wgpu::Device, output: &WgpuOutput) {
        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit-bind-group"),
            layout: &self.bind_group_layout,
            entries: &[
                output.as_binding(0, WgpuView::Albedo),
                output.as_binding(1, WgpuView::Radiance),
                output.as_binding(2, WgpuView::SSAO),
            ],
        });
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, output: &wgpu::TextureView) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: output,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
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
    pipeline: wgpu::ComputePipeline,

    filter_uniform_direction_buffer: wgpu::Buffer,
    filter_direction_x: wgpu::Buffer,
    filter_direction_y: wgpu::Buffer,
    filter_bind_group_layout: wgpu::BindGroupLayout,
    filter_bind_group1: wgpu::BindGroup,
    filter_bind_group2: wgpu::BindGroup,
    filter_pipeline: wgpu::ComputePipeline,
}

impl SSAOPass {
    pub fn new(
        device: &wgpu::Device,
        uniform_bind_group_layout: &wgpu::BindGroupLayout,
        output: &WgpuOutput,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            compare: None,
            anisotropy_clamp: None,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ssao-bind-group-layout"),
            entries: &[
                output.as_storage_entry(0, wgpu::ShaderStage::COMPUTE, WgpuView::SSAO, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    count: None,
                    visibility: wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::Sampler { comparison: false },
                },
                output.as_sampled_entry(2, wgpu::ShaderStage::COMPUTE, WgpuView::ScreenSpace),
                output.as_sampled_entry(3, wgpu::ShaderStage::COMPUTE, WgpuView::Normal),
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ssao-bind-group"),
            layout: &bind_group_layout,
            entries: &[
                output.as_binding(0, WgpuView::SSAO),
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                output.as_binding(2, WgpuView::ScreenSpace),
                output.as_binding(3, WgpuView::Normal),
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[uniform_bind_group_layout, &bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader: &[u8] = include_bytes!("../shaders/ssao.comp.spv");
        let shader_module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(
            Cow::from(shader.as_quad_bytes()),
        ));
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ssao-pipeline"),
            layout: Some(&pipeline_layout),
            compute_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &shader_module,
            },
        });

        let filter_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("filter-bind-group-layout"),
                entries: &[
                    output.as_storage_entry(0, wgpu::ShaderStage::COMPUTE, WgpuView::SSAO, false),
                    output.as_storage_entry(1, wgpu::ShaderStage::COMPUTE, WgpuView::SSAO, true),
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::UniformBuffer {
                            min_binding_size: None,
                            dynamic: false,
                        },
                    },
                ],
            });

        let direction_x: [u32; 2] = [1, 0];
        let direction_y: [u32; 2] = [0, 1];
        let dir_x = unsafe { std::slice::from_raw_parts(direction_x.as_ptr() as *const u8, 8) };
        let dir_y = unsafe { std::slice::from_raw_parts(direction_y.as_ptr() as *const u8, 8) };
        let filter_direction_x = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: dir_x,
            usage: wgpu::BufferUsage::COPY_SRC,
        });
        let filter_direction_y = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: dir_y,
            usage: wgpu::BufferUsage::COPY_SRC,
        });

        let filter_uniform_direction_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("filter-uniform-direction-mem"),
            size: 8,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        let filter_bind_group1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("filter-bind-group"),
            layout: &filter_bind_group_layout,
            entries: &[
                output.as_binding(0, WgpuView::FilteredSSAO),
                output.as_binding(1, WgpuView::SSAO),
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(
                        filter_uniform_direction_buffer.slice(0..8),
                    ),
                },
            ],
        });

        let filter_bind_group2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("filter-bind-group"),
            layout: &filter_bind_group_layout,
            entries: &[
                output.as_binding(0, WgpuView::SSAO),
                output.as_binding(1, WgpuView::FilteredSSAO),
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(
                        filter_uniform_direction_buffer.slice(0..8),
                    ),
                },
            ],
        });

        let filter_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&filter_bind_group_layout],
                push_constant_ranges: &[],
            });
        let shader: &[u8] = include_bytes!("../shaders/ssao_filter.comp.spv");
        let shader_module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(
            Cow::from(shader.as_quad_bytes()),
        ));
        let filter_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ssao-filter-pipeline"),
            layout: Some(&filter_pipeline_layout),
            compute_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &shader_module,
            },
        });

        Self {
            sampler,
            bind_group_layout,
            bind_group,
            pipeline,
            filter_uniform_direction_buffer,
            filter_direction_x,
            filter_direction_y,
            filter_bind_group_layout,
            filter_bind_group1,
            filter_bind_group2,
            filter_pipeline,
        }
    }

    pub fn update_bind_groups(&mut self, device: &wgpu::Device, output: &WgpuOutput) {
        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ssao-bind-group"),
            layout: &self.bind_group_layout,
            entries: &[
                output.as_binding(0, WgpuView::SSAO),
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                output.as_binding(2, WgpuView::ScreenSpace),
                output.as_binding(3, WgpuView::Normal),
            ],
        });

        self.filter_bind_group1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("filter-bind-group"),
            layout: &self.filter_bind_group_layout,
            entries: &[
                output.as_binding(0, WgpuView::FilteredSSAO),
                output.as_binding(1, WgpuView::SSAO),
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(
                        self.filter_uniform_direction_buffer.slice(0..8),
                    ),
                },
            ],
        });

        self.filter_bind_group2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("filter-bind-group"),
            layout: &self.filter_bind_group_layout,
            entries: &[
                output.as_binding(0, WgpuView::SSAO),
                output.as_binding(1, WgpuView::FilteredSSAO),
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(
                        self.filter_uniform_direction_buffer.slice(0..8),
                    ),
                },
            ],
        });
    }

    pub fn launch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        width: u32,
        height: u32,
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
    pipeline: wgpu::ComputePipeline,
    shadow_sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    lights_bind_group_layout: wgpu::BindGroupLayout,
    lights_bind_group: wgpu::BindGroup,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    uniform_bind_group: wgpu::BindGroup,
}

impl RadiancePass {
    pub fn new(
        device: &wgpu::Device,
        camera_buffer: &wgpu::Buffer,
        material_buffer: &wgpu::Buffer,
        output: &WgpuOutput,
        lights: &WgpuLights,
    ) -> Self {
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::UniformBuffer {
                            dynamic: false,
                            min_binding_size: NonZeroU64::new(
                                super::WgpuBackend::UNIFORM_CAMERA_SIZE,
                            ),
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                    },
                    output.as_storage_entry(
                        2,
                        wgpu::ShaderStage::COMPUTE,
                        WgpuView::Radiance,
                        false,
                    ),
                ],
            });

        let uniform_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &uniform_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(camera_buffer.slice(
                            0..super::WgpuBackend::UNIFORM_CAMERA_SIZE as wgpu::BufferAddress,
                        )),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(material_buffer.slice(..)),
                    },
                    output.as_binding(2, WgpuView::Radiance),
                ],
            });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("radiance-bind-group-layout"),
            entries: &[
                output.as_storage_entry(0, wgpu::ShaderStage::COMPUTE, WgpuView::Albedo, true),
                output.as_storage_entry(1, wgpu::ShaderStage::COMPUTE, WgpuView::Normal, true),
                output.as_storage_entry(2, wgpu::ShaderStage::COMPUTE, WgpuView::WorldPos, true),
                output.as_storage_entry(3, wgpu::ShaderStage::COMPUTE, WgpuView::MatParams, true),
            ],
        });

        let lights_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("lights-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::UniformBuffer {
                            min_binding_size: None,
                            dynamic: false,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::UniformBuffer {
                            min_binding_size: None,
                            dynamic: false,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::UniformBuffer {
                            min_binding_size: None,
                            dynamic: false,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::Sampler { comparison: false },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::SampledTexture {
                            component_type: wgpu::TextureComponentType::Float,
                            dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::SampledTexture {
                            component_type: wgpu::TextureComponentType::Float,
                            dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 8,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::SampledTexture {
                            component_type: wgpu::TextureComponentType::Float,
                            dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 10,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::UniformBuffer {
                            min_binding_size: None,
                            dynamic: false,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 11,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::UniformBuffer {
                            min_binding_size: None,
                            dynamic: false,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 12,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::UniformBuffer {
                            min_binding_size: None,
                            dynamic: false,
                        },
                    },
                ],
            });
        let shadow_sampler = ShadowMapArray::create_sampler(device);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            label: Some("output-bind-group"),
            entries: &[
                output.as_binding(0, WgpuView::Albedo),
                output.as_binding(1, WgpuView::Normal),
                output.as_binding(2, WgpuView::WorldPos),
                output.as_binding(3, WgpuView::MatParams),
            ],
        });

        let lights_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lights-bind-group"),
            layout: &lights_bind_group_layout,
            entries: &[
                lights.area_lights.uniform_binding(1),
                lights.spot_lights.uniform_binding(2),
                lights.directional_lights.uniform_binding(3),
                wgpu::BindGroupEntry {
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
            label: None,
            bind_group_layouts: &[
                &uniform_bind_group_layout,
                &bind_group_layout,
                &lights_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let spirv: &[u8] = include_bytes!("../shaders/lighting.comp.spv");
        let module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(Cow::from(
            spirv.as_quad_bytes(),
        )));

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("lighting-pipeline"),
            layout: Some(&pipeline_layout),
            compute_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &module,
            },
        });

        Self {
            pipeline,
            shadow_sampler,
            bind_group_layout,
            bind_group,
            lights_bind_group_layout,
            lights_bind_group,
            uniform_bind_group_layout,
            uniform_bind_group,
        }
    }

    pub fn update_bind_groups(
        &mut self,
        device: &wgpu::Device,
        output: &WgpuOutput,
        lights: &WgpuLights,
        camera_buffer: &wgpu::Buffer,
        material_buffer: &wgpu::Buffer,
    ) {
        self.uniform_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.uniform_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(camera_buffer.slice(
                            0..super::WgpuBackend::UNIFORM_CAMERA_SIZE as wgpu::BufferAddress,
                        )),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(material_buffer.slice(..)),
                    },
                    output.as_binding(2, WgpuView::Radiance),
                ],
            });

        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.bind_group_layout,
            label: Some("output-bind-group"),
            entries: &[
                output.as_binding(0, WgpuView::Albedo),
                output.as_binding(1, WgpuView::Normal),
                output.as_binding(2, WgpuView::WorldPos),
                output.as_binding(3, WgpuView::MatParams),
            ],
        });

        self.lights_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lights-bind-group"),
            layout: &self.lights_bind_group_layout,
            entries: &[
                lights.area_lights.uniform_binding(1),
                lights.spot_lights.uniform_binding(2),
                lights.directional_lights.uniform_binding(3),
                wgpu::BindGroupEntry {
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

    pub fn launch(&self, encoder: &mut wgpu::CommandEncoder, width: u32, height: u32) {
        let mut compute_pass = encoder.begin_compute_pass();
        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        compute_pass.set_bind_group(1, &self.bind_group, &[]);
        compute_pass.set_bind_group(2, &self.lights_bind_group, &[]);
        compute_pass.dispatch(
            (width as f32 / 8.0).ceil() as u32,
            (height as f32 / 8.0).ceil() as u32,
            1,
        );
    }
}
