use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

pub struct ButtonState<T: Sized + Eq + Hash> {
    states: HashMap<T, bool>,
}

impl<T: Sized + Eq + Hash> Clone for ButtonState<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            states: self.states.clone(),
        }
    }
}

impl<T: Sized + Eq + Hash> Default for ButtonState<T> {
    fn default() -> Self {
        Self {
            states: Default::default(),
        }
    }
}

impl<T: Sized + Eq + Hash> Debug for ButtonState<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ButtonState")
            .field("states", &self.states)
            .finish()
    }
}

impl<T: Sized + Eq + Hash> ButtonState<T> {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: T, state: bool) {
        self.states.insert(key, state);
    }

    pub fn pressed(&self, key: T) -> bool {
        if let Some(state) = self.states.get(&key) {
            return *state;
        }
        false
    }
}
