use super::output::*;
use rfw::prelude::*;
use std::borrow::Cow;

pub struct RenderPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub anim_pipeline: wgpu::RenderPipeline,
    pub layout: wgpu::PipelineLayout,
    pub anim_layout: wgpu::PipelineLayout,
}

impl RenderPipeline {
    pub fn new(
        device: &wgpu::Device,
        uniform_layout: &wgpu::BindGroupLayout,
        skin_layout: &wgpu::BindGroupLayout,
        texture_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let vert_shader: &[u8] = include_bytes!("../shaders/mesh.vert.spv");
        let frag_shader: &[u8] = include_bytes!("../shaders/deferred.frag.spv");

        let vert_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            flags: Default::default(),
            label: None,
            source: wgpu::ShaderSource::SpirV(Cow::from(vert_shader.as_quad_bytes())),
        });
        let frag_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            flags: Default::default(),
            label: None,
            source: wgpu::ShaderSource::SpirV(Cow::from(frag_shader.as_quad_bytes())),
        });

        let fragment_state = wgpu::FragmentState {
            module: &frag_module,
            entry_point: "main",
            targets: &[
                wgpu::ColorTargetState {
                    // Albedo
                    format: WgpuOutput::STORAGE_FORMAT,
                    alpha_blend: wgpu::BlendState::REPLACE,
                    color_blend: wgpu::BlendState::REPLACE,
                    write_mask: wgpu::ColorWrite::ALL,
                },
                wgpu::ColorTargetState {
                    // Normal
                    format: WgpuOutput::STORAGE_FORMAT,
                    alpha_blend: wgpu::BlendState::REPLACE,
                    color_blend: wgpu::BlendState::REPLACE,
                    write_mask: wgpu::ColorWrite::ALL,
                },
                wgpu::ColorTargetState {
                    // World pos
                    format: WgpuOutput::STORAGE_FORMAT,
                    alpha_blend: wgpu::BlendState::REPLACE,
                    color_blend: wgpu::BlendState::REPLACE,
                    write_mask: wgpu::ColorWrite::ALL,
                },
                wgpu::ColorTargetState {
                    // Screen space
                    format: WgpuOutput::STORAGE_FORMAT,
                    alpha_blend: wgpu::BlendState::REPLACE,
                    color_blend: wgpu::BlendState::REPLACE,
                    write_mask: wgpu::ColorWrite::ALL,
                },
                wgpu::ColorTargetState {
                    // Mat params
                    format: WgpuOutput::MAT_PARAM_FORMAT,
                    alpha_blend: wgpu::BlendState::REPLACE,
                    color_blend: wgpu::BlendState::REPLACE,
                    write_mask: wgpu::ColorWrite::ALL,
                },
            ],
        };

        let primitive_state = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: wgpu::CullMode::Back,
            polygon_mode: wgpu::PolygonMode::Fill,
            strip_index_format: None,
        };

        let depth_state = wgpu::DepthStencilState {
            format: WgpuOutput::DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: Default::default(),
            bias: wgpu::DepthBiasState::default(),
            clamp_depth: false,
        };

        let multisample_state = wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        };

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            // bind_group_layouts: &[uniform_layout, instance_layout, texture_layout],
            bind_group_layouts: &[uniform_layout, texture_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mesh-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &vert_module,
                entry_point: "main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex3D>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            format: wgpu::VertexFormat::Float4,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            format: wgpu::VertexFormat::Float3,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            offset: 28,
                            format: wgpu::VertexFormat::Uint,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            format: wgpu::VertexFormat::Float2,
                            shader_location: 3,
                        },
                        wgpu::VertexAttribute {
                            offset: 40,
                            format: wgpu::VertexFormat::Float4,
                            shader_location: 4,
                        },
                    ],
                }],
            },
            fragment: Some(fragment_state.clone()),
            primitive: primitive_state.clone(),
            depth_stencil: Some(depth_state.clone()),
            multisample: multisample_state.clone(),
        });

        let vert_shader: &[u8] = include_bytes!("../shaders/mesh_anim.vert.spv");

        let vert_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            flags: Default::default(),
            label: None,
            source: wgpu::ShaderSource::SpirV(Cow::from(vert_shader.as_quad_bytes())),
        });

        let anim_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[uniform_layout, texture_layout, skin_layout],
            push_constant_ranges: &[],
        });
        let anim_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("anim-mesh-pipeline"),
            layout: Some(&anim_layout),
            vertex: wgpu::VertexState {
                module: &vert_module,
                entry_point: "main",
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex3D>() as wgpu::BufferAddress,
                        step_mode: wgpu::InputStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                format: wgpu::VertexFormat::Float4,
                                shader_location: 0,
                            },
                            wgpu::VertexAttribute {
                                offset: 16,
                                format: wgpu::VertexFormat::Float3,
                                shader_location: 1,
                            },
                            wgpu::VertexAttribute {
                                offset: 28,
                                format: wgpu::VertexFormat::Uint,
                                shader_location: 2,
                            },
                            wgpu::VertexAttribute {
                                offset: 32,
                                format: wgpu::VertexFormat::Float2,
                                shader_location: 3,
                            },
                            wgpu::VertexAttribute {
                                offset: 40,
                                format: wgpu::VertexFormat::Float4,
                                shader_location: 4,
                            },
                        ],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<JointData>() as wgpu::BufferAddress,
                        step_mode: wgpu::InputStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                format: wgpu::VertexFormat::Uint4,
                                shader_location: 5,
                            },
                            wgpu::VertexAttribute {
                                offset: 16,
                                format: wgpu::VertexFormat::Float4,
                                shader_location: 6,
                            },
                        ],
                    },
                ],
            },
            fragment: Some(fragment_state),
            primitive: primitive_state,
            depth_stencil: Some(depth_state),
            multisample: multisample_state,
        });

        Self {
            pipeline,
            anim_pipeline,
            layout,
            anim_layout,
        }
    }
}
