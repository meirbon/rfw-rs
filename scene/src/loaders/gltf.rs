use crate::{
    graph::{AnimationDescriptor, NodeDescriptor, SceneDescriptor, SkinDescriptor},
    AnimatedMesh, Flip, Material, MaterialList, Mesh, ObjectLoader, ObjectRef, SceneError,
    TextureDescriptor, TextureFormat,
};
use glam::*;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::RwLock,
};

use crate::graph::animation::{Channel, Method, Target};
use crate::graph::Node;
use crate::utils::TrackedStorage;
use crate::{material::Texture, LoadResult, TextureSource};
use gltf::animation::util::{MorphTargetWeights, ReadOutputs, Rotations};
use gltf::json::animation::{Interpolation, Property};
use gltf::mesh::util::{ReadIndices, ReadJoints, ReadTexCoords, ReadWeights};
use gltf::scene::Transform;

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

impl ObjectLoader for GltfLoader {
    fn load(
        &self,
        path: PathBuf,
        mat_manager: &RwLock<MaterialList>,
        mesh_storage: &RwLock<TrackedStorage<Mesh>>,
        animated_mesh_storage: &RwLock<TrackedStorage<AnimatedMesh>>,
    ) -> Result<LoadResult, SceneError> {
        let file = std::fs::File::open(&path).map_err(|_| SceneError::LoadError(path.clone()))?;
        let gltf =
            gltf::Gltf::from_reader(&file).map_err(|_| SceneError::LoadError(path.clone()))?;
        let document = &gltf;

        let base_path = path.parent().expect("gltf base path");
        let gltf_buffers = GltfBuffers::load_from_gltf(&base_path, &gltf)?;

        let mut mat_mapping = HashMap::new();

        {
            let mut mat_manager = mat_manager.write().unwrap();
            let parent_folder = match path.parent() {
                Some(parent) => parent.to_path_buf(),
                None => PathBuf::from(""),
            };

            let load_texture = |source: gltf::image::Source| match source {
                gltf::image::Source::View { view, .. } => {
                    let texture_bytes =
                        gltf_buffers.view(&gltf, &view).expect("glTF texture bytes");
                    let texture = load_texture_from_memory(texture_bytes);
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

                let mut textures = TextureDescriptor::default();
                if let Some(tex) = pbr.base_color_texture() {
                    if let Some(tex) = load_texture(tex.texture().source().source()) {
                        textures = textures.with_albedo(tex);
                    }
                }
                if let Some(tex) = m.normal_texture() {
                    if let Some(tex) = load_texture(tex.texture().source().source()) {
                        textures = textures.with_normal(tex);
                    }
                }
                // TODO: Make sure this works correctly in renderers & modify other loaders to use similar kind of system
                // The metalness values are sampled from the B channel.
                // The roughness values are sampled from the G channel.
                if let Some(tex) = pbr.metallic_roughness_texture() {
                    if let Some(tex) = load_texture(tex.texture().source().source()) {
                        textures = textures.with_metallic_roughness(tex);
                    }
                }
                if let Some(tex) = m.emissive_texture() {
                    if let Some(tex) = load_texture(tex.texture().source().source()) {
                        textures = textures.with_emissive(tex);
                    }
                }

                let index = mat_manager.add_with_maps(
                    Vec4::from(pbr.base_color_factor()).truncate().into(),
                    pbr.roughness_factor(),
                    Vec4::from(pbr.base_color_factor()).truncate().into(),
                    0.0,
                    textures,
                );

                mat_mapping.insert(m.index().unwrap_or(i), index);
            });
        }

        let meshes: Vec<LoadedMesh> = document
            .meshes()
            .map(|mesh| {
                let mut tmp_indices = Vec::new();

                let mut vertices: Vec<Vec3A> = Vec::new();
                let mut normals: Vec<Vec3A> = Vec::new();
                let mut indices: Vec<[u32; 3]> = Vec::new();
                let mut joints: Vec<Vec<[u16; 4]>> = Vec::new();
                let mut weights: Vec<Vec<Vec4>> = Vec::new();
                let mut material_ids: Vec<u32> = Vec::new();
                let mut tex_coords: Vec<Vec2> = Vec::new();

                mesh.primitives().for_each(|prim| {
                    let reader = prim.reader(|buffer| gltf_buffers.buffer(&gltf, &buffer));
                    if let Some(iter) = reader.read_positions() {
                        for pos in iter {
                            vertices.push(Vec3A::from(pos));
                        }
                    }

                    if let Some(iter) = reader.read_normals() {
                        for n in iter {
                            normals.push(Vec3A::from(n));
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
                                    let idx = idx as u32;
                                    tmp_indices.push(idx as u32);
                                }
                            }
                            ReadIndices::U16(iter) => {
                                for idx in iter {
                                    let idx = idx as u32;
                                    tmp_indices.push(idx as u32);
                                }
                            }
                            ReadIndices::U32(iter) => {
                                for idx in iter {
                                    let idx = idx;
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
                        indices.push([
                            ids[0],
                            ids[1.min(ids.len() - 1)],
                            ids[2.min(ids.len() - 1)],
                        ]);
                    }

                    material_ids.resize(material_ids.len() + length, mat_id);
                });

                if !joints.is_empty() || !weights.is_empty() {
                    LoadedMesh::Animated(AnimatedMesh::new_indexed(
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
                    ))
                } else {
                    LoadedMesh::Static(Mesh::new_indexed(
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
                    ))
                }
            })
            .collect();

        let meshes: Vec<ObjectRef> = meshes
            .into_iter()
            .map(|m| match m {
                LoadedMesh::Static(m) => {
                    let mut mesh_storage = mesh_storage.write().unwrap();
                    let mesh_id = mesh_storage.push(m);
                    ObjectRef::Static(mesh_id as u32)
                }
                LoadedMesh::Animated(m) => {
                    let mut animated_mesh_storage = animated_mesh_storage.write().unwrap();
                    let mesh_id = animated_mesh_storage.push(m);
                    ObjectRef::Animated(mesh_id as u32)
                }
            })
            .collect();

        let mut animations: Vec<AnimationDescriptor> = Vec::new();
        for anim in document.animations() {
            let channels: Vec<(u32, Channel)> = anim
                .channels()
                .map(|c| {
                    let mut channel = Channel::default();
                    let reader = c.reader(|buffer| gltf_buffers.buffer(&gltf, &buffer));

                    channel.sampler = match c.sampler().interpolation() {
                        Interpolation::Linear => Method::Linear,
                        Interpolation::Step => Method::Step,
                        Interpolation::CubicSpline => Method::Spline,
                    };

                    let target = c.target();
                    let target_node_id = target.node().index();

                    channel.targets.push(match target.property() {
                        Property::Translation => Target::Translation,
                        Property::Rotation => Target::Rotation,
                        Property::Scale => Target::Scale,
                        Property::MorphTargetWeights => Target::MorphWeights,
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
                                    channel.vec3s.push(Vec3A::from(t));
                                });
                            }
                            ReadOutputs::Rotations(r) => match r {
                                Rotations::I8(r) => {
                                    r.for_each(|r| {
                                        let r = [
                                            r[0] as f32 / (std::i8::MAX) as f32,
                                            r[1] as f32 / (std::i8::MAX) as f32,
                                            r[2] as f32 / (std::i8::MAX) as f32,
                                            r[3] as f32 / (std::i8::MAX) as f32,
                                        ];
                                        channel
                                            .rotations
                                            .push(Quat::from_xyzw(r[0], r[1], r[2], r[3]));
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
                                        channel
                                            .rotations
                                            .push(Quat::from_xyzw(r[0], r[1], r[2], r[3]));
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
                                        channel
                                            .rotations
                                            .push(Quat::from_xyzw(r[0], r[1], r[2], r[3]));
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
                                        channel
                                            .rotations
                                            .push(Quat::from_xyzw(r[0], r[1], r[2], r[3]));
                                    });
                                }
                                Rotations::F32(r) => {
                                    r.for_each(|r| {
                                        channel
                                            .rotations
                                            .push(Quat::from_xyzw(r[0], r[1], r[2], r[3]));
                                    });
                                }
                            },
                            ReadOutputs::Scales(s) => {
                                s.for_each(|s| {
                                    channel.vec3s.push(Vec3A::from(s));
                                });
                            }
                            ReadOutputs::MorphTargetWeights(m) => match m {
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
                            },
                        }
                    }

                    channel.duration = *channel.key_frames.last().unwrap();

                    (target_node_id as u32, channel)
                })
                .collect();

