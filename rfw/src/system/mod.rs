use rfw_backend::{
    Backend, DataFormat, InstancesData2D, InstancesData3D, MeshData2D, MeshData3D, SkinData,
    TextureData,
};
use rfw_utils::BytesConversion;

pub struct RenderSystem {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) scale_factor: f64,
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

    pub(crate) fn synchronize<T: Backend>(&mut self, scene: &mut rfw_scene::Scene, renderer: &mut T) {
        let mut changed = false;
        let mut update_lights = false;
        let mut found_light = false;

        scene
            .objects
            .graph
            .synchronize(&mut scene.instances_3d, &mut scene.objects.skins);

        if scene.objects.skins.any_changed() {
            let skins: Vec<SkinData> = scene
                .objects
                .skins
                .iter()
                .map(|(_, s)| SkinData {
                    name: s.name.as_str(),
                    inverse_bind_matrices: s.inverse_bind_matrices.as_slice(),
                    joint_matrices: s.joint_matrices.as_slice(),
                })
                .collect();
            renderer.set_skins(skins.as_slice(), scene.objects.skins.changed());
            scene.objects.skins.reset_changed();
        }

        if scene.objects.meshes_2d.any_changed() {
            for (i, m) in scene.objects.meshes_2d.iter_changed() {
                renderer.set_2d_mesh(
                    i,
                    MeshData2D {
                        vertices: m.vertices.as_slice(),
                        tex_id: m.tex_id,
                    },
                );
            }
            scene.objects.meshes_2d.reset_changed();
        }

        if scene.instances_2d.any_changed() {
            renderer.set_2d_instances(InstancesData2D {
                matrices: scene.instances_2d.matrices(),
                mesh_ids: scene.instances_2d.mesh_ids(),
            });
            scene.instances_2d.reset_changed();
        }

        if scene.objects.meshes_3d.any_changed() {
            for (i, m) in scene.objects.meshes_3d.iter_changed() {
                renderer.set_3d_mesh(
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
            scene.objects.meshes_3d.reset_changed();
        }

        let light_flags = scene.materials.light_flags();
        changed |= scene.instances_3d.any_changed();

        for instance in scene.instances_3d.iter() {
            if found_light {
                break;
            }

            if let Some(mesh_id) = instance.get_mesh_id().as_index() {
                for j in 0..scene.objects.meshes_3d[mesh_id].ranges.len() {
                    match light_flags
                        .get(scene.objects.meshes_3d[mesh_id].ranges[j].mat_id as usize)
                    {
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
        }

        if scene.instances_3d.any_changed() {
            renderer.set_3d_instances(InstancesData3D {
                matrices: scene.instances_3d.matrices(),
                mesh_ids: scene.instances_3d.mesh_ids(),
                skin_ids: scene.instances_3d.skin_ids(),
            });
            changed = true;
            scene.instances_3d.reset_changed();
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

            renderer.set_textures(tex_data.as_slice(), scene.materials.get_textures_changed());
            changed = true;
            mat_changed = true;
        }

        if scene.materials.changed() {
            scene.materials.update_device_materials();
            renderer.set_materials(
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
            renderer.set_point_lights(
                scene.lights.point_lights.as_slice(),
                scene.lights.point_lights.changed(),
            );
            scene.lights.point_lights.reset_changed();
            changed = true;
        }

        if scene.lights.spot_lights.any_changed() {
            renderer.set_spot_lights(
                scene.lights.spot_lights.as_slice(),
                scene.lights.spot_lights.changed(),
            );
            scene.lights.spot_lights.reset_changed();
            changed = true;
        }

        if scene.lights.area_lights.any_changed() {
            renderer.set_area_lights(
                scene.lights.area_lights.as_slice(),
                scene.lights.area_lights.changed(),
            );
            changed = true;
        }

        if scene.lights.directional_lights.any_changed() {
            renderer.set_directional_lights(
                scene.lights.directional_lights.as_slice(),
                scene.lights.directional_lights.changed(),
            );
            scene.lights.directional_lights.reset_changed();
            changed = true;
        }

        let deleted_meshes = scene.objects.meshes_3d.take_erased();
        let deleted_instances = scene.instances_3d.take_removed();

        if !deleted_meshes.is_empty() {
            changed = true;
            renderer.unload_3d_meshes(deleted_meshes);
        }

        if !deleted_instances.is_empty() {
            changed = true;
            renderer.unload_3d_instances(deleted_instances);
        }

        if changed {
            renderer.synchronize();
        }
    }
}
