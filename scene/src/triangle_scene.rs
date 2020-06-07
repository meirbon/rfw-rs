use crate::objects::*;
use crate::{loaders, Mesh, *};

use glam::*;
use rtbvh::{Bounds, AABB};

use bitvec::prelude::*;
use loaders::obj;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError, TryLockResult};
use std::{
    collections::HashSet,
    error,
    error::Error,
    ffi::OsString,
    fmt,
    fs::File,
    io::prelude::*,
    io::BufReader,
    path::{Path, PathBuf},
};
use utils::Flags;

#[derive(Debug, Clone)]
pub enum SceneError {
    InvalidObjectIndex(usize),
    InvalidInstanceIndex(usize),
    LoadError(PathBuf),
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum SceneFlags {
    BuildBVHs = 0,
}

impl Into<u8> for SceneFlags {
    fn into(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let string = match self {
            Self::InvalidObjectIndex(idx) => format!("invalid object index {}", idx),
            Self::InvalidInstanceIndex(idx) => format!("invalid instances index {}", idx),
            SceneError::LoadError(path) => format!("could not load file: {}", path.display()),
        };

        write!(f, "{}", string)
    }
}

impl error::Error for SceneError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstancedObjects {
    pub objects: Vec<Mesh>,
    pub object_references: Vec<HashSet<usize>>,
    pub objects_changed: BitVec,
    pub instances: Vec<Instance>,
    pub instances_changed: BitVec,
    pub instance_references: Vec<usize>,
    pub empty_object_slots: Vec<usize>,
    pub empty_instance_slots: Vec<usize>,
}

impl Default for InstancedObjects {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            object_references: Vec::new(),
            objects_changed: BitVec::new(),
            instances: Vec::new(),
            instances_changed: BitVec::new(),
            instance_references: Vec::new(),
            empty_object_slots: Vec::new(),
            empty_instance_slots: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneLights {
    pub point_lights: Vec<PointLight>,
    pub pl_changed: BitVec,
    pub spot_lights: Vec<SpotLight>,
    pub sl_changed: BitVec,
    pub area_lights: Vec<AreaLight>,
    pub al_changed: BitVec,
    pub directional_lights: Vec<DirectionalLight>,
    pub dl_changed: BitVec,
}

impl Default for SceneLights {
    fn default() -> Self {
        Self {
            point_lights: Vec::new(),
            pl_changed: BitVec::new(),
            spot_lights: Vec::new(),
            sl_changed: BitVec::new(),
            area_lights: Vec::new(),
            al_changed: BitVec::new(),
            directional_lights: Vec::new(),
            dl_changed: BitVec::new(),
        }
    }
}

/// Scene optimized for triangles
/// Does not support objects other than Meshes, but does not require virtual calls because of this.
#[derive(Debug, Clone)]
pub struct TriangleScene {
    scene: Arc<Mutex<InstancedObjects>>,
    lights: Arc<Mutex<SceneLights>>,
    pub materials: Arc<Mutex<MaterialList>>,
    pub settings: Arc<Mutex<Flags>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerializableScene {
    objects: InstancedObjects,
    lights: SceneLights,
    pub materials: MaterialList,
    pub settings: Flags,
}

impl From<&TriangleScene> for SerializableScene {
    fn from(scene: &TriangleScene) -> Self {
        let lock = scene.scene.lock().unwrap();
        let lights = scene.lights.lock().unwrap();
        let mat_lock = scene.materials.lock().unwrap();
        let settings = scene.settings.lock().unwrap();

        Self {
            objects: lock.clone(),
            lights: lights.clone(),
            materials: mat_lock.clone(),
            settings: settings.clone(),
        }
    }
}

impl Into<TriangleScene> for SerializableScene {
    fn into(self) -> TriangleScene {
        TriangleScene {
            scene: Arc::new(Mutex::new(self.objects)),
            lights: Arc::new(Mutex::new(self.lights)),
            materials: Arc::new(Mutex::new(self.materials)),
            settings: Arc::new(Mutex::new(self.settings)),
        }
    }
}

#[allow(dead_code)]
impl TriangleScene {
    const FF_EXTENSION: &'static str = ".scenev1";

    pub fn new() -> TriangleScene {
        TriangleScene {
            scene: Arc::new(Mutex::new(InstancedObjects::default())),
            lights: Arc::new(Mutex::new(SceneLights::default())),
            materials: Arc::new(Mutex::new(MaterialList::new())),
            settings: Arc::new(Mutex::new(Flags::default())),
        }
    }

    pub fn get_scene(&self) -> Arc<Mutex<InstancedObjects>> {
        self.scene.clone()
    }

    pub fn get_lights(&self) -> Arc<Mutex<SceneLights>> {
        self.lights.clone()
    }

