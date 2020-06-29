use scene::{raw_window_handle, renderers::Renderer};

use cocoa::{appkit::NSView, base::id as cocoa_id, foundation::NSRange};
use core_graphics::geometry::CGSize;
use metal::{
    Buffer, CommandQueue, CoreAnimationLayer, DeviceRef, Library, LibraryRef, MTLClearColor,
    MTLLoadAction, MTLPixelFormat, MTLPrimitiveType, MTLResourceOptions, MTLStoreAction,
    RenderPassDescriptor, RenderPassDescriptorRef, RenderPipelineDescriptor, RenderPipelineState,
    TextureRef,
};
use objc::runtime::YES;
use raw_window_handle::macos::MacOSHandle;

pub struct MetalRT {
    view: MacOSHandle,
    ns_view: cocoa_id,
    layer: CoreAnimationLayer,
    width: usize,
    height: usize,
    library: Library,
    pipeline_state: RenderPipelineState,
    command_queue: CommandQueue,
    vertex_buffer: Buffer,
    r: f32,
}

impl Renderer for MetalRT {
    fn init<T: raw_window_handle::HasRawWindowHandle>(
        window: &T,
        width: usize,
        height: usize,
    ) -> Result<Box<Self>, Box<dyn std::error::Error>> {
        let device = metal::Device::system_default().unwrap();

        let layer = CoreAnimationLayer::new();
        layer.set_device(&device);
        layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        layer.set_presents_with_transaction(false);

        unsafe {
            if let raw_window_handle::RawWindowHandle::MacOS(view) = window.raw_window_handle() {
                let ns_view: cocoa_id = view.ns_view as cocoa_id;

                ns_view.setWantsLayer(YES);
                ns_view.setLayer(std::mem::transmute(layer.as_ref()));

                layer.set_drawable_size(CGSize::new(width as f64, height as f64));

                let shaders: &str = include_str!("../shaders/shaders.metal");

                let compile_options = metal::CompileOptions::new();
                let library = device.new_library_with_source(shaders, &compile_options)?;

                let pipeline_state = Self::prepare_pipeline_state(&device, &library);
                let command_queue = device.new_command_queue();

                let vertex_buffer = {
                    let vertex_data = [
                        0.0f32, 0.5, 1.0, 0.0, 0.0, -0.5, -0.5, 0.0, 1.0, 0.0, 0.5, 0.5, 0.0, 0.0,
                        1.0,
                    ];

                    device.new_buffer_with_data(
                        vertex_data.as_ptr() as *const _,
                        (vertex_data.len() * std::mem::size_of::<f32>()) as u64,
                        MTLResourceOptions::CPUCacheModeDefaultCache
                            | MTLResourceOptions::StorageModeManaged,
                    )
                };

                Ok(Box::new(Self {
                    view,
                    ns_view,
                    layer,
                    width,
                    height,
                    library,
                    pipeline_state,
                    command_queue,
                    vertex_buffer,
                    r: 0.0,
                }))
            } else {
                panic!("Invalid window handle");
            }
        }
    }

    fn set_mesh(&mut self, _id: usize, _mesh: &scene::Mesh) {}

    fn set_instance(&mut self, _id: usize, _instance: &scene::Instance) {}

    fn set_materials(
        &mut self,
        _materials: &[scene::Material],
        _device_materials: &[scene::DeviceMaterial],
    ) {
    }

    fn set_textures(&mut self, _textures: &[scene::Texture]) {}

    fn synchronize(&mut self) {}

    fn render(&mut self, _camera: &scene::Camera, _mode: scene::renderers::RenderMode) {
        let drawable = match self.layer.next_drawable() {
            Some(drawable) => drawable,
            None => return,
        };

        let render_pass_descriptor = RenderPassDescriptor::new();
        let _a = Self::prepare_render_pass_descriptor(&render_pass_descriptor, drawable.texture());

        let command_buffer = self.command_queue.new_command_buffer();
        let encoder = command_buffer.new_render_command_encoder(&render_pass_descriptor);
        encoder.set_render_pipeline_state(&self.pipeline_state);
        encoder.set_vertex_buffer(0, Some(&self.vertex_buffer), 0);
        encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 3);
        encoder.end_encoding();

