#![allow(dead_code)]

#[global_allocator]
static ALLOC: rpmalloc::RpMalloc = rpmalloc::RpMalloc;

mod wgpu_renderer;

enum AppType {
    CPURayTracer,
    GPU,
}

use std::collections::HashMap;
pub use winit::event::MouseButton as MouseButtonCode;
pub use winit::event::VirtualKeyCode as KeyCode;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

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

use crate::utils::Timer;
use glam::*;
use scene::{
    renderers::{RenderMode, Setting, SettingValue},
    InstanceRef,
};
use shared::utils;

fn main() {
    let mut width = 1280;
    let mut height = 720;

    let mut key_handler = KeyHandler::new();
    let mut mouse_button_handler = MouseButtonHandler::new();
    let mut first_mouse_pos = true;

    let mut mouse_x = 0.0;
    let mut mouse_y = 0.0;

    let mut _old_mouse_x = 0.0;
    let mut _old_mouse_y = 0.0;

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("rfw-rs")
        .with_inner_size(LogicalSize::new(width as f64, height as f64))
        .build(&event_loop)
        .unwrap();

    width = window.inner_size().width as usize;
    height = window.inner_size().height as usize;

    let dpi_factor = window.current_monitor().scale_factor();
    let render_width = (width as f64 / dpi_factor) as usize;
    let render_height = (height as f64 / dpi_factor) as usize;

    use scene::RenderSystem;
    use wgpu_renderer::Deferred;

    let renderer: RenderSystem<Deferred> =
        RenderSystem::new(&window, render_width, render_height).unwrap();
    let mut camera = scene::Camera::new(render_width as u32, render_height as u32);
    camera.change_fov(60.0);
    let mut timer = Timer::new();
    let mut fps = utils::Averager::new();
    let mut resized = false;

    renderer
        .add_spot_light(
            [0.0, 10.0, 0.0],
            [0.0, 0.0, 1.0],
            [150.0, 100.0, 200.0],
            30.0,
            45.0,
        )
        .unwrap();
    renderer.add_directional_light([0.0, -1.0, -0.1], [1.0; 3]);
    let sponza = renderer.load_mesh("models/sponza/sponza.obj").unwrap();
    let mut instance: InstanceRef = renderer.add_instance(sponza).unwrap();
    instance.scale(Vec3::splat(0.1));
    instance.synchronize().unwrap();

    let settings: Vec<scene::renderers::Setting> = renderer.get_settings().unwrap();

    renderer.synchronize();

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
            } if window_id == window.id() => {
                *control_flow = ControlFlow::Exit;
            }
            Event::RedrawRequested(_) => {
                if key_handler.pressed(KeyCode::Escape) {
                    *control_flow = ControlFlow::Exit;
                }

                if !settings.is_empty() {
                    let mut value = None;
                    if key_handler.pressed(KeyCode::Key0) {
                        value = Some(0);
                    }
                    if key_handler.pressed(KeyCode::Key1) {
                        value = Some(1);
                    }
                    if key_handler.pressed(KeyCode::Key2) {
                        value = Some(2);
                    }
                    if key_handler.pressed(KeyCode::Key3) {
                        value = Some(3);
                    }
                    if key_handler.pressed(KeyCode::Key4) {
                        value = Some(4);
                    }
                    if key_handler.pressed(KeyCode::Key5) {
                        value = Some(5);
                    }
                    if key_handler.pressed(KeyCode::Key6) {
                        value = Some(6);
                    }
                    if key_handler.pressed(KeyCode::Key7) {
                        value = Some(7);
                    }

                    if let Some(value) = value {
                        let mut setting: Setting = settings[0].clone();
                        setting.set(SettingValue::Int(value));
                        renderer.set_setting(setting).unwrap();
                    }
                }

                let mut view_change = Vec3::new(0.0, 0.0, 0.0);
                let mut pos_change = Vec3::new(0.0, 0.0, 0.0);

                if key_handler.pressed(KeyCode::Up) {
                    view_change += (0.0, 1.0, 0.0).into();
                }
                if key_handler.pressed(KeyCode::Down) {
                    view_change -= (0.0, 1.0, 0.0).into();
                }
                if key_handler.pressed(KeyCode::Left) {
                    view_change -= (1.0, 0.0, 0.0).into();
                }
                if key_handler.pressed(KeyCode::Right) {
                    view_change += (1.0, 0.0, 0.0).into();
                }

                if key_handler.pressed(KeyCode::W) {
                    pos_change += (0.0, 0.0, 1.0).into();
                }
                if key_handler.pressed(KeyCode::S) {
                    pos_change -= (0.0, 0.0, 1.0).into();
                }
                if key_handler.pressed(KeyCode::A) {
                    pos_change -= (1.0, 0.0, 0.0).into();
                }
                if key_handler.pressed(KeyCode::D) {
                    pos_change += (1.0, 0.0, 0.0).into();
                }
                if key_handler.pressed(KeyCode::E) {
                    pos_change += (0.0, 1.0, 0.0).into();
                }
                if key_handler.pressed(KeyCode::Q) {
                    pos_change -= (0.0, 1.0, 0.0).into();
                }

                let elapsed = timer.elapsed_in_millis();
                fps.add_sample(1000.0 / elapsed);
                let title = format!("rfw-rs - FPS: {:.2}", fps.get_average());
                window.set_title(title.as_str());

                let elapsed = if key_handler.pressed(KeyCode::LShift) {
                    elapsed * 2.0
                } else {
                    elapsed
                };

                if key_handler.pressed(KeyCode::Space) {
                    instance.rotate_y(elapsed / 10.0);
                    instance.synchronize().unwrap();
                }

                timer.reset();

                let view_change = view_change * elapsed * 0.001;
                let pos_change = pos_change * elapsed * 0.01;

                if view_change != [0.0; 3].into() {
                    camera.translate_target(view_change);
                }
                if pos_change != [0.0; 3].into() {
                    camera.translate_relative(pos_change);
                }

                if resized {
                    let render_width = (width as f64 / dpi_factor) as usize;
                    let render_height = (height as f64 / dpi_factor) as usize;
                    renderer.resize(&window, render_width, render_height);
                    camera.resize(render_width as u32, render_height as u32);
                    resized = false;
                }

                renderer.synchronize();
                renderer.render(&camera, RenderMode::Default);
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                window_id,
            } if window_id == window.id() => {
                width = size.width as usize;
                height = size.height as usize;

                resized = true;
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                window_id,
            } if window_id == window.id() => {
                if first_mouse_pos {
                    mouse_x = position.x;
                    mouse_y = position.y;
                    _old_mouse_x = position.x;
                    _old_mouse_y = position.y;
                    first_mouse_pos = false;
                } else {
                    _old_mouse_x = mouse_x;
                    _old_mouse_y = mouse_y;

                    mouse_x = position.x;
                    mouse_y = position.y;
                }

                let _delta_x = mouse_x - _old_mouse_x;
                let _delta_y = mouse_y - _old_mouse_y;
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
