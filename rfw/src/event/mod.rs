use bevy_ecs::prelude::{IntoSystem, ResMut};
use std::iter::{FilterMap, Map};

use crate::prelude::CoreStage;

pub trait Event: Send + Sync {}

impl<T: Send + Sync> Event for T {}

pub trait EventStorage {
    fn clear(&mut self);
}

pub struct EventStorageVec<T: Event> {
    data: Vec<T>,
}

impl<T> EventStorage for EventStorageVec<T>
where
    T: Event,
{
    fn clear(&mut self) {
        self.data.clear()
    }
}

pub struct Events<T: Event> {
    current: Vec<T>,
    last: Vec<T>,
}

impl<T: 'static + Event> crate::ecs::Bundle for Events<T> {
    fn init(self, instance: &mut crate::Instance) {
        instance.add_resource(self);
        instance.add_system_at_stage(CoreStage::PreUpdate, Events::<T>::update_system.system());
    }
}

impl<T: Event> Default for Events<T> {
    fn default() -> Self {
        Self {
            current: Vec::new(),
            last: Vec::new(),
        }
    }
}

impl<T: 'static + Event> Events<T> {
    pub fn new() -> Self {
        Self {
            current: Vec::new(),
            last: Vec::new(),
        }
    }

    pub fn update(&mut self) {
        std::mem::swap(&mut self.current, &mut self.last);
        self.current.clear();
    }

    pub fn map<B, F>(&self, f: F) -> Map<std::slice::Iter<T>, F>
    where
        Self: Sized,
        F: FnMut(&T) -> B,
    {
        self.last.iter().map(f)
    }

    pub fn filter_map<B, F>(&self, f: F) -> FilterMap<std::slice::Iter<T>, F>
    where
        Self: Sized,
        F: FnMut(&T) -> Option<B>,
    {
        self.last.iter().filter_map(f)
    }

    pub fn iter(&self) -> std::slice::Iter<T> {
        self.last.iter()
    }

    pub fn contains<F>(&self, x: &T) -> bool
    where
        T: PartialEq,
    {
        self.last.contains(x)
    }

    pub fn push(&mut self, event: T) {
        self.current.push(event);
    }

    pub fn update_system(mut events: ResMut<Events<T>>) {
        events.update();
    }
}
