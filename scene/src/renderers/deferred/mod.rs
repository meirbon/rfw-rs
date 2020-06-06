// use crate::lights::*;
// use crate::triangle_scene::*;
// use crate::{objects::*, Camera, FrustrumG, FrustrumResult, MaterialList};
// use bvh::{Bounds, AABB};
// use fb_template::shader::ShaderKind;
// use futures::executor::block_on;
// use glam::*;
// use std::sync::{Arc, Mutex};

// pub struct DeferredRenderer {
//     lights: Arc<Mutex<SceneLights>>,
//     objects: Arc<Mutex<InstancedObjects>>,
//     materials: Arc<Mutex<MaterialList>>,

//     pub uniform_buffer: wgpu::Buffer,
//     pub staging_buffer: wgpu::Buffer,

//     pub render_pipeline: wgpu::RenderPipeline,
//     pub render_pipeline_layout: wgpu::PipelineLayout,
//     pub uniform_bind_group: wgpu::BindGroup,

//     pub blit_bind_group_layout: wgpu::BindGroupLayout,
//     pub blit_bind_group0: wgpu::BindGroup,
//     pub blit_bind_group1: wgpu::BindGroup,
//     pub blit_sampler: wgpu::Sampler,

//     pub blit_pipeline: wgpu::RenderPipeline,
//     pub blit_pipeline_layout: wgpu::PipelineLayout,

//     pub blit_albedo_pipeline: wgpu::RenderPipeline,
//     pub blit_normal_pipeline: wgpu::RenderPipeline,
//     pub blit_world_pos_pipeline: wgpu::RenderPipeline,
//     pub blit_depth_pipeline: wgpu::RenderPipeline,
//     pub blit_radiance_pipeline: wgpu::RenderPipeline,

//     pub albedo_texture: wgpu::Texture,
//     pub albedo_view: wgpu::TextureView,

//     pub normal_texture: wgpu::Texture,
//     pub normal_view: wgpu::TextureView,

//     pub world_pos_texture: wgpu::Texture,
//     pub world_pos_view: wgpu::TextureView,

//     pub radiance_bind_group_layout0: wgpu::BindGroupLayout,
//     pub radiance_bind_group_layout1: wgpu::BindGroupLayout,

//     pub radiance_bind_group0: wgpu::BindGroup,
//     pub radiance_bind_group1: wgpu::BindGroup,

//     pub radiance_texture: wgpu::Texture,
//     pub radiance_view: wgpu::TextureView,

//     pub screen_space_texture: wgpu::Texture,
//     pub screen_space_view: wgpu::TextureView,

//     pub output_format: wgpu::TextureFormat,
//     pub intermediate_texture0: wgpu::Texture,
//     pub intermediate_view0: wgpu::TextureView,

//     pub intermediate_texture1: wgpu::Texture,
//     pub intermediate_view1: wgpu::TextureView,

//     pub intermediate_texture0: wgpu::Texture,
//     pub intermediate_view0: wgpu::TextureView,

//     pub ssao_output: wgpu::Texture,
//     pub ssao_output_view: wgpu::TextureView,

//     pub ssao_filtered_output: wgpu::Texture,
//     pub ssao_filtered_output_view: wgpu::TextureView,

//     pub radiance_pipeline: wgpu::RenderPipeline,
//     pub radiance_pipeline_layout: wgpu::PipelineLayout,

//     pub instance_bind_groups: Vec<wgpu::BindGroup>,
//     pub instance_buffers: Vec<InstanceMatrices>,
//     pub vertex_buffers: Vec<VertexBuffer>,

//     pub material_buffer: (wgpu::BufferAddress, wgpu::Buffer),
//     pub material_textures: Vec<wgpu::Texture>,
//     pub material_texture_views: Vec<wgpu::TextureView>,
//     pub material_texture_sampler: wgpu::Sampler,
//     pub material_bind_groups: Vec<wgpu::BindGroup>,

//     pub uniform_bind_group_layout: wgpu::BindGroupLayout,
//     pub texture_bind_group_layout: wgpu::BindGroupLayout,
//     pub triangle_bind_group_layout: wgpu::BindGroupLayout,

//     pub shadow_map_sampler: wgpu::Sampler,

//     // pub point_lights_buffer: wgpu::Buffer,
//     // pub point_lights_matrices: Vec<[Mat4; 6]>,
//     // pub point_lights_shadow_maps: ShadowCubeMapArray,
//     pub area_lights_buffer: wgpu::Buffer,
//     pub area_lights_matrices: Vec<LightInfo>,
//     pub area_lights_shadow_maps: ShadowMapArray,

//     pub spot_lights_buffer: wgpu::Buffer,
//     pub spot_lights_matrices: Vec<LightInfo>,
//     pub spot_lights_shadow_maps: ShadowMapArray,

//     pub directional_lights_buffer: wgpu::Buffer,
//     pub directional_lights_matrices: Vec<LightInfo>,
//     pub directional_lights_shadow_maps: ShadowMapArray,

//     pub fxaa_render_pipeline: wgpu::RenderPipeline,

//     pub ssao_hemisphere_sample_buffer: wgpu::Buffer,
//     pub ssao_noise_texture: wgpu::Texture,
//     pub ssao_noise_texture_view: wgpu::TextureView,
//     pub ssao_render_pipeline: wgpu::RenderPipeline,
//     pub ssao_filter_render_pipeline: wgpu::RenderPipeline,
//     pub ssao_filter_pipeline_layout: wgpu::PipelineLayout,
//     pub ssao_filter_bind_group_layout: wgpu::BindGroupLayout,
//     pub ssao_filter_bind_group: wgpu::BindGroup,

//     pub light_counts: [u32; 4],
// }

// impl DeferredRenderer {
//     const STORAGE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
//     const SSAO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
//     const SSAO_KERNEL_SIZE: usize = 64;
//     const SSAO_NOISE_SIZE: usize = 4;
//     const MAX_UPDATES_PER_LIGHT: usize = 10;

//     pub fn new(
//         scene: &TriangleScene,
//         device: &wgpu::Device,
//         queue: &wgpu::Queue,
//         output_format: wgpu::TextureFormat,
//         depth_format: wgpu::TextureFormat,
//         output_size: (u32, u32),
//     ) -> Self {
//         use wgpu::*;

//         let uniform_buffer = device.create_buffer(&BufferDescriptor {
//             label: Some("triangle-uniform-buffer"),
//             size: std::mem::size_of::<Mat4>() as BufferAddress
//                 + std::mem::size_of::<Mat4>() as BufferAddress
//                 + std::mem::size_of::<[u32; 4]>() as BufferAddress
//                 + std::mem::size_of::<Vec4>() as BufferAddress,
//             usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
//         });

//         let mut staging_buffer = device.create_buffer_mapped(&BufferDescriptor {
//             label: Some("staging-buffer"),
//             size: std::mem::size_of::<Mat4>() as BufferAddress
//                 + std::mem::size_of::<Mat4>() as BufferAddress
//                 + std::mem::size_of::<[u32; 4]>() as BufferAddress
//                 + std::mem::size_of::<Vec4>() as BufferAddress,
//             usage: BufferUsage::COPY_SRC | BufferUsage::MAP_WRITE,
//         });

//         unsafe {
//             let matrix = Mat4::identity();
//             std::ptr::copy(
//                 matrix.as_ref().as_ptr() as *const u8,
//                 staging_buffer.data().as_mut_ptr(),
//                 std::mem::size_of::<Mat4>(),
//             );
//             let matrix = Mat4::identity();
//             std::ptr::copy(
//                 matrix.as_ref().as_ptr() as *const u8,
//                 staging_buffer.data().as_mut_ptr().add(64),
//                 std::mem::size_of::<Mat4>(),
//             );
//             let counts = [0 as u32; 4];
//             std::ptr::copy(
//                 counts.as_ptr() as *const u8,
//                 staging_buffer.data().as_mut_ptr().add(128),
//                 std::mem::size_of::<u32>() * 4,
//             );
//             let pos = Vec4::zero();
//             std::ptr::copy(
//                 pos.as_ref().as_ptr() as *const u8,
//                 staging_buffer.data().as_mut_ptr().add(144),
//                 std::mem::size_of::<Vec4>(),
//             );
//         }
//         let staging_buffer = staging_buffer.finish();

//         let uniform_bind_group_layout = Self::create_uniform_bind_group_layout(device);
//         let triangle_bind_group_layout = Self::create_instance_bind_group_layout(device);
//         let texture_bind_group_layout = Self::create_texture_bind_group_layout(device);
//         let (render_pipeline_layout, render_pipeline) = Self::create_render_pipeline(
//             device,
//             depth_format,create_uniform_bind_group_layout
//             &uniform_bind_group_layout,
//             &triangle_bind_group_layout,
//             &texture_bind_group_layout,
//         );

//         // let material_buffer = ;
//         let material_texture_sampler = Self::create_texture_sampler(device);

//         let l = scene.get_lights();
//         let lights = l.lock().unwrap();

//         let (
//             _point_lights_buffer,
//             area_lights_buffer,
//             spot_lights_buffer,
//             directional_lights_buffer,
//         ) = Self::create_light_buffers(device, queue, &lights);

//         let bounds = scene.objects_lock().unwrap().bounds();
//         // let point_lights_matrices: Vec<[Mat4; 6]> = scene
//         //     .point_lights()
//         //     .iter()
//         //     .map(|l| l.get_matrices())
//         //     .collect();
//         // let point_lights_shadow_maps = ShadowCubeMapArray::new(
//         //     device,
//         //     scene.point_lights().len().max(1),
//         //     &triangle_bind_group_layout,
//         // );

//         let area_lights_matrices: Vec<LightInfo> = lights
//             .area_lights
//             .iter()
//             .map(|l| l.get_light_info())
//             .collect();
//         let area_lights_shadow_maps = ShadowMapArray::new(
//             device,
//             lights.area_lights.len().max(1),
//             &triangle_bind_group_layout,
//             false,
//         );

//         let spot_lights_matrices: Vec<LightInfo> = lights
//             .spot_lights
//             .iter()
//             .map(|l| l.get_light_info())
//             .collect();
//         let spot_lights_shadow_maps = ShadowMapArray::new(
//             device,
//             lights.spot_lights.len().max(1),
//             &triangle_bind_group_layout,
//             false,
//         );

//         let directional_lights_matrices: Vec<LightInfo> = lights
//             .directional_lights
//             .iter()
//             .map(|l| l.get_light_info())
//             .collect();
//         let directional_lights_shadow_maps = ShadowMapArray::new(
//             device,
//             lights.directional_lights.len().max(1),
//             &triangle_bind_group_layout,
//             true,
//         );

//         let shadow_map_sampler = ShadowMapArray::create_sampler(device);

//         let uniform_bind_group = device.create_bind_group(&BindGroupDescriptor {
//             layout: &uniform_bind_group_layout,
//             bindings: &[
//                 Binding {
//                     binding: 0,
//                     resource: BindingResource::Buffer {
//                         buffer: &uniform_buffer,
//                         range: 0..160,
//                     },
//                 },
//                 Binding {
//                     binding: 1,
//                     resource: BindingResource::Buffer {
//                         buffer: &material_buffer.1,
//                         range: 0..(material_buffer.0),
//                     },
//                 },
//                 Binding {
//                     binding: 2,
//                     resource: BindingResource::Sampler(&material_texture_sampler),
//                 },
//             ],
//             label: Some("mesh-bind-group-descriptor"),
//         });

//         let light_counts = [
//             lights.point_lights.len() as u32,
//             lights.area_lights.len() as u32,
//             lights.spot_lights.len() as u32,
//             lights.directional_lights.len() as u32,
//         ];

