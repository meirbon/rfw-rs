pub mod shader;

use glam::*;
use std::collections::{HashMap, VecDeque};

use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use futures::executor::block_on;
pub use imgui::*;
use wgpu::BufferAddress;
pub use winit::event::MouseButton as MouseButtonCode;
pub use winit::event::VirtualKeyCode as KeyCode;

pub struct KeyHandler {
    states: HashMap<VirtualKeyCode, bool>,
}

impl KeyHandler {
    pub fn new() -> KeyHandler {
        Self {
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

pub struct MouseButtonHandler {
    states: HashMap<MouseButtonCode, bool>,
}

impl MouseButtonHandler {
    pub fn new() -> MouseButtonHandler {
        Self {
            states: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: MouseButtonCode, state: ElementState) {
        self.states.insert(
            key,
            match state {
                ElementState::Pressed => true,
                _ => false,
            },
        );
    }

    pub fn pressed(&self, key: MouseButtonCode) -> bool {
        if let Some(state) = self.states.get(&key) {
            return *state;
        }
        false
    }
}

pub enum Request {
    Exit,
    TitleChange(String),
    CommandBuffer(wgpu::CommandBuffer),
}

pub trait HostFramebuffer {
    fn init(&mut self, width: u32, height: u32) -> Option<Request>;
    fn render(&mut self, fb: &mut [u8]) -> Option<Request>;
    fn key_handling(&mut self, states: &KeyHandler) -> Option<Request>;
    fn mouse_button_handling(&mut self, states: &MouseButtonHandler) -> Option<Request>;
    fn mouse_handling(&mut self, x: f64, y: f64, delta_x: f64, delta_y: f64) -> Option<Request>;
    fn scroll_handling(&mut self, dx: f64, dy: f64) -> Option<Request>;
    fn resize(&mut self, width: u32, height: u32) -> Option<Request>;
    fn imgui(&mut self, ui: &imgui::Ui);
}

pub trait DeviceFramebuffer {
    /// This function is ran once.
    /// Take the device reference to do more in your code.
    fn init(
        &mut self,
        width: u32,
        height: u32,
        device: &wgpu::Device,
        queue: &mut wgpu::Queue,
        sc_format: wgpu::TextureFormat,
        requests: &mut VecDeque<Request>,
    );
    fn render(
        &mut self,
        fb: &wgpu::SwapChainOutput,
        device: &wgpu::Device,
        requests: &mut VecDeque<Request>,
    );
    fn mouse_button_handling(
        &mut self,
        states: &MouseButtonHandler,
        requests: &mut VecDeque<Request>,
    );
    fn key_handling(&mut self, states: &KeyHandler, requests: &mut VecDeque<Request>);
    fn mouse_handling(
        &mut self,
        x: f64,
        y: f64,
        delta_x: f64,
        delta_y: f64,
        requests: &mut VecDeque<Request>,
    );
    fn scroll_handling(&mut self, dx: f64, dy: f64, requests: &mut VecDeque<Request>);
    fn resize(
        &mut self,
        width: u32,
        height: u32,
        device: &wgpu::Device,
        requests: &mut VecDeque<Request>,
    );
    fn imgui(&mut self, ui: &imgui::Ui);
}

pub fn run_device_app<T: 'static + DeviceFramebuffer>(
    mut app: T,
    title: &str,
    start_width: u32,
    start_height: u32,
) {
    let mut key_handler = KeyHandler::new();
    let mut mouse_button_handler = MouseButtonHandler::new();
    let mut first_mouse_pos = true;

    let mut mouse_x = 0.0;
    let mut mouse_y = 0.0;

    let mut old_mouse_x = 0.0;
    let mut old_mouse_y = 0.0;

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(LogicalSize::new(start_width as f64, start_height as f64))
        .build(&event_loop)
        .unwrap();

    let surface = wgpu::Surface::create(&window);

    let adapter: wgpu::Adapter = block_on(wgpu::Adapter::request(
        &wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
        },
        wgpu::BackendBit::PRIMARY,
    ))
    .unwrap();

    let (device, mut queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        extensions: wgpu::Extensions {
            anisotropic_filtering: false,
        },
        limits: wgpu::Limits::default(),
    }));

