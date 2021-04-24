#![allow(dead_code)]

use rfw::{math::*, prelude::*, utils::Timer};
pub use winit::event::MouseButton as MouseButtonCode;
pub use winit::event::VirtualKeyCode as KeyCode;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

type KeyHandler = rfw::utils::input::Input<VirtualKeyCode>;
type MouseButtonHandler = rfw::utils::input::Input<MouseButtonCode>;

use nphysics3d::{
    algebra::{Force3, ForceType},
    force_generator::DefaultForceGeneratorSet,
    joint::DefaultJointConstraintSet,
    material::{BasicMaterial, MaterialHandle},
    nalgebra::Vector3,
    nalgebra::{Isometry3, Unit},
    ncollide3d::shape::{Ball, ShapeHandle},
    object::{
        BodyPartHandle, BodyStatus, ColliderDesc, DefaultBodySet, DefaultColliderSet, RigidBodyDesc,
    },
    world::{DefaultGeometricalWorld, DefaultMechanicalWorld},
};
use std::error::Error;
use winit::window::Fullscreen;

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

    width = window.inner_size().width as u32;
    height = window.inner_size().height as u32;

    let scale_factor: f64 = window
        .current_monitor()
        .map(|m| 1.0 / m.scale_factor())
        .unwrap_or(1.0);

    use rfw_backend_wgpu::WgpuBackend;

    let mut renderer: rfw::Instance = rfw::Instance::new(
        WgpuBackend::init(&window, (width, height), scale_factor)?,
        (width, height),
        Some(scale_factor),
    )?;

    let mut mechanical_world = DefaultMechanicalWorld::new(Vector3::new(0.0_f32, -9.81, 0.0));
    let mut geometrical_world = DefaultGeometricalWorld::new();
    let mut bodies = DefaultBodySet::new();
    let mut colliders = DefaultColliderSet::new();
    let mut joint_constraints = DefaultJointConstraintSet::new();
    let mut force_generators = DefaultForceGeneratorSet::new();

    let mut timer = Timer::new();
    let mut timer2 = Timer::new();
    let mut fps = Averager::new();
    let mut synchronize = Averager::new();
    let mut physics = Averager::new();
    let mut render = Averager::new();
    let mut resized = false;

    renderer.add_directional_light(Vec3::new(0.0, -1.0, 0.5), Vec3::ONE);

    // Ground
    let plane_material =
        renderer
            .get_scene_mut()
            .materials
            .add(Vec3::new(0.3, 0.4, 0.6), 1.0, Vec3::ONE, 0.0);
    let plane = Quad3D::new(Vec3::Y, Vec3::ZERO, 50.0, 50.0, plane_material as _);

    let plane = renderer.get_scene_mut().add_3d_object(plane);
    let _plane_inst = renderer.get_scene_mut().add_3d_instance(plane).unwrap();

    let ground_shape = ShapeHandle::new(nphysics3d::ncollide3d::shape::Plane::new(
        Unit::new_normalize(Vector3::new(0.0_f32, 1.0, 0.0)),
    ));
    let ground_handle = bodies.insert(
        RigidBodyDesc::new()
            .mass(1.0_f32)
            .angular_damping(0.5)
            .status(BodyStatus::Static)
            .build(),
    );
    let ground_collider = ColliderDesc::new(ground_shape)
        .material(MaterialHandle::new(BasicMaterial::new(0.3, 1.2)))
        .build(BodyPartHandle(ground_handle, 0));

    colliders.insert(ground_collider);

    let sphere_material =
        renderer
            .get_scene_mut()
            .materials
            .add(Vec3::new(1.0, 0.0, 0.0), 1.0, Vec3::ONE, 0.0);
    let sphere_radius = 0.5_f32;
    let sphere_center: [f32; 3] = [0.0, 5.0, 0.0];
    let sphere = Sphere::new([0.0; 3], sphere_radius, sphere_material as _);
    let sphere = renderer.get_scene_mut().add_3d_object(sphere);
    let mut sphere_inst = renderer.get_scene_mut().add_3d_instance(sphere)?;
    sphere_inst
        .get_transform()
        .set_translation(sphere_center.into());

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

    timer2.reset();
    synchronize.add_sample(timer2.elapsed_in_millis());

    let mut first = true;

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
                    key_handler.insert(key, input.state == ElementState::Pressed);
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

                let mut view_change = Vec3::ZERO;
                let mut pos_change = Vec3::ZERO;
                let mut sphere_forces = Vec3::ZERO;

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

                if fullscreen_timer > 500.0
                    && key_handler.pressed(KeyCode::LControl)
                    && key_handler.pressed(KeyCode::F)
                {
                    if window.fullscreen().is_none() {
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
                    "rfw-rs - FPS: {:.2}, render: {:.2}, physics: {:.2}, synchronize: {:.2}",
                    fps.get_average(),
                    render.get_average(),
                    physics.get_average(),
                    synchronize.get_average()
                );
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

                renderer.get_camera_3d().translate_target(view_change);
                renderer.get_camera_3d().translate_relative(pos_change);

                if resized {
                    renderer.resize((width, height), None);
                    renderer
                        .get_camera_3d()
                        .set_aspect_ratio(width as f32 / height as f32);
                    resized = false;
                }

                timer2.reset();
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

                if !sphere_forces.cmpeq(Vec3::ZERO).all() {
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
                physics.add_sample(timer2.elapsed_in_millis());

                sphere_inst
                    .get_transform()
                    .set_translation(Vec3::new(
                        data.translation.x,
                        data.translation.y,
                        data.translation.z,
                    ))
                    .set_rotation(Quat::from(Vec4::new(
                        data.rotation.i,
                        data.rotation.j,
                        data.rotation.k,
                        data.rotation.w,
                    )));

                timer2.reset();
                synchronize.add_sample(timer2.elapsed_in_millis());

                timer2.reset();
                renderer.render();
                render.add_sample(timer2.elapsed_in_millis());
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                window_id,
            } if window_id == window.id() => {
                width = size.width;
                height = size.height;

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
                mouse_button_handler.insert(button, state == ElementState::Pressed);
            }
            _ => (),
        }
    });
}