//         let descriptor = wgpu::TextureDescriptor {
//             label: None,
//             size: Extent3d {
//                 width: output_size.0,
//                 height: output_size.1,
//                 depth: 1,
//             },
//             mip_level_count: 1,
//             sample_count: 1,
//             dimension: wgpu::TextureDimension::D2,
//             format: Self::STORAGE_FORMAT,
//             usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
//         };

//         let albedo_texture = device.create_texture(&descriptor);
//         let albedo_view = albedo_texture.create_default_view();

//         let normal_texture = device.create_texture(&descriptor);
//         let normal_view = normal_texture.create_default_view();

//         let world_pos_texture = device.create_texture(&descriptor);
//         let world_pos_view = world_pos_texture.create_default_view();

//         let radiance_texture = device.create_texture(&descriptor);
//         let radiance_view = radiance_texture.create_default_view();

//         let screen_space_texture = device.create_texture(&descriptor);
//         let screen_space_view = screen_space_texture.create_default_view();

//         let intermediate_texture0 = device.create_texture(&wgpu::TextureDescriptor {
//             label: None,
//             size: Extent3d {
//                 width: output_size.0,
//                 height: output_size.1,
//                 depth: 1,
//             },
//             mip_level_count: 1,
//             sample_count: 1,
//             dimension: wgpu::TextureDimension::D2,
//             format: output_format,
//             usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
//         });
//         let intermediate_view0 = intermediate_texture0.create_default_view();

//         let intermediate_texture1 = device.create_texture(&wgpu::TextureDescriptor {
//             label: None,
//             size: Extent3d {
//                 width: output_size.0,
//                 height: output_size.1,
//                 depth: 1,
//             },
//             mip_level_count: 1,
//             sample_count: 1,
//             dimension: wgpu::TextureDimension::D2,
//             format: output_format,
//             usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
//         });
//         let intermediate_view1 = intermediate_texture1.create_default_view();

//         let mut compiler = fb_template::shader::CompilerBuilder::new().build();

//         let blit_bind_group_layout =
//             device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
//                 label: Some("blit-bind-group-layout"),
//                 bindings: &[
//                     wgpu::BindGroupLayoutEntry {
//                         binding: 0,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::Sampler { comparison: false },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         // Albedo
//                         binding: 1,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::SampledTexture {
//                             dimension: wgpu::TextureViewDimension::D2,
//                             component_type: wgpu::TextureComponentType::Float,
//                             multisampled: false,
//                         },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         // Normal
//                         binding: 2,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::SampledTexture {
//                             dimension: wgpu::TextureViewDimension::D2,
//                             component_type: wgpu::TextureComponentType::Float,
//                             multisampled: false,
//                         },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         // World pos
//                         binding: 3,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::SampledTexture {
//                             dimension: wgpu::TextureViewDimension::D2,
//                             component_type: wgpu::TextureComponentType::Float,
//                             multisampled: false,
//                         },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         // Radiance
//                         binding: 4,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::SampledTexture {
//                             dimension: wgpu::TextureViewDimension::D2,
//                             component_type: wgpu::TextureComponentType::Float,
//                             multisampled: false,
//                         },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         // Screen space view
//                         binding: 5,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::SampledTexture {
//                             dimension: wgpu::TextureViewDimension::D2,
//                             component_type: wgpu::TextureComponentType::Float,
//                             multisampled: false,
//                         },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         // Output view
//                         binding: 6,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::SampledTexture {
//                             dimension: wgpu::TextureViewDimension::D2,
//                             component_type: wgpu::TextureComponentType::Float,
//                             multisampled: false,
//                         },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         // Camera
//                         binding: 7,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::UniformBuffer { dynamic: false },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         // SSAO Samples
//                         binding: 8,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::UniformBuffer { dynamic: false },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         // SSAO Noise
//                         binding: 9,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::SampledTexture {
//                             dimension: wgpu::TextureViewDimension::D2,
//                             component_type: wgpu::TextureComponentType::Float,
//                             multisampled: false,
//                         },
//                     },
//                 ],
//             });

//         let ssao_hemisphere_sample_buffer = device.create_buffer(&wgpu::BufferDescriptor {
//             size: (Self::SSAO_KERNEL_SIZE * std::mem::size_of::<Vec3>()) as BufferAddress,
//             label: Some("ssao_hemisphere_sample_buffer"),
//             usage: wgpu::BufferUsage::COPY_DST | wgpu::BufferUsage::UNIFORM,
//         });

//         let ssao_noise_texture = device.create_texture(&wgpu::TextureDescriptor {
//             label: Some("ssao_noise_texture"),
//             size: wgpu::Extent3d {
//                 width: Self::SSAO_NOISE_SIZE as u32,
//                 height: Self::SSAO_NOISE_SIZE as u32,
//                 depth: 1,
//             },
//             mip_level_count: 1,
//             sample_count: 1,
//             dimension: wgpu::TextureDimension::D2,
//             format: wgpu::TextureFormat::Rgba32Float,
//             usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
//         });
//         let ssao_noise_texture_view = ssao_noise_texture.create_default_view();

//         let lerp = |a: f32, b: f32, f: f32| -> f32 { a + f * (b - a) };
//         let ssao_samples: Vec<Vec3> = (0..Self::SSAO_KERNEL_SIZE)
//             .into_iter()
//             .map(|i| {
//                 use rand::{thread_rng, Rng};
//                 let s0: f32 = thread_rng().gen_range(0.0, 1.0) * 2.0 - 1.0;
//                 let s1: f32 = thread_rng().gen_range(0.0, 1.0) * 2.0 - 1.0;
//                 let s2: f32 = thread_rng().gen_range(0.0, 1.0);

//                 let s = Vec3::new(s0, s1, s2).normalize() * thread_rng().gen_range(0.0, 1.0);
//                 let scale = i as f32 / Self::SSAO_KERNEL_SIZE as f32;
//                 let scale = lerp(0.1, 1.0, scale * scale);
//                 s * scale
//             })
//             .collect();
//         let ssao_noise: Vec<Vec3> = (0..(Self::SSAO_NOISE_SIZE * Self::SSAO_NOISE_SIZE))
//             .into_iter()
//             .map(|_| {
//                 use rand::{thread_rng, Rng};
//                 Vec3::new(
//                     thread_rng().gen_range(0.0, 1.0) * 2.0 - 1.0,
//                     thread_rng().gen_range(0.0, 1.0) * 2.0 - 1.0,
//                     0.0,
//                 )
//             })
//             .collect();

//         let mut samples_buffer = device.create_buffer_mapped(&wgpu::BufferDescriptor {
//             label: Some("ssao-staging-buffer"),
//             usage: wgpu::BufferUsage::COPY_SRC,
//             size: (Self::SSAO_KERNEL_SIZE * std::mem::size_of::<Vec3>()) as BufferAddress,
//         });
//         let mut noise_buffer = device.create_buffer_mapped(&wgpu::BufferDescriptor {
//             label: Some("ssao-staging-buffer"),
//             usage: wgpu::BufferUsage::COPY_SRC,
//             size: ((Self::SSAO_NOISE_SIZE * Self::SSAO_NOISE_SIZE) * std::mem::size_of::<Vec3>())
//                 as BufferAddress,
//         });

//         let samples_data = samples_buffer.data();
//         let noise_data = noise_buffer.data();
//         unsafe {
//             std::ptr::copy(
//                 ssao_samples.as_ptr() as *const u8,
//                 samples_data.as_mut_ptr(),
//                 Self::SSAO_KERNEL_SIZE * std::mem::size_of::<Vec3>(),
//             );

//             std::ptr::copy(
//                 ssao_noise.as_ptr() as *const u8,
//                 noise_data.as_mut_ptr(),
//                 (Self::SSAO_NOISE_SIZE * Self::SSAO_NOISE_SIZE) * std::mem::size_of::<Vec3>(),
//             );
//         }
//         let samples_buffer = samples_buffer.finish();
//         let noise_buffer = noise_buffer.finish();

//         let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
//             label: Some("ssao-staging-cmd-buffer"),
//         });
//         encoder.copy_buffer_to_buffer(
//             &samples_buffer,
//             0,
//             &ssao_hemisphere_sample_buffer,
//             0,
//             (Self::SSAO_KERNEL_SIZE * std::mem::size_of::<Vec3>()) as BufferAddress,
//         );
//         encoder.copy_buffer_to_texture(
//             wgpu::BufferCopyView {
//                 buffer: &noise_buffer,
//                 rows_per_image: Self::SSAO_NOISE_SIZE as u32,
//                 bytes_per_row: (Self::SSAO_NOISE_SIZE * std::mem::size_of::<Vec3>()) as u32,
//                 offset: 0,
//             },
//             wgpu::TextureCopyView {
//                 texture: &ssao_noise_texture,
//                 mip_level: 0,
//                 array_layer: 0,
//                 origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
//             },
//             wgpu::Extent3d {
//                 width: 4,
//                 height: 4,
//                 depth: 1,
//             },
//         );

//         queue.submit(Some(encoder.finish()));

//         let vert_spirv = compiler
//             .compile_from_file("shaders/quad.vert", ShaderKind::Vertex)
//             .unwrap();
//         let frag_spirv = compiler
//             .compile_from_file("shaders/deferred_blit.frag", ShaderKind::Fragment)
//             .unwrap();

//         let vert_module = device.create_shader_module(vert_spirv.as_slice());
//         let frag_module = device.create_shader_module(frag_spirv.as_slice());

//         let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
//             bind_group_layouts: &[&blit_bind_group_layout],
//         });

//         let blit_pipeline = Self::create_blit_pipeline(
//             device,
//             output_format,
//             &blit_pipeline_layout,
//             &vert_module,
//             &frag_module,
//         );

//         let blit_albedo_pipeline = {
//             let vert_spirv = compiler
//                 .compile_from_file("shaders/quad.vert", ShaderKind::Vertex)
//                 .unwrap();
//             let frag_spirv = compiler
//                 .compile_from_file("shaders/deferred_albedo.frag", ShaderKind::Fragment)
//                 .unwrap();

//             let vert_module = device.create_shader_module(vert_spirv.as_slice());
//             let frag_module = device.create_shader_module(frag_spirv.as_slice());
//             Self::create_blit_pipeline(
//                 device,
//                 output_format,
//                 &blit_pipeline_layout,
//                 &vert_module,
//                 &frag_module,
//             )
//         };

//         let blit_normal_pipeline = {
//             let vert_spirv = compiler
//                 .compile_from_file("shaders/quad.vert", ShaderKind::Vertex)
//                 .unwrap();
//             let frag_spirv = compiler
//                 .compile_from_file("shaders/deferred_normal.frag", ShaderKind::Fragment)
//                 .unwrap();

//             let vert_module = device.create_shader_module(vert_spirv.as_slice());
//             let frag_module = device.create_shader_module(frag_spirv.as_slice());
//             Self::create_blit_pipeline(
//                 device,
//                 output_format,
//                 &blit_pipeline_layout,
//                 &vert_module,
//                 &frag_module,
//             )
//         };

//         let blit_world_pos_pipeline = {
//             let vert_spirv = compiler
//                 .compile_from_file("shaders/quad.vert", ShaderKind::Vertex)
//                 .unwrap();
//             let frag_spirv = compiler
//                 .compile_from_file("shaders/deferred_world_pos.frag", ShaderKind::Fragment)
//                 .unwrap();

//             let vert_module = device.create_shader_module(vert_spirv.as_slice());
//             let frag_module = device.create_shader_module(frag_spirv.as_slice());
//             Self::create_blit_pipeline(
//                 device,
//                 output_format,
//                 &blit_pipeline_layout,
//                 &vert_module,
//                 &frag_module,
//             )
//         };

//         let blit_depth_pipeline = {
//             let vert_spirv = compiler
//                 .compile_from_file("shaders/quad.vert", ShaderKind::Vertex)
//                 .unwrap();
//             let frag_spirv = compiler
//                 .compile_from_file("shaders/deferred_depth.frag", ShaderKind::Fragment)
//                 .unwrap();