            animations.push(AnimationDescriptor {
                name: anim.name().unwrap_or("").to_string(),
                // TODO
                //affected_roots: nodes.root_nodes(),
                channels,
            });
        }

        let mut node_descriptors = vec![];

        for scene in document.scenes().into_iter() {
            // Iterate over root nodes.
            for node in scene.nodes() {
                node_descriptors.push(load_node(&gltf, &gltf_buffers, &meshes, &node));
            }
        }

        Ok(LoadResult::Scene(SceneDescriptor {
            nodes: node_descriptors,
            animations,
        }))
    }
}

fn load_node(
    gltf: &gltf::Gltf,
    gltf_buffers: &GltfBuffers,
    meshes: &Vec<ObjectRef>,
    node: &gltf::Node,
) -> NodeDescriptor {
    let mut new_node = Node::default();
    let (scale, rotation, translation): (Vec3A, Quat, Vec3A) = match node.transform() {
        Transform::Matrix { matrix } => {
            let (scale, rotation, translation) =
                Mat4::from_cols_array_2d(&matrix).to_scale_rotation_translation();

            (scale.into(), rotation, translation.into())
        }
        Transform::Decomposed {
            translation,
            rotation,
            scale,
        } => {
            let scale = Vec3A::from(scale);
            let rotation = Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]);
            let translation = Vec3A::from(translation);

            (scale, rotation, translation)
        }
    };

    let mut node_meshes = vec![];
    if let Some(mesh) = node.mesh() {
        node_meshes.push(meshes[mesh.index()]);
    }

    let maybe_skin = node.skin().map(|s| {
        let name = s.name().map(|n| n.into()).unwrap_or(String::new());
        let joint_nodes = s
            .joints()
            .map(|joint_node| joint_node.index() as u32)
            .collect();

        let mut inverse_bind_matrices = vec![];
        let reader = s.reader(|buffer| gltf_buffers.buffer(&gltf, &buffer));
        if let Some(ibm) = reader.read_inverse_bind_matrices() {
            ibm.for_each(|m| {
                inverse_bind_matrices.push(Mat4::from_cols_array_2d(&m));
            });
        }

        SkinDescriptor {
            name,
            inverse_bind_matrices,
            joint_nodes,
        }
    });

    let mut child_nodes = vec![];
    if node.children().len() > 0 {
        child_nodes.reserve(node.children().len());
        for child in node.children() {
            child_nodes.push(load_node(gltf, gltf_buffers, meshes, &child));
        }
    }

    if let Some(name) = node.name() {
        new_node.name = String::from(name);
    }

    // TODO: Implement camera as well
    // node.camera().unwrap();

    NodeDescriptor {
        name: node.name().map(|n| n.into()).unwrap_or("".into()),
        child_nodes,

        translation,
        rotation,
        scale,

        meshes: node_meshes,
        skin: maybe_skin,
        weights: node.weights().map(|w| w.to_vec()).unwrap_or(vec![]),

        id: node.index() as u32,
    }
}

