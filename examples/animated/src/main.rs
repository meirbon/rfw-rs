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

use rayon::prelude::*;
use rfw_gfx::GfxBackend;
use rfw_system::{
    scene::r2d::{D2Mesh, D2Vertex},
    scene::Texture,
    scene::{
        self,
        renderers::{RenderMode, Setting, SettingValue},
        Renderer,
    },
    RenderSystem,
};
use rfw_utils::prelude::*;
use shared::utils;
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

    use rfw_deferred::Deferred;
    use rfw_gpu_rt::RayTracer;

    match matches.value_of("renderer") {
        Some("gpu-rt") => run_application::<RayTracer>(),
        Some("gfx") => run_application::<GfxBackend>(),
        _ => run_application::<Deferred>(),
    }
}

fn run_application<T: 'static + Sized + Renderer>() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("rfw-rs")
        .with_inner_size(LogicalSize::new(1280_f64, 720_f64))
        .build(&event_loop)
        .unwrap();

    let mut width = window.inner_size().width as usize;
    let mut height = window.inner_size().height as usize;

    let res_scale = if let Some(m) = window.current_monitor() {
        1.0 / m.scale_factor()
    } else {
        1.0
    };
    let mut render_width = (width as f64 * res_scale) as usize;
    let mut render_height = (height as f64 * res_scale) as usize;

    let mut renderer = RenderSystem::new(&window, (width, height), (render_width, render_height))
        .unwrap() as RenderSystem<T>;

    let mut key_handler = KeyHandler::new();
    let mut mouse_button_handler = MouseButtonHandler::new();

    let cam_id = renderer.create_camera(render_width as u32, render_height as u32);

    let mut timer = utils::Timer::new();
    let mut timer2 = utils::Timer::new();
    let mut fps = utils::Averager::new();
    let mut render = utils::Averager::new();
    let mut synchronize = utils::Averager::new();

    let mut resized = false;

    use glyph_brush::{
        ab_glyph::FontArc, BrushAction, BrushError, GlyphBrushBuilder, Section, Text,
    };
    let font = include_bytes!("../../../assets/good-times-rg.ttf");
    let roboto = FontArc::try_from_slice(font)?;
    let mut glyph_brush = GlyphBrushBuilder::using_font(roboto).build();

    let tex = renderer.add_texture(Texture::default())?;
    let d2_mesh = renderer.add_2d_object(D2Mesh::new(
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
    ))?;

    let (mut tex_width, mut tex_height) = glyph_brush.texture_dimensions();
    let mut tex_data = vec![0_u32; (tex_width * tex_height) as usize];

    let d2_inst = renderer.create_2d_instance(d2_mesh)?;
    if let Some(inst) = renderer.get_2d_instance_mut(d2_inst) {
        inst.transform =
            Mat4::orthographic_lh(0.0, width as f32, height as f32, 0.0, 1.0, -1.0).to_cols_array();
    }

    renderer.add_spot_light(
        Vec3::new(0.0, 15.0, 0.0),
        Vec3::new(0.0, -1.0, 0.3),
        Vec3::new(105.0, 100.0, 110.0),
        45.0,
        60.0,
    );

    let cesium_man = renderer
        .load("models/CesiumMan/CesiumMan.gltf")?
        .scene()
        .unwrap();

    let mut cesium_man1 = scene::graph::NodeGraph::from_scene_descriptor(
        &cesium_man,
        &mut renderer.scene.objects.instances,
    );
    let mut cesium_man2 = scene::graph::NodeGraph::from_scene_descriptor(
        &cesium_man,
        &mut renderer.scene.objects.instances,
    );

    for node in cesium_man1.iter_root_nodes_mut() {
        node.set_scale(Vec3::splat(3.0));
    }

    for node in cesium_man2.iter_root_nodes_mut() {
        node.translate(Vec3::new(-3.0, 0.0, 0.0));
    }

    let cesium_man1 = renderer.add_scene(cesium_man1);
    let cesium_man2 = renderer.add_scene(cesium_man2);

    let pica_desc = renderer.load("models/pica/scene.gltf")?.scene().unwrap();
    let mut pica = scene::graph::NodeGraph::new();
    pica.load_scene_descriptor(&pica_desc, &mut renderer.scene.objects.instances);
    renderer.add_scene(pica);

    let settings: Vec<Setting> = renderer.get_settings().unwrap();

    let app_time = utils::Timer::new();

    timer2.reset();
    renderer.set_animations_time(0.0);
    renderer.synchronize();
    synchronize.add_sample(timer2.elapsed_in_millis());

    let mut scene_timer = utils::Timer::new();
    let mut scene_id = None;

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

                if !settings.is_empty() {
                    let mut value = None;
                    if key_handler.pressed(KeyCode::Key0) {
                        value = Some(0);
                    }
                    if key_handler.pressed(KeyCode::Key1) {
                        value = Some(1);
                    }
                    if key_handler.pressed(KeyCode::Key2) {
                        value = Some(2);
                    }
                    if key_handler.pressed(KeyCode::Key3) {
                        value = Some(3);
                    }
                    if key_handler.pressed(KeyCode::Key4) {
                        value = Some(4);
                    }
                    if key_handler.pressed(KeyCode::Key5) {
                        value = Some(5);
                    }
                    if key_handler.pressed(KeyCode::Key6) {
                        value = Some(6);
                    }
                    if key_handler.pressed(KeyCode::Key7) {
                        value = Some(7);
                    }

                    if let Some(value) = value {
                        let mut setting: Setting = settings[0].clone();
                        setting.set(SettingValue::Int(value));
                        renderer.set_setting(setting).unwrap();
                    }
                }

                if scene_timer.elapsed_in_millis() >= 500.0 && key_handler.pressed(KeyCode::Space) {
                    if let Some(id) = scene_id {
                        renderer.remove_scene(id).unwrap();
                        scene_id = None;
                    } else {
                        let mut cesium_man3 = scene::graph::NodeGraph::from_scene_descriptor(
                            &cesium_man,
                            &mut renderer.scene.objects.instances,
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
                    renderer.resize(&window, (width, height), (render_width, render_height));
                    renderer.get_camera_mut(cam_id).map(|c| {
                        c.resize(render_width as u32, render_height as u32);
                    });
                    resized = false;

                    if let Some(inst) = renderer.get_2d_instance_mut(d2_inst) {
                        inst.transform =
                            Mat4::orthographic_lh(0.0, width as f32, height as f32, 0.0, 1.0, -1.0)
                                .to_cols_array();
                    }
                }

                glyph_brush.queue(
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
                                let v0 = D2Vertex {
                                    vertex: [v.min_x, v.min_y, 0.5],
                                    uv: [v.uv_min_x, v.uv_min_y],
                                    has_tex: tex,
                                    color: v.color,
                                };
                                let v1 = D2Vertex {
                                    vertex: [v.max_x, v.min_y, 0.5],
                                    uv: [v.uv_max_x, v.uv_min_y],
                                    has_tex: tex,
                                    color: v.color,
                                };
                                let v2 = D2Vertex {
                                    vertex: [v.max_x, v.max_y, 0.5],
                                    uv: [v.uv_max_x, v.uv_max_y],
                                    has_tex: tex,
                                    color: v.color,
                                };
                                let v3 = D2Vertex {
                                    vertex: [v.min_x, v.max_y, 0.5],
                                    uv: [v.uv_min_x, v.uv_max_y],
                                    has_tex: tex,
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

                        let mut mesh = D2Mesh::from(verts);
                        mesh.tex_id = Some(tex);
                        renderer.set_2d_object(d2_mesh, mesh).unwrap();
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
                        .set_texture(
                            tex,
                            Texture {
                                width: tex_width as u32,
                                height: tex_height as u32,
                                data: tex_data.clone(),
                                mip_levels: 1,
                            },
                        )
                        .unwrap();
                }

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
                renderer.synchronize();
                synchronize.add_sample(timer2.elapsed_in_millis());

                timer2.reset();
                if let Err(e) = renderer.render(cam_id, RenderMode::Reset) {
                    eprintln!("Error while rendering: {}", e);
                    *control_flow = ControlFlow::Exit;
                }
                render.add_sample(timer2.elapsed_in_millis());
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                window_id,
            } if window_id == window.id() => {
                width = size.width as usize;
                height = size.height as usize;
                render_width = (width as f64 * res_scale) as usize;
                render_height = (height as f64 * res_scale) as usize;
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

    use glyph_brush::ab_glyph::{point, Rect};

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
