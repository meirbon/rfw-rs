use fb_template::{
    shader::*, DeviceFramebuffer, KeyCode, KeyHandler, MouseButtonHandler, Request, Ui,
};
use glam::*;

use crate::camera::*;
use crate::utils::*;
use scene::{InstanceMatrices, TriangleScene, VertexBuffer, VertexData};
use std::collections::VecDeque;
use futures::executor::block_on;

pub struct GPUApp<'a> {
    width: u32,
    height: u32,
    compiler: Compiler<'a>,
    pipeline: Option<wgpu::RenderPipeline>,
    blit_pipeline: Option<wgpu::RenderPipeline>,
    blit_bind_group_layout: Option<wgpu::BindGroupLayout>,
    blit_bind_group: Option<wgpu::BindGroup>,
    pipeline_layout: Option<wgpu::PipelineLayout>,
    triangle_bind_group_layout: Option<wgpu::BindGroupLayout>,
    bind_group_layout: Option<wgpu::BindGroupLayout>,
    bind_group: Option<wgpu::BindGroup>,
    vertex_buffers: Vec<VertexBuffer>,
    instance_bind_groups: Vec<wgpu::BindGroup>,
    instance_buffers: Vec<InstanceMatrices>,
    uniform_buffer: Option<wgpu::Buffer>,
    staging_buffer: Option<wgpu::Buffer>,
    output_texture: Option<wgpu::Texture>,
    output_texture_view: Option<wgpu::TextureView>,
    output_sampler: Option<wgpu::Sampler>,
    depth_texture: Option<wgpu::Texture>,
    depth_texture_view: Option<wgpu::TextureView>,
    material_buffer: Option<(wgpu::BufferAddress, wgpu::Buffer)>,
    scene: TriangleScene,
    camera: Camera,
    timer: Timer,
    sc_format: wgpu::TextureFormat,
    fps: Averager<f32>,
}

impl<'a> GPUApp<'a> {
    pub fn new() -> Self {
        let compiler = CompilerBuilder::new().build();

        let scene = TriangleScene::new();

        Self {
            width: 1,
            height: 1,
            compiler,
            pipeline: None,
            blit_pipeline: None,
            blit_bind_group_layout: None,
            blit_bind_group: None,
            pipeline_layout: None,
            triangle_bind_group_layout: None,
            bind_group_layout: None,
            bind_group: None,
            vertex_buffers: Vec::new(),
            instance_bind_groups: Vec::new(),
            instance_buffers: Vec::new(),
            uniform_buffer: None,
            staging_buffer: None,
            output_texture: None,
            output_texture_view: None,
            output_sampler: None,
            depth_texture: None,
            depth_texture_view: None,
            material_buffer: None,
            scene,
            camera: Camera::zero(),
            timer: Timer::new(),
            sc_format: wgpu::TextureFormat::Rgba8UnormSrgb,
            fps: Averager::with_capacity(25),
        }
    }
}

impl<'a> GPUApp<'a> {
    fn record_render_pipeline(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let pipeline = self.pipeline.as_ref().unwrap();
        let frustrum: FrustrumG = FrustrumG::from_matrix(self.camera.get_rh_matrix());

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: self.output_texture_view.as_ref().unwrap(),
                    resolve_target: None,
                    load_op: wgpu::LoadOp::Clear,
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color {
                        r: 0.0 as f64,
                        g: 0.0 as f64,
                        b: 0.0 as f64,
                        a: 0.0 as f64,
                    },
                }],
                depth_stencil_attachment: Some(
                    wgpu::RenderPassDepthStencilAttachmentDescriptor {
                        attachment: self.depth_texture_view.as_ref().unwrap(),
                        depth_load_op: wgpu::LoadOp::Clear,
                        depth_store_op: wgpu::StoreOp::Store,
                        clear_depth: 1.0,
                        stencil_load_op: wgpu::LoadOp::Clear,
                        stencil_store_op: wgpu::StoreOp::Clear,
                        clear_stencil: 0,
                    },
                ),
            });
            render_pass.set_bind_group(0, self.bind_group.as_ref().unwrap(), &[]);
            render_pass.set_pipeline(pipeline);

            for i in 0..self.instance_buffers.len() {
                let instance_buffers: &InstanceMatrices = &self.instance_buffers[i];
                if instance_buffers.count <= 0 {
                    continue;
                }

                let instance_bind_group = &self.instance_bind_groups[i];
                let vb: &VertexBuffer = &self.vertex_buffers[i];
                render_pass.set_bind_group(1, instance_bind_group, &[]);
                render_pass.set_vertex_buffer(0, &vb.buffer, 0, 0);
                render_pass.set_vertex_buffer(1, &vb.buffer, 0, 0);
                render_pass.set_vertex_buffer(2, &vb.buffer, 0, 0);
                render_pass.set_vertex_buffer(3, &vb.buffer, 0, 0);

                for i in 0..instance_buffers.count {
                    let bounds = vb.bounds.transformed(instance_buffers.actual_matrices[i]);
                    if frustrum.aabb_in_frustrum(&bounds) != FrustrumResult::Outside {
                        let i = i as u32;
                        render_pass.draw(0..(vb.count as u32), i..(i + 1));
                    }
                }
            }
        }
    }
}

