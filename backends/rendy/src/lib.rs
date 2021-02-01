use rendy::core::{rendy_with_metal_backend, rendy_without_metal_backend};
use rfw_backend::Backend;

pub struct RendyBackend {}

impl Backend for RendyBackend {
    fn init<T: rfw::prelude::raw_window_handle::HasRawWindowHandle>(
        window: &T,
        window_size: (usize, usize),
        render_size: (usize, usize),
    ) -> Result<Box<Self>, Box<dyn std::error::Error>> {
    }

    fn set_2d_meshes(
        &mut self,
        meshes: rfw::prelude::ChangedIterator<'_, rfw_backend::Mesh2D>,
    ) {
    }

    fn set_2d_instances(
        &mut self,
        instances: rfw::prelude::ChangedIterator<'_, rfw_backend::Instance2D>,
    ) {
    }

    fn set_3d_meshes(&mut self, meshes: rfw::prelude::ChangedIterator<'_, rfw_backend::Mesh3D>) {}

    fn unload_3d_meshes(&mut self, ids: Vec<usize>) {}

    fn set_animated_meshes(
        &mut self,
        meshes: rfw::prelude::ChangedIterator<'_, rfw_backend::AnimatedMesh>,
    ) {
    }

    fn unload_animated_meshes(&mut self, ids: Vec<usize>) {}

    fn set_instances(
        &mut self,
        instances: rfw::prelude::ChangedIterator<'_, rfw_backend::Instance3D>,
    ) {
    }

    fn unload_instances(&mut self, ids: Vec<usize>) {}

    fn set_materials(
        &mut self,
        materials: rfw::prelude::ChangedIterator<'_, rfw::prelude::DeviceMaterial>,
    ) {
    }

    fn set_textures(&mut self, textures: rfw::prelude::ChangedIterator<'_, rfw::prelude::Texture>) {
    }

    fn synchronize(&mut self) {}

    fn render(&mut self, camera: &rfw::prelude::Camera, mode: rfw::prelude::RenderMode) {}

    fn resize<T: rfw::prelude::raw_window_handle::HasRawWindowHandle>(
        &mut self,
        window: &T,
        window_size: (usize, usize),
        render_size: (usize, usize),
    ) {
    }

    fn set_point_lights(
        &mut self,
        lights: rfw::prelude::ChangedIterator<'_, rfw::prelude::PointLight>,
    ) {
    }

    fn set_spot_lights(
        &mut self,
        lights: rfw::prelude::ChangedIterator<'_, rfw::prelude::SpotLight>,
    ) {
    }

    fn set_area_lights(
        &mut self,
        lights: rfw::prelude::ChangedIterator<'_, rfw::prelude::AreaLight>,
    ) {
    }

    fn set_directional_lights(
        &mut self,
        lights: rfw::prelude::ChangedIterator<'_, rfw::prelude::DirectionalLight>,
    ) {
    }

    fn set_skybox(&mut self, skybox: rfw::prelude::Texture) {}

    fn set_skins(&mut self, skins: rfw::prelude::ChangedIterator<'_, rfw::prelude::graph::Skin>) {}

    fn get_settings(&self) -> Vec<rfw::prelude::Setting> {}

    fn set_setting(&mut self, setting: rfw::prelude::Setting) {}
}
