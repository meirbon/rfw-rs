use crate::{
    list::{InstanceList, VertexList},
    mesh::WgpuSkin,
};
use crate::{InstanceMatrices, WgpuSettings};
use rfw::prelude::{AABB, *};
use std::borrow::Cow;
use std::fmt::Debug;
use std::num::NonZeroU32;
use std::ops::Range;
use wgpu::util::DeviceExt;

pub struct WgpuLights {
    // point_lights: LightShadows<PointLight>,
    pub spot_lights: LightShadows<SpotLight>,
    pub area_lights: LightShadows<AreaLight>,
    pub directional_lights: LightShadows<DirectionalLight>,
}

impl WgpuLights {
    pub fn new(
        capacity: usize,
        device: &wgpu::Device,
        instance_bind_group_layout: &wgpu::BindGroupLayout,
        skin_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            // point_lights: LightShadows::new(device, instance_bind_group_layout, capacity, false),
            spot_lights: LightShadows::new(
                device,
                instance_bind_group_layout,
                skin_layout,
                capacity,
                false,
            ),
            area_lights: LightShadows::new(
                device,
                instance_bind_group_layout,
                skin_layout,
                capacity,
                false,
            ),
            directional_lights: LightShadows::new(
                device,
                instance_bind_group_layout,
                skin_layout,
                capacity,
                true,
            ),
        }
    }

    pub fn counts(&self) -> [u32; 4] {
        [
            0,
            self.spot_lights.len() as u32,
            self.area_lights.len() as u32,
            self.directional_lights.len() as u32,
        ]
    }

    pub fn set_spot_lights(
        &mut self,
        changed: &BitSlice,
        lights: &[SpotLight],
        scene_bounds: &AABB,
    ) {
        self.spot_lights.set(changed, lights, scene_bounds);
    }

    pub fn set_area_lights(
        &mut self,
        changed: &BitSlice,
        lights: &[AreaLight],
        scene_bounds: &AABB,
    ) {
        self.area_lights.set(changed, lights, scene_bounds);
    }

    pub fn set_directional_lights(
        &mut self,
        changed: &BitSlice,
        lights: &[DirectionalLight],
        scene_bounds: &AABB,
    ) {
        self.directional_lights.set(changed, lights, scene_bounds);
    }

    pub fn synchronize(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> bool {
        if !self.spot_lights.needs_update()
            && !self.area_lights.needs_update()
            && !self.directional_lights.needs_update()
        {
            return false;
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("light-mem-copy"),
        });

        self.spot_lights.synchronize(&mut encoder, device, queue);
        self.area_lights.synchronize(&mut encoder, device, queue);
        self.directional_lights
            .synchronize(&mut encoder, device, queue);

        queue.submit(std::iter::once(encoder.finish()));
        true
    }

    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        uniform_bind_group: &wgpu::BindGroup,
        vertices: &VertexList<Vertex3D, JointData>,
        instances: &InstanceList<InstanceMatrices, Vec<Option<u16>>>,
        skins: &[WgpuSkin],
        settings: &WgpuSettings,
    ) {
        self.area_lights.render(
            encoder,
            uniform_bind_group,
            vertices,
            instances,
            skins,
            settings,
        );
        self.spot_lights.render(
            encoder,
            uniform_bind_group,
            vertices,
            instances,
            skins,
            settings,
        );
        self.directional_lights.render(
            encoder,
            uniform_bind_group,
            vertices,
            instances,
            skins,
            settings,
        );
    }
}

pub struct LightShadows<T: Sized + Light + Clone + Debug + Default> {
    lights: TrackedStorage<T>,
    light_buffer: wgpu::Buffer,
    light_buffer_size: wgpu::BufferAddress,
    info: Vec<LightInfo>,
    shadow_maps: ShadowMapArray,
}