//             let vert_module = device.create_shader_module(vert_spirv.as_slice());
//             let frag_module = device.create_shader_module(frag_spirv.as_slice());
//             Self::create_blit_pipeline(
//                 device,
//                 output_format,
//                 &blit_pipeline_layout,
//                 &vert_module,
//                 &frag_module,
//             )
//         };

//         let blit_radiance_pipeline = {
//             let vert_spirv = compiler
//                 .compile_from_file("shaders/quad.vert", ShaderKind::Vertex)
//                 .unwrap();
//             let frag_spirv = compiler
//                 .compile_from_file("shaders/deferred_radiance.frag", ShaderKind::Fragment)
//                 .unwrap();

//             let vert_module = device.create_shader_module(vert_spirv.as_slice());
//             let frag_module = device.create_shader_module(frag_spirv.as_slice());
//             Self::create_blit_pipeline(
//                 device,
//                 output_format,
//                 &blit_pipeline_layout,
//                 &vert_module,
//                 &frag_module,
//             )
//         };

//         let ssao_output = device.create_texture(&wgpu::TextureDescriptor {
//             label: Some("ssao_output"),
//             size: wgpu::Extent3d {
//                 width: output_size.0,
//                 height: output_size.1,
//                 depth: 1,
//             },
//             mip_level_count: 1,
//             sample_count: 1,
//             dimension: wgpu::TextureDimension::D2,
//             format: Self::SSAO_FORMAT,
//             usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
//         });

//         let ssao_output_view = ssao_output.create_default_view();

//         let ssao_filtered_output = device.create_texture(&wgpu::TextureDescriptor {
//             label: Some("ssao_output"),
//             size: wgpu::Extent3d {
//                 width: output_size.0,
//                 height: output_size.1,
//                 depth: 1,
//             },
//             mip_level_count: 1,
//             sample_count: 1,
//             dimension: wgpu::TextureDimension::D2,
//             format: Self::SSAO_FORMAT,
//             usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
//         });

//         let ssao_filtered_output_view = ssao_output.create_default_view();

//         let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
//             address_mode_u: wgpu::AddressMode::Repeat,
//             address_mode_v: wgpu::AddressMode::Repeat,
//             address_mode_w: wgpu::AddressMode::Repeat,
//             mag_filter: wgpu::FilterMode::Linear,
//             min_filter: wgpu::FilterMode::Linear,
//             mipmap_filter: wgpu::FilterMode::Linear,
//             lod_min_clamp: 0.0,
//             lod_max_clamp: 1.0,
//             compare: wgpu::CompareFunction::Never,
//         });

//         let blit_bind_group0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
//             layout: &blit_bind_group_layout,
//             bindings: &[
//                 wgpu::Binding {
//                     binding: 0,
//                     resource: wgpu::BindingResource::Sampler(&blit_sampler),
//                 },
//                 wgpu::Binding {
//                     binding: 1,
//                     resource: wgpu::BindingResource::TextureView(&albedo_view),
//                 },
//                 wgpu::Binding {
//                     binding: 2,
//                     resource: wgpu::BindingResource::TextureView(&normal_view),
//                 },
//                 wgpu::Binding {
//                     binding: 3,
//                     resource: wgpu::BindingResource::TextureView(&world_pos_view),
//                 },
//                 wgpu::Binding {
//                     binding: 4,
//                     resource: wgpu::BindingResource::TextureView(&radiance_view),
//                 },
//                 wgpu::Binding {
//                     binding: 5,
//                     resource: wgpu::BindingResource::TextureView(&screen_space_view),
//                 },
//                 wgpu::Binding {
//                     binding: 6,
//                     resource: wgpu::BindingResource::TextureView(&intermediate_view0),
//                 },
//                 wgpu::Binding {
//                     binding: 7,
//                     resource: wgpu::BindingResource::Buffer {
//                         buffer: &uniform_buffer,
//                         range: 0..160,
//                     },
//                 },
//                 wgpu::Binding {
//                     binding: 8,
//                     resource: wgpu::BindingResource::Buffer {
//                         buffer: &ssao_hemisphere_sample_buffer,
//                         range: 0..((Self::SSAO_KERNEL_SIZE * std::mem::size_of::<Vec3>())
//                             as BufferAddress),
//                     },
//                 },
//                 wgpu::Binding {
//                     binding: 9,
//                     resource: wgpu::BindingResource::TextureView(&ssao_noise_texture_view),
//                 },
//             ],
//             label: Some("blit-bind-group"),
//         });

//         let blit_bind_group1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
//             layout: &blit_bind_group_layout,
//             bindings: &[
//                 wgpu::Binding {
//                     binding: 0,
//                     resource: wgpu::BindingResource::Sampler(&blit_sampler),
//                 },
//                 wgpu::Binding {
//                     binding: 1,
//                     resource: wgpu::BindingResource::TextureView(&albedo_view),
//                 },
//                 wgpu::Binding {
//                     binding: 2,
//                     resource: wgpu::BindingResource::TextureView(&normal_view),
//                 },
//                 wgpu::Binding {
//                     binding: 3,
//                     resource: wgpu::BindingResource::TextureView(&world_pos_view),
//                 },
//                 wgpu::Binding {
//                     binding: 4,
//                     resource: wgpu::BindingResource::TextureView(&radiance_view),
//                 },
//                 wgpu::Binding {
//                     binding: 5,
//                     resource: wgpu::BindingResource::TextureView(&intermediate_view1),
//                 },
//                 wgpu::Binding {
//                     binding: 6,
//                     resource: wgpu::BindingResource::TextureView(&screen_space_view),
//                 },
//                 wgpu::Binding {
//                     binding: 7,
//                     resource: wgpu::BindingResource::Buffer {
//                         buffer: &uniform_buffer,
//                         range: 0..160,
//                     },
//                 },
//                 wgpu::Binding {
//                     binding: 8,
//                     resource: wgpu::BindingResource::Buffer {
//                         buffer: &ssao_hemisphere_sample_buffer,
//                         range: 0..((Self::SSAO_KERNEL_SIZE * std::mem::size_of::<Vec3>())
//                             as BufferAddress),
//                     },
//                 },
//                 wgpu::Binding {
//                     binding: 9,
//                     resource: wgpu::BindingResource::TextureView(&ssao_noise_texture_view),
//                 },
//             ],
//             label: Some("blit-bind-group"),
//         });

//         let radiance_bind_group_layout0 =
//             device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
//                 label: Some("radiance-bind-group-layout0"),
//                 bindings: &[
//                     wgpu::BindGroupLayoutEntry {
//                         binding: 0,
//                         visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::UniformBuffer { dynamic: false },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         binding: 1,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::Sampler { comparison: false },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         // Albedo
//                         binding: 2,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::SampledTexture {
//                             /// Dimension of the texture view that is going to be sampled.
//                             dimension: wgpu::TextureViewDimension::D2,
//                             /// Component type of the texture.
//                             /// This must be compatible with the format of the texture.
//                             component_type: wgpu::TextureComponentType::Float,
//                             /// True if the texture has a sample count greater than 1.
//                             multisampled: false,
//                         },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         // Normal
//                         binding: 3,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::SampledTexture {
//                             /// Dimension of the texture view that is going to be sampled.
//                             dimension: wgpu::TextureViewDimension::D2,
//                             /// Component type of the texture.
//                             /// This must be compatible with the format of the texture.
//                             component_type: wgpu::TextureComponentType::Float,
//                             /// True if the texture has a sample count greater than 1.
//                             multisampled: false,
//                         },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         // WorldPos
//                         binding: 4,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::SampledTexture {
//                             /// Dimension of the texture view that is going to be sampled.
//                             dimension: wgpu::TextureViewDimension::D2,
//                             /// Component type of the texture.
//                             /// This must be compatible with the format of the texture.
//                             component_type: wgpu::TextureComponentType::Float,
//                             /// True if the texture has a sample count greater than 1.
//                             multisampled: false,
//                         },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         // SSAO
//                         binding: 5,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::SampledTexture {
//                             /// Dimension of the texture view that is going to be sampled.
//                             dimension: wgpu::TextureViewDimension::D2,
//                             /// Component type of the texture.
//                             /// This must be compatible with the format of the texture.
//                             component_type: wgpu::TextureComponentType::Float,
//                             /// True if the texture has a sample count greater than 1.
//                             multisampled: false,
//                         },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         binding: 6,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::StorageBuffer {
//                             dynamic: false,
//                             readonly: true,
//                         },
//                     },
//                 ],
//             });

//         let radiance_bind_group0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
//             label: Some("radiance-bind-group0"),
//             layout: &radiance_bind_group_layout0,
//             bindings: &[
//                 Binding {
//                     binding: 0,
//                     resource: BindingResource::Buffer {
//                         buffer: &uniform_buffer,
//                         range: 0..160,
//                     },
//                 },
//                 wgpu::Binding {
//                     binding: 1,
//                     resource: wgpu::BindingResource::Sampler(&blit_sampler),
//                 },
//                 wgpu::Binding {
//                     binding: 2,
//                     resource: wgpu::BindingResource::TextureView(&albedo_view),
//                 },
//                 wgpu::Binding {
//                     binding: 3,
//                     resource: wgpu::BindingResource::TextureView(&normal_view),
//                 },
//                 wgpu::Binding {
//                     binding: 4,
//                     resource: wgpu::BindingResource::TextureView(&world_pos_view),
//                 },
//                 wgpu::Binding {
//                     binding: 5,
//                     resource: wgpu::BindingResource::TextureView(&ssao_filtered_output_view),
//                 },
//                 wgpu::Binding {
//                     binding: 6,
//                     resource: wgpu::BindingResource::Buffer {
//                         buffer: &material_buffer.1,
//                         range: 0..material_buffer.0,
//                     },
//                 },
//             ],
//         });

//         let radiance_bind_group_layout1 =
//             device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
//                 label: Some("radiance-bind-group-layout1"),
//                 bindings: &[
//                     // wgpu::BindGroupLayoutEntry {
//                     //     binding: 0,
//                     //     visibility: wgpu::ShaderStage::FRAGMENT,
//                     //     ty: wgpu::BindingType::UniformBuffer { dynamic: false },
//                     // },
//                     wgpu::BindGroupLayoutEntry {
//                         binding: 1,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::UniformBuffer { dynamic: false },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         binding: 2,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::UniformBuffer { dynamic: false },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         binding: 3,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::UniformBuffer { dynamic: false },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         // Shadow sampler
//                         binding: 4,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::Sampler { comparison: false },
//                     },
//                     // wgpu::BindGroupLayoutEntry {
//                     //     binding: 5,
//                     //     visibility: wgpu::ShaderStage::FRAGMENT,
//                     //     ty: wgpu::BindingType::SampledTexture {
//                     //         dimension: wgpu::TextureViewDimension::D2,
//                     //         multisampled: false,
//                     //         component_type: wgpu::TextureComponentType::Float,
//                     //     },
//                     // },
//                     wgpu::BindGroupLayoutEntry {
//                         binding: 6,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::SampledTexture {
//                             dimension: wgpu::TextureViewDimension::D2,
//                             multisampled: false,
//                             component_type: wgpu::TextureComponentType::Float,
//                         },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         binding: 7,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::SampledTexture {
//                             dimension: wgpu::TextureViewDimension::D2,
//                             multisampled: false,
//                             component_type: wgpu::TextureComponentType::Float,
//                         },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         binding: 8,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::SampledTexture {
//                             dimension: wgpu::TextureViewDimension::D2,
//                             multisampled: false,
//                             component_type: wgpu::TextureComponentType::Float,
//                         },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         binding: 9,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::UniformBuffer { dynamic: false },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         binding: 10,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::UniformBuffer { dynamic: false },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         binding: 11,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::UniformBuffer { dynamic: false },
//                     },
//                 ],
//             });