    pub fn get_materials(&self) -> Arc<Mutex<MaterialList>> {
        self.materials.clone()
    }

    pub fn objects_lock(&self) -> TryLockResult<MutexGuard<'_, InstancedObjects>> {
        self.scene.try_lock()
    }

    pub fn lights_lock(&self) -> TryLockResult<MutexGuard<'_, SceneLights>> {
        self.lights.try_lock()
    }

    pub fn materials_lock(&self) -> TryLockResult<MutexGuard<'_, MaterialList>> {
        self.materials.try_lock()
    }

    pub async fn load_mesh<S: AsRef<Path>>(&self, path: S) -> Option<usize> {
        let path = path.as_ref();
        let extension = path.extension();
        let build_bvh = self
            .settings
            .lock()
            .unwrap()
            .has_flag(SceneFlags::BuildBVHs);
        if extension.is_none() {
            return None;
        }
        let extension = extension.unwrap();

        if extension == "obj" {
            let cached_object = path.with_extension("rm");
            if !cached_object.exists() {
                let mesh = {
                    // Load if cached object is not available
                    let obj = obj::Obj::new(path, self.materials.clone());
                    if obj.is_err() {
                        return None;
                    }

                    let obj = obj.unwrap();
                    let mesh = if build_bvh {
                        let mut mesh = obj.into_mesh();
                        mesh.construct_bvh();
                        mesh
                    } else {
                        obj.into_mesh()
                    };

                    let materials = self.materials.lock().unwrap();
                    // Serialize object for future use
                    mesh.serialize_object(cached_object.as_path(), &materials)
                        .unwrap();
                    mesh
                };

                return Some(self.add_object(mesh));
            }

            let mesh = {
                let mut materials = self.materials.lock().unwrap();
                // Attempt to deserialize
                crate::objects::Mesh::deserialize_object(cached_object.as_path(), &mut materials)
            };

            // Reload if necessary
            if mesh.is_err() {
                let mesh = {
                    let obj = obj::Obj::new(path, self.materials.clone());
                    if obj.is_err() {
                        return None;
                    }

                    let obj = obj.unwrap();
                    let mesh = if build_bvh {
                        let mut mesh = obj.into_mesh();
                        mesh.construct_bvh();
                        mesh
                    } else {
                        obj.into_mesh()
                    };

                    let materials = self.materials.lock().unwrap();
                    mesh.serialize_object(cached_object.as_path(), &materials)
                        .unwrap();
                    mesh
                };

                return Some(self.add_object(mesh));
            }

            let mesh = mesh.unwrap();
            return Some(self.add_object(mesh));
        }

        None
    }

    pub fn get_object<T>(&self, index: usize, mut cb: T)
    where
        T: FnMut(Option<&Mesh>),
    {
        let scene = self.scene.lock().unwrap();
        cb(scene.objects.get(index));
    }

    pub fn get_object_mut<T>(&self, index: usize, mut cb: T)
    where
        T: FnMut(Option<&mut Mesh>),
    {
        let mut scene = self.scene.lock().unwrap();
        {
            let object_references = scene.object_references.clone();
            for i in object_references[index].iter() {
                scene.instances_changed.set(*i, true);
            }
        }
        cb(scene.objects.get_mut(index));
    }

    pub fn add_object(&self, object: Mesh) -> usize {
        let mut scene = self.scene.lock().unwrap();

        if !scene.empty_object_slots.is_empty() {
            let new_index = scene.empty_object_slots.pop().unwrap();
            scene.objects[new_index] = object;
            scene.object_references[new_index] = HashSet::new();
            scene.objects_changed.set(new_index, true);
            return new_index;
        }

        scene.objects.push(object);
        scene.object_references.push(HashSet::new());
        scene.objects_changed.push(true);
        scene.objects.len() - 1
    }

    pub fn set_object(&self, index: usize, object: Mesh) -> Result<(), SceneError> {
        let mut scene = self.scene.lock().unwrap();

        if scene.objects.get(index).is_none() {
            return Err(SceneError::InvalidObjectIndex(index));
        }

        scene.objects[index] = object;
        let object_refs = scene.object_references[index].clone();
        for i in object_refs {
            self.remove_instance(i).unwrap();
        }

        let object_references = scene.object_references[index].clone();
        for i in object_references.iter() {
            scene.instances_changed.set(*i, true);
        }

        scene.object_references[index].clear();
        scene.objects_changed.set(index, true);

        Ok(())
    }

