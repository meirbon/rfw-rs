pub use l3d::mat::Texture;
pub use raw_window_handle::HasRawWindowHandle;
pub use rfw_scene::{
    AreaLight, Camera, DeviceMaterial, DirectionalLight, Instance2D, Instance3D, Mesh2D, Mesh3D,
    PointLight, Skin, SpotLight,
};
pub use rfw_utils::collections::ChangedIterator;
use std::error::Error;

#[derive(Debug, Copy, Clone)]
pub enum SettingType {
    String,
    Int,
    Float,
}

#[derive(Debug, Clone)]
pub enum SettingValue {
    String(String),
    Int(isize),
    Float(f64),
}

pub type SettingRange = std::ops::Range<isize>;

#[derive(Debug, Clone)]
pub struct Setting {
    key: String,
    value: SettingValue,
    value_type: SettingType,
    pub range: SettingRange,
}

impl Setting {
    pub fn new(key: String, value: SettingValue, range: Option<SettingRange>) -> Self {
        let value_type = match &value {
            SettingValue::String(_) => SettingType::String,
            SettingValue::Int(_) => SettingType::Int,
            SettingValue::Float(_) => SettingType::Float,
        };

        let range = if let Some(r) = range {
            r
        } else {
            std::isize::MIN..std::isize::MAX
        };

        Self {
            key,
            value,
            value_type,
            range,
        }
    }

    pub fn key(&self) -> &String {
        &self.key
    }

    pub fn set(&mut self, value: SettingValue) {
        match self.value_type {
            SettingType::String => match value {
                SettingValue::String(_) => {}
                _ => panic!("invalid setting value type; string expected"),
            },
            SettingType::Int => match value {
                SettingValue::Int(_) => {}
                _ => panic!("invalid setting value type; int expected"),
            },
            SettingType::Float => match value {
                SettingValue::Float(_) => assert!(true),
                _ => panic!("invalid setting value type; float expected"),
            },
        }

        match &value {
            SettingValue::String(s) => {
                assert!(s.len() < self.range.end as usize && s.len() >= self.range.start as usize)
            }
            SettingValue::Int(i) => assert!(*i < self.range.end && *i >= self.range.start),
            SettingValue::Float(i) => {
                assert!(*i < self.range.end as f64 && *i >= self.range.start as f64)
            }
        }

        self.value = value;
    }

    pub fn value(&self) -> &SettingValue {
        &self.value
    }
}

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
//
// pub struct Mesh2D<'a> {
//     pub vertices: &'a [Vec3],
//     pub tex_id: Option<u32>,
// }
//
// pub struct SubMesh {
//     pub first: u32,
//     pub last: u32,
//     pub mat_id: u32,
//     pub bounds_min: [f32; 3],
//     pub bounds_max: [f32; 3],
// }
//
// pub struct Mesh3D<'a> {
//     pub triangles: &'a [RTTriangle],
//     pub vertices: &'a [VertexData],
//     pub meshes: &'a [VertexMesh],
// }

pub trait Backend {
    /// Initializes renderer with surface given through a raw window handle
    fn init<T: HasRawWindowHandle>(
        window: &T,
        window_size: (usize, usize),
        render_size: (usize, usize),
    ) -> Result<Box<Self>, Box<dyn Error>>;

    /// Updates 2d meshes
    fn set_2d_meshes(&mut self, meshes: ChangedIterator<'_, Mesh2D>);

    /// Updates instances of 2d meshes
    fn set_2d_instances(&mut self, instances: ChangedIterator<'_, Instance2D>);

    /// Updates meshes
    fn set_3d_meshes(&mut self, meshes: ChangedIterator<'_, Mesh3D>);

    fn unload_3d_meshes(&mut self, ids: Vec<usize>);

    /// Sets an instance with a 4x4 transformation matrix in column-major format
    fn set_instances(&mut self, instances: ChangedIterator<'_, Instance3D>);

    fn unload_instances(&mut self, ids: Vec<usize>);

    /// Updates materials
    fn set_materials(&mut self, materials: ChangedIterator<'_, DeviceMaterial>);

    /// Updates textures
    fn set_textures(&mut self, textures: ChangedIterator<'_, Texture>);

    /// Synchronizes scene after updating meshes, instances, materials and lights
    /// This is an expensive step as it can involve operations such as acceleration structure rebuilds
    fn synchronize(&mut self);

    /// Renders an image to the window surface
    fn render(&mut self, camera: &Camera, mode: RenderMode);

    /// Resizes framebuffer
    fn resize<T: HasRawWindowHandle>(
        &mut self,
        window: &T,
        window_size: (usize, usize),
        render_size: (usize, usize),
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

    fn get_settings(&self) -> Vec<Setting>;

    fn set_setting(&mut self, setting: Setting);
}
