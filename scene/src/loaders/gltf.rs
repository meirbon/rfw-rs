use crate::{
    triangle_scene::SceneError, AnimatedMesh, Flip, Instance, Material, MaterialList, Mesh,
    ObjectLoader, ObjectRef, TextureFormat,
};
use glam::*;
use std::{collections::HashMap, path::PathBuf, sync::Mutex};

use crate::graph::{Node, NodeGraph, NodeMesh, Skin};
use crate::utils::TrackedStorage;
use crate::{material::Texture, LoadResult, TextureSource};
use gltf::mesh::util::{ReadIndices, ReadJoints, ReadTexCoords, ReadWeights};
use rtbvh::AABB;
use crate::graph::animation::{Channel, Animation, Method, Target};
use gltf::json::animation::{Interpolation, Property};
use gltf::animation::util::{ReadOutputs, Rotations, MorphTargetWeights};

#[derive(Debug, Copy, Clone)]
pub struct GltfLoader {}

impl std::fmt::Display for GltfLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "gltf-loader")
    }
}

impl Default for GltfLoader {
    fn default() -> Self {
        Self {}
    }
}

#[derive(Debug, Clone)]
enum LoadedMesh {
    Static(Mesh),
    Animated(AnimatedMesh),
}

#[derive(Debug, Clone)]
enum LoadedMeshID {
    Static(usize, AABB),
    Animated(usize, AABB),
}

