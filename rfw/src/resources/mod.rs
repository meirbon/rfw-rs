use crossbeam::sync::ShardedLock;
use downcast_rs::{impl_downcast, Downcast};
use std::any::TypeId;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::fmt::*;
use std::sync::Arc;
pub use storage::*;

mod storage;

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
    pub(crate) resources: HashMap<TypeId, Arc<ShardedLock<Box<dyn ResourceStorage>>>>,
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
            Arc::new(ShardedLock::new(Box::new(ResourceData {
                value: UnsafeCell::new(resource),
                _mutated: UnsafeCell::new(true),
            }))),
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
