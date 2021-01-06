use rfw::prelude::*;
use std::borrow::Cow;

#[derive(Debug)]
pub struct WgpuOutput {
    pub width: usize,
    pub height: usize,

    blit_output_layout: wgpu::BindGroupLayout,
    blit_debug_layout: wgpu::BindGroupLayout,

    blit_pipeline: wgpu::RenderPipeline,

    blit_debug_pipeline: wgpu::RenderPipeline,

    debug_bind_groups: Vec<wgpu::BindGroup>,

    pub output_texture: wgpu::Texture,
    pub output_texture_view: wgpu::TextureView,
    pub output_sampler: wgpu::Sampler,

    pub depth_texture: wgpu::Texture,
    pub depth_texture_view: wgpu::TextureView,

    pub albedo_texture: wgpu::Texture,
    pub albedo_view: wgpu::TextureView,

    pub normal_texture: wgpu::Texture,
    pub normal_view: wgpu::TextureView,

    pub world_pos_texture: wgpu::Texture,
    pub world_pos_view: wgpu::TextureView,

    pub radiance_texture: wgpu::Texture,
    pub radiance_view: wgpu::TextureView,

    pub screen_space_texture: wgpu::Texture,
    pub screen_space_view: wgpu::TextureView,

    pub intermediate_texture: wgpu::Texture,
    pub intermediate_view: wgpu::TextureView,

    pub ssao_output: wgpu::Texture,
    pub ssao_output_view: wgpu::TextureView,

    pub ssao_filtered_output: wgpu::Texture,
    pub ssao_filtered_output_view: wgpu::TextureView,

    pub mat_param_texture: wgpu::Texture,
    pub mat_param_view: wgpu::TextureView,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum WgpuView {
    Output = 0,
    Albedo = 1,
    Normal = 2,
    WorldPos = 3,
    Radiance = 4,
    ScreenSpace = 5,
    SSAO = 6,
    FilteredSSAO = 7,
    MatParams = 8,
}

impl WgpuView {
    pub const COUNT: usize = 9;
}

impl From<isize> for WgpuView {
    fn from(index: isize) -> Self {
        match index {
            0 => WgpuView::Output,
            1 => WgpuView::Albedo,
            2 => WgpuView::Normal,
            3 => WgpuView::WorldPos,
            4 => WgpuView::Radiance,
            5 => WgpuView::ScreenSpace,
            6 => WgpuView::SSAO,
            7 => WgpuView::FilteredSSAO,
            8 => WgpuView::MatParams,
            _ => WgpuView::Output,
        }
    }
}

impl From<usize> for WgpuView {
    fn from(index: usize) -> Self {
        match index {
            0 => WgpuView::Output,
            1 => WgpuView::Albedo,
            2 => WgpuView::Normal,
            3 => WgpuView::WorldPos,
            4 => WgpuView::Radiance,
            5 => WgpuView::ScreenSpace,
            6 => WgpuView::SSAO,
            7 => WgpuView::FilteredSSAO,
            8 => WgpuView::MatParams,
            _ => WgpuView::Output,
        }
    }
}

impl WgpuOutput {
    pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
    pub const STORAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
    pub const SSAO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    pub const MAT_PARAM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    pub fn new(device: &wgpu::Device, width: usize, height: usize) -> Self {
        let output_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            compare: None,
            anisotropy_clamp: None,
        });