//         let radiance_bind_group1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
//             label: Some("radiance-bind-group1"),
//             layout: &radiance_bind_group_layout1,
//             bindings: &[
//                 // Binding {
//                 //     binding: 0,
//                 //     resource: BindingResource::Buffer {
//                 //         buffer: &point_lights_buffer,
//                 //         range: 0..((scene.point_lights().len().max(1)
//                 //             * std::mem::size_of::<PointLight>())
//                 //             as BufferAddress),
//                 //     },
//                 // },
//                 Binding {
//                     binding: 1,
//                     resource: BindingResource::Buffer {
//                         buffer: &area_lights_buffer,
//                         range: 0..((lights.area_lights.len().max(1)
//                             * std::mem::size_of::<AreaLight>())
//                             as BufferAddress),
//                     },
//                 },
//                 Binding {
//                     binding: 2,
//                     resource: BindingResource::Buffer {
//                         buffer: &spot_lights_buffer,
//                         range: 0..((lights.spot_lights.len().max(1)
//                             * std::mem::size_of::<SpotLight>())
//                             as BufferAddress),
//                     },
//                 },
//                 Binding {
//                     binding: 3,
//                     resource: BindingResource::Buffer {
//                         buffer: &directional_lights_buffer,
//                         range: 0..((lights.directional_lights.len().max(1)
//                             * std::mem::size_of::<DirectionalLight>())
//                             as BufferAddress),
//                     },
//                 },
//                 Binding {
//                     binding: 4,
//                     resource: BindingResource::Sampler(&shadow_map_sampler),
//                 },
//                 // point_lights_shadow_maps.as_binding(5),
//                 area_lights_shadow_maps.as_binding(6),
//                 spot_lights_shadow_maps.as_binding(7),
//                 directional_lights_shadow_maps.as_binding(8),
//                 Binding {
//                     binding: 9,
//                     resource: BindingResource::Buffer {
//                         range: 0..(lights.area_lights.len().max(1) as BufferAddress
//                             * ShadowMapArray::UNIFORM_ELEMENT_SIZE as BufferAddress),
//                         buffer: &area_lights_shadow_maps.uniform_buffer,
//                     },
//                 },
//                 Binding {
//                     binding: 10,
//                     resource: BindingResource::Buffer {
//                         range: 0..(lights.spot_lights.len().max(1) as BufferAddress
//                             * ShadowMapArray::UNIFORM_ELEMENT_SIZE as BufferAddress),
//                         buffer: &spot_lights_shadow_maps.uniform_buffer,
//                     },
//                 },
//                 Binding {
//                     binding: 11,
//                     resource: BindingResource::Buffer {
//                         range: 0..(lights.directional_lights.len() as BufferAddress
//                             * ShadowMapArray::UNIFORM_ELEMENT_SIZE as BufferAddress),
//                         buffer: &directional_lights_shadow_maps.uniform_buffer,
//                     },
//                 },
//             ],
//         });

//         let radiance_pipeline_layout =
//             device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
//                 bind_group_layouts: &[&radiance_bind_group_layout0, &radiance_bind_group_layout1],
//             });

//         let radiance_pipeline = {
//             let vert_spirv = compiler
//                 .compile_from_file("shaders/quad.vert", ShaderKind::Vertex)
//                 .unwrap();
//             let frag_spirv = compiler
//                 .compile_from_file("shaders/deferred_lighting.frag", ShaderKind::Fragment)
//                 .unwrap();

//             let vert_module = device.create_shader_module(vert_spirv.as_slice());
//             let frag_module = device.create_shader_module(frag_spirv.as_slice());
//             Self::create_blit_pipeline(
//                 device,
//                 Self::STORAGE_FORMAT,
//                 &radiance_pipeline_layout,
//                 &vert_module,
//                 &frag_module,
//             )
//         };

//         let fxaa_render_pipeline = {
//             let vert_spirv = compiler
//                 .compile_from_file("shaders/quad.vert", ShaderKind::Vertex)
//                 .unwrap();
//             let frag_spirv = compiler
//                 .compile_from_file("shaders/deferred_fxaa.frag", ShaderKind::Fragment)
//                 .unwrap();

//             let vert_module = device.create_shader_module(vert_spirv.as_slice());
//             let frag_module = device.create_shader_module(frag_spirv.as_slice());
//             Self::create_blit_pipeline(
//                 device,
//                 output_format,
//                 &blit_pipeline_layout,
//                 &vert_module,
//                 &frag_module,
//             )
//         };

//         let ssao_render_pipeline = {
//             let vert_spirv = compiler
//                 .compile_from_file("shaders/quad.vert", ShaderKind::Vertex)
//                 .unwrap();
//             let frag_spirv = compiler
//                 .compile_from_file("shaders/deferred_ssao.frag", ShaderKind::Fragment)
//                 .unwrap();

//             let vert_module = device.create_shader_module(vert_spirv.as_slice());
//             let frag_module = device.create_shader_module(frag_spirv.as_slice());
//             Self::create_blit_pipeline(
//                 device,
//                 Self::SSAO_FORMAT,
//                 &blit_pipeline_layout,
//                 &vert_module,
//                 &frag_module,
//             )
//         };

//         let ssao_filter_bind_group_layout =
//             device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
//                 label: Some("ssao_filter_bind_group_layout"),
//                 bindings: &[
//                     wgpu::BindGroupLayoutEntry {
//                         binding: 0,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::Sampler { comparison: false },
//                     },
//                     wgpu::BindGroupLayoutEntry {
//                         binding: 1,
//                         visibility: wgpu::ShaderStage::FRAGMENT,
//                         ty: wgpu::BindingType::SampledTexture {
//                             component_type: wgpu::TextureComponentType::Float,
//                             dimension: wgpu::TextureViewDimension::D2,
//                             multisampled: false,
//                         },
//                     },
//                 ],
//             });

//         let ssao_filter_pipeline_layout =
//             device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
//                 bind_group_layouts: &[&ssao_filter_bind_group_layout],
//             });

//         let ssao_filter_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
//             label: Some("ssao_filter_bind_group"),
//             layout: &ssao_filter_bind_group_layout,
//             bindings: &[
//                 wgpu::Binding {
//                     binding: 0,
//                     resource: wgpu::BindingResource::Sampler(&blit_sampler),
//                 },
//                 wgpu::Binding {
//                     binding: 1,
//                     resource: wgpu::BindingResource::TextureView(&ssao_output_view),
//                 },
//             ],
//         });

//         let ssao_filter_render_pipeline = {
//             let vert_spirv = compiler
//                 .compile_from_file("shaders/quad.vert", ShaderKind::Vertex)
//                 .unwrap();
//             let frag_spirv = compiler
//                 .compile_from_file("shaders/deferred_ssao_filter.frag", ShaderKind::Fragment)
//                 .unwrap();

//             let vert_module = device.create_shader_module(vert_spirv.as_slice());
//             let frag_module = device.create_shader_module(frag_spirv.as_slice());
//             Self::create_blit_pipeline(
//                 device,
//                 Self::SSAO_FORMAT,
//                 &ssao_filter_pipeline_layout,
//                 &vert_module,
//                 &frag_module,
//             )
//         };

//         DeferredRenderer {
//             objects: scene.get_scene(),
//             lights: scene.get_lights(),
//             materials: scene.get_materials(),
//             uniform_buffer,
//             staging_buffer,
//             render_pipeline,
//             render_pipeline_layout,
//             output_format,
//             blit_bind_group_layout,
//             blit_bind_group0,
//             blit_bind_group1,
//             blit_sampler,
//             blit_pipeline,
//             blit_pipeline_layout,
//             blit_albedo_pipeline,
//             blit_normal_pipeline,
//             blit_world_pos_pipeline,
//             blit_depth_pipeline,
//             blit_radiance_pipeline,
//             albedo_texture,
//             albedo_view,
//             normal_texture,
//             normal_view,
//             world_pos_texture,
//             world_pos_view,
//             screen_space_texture,
//             screen_space_view,
//             intermediate_texture0,
//             intermediate_view0,
//             intermediate_texture1,
//             intermediate_view1,
//             ssao_output,
//             ssao_output_view,
//             ssao_filtered_output,
//             ssao_filtered_output_view,

//             radiance_texture,
//             radiance_view,
//             radiance_pipeline,
//             radiance_pipeline_layout,
//             radiance_bind_group0,
//             radiance_bind_group1,
//             radiance_bind_group_layout0,
//             radiance_bind_group_layout1,
//             uniform_bind_group,
//             instance_bind_groups: Vec::new(),
//             instance_buffers: Vec::new(),
//             vertex_buffers: Vec::new(),
//             material_buffer,
//             material_textures: Vec::new(),
//             material_texture_views: Vec::new(),
//             material_texture_sampler,
//             material_bind_groups: Vec::new(),
//             uniform_bind_group_layout,
//             texture_bind_group_layout,
//             triangle_bind_group_layout,
//             shadow_map_sampler,
//             // point_lights_buffer,
//             // point_lights_matrices,
//             // point_lights_shadow_maps,
//             area_lights_buffer,
//             area_lights_matrices,
//             area_lights_shadow_maps,
//             spot_lights_buffer,
//             spot_lights_matrices,
//             spot_lights_shadow_maps,
//             directional_lights_buffer,
//             directional_lights_matrices,
//             directional_lights_shadow_maps,
//             light_counts,
//             fxaa_render_pipeline,

//             ssao_hemisphere_sample_buffer,
//             ssao_noise_texture,
//             ssao_noise_texture_view,
//             ssao_render_pipeline,
//             ssao_filter_render_pipeline,
//             ssao_filter_pipeline_layout,
//             ssao_filter_bind_group_layout,
//             ssao_filter_bind_group,
//         }
//     }

//     pub fn synchronize_lights(
//         &mut self,
//         lights: &mut SceneLights,
//         scene: &InstancedObjects,
//         device: &wgpu::Device,
//         queue: &wgpu::Queue,
//         camera: Option<&Camera>,
//     ) {
//         let mut cmd_buffers = Vec::with_capacity(4);

//         let mut frustrum = None;
//         if let Some(camera) = camera {
//             frustrum = Some(FrustrumG::from(camera.get_rh_matrix()));
//         }

//         let scene_bounds = scene.bounds();
//         let instances_changed = scene.instances_changed.any();
//         let pl_changed = lights.pl_changed.any();
//         let sl_changed = lights.sl_changed.any();
//         let al_changed = lights.al_changed.any();
//         let dl_changed = lights.dl_changed.any();

//         if pl_changed || sl_changed || al_changed || dl_changed {
//             let (
//                 _point_lights_buffer,
//                 area_lights_buffer,
//                 spot_lights_buffer,
//                 directional_lights_buffer,
//             ) = Self::create_light_buffers(device, queue, &lights);

//             // TODO: Point light shadow map
//             // self.point_lights_buffer = point_lights_buffer;
//             self.area_lights_buffer = area_lights_buffer;
//             self.spot_lights_buffer = spot_lights_buffer;
//             self.directional_lights_buffer = directional_lights_buffer;

//             self.light_counts = [
//                 lights.point_lights.len() as u32,
//                 lights.area_lights.len() as u32,
//                 lights.spot_lights.len() as u32,
//                 lights.directional_lights.len() as u32,
//             ];
//         }

//         let _pl_changed = (instances_changed || pl_changed) && !lights.point_lights.is_empty();
//         let sl_changed = (instances_changed || sl_changed) & &!lights.spot_lights.is_empty();
//         let al_changed = (instances_changed || al_changed) & &!lights.area_lights.is_empty();
//         let dl_changed = (instances_changed || dl_changed) & &!lights.directional_lights.is_empty();