fn load_texture_from_memory(texture_bytes: &[u8]) -> Texture {
    use image::DynamicImage::*;
    use image::GenericImageView;

    let image = image::load_from_memory(texture_bytes).unwrap();
    let width = image.width();
    let height = image.height();
    let (format, bytes_per_px) = match image {
        ImageLuma8(_) => (TextureFormat::R, 1),
        ImageLumaA8(_) => (TextureFormat::RG, 2),
        ImageRgb8(_) => (TextureFormat::RGB, 3),
        ImageRgba8(_) => (TextureFormat::RGBA, 4),
        ImageBgr8(_) => (TextureFormat::BGR, 3),
        ImageBgra8(_) => (TextureFormat::BGRA, 4),
        ImageLuma16(_) => (TextureFormat::R16, 2),
        ImageLumaA16(_) => (TextureFormat::RG16, 4),
        ImageRgb16(_) => (TextureFormat::RGB16, 6),
        ImageRgba16(_) => (TextureFormat::RGBA16, 8),
    };

    Texture::from_bytes(&image.to_bytes(), width, height, format, bytes_per_px)
}

struct GltfBuffers {
    pub uri_buffers: Vec<Option<Vec<u8>>>,
}

impl GltfBuffers {
    pub fn load_from_gltf(
        base_path: impl AsRef<Path>,
        gltf: &gltf::Document,
    ) -> Result<Self, SceneError> {
        use gltf::buffer::Source;
        use std::io::Read;

        let mut buffers = vec![];
        for (_index, buffer) in gltf.buffers().enumerate() {
            let data = match buffer.source() {
                Source::Uri(uri) => {
                    if uri.starts_with("data:") {
                        unimplemented!();
                    } else {
                        let path = base_path.as_ref().join(uri);
                        let mut file = std::fs::File::open(&path)
                            .map_err(|_| SceneError::LoadError(path.clone()))?;
                        let metadata = file
                            .metadata()
                            .map_err(|_| SceneError::LoadError(path.clone()))?;
                        let mut data: Vec<u8> = Vec::with_capacity(metadata.len() as usize);
                        file.read_to_end(&mut data)
                            .map_err(|_| SceneError::LoadError(path.clone()))?;

                        assert!(data.len() >= buffer.length());

                        Some(data)
                    }
                }
                Source::Bin => None,
            };

            buffers.push(data);
        }
        Ok(GltfBuffers {
            uri_buffers: buffers,
        })
    }

    /// Obtain the contents of a loaded buffer.
    pub fn buffer<'a>(
        &'a self,
        gltf: &'a gltf::Gltf,
        buffer: &gltf::Buffer<'_>,
    ) -> Option<&'a [u8]> {
        use gltf::buffer::Source;

        match buffer.source() {
            Source::Uri(_) => self
                .uri_buffers
                .get(buffer.index())
                .map(Option::as_ref)
                .flatten()
                .map(Vec::as_slice),
            Source::Bin => gltf.blob.as_ref().map(Vec::as_slice),
        }
    }

    /// Obtain the contents of a loaded buffer view.
    #[allow(unused)]
    pub fn view<'a>(
        &'a self,
        gltf: &'a gltf::Gltf,
        view: &gltf::buffer::View<'_>,
    ) -> Option<&'a [u8]> {
        self.buffer(gltf, &view.buffer()).map(|data| {
            let begin = view.offset();
            let end = begin + view.length();
            &data[begin..end]
        })
    }
}