impl<'a> DeviceFramebuffer for GPUApp<'a> {
    fn init(
        &mut self,
        width: u32,
        height: u32,
        device: &wgpu::Device,
        sc_format: wgpu::TextureFormat,
        _requests: &mut VecDeque<Request>,
    ) {
        self.width = width;
        self.height = height;

        use wgpu::*;
        self.sc_format = sc_format;

        if let Ok(scene) = TriangleScene::deserialize("models/dragon.scene") {
            println!("Loaded scene from cached file: models/dragon.scene");
            self.scene = scene;
        } else {
            let (object, scale) = {
                #[cfg(not(debug_assertions))]
                    {
                        (
                            self.scene
                                .load_mesh("models/dragon.obj")
                                .expect("Could not load dragon.obj"),
                            Vec3::splat(5.0),
                        )
                    }

                #[cfg(debug_assertions)]
                    {
                        (
                            self.scene
                                .load_mesh("models/sphere.obj")
                                .expect("Could not load sphere.obj"),
                            Vec3::splat(0.05),
                        )
                    }
            };

            let _object = self
                .scene
                .add_instance(
                    object,
                    Mat4::from_translation(Vec3::new(0.0, 0.0, 5.0)) * Mat4::from_scale(scale),
                )
                .unwrap();
            let _object = self
                .scene
                .add_instance(
                    object,
                    Mat4::from_translation(Vec3::new(5.0, 0.0, 5.0)) * Mat4::from_scale(scale),
                )
                .unwrap();
            let _object = self
                .scene
                .add_instance(
                    object,
                    Mat4::from_translation(Vec3::new(-5.0, 0.0, 5.0)) * Mat4::from_scale(scale),
                )
                .unwrap();

            self.scene.serialize("models/dragon.scene").unwrap();
        }

        let vert_shader = include_str!("shaders/mesh.vert");
        let frag_shader = include_str!("shaders/mesh.frag");

        let vert_shader = self
            .compiler
            .compile_from_string(vert_shader, ShaderKind::Vertex)
            .unwrap();
        let frag_shader = self
            .compiler
            .compile_from_string(frag_shader, ShaderKind::Fragment)
            .unwrap();

        let vert_module = device.create_shader_module(vert_shader.as_slice());
        let frag_module = device.create_shader_module(frag_shader.as_slice());

        self.triangle_bind_group_layout = Some(self.scene.create_bind_group_layout(device));
        self.bind_group_layout =
            Some(device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                bindings: &[
                    BindGroupLayoutEntry {
                        // Matrix buffer
                        binding: 0,
                        visibility: ShaderStage::VERTEX,
                        ty: BindingType::UniformBuffer { dynamic: false },
                    },
                    BindGroupLayoutEntry {
                        // Material buffer
                        binding: 1,
                        visibility: ShaderStage::FRAGMENT,
                        ty: BindingType::StorageBuffer {
                            readonly: true,
                            dynamic: false,
                        },
                    },
                ],
                label: Some("uniform-layout"),
            }));

        self.output_texture = Some(device.create_texture(&TextureDescriptor {
            label: Some("output-texture"),
            size: Extent3d {
                width: self.width,
                height: self.height,
                depth: 1,
            },
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: sc_format,
            usage: TextureUsage::OUTPUT_ATTACHMENT | TextureUsage::STORAGE | TextureUsage::SAMPLED,
        }));

        self.output_texture_view = Some(self.output_texture.as_ref().unwrap().create_default_view());

        self.depth_texture = Some(device.create_texture(&TextureDescriptor {
            label: Some("depth-texture"),
            size: Extent3d {
                width: self.width,
                height: self.height,
                depth: 1,
            },
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsage::OUTPUT_ATTACHMENT,
        }));
        self.depth_texture_view = Some(self.depth_texture.as_ref().unwrap().create_view(
            &TextureViewDescriptor {
                format: TextureFormat::Depth32Float,
                dimension: TextureViewDimension::D2,
                aspect: TextureAspect::DepthOnly,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                array_layer_count: 1,
            },
        ));

        self.pipeline_layout = Some(device.create_pipeline_layout(&PipelineLayoutDescriptor {
            bind_group_layouts: &[
                self.bind_group_layout.as_ref().unwrap(),
                self.triangle_bind_group_layout.as_ref().unwrap(),
            ],
        }));

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            layout: self.pipeline_layout.as_ref().unwrap(),
            vertex_stage: ProgrammableStageDescriptor {
                module: &vert_module,
                entry_point: "main",
            },
            fragment_stage: Some(ProgrammableStageDescriptor {
                module: &frag_module,
                entry_point: "main",
            }),
            rasterization_state: Some(RasterizationStateDescriptor {
                front_face: FrontFace::Ccw,
                cull_mode: CullMode::Back,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            }),
            primitive_topology: PrimitiveTopology::TriangleList,
            color_states: &[ColorStateDescriptor {
                format: sc_format,
                alpha_blend: BlendDescriptor::REPLACE,
                color_blend: BlendDescriptor::REPLACE,
                write_mask: ColorWrite::ALL,
            }],
            depth_stencil_state: Some(DepthStencilStateDescriptor {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: CompareFunction::LessEqual,
                stencil_front: StencilStateFaceDescriptor::IGNORE,
                stencil_back: StencilStateFaceDescriptor::IGNORE,
                stencil_read_mask: 0,
                stencil_write_mask: 0,
            }),
            vertex_state: VertexStateDescriptor {
                vertex_buffers: &[
                    VertexBufferDescriptor {
                        stride: std::mem::size_of::<VertexData>() as BufferAddress,
                        step_mode: InputStepMode::Vertex,
                        attributes: &[VertexAttributeDescriptor {
                            offset: 0,
                            format: VertexFormat::Float4,
                            shader_location: 0,
                        }],
                    },
                    VertexBufferDescriptor {
                        stride: std::mem::size_of::<VertexData>() as BufferAddress,
                        step_mode: InputStepMode::Vertex,
                        attributes: &[VertexAttributeDescriptor {
                            offset: 16,
                            format: VertexFormat::Float3,
                            shader_location: 1,
                        }],
                    },
                    VertexBufferDescriptor {
                        stride: std::mem::size_of::<VertexData>() as BufferAddress,
                        step_mode: InputStepMode::Vertex,
                        attributes: &[VertexAttributeDescriptor {
                            offset: 28,
                            format: VertexFormat::Uint,
                            shader_location: 2,
                        }],
                    },
                    VertexBufferDescriptor {
                        stride: std::mem::size_of::<VertexData>() as BufferAddress,
                        step_mode: InputStepMode::Vertex,
                        attributes: &[VertexAttributeDescriptor {
                            offset: 32,
                            format: VertexFormat::Float2,
                            shader_location: 3,
                        }],
                    },
                ],
                index_format: IndexFormat::Uint32,
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        let uniform_buffer = device.create_buffer_mapped(&BufferDescriptor {
            label: Some("vp-uniform"),
            size: 64,
            usage: BufferUsage::UNIFORM | BufferUsage::COPY_DST,
        });

        let matrix = self.camera.get_rh_matrix();

        let staging_buffer = device.create_buffer_mapped(&BufferDescriptor {
            label: Some("staging-buffer"),
            size: 64 as BufferAddress,
            usage: BufferUsage::COPY_SRC | BufferUsage::MAP_WRITE,
        });

        staging_buffer.data.copy_from_slice(unsafe {
            std::slice::from_raw_parts(matrix.as_ref().as_ptr() as *const u8, 64)
        });

        self.staging_buffer = Some(staging_buffer.finish());

        uniform_buffer.data.copy_from_slice(unsafe {
            std::slice::from_raw_parts(matrix.as_ref().as_ptr() as *const u8, 64)
        });

        self.uniform_buffer = Some(uniform_buffer.finish());
        self.material_buffer = Some(self.scene.get_material_list().create_wgpu_buffer(device));

        self.vertex_buffers = self.scene.create_vertex_buffers(device);
        self.instance_buffers = self.scene.create_wgpu_instances_buffer(device);
        self.instance_bind_groups = self.scene.create_bind_groups(
            device,
            self.triangle_bind_group_layout.as_ref().unwrap(),
            &self.instance_buffers,
        );

        let (size, mat_buffer) = self.material_buffer.as_ref().unwrap();

        self.bind_group = Some(device.create_bind_group(&BindGroupDescriptor {
            layout: self.bind_group_layout.as_ref().unwrap(),
            bindings: &[
                Binding {
                    binding: 0,
                    resource: BindingResource::Buffer {
                        buffer: self.uniform_buffer.as_ref().unwrap(),
                        range: 0..64,
                    },
                },
                Binding {
                    binding: 1,
                    resource: BindingResource::Buffer {
                        buffer: mat_buffer,
                        range: 0..(*size),
                    },
                },
            ],
            label: Some("mesh-bind-group-descriptor"),
        }));

        self.pipeline = Some(render_pipeline);

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("blit-layout"),
            bindings: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::SampledTexture {
                    multisampled: false,
                    component_type: wgpu::TextureComponentType::Uint,
                    dimension: wgpu::TextureViewDimension::D2,
                },
            }, wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::Sampler { comparison: false },
            }],
        });

        self.output_sampler = Some(device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 1.0,
            compare: wgpu::CompareFunction::Never,
        }));

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("blit-bind-group"),
            bindings: &[Binding {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(self.output_texture_view.as_ref().unwrap()),
            }, Binding {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(self.output_sampler.as_ref().unwrap()),
            }, ],
            layout: &bind_group_layout,
        });

        let blit_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            bind_group_layouts: &[&bind_group_layout]
        });

        let vert_shader = include_str!("shaders/quad.vert");
        let frag_shader = include_str!("shaders/quad.frag");

        let vert_module = self.compiler.compile_from_string(vert_shader, ShaderKind::Vertex).unwrap();
        let frag_module = self.compiler.compile_from_string(frag_shader, ShaderKind::Fragment).unwrap();

        let vert_module = device.create_shader_module(vert_module.as_slice());
        let frag_module = device.create_shader_module(frag_module.as_slice());

        self.blit_pipeline = Some(device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
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
        }));

        self.blit_bind_group_layout = Some(bind_group_layout);
        self.blit_bind_group = Some(bind_group);
    }

    fn render(
        &mut self,
        fb: &wgpu::SwapChainOutput,
        device: &wgpu::Device,
        requests: &mut VecDeque<Request>,
    ) {
        use wgpu::*;
        self.camera.far_plane = 1e2;

        let mapping = self.staging_buffer.as_ref().unwrap().map_write(0, 64);
        let matrix = self.camera.get_rh_matrix();

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("render")
        });

        let staging_buffer = self.staging_buffer.as_ref().unwrap();
        let uniform_buffer = self.uniform_buffer.as_ref().unwrap();

        encoder.copy_buffer_to_buffer(staging_buffer, 0, uniform_buffer, 0, 64);

        device.poll(wgpu::Maintain::Wait);

        if let Ok(mut mapping) = block_on(mapping) {
            let slice = mapping.as_slice();
            slice.copy_from_slice(unsafe {
                std::slice::from_raw_parts(matrix.as_ref().as_ptr() as *const u8, 64)
            });
        }

        if self.instance_buffers.is_empty() {
            requests.push_back(Request::CommandBuffer(encoder.finish()));
            return;
        }

        self.record_render_pipeline(&mut encoder);

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                color_attachments: &[RenderPassColorAttachmentDescriptor {
                    attachment: &fb.view,
                    clear_color: Color::BLACK,
                    load_op: LoadOp::Clear,
                    store_op: StoreOp::Store,
                    resolve_target: None,
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(self.blit_pipeline.as_ref().unwrap());
            render_pass.set_bind_group(0, self.blit_bind_group.as_ref().unwrap(), &[]);
            render_pass.draw(0..6, 0..1);
        }

        requests.push_back(Request::CommandBuffer(encoder.finish()));
    }

    fn mouse_button_handling(
        &mut self,
        _states: &MouseButtonHandler,
        _requests: &mut VecDeque<Request>,
    ) {}

    fn key_handling(&mut self, states: &KeyHandler, requests: &mut VecDeque<Request>) {
        #[cfg(target_os = "macos")]
            {
                if states.pressed(KeyCode::LWin) && states.pressed(KeyCode::Q) {
                    requests.push_back(Request::Exit);
                    return;
                }
            }

        #[cfg(any(target_os = "linux", target_os = "windows"))]
            {
                if states.pressed(KeyCode::LAlt) && states.pressed(KeyCode::F4) {
                    requests.push_back(Request::Exit);
                    return;
                }
            }

        if states.pressed(KeyCode::Escape) {
            requests.push_back(Request::Exit);
            return;
        }

        let mut view_change = Vec3::new(0.0, 0.0, 0.0);
        let mut pos_change = Vec3::new(0.0, 0.0, 0.0);

        if states.pressed(KeyCode::Up) {
            view_change += (0.0, 1.0, 0.0).into();
        }
        if states.pressed(KeyCode::Down) {
            view_change -= (0.0, 1.0, 0.0).into();
        }
        if states.pressed(KeyCode::Left) {
            view_change -= (1.0, 0.0, 0.0).into();
        }
        if states.pressed(KeyCode::Right) {
            view_change += (1.0, 0.0, 0.0).into();
        }

        if states.pressed(KeyCode::W) {
            pos_change += (0.0, 0.0, 1.0).into();
        }
        if states.pressed(KeyCode::S) {
            pos_change -= (0.0, 0.0, 1.0).into();
        }
        if states.pressed(KeyCode::A) {
            pos_change -= (1.0, 0.0, 0.0).into();
        }
        if states.pressed(KeyCode::D) {
            pos_change += (1.0, 0.0, 0.0).into();
        }
        if states.pressed(KeyCode::E) {
            pos_change += (0.0, 1.0, 0.0).into();
        }
        if states.pressed(KeyCode::Q) {
            pos_change -= (0.0, 1.0, 0.0).into();
        }

        let elapsed = self.timer.elapsed_in_millis();
        let elapsed = if states.pressed(KeyCode::LShift) {
            elapsed * 2.0
        } else {
            elapsed
        };

        let view_change = view_change * elapsed * 0.001;
        let pos_change = pos_change * elapsed * 0.01;

        if view_change != [0.0; 3].into() {
            self.camera.translate_target(view_change);
        }
        if pos_change != [0.0; 3].into() {
            self.camera.translate_relative(pos_change);
        }

        let elapsed = self.timer.elapsed_in_millis();
        self.fps.add_sample(1000.0 / elapsed);
        let avg = self.fps.get_average();
        self.timer.reset();
        requests.push_back(Request::TitleChange(format!("FPS: {:.2}", avg)))
    }

    fn mouse_handling(
        &mut self,
        _x: f64,
        _y: f64,
        _delta_x: f64,
        _delta_y: f64,
        _requests: &mut VecDeque<Request>,
    ) {}

    fn scroll_handling(&mut self, _dx: f64, dy: f64, _requests: &mut VecDeque<Request>) {
        self.camera
            .change_fov(self.camera.get_fov() - (dy as f32) * 0.01);
    }

    fn resize(
        &mut self,
        width: u32,
        height: u32,
        device: &wgpu::Device,
        _requests: &mut VecDeque<Request>,
    ) {
        use wgpu::*;

        self.width = width;
        self.height = height;
        self.camera.resize(width, height);

        let new_texture = device.create_texture(&TextureDescriptor {
            label: Some("output-texture"),
            size: Extent3d {
                width: self.width,
                height: self.height,
                depth: 1,
            },
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: self.sc_format,
            usage: TextureUsage::OUTPUT_ATTACHMENT | TextureUsage::STORAGE | TextureUsage::SAMPLED,
        });

        self.output_texture = Some(new_texture);
        let new_texture_view = self.output_texture.as_ref().unwrap().create_default_view();
        self.output_texture_view = Some(new_texture_view);

        let new_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth-texture"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth: 1,
            },
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
        });

        let new_view = new_texture.create_view(&TextureViewDescriptor {
            format: TextureFormat::Depth32Float,
            dimension: TextureViewDimension::D2,
            aspect: TextureAspect::DepthOnly,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            array_layer_count: 1,
        });
        self.depth_texture = Some(new_texture);
        self.depth_texture_view = Some(new_view);
    }

    fn imgui(&mut self, _ui: &Ui) {}
}