//         // TODO: Only update those necessary

//         if sl_changed {
//             let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
//                 label: Some("spot-light-map-render"),
//             });

//             self.spot_lights_shadow_maps
//                 .resize(device, queue, lights.spot_lights.len());
//             self.spot_lights_matrices = lights
//                 .spot_lights
//                 .iter()
//                 .map(|l| l.get_light_info())
//                 .collect();

//             self.spot_lights_shadow_maps
//                 .update_infos(self.spot_lights_matrices.as_slice(), device);

//             if let Some(frustrum) = frustrum.as_ref() {
//                 let mut ranges: Vec<(usize, AABB)> = lights
//                     .spot_lights
//                     .iter()
//                     .enumerate()
//                     .filter(|(i, _)| lights.sl_changed[*i])
//                     .map(|(i, l)| (i, l.get_range()))
//                     .filter(|(_, r)| frustrum.aabb_in_frustrum(&r) != FrustrumResult::Outside)
//                     .collect();

//                 if let Some(camera) = camera {
//                     let pos: Vec3 = camera.pos.into();

//                     // Favor lights closer to camera
//                     ranges.sort_unstable_by(|a, b| {
//                         let center_a: Vec3 = lights.spot_lights[a.0].position.into();
//                         let center_b: Vec3 = lights.spot_lights[b.0].position.into();

//                         let v_a = pos - center_a;
//                         let v_b = pos - center_b;

//                         let dist_a = v_a.dot(v_a);
//                         let dist_b = v_b.dot(v_b);

//                         if dist_a <= dist_b {
//                             std::cmp::Ordering::Less
//                         } else {
//                             std::cmp::Ordering::Greater
//                         }
//                     });
//                 }

//                 for i in 0..(ranges.len().min(Self::MAX_UPDATES_PER_LIGHT)) {
//                     let i = ranges[i].0;
//                     lights.sl_changed.set(i, false);
//                     let i = i as u32;
//                     self.spot_lights_shadow_maps.render(
//                         i..(i + 1),
//                         &mut encoder,
//                         self.instance_bind_groups.as_slice(),
//                         self.instance_buffers.as_slice(),
//                         self.vertex_buffers.as_slice(),
//                     );
//                 }
//             } else {
//                 self.spot_lights_shadow_maps.render(
//                     0..(self.spot_lights_matrices.len() as u32),
//                     &mut encoder,
//                     self.instance_bind_groups.as_slice(),
//                     self.instance_buffers.as_slice(),
//                     self.vertex_buffers.as_slice(),
//                 );
//                 lights.sl_changed.set_all(false);
//             }

//             cmd_buffers.push(encoder.finish());
//         }

//         if al_changed {
//             let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
//                 label: Some("area-light-map-render"),
//             });
//             self.area_lights_shadow_maps
//                 .resize(device, queue, lights.area_lights.len());
//             self.area_lights_matrices = lights
//                 .area_lights
//                 .iter()
//                 .map(|l| l.get_light_info())
//                 .collect();
//             self.area_lights_shadow_maps
//                 .update_infos(self.area_lights_matrices.as_slice(), device);
//             if let Some(frustrum) = frustrum.as_ref() {
//                 for i in 0..self.area_lights_matrices.len() {
//                     if !lights.al_changed[i] {
//                         continue;
//                     }
//                     let range = lights.area_lights[i].get_range();
//                     if frustrum.aabb_in_frustrum(&range) != FrustrumResult::Outside {
//                         let i = i as u32;
//                         self.area_lights_shadow_maps.render(
//                             i..(i + 1),
//                             &mut encoder,
//                             self.instance_bind_groups.as_slice(),
//                             self.instance_buffers.as_slice(),
//                             self.vertex_buffers.as_slice(),
//                         );
//                         lights.al_changed.set(i as usize, false);
//                     }
//                 }
//             } else {
//                 self.area_lights_shadow_maps.render(
//                     0..(self.area_lights_matrices.len() as u32),
//                     &mut encoder,
//                     self.instance_bind_groups.as_slice(),
//                     self.instance_buffers.as_slice(),
//                     self.vertex_buffers.as_slice(),
//                 );
//                 lights.sl_changed.set_all(false);
//             }

//             cmd_buffers.push(encoder.finish());
//         }

//         // if dl_changed {
//         let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
//             label: Some("dir-light-map-render"),
//         });
//         self.directional_lights_shadow_maps
//             .resize(device, queue, lights.directional_lights.len());

//         self.directional_lights_matrices = lights
//             .directional_lights
//             .iter()
//             .map(|l| l.get_light_info())
//             .collect();

//         self.directional_lights_shadow_maps
//             .update_infos(self.directional_lights_matrices.as_slice(), device);

//         self.directional_lights_shadow_maps.render(
//             0..(self.directional_lights_matrices.len() as u32),
//             &mut encoder,
//             self.instance_bind_groups.as_slice(),
//             self.instance_buffers.as_slice(),
//             self.vertex_buffers.as_slice(),
//         );
//         // lights.dl_changed.set_all(false);

//         cmd_buffers.push(encoder.finish());
//         // }

//         queue.submit(cmd_buffers);
//     }

//     pub fn synchronize(
//         &mut self,
//         device: &wgpu::Device,
//         queue: &wgpu::Queue,
//         camera: Option<&Camera>,
//     ) {
//         use wgpu::*;

//         let l = self.lights.clone();
//         let i = self.objects.clone();
//         let m = self.materials.clone();

//         let mut lights = l.lock().unwrap();
//         let scene = i.lock().unwrap();
//         let materials = m.lock().unwrap();

//         self.synchronize_lights(&mut lights, &scene, device, queue, camera);

//         self.uniform_bind_group = device.create_bind_group(&BindGroupDescriptor {
//             layout: &self.uniform_bind_group_layout,
//             bindings: &[
//                 Binding {
//                     binding: 0,
//                     resource: BindingResource::Buffer {
//                         buffer: &self.uniform_buffer,
//                         range: 0..160 as u64,
//                     },
//                 },
//                 Binding {
//                     binding: 1,
//                     resource: BindingResource::Buffer {
//                         buffer: &self.material_buffer.1,
//                         range: 0..(self.material_buffer.0),
//                     },
//                 },
//                 Binding {
//                     binding: 2,
//                     resource: BindingResource::Sampler(&self.material_texture_sampler),
//                 },
//             ],
//             label: Some("mesh-bind-group-descriptor"),
//         });

//         self.radiance_bind_group1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
//             label: Some("radiance-bind-group1"),
//             layout: &self.radiance_bind_group_layout1,
//             bindings: &[
//                 // Binding {
//                 //     binding: 0,
//                 //     resource: BindingResource::Buffer {
//                 //         buffer: &self.point_lights_buffer,
//                 //         range: 0..((scene.point_lights().len().max(1)
//                 //             * std::mem::size_of::<PointLight>())
//                 //             as BufferAddress),
//                 //     },
//                 // },
//                 Binding {
//                     binding: 1,
//                     resource: BindingResource::Buffer {
//                         buffer: &self.area_lights_buffer,
//                         range: 0..((lights.area_lights.len().max(1)
//                             * std::mem::size_of::<AreaLight>())
//                             as BufferAddress),
//                     },
//                 },
//                 Binding {
//                     binding: 2,
//                     resource: BindingResource::Buffer {
//                         buffer: &self.spot_lights_buffer,
//                         range: 0..((lights.spot_lights.len().max(1)
//                             * std::mem::size_of::<SpotLight>())
//                             as BufferAddress),
//                     },
//                 },
//                 Binding {
//                     binding: 3,
//                     resource: BindingResource::Buffer {
//                         buffer: &self.directional_lights_buffer,
//                         range: 0..((lights.directional_lights.len().max(1)
//                             * std::mem::size_of::<DirectionalLight>())
//                             as BufferAddress),
//                     },
//                 },
//                 Binding {
//                     binding: 4,
//                     resource: BindingResource::Sampler(&self.shadow_map_sampler),
//                 },
//                 // self.point_lights_shadow_maps.as_binding(5),
//                 self.area_lights_shadow_maps.as_binding(6),
//                 self.spot_lights_shadow_maps.as_binding(7),
//                 self.directional_lights_shadow_maps.as_binding(8),
//                 Binding {
//                     binding: 9,
//                     resource: BindingResource::Buffer {
//                         range: 0..(lights.area_lights.len().max(1) as BufferAddress
//                             * ShadowMapArray::UNIFORM_ELEMENT_SIZE as BufferAddress),
//                         buffer: &self.area_lights_shadow_maps.uniform_buffer,
//                     },
//                 },
//                 Binding {
//                     binding: 10,
//                     resource: BindingResource::Buffer {
//                         range: 0..(lights.spot_lights.len().max(1) as BufferAddress
//                             * ShadowMapArray::UNIFORM_ELEMENT_SIZE as BufferAddress),
//                         buffer: &self.spot_lights_shadow_maps.uniform_buffer,
//                     },
//                 },
//                 Binding {
//                     binding: 11,
//                     resource: BindingResource::Buffer {
//                         range: 0..(lights.directional_lights.len() as BufferAddress
//                             * ShadowMapArray::UNIFORM_ELEMENT_SIZE as BufferAddress),
//                         buffer: &self.directional_lights_shadow_maps.uniform_buffer,
//                     },
//                 },
//             ],
//         });
//     }

//     fn record_pass(
//         &self,
//         camera: &Camera,
//         device: &wgpu::Device,
//         encoder: &mut wgpu::CommandEncoder,
//         depth_texture: &wgpu::TextureView,
//     ) {
//         let mapping = self.staging_buffer.map_write(0, 168);
//         let matrix = camera.get_rh_matrix();
//         let frustrum: FrustrumG = FrustrumG::from_matrix(matrix);

//         encoder.copy_buffer_to_buffer(&self.staging_buffer, 0, &self.uniform_buffer, 0, 168);

//         {
//             let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
//                 color_attachments: &[
//                     wgpu::RenderPassColorAttachmentDescriptor {
//                         attachment: &self.albedo_view,
//                         resolve_target: None,
//                         load_op: wgpu::LoadOp::Clear,
//                         store_op: wgpu::StoreOp::Store,
//                         clear_color: wgpu::Color {
//                             r: 0.0 as f64,
//                             g: 0.0 as f64,
//                             b: 0.0 as f64,
//                             a: 0.0 as f64,
//                         },
//                     },
//                     wgpu::RenderPassColorAttachmentDescriptor {
//                         attachment: &self.normal_view,
//                         resolve_target: None,
//                         load_op: wgpu::LoadOp::Clear,
//                         store_op: wgpu::StoreOp::Store,
//                         clear_color: wgpu::Color {
//                             r: 0.0 as f64,
//                             g: 0.0 as f64,
//                             b: 0.0 as f64,
//                             a: 0.0 as f64,
//                         },
//                     },
//                     wgpu::RenderPassColorAttachmentDescriptor {
//                         attachment: &self.world_pos_view,
//                         resolve_target: None,
//                         load_op: wgpu::LoadOp::Clear,
//                         store_op: wgpu::StoreOp::Store,
//                         clear_color: wgpu::Color {
//                             r: 0.0 as f64,
//                             g: 0.0 as f64,
//                             b: 0.0 as f64,
//                             a: 1.0 as f64,
//                         },
//                     },
//                     wgpu::RenderPassColorAttachmentDescriptor {
//                         attachment: &self.screen_space_view,
//                         resolve_target: None,
//                         load_op: wgpu::LoadOp::Clear,
//                         store_op: wgpu::StoreOp::Store,
//                         clear_color: wgpu::Color {
//                             r: 0.0 as f64,
//                             g: 0.0 as f64,
//                             b: 0.0 as f64,
//                             a: 1.0 as f64,
//                         },
//                     },
//                 ],
//                 depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
//                     attachment: depth_texture,
//                     depth_load_op: wgpu::LoadOp::Clear,
//                     depth_store_op: wgpu::StoreOp::Store,
//                     clear_depth: 1.0,
//                     stencil_load_op: wgpu::LoadOp::Clear,
//                     stencil_store_op: wgpu::StoreOp::Clear,
//                     clear_stencil: 0,
//                 }),
//             });
//             render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
//             render_pass.set_pipeline(&self.render_pipeline);

