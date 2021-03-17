mod list;
mod mem;
mod objects;

use crate::objects::MetalMesh2D;
use cocoa::{
    appkit::NSView,
    base::{id as cocoa_id, YES},
};
use core::panic;
use list::*;
use mem::ManagedBuffer;
use metal::*;
use objects::{Matrices, MetalMesh3D};
use rfw::prelude::*;
use std::{collections::HashMap, rc::Rc};

#[derive(Default)]
#[repr(C)]
pub struct CameraUniform {
    pub projection: Mat4,
    pub view_matrix: Mat4,
    pub combined: Mat4,
    pub matrix_2d: Mat4,
    pub view: CameraView3D,
}

pub struct MetalBackend {
    device: Device,
    queue: CommandQueue,
    vertex_3d_list: VertexList<Vertex3D, JointData>,
    vertex_2d_list: VertexList<Vertex2D>,
    mesh_2d_textures: Vec<Option<usize>>,
    instance_3d_matrices: Vec<Rc<Vec<Matrices>>>,
    instance_3d_list: InstanceList<Matrices>,
    instance_2d_list: InstanceList<Mat4>,

    textures: Vec<metal::Texture>,
    camera: ManagedBuffer<CameraUniform>,
    state: RenderPipelineState,
    state_2d: RenderPipelineState,
    depth_state: DepthStencilState,
    depth_state_2d: DepthStencilState,
    layer: MetalLayer,
    materials: ManagedBuffer<DeviceMaterial>,
    depth_texture: metal::Texture,
    settings: MetalSettings,
    window_size: (u32, u32),
    render_size: (u32, u32),

    update_2d: bool,
    update_3d: bool,
    update_2d_instances: bool,
    update_3d_instances: bool,
}

pub struct MetalSettings {}

impl MetalBackend {
    pub const FORMAT: MTLPixelFormat = MTLPixelFormat::BGRA8Unorm;
    pub const DEPTH_FORMAT: MTLPixelFormat = MTLPixelFormat::Depth32Float;
    pub const T_FORMAT: MTLPixelFormat = MTLPixelFormat::BGRA8Unorm;
}

impl Backend for MetalBackend {
    type Settings = MetalSettings;

