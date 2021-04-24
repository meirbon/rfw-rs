use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use bevy_ecs::prelude::ResMut;

#[derive(Debug)]
pub struct Input<T: 'static + Debug + Sized + Eq + Hash + Send + Sync> {
    pub(crate) states: HashMap<T, u32>,
}

impl<T: 'static + Debug + Sized + Eq + Hash + Send + Sync> Clone for Input<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            states: self.states.clone(),
        }
    }
}

impl<T: 'static + Debug + Sized + Eq + Hash + Send + Sync> Default for Input<T> {
    fn default() -> Self {
        Self {
            states: Default::default(),
        }
    }
}

impl<T: 'static + Debug + Sized + Eq + Hash + Send + Sync> Input<T> {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: T, state: bool) {
        if state {
            self.states.insert(key, 0);
        } else {
            self.states.remove(&key);
        }
    }

    pub fn take(&mut self, key: T) -> bool {
        self.states.remove(&key).is_some()
    }

    pub fn update(mut keys: ResMut<Input<T>>) {
        keys.states.iter_mut().for_each(|(_, k)| *k += 1);
    }

    pub fn just_pressed(&self, key: T) -> bool {
        if let Some(state) = self.states.get(&key) {
            *state == 0
        } else {
            false
        }
    }

    pub fn pressed(&self, key: T) -> bool {
        self.states.get(&key).is_some()
    }
}
