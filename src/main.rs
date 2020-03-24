struct App {
    pub width: u32,
    pub height: u32,
}

impl cpu_fb_template::App for App {
    fn render(&mut self, fb: &mut [u8]) {
        let fw = 1.0 / self.width as f32;
        let fh = 1.0 / self.height as f32;

        for (i, pixel) in fb.chunks_exact_mut(4).enumerate() {
            let x = (i % self.width as usize) as f32;
            let y = (i / self.width as usize) as f32;

            let red = (x * fw * 255.0) as u8;
            let green = (y * fh * 255.0) as u8;
            let blue = (0.2 * 255.0) as u8;

            pixel.copy_from_slice(&[red, green, blue, 0xff]);
        }
    }

    fn key_handling(
        &mut self,
        states: &cpu_fb_template::KeyHandler,
    ) -> Option<cpu_fb_template::Request> {
        if states.pressed(cpu_fb_template::KeyCode::Escape) {
            return Some(cpu_fb_template::Request::Exit);
        }

        None
    }

    fn mouse_handling(&mut self, _x: f64, _y: f64, _dx: f64, _dy: f64) {}

    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

fn main() {
    let app = App {
        width: 512,
        height: 512,
    };

    cpu_fb_template::run_app::<App>(app, "rust raytracer", 512, 512);
}
