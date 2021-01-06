#![allow(dead_code)]

use std::collections::HashMap;
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

use rfw::{
    backend::RenderMode,
    math::*,
    scene::{self},
    system::RenderSystem,
    utils,
};
use rfw_backend_wgpu::{WgpuBackend, WgpuView};
use rfw_font::*;
use winit::window::Fullscreen;

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

    let scale_factor: f64 = window
        .current_monitor()
        .and_then(|m| Some(1.0 / m.scale_factor()))
        .unwrap_or(1.0);

    let mut renderer: RenderSystem<WgpuBackend> =
        RenderSystem::new(&window, (width, height), Some(scale_factor)).unwrap();

    let mut key_handler = KeyHandler::new();
    let mut mouse_button_handler = MouseButtonHandler::new();

    let cam_id = renderer.create_camera(width as u32, height as u32);

    let mut timer = utils::Timer::new();
    let mut timer2 = utils::Timer::new();
    let mut fps = utils::Averager::with_capacity(250);
    let mut render = utils::Averager::new();
    let mut synchronize = utils::Averager::new();

    let mut resized = false;

    let font = include_bytes!("../../../assets/good-times-rg.ttf");
    let mut font =
        Font::from_bytes(&mut renderer, &font[0..font.len()]).expect("Could not initialize font.");

    renderer.add_spot_light(
        Vec3::new(0.0, 15.0, 0.0),
        Vec3::new(0.0, -1.0, 0.3),
        Vec3::new(105.0, 100.0, 110.0),
        45.0,
        60.0,
    );

    let cesium_man = renderer
        .load("assets/models/CesiumMan/CesiumMan.gltf")?
        .scene()
        .unwrap();

    let mut cesium_man1 =
        scene::graph::NodeGraph::from_scene_descriptor(&cesium_man, &mut renderer.scene.instances);
    let mut cesium_man2 =
        scene::graph::NodeGraph::from_scene_descriptor(&cesium_man, &mut renderer.scene.instances);

    for node in cesium_man1.iter_root_nodes_mut() {
        node.set_scale(Vec3::splat(3.0));
    }

    for node in cesium_man2.iter_root_nodes_mut() {
        node.translate(Vec3::new(-3.0, 0.0, 0.0));
    }

    let cesium_man1 = renderer.add_scene(cesium_man1);
    let cesium_man2 = renderer.add_scene(cesium_man2);

    let pica_desc = renderer
        .load("assets/models/pica/scene.gltf")?
        .scene()
        .unwrap();
    let mut pica = scene::graph::NodeGraph::new();
    pica.load_scene_descriptor(&pica_desc, &mut renderer.scene.instances);
    renderer.add_scene(pica);

    let app_time = utils::Timer::new();

    timer2.reset();
    renderer.set_animations_time(0.0);
    renderer.synchronize();
    synchronize.add_sample(timer2.elapsed_in_millis());

    let mut scene_timer = utils::Timer::new();
    let mut scene_id = None;

    renderer.get_settings().setup_imgui(&window);

    let mut fullscreen_timer = 0.0;
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
                    if let Some(id) = scene_id {
                        renderer.remove_scene(id).unwrap();
                        scene_id = None;
                    } else {
                        let mut cesium_man3 = scene::graph::NodeGraph::from_scene_descriptor(
                            &cesium_man,
                            &mut renderer.scene.instances,
                        );
                        for node in cesium_man3.iter_root_nodes_mut() {
                            node.translate(Vec3::new(-6.0, 0.0, 0.0));
                        }
                        scene_id = Some(renderer.add_scene(cesium_man3));
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
                let fps_avg = fps.get_average();
                let render_avg = render.get_average();
                let sync_avg = synchronize.get_average();

                let elapsed = if key_handler.pressed(KeyCode::LShift) {
                    elapsed * 2.0
                } else {
                    elapsed
                };

                timer.reset();

                let view_change = view_change * elapsed * 0.001;
                let pos_change = pos_change * elapsed * 0.01;

                if let Some(camera) = renderer.get_camera_mut(cam_id) {
                    if view_change != [0.0; 3].into() {
                        camera.translate_target(view_change);
                    }
                    if pos_change != [0.0; 3].into() {
                        camera.translate_relative(pos_change);
                    }
                }

                if resized {
                    font.resize(&mut renderer);
                    renderer.resize(&window, (width, height), None);
                    renderer.get_camera_mut(cam_id).map(|c| {
                        c.resize(width as u32, height as u32);
                    });
                    resized = false;
                }

                font.draw(
                    Section::default()
                        .with_screen_position((0.0, 0.0))
                        .add_text(
                            Text::new(
                                format!(
                                    "FPS: {:.2}\nRender: {:.2} ms\nSynchronize: {:.2} ms",
                                    fps_avg, render_avg, sync_avg
                                )
                                .as_str(),
                            )
                            .with_scale(32.0)
                            .with_color([1.0; 4]),
                        ),
                );

                {
                    let lights = renderer.get_lights_mut();
                    lights.spot_lights.iter_mut().for_each(|(_, sl)| {
                        let direction = Vec3::from(sl.direction);
                        let direction = Quat::from_rotation_y((elapsed / 10.0).to_radians())
                            .mul_vec3(direction);
                        sl.direction = direction.into();
                    });
                }

                timer2.reset();
                let time = app_time.elapsed_in_millis() / 1000.0;
                renderer.set_animation_time(cesium_man1, time);
                renderer.set_animation_time(cesium_man2, time / 2.0);
                if let Some(cesium_man3) = scene_id {
                    renderer.set_animation_time(cesium_man3, time / 3.0);
                }

                font.synchronize(&mut renderer);
                renderer.synchronize();
                synchronize.add_sample(timer2.elapsed_in_millis());

                timer2.reset();

                {
                    let instances = renderer.scene.instances.len();
                    let meshes = renderer.scene.objects.meshes.len();
                    let settings = renderer.get_settings();

                    settings.draw_ui(&window, |ui| {
                        use rfw_backend_wgpu::imgui;
                        let window = imgui::Window::new(imgui::im_str!("RFW"));
                        window
                            .size([350.0, 250.0], imgui::Condition::FirstUseEver)
                            .position([900.0, 25.0], imgui::Condition::FirstUseEver)
                            .build(&ui, || {
                                ui.plot_histogram(
                                    &*imgui::im_str!("FPS {:.2} ms", fps_avg),
                                    fps.data(),
                                )
                                .graph_size([0.0, 50.0])
                                .build();
                                ui.text(imgui::im_str!("Synchronize: {:.2} ms", sync_avg));
                                ui.text(imgui::im_str!("Render: {:.2} ms", render_avg));
                                ui.separator();
                                ui.text(imgui::im_str!("Instance count: {}", instances));
                                ui.text(imgui::im_str!("Mesh count: {}", meshes));
                            });
                    });
                }

                if let Err(e) = renderer.render(cam_id, RenderMode::Reset) {
                    eprintln!("Error while rendering: {}", e);
                    *control_flow = ControlFlow::Exit;
                }
                render.add_sample(timer2.elapsed_in_millis());
            }
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                window_id,
            } if *window_id == window.id() => {
                if let Some(key) = input.virtual_keycode {
                    key_handler.insert(key, input.state);
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
                mouse_button_handler.insert(*button, *state);
            }
            _ => (),
        }
    });
}
