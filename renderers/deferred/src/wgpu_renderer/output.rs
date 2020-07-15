use shared::*;

pub struct DeferredOutput {
    pub width: usize,
    pub height: usize,

    blit_output_layout: wgpu::BindGroupLayout,
    blit_debug_layout: wgpu::BindGroupLayout,

    blit_pipeline: wgpu::RenderPipeline,
    blit_pipeline_layout: wgpu::PipelineLayout,

    blit_debug_pipeline: wgpu::RenderPipeline,
    blit_debug_pipeline_layout: wgpu::PipelineLayout,

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
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum DeferredView {
    Output = 0,
    Albedo = 1,
    Normal = 2,
    WorldPos = 3,
    Radiance = 4,
    ScreenSpace = 5,
    SSAO = 6,
    FilteredSSAO = 7,
}

impl DeferredView {
    pub const COUNT: usize = 8;
}

impl From<isize> for DeferredView {
    fn from(index: isize) -> Self {
        match index {
            0 => DeferredView::Output,
            1 => DeferredView::Albedo,
            2 => DeferredView::Normal,
            3 => DeferredView::WorldPos,
            4 => DeferredView::Radiance,
            5 => DeferredView::ScreenSpace,
            6 => DeferredView::SSAO,
            7 => DeferredView::FilteredSSAO,
            _ => DeferredView::Output,
        }
    }
}

impl From<usize> for DeferredView {
    fn from(index: usize) -> Self {
        match index {
            0 => DeferredView::Output,
            1 => DeferredView::Albedo,
            2 => DeferredView::Normal,
            3 => DeferredView::WorldPos,
            4 => DeferredView::Radiance,
            5 => DeferredView::ScreenSpace,
            6 => DeferredView::SSAO,
            7 => DeferredView::FilteredSSAO,
            _ => DeferredView::Output,
        }
    }
}

impl DeferredOutput {
    pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
    pub const STORAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
    pub const SSAO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    pub const VIEW_DIMENSION: wgpu::TextureViewDimension = wgpu::TextureViewDimension::D2;

    pub const OUTPUT_TYPE: wgpu::TextureComponentType = wgpu::TextureComponentType::Uint;
    pub const STORAGE_TYPE: wgpu::TextureComponentType = wgpu::TextureComponentType::Float;

    pub fn new(
        device: &wgpu::Device,
        width: usize,
        height: usize,
        compiler: &mut Compiler,
    ) -> Self {
        let output_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            compare: wgpu::CompareFunction::Never,
        });

