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
    pub struct InstanceFlags2D: u32 {
        const TRANSFORMED = 1;
        const CHANGED_MESH = 2;
    }
}

#[derive(Debug, Clone)]
pub struct InstanceList2D {
    pub(crate) list: Arc<UnsafeCell<List2D>>,
}

/// Although sharing instances amongst multiple threads without any mitigations against data races
/// is unsafe, the performance benefits of not doing any mitigation is too great to neglect this
/// opportunity (especially with many instances).
unsafe impl Send for InstanceList2D {}
unsafe impl Sync for InstanceList2D {}

impl From<List2D> for InstanceList2D {
    fn from(l: List2D) -> Self {
        Self {
            list: Arc::new(UnsafeCell::new(l)),
        }
    }
}

impl Into<List2D> for InstanceList2D {
    fn into(self) -> List2D {
        self.clone_inner()
    }
}

impl Default for InstanceList2D {
    fn default() -> Self {
        Self {
            list: Arc::new(UnsafeCell::new(List2D::default())),
        }
    }
}

impl InstanceList2D {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        unsafe { (*self.list.get()).ptr.load(Ordering::SeqCst) }
    }

    pub fn is_empty(&self) -> bool {
        (unsafe { (*self.list.get()).ptr.load(Ordering::SeqCst) }) == 0
    }

    pub fn allocate(&mut self) -> InstanceHandle2D {
        let list = unsafe { self.list.get().as_mut().unwrap() };
        if let Some(id) = list.free_slots.pop() {
            return InstanceHandle2D {
                index: id,
                ptr: self.list.clone(),
            };
        }

        let id = list.ptr.load(Ordering::Acquire);
        if (id + 1) >= list.matrices.len() {
            self.resize((id + 1) * 4);
        }
        list.ptr.store(id + 1, Ordering::Release);
        list.flags[id] = InstanceFlags2D::all();

        InstanceHandle2D {
            index: id,
            ptr: self.list.clone(),
        }
    }

    pub fn make_invalid(&mut self, handle: InstanceHandle2D) {
        let list = unsafe { self.list.get().as_mut().unwrap() };
        list.matrices[handle.index] = Mat4::identity();
        list.mesh_ids[handle.index] = MeshID::INVALID;
        list.flags[handle.index] = InstanceFlags2D::all();
        list.free_slots.push(handle.index);
        list.removed.push(handle.index);
    }

    pub fn resize(&mut self, new_size: usize) {
        let list = unsafe { self.list.get().as_mut().unwrap() };
        list.matrices.resize(new_size, Mat4::identity());
        list.mesh_ids.resize(new_size, MeshID::INVALID);
        list.flags.resize(new_size, InstanceFlags2D::empty());
    }

    pub fn get(&self, index: usize) -> Option<InstanceHandle2D> {
        let list = unsafe { self.list.get().as_mut().unwrap() };
        if list.matrices.get(index).is_some() {
            Some(InstanceHandle2D {
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

    pub fn mesh_ids(&self) -> &[MeshID] {
        let list = self.list.get();
        unsafe { &(*list).mesh_ids[0..(*list).len()] }
    }

    pub fn flags(&self) -> &[InstanceFlags2D] {
        let list = self.list.get();
        unsafe { &(*list).flags[0..(*list).len()] }
    }

    pub fn clone_inner(&self) -> List2D {
        unsafe { self.list.get().as_ref().unwrap() }.clone()
    }

    pub fn iter(&self) -> InstanceIterator2D {
        InstanceIterator2D {
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
            *v = InstanceFlags2D::empty();
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
pub struct List2D {
    matrices: Vec<Mat4>,
    mesh_ids: Vec<MeshID>,
    flags: Vec<InstanceFlags2D>,

    ptr: AtomicUsize,
    free_slots: Vec<usize>,
    removed: Vec<usize>,
}

impl Clone for List2D {
    fn clone(&self) -> Self {
        let ptr = AtomicUsize::new(self.ptr.load(Ordering::Acquire));
        let this = Self {
            matrices: self.matrices.clone(),
            mesh_ids: self.mesh_ids.clone(),
            flags: self.flags.clone(),

            ptr,
            free_slots: self.free_slots.clone(),
            removed: self.removed.clone(),
        };

        self.ptr.load(Ordering::Acquire);
        this
    }
}

impl List2D {
    pub fn len(&self) -> usize {
        self.ptr.load(Ordering::Acquire)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug)]
pub struct InstanceIterator2D {
    list: Arc<UnsafeCell<List2D>>,
    current: usize,
    ptr: usize,
}

impl Clone for InstanceIterator2D {
    fn clone(&self) -> Self {
        Self {
            list: self.list.clone(),
            current: 0,
            ptr: unsafe { (*self.list.get()).ptr.load(Ordering::Acquire) },
        }
    }
}

impl Iterator for InstanceIterator2D {
    type Item = InstanceHandle2D;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.ptr {
            let index = self.current;
            self.current += 1;
            return Some(InstanceHandle2D {
                index,
                ptr: self.list.clone(),
            });
        }

        None
    }
}

#[derive(Debug)]
pub struct InstanceHandle2D {
    index: usize,
    ptr: Arc<UnsafeCell<List2D>>,
}

impl InstanceHandle2D {
    #[inline]
    pub fn set_mesh(&mut self, mesh: MeshID) {
        let list = unsafe { self.ptr.get().as_mut().unwrap() };
        list.mesh_ids[self.index] = mesh;
        list.flags[self.index] |= InstanceFlags2D::CHANGED_MESH;
    }

    #[inline]
    pub fn set_matrix(&mut self, matrix: Mat4) {
        let list = unsafe { self.ptr.get().as_mut().unwrap() };
        list.matrices[self.index] = matrix;
        list.flags[self.index] |= InstanceFlags2D::TRANSFORMED;
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
    pub fn get_matrix(&self) -> Mat4 {
        unsafe { (*self.ptr.get()).matrices[self.index] }
    }

    #[inline]
    pub fn get_mesh_id(&self) -> MeshID {
        unsafe { (*self.ptr.get()).mesh_ids[self.index] }
    }

    #[inline]
    pub fn get_flags(&self) -> InstanceFlags2D {
        unsafe { (*self.ptr.get()).flags[self.index] }
    }

    #[inline]
    pub fn get_id(&self) -> usize {
        self.index
    }

    #[inline]
    pub fn transformed(&self) -> bool {
        unsafe { (*self.ptr.get()).flags[self.index].contains(InstanceFlags2D::TRANSFORMED) }
    }

    #[inline]
    pub fn changed_mesh(&mut self) -> bool {
        unsafe { (*self.ptr.get()).flags[self.index].contains(InstanceFlags2D::CHANGED_MESH) }
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
