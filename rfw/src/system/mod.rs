use crate::ecs::*;
use rfw_backend::{Backend, DataFormat, MeshData2D, MeshData3D, SkinData, TextureData};
use rfw_scene::Scene;
use rfw_utils::BytesConversion;
use crate::backend::RenderMode;

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

    let scene: &mut Scene = &mut *scene;

    let materials = &mut scene.materials;
    let objects = &mut scene.objects;
    let graph = &mut objects.graph;
    let meshes_3d = &mut objects.meshes_3d;
    let instances_3d = &mut objects.instances_3d;
    let meshes_2d = &mut objects.meshes_2d;
    let instances_2d = &mut objects.instances_2d;
    let skins = &mut objects.skins;

    graph.synchronize(meshes_3d, instances_3d, skins);

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
        skins.reset_changed();
    }

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
    meshes_2d.reset_changed();

    for (id, instances) in instances_2d.iter_mut() {
        if !instances.any_changed() {
            continue;
        }

        instances.reset_changed();
        system.renderer.set_2d_instances(id, instances.into());
    }

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
                },
            );
        }
        changed = true;
    }
    meshes_3d.reset_changed();

    let light_flags = materials.light_flags();
    for (i, mesh) in meshes_3d.iter() {
        let instances = &mut instances_3d[i];
        if !instances.any_changed() {
            continue;
        }

        instances.reset_changed();

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
        system.renderer.set_3d_instances(i, instances.into());

        instances.reset_changed();
    }

    update_lights |= found_light;

    let mut mat_changed = false;
    if scene.materials.textures_changed() {
        let textures = scene.materials.get_textures();
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
            .set_textures(tex_data.as_slice(), scene.materials.get_textures_changed());
        changed = true;
        mat_changed = true;
    }

    if scene.materials.changed() {
        scene.materials.update_device_materials();
        system.renderer.set_materials(
            scene.materials.get_device_materials(),
            scene.materials.get_materials_changed(),
        );
        changed = true;
        mat_changed = true;
    }

    scene.materials.reset_changed();
    update_lights = update_lights || mat_changed;

    if update_lights {
        scene.update_lights();
    }

    if scene.lights.point_lights.any_changed() {
        system.renderer.set_point_lights(
            scene.lights.point_lights.as_slice(),
            scene.lights.point_lights.changed(),
        );
        scene.lights.point_lights.reset_changed();
        changed = true;
    }

    if scene.lights.spot_lights.any_changed() {
        system.renderer.set_spot_lights(
            scene.lights.spot_lights.as_slice(),
            scene.lights.spot_lights.changed(),
        );
        scene.lights.spot_lights.reset_changed();
        changed = true;
    }

    if scene.lights.area_lights.any_changed() {
        system.renderer.set_area_lights(
            scene.lights.area_lights.as_slice(),
            scene.lights.area_lights.changed(),
        );
        changed = true;
    }

    if scene.lights.directional_lights.any_changed() {
        system.renderer.set_directional_lights(
            scene.lights.directional_lights.as_slice(),
            scene.lights.directional_lights.changed(),
        );
        scene.lights.directional_lights.reset_changed();
        changed = true;
    }

    let deleted_meshes = scene.objects.meshes_3d.take_erased();
    if !deleted_meshes.is_empty() {
        changed = true;
        system.renderer.unload_3d_meshes(deleted_meshes);
    }

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
}

impl crate::Plugin for RenderSystem {
    fn init(&mut self, _resources: &mut World, scheduler: &mut Scheduler) {
        scheduler.add_system(CoreStage::PostUpdate, synchronize_system.system());
    }
}
