use scene::Camera;
use std::sync::Arc;
use crate::wgpu_renderer::instance::InstanceList;

#[async_trait]
pub trait RenderList<T: Default + Clone> {
    fn init(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, instances: InstanceList) -> Self;

    async fn render(&self, camera: &Camera);

    async fn set_renderable(&self, id: usize, object: T);
}
