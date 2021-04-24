use rayon::prelude::*;
use std::error::Error;

pub use winit::event::MouseButton as MouseButtonCode;
pub use winit::event::VirtualKeyCode as KeyCode;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use rfw::prelude::*;
use rfw_backend_wgpu::WgpuBackend;
use rfw_font::*;
use winit::window::Fullscreen;

type KeyHandler = rfw::utils::input::ButtonState<VirtualKeyCode>;
type MouseButtonHandler = rfw::utils::input::ButtonState<MouseButtonCode>;

#[derive(Debug, Default)]
struct FpsSystem {
    timer: Timer,
    average: Averager<f32>,
}

fn fps_system(mut font_renderer: ResMut<FontRenderer>, mut fps_component: Query<&mut FpsSystem>) {
    for mut c in fps_component.iter_mut() {
        let elapsed = c.timer.elapsed_in_millis();
        c.timer.reset();
        c.average.add_sample(elapsed);
        let average = c.average.get_average();

        font_renderer.draw(
            Section::default()
                .with_screen_position((0.0, 0.0))
                .add_text(
                    Text::new(
                        format!("FPS: {:.2}\nFRAMETIME: {:.2} MS", 1000.0 / average, average)
                            .as_str(),
                    )
                    .with_scale(32.0)
                    .with_color([1.0; 4]),
                ),
        );
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("rfw-rs")
        .with_inner_size(LogicalSize::new(1280_f64, 720_f64))
        .build(&event_loop)
        .unwrap();

    let mut width = window.inner_size().width;
    let mut height = window.inner_size().height;

    let scale_factor: f32 = window
        .current_monitor()
        .map(|m| 1.0 / m.scale_factor() as f32)
        .unwrap_or(1.0);

    let font = include_bytes!("../../../assets/good-times-rg.ttf");
    let mut renderer: Instance = Instance::new(
        WgpuBackend::init(&window, (width, height), scale_factor as f64).unwrap(),
        (width, height),
        Some(scale_factor as f64),
    )
    .unwrap()
    .with_plugin(FontRenderer::from_bytes(font))
    .with_system(fps_system.system());

    renderer.spawn().insert(FpsSystem::default());

    let mut key_handler = KeyHandler::new();
    let mut mouse_button_handler = MouseButtonHandler::new();

    let mut timer = Timer::new();

    let mut resized = false;

    renderer.add_spot_light(
        vec3(2.5, 15.0, 0.0),
        vec3(0.0, -1.0, 0.3),
        vec3(105.0, 10.0, 10.0),
        45.0,
        60.0,
    );

    renderer.add_spot_light(
        vec3(0.0, 15.0, 0.0),
        vec3(0.0, -1.0, 0.3),
        vec3(10.0, 105.0, 10.0),
        45.0,
        60.0,
    );

    renderer.add_spot_light(
        vec3(-2.5, 15.0, 0.0),
        vec3(0.0, -1.0, -0.3),
        vec3(10.0, 10.0, 105.0),
        45.0,
        60.0,
    );

    renderer.add_directional_light(vec3(0.0, -1.0, 0.5), vec3(0.6, 0.4, 0.4));

    let material =
        renderer
            .get_scene_mut()
            .materials
            .add(vec3(1.0, 0.2, 0.03), 1.0, Vec3::ONE, 0.0);
    let sphere = Sphere::new(Vec3::ZERO, 0.2, material as u32).with_quality(Quality::Medium);
    let sphere = renderer.get_scene_mut().add_3d_object(sphere);
    let sphere_x = 20_i32;
    let sphere_z = 15_i32;

    let mut handles = {
        let mut handles = Vec::new();
        let mut scene = renderer.get_scene_mut();
        for x in -sphere_x..=sphere_x {
            for z in -sphere_z..=sphere_z {
                let mut instance = scene.add_3d(&sphere);
                instance
                    .get_transform()
                    .set_matrix(Mat4::from_translation(Vec3::new(x as f32, 0.3, z as f32)));
                handles.push(instance);
            }
        }
        handles
    };

    let cesium_man = renderer
        .get_scene_mut()
        .load("assets/models/CesiumMan/CesiumMan.gltf")?
        .scene()?;

    let mut cesium_man1 = renderer.get_scene_mut().add_3d(&cesium_man);
    cesium_man1
        .get_transform()
        .set_scale(Vec3::splat(3.0))
        .rotate_y(180.0_f32.to_radians());
    let mut cesium_man2 = renderer.get_scene_mut().add_3d(&cesium_man);
    cesium_man2
        .get_transform()
        .translate(Vec3::new(-3.0, 0.0, 0.0))
        .rotate_y(180.0_f32.to_radians());

    let pica_desc = renderer
        .get_scene_mut()
        .load("assets/models/pica/scene.gltf")?
        .scene()
        .unwrap();
    renderer.get_scene_mut().add_3d(&pica_desc);

    let app_time = Timer::new();

    renderer.set_animations_time(0.0);

    let mut scene_timer = Timer::new();
    let mut scene_id = None;

    let mut fullscreen_timer = 0.0;
    let mut scale_factor_changed = false;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match &event {
            Event::MainEventsCleared => {
                if key_handler.pressed(KeyCode::Escape) {
                    *control_flow = ControlFlow::Exit;
                }

                if let Some(mut system) = renderer.get_resource_mut::<RenderSystem>() {
                    if key_handler.pressed(KeyCode::Key0) {
                        system.mode = RenderMode::Default;
                    }
                    if key_handler.pressed(KeyCode::Key1) {
                        system.mode = RenderMode::Albedo;
                    }
                    if key_handler.pressed(KeyCode::Key2) {
                        system.mode = RenderMode::Normal;
                    }
                    if key_handler.pressed(KeyCode::Key3) {
                        system.mode = RenderMode::GBuffer;
                    }
                    if key_handler.pressed(KeyCode::Key5) {
                        system.mode = RenderMode::ScreenSpace;
                    }
                    if key_handler.pressed(KeyCode::Key6) {
                        system.mode = RenderMode::SSAO;
                    }
                    if key_handler.pressed(KeyCode::Key7) {
                        system.mode = RenderMode::FilteredSSAO;
                    }
                }

                if scene_timer.elapsed_in_millis() >= 500.0 && key_handler.pressed(KeyCode::Space) {
                    if let Some(handle) = scene_id.take() {
                        renderer.get_scene_mut().remove_3d(handle);
                        scene_id = None;
                    } else {
                        let mut handle = renderer.get_scene_mut().add_3d(&cesium_man);
                        handle
                            .get_transform()
                            .translate(Vec3::new(-6.0, 0.0, 0.0))
                            .rotate_y(180.0_f32.to_radians());
                        scene_id = Some(handle);
                    }

                    scene_timer.reset();
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

                let elapsed = if key_handler.pressed(KeyCode::LShift) {
                    elapsed * 2.0
                } else {
                    elapsed
                };

                timer.reset();

                let view_change = view_change * elapsed * 0.001;
                let pos_change = pos_change * elapsed * 0.01;

                if view_change != [0.0; 3].into() {
                    renderer.get_camera_3d().translate_target(view_change);
                }
                if pos_change != [0.0; 3].into() {
                    renderer.get_camera_3d().translate_relative(pos_change);
                }

                if resized || scale_factor_changed {
                    renderer.resize((width, height), Some(scale_factor as f64));
                    *renderer.get_camera_2d() =
                        Camera2D::from_width_height(width, height, Some(scale_factor as f64));
                    renderer
                        .get_camera_3d()
                        .set_aspect_ratio(width as f32 / height as f32);
                    resized = false;
                    scale_factor_changed = false;
                }

                renderer
                    .get_scene_mut()
                    .lights
                    .spot_lights
                    .iter_mut()
                    .for_each(|(_, sl)| {
                        let direction = Vec3::from(sl.direction);
                        let direction = Quat::from_rotation_y((elapsed / 10.0).to_radians())
                            .mul_vec3(direction);
                        sl.direction = direction.into();
                    });

                let time = app_time.elapsed_in_millis() / 1000.0;
                renderer.set_animation_time(&cesium_man1, time);
                renderer.set_animation_time(&cesium_man2, time / 2.0);
                if let Some(cesium_man3) = &scene_id {
                    renderer.set_animation_time(cesium_man3, time / 3.0);
                }
//
// <<<<<<< HEAD
// =======
//                 {
//                     let mut instances_3d = 0;
//                     let mut vertices = 0;
//                     {
//                         let scene = renderer.get_scene();
//
//                         for (i, m) in scene.objects.meshes_3d.iter() {
//                             instances_3d += scene.objects.instances_3d[i].len();
//                             vertices += m.vertices.len() * scene.objects.instances_3d[i].len();
//                         }
//                     }
//                     let meshes_3d = renderer.get_scene().objects.meshes_3d.len();
//                     let instances_2d: usize = renderer
//                         .get_scene()
//                         .objects
//                         .instances_2d
//                         .iter()
//                         .map(|(_, i)| i.len())
//                         .sum();
//                     let meshes_2d = renderer.get_scene().objects.meshes_2d.len();
//
//                     let settings = renderer.get_settings();
//                     settings.draw_ui(&window, |ui| {
//                         use rfw_backend_wgpu::imgui;
//                         let window = imgui::Window::new(imgui::im_str!("RFW"));
//                         window
//                             .size([350.0, 250.0], imgui::Condition::FirstUseEver)
//                             .position([900.0, 25.0], imgui::Condition::FirstUseEver)
//                             .build(&ui, || {
//                                 ui.checkbox(imgui::im_str!("Skinning"), &mut enable_skinning);
//                                 ui.text(imgui::im_str!("FPS: {}", ui.io().framerate));
//                                 ui.text(imgui::im_str!("3D Vertex count: {}", vertices));
//                                 ui.text(imgui::im_str!("3D Instance count: {}", instances_3d));
//                                 ui.text(imgui::im_str!("3D Mesh count: {}", meshes_3d));
//                                 ui.text(imgui::im_str!("2D Instance count: {}", instances_2d));
//                                 ui.text(imgui::im_str!("2D Mesh count: {}", meshes_2d));
//                                 scale_factor_changed = ui
//                                     .input_float(imgui::im_str!("Scale factor"), &mut scale_factor)
//                                     .step(0.05)
//                                     .build();
//                                 scale_factor = scale_factor.max(0.1).min(2.0);
//                             });
//                     });
//
//                     settings.enable_skinning = enable_skinning;
//                 }
//
// >>>>>>> vulkan-backend
                let t = app_time.elapsed_in_millis() / 1000.0;
                handles.par_iter_mut().enumerate().for_each(|(i, h)| {
                    let x = (i as i32 % (sphere_x * 2)) - sphere_x;
                    let z = (i as i32 / (sphere_x * 2)) - sphere_z;
                    let _x = (((x + sphere_x) as f32) + t).sin();
                    let _z = (((z + sphere_z) as f32) + t).sin();
                    let height = (_z + _x) * 0.5 + 1.0;

                    h.get_transform()
                        .set_matrix(Mat4::from_translation(Vec3::new(
                            x as f32,
                            0.3 + height,
                            z as f32,
                        )));
                });

                renderer.render();
            }
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                window_id,
            } if *window_id == window.id() => {
                if let Some(key) = input.virtual_keycode {
                    key_handler.insert(key, input.state == ElementState::Pressed);
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if *window_id == window.id() => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                window_id,
            } if *window_id == window.id() => {
                width = size.width;
                height = size.height;
                resized = true;
            }
            Event::WindowEvent {
                event: WindowEvent::MouseInput { state, button, .. },
                window_id,
            } if *window_id == window.id() => {
                mouse_button_handler.insert(*button, *state == ElementState::Pressed);
            }
            _ => (),
        }
    });
}
