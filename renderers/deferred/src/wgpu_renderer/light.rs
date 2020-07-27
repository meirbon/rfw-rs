use super::{instance::InstanceList, mesh::DeferredMesh};
use crate::wgpu_renderer::mesh::DeferredAnimMesh;
use rtbvh::AABB;
use scene::{
    lights::*, AnimVertexData, BitVec, FrustrumG, FrustrumResult, ObjectRef, TrackedStorage,
    VertexData,
};
use shared::*;
use std::ops::Range;

pub struct DeferredLights {
    // point_lights: LightShadows<PointLight>,
    pub spot_lights: LightShadows<SpotLight>,
    pub area_lights: LightShadows<AreaLight>,
    pub directional_lights: LightShadows<DirectionalLight>,
}

impl DeferredLights {
    pub fn new(
        capacity: usize,
        device: &wgpu::Device,
        instance_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            // point_lights: LightShadows::new(device, instance_bind_group_layout, capacity, false),
            spot_lights: LightShadows::new(device, instance_bind_group_layout, capacity, false),
            area_lights: LightShadows::new(device, instance_bind_group_layout, capacity, false),
            directional_lights: LightShadows::new(
                device,
                instance_bind_group_layout,
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

    pub fn set_spot_lights(&mut self, changed: &BitVec, lights: &[SpotLight], scene_bounds: &AABB) {
        self.spot_lights.set(changed, lights, scene_bounds);
    }

    pub fn set_area_lights(&mut self, changed: &BitVec, lights: &[AreaLight], scene_bounds: &AABB) {
        self.area_lights.set(changed, lights, scene_bounds);
    }

    pub fn set_directional_lights(
        &mut self,
        changed: &BitVec,
        lights: &[DirectionalLight],
        scene_bounds: &AABB,
    ) {
        self.directional_lights.set(changed, lights, scene_bounds);
    }

    pub fn synchronize(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> bool {
        let mut changed = self.spot_lights.synchronize(device, queue);
        changed |= self.area_lights.synchronize(device, queue);
        changed |= self.directional_lights.synchronize(device, queue);

        changed
    }

    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        instances: &InstanceList,
        meshes: &TrackedStorage<DeferredMesh>,
        anim_meshes: &TrackedStorage<DeferredAnimMesh>,
    ) {
        self.area_lights
            .render(encoder, instances, meshes, anim_meshes);
        self.spot_lights
            .render(encoder, instances, meshes, anim_meshes);
        self.directional_lights
            .render(encoder, instances, meshes, anim_meshes);
    }
}

pub struct LightShadows<T: Sized + Light + Clone> {
    lights: Vec<T>,
    light_buffer: wgpu::Buffer,
    light_buffer_size: wgpu::BufferAddress,
    info: Vec<LightInfo>,
    shadow_maps: ShadowMapArray,
    changed: BitVec,
}

impl<T: Sized + Light + Clone> LightShadows<T> {
    pub fn new(
        device: &wgpu::Device,
        instance_bind_group_layout: &wgpu::BindGroupLayout,
        capacity: usize,
        linear: bool,
    ) -> Self {
        let light_buffer_size = (capacity * std::mem::size_of::<T>()) as wgpu::BufferAddress;
        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light-buffer"),
            size: light_buffer_size,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        });

        Self {
            lights: Vec::with_capacity(capacity),
            light_buffer,
            light_buffer_size,
            info: Vec::with_capacity(capacity),
            shadow_maps: ShadowMapArray::new(device, capacity, instance_bind_group_layout, linear),
            changed: BitVec::new(),
        }
    }

    pub fn push(&mut self, light: T, scene_bounds: &AABB) {
        self.info.push(light.get_light_info(scene_bounds));
        self.lights.push(light);
        self.changed.push(true);
    }