impl<T: Sized + Light + Clone + Debug + Default> LightShadows<T> {
    pub fn new(
        device: &wgpu::Device,
        instance_bind_group_layout: &wgpu::BindGroupLayout,
        skin_layout: &wgpu::BindGroupLayout,
        capacity: usize,
        linear: bool,
    ) -> Self {
        let light_buffer_size = (capacity * std::mem::size_of::<T>()) as wgpu::BufferAddress;
        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light-mem"),
            size: light_buffer_size,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            lights: TrackedStorage::new(),
            light_buffer,
            light_buffer_size,
            info: Vec::new(),
            shadow_maps: ShadowMapArray::new(
                device,
                capacity,
                instance_bind_group_layout,
                skin_layout,
                linear,
            ),
        }
    }

    pub fn set(&mut self, changed: &BitSlice, lights: &[T], scene_bounds: &AABB) {
        self.lights = TrackedStorage::from(lights);
        self.lights.reset_changed();
        (0..lights.len())
            .into_iter()
            .filter(|i| *changed.get(*i).unwrap())
            .for_each(|i| self.lights.trigger_changed(i));

        self.info.resize(lights.len(), LightInfo::default());

        for (i, _) in self.lights.iter().filter(|(i, _)| match changed.get(*i) {
            Some(val) => *val,
            None => false,
        }) {
            let info = self.lights[i].get_light_info(scene_bounds);
            self.info[i] = info;
        }
    }

    pub fn needs_update(&self) -> bool {
        !self.is_empty() && self.lights.any_changed()
    }

    pub fn synchronize(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> bool {
        if self.is_empty() || !self.lights.any_changed() {
            return false;
        }

        let mut changed = self.shadow_maps.resize(device, queue, self.lights.len());

        let light_buffer_size =
            (self.lights.len() * std::mem::size_of::<T>()) as wgpu::BufferAddress;
        if light_buffer_size > self.light_buffer_size {
            self.light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("light-mem"),
                size: light_buffer_size,
                usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
                mapped_at_creation: false,
            });
            self.light_buffer_size = light_buffer_size;

            changed = true;
        }

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lights-staging-mem"),
            size: self.light_buffer_size,
            usage: wgpu::BufferUsage::COPY_SRC,
            mapped_at_creation: true,
        });

        staging_buffer
            .slice(0..light_buffer_size as _)
            .get_mapped_range_mut()
            .copy_from_slice(unsafe {
                std::slice::from_raw_parts(
                    self.lights.as_ptr() as *const u8,
                    light_buffer_size as usize,
                )
            });

        staging_buffer.unmap();

        encoder.copy_buffer_to_buffer(&staging_buffer, 0, &self.light_buffer, 0, light_buffer_size);
        self.shadow_maps.update_infos(self.info.as_slice(), queue);

        changed
    }

    pub fn len(&self) -> usize {
        self.lights.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lights.is_empty()
    }

    pub fn uniform_binding(&self, binding: u32) -> wgpu::BindGroupEntry {
        wgpu::BindGroupEntry {
            binding,
            resource: self.light_buffer.as_entire_binding(),
        }
    }

    pub fn shadow_map_binding(&self, binding: u32) -> wgpu::BindGroupEntry {
        wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(&self.shadow_maps.view),
        }
    }

    pub fn infos_binding(&self, binding: u32) -> wgpu::BindGroupEntry {
        wgpu::BindGroupEntry {
            binding,
            resource: self.shadow_maps.uniform_buffer.as_entire_binding(),
        }
    }

    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        uniform_bind_group: &wgpu::BindGroup,
        vertices: &VertexList<Vertex3D, JointData>,
        instances: &InstanceList<InstanceMatrices, Vec<Option<u16>>>,
        skins: &[WgpuSkin],
        settings: &WgpuSettings,
    ) {
        self.shadow_maps.render(
            0..self.lights.len() as u32,
            encoder,
            uniform_bind_group,
            vertices,
            instances,
            skins,
            settings,
        );

        self.lights.reset_changed();
    }
}

#[allow(dead_code)]
pub struct ShadowMapArray {
    pub map: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub views: Vec<wgpu::TextureView>,

    filter_map: wgpu::Texture,
    filter_view: wgpu::TextureView,
    filter_views: Vec<wgpu::TextureView>,

    depth_map: wgpu::Texture,
    depth_view: wgpu::TextureView,
    pub uniform_buffer: wgpu::Buffer,

    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,

    pipeline_layout: wgpu::PipelineLayout,
    pipeline: wgpu::RenderPipeline,

    anim_pipeline_layout: wgpu::PipelineLayout,
    anim_pipeline: wgpu::RenderPipeline,

