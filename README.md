# Rust RFW
Rendering framework written in Rust.
This project is a playground for me ([@MeirBon](https://github.com/MeirBon)) to get to know Rust better and easily play around with libraries such as 
[wgpu](https://github.com/gfx-rs/wgpu) and [ash](https://github.com/MaikKlein/ash).
It is heavily based on my similarly named C++ project [rendering-fw](https://github.com/meirbon/rendering-fw).

## Todo list
- The scene crate in this project needs to automatically figure out which triangles are area lights.
- Ability to generate various levels of lod meshes, this could be useful for shadow maps & ray traced shadows.
- Implement gltf support including animation support
- Port my Vulkan RTX renderer to this project

## Features
The project currently contains 2 working apps
- CPU ray tracer
- Deferred GPU renderer implemented using wgpu

This project contains 1 crate
- scene

### scene
The scene library implements quads, triangle meshes, spheres and planes. Non-triangle objects
can only (currently) be rendered by the CPU ray tracer. Scenes can be serialized and deserialized using [serde](https://serde.rs).
Components such as BVHs can be serialized as well allowing for quick startup times.

![gpu-rt](docs/gpu-rt.png)
![deferred.png](docs/deferred.png)
![cpu-rt](docs/cpu-rt.png) 