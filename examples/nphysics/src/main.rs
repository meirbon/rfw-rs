#![allow(dead_code)]

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
use nphysics3d::algebra::{Force3, ForceType};
use nphysics3d::force_generator::DefaultForceGeneratorSet;
use nphysics3d::joint::DefaultJointConstraintSet;
use nphysics3d::material::{BasicMaterial, MaterialHandle};
use nphysics3d::nalgebra::Vector3;
use nphysics3d::ncollide3d::nalgebra::{Isometry3, Unit};
use nphysics3d::ncollide3d::shape::{Ball, ShapeHandle};
use nphysics3d::object::{
    BodyPartHandle, BodyStatus, ColliderDesc, DefaultBodySet, DefaultColliderSet,
    RigidBodyDesc,
};
use nphysics3d::world::{DefaultGeometricalWorld, DefaultMechanicalWorld};
use scene::renderers::RenderMode;
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
    use scene::RenderSystem;

    let renderer: RenderSystem<Deferred> = RenderSystem::new(&window, width, height).unwrap();

    let mut mechanical_world = DefaultMechanicalWorld::new(Vector3::new(0.0_f32, -9.81, 0.0));
    let mut geometrical_world = DefaultGeometricalWorld::new();
    let mut bodies = DefaultBodySet::new();
    let mut colliders = DefaultColliderSet::new();
    let mut joint_constraints = DefaultJointConstraintSet::new();
    let mut force_generators = DefaultForceGeneratorSet::new();

    let mut camera =
        scene::Camera::new(width as u32, height as u32).with_position(Vec3::new(0.0, 1.0, -4.0));
    let mut timer = Timer::new();
    let mut fps = utils::Averager::new();
    let mut resized = false;

    // renderer.add_spot_light(
    //     Vec3::new(0.0, 10.0, 0.0),
    //     Vec3::new(0.0, -1.0, 0.0),
    //     Vec3::splat(100.0),
    //     45.0,
    //     60.0,
    // );
    renderer.add_directional_light(Vec3::new(0.0, -1.0, 0.5), Vec3::splat(1.0));

    // Ground
    let plane_material = renderer.add_material([1.0, 0.3, 0.3], 1.0, [1.0; 3], 0.0)?;
    let plane = scene::Plane::new([0.0; 3], [0.0, 1.0, 0.0], [50.0; 2], plane_material);
    let plane = renderer.add_object(plane)?;
    let _plane_inst = renderer.create_instance(plane)?;

    let ground_shape = ShapeHandle::new(nphysics3d::ncollide3d::shape::Plane::new(
        Unit::new_normalize(Vector3::new(0.0_f32, 1.0, 0.0)),
    ));
    let ground_handle = bodies.insert(RigidBodyDesc::new()
        .mass(1.0_f32)
        .angular_damping(0.5)
        .status(BodyStatus::Static)
        .build());
    let ground_collider = ColliderDesc::new(ground_shape)
        .material(MaterialHandle::new(BasicMaterial::new(0.3, 1.2)))
        .build(BodyPartHandle(ground_handle, 0));

    colliders.insert(ground_collider);

    let sphere_material = renderer.add_material([0.0, 0.5, 1.0], 1.0, [1.0; 3], 0.0)?;
    let sphere_radius = 0.5_f32;
    let sphere_center: [f32; 3] = [0.0, 5.0, 0.0];
    let sphere = scene::Sphere::new([0.0; 3], sphere_radius, sphere_material);
    let sphere = renderer.add_object(sphere)?;
    let sphere_inst = renderer.create_instance(sphere)?;
    renderer.get_instance_mut(sphere_inst, |instance| {
        instance.unwrap().set_translation(sphere_center);
    });

    let sphere = ShapeHandle::new(Ball::new(sphere_radius));

    // Build the rigid body.
    let sphere_rb = RigidBodyDesc::new()
        .translation(Vector3::new(
            sphere_center[0],
            sphere_center[1],
            sphere_center[2],
        ))
        .mass(1.0_f32)
        .angular_damping(0.5)
        .status(BodyStatus::Dynamic)
        .build();
    let sphere_rb_handle = bodies.insert(sphere_rb);

    // Build the collider.
    let sphere_collider = ColliderDesc::new(sphere)
        .density(0.1_f32)
        .material(MaterialHandle::new(BasicMaterial::new(0.3, 0.8)))
        .build(BodyPartHandle(sphere_rb_handle, 0));
    let sphere_collider = colliders.insert(sphere_collider);

    geometrical_world.maintain(&mut bodies, &mut colliders);

    renderer.synchronize();

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

                let mut view_change = Vec3::zero();
                let mut pos_change = Vec3::zero();
                let mut sphere_forces = Vec3::zero();

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

                if key_handler.pressed(KeyCode::I) {
                    sphere_forces += (0.0, 0.0, 1.0).into();
                }
                if key_handler.pressed(KeyCode::K) {
                    sphere_forces -= (0.0, 0.0, 1.0).into();
                }
                if key_handler.pressed(KeyCode::J) {
                    sphere_forces += (1.0, 0.0, 0.0).into();
                }
                if key_handler.pressed(KeyCode::L) {
                    sphere_forces -= (1.0, 0.0, 0.0).into();
                }

                if key_handler.pressed(KeyCode::Space) {
                    sphere_forces += (0.0, 50.0, 0.0).into();
                }

                let elapsed = timer.elapsed_in_millis();
                fps.add_sample(1000.0 / elapsed);
                let title = format!("rfw-rs - FPS: {:.2}", fps.get_average());
                window.set_title(title.as_str());

                sphere_forces *= elapsed * 0.01;

                let camera_elapsed = if key_handler.pressed(KeyCode::LShift) {
                    elapsed * 2.0
                } else {
                    elapsed
                };

                timer.reset();

                let view_change = view_change * camera_elapsed * 0.001;
                let pos_change = pos_change * camera_elapsed * 0.01;

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

                mechanical_world.set_timestep(match first {
                    true => {
                        first = false;
                        0.0
                    }
                    _ => elapsed / 1000.0,
                });
                mechanical_world.step(
                    &mut geometrical_world,
                    &mut bodies,
                    &mut colliders,
                    &mut joint_constraints,
                    &mut force_generators,
                );

                let sphere_collider = colliders.get(sphere_collider).unwrap();
                let data: &Isometry3<f32> = sphere_collider.position();

                if !sphere_forces.cmpeq(Vec3::zero()).all() {
                    if data.translation.y >= (sphere_radius + 0.05) {
                        sphere_forces[1] = 0.0;
                    }

                    bodies.get_mut(sphere_rb_handle).unwrap().apply_force(
                        0,
                        &Force3::new(
                            Vector3::new(sphere_forces[0], sphere_forces[1], sphere_forces[2]),
                            Vector3::default(),
                        ),
                        ForceType::Impulse,
                        true,
                    );
                }

                renderer.get_instance_mut(sphere_inst, |instance| {
                    if let Some(instance) = instance {
                        let translation =
                            Vec3::new(data.translation.x, data.translation.y, data.translation.z);
                        instance.set_translation(translation);
                        let rotation = Quat::from(Vec4::new(
                            data.rotation.i,
                            data.rotation.j,
                            data.rotation.k,
                            data.rotation.w,
                        ));
                        instance.set_rotation_quat(rotation);
                    }
                });
                renderer.synchronize();

                renderer.render(&camera, RenderMode::Reset);
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
