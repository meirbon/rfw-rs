use crate::backend::RenderMode;
use crate::ecs::*;
use crate::prelude::InstancesData3D;
use rfw_backend::{Backend, DataFormat, MeshData2D, MeshData3D, SkinData, TextureData};
use rfw_scene::Scene;
use rfw_utils::BytesConversion;

pub struct RenderSystem {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) scale_factor: f64,
    pub(crate) renderer: Box<dyn Backend>,
    pub mode: RenderMode,
}

unsafe impl Send for RenderSystem {}
unsafe impl Sync for RenderSystem {}

fn synchronize_system(mut system: ResMut<RenderSystem>, mut scene: ResMut<Scene>) {
    let mut changed = false;
    let mut update_lights = false;
    let mut found_light = false;

    scene.synchronize_graph();

    let skins = scene.get_skins();
    if skins.any_changed() {
        let data: Vec<SkinData> = skins
            .iter()
            .map(|(_, s)| SkinData {
                name: s.name.as_str(),
                inverse_bind_matrices: s.inverse_bind_matrices.as_slice(),
                joint_matrices: s.joint_matrices.as_slice(),
            })
            .collect();
        system.renderer.set_skins(data.as_slice(), skins.changed());
    }

    let meshes_2d = scene.get_meshes_2d();
    if meshes_2d.any_changed() {
        for (i, m) in meshes_2d.iter_changed() {
            system.renderer.set_2d_mesh(
                i,
                MeshData2D {
                    vertices: m.vertices.as_slice(),
                    tex_id: m.tex_id,
                },
            );
        }
    }

    let instances_2d = scene.get_instances_2d();
    for (id, instances) in instances_2d.iter() {
        if !instances.any_changed() {
            continue;
        }

        system.renderer.set_2d_instances(id, instances.into());
    }

    let meshes_3d = scene.get_meshes_3d();
    if meshes_3d.any_changed() {
        for (i, m) in meshes_3d.iter_changed() {
            system.renderer.set_3d_mesh(
                i,
                MeshData3D {
                    name: m.name.as_str(),
                    bounds: m.bounds,
                    vertices: m.vertices.as_slice(),
                    triangles: m.triangles.as_slice(),
                    ranges: m.ranges.as_slice(),
                    skin_data: m.skin_data.as_slice(),
                    flags: m.flags,
                },
            );
        }
        changed = true;
    }

    let instances_3d = scene.get_instances_3d();
    let light_flags = scene.get_materials().light_flags();
    for (i, mesh) in meshes_3d.iter() {
        let instances = &instances_3d[i];
        if !instances.any_changed() {
            continue;
        }

        if !found_light {
            for r in mesh.ranges.iter() {
                match light_flags.get(r.mat_id as usize) {
                    None => {}
                    Some(flag) => {
                        if *flag {
                            found_light = true;
                            break;
                        }
                    }
                }
            }
        }

        changed = true;
        system.renderer.set_3d_instances(
            i,
            InstancesData3D {
                matrices: instances.matrices(),
                skin_ids: instances.skin_ids(),
                flags: instances.flags(),
                local_aabb: meshes_3d[i].bounds,
            },
        );
    }

    update_lights |= found_light;

    let mut mat_changed = false;
    let materials = scene.get_materials();
    if materials.textures_changed() {
        let textures = materials.get_textures();
        let tex_data: Vec<TextureData> = textures
            .iter()
            .map(|t| TextureData {
                width: t.width,
                height: t.height,
                mip_levels: t.mip_levels,
                bytes: t.data.as_bytes(),
                format: DataFormat::BGRA8,
            })
            .collect();

        system
            .renderer
            .set_textures(tex_data.as_slice(), materials.get_textures_changed());
        changed = true;
        mat_changed = true;
    }

    if materials.changed() {
        let materials = scene.get_materials_mut();
        materials.update_device_materials();
        system.renderer.set_materials(
            materials.get_device_materials(),
            materials.get_materials_changed(),
        );
        changed = true;
        mat_changed = true;
    }

    update_lights = update_lights || mat_changed;

    // Update automatically generated area lights in scene
    if update_lights {
        scene.update_lights();
    }

    let lights = scene.get_lights();

    // Update point lights if necessary
    if lights.point_lights.any_changed() {
        system.renderer.set_point_lights(
            lights.point_lights.as_slice(),
            lights.point_lights.changed(),
        );
        changed = true;
    }

    // Update spot lights if necessary
    if lights.spot_lights.any_changed() {
        system
            .renderer
            .set_spot_lights(lights.spot_lights.as_slice(), lights.spot_lights.changed());
        changed = true;
    }

    // Update area lights if necessary
    if lights.area_lights.any_changed() {
        system
            .renderer
            .set_area_lights(lights.area_lights.as_slice(), lights.area_lights.changed());
        changed = true;
    }

    // Update directional lights if necessary
    if lights.directional_lights.any_changed() {
        system.renderer.set_directional_lights(
            lights.directional_lights.as_slice(),
            lights.directional_lights.changed(),
        );
        changed = true;
    }

    // Unload deleted meshes from renderer
    let deleted_meshes = scene.get_erased_meshed_3d();
    if !deleted_meshes.is_empty() {
        changed = true;
        system.renderer.unload_3d_meshes(deleted_meshes);
    }

    // Reset changes in scene
    scene.reset_changed();

    // Notify renderer if stuff has changed
    if changed {
        system.renderer.synchronize();
    }
}

impl RenderSystem {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn render_width(&self) -> u32 {
        (self.width as f64 * self.scale_factor) as u32
    }

    pub fn render_height(&self) -> u32 {
        (self.height as f64 * self.scale_factor) as u32
    }

    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    pub fn resize(&mut self, width: u32, height: u32, scale_factor: Option<f64>) {
        let scale_factor = scale_factor.unwrap_or(self.scale_factor);
        self.renderer.resize((width, height), scale_factor);
        self.width = width;
        self.height = height;
        self.scale_factor = scale_factor;
    }
}

impl crate::Plugin for RenderSystem {
    fn init(&mut self, instance: &mut crate::Instance) {
        instance.add_system_at_stage(CoreStage::PostUpdate, synchronize_system.system());
    }
}