//             for i in 0..self.instance_buffers.len() {
//                 let instance_buffers: &InstanceMatrices = &self.instance_buffers[i];
//                 if instance_buffers.count <= 0 {
//                     continue;
//                 }

//                 let instance_bind_group = &self.instance_bind_groups[i];
//                 let vb: &VertexBuffer = &self.vertex_buffers[i];

//                 render_pass.set_bind_group(1, instance_bind_group, &[]);
//                 render_pass.set_vertex_buffer(0, &vb.buffer, 0, 0);
//                 render_pass.set_vertex_buffer(1, &vb.buffer, 0, 0);
//                 render_pass.set_vertex_buffer(2, &vb.buffer, 0, 0);
//                 render_pass.set_vertex_buffer(3, &vb.buffer, 0, 0);
//                 render_pass.set_vertex_buffer(4, &vb.buffer, 0, 0);

//                 for i in 0..instance_buffers.count {
//                     let bounds = vb.bounds.transformed(instance_buffers.actual_matrices[i]);
//                     if frustrum.aabb_in_frustrum(&bounds) != FrustrumResult::Outside {
//                         let i = i as u32;
//                         for mesh in vb.meshes.iter() {
//                             if frustrum.aabb_in_frustrum(&mesh.bounds) != FrustrumResult::Outside {
//                                 render_pass.set_bind_group(
//                                     2,
//                                     &self.material_bind_groups[mesh.mat_id as usize],
//                                     &[],
//                                 );
//                                 render_pass.draw(mesh.first..mesh.last, i..(i + 1));
//                             }
//                         }
//                     }
//                 }
//             }
//         }

//         device.poll(wgpu::Maintain::Wait);

//         if let Ok(mut mapping) = futures::executor::block_on(mapping) {
//             let slice: &mut [u8] = mapping.as_slice();

//             let view = camera.get_view_matrix();
//             let projection = camera.get_projection();

//             unsafe {
//                 let ptr = slice.as_mut_ptr();
//                 ptr.copy_from(view.as_ref().as_ptr() as *const u8, 64);
//                 ptr.add(64)
//                     .copy_from(projection.as_ref().as_ptr() as *const u8, 64);

//                 ptr.add(128)
//                     .copy_from(self.light_counts.as_ptr() as *const u8, 16);
//                 ptr.add(144).copy_from(
//                     Vec3::from(camera.pos).extend(1.0).as_ref().as_ptr() as *const u8,
//                     16,
//                 );
//             }
//         }

//         // Calculate SSAO
//         {
//             let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
//                 color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
//                     attachment: &self.ssao_output_view,
//                     clear_color: wgpu::Color::BLACK,
//                     load_op: wgpu::LoadOp::Clear,
//                     store_op: wgpu::StoreOp::Store,
//                     resolve_target: None,
//                 }],
//                 depth_stencil_attachment: None,
//             });

//             render_pass.set_pipeline(&self.ssao_render_pipeline);
//             render_pass.set_bind_group(0, &self.blit_bind_group0, &[]);
//             render_pass.draw(0..6, 0..1);
//         }

//         // Filter SSAO
//         {
//             let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
//                 color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
//                     attachment: &self.ssao_filtered_output_view,
//                     clear_color: wgpu::Color::BLACK,
//                     load_op: wgpu::LoadOp::Clear,
//                     store_op: wgpu::StoreOp::Store,
//                     resolve_target: None,
//                 }],
//                 depth_stencil_attachment: None,
//             });

//             render_pass.set_pipeline(&self.ssao_filter_render_pipeline);
//             render_pass.set_bind_group(0, &self.ssao_filter_bind_group, &[]);
//             render_pass.draw(0..6, 0..1);
//         }

//         // Render radiance
//         {
//             let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
//                 color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
//                     attachment: &self.radiance_view,
//                     clear_color: wgpu::Color::BLACK,
//                     load_op: wgpu::LoadOp::Clear,
//                     store_op: wgpu::StoreOp::Store,
//                     resolve_target: None,
//                 }],
//                 depth_stencil_attachment: None,
//             });

//             render_pass.set_pipeline(&self.radiance_pipeline);
//             render_pass.set_bind_group(0, &self.radiance_bind_group0, &[]);
//             render_pass.set_bind_group(1, &self.radiance_bind_group1, &[]);
//             render_pass.draw(0..6, 0..1);
//         }
//     }

//     pub fn record_render(
//         &self,
//         camera: &Camera,
//         device: &wgpu::Device,
//         encoder: &mut wgpu::CommandEncoder,
//         output: &wgpu::TextureView,
//         depth_texture: &wgpu::TextureView,
//     ) {
//         self.record_pass(camera, device, encoder, depth_texture);

//         {
//             // Combine data from wgpu_renderer passes
//             let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
//                 color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
//                     attachment: &self.intermediate_view0,
//                     clear_color: wgpu::Color::BLACK,
//                     load_op: wgpu::LoadOp::Clear,
//                     store_op: wgpu::StoreOp::Store,
//                     resolve_target: None,
//                 }],
//                 depth_stencil_attachment: None,
//             });

//             render_pass.set_pipeline(&self.blit_pipeline);
//             render_pass.set_bind_group(0, &self.blit_bind_group1, &[]);
//             render_pass.draw(0..6, 0..1);
//         }

//         {
//             // Apply FXAA
//             let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
//                 color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
//                     attachment: output,
//                     clear_color: wgpu::Color::BLACK,
//                     load_op: wgpu::LoadOp::Clear,
//                     store_op: wgpu::StoreOp::Store,
//                     resolve_target: None,
//                 }],
//                 depth_stencil_attachment: None,
//             });

//             render_pass.set_pipeline(&self.fxaa_render_pipeline);
//             render_pass.set_bind_group(0, &self.blit_bind_group0, &[]);
//             render_pass.draw(0..6, 0..1);
//         }
//     }

//     pub fn render(
//         &self,
//         camera: &Camera,
//         device: &wgpu::Device,
//         output: &wgpu::TextureView,
//         depth_texture: &wgpu::TextureView,
//     ) -> wgpu::CommandBuffer {
//         let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
//             label: Some("render-command"),
//         });

//         self.record_render(camera, device, &mut encoder, output, depth_texture);
//         encoder.finish()
//     }

//     pub fn render_albedo(
//         &self,
//         camera: &Camera,
//         device: &wgpu::Device,
//         output: &wgpu::TextureView,
//         depth_texture: &wgpu::TextureView,
//     ) -> wgpu::CommandBuffer {
//         let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
//             label: Some("render-albedo-command"),
//         });

//         self.record_pass(camera, device, &mut encoder, depth_texture);

//         {
//             let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
//                 color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
//                     attachment: output,
//                     clear_color: wgpu::Color::BLACK,
//                     load_op: wgpu::LoadOp::Clear,
//                     store_op: wgpu::StoreOp::Store,
//                     resolve_target: None,
//                 }],
//                 depth_stencil_attachment: None,
//             });

//             render_pass.set_pipeline(&self.blit_albedo_pipeline);
//             render_pass.set_bind_group(0, &self.blit_bind_group0, &[]);
//             render_pass.draw(0..6, 0..1);
//         }

//         encoder.finish()
//     }

//     pub fn render_normals(
//         &self,
//         camera: &Camera,
//         device: &wgpu::Device,
//         output: &wgpu::TextureView,
//         depth_texture: &wgpu::TextureView,
//     ) -> wgpu::CommandBuffer {
//         let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
//             label: Some("render-normal-command"),
//         });

//         self.record_pass(camera, device, &mut encoder, depth_texture);

//         {
//             let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
//                 color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
//                     attachment: output,
//                     clear_color: wgpu::Color::BLACK,
//                     load_op: wgpu::LoadOp::Clear,
//                     store_op: wgpu::StoreOp::Store,
//                     resolve_target: None,
//                 }],
//                 depth_stencil_attachment: None,
//             });

//             render_pass.set_pipeline(&self.blit_normal_pipeline);
//             render_pass.set_bind_group(0, &self.blit_bind_group0, &[]);
//             render_pass.draw(0..6, 0..1);
//         }

//         encoder.finish()
//     }

//     pub fn render_world_pos(
//         &self,
//         camera: &Camera,
//         device: &wgpu::Device,
//         output: &wgpu::TextureView,
//         depth_texture: &wgpu::TextureView,
//     ) -> wgpu::CommandBuffer {
//         let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
//             label: Some("render-world-pos-command"),
//         });

//         self.record_pass(camera, device, &mut encoder, depth_texture);

//         {
//             let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
//                 color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
//                     attachment: output,
//                     clear_color: wgpu::Color::BLACK,
//                     load_op: wgpu::LoadOp::Clear,
//                     store_op: wgpu::StoreOp::Store,
//                     resolve_target: None,
//                 }],
//                 depth_stencil_attachment: None,
//             });

//             render_pass.set_pipeline(&self.blit_world_pos_pipeline);
//             render_pass.set_bind_group(0, &self.blit_bind_group0, &[]);
//             render_pass.draw(0..6, 0..1);
//         }

//         encoder.finish()
//     }

//     pub fn render_depth(
//         &self,
//         camera: &Camera,
//         device: &wgpu::Device,
//         output: &wgpu::TextureView,
//         depth_texture: &wgpu::TextureView,
//     ) -> wgpu::CommandBuffer {
//         let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
//             label: Some("render-world-pos-command"),
//         });

//         self.record_pass(camera, device, &mut encoder, depth_texture);

//         {
//             let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
//                 color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
//                     attachment: output,
//                     clear_color: wgpu::Color::BLACK,
//                     load_op: wgpu::LoadOp::Clear,
//                     store_op: wgpu::StoreOp::Store,
//                     resolve_target: None,
//                 }],
//                 depth_stencil_attachment: None,
//             });

//             render_pass.set_pipeline(&self.blit_depth_pipeline);
//             render_pass.set_bind_group(0, &self.blit_bind_group0, &[]);
//             render_pass.draw(0..6, 0..1);
//         }

//         encoder.finish()
//     }

//     pub fn render_radiance(
//         &self,
//         camera: &Camera,
//         device: &wgpu::Device,
//         output: &wgpu::TextureView,
//         depth_texture: &wgpu::TextureView,
//     ) -> wgpu::CommandBuffer {
//         let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
//             label: Some("render-normal-command"),
//         });

//         self.record_pass(camera, device, &mut encoder, depth_texture);

//         {
//             let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
//                 color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
//                     attachment: output,
//                     clear_color: wgpu::Color::BLACK,
//                     load_op: wgpu::LoadOp::Clear,
//                     store_op: wgpu::StoreOp::Store,
//                     resolve_target: None,
//                 }],
//                 depth_stencil_attachment: None,
//             });

//             render_pass.set_pipeline(&self.blit_radiance_pipeline);
//             render_pass.set_bind_group(0, &self.blit_bind_group0, &[]);
//             render_pass.draw(0..6, 0..1);
//         }

//         encoder.finish()
//     }

//     fn create_instance_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
//         use wgpu::*;
//         device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
//             bindings: &[
//                 BindGroupLayoutEntry {
//                     // Instance matrices
//                     binding: 0,
//                     visibility: ShaderStage::VERTEX,
//                     ty: BindingType::StorageBuffer {
//                         dynamic: false,
//                         readonly: true,
//                     },
//                 },
//                 BindGroupLayoutEntry {
//                     // Instance inverse matrices
//                     binding: 1,
//                     visibility: ShaderStage::VERTEX,
//                     ty: BindingType::StorageBuffer {
//                         dynamic: false,
//                         readonly: true,
//                     },
//                 },
//             ],
//             label: Some("mesh-bind-group-descriptor-layout"),
//         })
//     }

