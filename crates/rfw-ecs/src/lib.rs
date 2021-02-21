use crossbeam::sync::ShardedLock;
use downcast_rs::{impl_downcast, Downcast};
use rayon::prelude::*;
use std::any::type_name;
use std::any::TypeId;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::fmt::*;

pub trait Plugin {
    fn init(&mut self, resources: &mut ResourceList, scheduler: &mut Scheduler);
}

pub trait System: 'static + Send + Sync {
    fn run(&mut self, resources: &ResourceList);
}

pub struct Scheduler {
    systems: Vec<Box<dyn System>>,
}

impl std::fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scheduler")
            .field("systems", &self.systems.len())
            .finish()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self {
            systems: Default::default(),
        }
    }
}

#[allow(dead_code)]
impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_system<S: 'static + System>(&mut self, system: S) {
        rfw_utils::log::success(format!("created resource: {}", type_name::<S>()));
        self.systems.push(Box::new(system));
    }

    pub fn run(&mut self, resources: &ResourceList) {
        self.systems.par_iter_mut().for_each(|s| s.run(resources));
    }
}

pub trait Resource: 'static {}

impl<T: 'static> Resource for T {}

pub trait ResourceStorage: Downcast {}
impl_downcast!(ResourceStorage);
struct ResourceData<T: 'static> {
    value: UnsafeCell<T>,
    _mutated: UnsafeCell<bool>,
}

impl<T: 'static> ResourceStorage for ResourceData<T> {}

#[derive(Default)]
pub struct ResourceList {
    /// List of resources
    pub(crate) resources: HashMap<TypeId, ShardedLock<Box<dyn ResourceStorage>>>,
}

impl Debug for ResourceList {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("ResourceList")
            .field("resources", &self.resources.len())
            .finish()
    }
}

unsafe impl Send for ResourceList {}
unsafe impl Sync for ResourceList {}

#[allow(dead_code)]
impl ResourceList {
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
        }
    }

    pub fn add_resource<T: Resource>(&mut self, resource: T) {
        self.resources.insert(
            TypeId::of::<T>(),
            ShardedLock::new(Box::new(ResourceData {
                value: UnsafeCell::new(resource),
                _mutated: UnsafeCell::new(true),
            })),
        );
    }

    pub fn get_resource<T: Resource>(
        &self,
    ) -> Option<LockedValue<'_, Box<dyn ResourceStorage>, T>> {
        if let Some(r) = self.resources.get(&TypeId::of::<T>()) {
            let lock = r.read().unwrap();
            if let Some(data) = lock.downcast_ref::<ResourceData<T>>() {
                return Some(LockedValue {
                    data: data.value.get(),
                    _lock: lock,
                });
            }
        }

        None
    }

    pub fn get_resource_mut<T: Resource>(
        &self,
    ) -> Option<LockedValueMut<'_, Box<dyn ResourceStorage>, T>> {
        if let Some(r) = self.resources.get(&TypeId::of::<T>()) {
            let mut lock = r.write().unwrap();
            if let Some(data) = lock.downcast_mut::<ResourceData<T>>() {
                data._mutated = UnsafeCell::new(true);
                return Some(LockedValueMut {
                    data: data.value.get(),
                    _lock: lock,
                });
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::ResourceList;
    struct ScoreBoard(usize);

    #[test]
    fn test_scheduler() {
        let mut resources = ResourceList::new();

        resources.add_resource(ScoreBoard(0));
        // let system_count = 32768;
        // let additions_per_thread = 256;
        assert!(resources.get_resource::<ScoreBoard>().is_some());

        // scheduler.run(&resources, &world);

        // if let Some(resource) = resources.get_resource::<ScoreBoard>() {
        // assert_eq!(resource.0, system_count * additions_per_thread);
        // };
    }
}

// use rfw_scene::Scene;
// use std::{any::TypeId, collections::HashMap};

// use crate::system::System;
// use downcast::Downcast;

// pub trait IntoResource {
//     fn init(&mut self, scene: &mut Scene, system: &mut System);
// }

// pub trait Resource: 'static {
//     fn tick(&mut self, scene: &mut Scene, system: &mut System);
// }

// downcast::impl_downcast!(Resource);

// pub struct ResourceList {
//     pub(crate) resources: HashMap<TypeId, Box<dyn Resource>>,
// }

// impl Default for ResourceList {
//     fn default() -> Self {
//         Self {
//             resources: Default::default(),
//         }
//     }
// }

// impl ResourceList {
//     pub fn new() -> Self {
//         Self::default()
//     }

//     pub fn get_resource<T: Resource>(&mut self) -> Option<&mut T> {
//         let type_id = TypeId::of::<T>();
//         if let Some(r) = self.resources.get_mut(&type_id) {
//             let r = r.downcast_mut().unwrap();
//             Some(r)
//         } else {
//             None
//         }
//     }

//     pub fn register_plugin<T: Resource>(&mut self, resource: T) {
//         let type_id = TypeId::of::<T>();
//         self.resources.insert(type_id, Box::new(resource));
//     }

//     pub fn tick(&mut self, scene: &mut Scene, system: &mut System) {
//         for (_, r) in self.resources.iter_mut() {
//             r.tick(scene, system);
//         }
//     }
// }

use crossbeam::sync::{ShardedLockReadGuard, ShardedLockWriteGuard};
use std::hash::Hash;
use std::ops::{Deref, DerefMut};

pub struct LockedValue<'a, T, V = T> {
    pub(crate) data: *const V,
    pub(crate) _lock: ShardedLockReadGuard<'a, T>,
}

