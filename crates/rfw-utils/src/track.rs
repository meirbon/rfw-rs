use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

pub struct Tracked<T> {
    data: T,
    changed: bool,
}

impl<T> Tracked<T> {
    pub fn changed(&self) -> bool {
        self.changed
    }

    pub fn trigger(&mut self) {
        self.changed = true;
    }

    pub fn reset(&mut self) -> bool {
        let last_state = self.changed;
        self.changed = false;
        last_state
    }
}

impl<T> Deref for Tracked<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for Tracked<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.changed = true;
        &mut self.data
    }
}

impl<T> Default for Tracked<T>
where
    T: Default,
{
    fn default() -> Self {
        Self {
            data: Default::default(),
            changed: false,
        }
    }
}

impl<T> Copy for Tracked<T> where T: Copy {}

impl<T> PartialOrd for Tracked<T>
where
    T: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.data.partial_cmp(&other.data)
    }
}

impl<T> Hash for Tracked<T>
where
    T: Hash,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.hash(state);
        self.changed.hash(state);
    }
}

impl<T> Ord for Tracked<T>
where
    T: PartialOrd + Ord + Eq,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.data.cmp(&other.data)
    }
}

impl<T> PartialEq for Tracked<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.data.eq(&other.data)
    }
}

impl<T> Eq for Tracked<T> where T: Eq + PartialEq {}

impl<T> Clone for Tracked<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            changed: false,
        }
    }
}

impl<T> Debug for Tracked<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tracked")
            .field("data", &self.data)
            .field("changed", &self.changed)
            .finish()
    }
}
