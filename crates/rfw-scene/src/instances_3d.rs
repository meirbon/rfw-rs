use rfw_backend::*;
use rfw_math::*;
use std::cell::UnsafeCell;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::utils::Transform;

bitflags::bitflags! {
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[repr(transparent)]
    pub struct InstanceFlags3D: u32 {
        const TRANSFORMED = 1;
    }
}

#[derive(Debug, Clone)]
pub struct InstanceList3D {
    pub(crate) list: Arc<UnsafeCell<List3D>>,
}

/// Although sharing instances amongst multiple threads without any mitigations against data races
/// is unsafe, the performance benefits of not doing any mitigation is too great to neglect this
/// opportunity (especially with many instances).
unsafe impl Send for InstanceList3D {}
unsafe impl Sync for InstanceList3D {}

impl From<List3D> for InstanceList3D {
    fn from(l: List3D) -> Self {
        Self {
            list: Arc::new(UnsafeCell::new(l)),
        }
    }
}

impl Into<List3D> for InstanceList3D {
    fn into(self) -> List3D {
        self.clone_inner()
    }
}

impl Default for InstanceList3D {
    fn default() -> Self {
        Self {
            list: Arc::new(UnsafeCell::new(List3D::default())),
        }
    }
}

impl InstanceList3D {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        unsafe { (*self.list.get()).ptr.load(Ordering::SeqCst) }
    }

    pub fn is_empty(&self) -> bool {
        (unsafe { (*self.list.get()).ptr.load(Ordering::SeqCst) }) == 0
    }

    pub fn allocate(&mut self) -> InstanceHandle3D {
        let list = unsafe { self.list.get().as_mut().unwrap() };
        if let Some(id) = list.free_slots.pop() {
            return InstanceHandle3D {
                index: id,
                ptr: self.list.clone(),
            };
        }

        let id = list.ptr.load(Ordering::Acquire);
        if (id + 1) >= list.matrices.len() {
            self.resize((id + 1) * 4);
        }
        list.ptr.store(id + 1, Ordering::Release);
        list.flags[id] = InstanceFlags3D::all();

        InstanceHandle3D {
            index: id,
            ptr: self.list.clone(),
        }
    }

    pub fn make_invalid(&mut self, handle: InstanceHandle3D) {
        let list = unsafe { self.list.get().as_mut().unwrap() };
        list.matrices[handle.index] = Mat4::identity();
        list.skin_ids[handle.index] = SkinID::INVALID;
        list.flags[handle.index] = InstanceFlags3D::all();
        list.free_slots.push(handle.index);
        list.removed.push(handle.index);
    }

    pub fn resize(&mut self, new_size: usize) {
        let list = unsafe { self.list.get().as_mut().unwrap() };
        list.matrices.resize(new_size, Mat4::identity());
        list.skin_ids.resize(new_size, SkinID::INVALID);
        list.flags.resize(new_size, InstanceFlags3D::empty());
    }

    pub fn get(&self, index: usize) -> Option<InstanceHandle3D> {
        let list = unsafe { self.list.get().as_mut().unwrap() };
        if list.matrices.get(index).is_some() {
            Some(InstanceHandle3D {
                index,
                ptr: self.list.clone(),
            })
        } else {
            None
        }
    }

    pub fn matrices(&self) -> &[Mat4] {
        let list = self.list.get();
        unsafe { &(*list).matrices[0..(*list).len()] }
    }

    pub fn skin_ids(&self) -> &[SkinID] {
        let list = self.list.get();
        unsafe { &(*list).skin_ids[0..(*list).len()] }
    }

    pub fn flags(&self) -> &[InstanceFlags3D] {
        let list = self.list.get();
        unsafe { &(*list).flags[0..(*list).len()] }
    }

    pub fn set_all_flags(&mut self, flag: InstanceFlags3D) {
        let list = self.list.get();
        let flags = unsafe { &mut (*list).flags[0..(*list).len()] };
        flags.iter_mut().for_each(|f| {
            (*f) |= flag;
        });
    }

    pub fn clone_inner(&self) -> List3D {
        unsafe { self.list.get().as_ref().unwrap() }.clone()
    }

    pub fn iter(&self) -> InstanceIterator3D {
        InstanceIterator3D {
            list: self.list.clone(),
            current: 0,
            ptr: unsafe { (*self.list.get()).len() },
        }
    }

    pub fn any_changed(&mut self) -> bool {
        for flag in self.flags() {
            if !flag.is_empty() {
                return true;
            }
        }

        false
    }

    pub fn reset_changed(&mut self) {
        let list = unsafe { (*self.list.get()).flags.as_mut_slice() };
        for v in list.iter_mut() {
            *v = InstanceFlags3D::empty();
        }
    }

    pub fn take_removed(&mut self) -> Vec<usize> {
        let list = unsafe { self.list.get().as_mut().unwrap() };
        let mut vec = Vec::new();
        std::mem::swap(&mut vec, &mut list.removed);
        vec
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Default)]
pub struct List3D {
    matrices: Vec<Mat4>,
    skin_ids: Vec<SkinID>,
    flags: Vec<InstanceFlags3D>,

