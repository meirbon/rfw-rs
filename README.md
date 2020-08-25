# Rust RFW
Rendering framework written in Rust.
This project is a playground for me ([@MeirBon](https://github.com/MeirBon)) to get to know Rust better and easily play around with libraries such as 
[wgpu](https://github.com/gfx-rs/wgpu) and [ash](https://github.com/MaikKlein/ash).
It is heavily based on my similarly named C++ project [rendering-fw](https://github.com/meirbon/rendering-fw).

## Features
The project currently contains the following working apps
- Deferred wgpu renderer
- WIP gfx-based renderer
- GPU Path tracer using wgpu

### scene crate
The scene library implements quads, triangle meshes, spheres and planes.
Scenes can be serialized and deserialized using [serde](https://serde.rs).
Components such as BVHs can be serialized as well allowing for quick startup times.

![deferred.png](docs/deferred-gif.gif)
![gpu-rt](docs/gpu-rt.png)
![cpu-rt](docs/cpu-rt.png) 