pub mod shader;

use glam::*;
use std::collections::HashMap;
use std::time::Instant;

use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

pub use imgui::*;
pub use winit::event::VirtualKeyCode as KeyCode;

pub struct KeyHandler {
    states: HashMap<VirtualKeyCode, bool>,
}

impl KeyHandler {
    pub fn new() -> KeyHandler {
        KeyHandler {
            states: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: KeyCode, state: ElementState) {
        self.states.insert(
            key,
            match state {
                ElementState::Pressed => true,
                _ => false,
            },
        );
    }

    pub fn pressed(&self, key: KeyCode) -> bool {
        if let Some(state) = self.states.get(&key) {
            return *state;
        }
        false
    }
}

pub enum Request {
    Exit,
    TitleChange(String),
}

pub trait HostFramebuffer {
    fn render(&mut self, fb: &mut [u8]) -> Option<Request>;
    fn key_handling(&mut self, states: &KeyHandler) -> Option<Request>;
    fn mouse_handling(&mut self, x: f64, y: f64, delta_x: f64, delta_y: f64) -> Option<Request>;
    fn scroll_handling(&mut self, dx: f64, dy: f64) -> Option<Request>;
    fn resize(&mut self, width: u32, height: u32) -> Option<Request>;
    fn imgui(&mut self, ui: &imgui::Ui);
}

pub trait DeviceFramebuffer {
    fn render(&mut self, fb: &wgpu::SwapChainOutput) -> Option<wgpu::CommandBuffer>;
    fn key_handling(&mut self, states: &KeyHandler) -> Option<Request>;
    fn mouse_handling(&mut self, x: f64, y: f64, delta_x: f64, delta_y: f64) -> Option<Request>;
    fn scroll_handling(&mut self, dx: f64, dy: f64) -> Option<Request>;
    fn resize(&mut self, width: u32, height: u32) -> Option<Request>;
    fn imgui(&mut self, ui: &imgui::Ui);
}

pub fn run_host_app<T: 'static + HostFramebuffer>(
    mut app: T,
    title: &str,
    start_width: u32,
    start_height: u32,
    v_sync: bool,
) {
    let mut key_handler = KeyHandler::new();
    let (mut width, mut height) = (start_width, start_height);
    let mut first_mouse_pos = true;

    let mut mouse_x = 0.0;
    let mut mouse_y = 0.0;

    let mut old_mouse_x = 0.0;
    let mut old_mouse_y = 0.0;

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(LogicalSize::new(width as f64, height as f64))
        .build(&event_loop)
        .unwrap();

    let surface = wgpu::Surface::create(&window);
    let adapter = wgpu::Adapter::request(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::Default,
        backends: wgpu::BackendBit::PRIMARY,
    })
    .expect("Could not initialize wgpu adapter");

    let mut compiler = shader::CompilerBuilder::new().build();
    let vert_source = include_str!("../shaders/quad.vert");
    let frag_source = include_str!("../shaders/quad.frag");

    let (mut device, mut queue) = adapter.request_device(&wgpu::DeviceDescriptor {
        extensions: wgpu::Extensions {
            anisotropic_filtering: false,
        },
        limits: wgpu::Limits::default(),
    });

    let vert_module = compiler
        .compile_from_string(vert_source, shaderc::ShaderKind::Vertex)
        .unwrap();
    let frag_module = compiler
        .compile_from_string(frag_source, shaderc::ShaderKind::Fragment)
        .unwrap();

    let vert_module = device.create_shader_module(vert_module.as_slice());
    let frag_module = device.create_shader_module(frag_module.as_slice());

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        bindings: &[
            wgpu::BindGroupLayoutBinding {
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
            },
            wgpu::BindGroupLayoutBinding {
                binding: 1,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::SampledTexture {
                    multisampled: false,
                    dimension: wgpu::TextureViewDimension::D2,
                },
            },
            wgpu::BindGroupLayoutBinding {
                binding: 2,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::Sampler,
            },
        ],
    });

    let uniform_buffer = device
        .create_buffer_mapped::<Mat4>(1, wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::UNIFORM);
    let uniform_buffer = uniform_buffer.fill_from_slice(&[Mat4::identity()]);

    // Create a texture sampler with nearest neighbor
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        lod_min_clamp: 0.0,
        lod_max_clamp: 1.0,
        compare_function: wgpu::CompareFunction::Always,
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        bind_group_layouts: &[&bind_group_layout],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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

    let mut sc_descriptor = wgpu::SwapChainDescriptor {
        usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        width,
        height,
        present_mode: if v_sync {
            wgpu::PresentMode::Vsync
        } else {
            wgpu::PresentMode::NoVsync
        },
    };

    let mut tex_descriptor = wgpu::TextureDescriptor {
        size: wgpu::Extent3d {
            width,
            height,
            depth: 1,
        },
        array_layer_count: 1,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsage::COPY_DST | wgpu::TextureUsage::SAMPLED,
    };

    let mut swap_chain = device.create_swap_chain(&surface, &sc_descriptor);
    let mut pixels: Vec<u8> = vec![0; (width * height * 4) as usize];
    let mut render_texture = device.create_texture(&tex_descriptor);
    let mut render_texture_view = render_texture.create_default_view();

    let hidpi_factor = window.scale_factor();
    let mut imgui = imgui::Context::create();
    let font_size = (13.0 * hidpi_factor) as f32;
    imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

    imgui.fonts().add_font(&[FontSource::DefaultFontData {
        config: Some(imgui::FontConfig {
            oversample_h: 1,
            pixel_snap_h: true,
            size_pixels: font_size,
            ..Default::default()
        }),
    }]);

    let mut platform = imgui_winit_support::WinitPlatform::init(&mut imgui);

    platform.attach_window(
        imgui.io_mut(),
        &window,
        imgui_winit_support::HiDpiMode::Default,
    );

    let mut renderer =
        imgui_wgpu::Renderer::new(&mut imgui, &device, &mut queue, sc_descriptor.format, None);

    let mut last_frame = Instant::now();
    let mut last_cursor = None;

    let mut resized = false;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::MainEventsCleared => window.request_redraw(),
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                window_id,
            } if window_id == window.id() => {
                if let Some(key) = input.virtual_keycode {
                    key_handler.insert(key, input.state);
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,
            Event::RedrawRequested(_) => {
                let pixel_count = (width * height * 4) as usize;

                if resized {
                    sc_descriptor.width = width;
                    sc_descriptor.height = height;
                    app.resize(width, height);

                    if pixels.len() < pixel_count {
                        pixels.resize((pixel_count as f64 * 1.2) as usize, 0);
                    }
                    swap_chain = device.create_swap_chain(&surface, &sc_descriptor);

                    tex_descriptor.size = wgpu::Extent3d {
                        width,
                        height,
                        depth: 1,
                    };
                    render_texture = device.create_texture(&tex_descriptor);
                    render_texture_view = render_texture.create_default_view();
                    resized = false;
                }

                if let Some(request) = app.key_handling(&key_handler) {
                    match request {
                        Request::Exit => *control_flow = ControlFlow::Exit,
                        Request::TitleChange(title) => window.set_title(title.as_str()),
                    };
                }

                app.render(&mut pixels[0..((width * height * 4) as usize)]);

                let render_buffer = device
                    .create_buffer_mapped::<u8>(pixel_count as usize, wgpu::BufferUsage::COPY_SRC)
                    .fill_from_slice(&pixels[0..pixel_count]);
                let output_texture = swap_chain.get_next_texture();

                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &bind_group_layout,
                    bindings: &[
                        wgpu::Binding {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer {
                                buffer: &uniform_buffer,
                                range: 0..64,
                            },
                        },
                        wgpu::Binding {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&render_texture_view),
                        },
                        wgpu::Binding {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                });

                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                encoder.copy_buffer_to_texture(
                    wgpu::BufferCopyView {
                        buffer: &render_buffer,
                        offset: 0 as wgpu::BufferAddress,
                        row_pitch: width * 4,
                        image_height: height,
                    },
                    wgpu::TextureCopyView {
                        texture: &render_texture,
                        mip_level: 0,
                        array_layer: 0,
                        origin: wgpu::Origin3d::default(),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth: 1,
                    },
                );

                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                            attachment: &output_texture.view,
                            resolve_target: None,
                            load_op: wgpu::LoadOp::Clear,
                            store_op: wgpu::StoreOp::Store,
                            clear_color: wgpu::Color::BLACK,
                        }],
                        depth_stencil_attachment: None,
                    });

                    render_pass.set_pipeline(&render_pipeline);
                    render_pass.set_bind_group(0, &bind_group, &[]);
                    render_pass.draw(0..6, 0..1);
                }

                last_frame = imgui.io_mut().update_delta_time(last_frame);

                platform
                    .prepare_frame(imgui.io_mut(), &window)
                    .expect("Failed to prepare ImGui frame.");
                let ui = imgui.frame();

                app.imgui(&ui);

                if last_cursor != Some(ui.mouse_cursor()) {
                    last_cursor = Some(ui.mouse_cursor());
                    platform.prepare_render(&ui, &window);
                }

                renderer
                    .render(ui.render(), &mut device, &mut encoder, &output_texture.view)
                    .expect("ImGui render failed.");

                queue.submit(&[encoder.finish()]);
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                window_id,
            } if window_id == window.id() => {
                let size = window.inner_size();

                width = size.width;
                height = size.height;

                width = ((width * 4 + 256 - 1) / 256) * 256 / 4;
                height = ((height * 4 + 256 - 1) / 256) * 256 / 4;

                resized = true;
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                window_id,
            } if window_id == window.id() => {
                if first_mouse_pos {
                    mouse_x = position.x;
                    mouse_y = position.y;
                    old_mouse_x = position.x;
                    old_mouse_y = position.y;
                    first_mouse_pos = false;
                } else {
                    old_mouse_x = mouse_x;
                    old_mouse_y = mouse_y;

                    mouse_x = position.x;
                    mouse_y = position.y;
                }

                let delta_x = mouse_x - old_mouse_x;
                let delta_y = mouse_y - old_mouse_y;

                app.mouse_handling(mouse_x, mouse_y, delta_x, delta_y);
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseWheel {
                        delta: winit::event::MouseScrollDelta::LineDelta(x, y),
                        ..
                    },
                window_id,
            } if window_id == window.id() => {
                app.scroll_handling(x as f64, y as f64);
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseWheel {
                        delta: winit::event::MouseScrollDelta::PixelDelta(delta),
                        ..
                    },
                window_id,
            } if window_id == window.id() => {
                app.scroll_handling(delta.x, delta.y);
            }
            _ => (),
        }

        platform.handle_event(imgui.io_mut(), &window, &event);
    });
}
