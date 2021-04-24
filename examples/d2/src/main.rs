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

use rfw::prelude::*;
use rfw_font::{FontRenderer, Section, Text};

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
    use rfw_backend_wgpu::WgpuBackend;

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
    let mut renderer: Instance = Instance::new(
        WgpuBackend::init(&window, (width, height), scale_factor)?,
        (width, height),
        Some(scale_factor),
    )?
    .with_plugin(FontRenderer::from_bytes(&font[0..font.len()]))
    .with_system(fps_system.system());

    renderer.spawn().insert(FpsSystem::default());

    let mut key_handler = KeyHandler::new();
    let mut mouse_button_handler = MouseButtonHandler::new();

    let quad = renderer.get_scene_mut().add_2d(Quad2D {
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

                let mut pos_change = Vec3::new(0.0, 0.0, 0.0);

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
                timer.reset();

                fullscreen_timer += elapsed;

                let elapsed = if key_handler.pressed(KeyCode::LShift) {
                    elapsed * 2.0
                } else {
                    elapsed
                };

                let pos_change = pos_change * elapsed * 0.01;

                if pos_change != Vec3::ZERO {
                    quad_instance.get_transform().translate(Vec3::new(
                        pos_change.x,
                        pos_change.z,
                        0.0,
                    ));
                }

                if resized {
                    *renderer.get_camera_2d() =
                        Camera2D::from_width_height(width, height, Some(scale_factor));
                    renderer.resize((width, height), None);
                    resized = false;
                }

                renderer.render();
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
