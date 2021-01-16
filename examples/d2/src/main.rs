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

use glyph_brush::ab_glyph::point;
use rayon::prelude::*;
use rfw::{
    backend::RenderMode,
    math::*,
    prelude::*,
    scene::{r2d::Mesh2D, Camera},
    utils, Instance,
};
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

    let mut renderer: Instance<T> =
        Instance::new(&window, (width, height), Some(scale_factor)).unwrap();

    let mut key_handler = KeyHandler::new();
    let mut mouse_button_handler = MouseButtonHandler::new();

    let mut camera = Camera::new()
        .with_aspect_ratio(1280.0 / 720.0)
        .with_fov(60.0);

    let mut timer = utils::Timer::new();
    let mut timer2 = utils::Timer::new();
    let mut fps = utils::Averager::new();
    let mut render = utils::Averager::new();

    let mut resized = false;

    use glyph_brush::{
        ab_glyph::FontArc, BrushAction, BrushError, GlyphBrushBuilder, Section, Text,
    };
    let font = include_bytes!("../../../assets/good-times-rg.ttf");
    let roboto = FontArc::try_from_slice(font)?;
    let mut glyph_brush = GlyphBrushBuilder::using_font(roboto).build();

    let tex = renderer
        .get_scene_mut()
        .add_texture(rfw::prelude::Texture::default());
    let d2_mesh = renderer.get_scene_mut().add_2d_object(Mesh2D::new(
        vec![
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
            [-0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
        ],
        vec![
            [0.01, 0.01],
            [0.99, 0.01],
            [0.99, 0.99],
            [0.01, 0.99],
            [0.01, 0.01],
            [0.99, 0.99],
        ],
        Some(tex),
        [1.0; 4],
    ));

    let d2_mesh2 = renderer.get_scene_mut().add_2d_object(Mesh2D::new(
        vec![
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
            [-0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
        ],
        vec![
            [0.01, 0.01],
            [0.99, 0.01],
            [0.99, 0.99],
            [0.01, 0.99],
            [0.01, 0.01],
            [0.99, 0.99],
        ],
        Some(tex),
        [1.0; 4],
    ));

    let (mut tex_width, mut tex_height) = glyph_brush.texture_dimensions();
    let mut tex_data = vec![0_u32; (tex_width * tex_height) as usize];

    let mut d2_inst = renderer.get_scene_mut().add_2d_instance(d2_mesh)?;
    d2_inst.set_matrix(Mat4::orthographic_lh(
        0.0,
        width as f32,
        height as f32,
        0.0,
        1.0,
        -1.0,
    ));

    let mut d2_inst = renderer.get_scene_mut().add_2d_instance(d2_mesh2)?;
    d2_inst.set_matrix(Mat4::from_scale(Vec3::new(
        1.0 / (width as f32 / height as f32),
        1.0,
        1.0,
    )));

    timer2.reset();

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
                    camera.set_aspect_ratio(width as f32 / height as f32);
                }

                if resized {
                    renderer.resize(&window, (width, height), None);

                    resized = false;

                    d2_inst.set_matrix(Mat4::orthographic_lh(
                        0.0,
                        width as f32,
                        height as f32,
                        0.0,
                        1.0,
                        -1.0,
                    ));
                }

                glyph_brush.queue(
                    Section::default()
                        .with_screen_position((0.0, 0.0))
                        .add_text(
                            Text::new(
                                format!("FPS: {:.2}\nRender: {:.2} ms", fps_avg, render_avg)
                                    .as_str(),
                            )
                            .with_scale(32.0)
                            .with_color([1.0; 4]),
                        ),
                );

                let mut tex_changed = false;
                match glyph_brush.process_queued(
                    |rect, t_data| {
                        let offset: [u32; 2] = [rect.min[0], rect.min[1]];
                        let size: [u32; 2] = [rect.width(), rect.height()];

                        let width = size[0] as usize;
                        let height = size[1] as usize;

                        for y in 0..height {
                            for x in 0..width {
                                let index = x + y * width;
                                let alpha = t_data[index] as u32;

                                let index = (x + offset[0] as usize)
                                    + (offset[1] as usize + y) * tex_width as usize;

                                tex_data[index] = (alpha << 24) | 0xFFFFFF;
                            }
                        }

                        tex_changed = true;
                    },
                    to_vertex,
                ) {
                    Ok(BrushAction::Draw(vertices)) => {
                        let mut verts = Vec::with_capacity(vertices.len() * 6);
                        let vertices: Vec<_> = vertices
                            .par_iter()
                            .map(|v| {
                                let v0 = Vertex2D {
                                    vertex: [v.min_x, v.min_y, 0.5],
                                    uv: [v.uv_min_x, v.uv_min_y],
                                    has_tex: tex as u32,
                                    color: v.color,
                                };
                                let v1 = Vertex2D {
                                    vertex: [v.max_x, v.min_y, 0.5],
                                    uv: [v.uv_max_x, v.uv_min_y],
                                    has_tex: tex as u32,
                                    color: v.color,
                                };
                                let v2 = Vertex2D {
                                    vertex: [v.max_x, v.max_y, 0.5],
                                    uv: [v.uv_max_x, v.uv_max_y],
                                    has_tex: tex as u32,
                                    color: v.color,
                                };
                                let v3 = Vertex2D {
                                    vertex: [v.min_x, v.max_y, 0.5],
                                    uv: [v.uv_min_x, v.uv_max_y],
                                    has_tex: tex as u32,
                                    color: v.color,
                                };

                                (v0, v1, v2, v3, v0, v2)
                            })
                            .collect();
                        vertices.into_iter().for_each(|vs| {
                            verts.push(vs.0);
                            verts.push(vs.1);
                            verts.push(vs.2);
                            verts.push(vs.3);
                            verts.push(vs.4);
                            verts.push(vs.5);
                        });

                        let mut mesh = Mesh2D::from(verts);
                        mesh.tex_id = Some(tex);
                        renderer
                            .get_scene_mut()
                            .set_2d_object(d2_mesh, mesh)
                            .unwrap();
                    }
                    Ok(BrushAction::ReDraw) => {}
                    Err(BrushError::TextureTooSmall { suggested }) => {
                        tex_data.resize((suggested.0 * suggested.1) as usize, 0);
                        tex_width = suggested.0;
                        tex_height = suggested.1;
                    }
                }

                if tex_changed {
                    renderer
                        .get_scene_mut()
                        .set_texture(
                            tex,
                            rfw::prelude::Texture {
                                width: tex_width as u32,
                                height: tex_height as u32,
                                data: tex_data.clone(),
                                mip_levels: 1,
                            },
                        )
                        .unwrap();
                }

                timer2.reset();

                timer2.reset();
                renderer.render(&camera, RenderMode::Reset).unwrap();
                render.add_sample(timer2.elapsed_in_millis());
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
                mouse_button_handler.insert(button, state);
            }
            _ => (),
        }
    });
}

