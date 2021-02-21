use rayon::prelude::*;
use std::error::Error;

use clap::{App, Arg};
pub use winit::event::MouseButton as MouseButtonCode;
pub use winit::event::VirtualKeyCode as KeyCode;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use rfw::prelude::*;
use rfw_backend_wgpu::{WgpuBackend, WgpuView};
use rfw_font::*;
use winit::window::Fullscreen;

type KeyHandler = rfw::utils::input::ButtonState<VirtualKeyCode>;
type MouseButtonHandler = rfw::utils::input::ButtonState<MouseButtonCode>;

struct FpsSystem {
    timer: Timer,
    average: Averager<f32>,
}

impl Default for FpsSystem {
    fn default() -> Self {
        Self {
            timer: Timer::new(),
            average: Averager::with_capacity(250),
        }
    }
}

impl System for FpsSystem {
    fn run(&mut self, resources: &rfw::prelude::ResourceList) {
        let elapsed = self.timer.elapsed_in_millis();
        self.timer.reset();
        self.average.add_sample(elapsed);
        let average = self.average.get_average();

        if let Some(mut font) = resources.get_resource_mut::<FontRenderer>() {
            font.draw(
                Section::default()
                    .with_screen_position((0.0, 0.0))
                    .add_text(
                        Text::new(
                            format!("FPS: {:.2}\nFrametime: {:.2} ms", 1000.0 / average, average)
                                .as_str(),
                        )
                        .with_scale(32.0)
                        .with_color([1.0; 4]),
                    ),
            );
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let matches = App::new("rfw-animated")
        .about("Example with animated meshes for the rfw framework.")
        .arg(
            Arg::with_name("renderer")
                .short("r")
                .long("renderer")
                .takes_value(true)
                .help("Which renderer to use (current options are: gpu-rt, deferred)"),
        )
        .get_matches();

    match matches.value_of("renderer") {
        // Some("gpu-rt") => run_application::<RayTracer>(),
        // Some("gfx") => run_application::<GfxBackend>(),
        _ => run_wgpu_backend(),
    }
}

fn run_wgpu_backend() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("rfw-rs")
        .with_inner_size(LogicalSize::new(1280_f64, 720_f64))
        .build(&event_loop)
        .unwrap();

    let mut width = window.inner_size().width;
    let mut height = window.inner_size().height;

    let mut scale_factor: f32 = window
        .current_monitor()
        .map(|m| 1.0 / m.scale_factor() as f32)
        .unwrap_or(1.0);

    let font = include_bytes!("../../../assets/good-times-rg.ttf");
    let mut renderer: Instance<WgpuBackend> =
        Instance::new(&window, (width, height), Some(scale_factor as f64))
            .unwrap()
            .with_plugin(FontRenderer::from_bytes(&font[0..font.len()]))
            .with_system(FpsSystem::default());

    let mut key_handler = KeyHandler::new();
    let mut mouse_button_handler = MouseButtonHandler::new();

    let mut camera_2d = Camera2D::from_width_height(width, height, Some(scale_factor as f64));
    let mut camera_3d = Camera3D::new().with_aspect_ratio(width as f32 / height as f32);

    let mut timer = utils::Timer::new();

    let mut resized = false;

    renderer.add_spot_light(
        Vec3::new(2.5, 15.0, 0.0),
        Vec3::new(0.0, -1.0, 0.3),
        Vec3::new(105.0, 10.0, 10.0),
        45.0,
        60.0,
    );

    renderer.add_spot_light(
        Vec3::new(0.0, 15.0, 0.0),
        Vec3::new(0.0, -1.0, 0.3),
        Vec3::new(10.0, 105.0, 10.0),
        45.0,
        60.0,
    );

    renderer.add_spot_light(
        Vec3::new(-2.5, 15.0, 0.0),
        Vec3::new(0.0, -1.0, -0.3),
        Vec3::new(10.0, 10.0, 105.0),
        45.0,
        60.0,
    );

    renderer.add_directional_light(Vec3::new(0.0, -1.0, 0.5), Vec3::new(0.6, 0.4, 0.4));

    let material = renderer.get_scene_mut().get_materials_mut().add(
        Vec3::new(1.0, 0.2, 0.03),
        1.0,
        Vec3::one(),
        0.0,
    );
    let sphere = Sphere::new(Vec3::zero(), 0.2, material as u32).with_quality(Quality::Medium);
    let sphere = renderer.get_scene_mut().add_3d_object(sphere);
    let sphere_x = 20 as i32;
    let sphere_z = 15 as i32;

    let mut handles = {
        let mut handles = Vec::new();
        let mut scene = renderer.get_scene_mut();
        for x in -sphere_x..=sphere_x {
            for z in -sphere_z..=sphere_z {
                let mut instance = scene.add_3d_instance(sphere).unwrap();
                instance.set_matrix(Mat4::from_translation(Vec3::new(x as f32, 0.3, z as f32)));
                handles.push(instance);
            }
        }
        handles
    };

    let cesium_man = renderer
        .get_scene_mut()
        .load("assets/models/CesiumMan/CesiumMan.gltf")?
        .scene()?;

    let mut cesium_man1 = renderer.get_scene_mut().add_3d_scene(&cesium_man);
    cesium_man1.get_transform().set_scale(Vec3::splat(3.0));
    let mut cesium_man2 = renderer.get_scene_mut().add_3d_scene(&cesium_man);
    cesium_man2
        .get_transform()
        .translate(Vec3::new(-3.0, 0.0, 0.0));

    let pica_desc = renderer
        .get_scene_mut()
        .load("assets/models/pica/scene.gltf")?
        .scene()
        .unwrap();
    renderer.get_scene_mut().add_3d_scene(&pica_desc);

    let app_time = utils::Timer::new();

    renderer.set_animations_time(0.0);

    let mut scene_timer = utils::Timer::new();
    let mut scene_id = None;

    renderer.get_settings().setup_imgui(&window);

    let mut fullscreen_timer = 0.0;
    let mut scale_factor_changed = false;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        renderer.get_settings().update_ui(&window, &event);

        match &event {
            Event::MainEventsCleared => {
                if key_handler.pressed(KeyCode::Escape) {
                    *control_flow = ControlFlow::Exit;
                }

                {
                    let settings = renderer.get_settings();
                    if key_handler.pressed(KeyCode::Key0) {
                        settings.view = WgpuView::Output;
                    }
                    if key_handler.pressed(KeyCode::Key1) {
                        settings.view = WgpuView::Albedo;
                    }
                    if key_handler.pressed(KeyCode::Key2) {
                        settings.view = WgpuView::Normal;
                    }
                    if key_handler.pressed(KeyCode::Key3) {
                        settings.view = WgpuView::WorldPos;
                    }
                    if key_handler.pressed(KeyCode::Key4) {
                        settings.view = WgpuView::Radiance;
                    }
                    if key_handler.pressed(KeyCode::Key5) {
                        settings.view = WgpuView::ScreenSpace;
                    }
                    if key_handler.pressed(KeyCode::Key6) {
                        settings.view = WgpuView::SSAO;
                    }
                    if key_handler.pressed(KeyCode::Key7) {
                        settings.view = WgpuView::FilteredSSAO;
                    }
                }

                if scene_timer.elapsed_in_millis() >= 500.0 && key_handler.pressed(KeyCode::Space) {
                    if let Some(handle) = scene_id.take() {
                        renderer.get_scene_mut().remove_3d_scene(handle);
                        scene_id = None;
                    } else {
                        let mut handle = renderer.get_scene_mut().add_3d_scene(&cesium_man);
                        handle.get_transform().translate(Vec3::new(-6.0, 0.0, 0.0));
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
                    camera_3d.translate_target(view_change);
                }
                if pos_change != [0.0; 3].into() {
                    camera_3d.translate_relative(pos_change);
                }

                if resized || scale_factor_changed {
                    renderer.resize(&window, (width, height), Some(scale_factor as f64));
                    camera_2d =
                        Camera2D::from_width_height(width, height, Some(scale_factor as f64));
                    camera_3d.set_aspect_ratio(width as f32 / height as f32);
                    resized = false;
                    scale_factor_changed = false;
                }

                renderer
                    .get_scene_mut()
                    .get_lights_mut()
                    .spot_lights
                    .iter_mut()
                    .enumerate()
                    .for_each(|(_, (i, sl))| {
                        let direction = Vec3::from(sl.direction);
                        let direction = Quat::from_rotation_y(
                            (elapsed / 10.0 + (i as f32 * 30.0)).to_radians(),
                        )
                        .mul_vec3(direction);
                        sl.direction = direction.into();
                    });

                let time = app_time.elapsed_in_millis() / 1000.0;
                renderer.set_animation_time(&cesium_man1, time);
                renderer.set_animation_time(&cesium_man2, time / 2.0);
                if let Some(cesium_man3) = &scene_id {
                    renderer.set_animation_time(cesium_man3, time / 3.0);
                }

                {
                    let mut instances_3d = 0;
                    let mut vertices = 0;
                    {
                        let scene = renderer.get_scene();

                        for (i, m) in scene.get_objects().meshes_3d.iter() {
                            instances_3d += scene.get_objects().instances_3d[i].len();
                            vertices +=
                                m.vertices.len() * scene.get_objects().instances_3d[i].len();
                        }
                    }
                    let meshes_3d = renderer.get_scene().get_objects().meshes_3d.len();
                    let instances_2d: usize = renderer
                        .get_scene()
                        .get_objects()
                        .instances_2d
                        .iter()
                        .map(|(_, i)| i.len())
                        .sum();
                    let meshes_2d = renderer.get_scene().get_objects().meshes_2d.len();

                    let settings = renderer.get_settings();
                    settings.draw_ui(&window, |ui| {
                        use rfw_backend_wgpu::imgui;
                        let window = imgui::Window::new(imgui::im_str!("RFW"));
                        window
                            .size([350.0, 250.0], imgui::Condition::FirstUseEver)
                            .position([900.0, 25.0], imgui::Condition::FirstUseEver)
                            .build(&ui, || {
                                ui.text(imgui::im_str!("3D Vertex count: {}", vertices));
                                ui.text(imgui::im_str!("3D Instance count: {}", instances_3d));
                                ui.text(imgui::im_str!("3D Mesh count: {}", meshes_3d));
                                ui.text(imgui::im_str!("2D Instance count: {}", instances_2d));
                                ui.text(imgui::im_str!("2D Mesh count: {}", meshes_2d));
                                scale_factor_changed = ui
                                    .input_float(imgui::im_str!("Scale factor"), &mut scale_factor)
                                    .step(0.05)
                                    .build();
                                scale_factor = scale_factor.max(0.1).min(2.0);
                            });
                    });
                }

                let t = app_time.elapsed_in_millis() / 1000.0;
                handles.par_iter_mut().enumerate().for_each(|(i, h)| {
                    let x = (i as i32 % (sphere_x * 2)) - sphere_x;
                    let z = (i as i32 / (sphere_x * 2)) - sphere_z;
                    let _x = (((x + sphere_x) as f32) + t).sin();
                    let _z = (((z + sphere_z) as f32) + t).sin();
                    let height = (_z + _x) * 0.5 + 1.0;

                    h.set_matrix(Mat4::from_translation(Vec3::new(
                        x as f32,
                        0.3 + height,
                        z as f32,
                    )));
                });

                if let Err(e) = renderer.render(&camera_2d, &camera_3d, RenderMode::Reset) {
                    eprintln!("Error while rendering: {}", e);
                    *control_flow = ControlFlow::Exit;
                }
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
