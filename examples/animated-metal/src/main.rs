#![allow(dead_code)]

use std::collections::HashMap;
use std::error::Error;

use clap::App;
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
    ecs::System,
    math::*,
    prelude::{Averager, Camera, Timer},
    utils, Instance,
};
use rfw_backend_metal::MetalBackend;
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

impl Default for MouseButtonHandler {
    fn default() -> Self {
        Self {
            states: Default::default(),
        }
    }
}

impl MouseButtonHandler {
    pub fn new() -> MouseButtonHandler {
        Self::default()
    }

    pub fn insert(&mut self, key: MouseButtonCode, state: ElementState) {
        self.states.insert(key, state == ElementState::Pressed);
    }

    pub fn pressed(&self, key: MouseButtonCode) -> bool {
        if let Some(state) = self.states.get(&key) {
            return *state;
        }
        false
    }
}

#[derive(Debug)]
struct FpsCounter {
    timer: Timer,
    average: Averager<f32>,
}

#[derive(Debug, Default)]
struct FpsSystem {}

impl Default for FpsCounter {
    fn default() -> Self {
        Self {
            timer: Timer::new(),
            average: Averager::with_capacity(250),
        }
    }
}

impl System for FpsSystem {
    fn run(&mut self, resources: &rfw::resources::ResourceList) {
        if let Some(mut counter) = resources.get_resource_mut::<FpsCounter>() {
            let elapsed = counter.timer.elapsed_in_millis();
            counter.timer.reset();
            counter.average.add_sample(elapsed);
            let average = counter.average.get_average();

            if let Some(mut font) = resources.get_resource_mut::<FontRenderer>() {
                font.draw(
                    Section::default()
                        .with_screen_position((0.0, 0.0))
                        .add_text(
                            Text::new(
                                format!(
                                    "FPS: {:.2}\nFrametime: {:.2} ms",
                                    1000.0 / average,
                                    average
                                )
                                .as_str(),
                            )
                            .with_scale(32.0)
                            .with_color([1.0; 4]),
                        ),
                );
            }
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let _ = App::new("rfw-animated")
        .about("Example with animated meshes for the rfw framework.")
        .get_matches();

    run_backend()
}

fn run_backend() -> Result<(), Box<dyn Error>> {
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
        .map(|m| 1.0 / m.scale_factor())
        .unwrap_or(1.0);

    let font = include_bytes!("../../../assets/good-times-rg.ttf");
    let mut renderer: Instance<MetalBackend> =
        Instance::new(&window, (width, height), Some(scale_factor))
            .unwrap()
            .with_plugin(FontRenderer::from_bytes(&font[0..font.len()]))
            .with_resource(FpsCounter::default())
            .with_system(FpsSystem::default());

    let mut key_handler = KeyHandler::new();
    let mut mouse_button_handler = MouseButtonHandler::new();

    let mut camera = Camera::new().with_aspect_ratio(width as f32 / height as f32);

    let mut timer = utils::Timer::new();

    let mut resized = false;

    renderer.add_spot_light(
        Vec3::new(0.0, 15.0, 0.0),
        Vec3::new(0.0, -1.0, 0.3),
        Vec3::new(105.0, 100.0, 110.0),
        45.0,
        60.0,
    );

    let cesium_man = renderer
        .get_scene_mut()
        .load("assets/models/CesiumMan/CesiumMan.gltf")?
        .scene()
        .unwrap();

    let mut cesium_man1 = renderer.get_scene_mut().add_3d_scene(&cesium_man);
    cesium_man1
        .get_transform()
        .set_scale(Vec3::splat(3.0))
        .rotate_y(180.0_f32.to_radians());
    let mut cesium_man2 = renderer.get_scene_mut().add_3d_scene(&cesium_man);
    cesium_man2
        .get_transform()
        .translate(Vec3::new(-3.0, 0.0, 0.0))
        .rotate_y(180.0_f32.to_radians());

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

    let mut fullscreen_timer = 0.0;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match &event {
            Event::MainEventsCleared => {
                if key_handler.pressed(KeyCode::Escape) {
                    *control_flow = ControlFlow::Exit;
                }

                if scene_timer.elapsed_in_millis() >= 500.0 && key_handler.pressed(KeyCode::Space) {
                    if let Some(handle) = scene_id.take() {
                        renderer.get_scene_mut().remove_3d_scene(handle);
                        scene_id = None;
                    } else {
                        let mut handle = renderer.get_scene_mut().add_3d_scene(&cesium_man);
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
                    camera.translate_target(view_change);
                }
                if pos_change != [0.0; 3].into() {
                    camera.translate_relative(pos_change);
                }

                if resized {
                    renderer.resize(&window, (width, height), None);
                    camera.set_aspect_ratio(width as f32 / height as f32);
                    resized = false;
                }

                if let Some(mut fps) = renderer.resources.get_resource_mut::<FpsCounter>() {
                    let average = fps.average.get_average();
                    window.set_title(format!("FPS: {}", 1000.0 / average).as_str());
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

                if let Err(e) = renderer.render(&camera, RenderMode::Reset) {
                    eprintln!("Error while rendering: {}", e);
                    *control_flow = ControlFlow::Exit;
                }
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