    pub fn remove_object(&mut self, object: usize) -> Result<(), SceneError> {
        let mut scene = self.scene.lock().unwrap();

        if scene.objects.get(object).is_none() {
            return Err(SceneError::InvalidObjectIndex(object));
        }

        scene.objects[object] = Mesh::empty();
        let object_refs = scene.object_references[object].clone();
        for i in object_refs {
            self.remove_instance(i).unwrap();
        }

        let object_references = scene.object_references[object].clone();
        for i in object_references.iter() {
            scene.instances_changed.set(*i, true);
        }

        scene.object_references[object].clear();
        scene.empty_object_slots.push(object);
        Ok(())
    }

    pub fn add_instance(&self, index: usize, transform: Mat4) -> Result<usize, SceneError> {
        let mut scene = self.scene.lock().unwrap();

        let instance_index = {
            if scene.objects.get(index).is_none() || scene.object_references.get(index).is_none() {
                return Err(SceneError::InvalidObjectIndex(index));
            }

            if !scene.empty_instance_slots.is_empty() {
                let new_index = scene.empty_instance_slots.pop().unwrap();
                scene.instances[new_index] =
                    Instance::new(index as isize, &scene.objects[index].bounds(), transform);
                scene.instance_references[new_index] = index;
                scene.instances_changed.set(new_index, true);
                return Ok(new_index);
            }

            let bounds = scene.objects[index].bounds();

            scene
                .instances
                .push(objects::Instance::new(index as isize, &bounds, transform));
            scene.instances.len() - 1
        };
        scene.instance_references.push(index);
        scene.instances_changed.push(true);

        scene.object_references[index].insert(instance_index);
        Ok(instance_index)
    }

    pub fn set_instance_object(&self, instance: usize, obj_index: usize) -> Result<(), SceneError> {
        let mut scene = self.scene.lock().unwrap();

        if scene.objects.get(obj_index).is_none() {
            return Err(SceneError::InvalidObjectIndex(obj_index));
        } else if scene.instances.get(instance).is_none() {
            return Err(SceneError::InvalidInstanceIndex(instance));
        }

        let old_obj_index = scene.instance_references[instance];
        scene.object_references[old_obj_index].remove(&instance);
        scene.instances[instance] = Instance::new(
            obj_index as isize,
            &scene.objects[obj_index].bounds(),
            scene.instances[instance].get_transform(),
        );
        scene.instances_changed.set(instance, true);
        scene.object_references[obj_index].insert(instance);
        scene.instance_references[instance] = obj_index;
        Ok(())
    }

    pub fn remove_instance(&self, index: usize) -> Result<(), SceneError> {
        let mut scene = self.scene.lock().unwrap();

        if scene.instances.get(index).is_none() {
            return Err(SceneError::InvalidInstanceIndex(index));
        }

        let old_obj_index = scene.instance_references[index];
        if scene.object_references.get(old_obj_index).is_some() {
            scene.object_references[old_obj_index].remove(&index);
        }

        scene.instances[index] = Instance::new(
            -1,
            &scene.objects[index].bounds(),
            scene.instances[index].get_transform(),
        );

        scene.instances_changed.set(index, true);
        scene.instance_references[index] = std::usize::MAX;
        scene.empty_instance_slots.push(index);
        Ok(())
    }

    pub fn serialize<S: AsRef<Path>>(&self, path: S) -> Result<(), Box<dyn Error>> {
        let ser_object = SerializableScene::from(self);
        let encoded: Vec<u8> = bincode::serialize(&ser_object)?;

        let mut output = OsString::from(path.as_ref().as_os_str());
        output.push(Self::FF_EXTENSION);

        let mut file = File::create(output)?;
        file.write_all(encoded.as_ref())?;
        Ok(())
    }

    pub fn deserialize<S: AsRef<Path>>(path: S) -> Result<Self, Box<dyn Error>> {
        let mut input = OsString::from(path.as_ref().as_os_str());
        input.push(Self::FF_EXTENSION);
        let file = File::open(input)?;
        let reader = BufReader::new(file);
        let mut object: SerializableScene = bincode::deserialize_from(reader)?;

        object.objects.instances_changed.set_all(true);
        object.lights.pl_changed.set_all(true);
        object.lights.sl_changed.set_all(true);
        object.lights.al_changed.set_all(true);
        object.lights.dl_changed.set_all(true);

        let object: Self = object.into();

        Ok(object)
    }

    pub fn add_point_light(
        &mut self,
        pos: Vec3,
        radiance: Vec3,
    ) -> Result<usize, TryLockError<MutexGuard<SceneLights>>> {
        match self.lights.try_lock() {
            Ok(mut lights) => {
                lights.point_lights.push(PointLight::new(pos, radiance));
                lights.pl_changed.push(true);
                Ok(lights.point_lights.len() - 1)
            }
            Err(e) => Err(e),
        }
    }