        render_pass_descriptor
            .color_attachments()
            .object_at(0)
            .unwrap()
            .set_load_action(MTLLoadAction::DontCare);

        let encoder = command_buffer.new_render_command_encoder(&render_pass_descriptor);
        let p = self.vertex_buffer.contents();
        let vertex_data = [
            0.0f32,
            0.5,
            1.0,
            0.0,
            0.0,
            -0.5 + (self.r.cos() / 2. + 0.5),
            -0.5,
            0.0,
            1.0,
            0.0,
            0.5 - (self.r.cos() / 2. + 0.5),
            -0.5,
            0.0,
            0.0,
            1.0,
        ];

        unsafe {
            std::ptr::copy(
                vertex_data.as_ptr(),
                p as *mut f32,
                (vertex_data.len() * std::mem::size_of::<f32>()) as usize,
            );
        }
        self.vertex_buffer.did_modify_range(NSRange::new(
            0 as u64,
            (vertex_data.len() * std::mem::size_of::<f32>()) as u64,
        ));

        encoder.set_render_pipeline_state(&self.pipeline_state);
        encoder.set_vertex_buffer(0, Some(&self.vertex_buffer), 0);
        encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 3);
        encoder.end_encoding();

        command_buffer.present_drawable(&drawable);
        command_buffer.commit();

        self.r += 0.01f32;
    }

    fn resize<T: scene::raw_window_handle::HasRawWindowHandle>(
        &mut self,
        _window: &T,
        _width: usize,
        _height: usize,
    ) {
    }

    fn set_point_lights(&mut self, _changed: &scene::BitVec, _lights: &[scene::PointLight]) {}

    fn set_spot_lights(&mut self, _changed: &scene::BitVec, _lights: &[scene::SpotLight]) {}

    fn set_area_lights(&mut self, _changed: &scene::BitVec, _lights: &[scene::AreaLight]) {}

    fn set_directional_lights(
        &mut self,
        _changed: &scene::BitVec,
        _lights: &[scene::DirectionalLight],
    ) {
    }

    fn get_settings(&self) -> Vec<scene::renderers::Setting> {
        todo!()
    }

    fn set_setting(&mut self, _setting: scene::renderers::Setting) {}
}

impl MetalRT {
    fn prepare_pipeline_state<'a>(device: &DeviceRef, library: &LibraryRef) -> RenderPipelineState {
        let vert = library.get_function("triangle_vertex", None).unwrap();
        let frag = library.get_function("triangle_fragment", None).unwrap();

        let pipeline_state_descriptor = RenderPipelineDescriptor::new();
        pipeline_state_descriptor.set_vertex_function(Some(&vert));
        pipeline_state_descriptor.set_fragment_function(Some(&frag));
        let attachment = pipeline_state_descriptor
            .color_attachments()
            .object_at(0)
            .unwrap();
        attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);

        attachment.set_blending_enabled(true);
        attachment.set_rgb_blend_operation(metal::MTLBlendOperation::Add);
        attachment.set_alpha_blend_operation(metal::MTLBlendOperation::Add);
        attachment.set_source_rgb_blend_factor(metal::MTLBlendFactor::SourceAlpha);
        attachment.set_source_alpha_blend_factor(metal::MTLBlendFactor::SourceAlpha);
        attachment.set_destination_rgb_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
        attachment.set_destination_alpha_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);

        device
            .new_render_pipeline_state(&pipeline_state_descriptor)
            .unwrap()
    }

    fn prepare_render_pass_descriptor(descriptor: &RenderPassDescriptorRef, texture: &TextureRef) {
        //descriptor.color_attachments().set_object_at(0, MTLRenderPassColorAttachmentDescriptor::alloc());
        //let color_attachment: MTLRenderPassColorAttachmentDescriptor = unsafe { msg_send![descriptor.color_attachments().0, _descriptorAtIndex:0] };//descriptor.color_attachments().object_at(0);
        let color_attachment = descriptor.color_attachments().object_at(0).unwrap();

        color_attachment.set_texture(Some(texture));
        color_attachment.set_load_action(MTLLoadAction::Clear);
        color_attachment.set_clear_color(MTLClearColor::new(0.0, 0.2, 0.2, 1.0));
        color_attachment.set_store_action(MTLStoreAction::Store);
    }
}