    let mut requests: VecDeque<Request> = VecDeque::new();
    let mut command_buffers: Vec<wgpu::CommandBuffer> = Vec::new();

    let mut sc_descriptor = wgpu::SwapChainDescriptor {
        usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        width: start_width,
        height: start_height,
        present_mode: wgpu::PresentMode::Mailbox,
    };

    app.init(
        sc_descriptor.width,
        sc_descriptor.height,
        &device,
        &mut queue,
        sc_descriptor.format,
        &mut requests,
    );

    let mut swap_chain = device.create_swap_chain(&surface, &sc_descriptor);

    // let hidpi_factor = window.scale_factor();
    // let mut imgui = imgui::Context::create();
    // let font_size = (13.0 * hidpi_factor) as f32;
    // imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

    // imgui.fonts().add_font(&[FontSource::DefaultFontData {
    //     config: Some(imgui::FontConfig {
    //         oversample_h: 1,
    //         pixel_snap_h: true,
    //         size_pixels: font_size,
    //         ..Default::default()
    //     }),
    // }]);

    // let mut platform = imgui_winit_support::WinitPlatform::init(&mut imgui);

    // platform.attach_window(
    //     imgui.io_mut(),
    //     &window,
    //     imgui_winit_support::HiDpiMode::Default,
    // );

    // let mut renderer =
    //     imgui_wgpu::Renderer::new(&mut imgui, &device, &mut queue, sc_descriptor.format, None);

    // let mut last_frame = Instant::now();
    // let mut last_cursor = None;

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
                if resized {
                    swap_chain = device.create_swap_chain(&surface, &sc_descriptor);
                    app.resize(
                        sc_descriptor.width,
                        sc_descriptor.height,
                        &device,
                        &mut requests,
                    );
                    resized = false;
                }

                app.key_handling(&key_handler, &mut requests);

                app.mouse_button_handling(&mouse_button_handler, &mut requests);

                let output_texture = swap_chain.get_next_texture().unwrap();

                // last_frame = imgui.io_mut().update_delta_time(last_frame);

                // platform
                //     .prepare_frame(imgui.io_mut(), &window)
                //     .expect("Failed to prepare ImGui frame.");
                // let ui = imgui.frame();

                // app.imgui(&ui);

                // if last_cursor != Some(ui.mouse_cursor()) {
                //     last_cursor = Some(ui.mouse_cursor());
                //     platform.prepare_render(&ui, &window);
                // }

                // let mut encoder =
                //     device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                //         label: Some("")
                //     });

                // renderer
                //     .render(ui.render(), &mut device, &mut encoder, &output_texture.view)
                //     .expect("ImGui render failed.");
                // command_buffers.push(encoder.finish());

                app.render(&output_texture, &device, &mut requests);

                loop {
                    let request = requests.pop_front();
                    match request {
                        Some(request) => match request {
                            Request::Exit => *control_flow = ControlFlow::Exit,
                            Request::TitleChange(title) => window.set_title(title.as_str()),
                            Request::CommandBuffer(command_buffer) => {
                                command_buffers.push(command_buffer)
                            }
                        },
                        _ => break,
                    }
                }

                if !command_buffers.is_empty() {
                    queue.submit(command_buffers.as_slice());
                    command_buffers.clear();
                }
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                window_id,
            } if window_id == window.id() => {
                sc_descriptor.width = size.width;
                sc_descriptor.height = size.height;

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

                app.mouse_handling(mouse_x, mouse_y, delta_x, delta_y, &mut requests);
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseWheel {
                        delta: winit::event::MouseScrollDelta::LineDelta(x, y),
                        ..
                    },
                window_id,
            } if window_id == window.id() => {
                app.scroll_handling(x as f64, y as f64, &mut requests);
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseWheel {
                        delta: winit::event::MouseScrollDelta::PixelDelta(delta),
                        ..
                    },
                window_id,
            } if window_id == window.id() => {
                app.scroll_handling(delta.x, delta.y, &mut requests);
            }
            Event::WindowEvent {
                event: WindowEvent::MouseInput { state, button, .. },
                window_id,
            } if window_id == window.id() => {
                mouse_button_handler.insert(button, state);
            }
            _ => (),
        }
    });
}

