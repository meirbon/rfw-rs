#![allow(dead_code)]

use std::{collections::HashMap, error::Error};
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

use futures::executor::block_on;
use glam::*;
use rfw_system::{scene::renderers::RenderMode, scene::Camera, RenderSystem};
use shared::utils;
use winit::window::Fullscreen;

fn main() -> Result<(), Box<dyn Error>> {
    run_application()
}

fn run_application() -> Result<(), Box<dyn Error>> {
    let mut width = 1600;
    let mut height = 900;

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

    let mut renderer =
        RenderSystem::<rfw_deferred::Deferred>::new(&window, render_width, render_height).unwrap();

    // Set build bvh flag to make sure intersector can actually hit something
    renderer.get_scene_flags(|flags| {
        flags.set_flag(rfw_system::scene::SceneFlags::BuildBVHs);
    });

    let mut camera =
        Camera::new(width as u32, height as u32).with_direction(Vec3::new(0.0, 0.0, -1.0));
    let mut timer = utils::Timer::new();
    let mut timer2 = utils::Timer::new();
    let mut fps = utils::Averager::new();
    let mut render = utils::Averager::new();
    let mut synchronize = utils::Averager::new();
    let mut resized = false;

    let pica = renderer.load_async("models/pica/scene.gltf");
    let cesium_man = renderer.load_async("models/CesiumMan/CesiumMan.gltf");

    match block_on(cesium_man)? {
        rfw_system::scene::LoadResult::Scene(root_nodes) => {
            root_nodes.iter().for_each(|node| {
                renderer.get_node_mut(*node, |node| {
                    if let Some(node) = node {
                        node.set_scale(Vec3::splat(3.0));
                        node.set_rotation(Quat::from_rotation_y(180.0_f32.to_radians()));
                    }
                });
            });
        }
        rfw_system::scene::LoadResult::Object(_) => panic!("Gltf files should be loaded as scenes"),
    };

    block_on(pica)?;

    let app_time = utils::Timer::new();

    timer2.reset();
    renderer.set_animation_time(0.0);
    renderer.synchronize();
    synchronize.add_sample(timer2.elapsed_in_millis());

    let mut fullscreen_timer = 0.0;
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

                if mouse_button_handler.pressed(MouseButtonCode::Left) {
                    let view = camera.get_view();
                    renderer.get_intersector(|i| {
                        let window_size = window.inner_size();
                        let x = mouse_x as u32;
                        let y = mouse_y as u32;
                        let x = (x as f32 / window_size.width as f32 / view.inv_width) as u32;
                        let y = (y as f32 / window_size.height as f32 / view.inv_height) as u32;

                        let ray = view.generate_ray(x, y);
                        println!("ray x: {}, y: {}", x, y);
                        let (origin, direction) = ray.get_vectors::<Vec3A>();
                        println!("origin: {}, direction: {}", origin, direction);
                        let hit = i.intersect(ray, 1e-5 as f32, 1e26 as f32);
                        if let Some(hit) = hit {
                            println!("hit material {}", hit.mat_id);
                        } else {
                            println!("hit nothing");
                        }
                    });
                }

                if fullscreen_timer > 500.0
                    && key_handler.pressed(KeyCode::LControl)
                    && key_handler.pressed(KeyCode::F)
                {
                    if let None = window.fullscreen() {
                        window
                            .set_fullscreen(Some(Fullscreen::Borderless(window.current_monitor())));
                    } else {
                        window.set_fullscreen(None);
                    }
                    fullscreen_timer = 0.0;
                }

                let elapsed = timer.elapsed_in_millis();
                fullscreen_timer += elapsed;
                fps.add_sample(1000.0 / elapsed);
                let title = format!(
                    "rfw-rs - FPS: {:.2}, render: {:.2} ms, synchronize: {:.2} ms",
                    fps.get_average(),
                    render.get_average(),
                    synchronize.get_average()
                );
                window.set_title(title.as_str());

                let elapsed = if key_handler.pressed(KeyCode::LShift) {
                    elapsed * 2.0
                } else {
                    elapsed
                };

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

                timer2.reset();
                renderer.set_animation_time(app_time.elapsed_in_millis() / 1000.0);
                renderer.synchronize();
                synchronize.add_sample(timer2.elapsed_in_millis());

                timer2.reset();
                renderer.render(&camera, RenderMode::Reset);
                render.add_sample(timer2.elapsed_in_millis());
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
