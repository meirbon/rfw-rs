use clap::{App, Arg};
use rfw::{ecs::System, prelude::*, Instance};
use rfw_font::{FontRenderer, Section, Text};
use std::error::Error;
pub use winit::event::MouseButton as MouseButtonCode;
pub use winit::event::VirtualKeyCode as KeyCode;
use winit::window::Fullscreen;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

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
    fn run(&mut self, resources: &rfw::resources::ResourceList) {
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

    use rfw_backend_wgpu::WgpuBackend;
    // use rfw_gpu_rt::RayTracer;

    match matches.value_of("renderer") {
        // Some("gpu-rt") => run_application::<RayTracer>(),
        // Some("gfx") => run_application::<GfxBackend>(),
        _ => run_application::<WgpuBackend>(),
    }
}

fn run_application<T: 'static + Sized + Backend>() -> Result<(), Box<dyn Error>> {
    let mut width = 1280;
    let mut height = 720;

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

    let font = include_bytes!("../../../assets/good-times-rg.ttf");
    let mut renderer: Instance<T> = Instance::new(&window, (width, height), Some(scale_factor))
        .unwrap()
        .with_plugin(FontRenderer::from_bytes(&font[0..font.len()]))
        .with_system(FpsSystem::default());

    let mut key_handler = KeyHandler::new();
    let mut mouse_button_handler = MouseButtonHandler::new();

    let mut camera_2d = Camera2D::from_width_height(width, height, Some(scale_factor));
    let mut camera = Camera3D::new()
        .with_aspect_ratio(1280.0 / 720.0)
        .with_fov(60.0);

    let quad = renderer.get_scene_mut().add_2d_object(Quad2D {
        bottom_left: Vec2::splat(-40.0),
        top_right: Vec2::splat(40.0),
        ..Default::default()
    });

    let mut quad_instance = renderer.get_scene_mut().add_2d_instance(quad)?;
    let mut timer = Timer::new();
    let mut resized = false;
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
                timer.reset();

                fullscreen_timer += elapsed;

                let elapsed = if key_handler.pressed(KeyCode::LShift) {
                    elapsed * 2.0
                } else {
                    elapsed
                };

                let view_change = view_change * elapsed * 0.001;
                let pos_change = pos_change * elapsed * 0.01;

                if view_change != Vec3::zero() {
                    camera.translate_target(view_change);
                }
                if pos_change != Vec3::zero() {
                    camera.translate_relative(pos_change);
                    quad_instance.get_transform().translate(Vec3::new(
                        pos_change.x,
                        pos_change.z,
                        0.0,
                    ));
                }

                if resized {
                    camera.set_aspect_ratio(width as f32 / height as f32);
                    camera_2d = Camera2D::from_width_height(width, height, Some(scale_factor));
                    renderer.resize(&window, (width, height), None);
                    resized = false;
                }

                renderer
                    .render(&camera_2d, &camera, RenderMode::Reset)
                    .unwrap();
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                window_id,
            } if window_id == window.id() => {
                width = size.width as u32;
                height = size.height as u32;

                resized = true;
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
