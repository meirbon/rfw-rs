use std::collections::HashMap;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

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

pub trait App {
    fn render(&mut self, fb: &mut [u8]) -> Option<Request>;
    fn key_handling(&mut self, states: &KeyHandler) -> Option<Request>;
    fn mouse_handling(&mut self, x: f64, y: f64, delta_x: f64, delta_y: f64) -> Option<Request>;
    fn scroll_handling(&mut self, dx: f64, dy: f64) -> Option<Request>;
    fn resize(&mut self, width: u32, height: u32) -> Option<Request>;
}

pub fn run_app<T: 'static + App>(mut app: T, title: &str, start_width: u32, start_height: u32) {
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
        .with_inner_size(LogicalSize::new(start_width as f64, start_height as f64))
        .with_min_inner_size(LogicalSize::new(start_width as f64, start_height as f64))
        .build(&event_loop)
        .unwrap();

    let mut pixels = {
        let surface = pixels::wgpu::Surface::create(&window);
        let surface_texture = pixels::SurfaceTexture::new(width, height, surface);
        pixels::PixelsBuilder::new(width, height, surface_texture)
            .request_adapter_options(wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::Default,
                backends: wgpu::BackendBit::PRIMARY
            })
            .build()
            .unwrap()
    };

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
                    pixels = {
                        let surface = pixels::wgpu::Surface::create(&window);
                        let surface_texture = pixels::SurfaceTexture::new(width, height, surface);
                        pixels::PixelsBuilder::new(width, height, surface_texture)
                            .build()
                            .unwrap()
                    };

                    resized = false;
                }

                if let Some(request) = app.key_handling(&key_handler) {
                    match request {
                        Request::Exit => *control_flow = ControlFlow::Exit,
                        Request::TitleChange(title) => window.set_title(title.as_str()),
                    };
                }

                // Assumes texture format: wgpu::TextureFormat::Rgba8UnormSrgb
                app.render(pixels.get_frame());
                pixels.render();
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                window_id,
            } if window_id == window.id() => {
                width = size.width;
                height = size.height;
                app.resize(width, height);

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
                event: WindowEvent::MouseWheel {
                    delta: winit::event::MouseScrollDelta::LineDelta(x, y),
                    ..
                },
                window_id
            } if window_id == window.id() => {
                app.scroll_handling(x as f64, y as f64);
            }
            Event::WindowEvent {
                event: WindowEvent::MouseWheel {
                    delta: winit::event::MouseScrollDelta::PixelDelta(delta),
                    ..
                },
                window_id
            } if window_id == window.id() => {
                app.scroll_handling(delta.x, delta.y);
            }
            _ => (),
        }
    });
}