pub fn run_host_app<T: 'static + HostFramebuffer>(
    mut app: T,
    title: &str,
    start_width: u32,
    start_height: u32,
) {
    let mut key_handler = KeyHandler::new();
    let mut mouse_button_handler = MouseButtonHandler::new();
    let mut first_mouse_pos = true;

    let mut mouse_x = 0.0;
    let mut mouse_y = 0.0;

    let mut old_mouse_x = 0.0;
    let mut old_mouse_y = 0.0;

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(LogicalSize::new(start_width as f64, start_height as f64))
        .build(&event_loop)
        .unwrap();

    let (mut width, mut height) = window.inner_size().into();

    let surface = wgpu::Surface::create(&window);
    let adapter: wgpu::Adapter = block_on(wgpu::Adapter::request(
        &wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::Default,
            compatible_surface: Some(&surface),
        },
        wgpu::BackendBit::PRIMARY,
    ))
    .unwrap();

    app.init(width, height);

    let mut compiler = shader::CompilerBuilder::new().build();
    let vert_source = include_str!("../shaders/quad.vert");
    let frag_source = include_str!("../shaders/quad.frag");

    let device_descriptor = wgpu::DeviceDescriptor {
        extensions: wgpu::Extensions {
            anisotropic_filtering: false,
        },
        limits: wgpu::Limits::default(),
    };

    let adapter_request = adapter.request_device(&device_descriptor);

    let vert_module = compiler
        .compile_from_string(vert_source, shaderc::ShaderKind::Vertex)
        .unwrap();
    let frag_module = compiler
        .compile_from_string(frag_source, shaderc::ShaderKind::Fragment)
        .unwrap();

    let (device, queue) = block_on(adapter_request);

    let vert_module = device.create_shader_module(vert_module.as_slice());
    let frag_module = device.create_shader_module(frag_module.as_slice());

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        bindings: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::SampledTexture {
                    multisampled: false,
                    component_type: wgpu::TextureComponentType::Uint,
                    dimension: wgpu::TextureViewDimension::D2,
                },
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::Sampler { comparison: false },
            },
        ],
        label: Some("blit-bind-group"),
    });

    let uniform_buffer = device.create_buffer_mapped(&wgpu::BufferDescriptor {
        label: Some("uniform-buffer"),
        /// The size of the buffer (in bytes).
        size: std::mem::size_of::<Mat4>() as BufferAddress,
        /// All possible ways the buffer can be used.
        usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::UNIFORM,
    });

    let matrix = Mat4::from_scale((1.0, -1.0, 1.0).into());

    uniform_buffer.data.copy_from_slice(unsafe {
        std::slice::from_raw_parts(
            matrix.as_ref().as_ptr() as *const u8,
            std::mem::size_of::<Mat4>(),
        )
    });
    let uniform_buffer = uniform_buffer.finish();

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
        compare: wgpu::CompareFunction::Never,
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
        vertex_state: wgpu::VertexStateDescriptor {
            index_format: wgpu::IndexFormat::Uint32,
            vertex_buffers: &[],
        },
        sample_count: 1,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
    });

    let mut sc_descriptor = wgpu::SwapChainDescriptor {
        usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        width,
        height,
        present_mode: wgpu::PresentMode::Mailbox,
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
        label: Some("render-texture"),
    };

    let mut swap_chain = device.create_swap_chain(&surface, &sc_descriptor);
    let mut pixels: Vec<u8> = vec![0; (width * height * 4) as usize];
    let mut render_texture = device.create_texture(&tex_descriptor);
    let mut render_texture_view = render_texture.create_default_view();

    // let hidpi_factor = window.scale_factor();
    // let mut imgui = imgui::Context::create();
    // let font_size = (13.0 * hidpi_factor) as f32;
    // imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

    // imgui.fonts().add_font(&[FontSource::DefaultFontData {
    //     config: Some(imgui::FontConfig {
    //         oversample_h: 1,
    //         pixel_snap_h: true,
    //         size_pixels: font_size,
    //         ..Default::default()
    //     }),
    // }]);

    // let mut platform = imgui_winit_support::WinitPlatform::init(&mut imgui);

    // platform.attach_window(
    //     imgui.io_mut(),
    //     &window,
    //     imgui_winit_support::HiDpiMode::Default,
    // );

    // let mut renderer =
    // imgui_wgpu::Renderer::new(&mut imgui, &device, &mut queue, wgpu::TextureFormat::, None);

    // let mut last_frame = Instant::now();
    // let mut last_cursor = None;

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
                    swap_chain = device.create_swap_chain(&surface, &sc_descriptor);

                    if pixels.len() < pixel_count {
                        pixels.resize((pixel_count as f64 * 1.2) as usize, 0);
                    }

                    app.resize(width, height);

                    tex_descriptor.size = wgpu::Extent3d {
                        width,
                        height,
                        depth: 1,
                    };
                    let new_texture = device.create_texture(&tex_descriptor);
                    let new_view = new_texture.create_default_view();

                    render_texture = new_texture;
                    render_texture_view = new_view;
                    resized = false;
                }

                if let Some(request) = app.key_handling(&key_handler) {
                    match request {
                        Request::Exit => *control_flow = ControlFlow::Exit,
                        Request::TitleChange(title) => window.set_title(title.as_str()),
                        _ => (),
                    };
                }

                if let Some(request) = app.mouse_button_handling(&mouse_button_handler) {
                    match request {
                        Request::Exit => *control_flow = ControlFlow::Exit,
                        Request::TitleChange(title) => window.set_title(title.as_str()),
                        _ => (),
                    };
                }

                app.render(&mut pixels[0..((width * height * 4) as usize)]);

                let render_buffer = device.create_buffer_mapped(&wgpu::BufferDescriptor {
                    label: Some("render-buffer"),
                    size: pixel_count as BufferAddress,
                    usage: wgpu::BufferUsage::COPY_SRC,
                });

                render_buffer.data.copy_from_slice(&pixels[0..pixel_count]);
                let render_buffer = render_buffer.finish();
                let output_texture = swap_chain.get_next_texture().unwrap();

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
                    label: Some("blit-bind-group"),
                });

                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("blit"),
                });
                encoder.copy_buffer_to_texture(
                    wgpu::BufferCopyView {
                        buffer: &render_buffer,
                        offset: 0 as wgpu::BufferAddress,
                        bytes_per_row: width * 4,
                        rows_per_image: height,
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

                // last_frame = imgui.io_mut().update_delta_time(last_frame);

                // platform
                //     .prepare_frame(imgui.io_mut(), &window)
                //     .expect("Failed to prepare ImGui frame.");
                // let ui = imgui.frame();

                // app.imgui(&ui);

                // if last_cursor != Some(ui.mouse_cursor()) {
                //     last_cursor = Some(ui.mouse_cursor());
                //     platform.prepare_render(&ui, &window);
                // }

                // renderer
                //     .render(ui.render(), &mut device, &mut encoder, &output_texture.view)
                //     .expect("ImGui render failed.");

                queue.submit(&[encoder.finish()]);
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                window_id,
            } if window_id == window.id() => {
                let size = window.inner_size();
                sc_descriptor.width = size.width;
                sc_descriptor.height = size.height;

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
            Event::WindowEvent {
                event: WindowEvent::MouseInput { state, button, .. },
                window_id,
            } if window_id == window.id() => {
                mouse_button_handler.insert(button, state);
            }
            _ => (),
        }

        // platform.handle_event(imgui.io_mut(), &window, &event);
    });
}
