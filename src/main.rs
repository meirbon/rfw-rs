#![allow(dead_code)]
#![feature(clamp)]

use bvh::Ray;
use fb_template::{run_app, KeyCode, KeyHandler, Request};
use glam::*;
use rayon::prelude::*;

mod camera;
mod constants;
mod material;
mod objects;
mod scene;
mod triangle_scene;
mod utils;

use crate::constants::{DEFAULT_T_MAX, DEFAULT_T_MIN};
use bvh::RayPacket4;
use camera::*;
use material::*;
use objects::*;
use scene::*;
use utils::*;

#[derive(Debug, Copy, Clone)]
enum RenderMode {
    Scene,
    BVH,
}

struct App {
    pub width: u32,
    pub height: u32,

    p_width: usize,
    p_height: usize,
    pixels: Vec<Vec4>,
    camera: Camera,
    timer: Timer,
    scene: Scene,
    materials: MaterialList,
    render_mode: RenderMode,
    fps: Averager<f32>,
    num_threads: usize,
}

impl App {
    const PACKET_WIDTH: u32 = 4;
    const PACKET_HEIGHT: u32 = 1;

    pub fn new(width: u32, height: u32) -> App {
        let mut materials = MaterialList::new();
        let mut scene = Scene::new();

        #[cfg(not(debug_assertions))]
        {
            let dragon = Box::new(
                Obj::new("models/dragon.obj", &mut materials)
                    .unwrap()
                    .into_mesh()
                    .scale(50.0),
            );
            let dragon = scene.add_object(dragon);
            scene
                .add_instance(dragon, Mat4::from_translation(Vec3::new(0.0, 0.0, 200.0)))
                .unwrap();
        }

        let sphere_mat_id = materials.add(Vec3::new(1.0, 0.0, 0.0), 1.0, Vec3::one(), 1.0);
        let sphere = scene.add_object(Box::new(Sphere::new(Vec3::zero(), 10.0, sphere_mat_id)));

        (-2..3).for_each(|x| {
            (3..8).for_each(|z| {
                let matrix =
                    Mat4::from_translation(Vec3::new(x as f32 * 50.0, 0.0, z as f32 * 100.0));
                scene.add_instance(sphere, matrix).unwrap();
            })
        });

        let timer = utils::Timer::new();
        scene.build_bvh();
        println!("Building BVH: took {} ms", timer.elapsed_in_millis());

        let num_threads = num_cpus::get();
        App {
            width,
            height,
            p_width: width as usize,
            p_height: height as usize,
            pixels: vec![Vec4::zero(); (width * height) as usize],
            camera: Camera::new(width, height),
            timer: Timer::new(),
            scene,
            materials,
            render_mode: RenderMode::BVH,
            fps: Averager::with_capacity(25),
            num_threads,
        }
    }

    pub fn blit_pixels(&self, fb: &mut [u8]) {
        let line_chunk = 4 * self.width as usize;
        let pixels = &self.pixels;
        let pixels_width = self.p_width;
        let fb_iterator = fb.par_chunks_mut(line_chunk).enumerate();

        fb_iterator.for_each(|(y, fb_pixels)| {
            let line_iterator = fb_pixels.chunks_exact_mut(4).enumerate();
            for (x, pixel) in line_iterator {
                let color = unsafe { pixels.get_unchecked(x + y * pixels_width) };
                let color = color.max([0.0; 4].into()).min([1.0; 4].into());
                let red = (color.x() * 255.0) as u8;
                let green = (color.y() * 255.0) as u8;
                let blue = (color.z() * 255.0) as u8;
                pixel.copy_from_slice(&[red, green, blue, 0xff]);
            }
        });
    }

    fn render_bvh(&mut self) {
        let view = self.camera.get_view();
        let pixels = &mut self.pixels;
        let intersector = self.scene.create_intersector();
        let _materials = &self.materials;
        let width = self.p_width as usize;

        pixels
            .par_chunks_mut(width)
            .enumerate()
            .for_each(|(y, pixels)| {
                for x in 0..width {
                    let ray = view.generate_ray(x as u32, y as u32);
                    pixels[x] = {
                        let (_, depth) = intersector.depth_test(ray, DEFAULT_T_MIN, DEFAULT_T_MAX);
                        if depth == 0 {
                            Vec4::from([0.0; 4])
                        } else {
                            let r = if depth > 8 {
                                depth.min(48) as f32 * (1.0 / 64.0)
                            } else {
                                0.0
                            };
                            let g = (32 - depth.clamp(0, 32)) as f32 * (1.0 / 32.0);
                            let b = if depth > 4 {
                                depth as f32 * (1.0 / 128.0)
                            } else {
                                0.0
                            };
                            (r, g, b, 1.0).into()
                        }
                    };
                }
            });
    }