    pub fn set(&mut self, changed: &BitVec, lights: &[T], scene_bounds: &AABB) {
        self.lights = Vec::from(lights);
        self.info.resize(lights.len(), LightInfo::default());
        self.changed = changed.clone();

        for i in 0..lights.len() {
            if !changed.get(i).unwrap() {
                continue;
            }

            self.info[i] = self.lights[i].get_light_info(scene_bounds);
        }
    }

    pub fn synchronize(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> bool {
        if self.len() == 0 || !self.changed.any() {
            return false;
        }

        let mut changed = self.shadow_maps.resize(device, queue, self.lights.len());

        let light_buffer_size =
            (self.lights.len() * std::mem::size_of::<T>()) as wgpu::BufferAddress;
        if light_buffer_size > self.light_buffer_size {
            self.light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("light-buffer"),
                size: light_buffer_size,
                usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            });
            self.light_buffer_size = light_buffer_size;
            changed = true;
        }

        let staging_buffer = device.create_buffer_with_data(
            unsafe {
                std::slice::from_raw_parts(
                    self.lights.as_ptr() as *const u8,
                    light_buffer_size as usize,
                )
            },
            wgpu::BufferUsage::COPY_SRC,
        );

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("light-buffer-copy"),
        });

        encoder.copy_buffer_to_buffer(&staging_buffer, 0, &self.light_buffer, 0, light_buffer_size);
        queue.submit(&[encoder.finish()]);
        self.shadow_maps.update_infos(self.info.as_slice(), device);

        changed
    }

    pub fn len(&self) -> usize {
        self.lights.len()
    }

    pub fn uniform_binding(&self, binding: u32) -> wgpu::Binding {
        wgpu::Binding {
            binding,
            resource: wgpu::BindingResource::Buffer {
                buffer: &self.light_buffer,
                range: 0..self.light_buffer_size,
            },
        }
    }

    pub fn shadow_map_binding(&self, binding: u32) -> wgpu::Binding {
        wgpu::Binding {
            binding,
            resource: wgpu::BindingResource::TextureView(&self.shadow_maps.filter_view),
        }
    }

    pub fn infos_binding(&self, binding: u32) -> wgpu::Binding {
        wgpu::Binding {
            binding,
            resource: wgpu::BindingResource::Buffer {
                buffer: &self.shadow_maps.uniform_buffer,
                range: 0..(self.shadow_maps.len() * ShadowMapArray::UNIFORM_ELEMENT_SIZE)
                    as wgpu::BufferAddress,
            },
        }
    }

    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        instances: &InstanceList,
        meshes: &TrackedStorage<DeferredMesh>,
        anim_meshes: &TrackedStorage<DeferredAnimMesh>,
    ) {
        if instances.changed() {
            self.shadow_maps.render(
                0..self.lights.len() as u32,
                encoder,
                instances,
                meshes,
                anim_meshes,
            );
        } else {
            if !self.changed.any() {
                return;
            }

            for i in 0..self.lights.len() {
                if !self.changed.get(i).unwrap() {
                    continue;
                }

                let i = i as u32;
                self.shadow_maps
                    .render(i..(i + 1), encoder, instances, meshes, anim_meshes);
            }
        }

        self.changed.set_all(false);
    }
}

#[allow(dead_code)]
pub struct ShadowMapArray {
    pub map: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub views: Vec<wgpu::TextureView>,

    pub filter_map: wgpu::Texture,
    pub filter_view: wgpu::TextureView,
    pub filter_views: Vec<wgpu::TextureView>,

    depth_map: wgpu::Texture,
    depth_view: wgpu::TextureView,
    pub uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline_layout: wgpu::PipelineLayout,
    pipeline: wgpu::RenderPipeline,
    anim_pipeline: wgpu::RenderPipeline,

    filter_bind_group_layout: wgpu::BindGroupLayout,
    filter_bind_groups: Vec<wgpu::BindGroup>,

    filter_sampler_bind_group_layout: wgpu::BindGroupLayout,
    filter_sampler_bind_group: wgpu::BindGroup,
    filter_sampler: wgpu::Sampler,