    pub fn add_spot_light(
        &mut self,
        pos: Vec3,
        direction: Vec3,
        radiance: Vec3,
        inner_angle: f32,
        outer_angle: f32,
    ) -> Result<usize, TryLockError<MutexGuard<SceneLights>>> {
        match self.lights.try_lock() {
            Ok(mut lights) => {
                lights.spot_lights.push(SpotLight::new(
                    pos,
                    direction,
                    inner_angle,
                    outer_angle,
                    radiance,
                ));
                lights.sl_changed.push(true);
                Ok(lights.spot_lights.len() - 1)
            }
            Err(e) => Err(e),
        }
    }

    pub fn add_directional_light(
        &mut self,
        direction: Vec3,
        radiance: Vec3,
    ) -> Result<usize, TryLockError<MutexGuard<SceneLights>>> {
        match self.lights.try_lock() {
            Ok(mut lights) => {
                lights
                    .directional_lights
                    .push(DirectionalLight::new(direction, radiance));
                lights.dl_changed.push(true);
                Ok(lights.directional_lights.len() - 1)
            }
            Err(e) => Err(e),
        }
    }

    pub fn reset_changed(&self) -> Result<(), ()> {
        let scene = self.scene.try_lock();
        if let Ok(mut scene) = scene {
            scene.instances_changed.set_all(false);
        } else {
            return Err(());
        }

        let lights = self.lights.try_lock();
        if let Ok(mut lights) = lights {
            lights.pl_changed.set_all(false);
            lights.sl_changed.set_all(false);
            lights.al_changed.set_all(false);
            lights.dl_changed.set_all(false);
        } else {
            return Err(());
        }

        let materials = self.materials.try_lock();
        if let Ok(mut materials) = materials {
            materials.reset_changed();
        } else {
            return Err(());
        }

        Ok(())
    }

    pub fn update_lights(&self) {
        let materials = self.materials.lock().unwrap();
        let light_flags = materials.light_flags();
        if light_flags.not_any() {
            if let Ok(mut lights) = self.lights.lock() {
                lights.area_lights = Vec::new();
                lights.al_changed.resize(0, false);
                lights.al_changed.set_all(false);
            }
            return;
        }

        let mut area_lights: Vec<AreaLight> = Vec::new();

        if let Ok(scene) = self.scene.lock() {
            let mut triangle_light_ids: Vec<(u32, u32, u32)> = Vec::new();

            scene
                .instances
                .iter()
                .enumerate()
                .for_each(|(inst_idx, instance)| {
                    let mesh_id = instance.get_hit_id();
                    let m = &scene.objects[mesh_id];
                    for v in m.meshes.iter() {
                        let light_flag = light_flags.get(v.mat_id as usize);
                        if light_flag.is_none() {
                            continue;
                        }

                        if *light_flag.unwrap() {
                            for i in (v.first as usize / 3)..(v.last as usize / 3) {
                                let i0 = i;
                                let i1 = i + 1;
                                let i2 = i + 2;

                                let v0 = &m.vertices[i0];
                                let v1 = &m.vertices[i1];
                                let v2 = &m.vertices[i2];

                                let vertex0: Vec3 =
                                    instance.transform_vertex(Vec4::from(v0.vertex).truncate());
                                let vertex1: Vec3 =
                                    instance.transform_vertex(Vec4::from(v1.vertex).truncate());
                                let vertex2: Vec3 =
                                    instance.transform_vertex(Vec4::from(v2.vertex).truncate());

                                let normal = RTTriangle::normal(vertex0, vertex1, vertex2);
                                let position = (vertex0 + vertex1 + vertex2) * (1.0 / 3.0);
                                let color = materials[v.mat_id as usize].color;

                                let triangle_id = i;
                                let id = area_lights.len();
                                triangle_light_ids.push((
                                    mesh_id as u32,
                                    triangle_id as u32,
                                    id as u32,
                                ));

                                area_lights.push(AreaLight::new(
                                    position,
                                    Vec4::from(color).truncate(),
                                    normal,
                                    inst_idx as i32,
                                    vertex0,
                                    vertex1,
                                    vertex2,
                                ));
                            }
                        }
                    }
                });

            let mut scene = scene;
            triangle_light_ids
                .into_iter()
                .for_each(|(mesh_id, triangle_id, id)| {
                    scene.objects[mesh_id as usize].triangles[triangle_id as usize].light_id =
                        id as i32;
                });
        }

        if let Ok(mut lights) = self.lights.lock() {
            lights.area_lights = area_lights;
            let new_len = lights.area_lights.len();
            lights.al_changed.resize(new_len, true);
            lights.al_changed.set_all(true);
        }
    }
}

impl Bounds for InstancedObjects {
    fn bounds(&self) -> AABB {
        let mut aabb = AABB::new();

        for instance in self.instances.iter() {
            aabb.grow_bb(&instance.bounds());
        }

        aabb
    }
}
