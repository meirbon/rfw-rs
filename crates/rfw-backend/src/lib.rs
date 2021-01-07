pub use bitvec::prelude::*;
pub use lights::*;
pub use raw_window_handle::HasRawWindowHandle;
pub use rtbvh::AABB;
pub use structs::*;

mod lights;
mod structs;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RenderMode {
    Default = 0,
    Reset = 1,
    Accumulate = 2,
}

impl Default for RenderMode {
    fn default() -> Self {
        RenderMode::Default
    }
}

pub trait Backend {
    type Settings;

    /// Initializes renderer with surface given through a raw window handle
    fn init<T: HasRawWindowHandle>(
        window: &T,
        window_size: (u32, u32),
        scale_factor: f64,
    ) -> Result<Box<Self>, Box<dyn std::error::Error>>;

    fn set_2d_mesh(&mut self, id: usize, data: MeshData2D<'_>);
    /// Sets an instance with a 4x4 transformation matrix in column-major format
    fn set_2d_instances(&mut self, instances: InstancesData2D<'_>);

    fn set_3d_mesh(&mut self, id: usize, data: MeshData3D<'_>);

    fn unload_3d_meshes(&mut self, ids: Vec<usize>);

    /// Sets an instance with a 4x4 transformation matrix in column-major format
    fn set_3d_instances(&mut self, instances: InstancesData3D<'_>);

    fn unload_3d_instances(&mut self, ids: Vec<usize>);

    /// Updates materials
    fn set_materials(&mut self, materials: &[DeviceMaterial], changed: &BitSlice);

    /// Updates textures
    /// Textures in BGRA format, 8 bytes per channel, 32 bytes per texel.
    fn set_textures(&mut self, textures: &[TextureData<'_>], changed: &BitSlice);

    /// Synchronizes scene after updating meshes, instances, materials and lights
    /// This is an expensive step as it can involve operations such as acceleration structure rebuilds
    fn synchronize(&mut self);

    /// Renders an image to the window surface
    fn render(&mut self, camera: CameraView, mode: RenderMode);

    /// Resizes framebuffer, uses scale factor provided in init function.
    fn resize<T: HasRawWindowHandle>(
        &mut self,
        window: &T,
        window_size: (u32, u32),
        scale_factor: f64,
    );

    /// Updates point lights, only lights with their 'changed' flag set to true have changed
    fn set_point_lights(&mut self, lights: &[PointLight], changed: &BitSlice);

    /// Updates spot lights, only lights with their 'changed' flag set to true have changed
    fn set_spot_lights(&mut self, lights: &[SpotLight], changed: &BitSlice);

    /// Updates area lights, only lights with their 'changed' flag set to true have changed
    fn set_area_lights(&mut self, lights: &[AreaLight], changed: &BitSlice);

    /// Updates directional lights, only lights with their 'changed' flag set to true have changed
    fn set_directional_lights(&mut self, lights: &[DirectionalLight], changed: &BitSlice);

    // Sets the scene skybox
    fn set_skybox(&mut self, skybox: TextureData<'_>);

    // Sets skins
    fn set_skins(&mut self, skins: &[SkinData<'_>], changed: &BitSlice);

    // Access backend settings
    fn settings(&mut self) -> &mut Self::Settings;
}