    filter_uniform_direction_buffer: wgpu::Buffer,
    filter_direction_x: wgpu::Buffer,
    filter_direction_y: wgpu::Buffer,
    filter_bind_group_layout: wgpu::BindGroupLayout,
    filter_bind_groups1: Vec<wgpu::BindGroup>,
    filter_bind_groups2: Vec<wgpu::BindGroup>,
    filter_pipeline_layout: wgpu::PipelineLayout,
    filter_pipeline: wgpu::ComputePipeline,
    light_infos: Vec<LightInfo>,
}

impl ShadowMapArray {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg32Float;
    pub const WIDTH: usize = 2048;
    pub const HEIGHT: usize = 2048;
    pub const UNIFORM_ELEMENT_SIZE: usize = std::mem::size_of::<LightInfo>();

    pub fn new(
        device: &wgpu::Device,
        count: usize,
        instance_bind_group_layout: &wgpu::BindGroupLayout,
        skin_layout: &wgpu::BindGroupLayout,
        linear: bool,
    ) -> ShadowMapArray {
        let map = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_map"),
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT
                | wgpu::TextureUsage::SAMPLED
                | wgpu::TextureUsage::STORAGE
                | wgpu::TextureUsage::COPY_SRC
                | wgpu::TextureUsage::COPY_DST,
            size: wgpu::Extent3d {
                width: Self::WIDTH as u32,
                height: Self::HEIGHT as u32,
                depth: count as u32,
            },
            dimension: wgpu::TextureDimension::D2,
            sample_count: 1,
            mip_level_count: 1,
            format: Self::FORMAT,
        });

        let view = map.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: NonZeroU32::new(count as _),
        });

        let views: Vec<wgpu::TextureView> = (0..count)
            .map(|i| {
                map.create_view(&wgpu::TextureViewDescriptor {
                    label: None,
                    format: Some(Self::FORMAT),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    level_count: None,
                    base_array_layer: i as u32,
                    array_layer_count: NonZeroU32::new(1),
                })
            })
            .collect();

        let filter_map = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_map"),
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT
                | wgpu::TextureUsage::SAMPLED
                | wgpu::TextureUsage::STORAGE
                | wgpu::TextureUsage::COPY_SRC
                | wgpu::TextureUsage::COPY_DST,
            size: wgpu::Extent3d {
                width: Self::WIDTH as u32,
                height: Self::HEIGHT as u32,
                depth: count as u32,
            },
            dimension: wgpu::TextureDimension::D2,
            sample_count: 1,
            mip_level_count: 1,
            format: Self::FORMAT,
        });

        let filter_view = filter_map.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: NonZeroU32::new(count as u32),
        });

        let filter_views: Vec<wgpu::TextureView> = (0..count)
            .map(|i| {
                filter_map.create_view(&wgpu::TextureViewDescriptor {
                    label: None,
                    format: Some(Self::FORMAT),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    level_count: None,
                    base_array_layer: i as u32,
                    array_layer_count: NonZeroU32::new(1),
                })
            })
            .collect();

        let depth_map = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_map"),
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT
                | wgpu::TextureUsage::SAMPLED
                | wgpu::TextureUsage::COPY_SRC
                | wgpu::TextureUsage::COPY_DST,
            size: wgpu::Extent3d {
                width: Self::WIDTH as u32,
                height: Self::HEIGHT as u32,
                depth: 1,
            },
            dimension: wgpu::TextureDimension::D2,
            sample_count: 1,
            mip_level_count: 1,
            format: Self::DEPTH_FORMAT,
        });

        let depth_view = depth_map.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::DEPTH_FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::DepthOnly,
            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });

        let uniform_size = (count * Self::UNIFORM_ELEMENT_SIZE) as wgpu::BufferAddress;
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow-map-uniform-mem"),
            size: uniform_size,
            usage: wgpu::BufferUsage::UNIFORM
                | wgpu::BufferUsage::COPY_SRC
                | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow-map-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                count: None,
                visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    has_dynamic_offset: true,
                    ty: wgpu::BufferBindingType::Uniform,
                    min_binding_size: wgpu::BufferSize::new(Self::UNIFORM_ELEMENT_SIZE as _),
                },
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            label: Some("shadow-map-uniform-bind-group"),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(Self::UNIFORM_ELEMENT_SIZE as _),
                },
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout, instance_bind_group_layout],
            push_constant_ranges: &[],
        });

        let vert_shader: &[u8] = include_bytes!("../shaders/shadow_single.vert.spv",);
        // let anim_vert_shader: &[u8] = include_bytes!("../shaders/shadow_single.vert.spv",);
        let anim_vert_shader: &[u8] = include_bytes!("../shaders/shadow_single_anim.vert.spv",);

        let regular_frag_shader: &[u8] = include_bytes!("../shaders/shadow_single.frag.spv");
        let linear_frag_shader: &[u8] = include_bytes!("../shaders/shadow_single_linear.frag.spv");

        let vert_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            flags: Default::default(),
            label: None,
            source: wgpu::ShaderSource::SpirV(Cow::from(vert_shader.as_quad_bytes())),
        });

        let anim_vert_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            flags: Default::default(),
            label: None,
            source: wgpu::ShaderSource::SpirV(Cow::from(anim_vert_shader.as_quad_bytes())),
        });

        let frag_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            flags: Default::default(),
            label: None,
            source: wgpu::ShaderSource::SpirV(Cow::from(if linear {
                linear_frag_shader.as_quad_bytes()
            } else {
                regular_frag_shader.as_quad_bytes()
            })),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex3D>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        offset: 0,
                        format: wgpu::VertexFormat::Float4,
                        shader_location: 0,
                    }],
                }],
                entry_point: "main",
                module: &vert_module,
            },
            fragment: Some(wgpu::FragmentState {
                entry_point: "main",
                module: &frag_module,
                targets: &[wgpu::ColorTargetState {
                    format: Self::FORMAT,
                    alpha_blend: wgpu::BlendState::REPLACE,
                    color_blend: wgpu::BlendState::REPLACE,
                    write_mask: wgpu::ColorWrite::ALL,
                }],
            }),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Self::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
                clamp_depth: false,
            }),
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                topology: wgpu::PrimitiveTopology::TriangleList,
                polygon_mode: wgpu::PolygonMode::Fill,
                strip_index_format: None,
            },
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        });

        let anim_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout, instance_bind_group_layout, skin_layout],
            push_constant_ranges: &[],
        });

        let anim_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow-pipeline"),
            layout: Some(&anim_pipeline_layout),
            vertex: wgpu::VertexState {
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex3D>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        offset: 0,
                        format: wgpu::VertexFormat::Float4,
                        shader_location: 0,
                    }],
                }, wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<JointData>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        offset: 0,
                        format: wgpu::VertexFormat::Uint4,
                        shader_location: 1,
                    }, wgpu::VertexAttribute {
                        offset: 16,
                        format: wgpu::VertexFormat::Float4,
                        shader_location: 2,
                    }],
                }],
                entry_point: "main",
                module: &anim_vert_module,
            },
            fragment: Some(wgpu::FragmentState {
                entry_point: "main",
                module: &frag_module,
                targets: &[wgpu::ColorTargetState {
                    format: Self::FORMAT,
                    alpha_blend: wgpu::BlendState::REPLACE,
                    color_blend: wgpu::BlendState::REPLACE,
                    write_mask: wgpu::ColorWrite::ALL,
                }],
            }),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Self::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
                clamp_depth: false,
            }),
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                topology: wgpu::PrimitiveTopology::TriangleList,
                polygon_mode: wgpu::PolygonMode::Fill,
                strip_index_format: None,
            },
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        });

        let filter_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("filter-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            format: Self::FORMAT,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::ReadOnly,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            format: Self::DEPTH_FORMAT,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        count: None,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            has_dynamic_offset: false,
                            ty: wgpu::BufferBindingType::Uniform,
                            min_binding_size: None,
                        },
                    },
                ],
            });

        let direction_x: [f32; 2] = [1.0, 0.0];
        let direction_y: [f32; 2] = [0.0, 1.0];
        let dir_x = unsafe { std::slice::from_raw_parts(direction_x.as_ptr() as *const u8, 8) };
        let dir_y = unsafe { std::slice::from_raw_parts(direction_y.as_ptr() as *const u8, 8) };
        let filter_direction_x = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: dir_x,
            usage: wgpu::BufferUsage::COPY_SRC,
        });
        let filter_direction_y = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: dir_y,
            usage: wgpu::BufferUsage::COPY_SRC,
        });

        let filter_uniform_direction_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("filter-uniform-direction-mem"),
            size: 8,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        let filter_bind_groups1: Vec<wgpu::BindGroup> = views
            .iter()
            .zip(filter_views.iter())
            .map(|(v1, v2)| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("filter-bind-group"),
                    layout: &filter_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(v2),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(v1),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: filter_uniform_direction_buffer.as_entire_binding(),
                        },
                    ],
                })
            })
            .collect();

        // TODO: Use push constants instead uniform bindings for filter direction
        let filter_bind_groups2: Vec<wgpu::BindGroup> = views
            .iter()
            .zip(filter_views.iter())
            .map(|(v1, v2)| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("filter-bind-group"),
                    layout: &filter_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(v1),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(v2),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: filter_uniform_direction_buffer.as_entire_binding(),
                        },
                    ],
                })
            })
            .collect();

        let filter_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&filter_bind_group_layout],
                push_constant_ranges: &[],
            });
        let shader: &[u8] = include_bytes!("../shaders/shadow_filter.comp.spv");
        let shader_module = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            flags: Default::default(),
            label: None,
            source: wgpu::ShaderSource::SpirV(Cow::from(shader.as_quad_bytes())),
        });
        let filter_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("shadow-filter-pipeline"),
            layout: Some(&filter_pipeline_layout),
            entry_point: "main",
            module: &shader_module,
        });

        Self {
            map,
            view,
            views,
            filter_map,
            filter_view,
            filter_views,
            depth_map,
            depth_view,
            uniform_buffer,
            bind_group_layout,
            bind_group,
            pipeline_layout,
            pipeline,
            anim_pipeline_layout,
            anim_pipeline,
            filter_uniform_direction_buffer,
            filter_direction_x,
            filter_direction_y,
            filter_bind_group_layout,
            filter_bind_groups1,
            filter_bind_groups2,
            filter_pipeline_layout,
            filter_pipeline,
            light_infos: vec![LightInfo::default(); count],
        }
    }

    pub fn len(&self) -> usize {
        self.views.len()
    }

    // Resizes texture to accommodate new size (do not do this too often, very expensive operation!)
    pub fn resize(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, size: usize) -> bool {
        if size <= self.len() {
            return false;
        }

        // Allocate more memory to make sure this does not run too often
        let size = size.max(self.len() * 2);

        let map = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_map"),
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT
                | wgpu::TextureUsage::SAMPLED
                | wgpu::TextureUsage::STORAGE
                | wgpu::TextureUsage::COPY_SRC
                | wgpu::TextureUsage::COPY_DST,
            size: wgpu::Extent3d {
                width: Self::WIDTH as u32,
                height: Self::HEIGHT as u32,
                depth: size as u32,
            },
            dimension: wgpu::TextureDimension::D2,
            sample_count: 1,
            mip_level_count: 1,
            format: Self::FORMAT,
        });

        let view = map.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: NonZeroU32::new(size as u32),
        });

        let views: Vec<wgpu::TextureView> = (0..size)
            .map(|i| {
                map.create_view(&wgpu::TextureViewDescriptor {
                    label: None,
                    format: Some(Self::FORMAT),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    level_count: None,
                    base_array_layer: i as u32,
                    array_layer_count: NonZeroU32::new(1),
                })
            })
            .collect();

        // Create new texture
        let new_depth_map = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_map"),
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT
                | wgpu::TextureUsage::SAMPLED
                | wgpu::TextureUsage::COPY_SRC
                | wgpu::TextureUsage::COPY_DST,
            size: wgpu::Extent3d {
                width: Self::WIDTH as u32,
                height: Self::HEIGHT as u32,
                depth: size as u32,
            },
            dimension: wgpu::TextureDimension::D2,
            sample_count: 1,
            mip_level_count: 1,
            format: Self::DEPTH_FORMAT,
        });

        let new_depth_view = new_depth_map.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::DEPTH_FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::DepthOnly,
            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: NonZeroU32::new(size as u32),
        });

        let filter_map = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_map"),
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT
                | wgpu::TextureUsage::SAMPLED
                | wgpu::TextureUsage::STORAGE
                | wgpu::TextureUsage::COPY_SRC
                | wgpu::TextureUsage::COPY_DST,
            size: wgpu::Extent3d {
                width: Self::WIDTH as u32,
                height: Self::HEIGHT as u32,
                depth: size as u32,
            },
            dimension: wgpu::TextureDimension::D2,
            sample_count: 1,
            mip_level_count: 1,
            format: Self::FORMAT,
        });

        let filter_view = filter_map.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            level_count: None,
            base_array_layer: 0,
            array_layer_count: NonZeroU32::new(size as u32),
        });

        let filter_views: Vec<wgpu::TextureView> = (0..size)
            .map(|i| {
                filter_map.create_view(&wgpu::TextureViewDescriptor {
                    label: None,
                    format: Some(Self::FORMAT),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    level_count: None,
                    base_array_layer: i as u32,
                    array_layer_count: NonZeroU32::new(1),
                })
            })
            .collect();

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("map-transfer-encoder"),
        });

        // Copy over shadow maps for already existing maps
        for i in 0..self.len() {
            encoder.copy_texture_to_texture(
                wgpu::TextureCopyView {
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: i as _,
                    },
                    texture: &self.map,
                },
                wgpu::TextureCopyView {
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: i as _,
                    },
                    texture: &map,
                },
                wgpu::Extent3d {
                    width: Self::WIDTH as u32,
                    height: Self::HEIGHT as u32,
                    depth: 1,
                },
            );

            encoder.copy_texture_to_texture(
                wgpu::TextureCopyView {
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: i as _,
                    },
                    texture: &self.filter_map,
                },
                wgpu::TextureCopyView {
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: i as _,
                    },
                    texture: &filter_map,
                },
                wgpu::Extent3d {
                    width: Self::WIDTH as u32,
                    height: Self::HEIGHT as u32,
                    depth: 1,
                },
            );
        }

        let filter_bind_groups1: Vec<wgpu::BindGroup> = views
            .iter()
            .zip(filter_views.iter())
            .map(|(v1, v2)| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("filter-bind-group"),
                    layout: &self.filter_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(v2),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(v1),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: self.filter_uniform_direction_buffer.as_entire_binding(),
                        },
                    ],
                })
            })
            .collect();

        let filter_bind_groups2: Vec<wgpu::BindGroup> = views
            .iter()
            .zip(filter_views.iter())
            .map(|(v1, v2)| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("filter-bind-group"),
                    layout: &self.filter_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(v1),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(v2),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: self.filter_uniform_direction_buffer.as_entire_binding(),
                        },
                    ],
                })
            })
            .collect();

        let new_size = (size * Self::UNIFORM_ELEMENT_SIZE) as wgpu::BufferAddress;

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow-map-uniform-mem"),
            size: new_size,
            usage: wgpu::BufferUsage::UNIFORM
                | wgpu::BufferUsage::COPY_SRC
                | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.bind_group_layout,
            label: Some("shadow-map-uniform-bind-group"),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(Self::UNIFORM_ELEMENT_SIZE as _),
                },
            }],
        });

        encoder.copy_buffer_to_buffer(
            &self.uniform_buffer,
            0,
            &uniform_buffer,
            0,
            (self.views.len() * Self::UNIFORM_ELEMENT_SIZE) as wgpu::BufferAddress,
        );

        queue.submit(std::iter::once(encoder.finish()));

        self.uniform_buffer = uniform_buffer;

        self.view = view;
        self.views = views;
        self.map = map;

        self.depth_view = new_depth_view;
        self.depth_map = new_depth_map;

        self.filter_view = filter_view;
        self.filter_views = filter_views;
        self.filter_bind_groups1 = filter_bind_groups1;
        self.filter_bind_groups2 = filter_bind_groups2;
        self.filter_map = filter_map;

        self.light_infos.resize(size, LightInfo::default());
        true
    }

    pub fn create_sampler(device: &wgpu::Device) -> wgpu::Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            compare: None,
            lod_max_clamp: 1.0,
            lod_min_clamp: 0.0,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        })
    }

    pub fn update_infos(&mut self, infos: &[LightInfo], queue: &wgpu::Queue) {
        self.light_infos = Vec::from(infos);
        queue.write_buffer(&self.uniform_buffer, 0, infos.as_bytes());
    }

    pub fn render(
        &self,
        range: Range<u32>,
        encoder: &mut wgpu::CommandEncoder,
        uniform_bind_group: &wgpu::BindGroup,
        vertices: &VertexList<Vertex3D, JointData>,
        instances: &InstanceList<InstanceMatrices, Vec<Option<u16>>>,
        skins: &[WgpuSkin],
        settings: &WgpuSettings,
    ) {
        let start = range.start;
        let end = range.end;

        let v_ranges = vertices.get_ranges();
        let i_ranges = instances.get_ranges();

        assert!(range.end as usize <= self.views.len());
        for v in range.into_iter() {
            let frustrum = FrustrumG::from_matrix(self.light_infos[v as usize].pm);

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &self.views[v as usize],
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                    attachment: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            let jw_buffer = vertices.get_jw_buffer().buffer();
            let vertex_buffer = vertices.get_vertex_buffer().buffer();

            render_pass.set_bind_group(0, uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(
                0,
                &self.bind_group,
                &[(v as usize * Self::UNIFORM_ELEMENT_SIZE) as wgpu::DynamicOffset],
            );
            render_pass.set_bind_group(1, uniform_bind_group, &[]);

            for (i, r) in i_ranges.iter() {
                if r.count == 0 {
                    continue;
                }

                let v = v_ranges.get(i).unwrap();
                let skin_ids = r.extra.as_slice();

                if settings.enable_skinning
                    && (v.jw_end - v.jw_start) > 0
                    && skin_ids.len() >= (r.count as usize)
                {
                    for i in 0..r.count {
                        if let Some(skin) = skin_ids.get(i as usize).and_then(|i| {
                            i.and_then(|i| {
                                skins.get(i as usize).and_then(|s| s.bind_group.as_ref())
                            })
                        }) {
                            // animated mesh
                            render_pass.set_pipeline(&self.anim_pipeline);
                            render_pass.set_vertex_buffer(
                                0,
                                vertex_buffer.slice(
                                    ((v.start as usize * std::mem::size_of::<Vertex3D>())
                                        as wgpu::BufferAddress)..,
                                ),
                            );
                            render_pass.set_vertex_buffer(
                                1,
                                jw_buffer.slice(
                                    ((v.jw_start as usize * std::mem::size_of::<JointData>())
                                        as wgpu::BufferAddress)..,
                                ),
                            );
                            render_pass.set_bind_group(2, skin, &[]);
                            render_pass
                                .draw(0..(v.end - v.start), (r.start + i)..(r.start + i + 1));
                        } else {
                            render_pass.set_pipeline(&self.pipeline);
                            render_pass.draw(v.start..v.end, r.start..r.end);
                        }
                    }

                    render_pass.set_pipeline(&self.pipeline);
                    render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                } else {
                    // static mesh
                    render_pass.draw(v.start..v.end, r.start..r.end);
                }
            }
        }

        encoder.copy_buffer_to_buffer(
            &self.filter_direction_x,
            0,
            &self.filter_uniform_direction_buffer,
            0,
            8,
        );

        for v in start..end {
            let mut filter_pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            filter_pass.set_pipeline(&self.filter_pipeline);
            filter_pass.set_bind_group(0, &self.filter_bind_groups1[v as usize], &[]);
            filter_pass.dispatch(
                (Self::WIDTH as f32 / 8.0).ceil() as u32,
                (Self::HEIGHT as f32 / 8.0).ceil() as u32,
                1,
            );
        }

        encoder.copy_buffer_to_buffer(
            &self.filter_direction_y,
            0,
            &self.filter_uniform_direction_buffer,
            0,
            8,
        );
        for v in start..end {
            let mut filter_pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            filter_pass.set_pipeline(&self.filter_pipeline);
            filter_pass.set_bind_group(0, &self.filter_bind_groups2[v as usize], &[]);
            filter_pass.dispatch(
                (Self::WIDTH as f32 / 8.0).ceil() as u32,
                (Self::HEIGHT as f32 / 8.0).ceil() as u32,
                1,
            );
        }
    }
}