    fn init<T: HasRawWindowHandle>(
        window: &T,
        window_size: (u32, u32),
        scale_factor: f64,
    ) -> Result<Box<Self>, Box<dyn std::error::Error>> {
        let device = metal::Device::system_default().expect("Could not find Metal device");
        println!("Picked Metal device: {}", device.name());

        let layer = MetalLayer::new();
        layer.set_device(&device);
        layer.set_pixel_format(Self::FORMAT);
        layer.set_presents_with_transaction(false);
        layer.set_display_sync_enabled(false);
        layer.set_maximum_drawable_count(3);

        unsafe {
            match window.raw_window_handle() as RawWindowHandle {
                RawWindowHandle::MacOS(handle) => {
                    let view = handle.ns_view as cocoa_id;
                    view.setWantsLayer(YES);
                    view.setLayer(std::mem::transmute(layer.as_ref()));
                }
                _ => panic!("Unsupported platform."),
            };
        }

        layer.set_drawable_size(CGSize::new(
            window_size.0 as f64 * scale_factor,
            window_size.1 as f64 * scale_factor,
        ));

        let render_size = (
            (window_size.0 as f64 * scale_factor) as u32,
            (window_size.1 as f64 * scale_factor) as u32,
        );

        let queue = device.new_command_queue();

        let library_source = include_str!("../shaders/shaders.metal");
        let options = CompileOptions::new();
        options.set_fast_math_enabled(true);
        options.set_language_version(MTLLanguageVersion::V2_2);

        let library = device
            .new_library_with_source(library_source, &options)
            .expect("Could not compile shader library.");

        let desc = RenderPipelineDescriptor::new();
        let vert = library.get_function("triangle_vertex", None).unwrap();
        let frag = library.get_function("triangle_fragment", None).unwrap();
        desc.set_vertex_function(Some(&vert));
        desc.set_fragment_function(Some(&frag));
        desc.set_depth_attachment_pixel_format(Self::DEPTH_FORMAT);
        desc.set_input_primitive_topology(MTLPrimitiveTopologyClass::Triangle);
        desc.set_rasterization_enabled(true);
        {
            let attachment = desc.color_attachments().object_at(0).unwrap();
            attachment.set_pixel_format(Self::FORMAT);
            attachment.set_blending_enabled(false);
        }
        let state = device
            .new_render_pipeline_state(&desc)
            .expect("Could not initialize render pipeline state.");

        let desc = RenderPipelineDescriptor::new();
        let vert = library.get_function("triangle_vertex_2d", None).unwrap();
        let frag = library.get_function("triangle_fragment_2d", None).unwrap();
        desc.set_vertex_function(Some(&vert));
        desc.set_fragment_function(Some(&frag));
        desc.set_depth_attachment_pixel_format(Self::DEPTH_FORMAT);
        desc.set_input_primitive_topology(MTLPrimitiveTopologyClass::Triangle);
        desc.set_rasterization_enabled(true);
        {
            let attachment = desc.color_attachments().object_at(0).unwrap();
            attachment.set_pixel_format(Self::FORMAT);
            attachment.set_blending_enabled(true);

            attachment.set_rgb_blend_operation(metal::MTLBlendOperation::Add);
            attachment.set_alpha_blend_operation(metal::MTLBlendOperation::Add);
            attachment.set_source_rgb_blend_factor(metal::MTLBlendFactor::SourceAlpha);
            attachment.set_source_alpha_blend_factor(metal::MTLBlendFactor::SourceAlpha);
            attachment.set_destination_rgb_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
            attachment.set_destination_alpha_blend_factor(metal::MTLBlendFactor::Zero);
        }
        let state_2d = device
            .new_render_pipeline_state(&desc)
            .expect("Could not initialize render pipeline state.");

        let materials = ManagedBuffer::new(&device, 32);
        let camera = ManagedBuffer::new(&device, 1);

        let desc = metal::TextureDescriptor::new();
        desc.set_pixel_format(Self::DEPTH_FORMAT);
        desc.set_width(render_size.0 as _);
        desc.set_height(render_size.1 as _);
        desc.set_depth(1_u64);
        desc.set_texture_type(MTLTextureType::D2);
        desc.set_storage_mode(MTLStorageMode::Private);

        let depth_texture = device.new_texture(&desc);

        let depth_desc = DepthStencilDescriptor::new();
        depth_desc.set_depth_compare_function(MTLCompareFunction::Less);
        depth_desc.set_depth_write_enabled(true);
        let depth_state = device.new_depth_stencil_state(&depth_desc);

        let depth_desc = DepthStencilDescriptor::new();
        depth_desc.set_depth_compare_function(MTLCompareFunction::Always);
        depth_desc.set_depth_write_enabled(true);
        let depth_state_2d = device.new_depth_stencil_state(&depth_desc);

        Ok(Box::new(Self {
            device,
            queue,
            vertex_3d_list: Default::default(),
            vertex_2d_list: Default::default(),
            mesh_2d_textures: Default::default(),
            instance_3d_matrices: Default::default(),
            instance_3d_list: Default::default(),
            instance_2d_list: Default::default(),
            textures: Vec::new(),
            camera,
            state,
            state_2d,
            depth_state,
            depth_state_2d,
            layer,
            materials,

            depth_texture,
            settings: MetalSettings {},
            window_size,
            render_size,
            update_2d: false,
            update_3d: false,
            update_2d_instances: false,
            update_3d_instances: false,
        }))
    }

    fn set_2d_mesh(&mut self, id: usize, data: MeshData2D<'_>) {
        if id >= self.mesh_2d_textures.len() {
            self.mesh_2d_textures.resize(id + 1, Default::default());
        }

        self.mesh_2d_textures[id] = data.tex_id;

        if self.vertex_2d_list.has(id) {
            self.vertex_2d_list.update_pointer(
                id,
                data.vertices.as_ptr(),
                None,
                data.vertices.len() as u32,
            );
        } else {
            self.vertex_2d_list.add_pointer(
                id,
                data.vertices.as_ptr(),
                None,
                data.vertices.len() as u32,
            );
        }

        self.update_2d = true;
    }

