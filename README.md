# Rust RFW
Rendering framework written in Rust.
This project is a playground for me (@MeirBon) to get to know Rust better and play around
with libraries such as wgpu.

## Features
The project currently contains 2 apps: a CPU ray tracer and a GPU rasterizer implemented using wgpu.
This project contains 3 crates: bvh, fb-template and scene.

### bvh
The bvh crate implements both a 'regular' bvh using the SAH and binning for faster builds.
It also includes a mbvh (quad-tree bvh). The traversal alglorithms for these bvh's is
well-optimized and is often able to reach 25-60% (in my experience) of the performance of Intel's
Embree library as it implements packet traversal as well. Peformance varies based on
your CPU architecture. E.g. my AMD 3900x seems to perform relatively better using my custom code,
while Intel-based hardware prefers Embree (as you would expect). The bvh itself could use a
higher quality building algorithm than pure SAH, but I haven't had the time to dive into this yet.

### fb-template
This library exposes 2 traits: HostFramebuffer and DeviceFramebuffer. Both these traits allow
you to easily implement a graphics application by implementing a number of callbacks that
allow for key handling, mouse handling and writing to a framebuffer. HostFramebuffer gives access
to a host uint framebuffer that is uploaded to the GPU every frame. The DeviceFramebuffer gives
access to a SwapChain texture from wgpu. Window handling, event handling and resizing is mostly
managed by the library itself. 

### scene
The scene library implements quads, triangle meshes, spheres and planes. Non-triangle objects
can only (currently) be rendered by the CPU ray tracer. The RTTriangleScene and TriangleScene
can be serialized and deserialized using [serde](https://serde.rs). Components such as BVHs are
serialized as well allowing for quick startup times. 