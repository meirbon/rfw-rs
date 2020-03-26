#![feature(clamp)]
use nalgebra_glm::*;
use rayon::prelude::*;

mod camera;
mod constants;

use camera::*;
use cpu_fb_template::{run_app, KeyCode};

struct App {
    pub width: u32,
    pub height: u32,

    pixels: Vec<Vec4>,
    camera: Camera,
}

impl App {
    pub fn new(width: u32, height: u32) -> App {
        App {
            width,
            height,
            pixels: vec![zero(); (width * height) as usize],
            camera: Camera::new(zero(), width, height, 60.0, 0.1, 0.1),
        }
    }

    pub fn blit_pixels(&self, fb: &mut [u8]) {
        fb.chunks_exact_mut(4).enumerate().for_each(|(i, pixel)| {
            let color: &Vec4 = &self.pixels[i];
            let red = ((color.x * 255.0) as u32).clamp(0, 255) as u8;
            let green = ((color.y * 255.0) as u32).clamp(0, 255) as u8;
            let blue = ((color.z * 255.0) as u32).clamp(0, 255) as u8;

            pixel.copy_from_slice(&[red, green, blue, 0xff]);
        });
    }
}

impl cpu_fb_template::App for App {
    fn render(&mut self, fb: &mut [u8]) {
        let uw = self.width as usize;

        let camera = &self.camera;

        self.pixels
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, pixel)| {
                let x = (i % uw) as u32;
                let y = (i / uw) as u32;

                let ray = camera.generate_ray(x, y);
                let dir = &ray.direction;

                *pixel = vec4(dir.x, dir.y, dir.z, 1.0);
            });

        self.blit_pixels(fb);
    }

    fn key_handling(
        &mut self,
        states: &cpu_fb_template::KeyHandler,
    ) -> Option<cpu_fb_template::Request> {
        if states.pressed(KeyCode::Escape) {
            return Some(cpu_fb_template::Request::Exit);
        }

        let mut view_change: Vec3 = zero();
        let mut pos_change: Vec3 = zero();

        if states.pressed(KeyCode::Up) {
            view_change.y += 1.0;
        }
        if states.pressed(KeyCode::Down) {
            view_change.y -= 1.0;
        }
        if states.pressed(KeyCode::Left) {
            view_change.x += 1.0;
        }
        if states.pressed(KeyCode::Right) {
            view_change.x -= 1.0;
        }

        if states.pressed(KeyCode::W) {
            pos_change.z += 1.0;
        }
        if states.pressed(KeyCode::S) {
            pos_change.z -= 1.0;
        }
        if states.pressed(KeyCode::A) {
            pos_change.x += 1.0;
        }
        if states.pressed(KeyCode::D) {
            pos_change.x -= 1.0;
        }
        if states.pressed(KeyCode::E) {
            pos_change.y += 1.0;
        }
        if states.pressed(KeyCode::Q) {
            pos_change.y -= 1.0;
        }

        if view_change != zero::<Vec3>() {
            self.camera.rotate(view_change.x, view_change.y);
        }

        if pos_change != zero::<Vec3>() {
            self.camera.move_relative(&pos_change);
        }

        None
    }

    fn mouse_handling(&mut self, _x: f64, _y: f64, _dx: f64, _dy: f64) {}

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.pixels.resize((width * height) as usize, zero());
        self.camera.resize(width, height);
    }
}

fn main() {
    let width = 1024;
    let height = 512;
    let app = App::new(width, height);

    run_app::<App>(app, "rust raytracer", width, height);
}