        let blit_output_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("blit-output-layout"),
                bindings: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::SampledTexture {
                            component_type: wgpu::TextureComponentType::Uint,
                            dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler { comparison: false },
                    },
                ],
            });
        let blit_debug_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit-debug-layout"),
            bindings: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::SampledTexture {
                        component_type: wgpu::TextureComponentType::Float,
                        dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Sampler { comparison: false },
                },
            ],
        });

        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&blit_output_layout],
        });

        let blit_debug_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[&blit_debug_layout],
            });

        let vert_spirv = compiler
            .compile_from_file("renderers/deferred/shaders/quad.vert", ShaderKind::Vertex)
            .unwrap();
        let frag_spirv = compiler
            .compile_from_file("renderers/deferred/shaders/quad.frag", ShaderKind::Fragment)
            .unwrap();

        let vert_module = device.create_shader_module(vert_spirv.as_slice());
        let frag_module = device.create_shader_module(frag_spirv.as_slice());

        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &blit_pipeline_layout,
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
            layout: &blit_debug_pipeline_layout,
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
        let output_texture_view = output_texture.create_default_view();

        let depth_texture = Self::create_texture(device, Self::DEPTH_FORMAT, width, height);
        let depth_texture_view = depth_texture.create_default_view();

        let albedo_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        let albedo_view = albedo_texture.create_default_view();

        let normal_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        let normal_view = normal_texture.create_default_view();

        let world_pos_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        let world_pos_view = world_pos_texture.create_default_view();

        let radiance_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        let radiance_view = radiance_texture.create_default_view();

        let screen_space_texture =
            Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        let screen_space_view = screen_space_texture.create_default_view();

        let intermediate_texture =
            Self::create_texture(device, super::Deferred::OUTPUT_FORMAT, width, height);
        let intermediate_view = intermediate_texture.create_default_view();

        let ssao_output = Self::create_texture(device, Self::SSAO_FORMAT, width, height);
        let ssao_output_view = ssao_output.create_default_view();

        let ssao_filtered_output = Self::create_texture(device, Self::SSAO_FORMAT, width, height);
        let ssao_filtered_output_view = ssao_filtered_output.create_default_view();

        let debug_bind_groups = (0..DeferredView::COUNT)
            .into_iter()
            .map(|i| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("debug-blit-bind-group"),
                    layout: if i == 0 {
                        &blit_output_layout
                    } else {
                        &blit_debug_layout
                    },
                    bindings: &[
                        wgpu::Binding {
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
                                _ => &output_texture_view,
                            }),
                        },
                        wgpu::Binding {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&output_sampler),
                        },
                    ],
                })
            })
            .collect();

        DeferredOutput {
            width,
            height,
            blit_output_layout,
            blit_debug_layout,
            blit_pipeline,
            blit_pipeline_layout,
            blit_debug_pipeline,
            blit_debug_pipeline_layout,
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
            array_layer_count: 1,
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
        self.output_texture_view = output_texture.create_default_view();
        self.output_texture = output_texture;

        let depth_texture = Self::create_texture(device, Self::DEPTH_FORMAT, width, height);
        self.depth_texture_view = depth_texture.create_default_view();
        self.depth_texture = depth_texture;

        let albedo_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        self.albedo_view = albedo_texture.create_default_view();
        self.albedo_texture = albedo_texture;

        let normal_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        self.normal_view = normal_texture.create_default_view();
        self.normal_texture = normal_texture;

        let world_pos_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        self.world_pos_view = world_pos_texture.create_default_view();
        self.world_pos_texture = world_pos_texture;

        let radiance_texture = Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        self.radiance_view = radiance_texture.create_default_view();
        self.radiance_texture = radiance_texture;

        let screen_space_texture =
            Self::create_texture(device, Self::STORAGE_FORMAT, width, height);
        self.screen_space_view = screen_space_texture.create_default_view();
        self.screen_space_texture = screen_space_texture;

        let intermediate_texture =
            Self::create_texture(device, super::Deferred::OUTPUT_FORMAT, width, height);
        self.intermediate_view = intermediate_texture.create_default_view();
        self.intermediate_texture = intermediate_texture;

        let ssao_output = Self::create_texture(device, Self::SSAO_FORMAT, width, height);
        self.ssao_output_view = ssao_output.create_default_view();
        self.ssao_output = ssao_output;

        let ssao_filtered_output = Self::create_texture(device, Self::SSAO_FORMAT, width, height);
        self.ssao_filtered_output_view = ssao_filtered_output.create_default_view();
        self.ssao_filtered_output = ssao_filtered_output;

        self.debug_bind_groups = (0..DeferredView::COUNT)
            .into_iter()
            .map(|i| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("debug-blit-bind-group"),
                    layout: if i == 0 {
                        &self.blit_output_layout
                    } else {
                        &self.blit_debug_layout
                    },
                    bindings: &[
                        wgpu::Binding {
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
                                _ => &self.output_texture_view,
                            }),
                        },
                        wgpu::Binding {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.output_sampler),
                        },
                    ],
                })
            })
            .collect();
    }

    pub fn as_descriptor(&self, view: DeferredView) -> wgpu::RenderPassColorAttachmentDescriptor {
        wgpu::RenderPassColorAttachmentDescriptor {
            attachment: match view {
                DeferredView::Output => &self.output_texture_view,
                DeferredView::Albedo => &self.albedo_view,
                DeferredView::Normal => &self.normal_view,
                DeferredView::WorldPos => &self.world_pos_view,
                DeferredView::Radiance => &self.radiance_view,
                DeferredView::ScreenSpace => &self.screen_space_view,
                DeferredView::SSAO => &self.ssao_output_view,
                DeferredView::FilteredSSAO => &self.ssao_filtered_output_view,
            },
            clear_color: wgpu::Color::BLACK,
            resolve_target: None,
            load_op: wgpu::LoadOp::Clear,
            store_op: wgpu::StoreOp::Store,
        }
    }

    pub fn as_depth_descriptor(&self) -> wgpu::RenderPassDepthStencilAttachmentDescriptor {
        wgpu::RenderPassDepthStencilAttachmentDescriptor {
            attachment: &self.depth_texture_view,
            clear_depth: 1.0,
            clear_stencil: 0,
            depth_load_op: wgpu::LoadOp::Clear,
            depth_store_op: wgpu::StoreOp::Store,
            stencil_load_op: wgpu::LoadOp::Clear,
            stencil_store_op: wgpu::StoreOp::Store,
        }
    }

    pub fn as_sampled_entry(
        &self,
        binding: usize,
        visibility: wgpu::ShaderStage,
        view: DeferredView,
    ) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding: binding as u32,
            visibility,
            ty: wgpu::BindingType::SampledTexture {
                component_type: match view {
                    DeferredView::Output => wgpu::TextureComponentType::Uint,
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
        view: DeferredView,
        readonly: bool,
    ) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding: binding as u32,
            visibility,
            ty: wgpu::BindingType::StorageTexture {
                format: match view {
                    DeferredView::Output => Self::OUTPUT_FORMAT,
                    DeferredView::SSAO | DeferredView::FilteredSSAO => Self::SSAO_FORMAT,
                    _ => Self::STORAGE_FORMAT,
                },
                readonly,
                component_type: match view {
                    DeferredView::Output => wgpu::TextureComponentType::Uint,
                    _ => wgpu::TextureComponentType::Float,
                },
                dimension: wgpu::TextureViewDimension::D2,
            },
        }
    }

    pub fn as_binding(&self, binding: usize, view: DeferredView) -> wgpu::Binding {
        wgpu::Binding {
            binding: binding as u32,
            resource: wgpu::BindingResource::TextureView(match view {
                DeferredView::Output => &self.output_texture_view,
                DeferredView::Albedo => &self.albedo_view,
                DeferredView::Normal => &self.normal_view,
                DeferredView::WorldPos => &self.world_pos_view,
                DeferredView::Radiance => &self.radiance_view,
                DeferredView::ScreenSpace => &self.screen_space_view,
                DeferredView::SSAO => &self.ssao_output_view,
                DeferredView::FilteredSSAO => &self.ssao_filtered_output_view,
            }),
        }
    }

    pub fn blit_debug(
        &self,
        output: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        view: DeferredView,
    ) {
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

        if view as u32 == 0 {
            render_pass.set_pipeline(&self.blit_pipeline);
        } else {
            render_pass.set_pipeline(&self.blit_debug_pipeline);
        }

        let bind_group = match view {
            DeferredView::Output => &self.debug_bind_groups[0],
            DeferredView::Albedo => &self.debug_bind_groups[1],
            DeferredView::Normal => &self.debug_bind_groups[2],
            DeferredView::WorldPos => &self.debug_bind_groups[3],
            DeferredView::Radiance => &self.debug_bind_groups[4],
            DeferredView::ScreenSpace => &self.debug_bind_groups[5],
            DeferredView::SSAO => &self.debug_bind_groups[6],
            DeferredView::FilteredSSAO => &self.debug_bind_groups[7],
        };

        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}