        let blit_output_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("blit-output-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        count: None,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::SampledTexture {
                            component_type: wgpu::TextureComponentType::Uint,
                            dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        count: None,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler { comparison: false },
                    },
                ],
            });
        let blit_debug_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit-debug-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    count: None,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::SampledTexture {
                        component_type: wgpu::TextureComponentType::Float,
                        dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    count: None,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Sampler { comparison: false },
                },
            ],
        });

        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&blit_output_layout],
            push_constant_ranges: &[],
        });

        let blit_debug_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&blit_debug_layout],
                push_constant_ranges: &[],
            });

        let vert_spirv: &[u8] = include_bytes!("../shaders/quad.vert.spv");
        let frag_spirv: &[u8] = include_bytes!("../shaders/quad.frag.spv");

        let vert_module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(Cow::from(
            vert_spirv.as_quad_bytes(),
        )));
        let frag_module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(Cow::from(
            frag_spirv.as_quad_bytes(),
        )));

        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit-pipeline"),
            layout: Some(&blit_pipeline_layout),
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &vert_module,
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: &frag_module,
                entry_point: "main",
            }),
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                clamp_depth: false,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            }),
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: Self::OUTPUT_FORMAT,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
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

        let blit_debug_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit-debug-pipeline"),
            layout: Some(&blit_debug_pipeline_layout),
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &vert_module,
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: &frag_module,
                entry_point: "main",
            }),
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                clamp_depth: false,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            }),
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: Self::OUTPUT_FORMAT,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
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

        let output_texture = Self::create_texture(device, Self::OUTPUT_FORMAT, width, height);
        let output_texture_view = output_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::OUTPUT_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let depth_texture = Self::create_texture(device, Self::DEPTH_FORMAT, width, height);
        let depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::DEPTH_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::DepthOnly,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let albedo_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        let albedo_view = albedo_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::STORAGE_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let normal_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        let normal_view = normal_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::STORAGE_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let world_pos_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        let world_pos_view = world_pos_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::STORAGE_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let radiance_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        let radiance_view = radiance_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::STORAGE_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let screen_space_texture =
            Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        let screen_space_view = screen_space_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::STORAGE_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let intermediate_texture =
            Self::create_texture(device, super::WgpuBackend::OUTPUT_FORMAT, width, height);
        let intermediate_view = intermediate_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::OUTPUT_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let ssao_output = Self::create_texture(device, Self::SSAO_FORMAT, width, height);
        let ssao_output_view = ssao_output.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::SSAO_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let ssao_filtered_output = Self::create_texture(device, Self::SSAO_FORMAT, width, height);
        let ssao_filtered_output_view =
            ssao_filtered_output.create_view(&wgpu::TextureViewDescriptor {
                label: None,
                format: Some(Self::SSAO_FORMAT),
                dimension: None,
                aspect: wgpu::TextureAspect::All,

                base_mip_level: 0,
                level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            });

        let mat_param_texture = Self::create_texture(device, Self::MAT_PARAM_FORMAT, width, height);
        let mat_param_view = mat_param_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::MAT_PARAM_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let debug_bind_groups = (0..WgpuView::COUNT)
            .into_iter()
            .map(|i| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("debug-blit-bind-group"),
                    layout: if i == 0 {
                        &blit_output_layout
                    } else {
                        &blit_debug_layout
                    },
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(match i {
                                0 => &output_texture_view,
                                1 => &albedo_view,
                                2 => &normal_view,
                                3 => &world_pos_view,
                                4 => &radiance_view,
                                5 => &screen_space_view,
                                6 => &ssao_output_view,
                                7 => &ssao_filtered_output_view,
                                8 => &mat_param_view,
                                _ => &output_texture_view,
                            }),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&output_sampler),
                        },
                    ],
                })
            })
            .collect();

        WgpuOutput {
            width,
            height,
            blit_output_layout,
            blit_debug_layout,
            blit_pipeline,
            blit_debug_pipeline,
            debug_bind_groups,
            output_texture,
            output_texture_view,
            output_sampler,
            depth_texture,
            depth_texture_view,
            albedo_texture,
            albedo_view,
            normal_texture,
            normal_view,
            world_pos_texture,
            world_pos_view,
            radiance_texture,
            radiance_view,
            screen_space_texture,
            screen_space_view,
            intermediate_texture,
            intermediate_view,
            ssao_output,
            ssao_output_view,
            ssao_filtered_output,
            ssao_filtered_output_view,
            mat_param_texture,
            mat_param_view,
        }
    }

    fn create_texture(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: usize,
        height: usize,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT
                | wgpu::TextureUsage::SAMPLED
                | wgpu::TextureUsage::STORAGE,
        })
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: usize, height: usize) {
        self.width = width;
        self.height = height;

        let output_texture = Self::create_texture(device, Self::OUTPUT_FORMAT, width, height);
        self.output_texture_view = output_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::OUTPUT_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });
        self.output_texture = output_texture;

        let depth_texture = Self::create_texture(device, Self::DEPTH_FORMAT, width, height);
        self.depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::DEPTH_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });
        self.depth_texture = depth_texture;

        let albedo_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        self.albedo_view = albedo_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::STORAGE_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });
        self.albedo_texture = albedo_texture;

        let normal_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        self.normal_view = normal_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::STORAGE_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });
        self.normal_texture = normal_texture;

        let world_pos_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        self.world_pos_view = world_pos_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::STORAGE_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });
        self.world_pos_texture = world_pos_texture;

        let radiance_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        self.radiance_view = radiance_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::STORAGE_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });
        self.radiance_texture = radiance_texture;

        let screen_space_texture =
            Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        self.screen_space_view = screen_space_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::STORAGE_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });
        self.screen_space_texture = screen_space_texture;

        let intermediate_texture =
            Self::create_texture(device, super::WgpuBackend::OUTPUT_FORMAT, width, height);
        self.intermediate_view = intermediate_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::OUTPUT_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });
        self.intermediate_texture = intermediate_texture;

        let ssao_output = Self::create_texture(device, Self::SSAO_FORMAT, width, height);
        self.ssao_output_view = ssao_output.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::SSAO_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });
        self.ssao_output = ssao_output;

        let ssao_filtered_output = Self::create_texture(device, Self::SSAO_FORMAT, width, height);
        self.ssao_filtered_output_view =
            ssao_filtered_output.create_view(&wgpu::TextureViewDescriptor {
                label: None,
                format: Some(Self::SSAO_FORMAT),
                dimension: None,
                aspect: wgpu::TextureAspect::All,

                base_mip_level: 0,
                level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            });
        self.ssao_filtered_output = ssao_filtered_output;

        let mat_param_texture = Self::create_texture(device, Self::MAT_PARAM_FORMAT, width, height);
        self.mat_param_view = mat_param_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::MAT_PARAM_FORMAT),
            dimension: None,
            aspect: wgpu::TextureAspect::All,

            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });
        self.mat_param_texture = mat_param_texture;

        self.debug_bind_groups = (0..WgpuView::COUNT)
            .into_iter()
            .map(|i| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("debug-blit-bind-group"),
                    layout: if i == 0 {
                        &self.blit_output_layout
                    } else {
                        &self.blit_debug_layout
                    },
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(match i {
                                0 => &self.output_texture_view,
                                1 => &self.albedo_view,
                                2 => &self.normal_view,
                                3 => &self.world_pos_view,
                                4 => &self.radiance_view,
                                5 => &self.screen_space_view,
                                6 => &self.ssao_output_view,
                                7 => &self.ssao_filtered_output_view,
                                8 => &self.mat_param_view,
                                _ => &self.output_texture_view,
                            }),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.output_sampler),
                        },
                    ],
                })
            })
            .collect();
    }

    pub fn as_descriptor(&self, view: WgpuView) -> wgpu::RenderPassColorAttachmentDescriptor {
        wgpu::RenderPassColorAttachmentDescriptor {
            attachment: match view {
                WgpuView::Output => &self.output_texture_view,
                WgpuView::Albedo => &self.albedo_view,
                WgpuView::Normal => &self.normal_view,
                WgpuView::WorldPos => &self.world_pos_view,
                WgpuView::Radiance => &self.radiance_view,
                WgpuView::ScreenSpace => &self.screen_space_view,
                WgpuView::SSAO => &self.ssao_output_view,
                WgpuView::FilteredSSAO => &self.ssao_filtered_output_view,
                WgpuView::MatParams => &self.mat_param_view,
            },
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: true,
            },
        }
    }

    pub fn as_depth_descriptor(&self) -> wgpu::RenderPassDepthStencilAttachmentDescriptor {
        wgpu::RenderPassDepthStencilAttachmentDescriptor {
            attachment: &self.depth_texture_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: true,
            }),
            stencil_ops: None,
        }
    }

    pub fn as_sampled_entry(
        &self,
        binding: usize,
        visibility: wgpu::ShaderStage,
        view: WgpuView,
    ) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding: binding as u32,
            count: None,
            visibility,
            ty: wgpu::BindingType::SampledTexture {
                component_type: match view {
                    WgpuView::Output => wgpu::TextureComponentType::Uint,
                    _ => wgpu::TextureComponentType::Float,
                },
                dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
        }
    }

    pub fn as_storage_entry(
        &self,
        binding: usize,
        visibility: wgpu::ShaderStage,
        view: WgpuView,
        readonly: bool,
    ) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding: binding as u32,
            count: None,
            visibility,
            ty: wgpu::BindingType::StorageTexture {
                format: match view {
                    WgpuView::Output => Self::OUTPUT_FORMAT,
                    WgpuView::SSAO | WgpuView::FilteredSSAO => Self::SSAO_FORMAT,
                    _ => Self::STORAGE_FORMAT,
                },
                readonly,
                dimension: wgpu::TextureViewDimension::D2,
            },
        }
    }

    pub fn as_binding(&self, binding: usize, view: WgpuView) -> wgpu::BindGroupEntry {
        wgpu::BindGroupEntry {
            binding: binding as u32,
            resource: wgpu::BindingResource::TextureView(match view {
                WgpuView::Output => &self.output_texture_view,
                WgpuView::Albedo => &self.albedo_view,
                WgpuView::Normal => &self.normal_view,
                WgpuView::WorldPos => &self.world_pos_view,
                WgpuView::Radiance => &self.radiance_view,
                WgpuView::ScreenSpace => &self.screen_space_view,
                WgpuView::SSAO => &self.ssao_output_view,
                WgpuView::FilteredSSAO => &self.ssao_filtered_output_view,
                WgpuView::MatParams => &self.mat_param_view,
            }),
        }
    }

    pub fn blit_debug(
        &self,
        output: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        view: WgpuView,
    ) {
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

        if view as u32 == 0 {
            render_pass.set_pipeline(&self.blit_pipeline);
        } else {
            render_pass.set_pipeline(&self.blit_debug_pipeline);
        }

        let bind_group = &self.debug_bind_groups[view as usize];
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}