    filter_pipeline_layout: wgpu::PipelineLayout,
    filter_pipeline: wgpu::RenderPipeline,
    light_infos: Vec<LightInfo>,
}

impl ShadowMapArray {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg32Float;
    pub const WIDTH: usize = 1536;
    pub const HEIGHT: usize = 1536;
    pub const UNIFORM_ELEMENT_SIZE: usize = std::mem::size_of::<LightInfo>();

    pub fn new(
        device: &wgpu::Device,
        count: usize,
        instance_bind_group_layout: &wgpu::BindGroupLayout,
        linear: bool,
    ) -> ShadowMapArray {
        let map = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_map"),
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT
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
            format: Self::FORMAT,
            array_layer_count: count as u32,
        });

        let view = map.create_view(&wgpu::TextureViewDescriptor {
            format: Self::FORMAT,
            dimension: wgpu::TextureViewDimension::D2Array,
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            array_layer_count: count as u32,
        });

        let views: Vec<wgpu::TextureView> = (0..count)
            .map(|i| {
                map.create_view(&wgpu::TextureViewDescriptor {
                    format: Self::FORMAT,
                    dimension: wgpu::TextureViewDimension::D2,
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: i as u32,
                    array_layer_count: 1,
                })
            })
            .collect();

        let filter_map = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_map"),
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT
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
            format: Self::FORMAT,
            array_layer_count: count as u32,
        });

        let filter_view = filter_map.create_view(&wgpu::TextureViewDescriptor {
            format: Self::FORMAT,
            dimension: wgpu::TextureViewDimension::D2Array,
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            array_layer_count: count as u32,
        });

        let filter_views = (0..count)
            .map(|i| {
                filter_map.create_view(&wgpu::TextureViewDescriptor {
                    format: Self::FORMAT,
                    dimension: wgpu::TextureViewDimension::D2,
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: i as u32,
                    array_layer_count: 1,
                })
            })
            .collect();

        let depth_map = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_map"),
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT
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
            array_layer_count: 1,
        });

        let depth_view = depth_map.create_default_view();

        let uniform_size = (count * Self::UNIFORM_ELEMENT_SIZE) as wgpu::BufferAddress;
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow-map-uniform-buffer"),
            size: uniform_size,
            usage: wgpu::BufferUsage::UNIFORM
                | wgpu::BufferUsage::COPY_SRC
                | wgpu::BufferUsage::COPY_DST
                | wgpu::BufferUsage::MAP_WRITE,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow-map-layout"),
            bindings: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::UniformBuffer { dynamic: true },
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            label: Some("shadow-map-uniform-bind-group"),
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &uniform_buffer,
                    range: 0..uniform_size,
                },
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&bind_group_layout, instance_bind_group_layout],
        });

        let vert_shader = include_bytes!("../../shaders/shadow_single.vert.spv",);
        let regular_frag_shader = include_bytes!("../../shaders/shadow_single.frag.spv");
        let linear_frag_shader = include_bytes!("../../shaders/shadow_single_linear.frag.spv");

        let vert_module = device.create_shader_module(vert_shader.to_quad_bytes());
        let frag_module = device.create_shader_module(match linear {
            true => linear_frag_shader.to_quad_bytes(),
            false => regular_frag_shader.to_quad_bytes(),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &pipeline_layout,
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &vert_module,
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &frag_module,
            }),
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            }),
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: Self::FORMAT,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
                format: Self::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil_front: wgpu::StencilStateFaceDescriptor::IGNORE,
                stencil_back: wgpu::StencilStateFaceDescriptor::IGNORE,
                stencil_read_mask: 0,
                stencil_write_mask: 0,
            }),
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint32,
                vertex_buffers: &[wgpu::VertexBufferDescriptor {
                    stride: std::mem::size_of::<VertexData>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &[wgpu::VertexAttributeDescriptor {
                        offset: 0,
                        format: wgpu::VertexFormat::Float4,
                        shader_location: 0,
                    }],
                }],
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        let anim_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &pipeline_layout,
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &vert_module,
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &frag_module,
            }),
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            }),
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: Self::FORMAT,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
                format: Self::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil_front: wgpu::StencilStateFaceDescriptor::IGNORE,
                stencil_back: wgpu::StencilStateFaceDescriptor::IGNORE,
                stencil_read_mask: 0,
                stencil_write_mask: 0,
            }),
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint32,
                vertex_buffers: &[wgpu::VertexBufferDescriptor {
                    stride: std::mem::size_of::<AnimVertexData>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &[wgpu::VertexAttributeDescriptor {
                        offset: 0,
                        format: wgpu::VertexFormat::Float4,
                        shader_location: 0,
                    }],
                }],
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        let filter_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("filter-bind-group-layout"),
                bindings: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    ty: wgpu::BindingType::SampledTexture {
                        component_type: wgpu::TextureComponentType::Float,
                        multisampled: false,
                        dimension: wgpu::TextureViewDimension::D2,
                    },
                    visibility: wgpu::ShaderStage::FRAGMENT,
                }],
            });
        let filter_bind_groups: Vec<wgpu::BindGroup> = views
            .iter()
            .map(|v| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("filter-bind-group"),
                    layout: &filter_bind_group_layout,
                    bindings: &[wgpu::Binding {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(v),
                    }],
                })
            })
            .collect();

        let filter_sampler_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("filter-sampler-bind-group-layout"),
                bindings: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    ty: wgpu::BindingType::Sampler { comparison: false },
                    visibility: wgpu::ShaderStage::FRAGMENT,
                }],
            });

        let filter_sampler = Self::create_sampler(device);
        let filter_sampler_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("filter-sampler-bind-group"),
            layout: &filter_sampler_bind_group_layout,
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&filter_sampler),
            }],
        });

        let filter_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[&filter_sampler_bind_group_layout, &filter_bind_group_layout],
            });

        let vert_shader = include_bytes!("../../shaders/quad.vert.spv");
        let frag_shader = include_bytes!("../../shaders/shadow_filter.frag.spv",);
        let vert_module = device.create_shader_module(vert_shader.to_quad_bytes());
        let frag_module = device.create_shader_module(frag_shader.to_quad_bytes());

        let filter_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &filter_pipeline_layout,
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &vert_module,
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &frag_module,
            }),
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            }),
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: Self::FORMAT,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: None,
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint32,
                vertex_buffers: &[],
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
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
            anim_pipeline,
            filter_bind_group_layout,
            filter_bind_groups,
            filter_pipeline_layout,
            filter_pipeline,
            filter_sampler_bind_group_layout,
            filter_sampler_bind_group,
            filter_sampler,
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
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT
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
            format: Self::FORMAT,
            array_layer_count: size as u32,
        });

        let view = map.create_view(&wgpu::TextureViewDescriptor {
            format: Self::FORMAT,
            dimension: wgpu::TextureViewDimension::D2Array,
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            array_layer_count: size as u32,
        });

        let views: Vec<wgpu::TextureView> = (0..size)
            .map(|i| {
                map.create_view(&wgpu::TextureViewDescriptor {
                    format: Self::FORMAT,
                    dimension: wgpu::TextureViewDimension::D2,
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: i as u32,
                    array_layer_count: 1,
                })
            })
            .collect();

        // Create new texture
        let new_depth_map = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_map"),
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT
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
            array_layer_count: size as u32,
        });

        let new_depth_view = new_depth_map.create_view(&wgpu::TextureViewDescriptor {
            format: Self::DEPTH_FORMAT,
            dimension: wgpu::TextureViewDimension::D2Array,
            aspect: wgpu::TextureAspect::DepthOnly,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            array_layer_count: size as u32,
        });

        let filter_map = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow_map"),
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT
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
            format: Self::FORMAT,
            array_layer_count: size as u32,
        });

        let filter_view = filter_map.create_view(&wgpu::TextureViewDescriptor {
            format: Self::FORMAT,
            dimension: wgpu::TextureViewDimension::D2Array,
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            array_layer_count: size as u32,
        });

        let filter_views: Vec<wgpu::TextureView> = (0..size)
            .map(|i| {
                filter_map.create_view(&wgpu::TextureViewDescriptor {
                    format: Self::FORMAT,
                    dimension: wgpu::TextureViewDimension::D2,
                    aspect: wgpu::TextureAspect::All,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: i as u32,
                    array_layer_count: 1,
                })
            })
            .collect();

        let filter_bind_groups: Vec<wgpu::BindGroup> = views
            .iter()
            .map(|v| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("filter-bind-group"),
                    layout: &self.filter_bind_group_layout,
                    bindings: &[wgpu::Binding {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(v),
                    }],
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
                    array_layer: i as u32,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                    texture: &self.map,
                },
                wgpu::TextureCopyView {
                    array_layer: i as u32,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
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
                    array_layer: i as u32,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                    texture: &self.filter_map,
                },
                wgpu::TextureCopyView {
                    array_layer: i as u32,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                    texture: &filter_map,
                },
                wgpu::Extent3d {
                    width: Self::WIDTH as u32,
                    height: Self::HEIGHT as u32,
                    depth: 1,
                },
            );
        }

        let new_size = (size * Self::UNIFORM_ELEMENT_SIZE) as wgpu::BufferAddress;

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow-map-uniform-buffer"),
            size: new_size,
            usage: wgpu::BufferUsage::UNIFORM
                | wgpu::BufferUsage::MAP_WRITE
                | wgpu::BufferUsage::COPY_SRC
                | wgpu::BufferUsage::COPY_DST,
        });

        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.bind_group_layout,
            label: Some("shadow-map-uniform-bind-group"),
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &uniform_buffer,
                    range: 0..new_size,
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

        queue.submit(&[encoder.finish()]);

        self.uniform_buffer = uniform_buffer;

        self.view = view;
        self.views = views;
        self.map = map;

        self.depth_view = new_depth_view;
        self.depth_map = new_depth_map;

        self.filter_view = filter_view;
        self.filter_views = filter_views;
        self.filter_bind_groups = filter_bind_groups;
        self.filter_map = filter_map;

        self.light_infos.resize(size, LightInfo::default());
        true
    }

    pub fn as_binding(&self, binding: usize) -> wgpu::Binding {
        wgpu::Binding {
            resource: wgpu::BindingResource::TextureView(&self.filter_view),
            binding: binding as u32,
        }
    }

    pub fn create_sampler(device: &wgpu::Device) -> wgpu::Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            compare: wgpu::CompareFunction::Never,
            lod_max_clamp: 1.0,
            lod_min_clamp: 0.0,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
        })
    }

    pub fn update_infos(&mut self, infos: &[LightInfo], device: &wgpu::Device) {
        self.light_infos = Vec::from(infos);

        let copy_size = infos.len() * Self::UNIFORM_ELEMENT_SIZE;
        let mapping = self
            .uniform_buffer
            .map_write(0, copy_size as wgpu::BufferAddress);

        // Copy matrices to staging buffer
        device.poll(wgpu::Maintain::Wait);

        let mapping = futures::executor::block_on(mapping);
        if let Ok(mut mapping) = mapping {
            unsafe {
                let ptr = mapping.as_slice().as_mut_ptr();
                ptr.copy_from(infos.as_ptr() as *const u8, copy_size);
            }
        }
    }

    pub fn render(
        &self,
        range: Range<u32>,
        encoder: &mut wgpu::CommandEncoder,
        instances: &super::instance::InstanceList,
        meshes: &TrackedStorage<DeferredMesh>,
        anim_meshes: &TrackedStorage<DeferredAnimMesh>,
    ) {
        // TODO: Use anim meshes
        assert!(range.end as usize <= self.views.len());
        for v in range {
            {
                let frustrum = FrustrumG::from_matrix(self.light_infos[v as usize].pm);
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                        attachment: &self.views[v as usize],
                        resolve_target: None,
                        load_op: wgpu::LoadOp::Clear,
                        store_op: wgpu::StoreOp::Store,
                        clear_color: wgpu::Color::BLACK,
                    }],
                    depth_stencil_attachment: Some(
                        wgpu::RenderPassDepthStencilAttachmentDescriptor {
                            attachment: &self.depth_view,
                            clear_depth: 1.0,
                            clear_stencil: 0,
                            depth_load_op: wgpu::LoadOp::Clear,
                            depth_store_op: wgpu::StoreOp::Store,
                            stencil_load_op: wgpu::LoadOp::Clear,
                            stencil_store_op: wgpu::StoreOp::Store,
                        },
                    ),
                });

                render_pass.set_bind_group(
                    0,
                    &self.bind_group,
                    &[(v as usize * Self::UNIFORM_ELEMENT_SIZE) as wgpu::DynamicOffset],
                );

                for i in 0..instances.len() {
                    let instance = &instances.instances[i];
                    let device_instance = &instances.device_instances[i];
                    let bounds = &instances.bounds[i];

                    if frustrum.aabb_in_frustrum(&bounds.root_bounds) == FrustrumResult::Outside {
                        continue;
                    }

                    match instance.object_id {
                        ObjectRef::None => panic!("Invalid"),
                        ObjectRef::Static(mesh_id) => {
                            let mesh = &meshes[mesh_id as usize];

                            render_pass.set_pipeline(&self.pipeline);
                            if let Some(buffer) = mesh.buffer.as_ref() {
                                render_pass.set_vertex_buffer(0, buffer, 0, mesh.buffer_size);
                                render_pass.set_bind_group(1, &device_instance.bind_group, &[]);

                                for j in 0..mesh.sub_meshes.len() {
                                    if let Some(bounds) = bounds.mesh_bounds.get(i) {
                                        if frustrum.aabb_in_frustrum(bounds)
                                            == FrustrumResult::Outside
                                        {
                                            continue;
                                        }
                                    }

                                    let sub_mesh = &mesh.sub_meshes[j];
                                    render_pass.draw(sub_mesh.first..sub_mesh.last, 0..1);
                                }
                            }
                        }
                        ObjectRef::Animated(mesh_id) => {
                            let mesh = &anim_meshes[mesh_id as usize];
                            render_pass.set_pipeline(&self.pipeline);
                            if let Some(buffer) = mesh.buffer.as_ref() {
                                render_pass.set_vertex_buffer(0, buffer, 0, mesh.buffer_size);
                                render_pass.set_bind_group(1, &device_instance.bind_group, &[]);

                                for j in 0..mesh.sub_meshes.len() {
                                    if let Some(bounds) = bounds.mesh_bounds.get(i) {
                                        if frustrum.aabb_in_frustrum(bounds)
                                            == FrustrumResult::Outside
                                        {
                                            continue;
                                        }
                                    }

                                    let sub_mesh = &mesh.sub_meshes[j];
                                    render_pass.draw(sub_mesh.first..sub_mesh.last, 0..1);
                                }
                            }
                        }
                    };
                }
            }

            {
                // Filter shadow maps for VSM
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                        attachment: &self.filter_views[v as usize],
                        clear_color: wgpu::Color::BLACK,
                        load_op: wgpu::LoadOp::Clear,
                        store_op: wgpu::StoreOp::Store,
                        resolve_target: None,
                    }],
                    depth_stencil_attachment: None,
                });

                render_pass.set_pipeline(&self.filter_pipeline);
                render_pass.set_bind_group(0, &self.filter_sampler_bind_group, &[]);
                render_pass.set_bind_group(1, &self.filter_bind_groups[v as usize], &[]);
                render_pass.draw(0..6, 0..1);
            }
        }
    }
}