    fn set_2d_instances(&mut self, mesh: usize, instances: InstancesData2D<'_>) {
        if self.instance_2d_list.has(mesh) {
            self.instance_2d_list.update_instances_list(
                mesh,
                instances.matrices.as_ptr(),
                instances.len() as u32,
            );
        } else {
            self.instance_2d_list.add_instances_list(
                mesh,
                instances.matrices.as_ptr(),
                instances.len() as u32,
            );
        }

        self.update_2d_instances = true;
    }

    fn set_3d_mesh(&mut self, id: usize, data: MeshData3D<'_>) {
        if self.vertex_3d_list.has(id) {
            self.vertex_3d_list.update_pointer(
                id,
                data.vertices.as_ptr(),
                if !data.skin_data.is_empty() {
                    Some(data.skin_data.as_ptr())
                } else {
                    None
                },
                data.vertices.len() as u32,
            );
        } else {
            self.vertex_3d_list.add_pointer(
                id,
                data.vertices.as_ptr(),
                if !data.skin_data.is_empty() {
                    Some(data.skin_data.as_ptr())
                } else {
                    None
                },
                data.vertices.len() as u32,
            );
        }

        self.update_3d = true;
    }

    fn unload_3d_meshes(&mut self, ids: Vec<usize>) {
        for id in ids {
            self.instance_3d_list.remove_instances_list(id);
            self.vertex_3d_list.remove_pointer(id);
        }
    }

    fn set_3d_instances(&mut self, mesh: usize, instances: InstancesData3D<'_>) {
        if mesh >= self.instance_3d_matrices.len() {
            self.instance_3d_matrices
                .resize(mesh + 1, Default::default());
        }

        let mut vec = vec![Default::default(); instances.len()];
        vec.iter_mut().enumerate().for_each(|(i, m)| {
            *m = Matrices::new(instances.matrices[i]);
        });

        self.instance_3d_matrices[mesh] = Rc::new(vec);

        if self.instance_3d_list.has(mesh) {
            self.instance_3d_list.update_instances_list(
                mesh,
                self.instance_3d_matrices[mesh].as_ptr(),
                instances.len() as u32,
            );
        } else {
            self.instance_3d_list.add_instances_list(
                mesh,
                self.instance_3d_matrices[mesh].as_ptr(),
                instances.len() as u32,
            );
        }

        self.update_3d_instances = true;
    }

    fn set_materials(&mut self, materials: &[DeviceMaterial], _changed: &BitSlice) {
        if self.materials.len() < materials.len() {
            self.materials = ManagedBuffer::with_data(&self.device, materials);
        } else {
            self.materials.as_mut(|slice| {
                slice[0..materials.len()].clone_from_slice(materials);
            });
        }
    }

