use crate::{
    triangle_scene::SceneError, Flip, Material, MaterialList, Mesh, TextureFormat, ToMesh,
};
use glam::*;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::{material::Texture, TextureSource};
use gltf::mesh::util::{ReadIndices, ReadJoints, ReadTexCoords, ReadWeights};

#[allow(dead_code)]
pub struct GltfObject {
    vertices: Vec<Vec3>,
    normals: Vec<Vec3>,
    indices: Vec<[u32; 3]>,
    joints: Vec<Vec<[u16; 4]>>,
    weights: Vec<Vec<Vec4>>,
    material_ids: Vec<u32>,
    tex_coords: Vec<Vec2>,
}

impl GltfObject {
    pub fn new<T: AsRef<Path>>(
        path: T,
        mat_manager: Arc<Mutex<MaterialList>>,
    ) -> Result<Self, SceneError> {
        let (document, buffers, images) = match gltf::import(path.as_ref()) {
            Ok((doc, buf, img)) => (doc, buf, img),
            Err(_) => return Err(SceneError::LoadError(path.as_ref().to_path_buf())),
        };

        assert_eq!(document.buffers().count(), buffers.len());
        assert_eq!(document.images().count(), images.len());

        let mut mat_mapping = HashMap::new();

        {
            let mut mat_manager = mat_manager.lock().unwrap();
            let parent_folder = match path.as_ref().parent() {
                Some(parent) => parent.to_path_buf(),
                None => PathBuf::from(""),
            };

            let load_texture = |source: gltf::image::Source| match source {
                gltf::image::Source::View { view, .. } => {
                    let image = &images[view.index()];
                    let texture = Texture::from_bytes(
                        image.pixels.as_slice(),
                        image.width,
                        image.height,
                        match image.format {
                            gltf::image::Format::R8 => TextureFormat::R,
                            gltf::image::Format::R8G8 => TextureFormat::RG,
                            gltf::image::Format::R8G8B8 => TextureFormat::RGB,
                            gltf::image::Format::R8G8B8A8 => TextureFormat::RGBA,
                            gltf::image::Format::B8G8R8 => TextureFormat::BGR,
                            gltf::image::Format::B8G8R8A8 => TextureFormat::BGRA,
                            gltf::image::Format::R16 => TextureFormat::R16,
                            gltf::image::Format::R16G16 => TextureFormat::RG16,
                            gltf::image::Format::R16G16B16 => TextureFormat::RGB16,
                            gltf::image::Format::R16G16B16A16 => TextureFormat::RGBA16,
                        },
                        match image.format {
                            gltf::image::Format::R8 => 1,
                            gltf::image::Format::R8G8 => 2,
                            gltf::image::Format::R8G8B8 => 3,
                            gltf::image::Format::R8G8B8A8 => 4,
                            gltf::image::Format::B8G8R8 => 3,
                            gltf::image::Format::B8G8R8A8 => 4,
                            gltf::image::Format::R16 => 2,
                            gltf::image::Format::R16G16 => 4,
                            gltf::image::Format::R16G16B16 => 6,
                            gltf::image::Format::R16G16B16A16 => 8,
                        },
                    );

                    Some(TextureSource::Loaded(texture))
                }
                gltf::image::Source::Uri { uri, .. } => Some(TextureSource::Filesystem(
                    parent_folder.join(uri),
                    Flip::None,
                )),
            };

            document.materials().enumerate().for_each(|(i, m)| {
                let mut material = Material::default();
                material.name = m.name().unwrap_or("").to_string();
                let pbr = m.pbr_metallic_roughness();
                material.roughness = pbr.roughness_factor();
                material.color = pbr.base_color_factor();
                material.metallic = pbr.metallic_factor();
                let index = mat_manager.add_with_maps(
                    Vec4::from(pbr.base_color_factor()).truncate(),
                    pbr.roughness_factor(),
                    Vec4::from(pbr.base_color_factor()).truncate(),
                    0.0,
                    match pbr.base_color_texture() {
                        Some(tex) => load_texture(tex.texture().source().source()),
                        None => None,
                    },
                    match m.normal_texture() {
                        Some(tex) => load_texture(tex.texture().source().source()),
                        None => None,
                    },
                    // TODO: Make sure this works correctly in renderers & modify other loaders to use similar kind of system
                    // The metalness values are sampled from the B channel.
                    // The roughness values are sampled from the G channel.
                    match pbr.metallic_roughness_texture() {
                        Some(tex) => load_texture(tex.texture().source().source()),
                        None => None,
                    },
                    match pbr.metallic_roughness_texture() {
                        Some(tex) => load_texture(tex.texture().source().source()),
                        None => None,
                    },
                    match m.emissive_texture() {
                        Some(tex) => load_texture(tex.texture().source().source()),
                        None => None,
                    },
                    None, //sheen_map
                );

                mat_mapping.insert(m.index().unwrap_or(i), index);
            });
        }

        let mut vertices: Vec<Vec3> = Vec::new();
        let mut normals: Vec<Vec3> = Vec::new();
        let mut indices: Vec<[u32; 3]> = Vec::new();
        let mut joints: Vec<Vec<[u16; 4]>> = Vec::new();
        let mut weights: Vec<Vec<Vec4>> = Vec::new();
        let mut material_ids: Vec<u32> = Vec::new();
        let mut tex_coords: Vec<Vec2> = Vec::new();

        let mut tmp_indices = Vec::new();

        document.meshes().for_each(|mesh| {
            mesh.primitives().for_each(|prim| {
                let reader = prim.reader(|buffer| Some(&buffers[buffer.index()]));
                if let Some(iter) = reader.read_positions() {
                    for pos in iter {
                        vertices.push(Vec3::from(pos));
                    }
                }

                if let Some(iter) = reader.read_normals() {
                    for n in iter {
                        normals.push(Vec3::from(n));
                    }
                }

                if let Some(iter) = reader.read_tex_coords(0) {
                    // TODO: Check whether we need to scale non-float types
                    match iter {
                        ReadTexCoords::U8(iter) => {
                            for uv in iter {
                                tex_coords.push(Vec2::new(uv[0] as f32, uv[1] as f32));
                            }
                        }
                        ReadTexCoords::U16(iter) => {
                            for uv in iter {
                                tex_coords.push(Vec2::new(uv[0] as f32, uv[1] as f32));
                            }
                        }
                        ReadTexCoords::F32(iter) => {
                            for uv in iter {
                                tex_coords.push(Vec2::from(uv));
                            }
                        }
                    }
                }

                let mut set = 0;
                loop {
                    let mut stop = true;

                    if let Some(iter) = reader.read_weights(set) {
                        stop = false;
                        weights.push(Vec::new());
                        match iter {
                            ReadWeights::U8(iter) => {
                                for w in iter {
                                    weights[set as usize].push(Vec4::new(
                                        w[0] as f32,
                                        w[1] as f32,
                                        w[2] as f32,
                                        w[3] as f32,
                                    ));
                                }
                            }
                            ReadWeights::U16(iter) => {
                                for w in iter {
                                    weights[set as usize].push(Vec4::new(
                                        w[0] as f32,
                                        w[1] as f32,
                                        w[2] as f32,
                                        w[3] as f32,
                                    ));
                                }
                            }
                            ReadWeights::F32(iter) => {
                                for w in iter {
                                    weights[set as usize].push(Vec4::from(w));
                                }
                            }
                        }
                    }

                    if let Some(iter) = reader.read_joints(set) {
                        stop = false;
                        joints.push(Vec::new());
                        match iter {
                            ReadJoints::U8(iter) => {
                                for j in iter {
                                    joints[set as usize].push([
                                        j[0] as u16,
                                        j[1] as u16,
                                        j[2] as u16,
                                        j[3] as u16,
                                    ]);
                                }
                            }
                            ReadJoints::U16(iter) => {
                                for j in iter {
                                    joints[set as usize].push(j);
                                }
                            }
                        }
                    }

                    if stop {
                        break;
                    }

                    set += 1;
                }

                tmp_indices.clear();
                if let Some(iter) = reader.read_indices() {
                    match iter {
                        ReadIndices::U8(iter) => {
                            for idx in iter {
                                tmp_indices.push(idx as u32);
                            }
                        }
                        ReadIndices::U16(iter) => {
                            for idx in iter {
                                tmp_indices.push(idx as u32);
                            }
                        }
                        ReadIndices::U32(iter) => {
                            for idx in iter {
                                tmp_indices.push(idx);
                            }
                        }
                    }
                }

                match prim.mode() {
                    gltf::mesh::Mode::Points => unimplemented!(),
                    gltf::mesh::Mode::Lines => unimplemented!(),
                    gltf::mesh::Mode::LineLoop => unimplemented!(),
                    gltf::mesh::Mode::LineStrip => unimplemented!(),
                    gltf::mesh::Mode::Triangles => {
                        // Nothing to do
                    }
                    gltf::mesh::Mode::TriangleStrip => {
                        let strip = tmp_indices.clone();
                        tmp_indices.clear();
                        for p in 2..strip.len() {
                            tmp_indices.push(strip[p - 2]);
                            tmp_indices.push(strip[p - 1]);
                            tmp_indices.push(strip[p]);
                        }
                    }
                    gltf::mesh::Mode::TriangleFan => {
                        let fan = tmp_indices.clone();
                        tmp_indices.clear();
                        for p in 2..fan.len() {
                            tmp_indices.push(fan[0]);
                            tmp_indices.push(fan[p - 1]);
                            tmp_indices.push(fan[p]);
                        }
                    }
                }

                let mat_id = *mat_mapping
                    .get(&prim.material().index().unwrap_or(0))
                    .unwrap_or(&0) as u32;

                let iter = tmp_indices.chunks(3);
                let length = iter.len();
                for ids in iter {
                    indices.push([ids[0], ids[1.min(ids.len() - 1)], ids[2.min(ids.len() - 1)]]);
                }

                material_ids.resize(material_ids.len() + length, mat_id);
            });
        });

        Ok(Self {
            vertices,
            normals,
            indices,
            joints,
            weights,
            material_ids,
            tex_coords,
        })
    }
}

impl ToMesh for GltfObject {
    fn into_mesh(self) -> Mesh {
        Mesh::new_indexed(
            self.indices,
            self.vertices,
            self.normals,
            self.tex_coords,
            self.material_ids,
            None,
        )
    }
}
