use crate::hal;
use crate::mem::Buffer;
use rfw::prelude::*;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct GfxAnimMesh<B: hal::Backend> {
    pub id: usize,
    pub buffer: Option<Arc<Buffer<B>>>,
    pub anim_offset: usize,
    pub sub_meshes: Vec<VertexMesh>,
    pub vertices: usize,
    pub bounds: AABB,
}

impl<B: hal::Backend> Default for GfxAnimMesh<B> {
    fn default() -> Self {
        Self {
            id: 0,
            buffer: None,
            anim_offset: 0,
            sub_meshes: Vec::new(),
            vertices: 0,
            bounds: AABB::empty(),
        }
    }
}

#[allow(dead_code)]
impl<B: hal::Backend> GfxAnimMesh<B> {
    pub fn default_id(id: usize) -> Self {
        Self {
            id,
            ..Self::default()
        }
    }

    pub fn valid(&self) -> bool {
        self.buffer.is_some()
    }
}
