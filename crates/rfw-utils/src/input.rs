use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

pub struct ButtonState<T: Sized + Eq + Hash, V: Sized = bool> {
    states: HashMap<T, V>,
}

impl<T: Sized + Eq + Hash, V: Sized> Clone for ButtonState<T, V>
where
    T: Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            states: self.states.clone(),
        }
    }
}

impl<T: Sized + Eq + Hash, V: Sized> Default for ButtonState<T, V> {
    fn default() -> Self {
        Self {
            states: Default::default(),
        }
    }
}

impl<T: Sized + Eq + Hash, V: Sized> Debug for ButtonState<T, V>
where
    T: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ButtonState")
            .field("states", &self.states)
            .finish()
    }
}

impl<T: Sized + Eq + Hash, V: Sized> ButtonState<T, V> {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: T, state: V) {
        self.states.insert(key, state);
    }

    pub fn take(&mut self, key: T) -> Option<V> {
        self.states.remove(&key)
    }

    pub fn pressed(&self, key: T) -> V
    where
        V: Copy + Default,
    {
        if let Some(state) = self.states.get(&key) {
            return *state;
        }
        V::default()
    }
}
