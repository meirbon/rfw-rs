#![feature(clamp)]

use rayon::prelude::*;

mod camera;
mod constants;
mod utils;
mod scene;
mod bvh;

use camera::*;
use utils::*;
use scene::*;

use glam::*;
use cpu_fb_template::{run_app, KeyCode};

struct App {
    pub width: u32,
    pub height: u32,

    pixels: Vec<Vec<Vec4>>,
    camera: Camera,
    timer: Timer,
    scene: Scene,
}

impl App {
    pub fn new(width: u32, height: u32) -> App {
        let mut scene = Scene::new();
        for x in -20..21 {
            for i in -20..21 {
                scene.spheres.push(Sphere::new(vec3(x as f32, 0.0, 2.0 + i as f32), 0.5, 0));
            }
        }
        scene.build_bvh();

        App {
            width,
            height,
            pixels: vec![vec![[0.0; 4].into(); width as usize]; height as usize],
            camera: Camera::new(width, height),
            timer: Timer::new(),
            scene,
        }
    }

    pub fn blit_pixels(&self, fb: &mut [u8]) {
        let line_chunk = 4 * self.width as usize;
        let pixels = &self.pixels;

        let fb_iterator = fb.par_chunks_mut(line_chunk).enumerate();

        fb_iterator.for_each(|(y, fb_pixels)| {
            let line_iterator = fb_pixels.chunks_exact_mut(4).enumerate();
            for (x, pixel) in line_iterator {
                let color = pixels[y][x].max([0.0; 4].into()).min([1.0; 4].into());
                let red = (color.x() * 255.0) as u8;
                let green = (color.y() * 255.0) as u8;
                let blue = (color.z() * 255.0) as u8;
                pixel.copy_from_slice(&[red, green, blue, 0xff]);
            }
        });
    }
}

impl cpu_fb_template::App for App {
    fn render(&mut self, fb: &mut [u8]) {
        let view = self.camera.get_view();
        let pixels = &mut self.pixels;
        let scene = &self.scene;

        pixels.par_iter_mut().enumerate().for_each(|(y, pixels)| {
            let y = y as u32;
            for (x, pixel) in pixels.iter_mut().enumerate() {
                let x = x as u32;

                let ray = view.generate_ray(x, y);

                // use rand::random;
                // let ray = view.generate_lens_ray(x, y, random(), random(), random(), random());

                *pixel = if let Some(hit) = scene.intersect(ray.origin, ray.direction) {
                    (hit.normal.x(), hit.normal.y(), hit.normal.z(), 1.0).into()
                } else {
                    [0.0; 4].into()
                }
            }
        });

        self.blit_pixels(fb);
    }

    fn key_handling(
        &mut self,
        states: &cpu_fb_template::KeyHandler,
    ) -> Option<cpu_fb_template::Request> {
        let elapsed = self.timer.elapsed_in_millis();
        self.timer.reset();

        if states.pressed(KeyCode::Escape) {
            return Some(cpu_fb_template::Request::Exit);
        }

        let mut view_change = Vec3::new(0.0, 0.0, 0.0);
        let mut pos_change = Vec3::new(0.0, 0.0, 0.0);

        if states.pressed(KeyCode::Up) { view_change += (0.0, 1.0, 0.0).into(); }
        if states.pressed(KeyCode::Down) { view_change -= (0.0, 1.0, 0.0).into(); }
        if states.pressed(KeyCode::Left) { view_change -= (1.0, 0.0, 0.0).into(); }
        if states.pressed(KeyCode::Right) { view_change += (1.0, 0.0, 0.0).into(); }

        if states.pressed(KeyCode::W) { pos_change += (0.0, 0.0, 1.0).into(); }
        if states.pressed(KeyCode::S) { pos_change -= (0.0, 0.0, 1.0).into(); }
        if states.pressed(KeyCode::A) { pos_change -= (1.0, 0.0, 0.0).into(); }
        if states.pressed(KeyCode::D) { pos_change += (1.0, 0.0, 0.0).into(); }
        if states.pressed(KeyCode::E) { pos_change += (0.0, 1.0, 0.0).into(); }
        if states.pressed(KeyCode::Q) { pos_change -= (0.0, 1.0, 0.0).into(); }

        let view_change = view_change * elapsed * 0.002;
        let pos_change = pos_change * elapsed * 0.002;

        if view_change != [0.0; 3].into() { self.camera.translate_target(view_change); }
        if pos_change != [0.0; 3].into() { self.camera.translate_relative(pos_change); }

        None
    }

    fn mouse_handling(&mut self, _x: f64, _y: f64, _dx: f64, _dy: f64) {}

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.pixels = vec![vec![[0.0; 4].into(); width as usize]; height as usize];
        self.camera.resize(width, height);
    }
}

fn main() {
    let width = 512;
    let height = 512;
    let app = App::new(width, height);

    run_app::<App>(app, "rust raytracer", width, height);
}
