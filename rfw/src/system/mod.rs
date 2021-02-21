use rfw_backend::{Backend, DataFormat, MeshData2D, MeshData3D, SkinData, TextureData};
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

    pub(crate) fn synchronize<T: Backend>(
        &mut self,
        scene: &mut rfw_scene::AssetStore,
        renderer: &mut T,
    ) {
        let mut changed = false;
        let mut update_lights = false;
        let mut found_light = false;

        {
            let objects = scene.get_objects_mut();
            futures::executor::block_on(objects.graph.update());
            // objects.graph.synchronize(
            //     &mut objects.meshes_3d,
            //     &mut objects.instances_3d,
            //     &mut objects.skins,
            // );

            if objects.skins.any_changed() {
                let skins: Vec<SkinData> = objects
                    .skins
                    .iter()
                    .map(|(_, s)| SkinData {
                        name: s.name.as_str(),
                        inverse_bind_matrices: s.inverse_bind_matrices.as_slice(),
                        joint_matrices: s.joint_matrices.as_slice(),
                    })
                    .collect();
                renderer.set_skins(skins.as_slice(), objects.skins.changed());
                objects.skins.reset_changed();
            }

            if objects.meshes_2d.any_changed() {
                for (i, m) in objects.meshes_2d.iter_changed() {
                    renderer.set_2d_mesh(
                        i,
                        MeshData2D {
                            vertices: m.vertices.as_slice(),
                            tex_id: m.tex_id,
                        },
                    );
                }
            }
            objects.meshes_2d.reset_changed();

            for (id, instances) in objects.instances_2d.iter_mut() {
                if !instances.any_changed() {
                    continue;
                }

                instances.reset_changed();
                renderer.set_2d_instances(id, instances.into());
            }

            if objects.meshes_3d.any_changed() {
                for (i, m) in objects.meshes_3d.iter_changed() {
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
            }
            objects.meshes_3d.reset_changed();
        }

        {
            let light_flags = scene.get_materials().light_flags().clone();
            let objects = scene.get_objects_mut();
            for (i, mesh) in objects.meshes_3d.iter() {
                let instances = &mut objects.instances_3d[i];
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
                renderer.set_3d_instances(i, instances.into());

                instances.reset_changed();
            }
        }

        update_lights |= found_light;

        let mat_changed = {
            let materials = scene.get_materials_mut();
            let mut mat_changed = false;
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

                renderer.set_textures(tex_data.as_slice(), materials.get_textures_changed());
                changed = true;
                mat_changed = true;
            }

            if materials.changed() {
                materials.update_device_materials();
                renderer.set_materials(
                    materials.get_device_materials(),
                    materials.get_materials_changed(),
                );
                changed = true;
                mat_changed = true;
            }

            materials.reset_changed();
            mat_changed
        };
        update_lights = update_lights || mat_changed;

        if update_lights {
            scene.update_lights();
        }

        {
            let lights = scene.get_lights_mut();
            if lights.point_lights.any_changed() {
                renderer.set_point_lights(
                    lights.point_lights.as_slice(),
                    lights.point_lights.changed(),
                );
                lights.point_lights.reset_changed();
                changed = true;
            }

            if lights.spot_lights.any_changed() {
                renderer
                    .set_spot_lights(lights.spot_lights.as_slice(), lights.spot_lights.changed());
                lights.spot_lights.reset_changed();
                changed = true;
            }

            if lights.area_lights.any_changed() {
                renderer
                    .set_area_lights(lights.area_lights.as_slice(), lights.area_lights.changed());
                changed = true;
            }

            if lights.directional_lights.any_changed() {
                renderer.set_directional_lights(
                    lights.directional_lights.as_slice(),
                    lights.directional_lights.changed(),
                );
                lights.directional_lights.reset_changed();
                changed = true;
            }
        }

        let deleted_meshes = scene.get_objects_mut().meshes_3d.take_erased();
        if !deleted_meshes.is_empty() {
            changed = true;
            renderer.unload_3d_meshes(deleted_meshes);
        }

        if changed {
            renderer.synchronize();
        }
    }
}