//     fn create_uniform_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
//         use wgpu::*;
//         device.create_bind_group_layout(&BindGroupLayoutDescriptor {
//             bindings: &[
//                 BindGroupLayoutEntry {
//                     // Matrix buffer
//                     binding: 0,
//                     visibility: ShaderStage::VERTEX | ShaderStage::FRAGMENT,
//                     ty: BindingType::UniformBuffer { dynamic: false },
//                 },
//                 BindGroupLayoutEntry {
//                     // Material buffer
//                     binding: 1,
//                     visibility: ShaderStage::FRAGMENT,
//                     ty: BindingType::StorageBuffer {
//                         readonly: true,
//                         dynamic: false,
//                     },
//                 },
//                 BindGroupLayoutEntry {
//                     // Texture sampler
//                     binding: 2,
//                     visibility: ShaderStage::FRAGMENT,
//                     ty: BindingType::Sampler { comparison: false },
//                 },
//             ],
//             label: Some("uniform-layout"),
//         })
//     }

//     fn create_texture_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
//         use wgpu::*;
//         device.create_bind_group_layout(&BindGroupLayoutDescriptor {
//             label: Some("texture-bind-group-layout"),
//             bindings: &[
//                 BindGroupLayoutEntry {
//                     // Albedo texture
//                     binding: 0,
//                     visibility: ShaderStage::FRAGMENT,
//                     ty: BindingType::SampledTexture {
//                         component_type: TextureComponentType::Uint,
//                         multisampled: false,
//                         dimension: TextureViewDimension::D2,
//                     },
//                 },
//                 BindGroupLayoutEntry {
//                     // Normal texture
//                     binding: 1,
//                     visibility: ShaderStage::FRAGMENT,
//                     ty: BindingType::SampledTexture {
//                         component_type: TextureComponentType::Uint,
//                         multisampled: false,
//                         dimension: TextureViewDimension::D2,
//                     },
//                 },
//             ],
//         })
//     }

//     fn create_texture_sampler(device: &wgpu::Device) -> wgpu::Sampler {
//         use wgpu::*;
//         device.create_sampler(&SamplerDescriptor {
//             address_mode_u: AddressMode::Repeat,
//             address_mode_v: AddressMode::Repeat,
//             address_mode_w: AddressMode::Repeat,
//             mag_filter: FilterMode::Linear,
//             min_filter: FilterMode::Nearest,
//             mipmap_filter: FilterMode::Nearest,
//             lod_max_clamp: 4.0,
//             lod_min_clamp: 0.0,
//             compare: CompareFunction::Undefined,
//         })
//     }

//     fn create_render_pipeline(
//         device: &wgpu::Device,
//         depth_format: wgpu::TextureFormat,
//         uniform_layout: &wgpu::BindGroupLayout,
//         triangle_layout: &wgpu::BindGroupLayout,
//         texture_layout: &wgpu::BindGroupLayout,
//     ) -> (wgpu::PipelineLayout, wgpu::RenderPipeline) {
//         use wgpu::*;

//         let mut compiler = fb_template::shader::CompilerBuilder::new().build();
//         let vert_shader = compiler
//             .compile_from_file("shaders/mesh.vert", ShaderKind::Vertex)
//             .expect("shaders/mesh.vert");
//         let frag_shader = compiler
//             .compile_from_file("shaders/wgpu_renderer.frag", ShaderKind::Fragment)
//             .expect("shaders/mesh.frag");

//         let vert_module = device.create_shader_module(vert_shader.as_slice());
//         let frag_module = device.create_shader_module(frag_shader.as_slice());

//         let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
//             bind_group_layouts: &[&uniform_layout, &triangle_layout, &texture_layout],
//         });
//         let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
//             layout: &pipeline_layout,
//             vertex_stage: ProgrammableStageDescriptor {
//                 module: &vert_module,
//                 entry_point: "main",
//             },
//             fragment_stage: Some(ProgrammableStageDescriptor {
//                 module: &frag_module,
//                 entry_point: "main",
//             }),
//             rasterization_state: Some(RasterizationStateDescriptor {
//                 front_face: FrontFace::Ccw,
//                 cull_mode: CullMode::Back,
//                 depth_bias: 0,
//                 depth_bias_slope_scale: 0.0,
//                 depth_bias_clamp: 0.0,
//             }),
//             primitive_topology: PrimitiveTopology::TriangleList,
//             color_states: &[
//                 ColorStateDescriptor {
//                     // Albedo
//                     format: Self::STORAGE_FORMAT,
//                     alpha_blend: BlendDescriptor::REPLACE,
//                     color_blend: BlendDescriptor::REPLACE,
//                     write_mask: ColorWrite::ALL,
//                 },
//                 ColorStateDescriptor {
//                     // Normal
//                     format: Self::STORAGE_FORMAT,
//                     alpha_blend: BlendDescriptor::REPLACE,
//                     color_blend: BlendDescriptor::REPLACE,
//                     write_mask: ColorWrite::ALL,
//                 },
//                 ColorStateDescriptor {
//                     // World pos
//                     format: Self::STORAGE_FORMAT,
//                     alpha_blend: BlendDescriptor::REPLACE,
//                     color_blend: BlendDescriptor::REPLACE,
//                     write_mask: ColorWrite::ALL,
//                 },
//                 ColorStateDescriptor {
//                     // Screen space
//                     format: Self::STORAGE_FORMAT,
//                     alpha_blend: BlendDescriptor::REPLACE,
//                     color_blend: BlendDescriptor::REPLACE,
//                     write_mask: ColorWrite::ALL,
//                 },
//             ],
//             depth_stencil_state: Some(DepthStencilStateDescriptor {
//                 format: depth_format,
//                 depth_write_enabled: true,
//                 depth_compare: CompareFunction::LessEqual,
//                 stencil_front: StencilStateFaceDescriptor::IGNORE,
//                 stencil_back: StencilStateFaceDescriptor::IGNORE,
//                 stencil_read_mask: 0,
//                 stencil_write_mask: 0,
//             }),
//             vertex_state: VertexStateDescriptor {
//                 vertex_buffers: &[
//                     VertexBufferDescriptor {
//                         stride: std::mem::size_of::<VertexData>() as BufferAddress,
//                         step_mode: InputStepMode::Vertex,
//                         attributes: &[VertexAttributeDescriptor {
//                             offset: 0,
//                             format: VertexFormat::Float4,
//                             shader_location: 0,
//                         }],
//                     },
//                     VertexBufferDescriptor {
//                         stride: std::mem::size_of::<VertexData>() as BufferAddress,
//                         step_mode: InputStepMode::Vertex,
//                         attributes: &[VertexAttributeDescriptor {
//                             offset: 16,
//                             format: VertexFormat::Float3,
//                             shader_location: 1,
//                         }],
//                     },
//                     VertexBufferDescriptor {
//                         stride: std::mem::size_of::<VertexData>() as BufferAddress,
//                         step_mode: InputStepMode::Vertex,
//                         attributes: &[VertexAttributeDescriptor {
//                             offset: 28,
//                             format: VertexFormat::Uint,
//                             shader_location: 2,
//                         }],
//                     },
//                     VertexBufferDescriptor {
//                         stride: std::mem::size_of::<VertexData>() as BufferAddress,
//                         step_mode: InputStepMode::Vertex,
//                         attributes: &[VertexAttributeDescriptor {
//                             offset: 32,
//                             format: VertexFormat::Float2,
//                             shader_location: 3,
//                         }],
//                     },
//                     VertexBufferDescriptor {
//                         stride: std::mem::size_of::<VertexData>() as BufferAddress,
//                         step_mode: InputStepMode::Vertex,
//                         attributes: &[VertexAttributeDescriptor {
//                             offset: 40,
//                             format: VertexFormat::Float4,
//                             shader_location: 4,
//                         }],
//                     },
//                 ],
//                 index_format: IndexFormat::Uint32,
//             },
//             sample_count: 1,
//             sample_mask: !0,
//             alpha_to_coverage_enabled: false,
//         });

//         (pipeline_layout, pipeline)
//     }

//     fn create_light_buffers(
//         device: &wgpu::Device,
//         queue: &wgpu::Queue,
//         lights: &SceneLights,
//     ) -> (wgpu::Buffer, wgpu::Buffer, wgpu::Buffer, wgpu::Buffer) {
//         use wgpu::*;

//         let mut staging_size = 0;
//         let point_size =
//             (lights.point_lights.len().max(1) * std::mem::size_of::<PointLight>()) as BufferAddress;
//         staging_size += point_size;
//         let point_light_buffer = device.create_buffer(&BufferDescriptor {
//             label: Some("point-lights-buffer"),
//             usage: BufferUsage::UNIFORM | BufferUsage::COPY_DST,
//             size: point_size,
//         });

//         let area_size =
//             (lights.area_lights.len().max(1) * std::mem::size_of::<AreaLight>()) as BufferAddress;
//         staging_size += area_size;
//         let area_light_buffer = device.create_buffer(&BufferDescriptor {
//             label: Some("area-lights-buffer"),
//             usage: BufferUsage::UNIFORM | BufferUsage::COPY_DST,
//             size: area_size,
//         });

//         let spot_size =
//             (lights.spot_lights.len().max(1) * std::mem::size_of::<SpotLight>()) as BufferAddress;
//         staging_size += spot_size;
//         let spot_light_buffer = device.create_buffer(&BufferDescriptor {
//             label: Some("spot-lights-buffer"),
//             usage: BufferUsage::UNIFORM | BufferUsage::COPY_DST,
//             size: spot_size,
//         });

//         let dir_size = (lights.directional_lights.len().max(1)
//             * std::mem::size_of::<DirectionalLight>()) as BufferAddress;
//         staging_size += dir_size;
//         let directional_light_buffer = device.create_buffer(&BufferDescriptor {
//             label: Some("directional-lights-buffer"),
//             usage: BufferUsage::UNIFORM | BufferUsage::COPY_DST,
//             size: dir_size,
//         });

//         let staging_buffer = device.create_buffer(&BufferDescriptor {
//             label: Some("lights-staging-buffer"),
//             usage: BufferUsage::COPY_SRC | BufferUsage::MAP_WRITE,
//             size: staging_size,
//         });

//         let mapping = staging_buffer.map_write(0, staging_size);

//         let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
//             label: Some("light-copy-encoder"),
//         });
//         let mut staging_data = vec![0 as u8; staging_size as usize];
//         let staging_ptr = staging_data.as_mut_ptr();

//         let mut offset = 0;
//         if !lights.point_lights.is_empty() {
//             unsafe {
//                 staging_ptr.add(offset as usize).copy_from(
//                     lights.point_lights.as_ptr() as *const u8,
//                     point_size as usize,
//                 );
//             }
//             encoder.copy_buffer_to_buffer(
//                 &staging_buffer,
//                 offset,
//                 &point_light_buffer,
//                 0,
//                 point_size,
//             );
//         }
//         offset += point_size;

//         if !lights.area_lights.is_empty() {
//             unsafe {
//                 staging_ptr
//                     .add(offset as usize)
//                     .copy_from(lights.area_lights.as_ptr() as *const u8, area_size as usize);
//             }
//             encoder.copy_buffer_to_buffer(
//                 &staging_buffer,
//                 offset,
//                 &area_light_buffer,
//                 0,
//                 area_size,
//             );
//         }
//         offset += area_size;