    fn render_scene(&mut self) {
        let view = self.camera.get_view();
        let pixels = &mut self.pixels;
        let intersector = self.scene.create_intersector();
        let materials = &self.materials;

        assert_eq!(Self::PACKET_WIDTH * Self::PACKET_HEIGHT, 4);

        let width = self.p_width;
        let height = self.p_height;

        let mut packets = (0..(width * height))
            .step_by(4)
            .map(|i| {
                let x = (i % width) as u32;
                let y = (i / width) as u32;
                view.generate_ray4(&[x, x + 1, x + 2, x + 3], &[y, y, y, y], width as u32)
            })
            .collect::<Vec<RayPacket4>>();

        let max_id = (self.p_width * self.p_height) as u32;

        packets
            .chunks_mut((width / 4) as usize)
            .zip(pixels.chunks_mut(width as usize))
            .par_bridge()
            .for_each(|(packet, output)| {
                let mut offset = 0;
                for p in 0..(width / 4) {
                    let packet: &mut RayPacket4 = &mut packet[p as usize];
                    let (instance_ids, prim_ids) =
                        intersector.intersect4(packet, [DEFAULT_T_MIN; 4]);

                    for i in 0..4 {
                        let pixel_id = packet.pixel_ids[i];
                        if pixel_id >= max_id {
                            continue;
                        }

                        let origin: [f32; 3] = [packet.origin_x[i], packet.origin_y[i], packet.origin_z[i]];
                        let direction: [f32; 3] = [packet.direction_x[i], packet.direction_y[i], packet.direction_z[i]];
                        let prim_id = prim_ids[i];
                        let instance_id = instance_ids[i];

                        output[i + offset] = if prim_id >= 0 || instance_id >= 0 {
                            let hit = intersector.get_hit_record(Ray { origin, direction }, packet.t[i], instance_id, prim_id);
                            Vec3::from(hit.normal).extend(1.0).max(Vec4::from([0.2; 4]))
                        } else {
                            Vec4::zero()
                        }
                    }
                    offset += 4;
                }
            });
    }
}

impl fb_template::App for App {
    fn render(&mut self, fb: &mut [u8]) -> Option<Request> {
        match self.render_mode {
            RenderMode::Scene => self.render_scene(),
            RenderMode::BVH => self.render_bvh(),
        };

        self.blit_pixels(fb);
        None
    }

    fn key_handling(&mut self, states: &KeyHandler) -> Option<Request> {
        if states.pressed(KeyCode::Escape) {
            return Some(Request::Exit);
        }

        let mut view_change = Vec3::new(0.0, 0.0, 0.0);
        let mut pos_change = Vec3::new(0.0, 0.0, 0.0);

        if states.pressed(KeyCode::Up) {
            view_change += (0.0, 1.0, 0.0).into();
        }
        if states.pressed(KeyCode::Down) {
            view_change -= (0.0, 1.0, 0.0).into();
        }
        if states.pressed(KeyCode::Left) {
            view_change -= (1.0, 0.0, 0.0).into();
        }
        if states.pressed(KeyCode::Right) {
            view_change += (1.0, 0.0, 0.0).into();
        }

        if states.pressed(KeyCode::W) {
            pos_change += (0.0, 0.0, 1.0).into();
        }
        if states.pressed(KeyCode::S) {
            pos_change -= (0.0, 0.0, 1.0).into();
        }
        if states.pressed(KeyCode::A) {
            pos_change -= (1.0, 0.0, 0.0).into();
        }
        if states.pressed(KeyCode::D) {
            pos_change += (1.0, 0.0, 0.0).into();
        }
        if states.pressed(KeyCode::E) {
            pos_change += (0.0, 1.0, 0.0).into();
        }
        if states.pressed(KeyCode::Q) {
            pos_change -= (0.0, 1.0, 0.0).into();
        }

        if states.pressed(KeyCode::Key1) {
            unsafe {
                crate::scene::USE_MBVH = true;
            }
        }

        if states.pressed(KeyCode::Key2) {
            unsafe {
                crate::scene::USE_MBVH = false;
            }
        }

        if states.pressed(KeyCode::B) {
            self.render_mode = RenderMode::BVH;
        }

        if states.pressed(KeyCode::N) {
            self.render_mode = RenderMode::Scene;
        }

        let elapsed = self.timer.elapsed_in_millis();
        let elapsed = if states.pressed(KeyCode::LShift) {
            elapsed * 2.0
        } else {
            elapsed
        };

        let view_change = view_change * elapsed * 0.001;
        let pos_change = pos_change * elapsed * 0.05;

        if view_change != [0.0; 3].into() {
            self.camera.translate_target(view_change);
        }
        if pos_change != [0.0; 3].into() {
            self.camera.translate_relative(pos_change);
        }

        let elapsed = self.timer.elapsed_in_millis();
        self.fps.add_sample(1000.0 / elapsed);
        let avg = self.fps.get_average();
        self.timer.reset();
        Some(Request::TitleChange(String::from(format!(
            "FPS: {:.2}",
            avg
        ))))
    }

    fn mouse_handling(&mut self, _x: f64, _y: f64, _dx: f64, _dy: f64) -> Option<Request> {
        None
    }

    fn resize(&mut self, width: u32, height: u32) -> Option<Request> {
        self.width = width;
        self.height = height;

        let width = {
            if width % Self::PACKET_WIDTH > 0 {
                width + Self::PACKET_WIDTH - (width % Self::PACKET_WIDTH)
            } else {
                width
            }
        };
        let height = {
            if width % Self::PACKET_HEIGHT > 0 {
                height + Self::PACKET_HEIGHT - (height % Self::PACKET_HEIGHT)
            } else {
                height
            }
        };

        self.p_width = width as usize;
        self.p_height = height as usize;

        self.pixels = vec![Vec4::zero(); (self.p_width * self.p_height) as usize];
        self.camera.resize(width, height);

        None
    }
}

fn main() {
    let width = 512;
    let height = 512;
    let app = App::new(width, height);

    run_app::<App>(app, "rust raytracer", width, height);
}
