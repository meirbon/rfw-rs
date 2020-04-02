#![allow(dead_code)]
#![feature(clamp)]

use fb_template::{run_app, KeyCode, KeyHandler, Request};
use glam::*;
use rayon::prelude::*;

mod camera;
mod constants;
mod material;
mod objects;
mod scene;
mod utils;

use camera::*;
use material::*;
use objects::*;
use scene::*;
use utils::*;

struct App {
    pub width: u32,
    pub height: u32,

    pixels: Vec<Vec<Vec4>>,
    camera: Camera,
    timer: Timer,
    scene: Scene,
    materials: MaterialList,
}

impl App {
    pub fn new(width: u32, height: u32) -> App {
        let mut materials = MaterialList::new();
        let mut scene = Scene::new();

        let dragon = Box::new(
            Obj::new("models/dragon.obj", &mut materials)
                .unwrap()
                .into_mesh()
                .scale(50.0),
        );
        let dragon = scene.add_object(dragon);
        scene.add_instance(dragon, Mat4::from_translation(Vec3::new(0.0, 0.0, 200.0)))
            .unwrap();

        let sphere = scene.add_object(Box::new(
            Obj::new("models/sphere.obj", &mut materials)
                .unwrap()
                .into_mesh(),
        ));
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

        App {
            width,
            height,
            pixels: vec![vec![[0.0; 4].into(); width as usize]; height as usize],
            camera: Camera::new(width, height),
            timer: Timer::new(),
            scene,
            materials,
        }
    }

    pub fn blit_pixels(&self, fb: &mut [u8]) {
        let line_chunk = 4 * self.width as usize;
        let pixels = &self.pixels;

        let fb_iterator = fb.par_chunks_mut(line_chunk).enumerate();

        fb_iterator.for_each(|(y, fb_pixels)| {
            let line_iterator = fb_pixels.chunks_exact_mut(4).enumerate();
            for (x, pixel) in line_iterator {
                let color = unsafe { pixels.get_unchecked(y).get_unchecked(x) };
                let color = color.max([0.0; 4].into()).min([1.0; 4].into());
                let red = (color.x() * 255.0) as u8;
                let green = (color.y() * 255.0) as u8;
                let blue = (color.z() * 255.0) as u8;
                pixel.copy_from_slice(&[red, green, blue, 0xff]);
            }
        });
    }
}

impl fb_template::App for App {
    fn render(&mut self, fb: &mut [u8]) -> Option<Request> {
        let view = self.camera.get_view();
        let pixels = &mut self.pixels;
        let scene = &self.scene;
        let materials = &self.materials;

        pixels.par_iter_mut().enumerate().for_each(|(y, pixels)| {
            let y = y as u32;
            for (x, pixel) in pixels.iter_mut().enumerate() {
                let x = x as u32;

                let ray = view.generate_ray(x, y);

                // use rand::random;
                // let ray = view.generate_lens_ray(x, y, random(), random(), random(), random());

                *pixel = if let Some(hit) = scene.intersect(ray.origin, ray.direction) {
                    // let material = materials.get(hit.mat_id as usize).unwrap();
                    // let color = material.color;
                    let color = hit.normal;

                    (color.x(), color.y(), color.z(), 1.0).into()
                } else {
                    [0.0; 4].into()
                }
            }
        });

        self.blit_pixels(fb);
        None
    }

    fn key_handling(&mut self, states: &KeyHandler) -> Option<Request> {
        let elapsed = self.timer.elapsed_in_millis();
        self.timer.reset();

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
            unsafe { crate::scene::USE_MBVH = true; }
        }

        if states.pressed(KeyCode::Key2) {
            unsafe { crate::scene::USE_MBVH = false; }
        }

        let view_change = view_change * elapsed * 0.002;
        let pos_change = pos_change * elapsed * 0.05;

        if view_change != [0.0; 3].into() {
            self.camera.translate_target(view_change);
        }
        if pos_change != [0.0; 3].into() {
            self.camera.translate_relative(pos_change);
        }

        Some(Request::TitleChange(String::from(format!("FPS: {:.2}", 1000.0 / elapsed))))
    }

    fn mouse_handling(&mut self, _x: f64, _y: f64, _dx: f64, _dy: f64) -> Option<Request> {
        None
    }

    fn resize(&mut self, width: u32, height: u32) -> Option<Request> {
        self.width = width;
        self.height = height;
        self.pixels = vec![vec![[0.0; 4].into(); width as usize]; height as usize];
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
