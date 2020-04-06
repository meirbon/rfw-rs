use fb_template::{
    shader::*, shader::*, DeviceFramebuffer, KeyCode, KeyHandler, MouseButtonCode,
    MouseButtonHandler, Request, Ui,
};
use glam::*;
use rayon::prelude::*;
use scene::{
    constants::{DEFAULT_T_MAX, DEFAULT_T_MIN},
    material::MaterialList,
    BVHMode, Obj, Plane, Scene, Sphere, ToMesh,
};

pub struct GPUApp<'a> {
    width: u32,
    height: u32,
    compiler: Compiler<'a>,
    pipeline: Option<wgpu::RenderPipeline>,
}

impl<'a> GPUApp<'a> {
    pub fn new() -> Self {
        let compiler = CompilerBuilder::new()
            .with_opt_level(OptimizationLevel::Performance)
            .build();

        Self {
            width: 1,
            height: 1,
            compiler,
            pipeline: None,
        }
    }
}

impl<'a> DeviceFramebuffer for GPUApp<'a> {
    fn init(&mut self, width: u32, height: u32, device: &wgpu::Device) -> Option<Request> {
        self.resize(width, height, &device);

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[],
        });

        let vert_shader = include_str!("shaders/quad.vert");
        let frag_shader = include_str!("shaders/quad.frag");

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

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &pipeline_layout,
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
            index_format: wgpu::IndexFormat::Uint16,
            vertex_buffers: &[],
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        self.pipeline = Some(pipeline);

        None
    }

    fn render(&mut self, fb: &wgpu::SwapChainOutput, device: &wgpu::Device) -> Option<Request> {
        if let Some(pipeline) = &self.pipeline {
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                        attachment: &fb.view,
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
                    depth_stencil_attachment: None,
                });

                render_pass.set_pipeline(pipeline);
                render_pass.draw(0..6, 0..1);
            }

            return Some(Request::CommandBuffer(encoder.finish()));
        }

        None
    }

    fn mouse_button_handling(&mut self, _states: &MouseButtonHandler) -> Option<Request> {
        None
    }

    fn key_handling(&mut self, _states: &KeyHandler) -> Option<Request> {
        None
    }

    fn mouse_handling(
        &mut self,
        _x: f64,
        _y: f64,
        _delta_x: f64,
        _delta_y: f64,
    ) -> Option<Request> {
        None
    }

    fn scroll_handling(&mut self, _dx: f64, _dy: f64) -> Option<Request> {
        None
    }

    fn resize(&mut self, _width: u32, _height: u32, _device: &wgpu::Device) -> Option<Request> {
        None
    }

    fn imgui(&mut self, _ui: &Ui) {}
}
