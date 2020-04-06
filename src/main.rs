#![allow(dead_code)]
#![feature(clamp)]

use fb_template::{run_host_app, run_device_app};

mod camera;
mod cpu_app;
mod gpu_app;
mod utils;

fn main() {
    let width = 1024;
    let height = 768;

    let gpu_app = gpu_app::GPUApp::new();
    let app = run_device_app(gpu_app, "GPU App", width, height);
    // let cpu_app = cpu_app::CPUApp::new().expect("Could not init App.");
    // let app = run_host_app(cpu_app, "Rust RT", width, height);

    futures::executor::block_on(app);

}