impl ObjectLoader for GltfLoader {
    fn load(
        &self,
        path: PathBuf,
        mat_manager: &Mutex<MaterialList>,
        mesh_storage: &Mutex<TrackedStorage<Mesh>>,
        animation_storage: &Mutex<TrackedStorage<Animation>>,
        animated_mesh_storage: &Mutex<TrackedStorage<AnimatedMesh>>,
        node_storage: &Mutex<NodeGraph>,
        skin_storage: &Mutex<TrackedStorage<Skin>>,
        instances_storage: &Mutex<TrackedStorage<Instance>>,
    ) -> Result<LoadResult, SceneError> {
        let (document, buffers, images) = match gltf::import(&path) {
            Ok((doc, buf, img)) => (doc, buf, img),
            Err(_) => return Err(SceneError::LoadError(path)),
        };

        assert_eq!(document.buffers().count(), buffers.len());
        assert_eq!(document.images().count(), images.len());

        let mut mat_mapping = HashMap::new();

        {
            let mut mat_manager = mat_manager.lock().unwrap();
            let parent_folder = match path.parent() {
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


        let mut skin_mapping: HashMap<usize, usize> = HashMap::new();
        let mut node_mapping: HashMap<usize, usize> = HashMap::new();

        // Store each skin and create a mapping
        document.skins().for_each(|s| {
            let mut skin = Skin::default();
            if let Some(name) = s.name() {
                skin.name = String::from(name);
            }

            s.joints().for_each(|j| {
                skin.joint_nodes.push(j.index() as u32);
            });

            let reader = s.reader(|buffer| Some(&buffers[buffer.index()]));
            if let Some(ibm) = reader.read_inverse_bind_matrices() {
                ibm.for_each(|m| {
                    skin.inverse_bind_matrices
                        .push(Mat4::from_cols_array_2d(&m));
                });

                skin.joint_matrices
                    .resize(skin.inverse_bind_matrices.len(), Mat4::identity());
            }

            let mut skin_storage = skin_storage.lock().unwrap();
            let skin_id = skin_storage.push(skin);
            skin_mapping.insert(s.index(), skin_id);
        });

        let mut root_nodes = Vec::new();

        // Create a mapping of all nodes
        {
            let mut node_storage = node_storage.lock().unwrap();

            document.scenes().into_iter().for_each(|scene| {
                scene.nodes().for_each(|node| {
                    match Self::traverse_tree(&node, &mut node_storage, &mut node_mapping) {
                        Some(id) => {
                            node_storage.add_root_node(id);
                            root_nodes.push(id as u32);
                        }
                        None => panic!("Root node was not allocated, was it added twice?"),
                    }
                });
            });
        }

        let mut tmp_indices = Vec::new();

        let mut meshes: Vec<LoadedMesh> = Vec::new();
        document.meshes().for_each(|mesh| {
            let mut vertices: Vec<Vec3> = Vec::new();
            let mut normals: Vec<Vec3> = Vec::new();
            let mut indices: Vec<[u32; 3]> = Vec::new();
            let mut joints: Vec<Vec<[u16; 4]>> = Vec::new();
            let mut weights: Vec<Vec<Vec4>> = Vec::new();
            let mut material_ids: Vec<u32> = Vec::new();
            let mut tex_coords: Vec<Vec2> = Vec::new();

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

            if !joints.is_empty() || !weights.is_empty() {
                meshes.push(LoadedMesh::Animated(AnimatedMesh::new_indexed(
                    indices,
                    vertices,
                    normals,
                    joints,
                    weights,
                    tex_coords,
                    material_ids,
                    if let Some(name) = mesh.name() {
                        Some(String::from(name))
                    } else {
                        None
                    },
                )));
            } else {
                meshes.push(LoadedMesh::Static(Mesh::new_indexed(
                    indices,
                    vertices,
                    normals,
                    tex_coords,
                    material_ids,
                    if let Some(name) = mesh.name() {
                        Some(String::from(name))
                    } else {
                        None
                    },
                )));
            };
        });

        let meshes = meshes
            .iter()
            .map(|m| match m {
                LoadedMesh::Static(m) => {
                    let mut mesh_storage = mesh_storage.lock().unwrap();
                    let mesh_id = mesh_storage.allocate();
                    mesh_storage[mesh_id] = m.clone();
                    LoadedMeshID::Static(mesh_id, m.bounds.clone())
                }
                LoadedMesh::Animated(m) => {
                    let mut animated_mesh_storage = animated_mesh_storage.lock().unwrap();
                    let mesh_id = animated_mesh_storage.allocate();
                    animated_mesh_storage[mesh_id] = m.clone();
                    LoadedMeshID::Animated(mesh_id, m.bounds.clone())
                }
            })
            .collect::<Vec<LoadedMeshID>>();

        document.scenes().into_iter().for_each(|scene| {
            scene.nodes().into_iter().for_each(|node| {
                Self::traverse_tree_callback(&node, |node: &gltf::Node| {
                    let node_id = match node_mapping.get(&node.index()) {
                        Some(id) => *id,
                        None => panic!("Node was not in mapping"),
                    };

                    let mut new_node = Node::default();
                    new_node.set_matrix(Mat4::from_cols_array_2d(&node.transform().matrix()));
                    if let Some(weights) = node.weights() {
                        new_node.weights = weights.to_vec();
                    }

                    if let Some(mesh) = node.mesh() {
                        let mesh = &meshes[mesh.index()];
                        let mut instance_storage = instances_storage.lock().unwrap();

                        match mesh {
                            LoadedMeshID::Static(id, bounds) => {
                                let instance_id = instance_storage.allocate();
                                instance_storage[instance_id] =
                                    Instance::new(ObjectRef::Static(*id as u32), bounds);

                                new_node.meshes.push(NodeMesh {
                                    object_id: ObjectRef::Static(*id as u32),
                                    instance_id: instance_id as u32,
                                });
                            }
                            LoadedMeshID::Animated(id, bounds) => {
                                let instance_id = instance_storage.allocate();
                                instance_storage[instance_id] =
                                    Instance::new(ObjectRef::Animated(*id as u32), bounds);

                                new_node.meshes.push(NodeMesh {
                                    object_id: ObjectRef::Animated(*id as u32),
                                    instance_id: instance_id as u32,
                                });
                            }
                        }
                    }

                    if node.children().len() != 0 {
                        new_node.child_nodes.reserve(node.children().len());
                        for child in node.children() {
                            new_node.child_nodes.push(
                                match node_mapping.get(&(child.index() as usize)) {
                                    Some(val) => *val as u32,
                                    None => {
                                        panic!("Node with id {} was not in mapping", child.index())
                                    }
                                },
                            );
                        }
                    }

                    new_node.skin = if let Some(skin) = node.skin() {
                        Some((*skin_mapping.get(&skin.index()).unwrap() as u32) as u32)
                    } else {
                        None
                    };

                    if let Some(name) = node.name() {
                        new_node.name = String::from(name);
                    }

                    // TODO: Implement camera as well
                    // node.camera().unwrap();

                    node_storage.lock().unwrap()[node_id] = new_node;
                });
            });
        });

        document.animations().for_each(|anim| {
            let channels = anim.channels().map(|c| {
                let mut channel = Channel::default();
                let reader = c.reader(|buffer| Some(&buffers[buffer.index()]));

                channel.sampler = match c.sampler().interpolation() {
                    Interpolation::Linear => Method::Linear,
                    Interpolation::Step => Method::Step,
                    Interpolation::CubicSpline => Method::Spline,
                };

                let target = c.target();
                let target_node = *node_mapping.get(&target.node().index()).unwrap() as u32;
                channel.targets.push(match target.property() {
                    Property::Translation => Target::Translation(target_node),
                    Property::Rotation => Target::Rotation(target_node),
                    Property::Scale => Target::Scale(target_node),
                    Property::MorphTargetWeights => Target::MorphWeights(target_node),
                });

                if let Some(inputs) = reader.read_inputs() {
                    inputs.for_each(|input| {
                        channel.key_frames.push(input);
                    });
                }

                if let Some(outputs) = reader.read_outputs() {
                    match outputs {
                        ReadOutputs::Translations(t) => {
                            t.for_each(|t| {
                                channel.vec3s.push(Vec3::from(t));
                            });
                        }
                        ReadOutputs::Rotations(r) => {
                            match r {
                                Rotations::I8(r) => {
                                    r.for_each(|r| {
                                        let r = [
                                            r[0] as f32 / (std::i8::MAX) as f32,
                                            r[1] as f32 / (std::i8::MAX) as f32,
                                            r[2] as f32 / (std::i8::MAX) as f32,
                                            r[3] as f32 / (std::i8::MAX) as f32,
                                        ];
                                        channel.rotations.push(Quat::from(r));
                                    });
                                }
                                Rotations::U8(r) => {
                                    r.for_each(|r| {
                                        let r = [
                                            r[0] as f32 / (std::u8::MAX) as f32,
                                            r[1] as f32 / (std::u8::MAX) as f32,
                                            r[2] as f32 / (std::u8::MAX) as f32,
                                            r[3] as f32 / (std::u8::MAX) as f32,
                                        ];
                                        channel.rotations.push(Quat::from(r))
                                    });
                                }
                                Rotations::I16(r) => {
                                    r.for_each(|r| {
                                        let r = [
                                            r[0] as f32 / (std::i16::MAX) as f32,
                                            r[1] as f32 / (std::i16::MAX) as f32,
                                            r[2] as f32 / (std::i16::MAX) as f32,
                                            r[3] as f32 / (std::i16::MAX) as f32,
                                        ];
                                        channel.rotations.push(Quat::from(r))
                                    });
                                }
                                Rotations::U16(r) => {
                                    r.for_each(|r| {
                                        let r = [
                                            r[0] as f32 / (std::u16::MAX) as f32,
                                            r[1] as f32 / (std::u16::MAX) as f32,
                                            r[2] as f32 / (std::u16::MAX) as f32,
                                            r[3] as f32 / (std::u16::MAX) as f32,
                                        ];
                                        channel.rotations.push(Quat::from(r))
                                    });
                                }
                                Rotations::F32(r) => {
                                    r.for_each(|r| {
                                        channel.rotations.push(Quat::from(r))
                                    });
                                }
                            }
                        }
                        ReadOutputs::Scales(s) => {
                            s.for_each(|s| {
                                channel.vec3s.push(Vec3::from(s));
                            });
                        }
                        ReadOutputs::MorphTargetWeights(m) => {
                            match m {
                                MorphTargetWeights::I8(m) => {
                                    m.for_each(|m| {
                                        let m = m as f32 / std::i8::MAX as f32;
                                        channel.weights.push(m);
                                    });
                                }
                                MorphTargetWeights::U8(m) => {
                                    m.for_each(|m| {
                                        let m = m as f32 / std::u8::MAX as f32;
                                        channel.weights.push(m);
                                    });
                                }
                                MorphTargetWeights::I16(m) => {
                                    m.for_each(|m| {
                                        let m = m as f32 / std::i16::MAX as f32;
                                        channel.weights.push(m);
                                    });
                                }
                                MorphTargetWeights::U16(m) => {
                                    m.for_each(|m| {
                                        let m = m as f32 / std::u16::MAX as f32;
                                        channel.weights.push(m);
                                    });
                                }
                                MorphTargetWeights::F32(m) => {
                                    m.for_each(|m| {
                                        channel.weights.push(m);
                                    });
                                }
                            }
                        }
                    }
                }

                channel.duration = *channel.key_frames.last().unwrap();

                channel
            }).collect::<Vec<Channel>>();

            let mut animations = animation_storage.lock().unwrap();
            let mut animation = Animation {
                name: anim.name().unwrap_or("").to_string(),
                channels,
                time: 0.0,
            };

            // animation.set_time(0.0, &mut node_storage.lock().unwrap());
            animations.push(animation);
        });

        Ok(LoadResult::Scene(root_nodes))
    }
}

impl GltfLoader {
    fn traverse_tree_callback<T>(node: &gltf::Node, mut cb: T)
        where
            T: FnMut(&gltf::Node) + Clone,
    {
        cb(node);
        node.children().for_each(|child| {
            Self::traverse_tree_callback(&child, cb.clone());
        });
    }

    fn traverse_tree(
        node: &gltf::Node,
        storage: &mut NodeGraph,
        mapping: &mut HashMap<usize, usize>,
    ) -> Option<usize> {
        let id = if !mapping.contains_key(&node.index()) {
            let node_id = storage.allocate();
            mapping.insert(node.index(), node_id);
            Some(node_id)
        } else {
            None
        };

        node.children().for_each(|child| {
            Self::traverse_tree(&child, storage, mapping);
        });

        id
    }
}
