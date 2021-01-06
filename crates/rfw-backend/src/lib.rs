pub use l3d::mat::Texture;
pub use raw_window_handle::HasRawWindowHandle;
pub use rfw_scene::{
    AreaLight, Camera, DeviceMaterial, DirectionalLight, Instance2D, Instance3D, InstanceList,
    Mesh2D, Mesh3D, PointLight, Skin, SpotLight,
};
pub use rfw_utils::collections::ChangedIterator;
use std::error::Error;

mod structs;
pub use structs::*;

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
    ) -> Result<Box<Self>, Box<dyn Error>>;

    fn set_2d_mesh(&mut self, id: usize, data: Mesh2dData);
    fn set_2d_instances(&mut self, instances: ChangedIterator<'_, Instance2D>);

    fn set_3d_mesh(&mut self, id: usize, data: Mesh3dData);
    fn unload_3d_meshes(&mut self, ids: Vec<usize>);

    /// Sets an instance with a 4x4 transformation matrix in column-major format
    fn set_3d_instances(&mut self, instances: &InstanceList);

    fn unload_3d_instances(&mut self, ids: Vec<usize>);

    /// Updates materials
    fn set_materials(&mut self, materials: ChangedIterator<'_, DeviceMaterial>);

    /// Updates textures
    fn set_textures(&mut self, textures: ChangedIterator<'_, Texture>);

    /// Synchronizes scene after updating meshes, instances, materials and lights
    /// This is an expensive step as it can involve operations such as acceleration structure rebuilds
    fn synchronize(&mut self);

    /// Renders an image to the window surface
    fn render(&mut self, camera: &Camera, mode: RenderMode);

    /// Resizes framebuffer, uses scale factor provided in init function.
    fn resize<T: HasRawWindowHandle>(
        &mut self,
        window: &T,
        window_size: (u32, u32),
        scale_factor: f64,
    );

    /// Updates point lights, only lights with their 'changed' flag set to true have changed
    fn set_point_lights(&mut self, lights: ChangedIterator<'_, PointLight>);

    /// Updates spot lights, only lights with their 'changed' flag set to true have changed
    fn set_spot_lights(&mut self, lights: ChangedIterator<'_, SpotLight>);

    /// Updates area lights, only lights with their 'changed' flag set to true have changed
    fn set_area_lights(&mut self, lights: ChangedIterator<'_, AreaLight>);

    /// Updates directional lights, only lights with their 'changed' flag set to true have changed
    fn set_directional_lights(&mut self, lights: ChangedIterator<'_, DirectionalLight>);

    // Sets the scene skybox
    fn set_skybox(&mut self, skybox: Texture);

    // Sets skins
    fn set_skins(&mut self, skins: ChangedIterator<'_, Skin>);

    // Access backend settings
    fn settings(&mut self) -> &mut Self::Settings;
}