#[derive(Debug, Copy, Clone)]
struct BrushVertex {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub uv_min_x: f32,
    pub uv_min_y: f32,
    pub uv_max_x: f32,
    pub uv_max_y: f32,
    pub color: [f32; 4],
}

#[inline]
fn to_vertex(
    glyph_brush::GlyphVertex {
        mut tex_coords,
        pixel_coords,
        bounds,
        extra,
    }: glyph_brush::GlyphVertex,
) -> BrushVertex {
    let gl_bounds = bounds;

    use glyph_brush::ab_glyph::Rect;

    let mut gl_rect = Rect {
        min: point(pixel_coords.min.x as f32, pixel_coords.min.y as f32),
        max: point(pixel_coords.max.x as f32, pixel_coords.max.y as f32),
    };
    //
    // // handle overlapping bounds, modify uv_rect to preserve texture aspect
    if gl_rect.max.x > gl_bounds.max.x {
        let old_width = gl_rect.width();
        gl_rect.max.x = gl_bounds.max.x;
        tex_coords.max.x = tex_coords.min.x + tex_coords.width() * gl_rect.width() / old_width;
    }
    //
    if gl_rect.min.x < gl_bounds.min.x {
        let old_width = gl_rect.width();
        gl_rect.min.x = gl_bounds.min.x;
        tex_coords.min.x = tex_coords.max.x - tex_coords.width() * gl_rect.width() / old_width;
    }

    if gl_rect.max.y > gl_bounds.max.y {
        let old_height = gl_rect.height();
        gl_rect.max.y = gl_bounds.max.y;
        tex_coords.max.y = tex_coords.min.y + tex_coords.height() * gl_rect.height() / old_height;
    }

    if gl_rect.min.y < gl_bounds.min.y {
        let old_height = gl_rect.height();
        gl_rect.min.y = gl_bounds.min.y;
        tex_coords.min.y = tex_coords.max.y - tex_coords.height() * gl_rect.height() / old_height;
    }

    BrushVertex {
        min_x: gl_rect.min.x,
        min_y: gl_rect.min.y,
        max_x: gl_rect.max.x,
        max_y: gl_rect.max.y,
        uv_min_x: tex_coords.min.x,
        uv_min_y: tex_coords.min.y,
        uv_max_x: tex_coords.max.x,
        uv_max_y: tex_coords.max.y,
        color: extra.color,
    }
}