    fn set_textures(&mut self, textures: &[TextureData<'_>], changed: &BitSlice) {
        if self.textures.len() != textures.len() {
            self.textures = textures
                .iter()
                .map(|tex| {
                    let texture_desc = metal::TextureDescriptor::new();
                    texture_desc.set_width(tex.width as _);
                    texture_desc.set_height(tex.height as _);
                    texture_desc.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
                    texture_desc.set_mipmap_level_count(tex.mip_levels as _);
                    texture_desc.set_sample_count(1);
                    texture_desc.set_storage_mode(MTLStorageMode::Managed);
                    texture_desc.set_texture_type(MTLTextureType::D2);
                    texture_desc.set_usage(MTLTextureUsage::ShaderRead);
                    let texture = self.device.new_texture(&texture_desc);
                    for m in 0..tex.mip_levels {
                        let (width, height) = tex.mip_level_width_height(m as _);
                        texture.replace_region(
                            MTLRegion {
                                origin: MTLOrigin { x: 0, y: 0, z: 0 },
                                size: MTLSize {
                                    width: width as _,
                                    height: height as _,
                                    depth: 1,
                                },
                            },
                            m as _,
                            tex.bytes.as_ptr() as _,
                            (width * std::mem::size_of::<u32>()) as _,
                        );
                    }
                    texture
                })
                .collect();
        } else {
            for i in 0..textures.len() {
                if !changed[i] {
                    continue;
                }
                let tex = &textures[i];

                let texture_desc = metal::TextureDescriptor::new();
                texture_desc.set_width(tex.width as _);
                texture_desc.set_height(tex.height as _);
                texture_desc.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
                texture_desc.set_mipmap_level_count(tex.mip_levels as _);
                texture_desc.set_sample_count(1);
                texture_desc.set_storage_mode(MTLStorageMode::Managed);
                texture_desc.set_texture_type(MTLTextureType::D2);
                texture_desc.set_usage(MTLTextureUsage::ShaderRead);
                let texture = self.device.new_texture(&texture_desc);
                for m in 0..tex.mip_levels {
                    let (width, height) = tex.mip_level_width_height(m as _);
                    texture.replace_region(
                        MTLRegion {
                            origin: MTLOrigin { x: 0, y: 0, z: 0 },
                            size: MTLSize {
                                width: width as _,
                                height: height as _,
                                depth: 1,
                            },
                        },
                        m as _,
                        tex.bytes.as_ptr() as _,
                        (width * std::mem::size_of::<u32>()) as _,
                    );
                }

                self.textures[i] = texture;
            }
        }
    }

    fn synchronize(&mut self) {
        if self.update_3d {
            self.vertex_3d_list.update_ranges();
            self.vertex_3d_list.update_data(&self.device);
        }

        if self.update_2d {
            self.vertex_2d_list.update_ranges();
            self.vertex_2d_list.update_data(&self.device);
        }

        if self.update_3d_instances {
            self.instance_3d_list.update_ranges();
            self.instance_3d_list.update_data(&self.device);
        }

        if self.update_2d_instances {
            self.instance_2d_list.update_ranges();
            self.instance_2d_list.update_data(&self.device);
        }

        self.update_2d = false;
        self.update_3d = false;
        self.update_2d_instances = false;
        self.update_3d_instances = false;
    }

    fn render(&mut self, camera_2d: CameraView2D, camera: CameraView3D, _mode: RenderMode) {
        self.camera.as_mut(|c| {
            let projection = camera.get_rh_projection();
            let view_matrix = camera.get_rh_view_matrix();
            c[0].projection = projection;
            c[0].view_matrix = view_matrix;
            c[0].combined = projection * view_matrix;
            c[0].matrix_2d = camera_2d.matrix;
            c[0].view = camera;
        });

        let drawable = if let Some(d) = self.layer.next_drawable() {
            d
        } else {
            return;
        };

        let render_desc = RenderPassDescriptor::new();
        {
            let depth_desc = render_desc.depth_attachment().unwrap();
            depth_desc.set_clear_depth(1.0);
            depth_desc.set_store_action(MTLStoreAction::Store);
            depth_desc.set_load_action(MTLLoadAction::Clear);
            depth_desc.set_texture(Some(&self.depth_texture));
            depth_desc.set_load_action(MTLLoadAction::Clear);
        }
        {
            let color_attachment = render_desc.color_attachments().object_at(0).unwrap();
            color_attachment.set_texture(Some(drawable.texture()));
            color_attachment.set_load_action(MTLLoadAction::Clear);
            color_attachment.set_clear_color(MTLClearColor::new(0.0, 0.0, 0.0, 1.0));
            color_attachment.set_store_action(MTLStoreAction::Store);
        }

        let command_buffer = self.queue.new_command_buffer();
        let encoder = command_buffer.new_render_command_encoder(&render_desc);

        let textures = self
            .textures
            .iter()
            .map(Self::to_texture_ref)
            .collect::<Vec<_>>();

        encoder.set_render_pipeline_state(&self.state);
        encoder.set_depth_stencil_state(&self.depth_state);
        encoder.set_front_facing_winding(MTLWinding::CounterClockwise);
        encoder.set_triangle_fill_mode(MTLTriangleFillMode::Fill);
        encoder.set_cull_mode(MTLCullMode::Back);

        let vertex_buffer = self.vertex_3d_list.get_vertex_buffer();
        let instance_buffer = self.instance_3d_list.get_buffer();

        if let (Some(vertex_buffer), Some(instance_buffer)) = (vertex_buffer, instance_buffer) {
            encoder.set_vertex_buffer(0, Some(&*vertex_buffer), 0);
            encoder.set_vertex_buffer(1, Some(&self.camera), 0);
            encoder.set_vertex_buffer(2, Some(&*instance_buffer), 0);

            let ranges = self.vertex_3d_list.get_ranges();
            let instances = self.instance_3d_list.get_ranges();
            encoder.set_fragment_buffer(0, Some(&self.materials), 0);
            encoder.set_fragment_textures(0, textures.as_slice());

            for (i, range) in ranges.iter() {
                let instances = if let Some(i) = instances.get(i) {
                    i
                } else {
                    continue;
                };
                if instances.count == 0 {
                    continue;
                }

                encoder.draw_primitives_instanced_base_instance(
                    MTLPrimitiveType::Triangle,
                    range.start as u64,
                    (range.end - range.start) as u64,
                    instances.count as u64,
                    instances.start as u64,
                );
            }
        }

        let vertex_buffer = self.vertex_2d_list.get_vertex_buffer();
        let instance_buffer = self.instance_2d_list.get_buffer();

        if let (Some(vertex_buffer), Some(instance_buffer)) = (vertex_buffer, instance_buffer) {
            encoder.set_vertex_buffer(0, Some(&*vertex_buffer), 0);
            encoder.set_vertex_buffer(1, Some(&*instance_buffer), 0);
            encoder.set_vertex_buffer(2, Some(&self.camera), 0);

            let ranges = self.vertex_2d_list.get_ranges();
            let instances = self.instance_2d_list.get_ranges();

            encoder.set_render_pipeline_state(&self.state_2d);
            encoder.set_depth_stencil_state(&self.depth_state_2d);
            encoder.set_front_facing_winding(MTLWinding::CounterClockwise);
            encoder.set_triangle_fill_mode(MTLTriangleFillMode::Fill);
            encoder.set_cull_mode(MTLCullMode::None);

            for (i, range) in ranges.iter() {
                let instances = if let Some(i) = instances.get(i) {
                    i
                } else {
                    continue;
                };

                if instances.count == 0 {
                    continue;
                }

                encoder.set_fragment_texture(
                    0,
                    Some(&self.textures[self.mesh_2d_textures[*i].unwrap_or(0)]),
                );
                encoder.draw_primitives_instanced_base_instance(
                    MTLPrimitiveType::Triangle,
                    range.start as u64,
                    (range.end - range.start) as u64,
                    instances.count as u64,
                    instances.start as u64,
                );
            }
        }

        encoder.end_encoding();

        command_buffer.present_drawable(&drawable);
        command_buffer.commit();
    }

    fn resize<T: HasRawWindowHandle>(
        &mut self,
        _window: &T,
        window_size: (u32, u32),
        scale_factor: f64,
    ) {
        self.layer.set_drawable_size(CGSize::new(
            window_size.0 as f64 * scale_factor,
            window_size.1 as f64 * scale_factor,
        ));

        self.window_size = window_size;
        self.render_size = (
            (window_size.0 as f64 * scale_factor) as u32,
            (window_size.1 as f64 * scale_factor) as u32,
        );

        let desc = metal::TextureDescriptor::new();
        desc.set_pixel_format(Self::DEPTH_FORMAT);
        desc.set_width(self.render_size.0 as _);
        desc.set_height(self.render_size.1 as _);
        desc.set_depth(1_u64);
        desc.set_texture_type(MTLTextureType::D2);
        desc.set_storage_mode(MTLStorageMode::Private);
        self.depth_texture = self.device.new_texture(&desc);
    }

    fn set_point_lights(&mut self, _lights: &[PointLight], _changed: &BitSlice) {}

    fn set_spot_lights(&mut self, _lights: &[SpotLight], _changed: &BitSlice) {}

    fn set_area_lights(&mut self, _lights: &[AreaLight], _changed: &BitSlice) {}

    fn set_directional_lights(&mut self, _lights: &[DirectionalLight], _changed: &BitSlice) {}

    fn set_skybox(&mut self, _skybox: TextureData<'_>) {}

    fn set_skins(&mut self, _skins: &[SkinData<'_>], _changed: &BitSlice) {}

    fn settings(&mut self) -> &mut Self::Settings {
        &mut self.settings
    }
}

impl MetalBackend {
    fn to_texture_ref(texture: &metal::Texture) -> Option<&metal::TextureRef> {
        Some(texture)
    }
}

#[derive(Debug, Default)]
#[repr(C)]
struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Default)]
#[repr(C)]
struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Debug, Default)]
#[repr(C)]
struct ClearRect {
    pub rect: Rect,
    pub color: Color,
}