    ptr: AtomicUsize,
    free_slots: Vec<usize>,
    removed: Vec<usize>,
}

impl Clone for List3D {
    fn clone(&self) -> Self {
        let ptr = AtomicUsize::new(self.ptr.load(Ordering::Acquire));
        let this = Self {
            matrices: self.matrices.clone(),
            skin_ids: self.skin_ids.clone(),
            flags: self.flags.clone(),

            ptr,
            free_slots: self.free_slots.clone(),
            removed: self.removed.clone(),
        };

        self.ptr.load(Ordering::Acquire);
        this
    }
}

impl List3D {
    pub fn len(&self) -> usize {
        self.ptr.load(Ordering::Acquire)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug)]
pub struct InstanceIterator3D {
    list: Arc<UnsafeCell<List3D>>,
    current: usize,
    ptr: usize,
}

impl Clone for InstanceIterator3D {
    fn clone(&self) -> Self {
        Self {
            list: self.list.clone(),
            current: 0,
            ptr: unsafe { (*self.list.get()).ptr.load(Ordering::Acquire) },
        }
    }
}

impl Iterator for InstanceIterator3D {
    type Item = InstanceHandle3D;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.ptr {
            let index = self.current;
            self.current += 1;
            return Some(InstanceHandle3D {
                index,
                ptr: self.list.clone(),
            });
        }

        None
    }
}

#[derive(Debug)]
pub struct InstanceHandle3D {
    index: usize,
    ptr: Arc<UnsafeCell<List3D>>,
}

impl InstanceHandle3D {
    #[inline]
    pub fn set_skin(&mut self, skin: SkinID) {
        let list = unsafe { self.ptr.get().as_mut().unwrap() };
        list.skin_ids[self.index] = skin;
    }

    #[inline]
    pub fn set_matrix(&mut self, matrix: Mat4) {
        let list = unsafe { self.ptr.get().as_mut().unwrap() };
        list.matrices[self.index] = matrix;
        list.flags[self.index] |= InstanceFlags3D::TRANSFORMED;
    }

    #[inline]
    pub fn get_matrix(&self) -> Mat4 {
        unsafe { (*self.ptr.get()).matrices[self.index] }
    }

    #[inline]
    pub fn get_transform(&mut self) -> Transform<Self> {
        let (scale, rotation, translation) = self.get_matrix().to_scale_rotation_translation();

        Transform {
            translation,
            rotation,
            scale,
            handle: self,
            changed: false,
        }
    }

    #[inline]
    pub fn get_skin_id(&self) -> SkinID {
        unsafe { (*self.ptr.get()).skin_ids[self.index] }
    }

    #[inline]
    pub fn get_flags(&self) -> InstanceFlags3D {
        unsafe { (*self.ptr.get()).flags[self.index] }
    }

    #[inline]
    pub fn get_id(&self) -> usize {
        self.index
    }

    #[inline]
    pub fn transformed(&self) -> bool {
        unsafe { (*self.ptr.get()).flags[self.index].contains(InstanceFlags3D::TRANSFORMED) }
    }

    #[inline]
    pub fn make_invalid(self) {
        let list = unsafe { self.ptr.get().as_mut().unwrap() };
        list.matrices[self.index] = Mat4::zero();
        list.skin_ids[self.index] = SkinID::INVALID;
        list.flags[self.index] = InstanceFlags3D::all();
        list.free_slots.push(self.index);
        list.removed.push(self.index);
    }

    /// # Safety
    ///
    /// There should only be a single instance of a handle at a time.
    /// Using these handles makes updating instances fast but leaves safety up to the user.
    pub unsafe fn clone_handle(&self) -> Self {
        Self {
            index: self.index,
            ptr: self.ptr.clone(),
        }
    }
}
