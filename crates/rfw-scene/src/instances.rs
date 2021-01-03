use rfw_math::*;
use std::cell::UnsafeCell;
use std::fmt::{Display, Formatter};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

bitflags::bitflags! {
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[repr(transparent)]
    pub struct InstanceFlags: u32 {
        const TRANSFORMED = 1;
        const CHANGED_MESH = 2;
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct MeshID(pub(crate) i32);

impl Display for MeshID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "MeshID({})", self.0)
    }
}

impl MeshID {
    pub const INVALID: Self = MeshID(-1);

    pub fn is_valid(&self) -> bool {
        self.0 >= 0
    }

    pub fn as_index(&self) -> Option<usize> {
        if self.0 >= 0 {
            Some(self.0 as usize)
        } else {
            None
        }
    }
}

impl Into<usize> for MeshID {
    fn into(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct SkinID(pub(crate) i32);

impl SkinID {
    pub const INVALID: Self = SkinID(-1);

    pub fn is_valid(&self) -> bool {
        self.0 >= 0
    }

    pub fn as_index(&self) -> Option<usize> {
        if self.0 >= 0 {
            Some(self.0 as usize)
        } else {
            None
        }
    }
}

impl Into<usize> for SkinID {
    fn into(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone)]
pub struct InstanceList {
    pub(crate) list: Arc<UnsafeCell<List>>,
}

/// Although sharing instances amongst multiple threads without any mitigations against data races
/// is unsafe, the performance benefits of not doing any mitigation is too great to neglect this
/// opportunity (especially with many instances).
unsafe impl Send for InstanceList {}
unsafe impl Sync for InstanceList {}

impl From<List> for InstanceList {
    fn from(l: List) -> Self {
        Self {
            list: Arc::new(UnsafeCell::new(l)),
        }
    }
}

impl Into<List> for InstanceList {
    fn into(self) -> List {
        self.clone_inner()
    }
}

impl Default for InstanceList {
    fn default() -> Self {
        Self {
            list: Arc::new(UnsafeCell::new(List::default())),
        }
    }
}

impl InstanceList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        unsafe { (*self.list.get()).ptr.load(Ordering::SeqCst) }
    }

    pub fn allocate(&mut self) -> InstanceHandle {
        let list = unsafe { self.list.get().as_mut().unwrap() };
        if let Some(id) = list.free_slots.pop() {
            return InstanceHandle {
                index: id,
                ptr: self.list.clone(),
            };
        }

        let id = list.ptr.load(Ordering::Acquire);
        if (id + 1) >= list.matrices.len() {
            self.resize((id + 1) * 4);
        }
        list.ptr.store(id + 1, Ordering::Release);
        list.flags[id] = InstanceFlags::all();

        InstanceHandle {
            index: id,
            ptr: self.list.clone(),
        }
    }

    pub fn make_invalid(&mut self, handle: InstanceHandle) {
        let list = unsafe { self.list.get().as_mut().unwrap() };
        list.matrices[handle.index] = Mat4::identity();
        list.normal_matrices[handle.index] = Mat4::identity();
        list.mesh_ids[handle.index] = MeshID::INVALID;
        list.skin_ids[handle.index] = SkinID::INVALID;
        list.flags[handle.index] = InstanceFlags::all();
        list.free_slots.push(handle.index);
        list.removed.push(handle.index);
    }

    pub fn resize(&mut self, new_size: usize) {
        let list = unsafe { self.list.get().as_mut().unwrap() };
        list.matrices.resize(new_size, Mat4::identity());
        list.normal_matrices.resize(new_size, Mat4::identity());
        list.mesh_ids.resize(new_size, MeshID::INVALID);
        list.skin_ids.resize(new_size, SkinID::INVALID);
        list.flags.resize(new_size, InstanceFlags::empty());
    }

