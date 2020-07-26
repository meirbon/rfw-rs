use bitvec::prelude::*;
use std::ops::{Index, IndexMut};

#[cfg(feature = "object_caching")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Flags {
    bits: BitVec,
}

#[allow(dead_code)]
impl Flags {
    pub fn new() -> Flags {
        Self::default()
    }

    pub fn set_flag<T: Into<u8>>(&mut self, flag: T) {
        self.bits.set(flag.into() as u8 as usize, true);
    }

    pub fn unset_flag<T: Into<u8>>(&mut self, flag: T) {
        self.bits.set(flag.into() as u8 as usize, false);
    }

    pub fn has_flag<T: Into<u8>>(&self, flag: T) -> bool {
        match self.bits.get(flag.into() as u8 as usize) {
            None => false,
            Some(flag) => *flag,
        }
    }

    pub fn any(&self) -> bool {
        self.bits.any()
    }
}

impl Default for Flags {
    fn default() -> Self {
        let mut bits = BitVec::new();
        bits.resize(32, false);
        Self { bits }
    }
}

impl<T: Default + Clone + std::fmt::Debug> Default for FlaggedStorage<T> {
    fn default() -> Self {
        Self {
            storage: Vec::new(),
            active: BitVec::new(),
            storage_ptr: 0,
            empty_slots: Vec::new(),
        }
    }
}

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct FlaggedStorage<T: Default + std::fmt::Debug + Clone> {
    storage: Vec<T>,
    active: BitVec,
    storage_ptr: usize,
    empty_slots: Vec<u32>,
}

#[allow(dead_code)]
impl<T: Default + Clone + std::fmt::Debug> FlaggedStorage<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.storage_ptr
    }

    pub fn allocate(&mut self) -> usize {
        let index = if let Some(index) = self.empty_slots.pop() {
            index as usize
        } else {
            let index = self.storage_ptr;
            self.storage_ptr += 1;

            if self.storage.len() <= self.storage_ptr {
                let new_size = self.storage_ptr * 2;
                self.storage.resize(new_size, T::default());
                self.active.resize(new_size, false);
            }
            index
        };

        self.active.set(index, true);
        index
    }

    /// Releases index but does not overwrite memory at index
    pub fn release(&mut self, index: usize) -> Result<(), ()> {
        match self.active.get(index) {
            None => Err(()),
            Some(_) => match unsafe { self.active.get_unchecked(index) } {
                true => {
                    self.active.set(index, false);
                    self.empty_slots.push(index as u32);
                    Ok(())
                }
                false => Err(()),
            },
        }
    }

    /// Releases index and resets memory at index
    pub fn erase(&mut self, index: usize) -> Result<(), ()> {
        match self.active.get(index) {
            None => Err(()),
            Some(_) => match unsafe { self.active.get_unchecked(index) } {
                true => {
                    self.active.set(index, false);
                    self.storage[index] = T::default();
                    self.empty_slots.push(index as u32);
                    Ok(())
                }
                false => Err(()),
            },
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        match self.active.get(index) {
            None => None,
            Some(active) => match *active {
                true => Some(&self.storage[index]),
                false => None,
            },
        }
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        match self.active.get(index) {
            None => None,
            Some(active) => match *active {
                true => Some(&mut self.storage[index]),
                false => None,
            },
        }
    }

    pub unsafe fn get_unchecked(&self, index: usize) -> &T {
        &self.storage[index]
    }

    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        &mut self.storage[index]
    }

    pub fn push(&mut self, val: T) -> usize {
        let index = self.allocate();
        self.storage[index] = val;
        index
    }

    pub fn iter(&self) -> FlaggedIterator<'_, T> {
        FlaggedIterator {
            storage: self.storage.as_slice(),
            flags: &self.active,
            length: self.storage_ptr,
            current: 0,
        }
    }

    pub fn iter_mut(&mut self) -> FlaggedIteratorMut<'_, T> {
        FlaggedIteratorMut {
            storage: self.storage.as_mut_slice(),
            flags: &self.active,
            length: self.storage_ptr,
            current: 0,
        }
    }

    pub fn as_slice(&self) -> &[T] {
        self.storage.as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.storage.as_mut_slice()
    }
}