//         if !lights.spot_lights.is_empty() {
//             unsafe {
//                 staging_ptr
//                     .add(offset as usize)
//                     .copy_from(lights.spot_lights.as_ptr() as *const u8, spot_size as usize);
//             }
//             encoder.copy_buffer_to_buffer(
//                 &staging_buffer,
//                 offset,
//                 &spot_light_buffer,
//                 0,
//                 spot_size,
//             );
//         }
//         offset += spot_size;

//         if !lights.directional_lights.is_empty() {
//             unsafe {
//                 staging_ptr.add(offset as usize).copy_from(
//                     lights.directional_lights.as_ptr() as *const u8,
//                     dir_size as usize,
//                 );
//             }
//             encoder.copy_buffer_to_buffer(
//                 &staging_buffer,
//                 offset,
//                 &directional_light_buffer,
//                 0,
//                 dir_size,
//             );
//         }

//         {
//             device.poll(Maintain::Wait);
//             let mut mapping = block_on(mapping).unwrap();
//             mapping.as_slice().copy_from_slice(staging_data.as_slice());
//         }

//         queue.submit(Some(encoder.finish()));
//         (
//             point_light_buffer,
//             area_light_buffer,
//             spot_light_buffer,
//             directional_light_buffer,
//         )
//     }

//     fn create_blit_pipeline(
//         device: &wgpu::Device,
//         output_format: wgpu::TextureFormat,
//         pipeline_layout: &wgpu::PipelineLayout,
//         vert_module: &wgpu::ShaderModule,
//         frag_module: &wgpu::ShaderModule,
//     ) -> wgpu::RenderPipeline {
//         let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
//             layout: pipeline_layout,
//             vertex_stage: wgpu::ProgrammableStageDescriptor {
//                 module: &vert_module,
//                 entry_point: "main",
//             },
//             fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
//                 module: &frag_module,
//                 entry_point: "main",
//             }),
//             rasterization_state: Some(wgpu::RasterizationStateDescriptor {
//                 front_face: wgpu::FrontFace::Ccw,
//                 cull_mode: wgpu::CullMode::None,
//                 depth_bias: 0,
//                 depth_bias_slope_scale: 0.0,
//                 depth_bias_clamp: 0.0,
//             }),
//             primitive_topology: wgpu::PrimitiveTopology::TriangleList,
//             color_states: &[wgpu::ColorStateDescriptor {
//                 format: output_format,
//                 color_blend: wgpu::BlendDescriptor::REPLACE,
//                 alpha_blend: wgpu::BlendDescriptor::REPLACE,
//                 write_mask: wgpu::ColorWrite::ALL,
//             }],
//             depth_stencil_state: None,
//             vertex_state: wgpu::VertexStateDescriptor {
//                 index_format: wgpu::IndexFormat::Uint32,
//                 vertex_buffers: &[],
//             },
//             sample_count: 1,
//             sample_mask: !0,
//             alpha_to_coverage_enabled: false,
//         });

//         blit_pipeline
//     }

//     pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
//         use wgpu::*;

//         let descriptor = wgpu::TextureDescriptor {
//             label: None,
//             size: wgpu::Extent3d {
//                 width,
//                 height,
//                 depth: 1,
//             },
//             mip_level_count: 1,
//             sample_count: 1,
//             dimension: wgpu::TextureDimension::D2,
//             format: Self::STORAGE_FORMAT,
//             usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
//         };

//         let albedo_texture = device.create_texture(&descriptor);
//         let albedo_view = albedo_texture.create_default_view();

//         self.albedo_view = albedo_view;
//         self.albedo_texture = albedo_texture;

//         let normal_texture = device.create_texture(&descriptor);
//         let normal_view = normal_texture.create_default_view();

//         self.normal_view = normal_view;
//         self.normal_texture = normal_texture;

//         let world_pos_texture = device.create_texture(&descriptor);
//         let world_pos_view = world_pos_texture.create_default_view();

//         self.world_pos_view = world_pos_view;
//         self.world_pos_texture = world_pos_texture;

//         let radiance_texture = device.create_texture(&descriptor);
//         let radiance_view = radiance_texture.create_default_view();

//         self.radiance_view = radiance_view;
//         self.radiance_texture = radiance_texture;

//         let screen_space_texture = device.create_texture(&descriptor);
//         let screen_space_view = screen_space_texture.create_default_view();

//         self.screen_space_texture = screen_space_texture;
//         self.screen_space_view = screen_space_view;

//         let intermediate_texture0 = device.create_texture(&wgpu::TextureDescriptor {
//             label: None,
//             size: Extent3d {
//                 width,
//                 height,
//                 depth: 1,
//             },
//             mip_level_count: 1,
//             sample_count: 1,
//             dimension: wgpu::TextureDimension::D2,
//             format: self.output_format,
//             usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
//         });
//         let intermediate_view0 = intermediate_texture0.create_default_view();

//         let intermediate_texture1 = device.create_texture(&wgpu::TextureDescriptor {
//             label: None,
//             size: Extent3d {
//                 width,
//                 height,
//                 depth: 1,
//             },
//             mip_level_count: 1,
//             sample_count: 1,
//             dimension: wgpu::TextureDimension::D2,
//             format: self.output_format,
//             usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
//         });
//         let intermediate_view1 = intermediate_texture1.create_default_view();

//         self.intermediate_view0 = intermediate_view0;
//         self.intermediate_view1 = intermediate_view1;

//         self.intermediate_texture0 = intermediate_texture0;
//         self.intermediate_texture1 = intermediate_texture1;

//         let ssao_descriptor = wgpu::TextureDescriptor {
//             label: Some("ssao_output"),
//             size: wgpu::Extent3d {
//                 width,
//                 height,
//                 depth: 1,
//             },
//             mip_level_count: 1,
//             sample_count: 1,
//             dimension: wgpu::TextureDimension::D2,
//             format: Self::SSAO_FORMAT,
//             usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
//         };
//         let ssao_output = device.create_texture(&ssao_descriptor);
//         let ssao_output_view = ssao_output.create_default_view();
//         let ssao_filtered_output = device.create_texture(&ssao_descriptor);
//         let ssao_filtered_output_view = ssao_filtered_output.create_default_view();

//         self.ssao_output_view = ssao_output_view;
//         self.ssao_output = ssao_output;

//         self.ssao_filtered_output_view = ssao_filtered_output_view;
//         self.ssao_filtered_output = ssao_filtered_output;

//         self.blit_bind_group0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
//             layout: &self.blit_bind_group_layout,
//             bindings: &[
//                 wgpu::Binding {
//                     binding: 0,
//                     resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
//                 },
//                 wgpu::Binding {
//                     binding: 1,
//                     resource: wgpu::BindingResource::TextureView(&self.albedo_view),
//                 },
//                 wgpu::Binding {
//                     binding: 2,
//                     resource: wgpu::BindingResource::TextureView(&self.normal_view),
//                 },
//                 wgpu::Binding {
//                     binding: 3,
//                     resource: wgpu::BindingResource::TextureView(&self.world_pos_view),
//                 },
//                 wgpu::Binding {
//                     binding: 4,
//                     resource: wgpu::BindingResource::TextureView(&self.radiance_view),
//                 },
//                 wgpu::Binding {
//                     binding: 5,
//                     resource: wgpu::BindingResource::TextureView(&self.screen_space_view),
//                 },
//                 wgpu::Binding {
//                     binding: 6,
//                     resource: wgpu::BindingResource::TextureView(&self.intermediate_view0),
//                 },
//                 wgpu::Binding {
//                     binding: 7,
//                     resource: wgpu::BindingResource::Buffer {
//                         buffer: &self.uniform_buffer,
//                         range: 0..160,
//                     },
//                 },
//                 wgpu::Binding {
//                     binding: 8,
//                     resource: wgpu::BindingResource::Buffer {
//                         buffer: &self.ssao_hemisphere_sample_buffer,
//                         range: 0..((Self::SSAO_KERNEL_SIZE * std::mem::size_of::<Vec3>())
//                             as BufferAddress),
//                     },
//                 },
//                 wgpu::Binding {
//                     binding: 9,
//                     resource: wgpu::BindingResource::TextureView(&self.ssao_noise_texture_view),
//                 },
//             ],
//             label: Some("blit-bind-group"),
//         });

//         self.blit_bind_group1 = device.create_bind_group(&wgpu::BindGroupDescriptor {
//             layout: &self.blit_bind_group_layout,
//             bindings: &[
//                 wgpu::Binding {
//                     binding: 0,
//                     resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
//                 },
//                 wgpu::Binding {
//                     binding: 1,
//                     resource: wgpu::BindingResource::TextureView(&self.albedo_view),
//                 },
//                 wgpu::Binding {
//                     binding: 2,
//                     resource: wgpu::BindingResource::TextureView(&self.normal_view),
//                 },
//                 wgpu::Binding {
//                     binding: 3,
//                     resource: wgpu::BindingResource::TextureView(&self.world_pos_view),
//                 },
//                 wgpu::Binding {
//                     binding: 4,
//                     resource: wgpu::BindingResource::TextureView(&self.radiance_view),
//                 },
//                 wgpu::Binding {
//                     binding: 5,
//                     resource: wgpu::BindingResource::TextureView(&self.screen_space_view),
//                 },
//                 wgpu::Binding {
//                     binding: 6,
//                     resource: wgpu::BindingResource::TextureView(&self.intermediate_view1),
//                 },
//                 wgpu::Binding {
//                     binding: 7,
//                     resource: wgpu::BindingResource::Buffer {
//                         buffer: &self.uniform_buffer,
//                         range: 0..160,
//                     },
//                 },
//                 wgpu::Binding {
//                     binding: 8,
//                     resource: wgpu::BindingResource::Buffer {
//                         buffer: &self.ssao_hemisphere_sample_buffer,
//                         range: 0..((Self::SSAO_KERNEL_SIZE * std::mem::size_of::<Vec3>())
//                             as BufferAddress),
//                     },
//                 },
//                 wgpu::Binding {
//                     binding: 9,
//                     resource: wgpu::BindingResource::TextureView(&self.ssao_noise_texture_view),
//                 },
//             ],
//             label: Some("blit-bind-group"),
//         });

//         self.radiance_bind_group0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
//             label: Some("radiance-bind-group0"),
//             layout: &self.radiance_bind_group_layout0,
//             bindings: &[
//                 Binding {
//                     binding: 0,
//                     resource: BindingResource::Buffer {
//                         buffer: &self.uniform_buffer,
//                         range: 0..160,
//                     },
//                 },
//                 wgpu::Binding {
//                     binding: 1,
//                     resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
//                 },
//                 wgpu::Binding {
//                     binding: 2,
//                     resource: wgpu::BindingResource::TextureView(&self.albedo_view),
//                 },
//                 wgpu::Binding {
//                     binding: 3,
//                     resource: wgpu::BindingResource::TextureView(&self.normal_view),
//                 },
//                 wgpu::Binding {
//                     binding: 4,
//                     resource: wgpu::BindingResource::TextureView(&self.world_pos_view),
//                 },
//                 wgpu::Binding {
//                     binding: 5,
//                     resource: wgpu::BindingResource::TextureView(&self.ssao_filtered_output_view),
//                 },
//                 wgpu::Binding {
//                     binding: 6,
//                     resource: wgpu::BindingResource::Buffer {
//                         buffer: &self.material_buffer.1,
//                         range: 0..self.material_buffer.0,
//                     },
//                 },
//             ],
//         });

//         self.ssao_filter_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
//             label: Some("ssao_filter_bind_group"),
//             layout: &self.ssao_filter_bind_group_layout,
//             bindings: &[
//                 wgpu::Binding {
//                     binding: 0,
//                     resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
//                 },
//                 wgpu::Binding {
//                     binding: 1,
//                     resource: wgpu::BindingResource::TextureView(&self.ssao_output_view),
//                 },
//             ],
//         });
//     }
// }