    pub fn get(&self, index: usize) -> Option<InstanceHandle> {
        let list = unsafe { self.list.get().as_mut().unwrap() };
        if let Some(_) = list.matrices.get(index) {
            Some(InstanceHandle {
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

    pub fn normal_matrices(&self) -> &[Mat4] {
        let list = self.list.get();
        unsafe { &(*list).normal_matrices[0..(*list).len()] }
    }

    pub fn mesh_ids(&self) -> &[MeshID] {
        let list = self.list.get();
        unsafe { &(*list).mesh_ids[0..(*list).len()] }
    }

    pub fn flags(&self) -> &[InstanceFlags] {
        let list = self.list.get();
        unsafe { &(*list).flags[0..(*list).len()] }
    }

    pub fn clone_inner(&self) -> List {
        unsafe { self.list.get().as_ref().unwrap() }.clone()
    }

    pub fn iter(&self) -> InstanceIterator {
        InstanceIterator {
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
            *v = InstanceFlags::empty();
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
pub struct List {
    matrices: Vec<Mat4>,
    normal_matrices: Vec<Mat4>,
    mesh_ids: Vec<MeshID>,
    skin_ids: Vec<SkinID>,
    flags: Vec<InstanceFlags>,

    ptr: AtomicUsize,
    free_slots: Vec<usize>,
    removed: Vec<usize>,
}

impl Clone for List {
    fn clone(&self) -> Self {
        let ptr = AtomicUsize::new(self.ptr.load(Ordering::Acquire));
        let this = Self {
            matrices: self.matrices.clone(),
            normal_matrices: self.normal_matrices.clone(),
            mesh_ids: self.mesh_ids.clone(),
            skin_ids: self.skin_ids.clone(),
            flags: self.flags.clone(),

            ptr,
            free_slots: self.free_slots.clone(),
            removed: self.removed.clone(),
        };

        self.ptr.load(Ordering::Release);
        this
    }
}

impl List {
    pub fn len(&self) -> usize {
        self.ptr.load(Ordering::SeqCst)
    }
}

#[derive(Debug)]
pub struct InstanceIterator {
    list: Arc<UnsafeCell<List>>,
    current: usize,
    ptr: usize,
}

impl Clone for InstanceIterator {
    fn clone(&self) -> Self {
        Self {
            list: self.list.clone(),
            current: 0,
            ptr: unsafe { (*self.list.get()).ptr.load(Ordering::AcqRel) },
        }
    }
}

impl Iterator for InstanceIterator {
    type Item = InstanceHandle;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.ptr {
            let index = self.current;
            self.current += 1;
            return Some(InstanceHandle {
                index,
                ptr: self.list.clone(),
            });
        }

        None
    }
}

#[derive(Debug, Default)]
pub struct InstanceHandle {
    index: usize,
    ptr: Arc<UnsafeCell<List>>,
}

impl InstanceHandle {
    #[inline]
    pub fn set_mesh(&mut self, mesh: MeshID) {
        let list = unsafe { self.ptr.get().as_mut().unwrap() };
        list.mesh_ids[self.index] = mesh;
        list.flags[self.index] |= InstanceFlags::CHANGED_MESH;
    }

    pub fn set_skin(&mut self, skin: SkinID) {
        let list = unsafe { self.ptr.get().as_mut().unwrap() };
        list.skin_ids[self.index] = skin;
    }

    #[inline]
    pub fn set_matrix(&mut self, matrix: Mat4) {
        let list = unsafe { self.ptr.get().as_mut().unwrap() };
        list.matrices[self.index] = matrix;
        list.normal_matrices[self.index] = matrix.inverse().transpose();
        list.flags[self.index] |= InstanceFlags::TRANSFORMED;
    }

    pub fn get_matrix(&self) -> Mat4 {
        unsafe { (*self.ptr.get()).matrices[self.index] }
    }

    pub fn get_normal_matrix(&self) -> Mat4 {
        unsafe { (*self.ptr.get()).normal_matrices[self.index] }
    }

    pub fn get_mesh_id(&self) -> MeshID {
        unsafe { (*self.ptr.get()).mesh_ids[self.index] }
    }

    pub fn get_skin_id(&self) -> SkinID {
        unsafe { (*self.ptr.get()).skin_ids[self.index] }
    }

    pub fn get_flags(&self) -> InstanceFlags {
        unsafe { (*self.ptr.get()).flags[self.index] }
    }

    pub fn get_id(&self) -> usize {
        self.index
    }

    #[inline]
    pub fn transformed(&self) -> bool {
        unsafe { (*self.ptr.get()).flags[self.index].contains(InstanceFlags::TRANSFORMED) }
    }

    #[inline]
    pub fn changed_mesh(&mut self) -> bool {
        unsafe { (*self.ptr.get()).flags[self.index].contains(InstanceFlags::CHANGED_MESH) }
    }
}

impl Clone for InstanceHandle {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            ptr: self.ptr.clone(),
        }
    }
}