impl<T, V> Deref for LockedValue<'_, T, V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        unsafe { self.data.as_ref().unwrap() }
    }
}

pub struct LockedValueMut<'a, T, V = T> {
    pub(crate) data: *mut V,
    pub(crate) _lock: ShardedLockWriteGuard<'a, T>,
}

impl<T, V> Deref for LockedValueMut<'_, T, V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        unsafe { self.data.as_ref().unwrap() }
    }
}

impl<T, V> DerefMut for LockedValueMut<'_, T, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.data.as_mut().unwrap() }
    }
}

pub struct TypeStorage<K: Sized + Hash + Eq, T> {
    pub(crate) data: HashMap<K, ShardedLock<T>>,
}

impl<K: Sized + Hash + Eq, T> std::fmt::Debug for TypeStorage<K, T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypeStorage")
            .field("data", &self.data.len())
            .finish()
    }
}

impl<K: Sized + Hash + Eq, T> Default for TypeStorage<K, T> {
    fn default() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
}

#[allow(dead_code)]
impl<K: Sized + Hash + Eq, T> TypeStorage<K, T> {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn get(&self, key: &K) -> Option<LockedValue<'_, T>> {
        if let Some(data) = self.data.get(key) {
            let lock = data.read().unwrap();
            let data = &(*lock) as *const T;
            Some(LockedValue { data, _lock: lock })
        } else {
            None
        }
    }

    pub fn get_mut(&self, key: &K) -> Option<LockedValueMut<'_, T>> {
        if let Some(data) = self.data.get(key) {
            let mut lock = data.write().unwrap();
            let data = &mut (*lock) as *mut T;
            Some(LockedValueMut { data, _lock: lock })
        } else {
            None
        }
    }

    pub fn get_or_insert<F: FnOnce() -> T>(&mut self, key: K, default: F) -> LockedValueMut<'_, T> {
        let result = self
            .data
            .entry(key)
            .or_insert_with(|| ShardedLock::new(default()));
        let mut lock = result.write().unwrap();
        let data = &mut (*lock) as *mut T;
        LockedValueMut { data, _lock: lock }
    }

    pub fn get_or_default(&mut self, key: K) -> LockedValueMut<'_, T>
    where
        T: Default,
    {
        let result = self
            .data
            .entry(key)
            .or_insert_with(|| ShardedLock::new(T::default()));
        let mut lock = result.write().unwrap();
        let data = &mut (*lock) as *mut T;
        LockedValueMut { data, _lock: lock }
    }
}
