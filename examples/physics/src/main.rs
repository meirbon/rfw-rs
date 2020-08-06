#![allow(dead_code)]

use physx::prelude::*;
use std::collections::HashMap;
pub use winit::event::MouseButton as MouseButtonCode;
pub use winit::event::VirtualKeyCode as KeyCode;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

const PX_PHYSICS_VERSION: u32 = physx::version(4, 1, 1);

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
use rfw_system::scene::renderers::RenderMode;
use shared::utils;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
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

    use rfw_deferred::Deferred;
    use rfw_system::RenderSystem;

    let renderer: RenderSystem<Deferred> = RenderSystem::new(&window, width, height).unwrap();

    let mut foundation = Foundation::new(PX_PHYSICS_VERSION);
    let mut physics = PhysicsBuilder::default()
        .load_extensions(false)
        .build(&mut foundation);
    let mut scene = physics.create_scene(
        SceneBuilder::default()
            .set_gravity(Vec3::new(0.0, -9.81, 0.0))
            .set_simulation_threading(SimulationThreadType::Dedicated(2)),
    );

    let mut camera = rfw_system::scene::Camera::new(width as u32, height as u32)
        .with_position(Vec3::new(0.0, 2.0, -8.0));
    let mut timer = Timer::new();
    let mut timer2 = Timer::new();
    let mut fps = utils::Averager::new();
    let mut render = utils::Averager::new();
    let mut synchronize = utils::Averager::new();
    let mut resized = false;

    renderer.add_spot_light(
        Vec3::new(0.0, 10.0, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::splat(100.0),
        45.0,
        60.0,
    );

    // Ground
    let plane_material = renderer.add_material([1.0, 0.3, 0.3], 1.0, [1.0; 3], 0.0)?;
    let plane = rfw_system::scene::Plane::new([0.0; 3], [0.0, 1.0, 0.0], [50.0; 2], plane_material);
    let plane = renderer.add_object(plane)?;
    let _plane_inst = renderer.create_instance(plane)?;

    let material = physics.create_material(0.5, 0.5, 0.6);
    let ground_plane = unsafe { physics.create_plane(Vec3::new(0.0, 1.0, 0.0), 0.0, material) };
    scene.add_actor(ground_plane);

    let sphere_material = renderer.add_material([0.0, 0.5, 1.0], 1.0, [1.0; 3], 0.0)?;
    let sphere_radius = 0.5_f32;
    let sphere_center: [f32; 3] = [0.0, 10.0, 0.0];
    let sphere = rfw_system::scene::Sphere::new([0.0; 3], sphere_radius, sphere_material);
    let sphere =
        renderer.add_object(sphere.with_quality(rfw_system::scene::sphere::Quality::Medium))?;
    let sphere_inst = renderer.create_instance(sphere)?;
    renderer.get_instance_mut(sphere_inst, |instance| {
        instance.unwrap().set_translation(sphere_center);
    });

    let sphere_geo = PhysicsGeometry::from(&ColliderDesc::Sphere(sphere_radius));
    let mut sphere_actor = unsafe {
        physics.create_dynamic(
            Mat4::from_translation(sphere_center.into()),
            sphere_geo.as_raw(),
            material,
            10.0,
            Mat4::identity(),
        )
    };

    sphere_actor.set_angular_damping(0.5);
    let sphere_handle = scene.add_dynamic(sphere_actor);

    scene.simulate(1.0 / 60.0);
    timer2.reset();
    renderer.synchronize();
    synchronize.add_sample(timer2.elapsed_in_millis());

    let mut first = true;

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

                let elapsed = timer.elapsed_in_millis();
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
                    renderer.resize(&window, width, height);
                    camera.resize(width as u32, height as u32);
                    resized = false;
                }

                scene.fetch_results(true).unwrap();
                let global_pos: [f32; 3] = unsafe {
                    scene
                        .get_rigid_actor_unchecked(&sphere_handle)
                        .get_global_position()
                }
                .into();

                renderer.get_instance_mut(sphere_inst, |instance| {
                    if let Some(instance) = instance {
                        // let translation = Vec3::new(data.translation.x, data.translation.y, data.translation.z);
                        instance.set_translation(global_pos);
                        // let rotation = Quat::from(Vec4::new(data.rotation.i, data.rotation.j, data.rotation.k, data.rotation.w));
                        // instance.set_rotation_quat(rotation);
                    }
                });

                timer2.reset();
                renderer.synchronize();
                synchronize.add_sample(timer2.elapsed_in_millis());

                timer2.reset();
                renderer.render(&camera, rfw_system::scene::RenderMode::Reset);
                render.add_sample(timer2.elapsed_in_millis());

                scene.simulate(match first {
                    true => {
                        first = false;
                        1.0 / 60.0
                    }
                    _ => elapsed / 1000.0,
                });
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
