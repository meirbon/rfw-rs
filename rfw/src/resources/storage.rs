use crossbeam::sync::{ShardedLock, ShardedLockReadGuard, ShardedLockWriteGuard};
use std::collections::HashMap;
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

    pub fn get_or_insert<F: FnOnce() -> T>(
        &mut self,
        key: K,
        default: F,
    ) -> LockedValueMut<'_, T> {
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