pub struct FlaggedIterator<'a, T: Default + Clone + std::fmt::Debug> {
    storage: &'a [T],
    flags: &'a BitVec,
    length: usize,
    current: usize,
}

impl<'a, T: Default + Clone + std::fmt::Debug> Iterator for FlaggedIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.length {
            match unsafe { self.flags.get_unchecked(self.current) } {
                true => {
                    return Some(unsafe {
                        let ptr = self.storage.as_ptr();
                        let reference = ptr.add(self.current).as_ref().unwrap();
                        self.current += 1;
                        reference
                    });
                }
                false => {
                    self.current += 1;
                    continue;
                }
            }
        }
        None
    }
}

pub struct FlaggedIteratorMut<'a, T: Default + Clone + std::fmt::Debug> {
    storage: &'a mut [T],
    flags: &'a BitVec,
    length: usize,
    current: usize,
}

impl<'a, T: Default + Clone + std::fmt::Debug> Iterator for FlaggedIteratorMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.length {
            match unsafe { self.flags.get_unchecked(self.current) } {
                true => {
                    return Some(unsafe {
                        let ptr = self.storage.as_mut_ptr();
                        let reference = ptr.add(self.current).as_mut().unwrap();
                        self.current += 1;
                        reference
                    });
                }
                false => {
                    self.current += 1;
                    continue;
                }
            }
        }
        None
    }
}

impl<T: Default + Clone + std::fmt::Debug> Index<usize> for FlaggedStorage<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        match unsafe { *self.active.get_unchecked(index) } {
            true => unsafe { self.get_unchecked(index) },
            false => panic!(format!("index {} was not active", index))
        }
    }
}

impl<T: Default + Clone + std::fmt::Debug> IndexMut<usize> for FlaggedStorage<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match unsafe { *self.active.get_unchecked(index) } {
            true => unsafe { self.get_unchecked_mut(index) },
            false => panic!(format!("index {} was not active", index))
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrackedStorage<T: Default + std::fmt::Debug + Clone> {
    storage: FlaggedStorage<T>,
    changed: BitVec,
}

impl<T: Default + Clone + std::fmt::Debug> Default for TrackedStorage<T> {
    fn default() -> Self {
        Self {
            storage: FlaggedStorage::default(),
            changed: BitVec::new()
        }
    }
}

#[allow(dead_code)]
impl<T: Default + Clone + std::fmt::Debug> TrackedStorage<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.storage.len()
    }

    pub fn allocate(&mut self) -> usize {
        let index = self.storage.allocate();
        self.changed.resize(self.changed.len().max(index + 1), false);
        self.changed.set(index, true);
        index
    }

    /// Releases index and resets memory at index
    pub fn erase(&mut self, index: usize) -> Result<(), ()> {
        match self.storage.erase(index) {
            Ok(_) => {
                self.changed.set(index, true);
                Ok(())
            }
            Err(_) => Err(()),
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.storage.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.changed.set(index, true);
        self.storage.get_mut(index)
    }

    pub unsafe fn get_unchecked(&self, index: usize) -> &T {
        &self.storage[index]
    }

    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        &mut self.storage[index]
    }

    pub fn push(&mut self, val: T) -> usize {
        let index = self.allocate();
        self.storage[index] = val;
        self.changed.set(index, true);
        index
    }

    pub fn iter(&self) -> FlaggedIterator<'_, T> {
        FlaggedIterator {
            storage: self.storage.as_slice(),
            flags: &self.storage.active,
            length: self.storage.storage_ptr,
            current: 0,
        }
    }

    pub fn iter_mut(&mut self) -> FlaggedIteratorMut<'_, T> {
        FlaggedIteratorMut {
            storage: self.storage.storage.as_mut_slice(),
            flags: &self.storage.active,
            length: self.storage.storage_ptr,
            current: 0,
        }
    }

    pub fn iter_changed(&self) -> ChangedIterator<'_, T> {
        ChangedIterator {
            storage: self.storage.storage.as_slice(),
            flags: &self.storage.active,
            changed: &self.changed,
            length: self.storage.storage_ptr,
            current: 0,
        }
    }

    pub fn iter_changed_mut(&mut self) -> ChangedIteratorMut<'_, T> {
        ChangedIteratorMut {
            storage: self.storage.storage.as_mut_slice(),
            flags: &self.storage.active,
            changed: &mut self.changed,
            length: self.storage.storage_ptr,
            current: 0,
        }
    }

    pub fn reset_changed(&mut self) {
        self.changed.set_all(false);
    }
}

pub struct ChangedIterator<'a, T: Default + Clone + std::fmt::Debug> {
    storage: &'a [T],
    flags: &'a BitVec,
    changed: &'a BitVec,
    length: usize,
    current: usize,
}

impl<'a, T: Default + Clone + std::fmt::Debug> Iterator for ChangedIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.length {
            match unsafe { (self.flags.get_unchecked(self.current), self.changed.get_unchecked(self.current)) } {
                (true, true) => {
                    return Some(unsafe {
                        let ptr = self.storage.as_ptr();
                        let reference = ptr.add(self.current).as_ref().unwrap();
                        self.current += 1;
                        reference
                    });
                }
                _ => {
                    self.current += 1;
                    continue;
                }
            }
        }

        None
    }
}

pub struct ChangedIteratorMut<'a, T: Default + Clone + std::fmt::Debug> {
    storage: &'a mut [T],
    flags: &'a BitVec,
    changed: &'a BitVec,
    length: usize,
    current: usize,
}

impl<'a, T: Default + Clone + std::fmt::Debug> Iterator for ChangedIteratorMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.length {
            match unsafe { (self.flags.get_unchecked(self.current), self.changed.get_unchecked(self.current)) } {
                (true, true) => {
                    return Some(unsafe {
                        let ptr = self.storage.as_mut_ptr();
                        let reference = ptr.add(self.current).as_mut().unwrap();
                        self.current += 1;
                        reference
                    });
                }
                _ => {
                    self.current += 1;
                    continue;
                }
            }
        }

        None
    }
}

impl<T: Default + Clone + std::fmt::Debug> Index<usize> for TrackedStorage<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.storage[index]
    }
}

impl<T: Default + Clone + std::fmt::Debug> IndexMut<usize> for TrackedStorage<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let reference = &mut self.storage[index];
        self.changed.set(index, true);
        reference
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterator_works() {
        let mut storage: FlaggedStorage<u32> = FlaggedStorage::new();
        assert_eq!(storage.push(0), 0);
        assert_eq!(storage.push(1), 1);
        assert_eq!(storage.push(2), 2);
        assert_eq!(storage.push(3), 3);

        let release = storage.release(1);
        assert!(release.is_ok());
        release.unwrap();

        let values: [u32; 3] = [0, 2, 3];
        let mut i = 0;
        for j in storage.iter() {
            assert_eq!(*j, values[i]);
            i += 1;
        }

        let mut i = 0;
        for j in storage.iter_mut() {
            assert_eq!(*j, values[i]);
            i += 1;
        }
    }

    #[test]
    fn release_erase_works() {
        let mut storage: FlaggedStorage<u32> = FlaggedStorage::new();
        assert_eq!(storage.push(0), 0);
        assert_eq!(storage.push(1), 1);
        assert_eq!(storage.push(2), 2);
        assert_eq!(storage.push(3), 3);

        let release = storage.release(1);
        assert!(release.is_ok());
        release.unwrap();

        let release = storage.erase(0);
        assert!(release.is_ok());
        release.unwrap();

        let release = storage.release(1);
        assert!(release.is_err());

        let release = storage.erase((0));
        assert!(release.is_err());

        assert_eq!(storage.allocate(), 0);
        assert_eq!(storage.allocate(), 1);
    }
}
