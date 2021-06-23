#import <AppKit/AppKit.h>

#include "renderer.hpp"

#include <cassert>
#include <cstring>
#include <filesystem>

#include <glm/ext.hpp>
#include <glm/glm.hpp>

#include "shaders.h"

using namespace glm;

mat4 get_rh_matrix(const CameraView3D &view)
{
    const float width = 1.0f / view.inv_width;
    const float height = 1.0f / view.inv_height;

    const vec3 pos = vec3(view.pos.x, view.pos.y, view.pos.z);
    const vec3 direction = vec3(view.direction.x, view.direction.y, view.direction.z);
    const vec3 up = vec3(0, 1, 0);
    const mat4 projection = perspectiveFovRH(view.fov, width, height, view.near_plane, view.far_plane);
    const mat4 v = lookAtRH(pos, pos + direction, up);
    return projection * v;
}

mat4 get_rh_projection_matrix(const CameraView3D &view)
{
    const float width = 1.0f / view.inv_width;
    const float height = 1.0f / view.inv_height;
    return perspectiveFovRH(view.fov, width, height, view.near_plane, view.far_plane);
}

mat4 get_rh_view_matrix(const CameraView3D &view)
{
    const vec3 pos = vec3(view.pos.x, view.pos.y, view.pos.z);
    const vec3 direction = vec3(view.direction.x, view.direction.y, view.direction.z);
    const vec3 up = vec3(0, 1, 0);
    return lookAtRH(pos, pos + direction, up);
}

MetalRenderer *MetalRenderer::create_instance(void *ns_window, void *ns_view, unsigned int width, unsigned int height,
                                              double scale)
{
    NSArray<id<MTLDevice>> *devices = MTLCopyAllDevices();
    id<MTLDevice> device = nil;
    for (id<MTLDevice> dev in devices)
    {
        if (!dev.lowPower)
            device = dev;
    }

    if (device == nil)
        device = MTLCreateSystemDefaultDevice();

    if (!device)
        return nullptr;
    return new MetalRenderer(device, ns_window, ns_view, width, height, scale);
}

#define MTL_ERROR(x)                                                                                                   \
    if (x)                                                                                                             \
    {                                                                                                                  \
        NSLog(@"%s:%i %@", __FILE__, __LINE__, [x localizedDescription]);                                              \
        assert(false);                                                                                                 \
    }

MetalRenderer::~MetalRenderer()
{
    _layer = nil;
    _queue = nil;
    _state = nil;
    _state_2d = nil;
    _library = nil;
    _depth_texture = nil;
    _depth_state = nil;
    _device = nil;
}

MetalRenderer::MetalRenderer(id<MTLDevice> device, void *ns_window, void *, unsigned int width, unsigned int height,
                             double scale)
    : _device(device), _uniforms(device, 1), _materials(device, 32), _camera(device, 1), _instance_3d_list(device),
      _instance_2d_list(device)
{
    NSLog(@"Picked Metal device %@", [_device name]);

    _layer = [CAMetalLayer layer];
    _layer.device = _device;
    _layer.pixelFormat = MTLPixelFormatBGRA8Unorm;
    _layer.presentsWithTransaction = false;
    _layer.displaySyncEnabled = false;
    _layer.maximumDrawableCount = 3;

    NSWindow *window = (__bridge NSWindow *)ns_window;
    window.contentView.wantsLayer = YES;
    window.contentView.layer = _layer;

    const auto scale_f = static_cast<float>(scale);
    _layer.drawableSize = CGSizeMake(static_cast<float>(width) * scale_f, static_cast<float>(height) * scale_f);

    _queue = [_device newCommandQueue];
    _sem = dispatch_semaphore_create(1);

    MTLCompileOptions *options = [[MTLCompileOptions alloc] init];
    options.fastMathEnabled = YES;
    options.languageVersion = MTLLanguageVersion2_3;

    dispatch_data_t data = dispatch_data_create(cpp_src_shaders_metallib, cpp_src_shaders_metallib_len, nil,
                                                DISPATCH_DATA_DESTRUCTOR_DEFAULT);
    NSError *err{nil};
    _library = [_device newLibraryWithData:data error:&err];
    MTL_ERROR(err);

    MTLRenderPipelineDescriptor *desc = [MTLRenderPipelineDescriptor new];
    desc.vertexFunction = [_library newFunctionWithName:@"triangle_vertex"];
    desc.fragmentFunction = [_library newFunctionWithName:@"triangle_fragment"];
    desc.depthAttachmentPixelFormat = MTLPixelFormatDepth32Float;
    desc.inputPrimitiveTopology = MTLPrimitiveTopologyClassTriangle;
    desc.rasterizationEnabled = YES;
    desc.rasterSampleCount = 1;
    desc.label = @"3D-Pipeline";
    desc.supportIndirectCommandBuffers = NO;

    desc.colorAttachments[0].pixelFormat = MTLPixelFormatBGRA8Unorm;
    desc.colorAttachments[0].blendingEnabled = NO;
    _state = [_device newRenderPipelineStateWithDescriptor:desc error:&err];
    MTL_ERROR(err);

    desc = [[MTLRenderPipelineDescriptor alloc] init];
    desc.vertexFunction = [_library newFunctionWithName:@"triangle_vertex_2d"];
    desc.fragmentFunction = [_library newFunctionWithName:@"triangle_fragment_2d"];
    desc.colorAttachments[0].pixelFormat = MTLPixelFormatBGRA8Unorm;
    desc.colorAttachments[0].blendingEnabled = YES;
    desc.colorAttachments[0].rgbBlendOperation = MTLBlendOperationAdd;
    desc.colorAttachments[0].alphaBlendOperation = MTLBlendOperationAdd;
    desc.colorAttachments[0].sourceRGBBlendFactor = MTLBlendFactorSourceAlpha;
    desc.colorAttachments[0].sourceAlphaBlendFactor = MTLBlendFactorSourceAlpha;
    desc.colorAttachments[0].destinationRGBBlendFactor = MTLBlendFactorOneMinusSourceAlpha;
    desc.colorAttachments[0].destinationAlphaBlendFactor = MTLBlendFactorZero;

    _state_2d = [_device newRenderPipelineStateWithDescriptor:desc error:&err];
    MTL_ERROR(err);

    MTLTextureDescriptor *tex_desc = [[MTLTextureDescriptor alloc] init];
    tex_desc.pixelFormat = MTLPixelFormatDepth32Float;
    tex_desc.width = static_cast<unsigned int>(static_cast<double>(width) * scale);
    tex_desc.height = static_cast<unsigned int>(static_cast<double>(height) * scale);
    tex_desc.depth = 1;
    tex_desc.textureType = MTLTextureType2D;
    tex_desc.storageMode = MTLStorageModePrivate;

    _depth_texture = [_device newTextureWithDescriptor:tex_desc];

    MTLDepthStencilDescriptor *depth_desc = [[MTLDepthStencilDescriptor alloc] init];
    depth_desc.depthCompareFunction = MTLCompareFunctionLess;
    depth_desc.depthWriteEnabled = YES;
    _depth_state = [_device newDepthStencilStateWithDescriptor:depth_desc];

    depth_desc.depthCompareFunction = MTLCompareFunctionAlways;
    depth_desc.depthWriteEnabled = NO;
    _depth_state_2d = [_device newDepthStencilStateWithDescriptor:depth_desc];
}

void MetalRenderer::set_2d_mesh(unsigned int id, MeshData2D data)
{
    if (_vertex_2d_list.has(id))
    {
        _vertex_2d_list.update_pointer(id, data.vertices, data.num_vertices);
    }
    else
    {
        _vertex_2d_list.add_pointer(id, data.vertices, data.num_vertices);
    }

    _flags |= Flags::Update2D;
}

void MetalRenderer::set_2d_instances(unsigned int id, InstancesData2D data)
{
    if (_instance_2d_list.has(id))
    {
        _instance_2d_list.update_instances_list(id, reinterpret_cast<const mat4 *>(data.matrices), data.num_matrices);
    }
    else
    {
        _instance_2d_list.add_instances_list(id, reinterpret_cast<const mat4 *>(data.matrices), data.num_matrices);
    }

    _flags |= Flags::UpdateInstances2D;
}

void MetalRenderer::set_3d_mesh(unsigned int id, MeshData3D data)
{
    if (_vertex_3d_list.has(id))
    {
        _vertex_3d_list.update_pointer(id, data.vertices, data.num_vertices, data.skin_data);
    }
    else
    {
        _vertex_3d_list.add_pointer(id, data.vertices, data.num_vertices, data.skin_data);
    }

    _flags |= Flags::Update3D;
}

void MetalRenderer::set_3d_instances(unsigned int id, InstancesData3D data)
{
    if (id >= _instance_3d_matrices.size())
        _instance_3d_matrices.resize(id + 1);

    if (!_instance_3d_matrices[id])
        _instance_3d_matrices[id] = std::make_shared<std::vector<Matrices>>();

    _instance_3d_matrices[id]->resize(data.num_matrices);
    for (unsigned int i = 0; i < data.num_matrices; i++)
    {
        std::memcpy(value_ptr(_instance_3d_matrices[id]->at(i).transform), &data.matrices[i], sizeof(mat4));
        _instance_3d_matrices[id]->at(i).normal_transform =
            transpose(inverse(_instance_3d_matrices[id]->at(i).transform));
    }

    if (_instance_3d_list.has(id))
    {
        _instance_3d_list.update_instances_list(id, _instance_3d_matrices[id]->data(),
                                                static_cast<unsigned int>(_instance_3d_matrices[id]->size()));
    }
    else
    {
        _instance_3d_list.add_instances_list(id, _instance_3d_matrices[id]->data(),
                                             static_cast<unsigned int>(_instance_3d_matrices[id]->size()));
    }

    _flags |= Flags::UpdateInstances3D;
}

void MetalRenderer::unload_3d_meshes(const unsigned int *ids, unsigned int num)
{
    for (size_t i = 0; i < num; i++)
    {
        const unsigned int id = ids[i];
        _vertex_3d_list.remove_pointer(id);
        _instance_3d_matrices[id]->clear();
        _instance_3d_list.remove_instances_list(id);
    }
}

void MetalRenderer::set_materials(const DeviceMaterial *materials, unsigned int num_materials)
{
    if (num_materials > _materials.size())
        _materials = Buffer<DeviceMaterial>(_device, num_materials);

    memcpy(_materials.data(), materials, num_materials * sizeof(DeviceMaterial));
    _materials.update();

    _flags |= Flags::UpdateMaterials;
}

MTLArgumentDescriptor *argumentDescriptorWithIndex(NSUInteger index, MTLDataType dataType)
{
    MTLArgumentDescriptor *argumentDescriptor = [MTLArgumentDescriptor argumentDescriptor];
    argumentDescriptor.index = index;
    argumentDescriptor.dataType = dataType;
    argumentDescriptor.access = MTLArgumentAccessReadOnly;
    return argumentDescriptor;
}

void MetalRenderer::synchronize()
{
    dispatch_semaphore_wait(_sem, DISPATCH_TIME_FOREVER);

    if (_flags & Flags::Update3D)
    {
        _vertex_3d_list.update_ranges();
        _vertex_3d_list.update_data(_device);
    }

    if (_flags & Flags::UpdateInstances3D)
    {
        _instance_3d_list.update_ranges();
        _instance_3d_list.update_data(_device);
    }

    if (_flags & Flags::Update2D)
    {
        _vertex_2d_list.update_ranges();
        _vertex_2d_list.update_data(_device);
    }

    if (_flags & Flags::UpdateInstances2D)
    {
        _instance_2d_list.update_ranges();
        _instance_2d_list.update_data(_device);
    }

    if (_flags != Flags::None)
    {
        MTLArgumentDescriptor *verticesArgument = argumentDescriptorWithIndex(VERTICES_ARG_INDEX, MTLDataTypePointer);
        MTLArgumentDescriptor *vertices2dArgument =
            argumentDescriptorWithIndex(VERTICES_2D_ARG_INDEX, MTLDataTypePointer);
        MTLArgumentDescriptor *texturesArgument = argumentDescriptorWithIndex(TEXTURES_ARG_INDEX, MTLDataTypePointer);
        MTLArgumentDescriptor *materialsArgument = argumentDescriptorWithIndex(MATERIALS_ARG_INDEX, MTLDataTypePointer);
        MTLArgumentDescriptor *instancesArgument = argumentDescriptorWithIndex(INSTANCES_ARG_INDEX, MTLDataTypePointer);
        MTLArgumentDescriptor *instances2dArgument =
            argumentDescriptorWithIndex(INSTANCES_2D_ARG_INDEX, MTLDataTypePointer);

        MTLArgumentDescriptor *textureArgument = argumentDescriptorWithIndex(0, MTLDataTypeTexture);
        id<MTLArgumentEncoder> textureEncoder = [_device newArgumentEncoderWithArguments:@[ textureArgument ]];

        id<MTLArgumentEncoder> sceneEncoder = [_device newArgumentEncoderWithArguments:@[
            verticesArgument, vertices2dArgument, texturesArgument, materialsArgument, instancesArgument,
            instances2dArgument
        ]];

        _args_buffer = [_device newBufferWithLength:sceneEncoder.encodedLength options:0];
        _textures_buffer = [_device newBufferWithLength:textureEncoder.encodedLength * _textures.size() options:0];

        for (size_t i = 0; i < _textures.size(); i++)
        {
            [textureEncoder setArgumentBuffer:_textures_buffer offset:textureEncoder.encodedLength * i];
            [textureEncoder setTexture:_textures[i] atIndex:0];
        }

        [_textures_buffer didModifyRange:NSMakeRange(0, _textures_buffer.length)];

        [sceneEncoder setArgumentBuffer:_args_buffer offset:0];
        [sceneEncoder setBuffer:_vertex_3d_list.vertex_buffer() offset:0 atIndex:VERTICES_ARG_INDEX];
        [sceneEncoder setBuffer:_vertex_2d_list.vertex_buffer() offset:0 atIndex:VERTICES_2D_ARG_INDEX];
        [sceneEncoder setBuffer:_textures_buffer offset:0 atIndex:TEXTURES_ARG_INDEX];
        [sceneEncoder setBuffer:_materials.buffer() offset:0 atIndex:MATERIALS_ARG_INDEX];
        [sceneEncoder setBuffer:_instance_3d_list.buffer() offset:0 atIndex:INSTANCES_ARG_INDEX];
        [sceneEncoder setBuffer:_instance_2d_list.buffer() offset:0 atIndex:INSTANCES_2D_ARG_INDEX];
        [_args_buffer didModifyRange:NSMakeRange(0, _args_buffer.length)];
    }

    _flags = Flags::None;
    dispatch_semaphore_signal(_sem);
}

void MetalRenderer::render(mat4 matrix_2d, CameraView3D view_3d)
{
    if (_args_buffer == nil)
        return;

    dispatch_semaphore_wait(_sem, DISPATCH_TIME_FOREVER);

    auto *uniforms = reinterpret_cast<Uniforms *>(_uniforms.data());
    if (uniforms)
    {
        const mat4 projection = get_rh_projection_matrix(view_3d);
        const mat4 view = get_rh_view_matrix(view_3d);
        memcpy(&uniforms->projection, value_ptr(projection), sizeof(mat4));
        memcpy(&uniforms->view_matrix, value_ptr(view), sizeof(mat4));
        const mat4 combined = projection * view;
        memcpy(&uniforms->combined, value_ptr(combined), sizeof(mat4));
        memcpy(&uniforms->matrix_2d, value_ptr(matrix_2d), sizeof(mat4));
        uniforms->view = view_3d;
    }
    _uniforms.update();

    id<CAMetalDrawable> drawable = [_layer nextDrawable];
    if (!drawable)
        return;

    MTLRenderPassDescriptor *render_desc = [[MTLRenderPassDescriptor alloc] init];

    render_desc.depthAttachment.clearDepth = 1.0;
    render_desc.depthAttachment.storeAction = MTLStoreActionStore;
    render_desc.depthAttachment.loadAction = MTLLoadActionClear;
    render_desc.depthAttachment.texture = _depth_texture;

    render_desc.colorAttachments[0].texture = drawable.texture;
    render_desc.colorAttachments[0].loadAction = MTLLoadActionClear;
    render_desc.colorAttachments[0].storeAction = MTLStoreActionStore;
    render_desc.colorAttachments[0].clearColor = MTLClearColorMake(0.0, 0.0, 0.0, 1.0);

    id<MTLCommandBuffer> command_buffer = [_queue commandBuffer];
    __block dispatch_semaphore_t semaphore = _sem;
    [command_buffer addCompletedHandler:^(id<MTLCommandBuffer>) {
      dispatch_semaphore_signal(semaphore);
    }];

    {
        id<MTLRenderCommandEncoder> encoder = [command_buffer renderCommandEncoderWithDescriptor:render_desc];

        [encoder setRenderPipelineState:_state];
        [encoder setDepthStencilState:_depth_state];
        [encoder setFrontFacingWinding:MTLWindingCounterClockwise];
        [encoder setTriangleFillMode:MTLTriangleFillModeFill];
        [encoder setCullMode:MTLCullModeBack];

        for (const auto &tex : _textures)
            [encoder useResource:tex usage:MTLResourceUsageRead];

        [encoder useResource:_vertex_3d_list.vertex_buffer() usage:MTLResourceUsageRead];
        [encoder useResource:_vertex_2d_list.vertex_buffer() usage:MTLResourceUsageRead];
        [encoder useResource:_textures_buffer usage:MTLResourceUsageRead];
        [encoder useResource:_materials.buffer() usage:MTLResourceUsageRead];
        [encoder useResource:_instance_3d_list.buffer() usage:MTLResourceUsageRead];
        [encoder useResource:_instance_2d_list.buffer() usage:MTLResourceUsageRead];

        [encoder setVertexBuffer:_args_buffer offset:0 atIndex:0];
        [encoder setVertexBuffer:_uniforms.buffer() offset:0 atIndex:1];
        [encoder setFragmentBuffer:_args_buffer offset:0 atIndex:0];

        const std::map<unsigned int, DrawDescriptor> &draw_ranges = _vertex_3d_list.get_draw_ranges();
        const std::map<unsigned int, InstanceRange<Matrices>> &instances = _instance_3d_list.get_ranges();

        for (const auto &[i, range] : draw_ranges)
        {
            const auto insts = instances.find(i);
            if (insts == instances.end() || insts->second.count == 0 || range.start >= range.end)
                continue;

            [encoder drawPrimitives:MTLPrimitiveTypeTriangle
                        vertexStart:range.start
                        vertexCount:(range.end - range.start)
                      instanceCount:insts->second.count
                       baseInstance:insts->second.start];
        }

        const std::map<unsigned int, DrawDescriptor> &ranges_2d = _vertex_2d_list.get_draw_ranges();
        const std::map<unsigned int, InstanceRange<mat4>> &instances_2d = _instance_2d_list.get_ranges();

        [encoder setRenderPipelineState:_state_2d];
        [encoder setDepthStencilState:_depth_state_2d];
        [encoder setFrontFacingWinding:MTLWindingCounterClockwise];
        [encoder setTriangleFillMode:MTLTriangleFillModeFill];
        [encoder setCullMode:MTLCullModeNone];
        [encoder setVertexBuffer:_args_buffer offset:0 atIndex:0];
        [encoder setVertexBuffer:_uniforms.buffer() offset:0 atIndex:1];
        [encoder setFragmentBuffer:_args_buffer offset:0 atIndex:0];

        for (const auto &[i, range] : ranges_2d)
        {
            const auto insts = instances_2d.find(i);
            if (insts == instances_2d.end() || insts->second.count == 0 || range.start >= range.end)
                continue;

            [encoder drawPrimitives:MTLPrimitiveTypeTriangle
                        vertexStart:range.start
                        vertexCount:(range.end - range.start)
                      instanceCount:insts->second.count
                       baseInstance:insts->second.start];
        }

        [encoder endEncoding];
    }

    [command_buffer presentDrawable:drawable];
    [command_buffer commit];
}

void MetalRenderer::resize(unsigned int width, unsigned int height, double scale)
{
    const auto scale_f = static_cast<float>(scale);
    const CGSize size = CGSizeMake(static_cast<float>(width) * scale_f, static_cast<float>(height) * scale_f);

    [_layer setDrawableSize:size];

    MTLTextureDescriptor *tex_desc = [[MTLTextureDescriptor alloc] init];
    tex_desc.pixelFormat = MTLPixelFormatDepth32Float;
    tex_desc.width = static_cast<unsigned int>(static_cast<double>(width) * scale);
    tex_desc.height = static_cast<unsigned int>(static_cast<double>(height) * scale);
    tex_desc.depth = 1;
    tex_desc.textureType = MTLTextureType2D;
    tex_desc.storageMode = MTLStorageModePrivate;

    _depth_texture = [_device newTextureWithDescriptor:tex_desc];
}

void mip_level_width_height(const TextureData &d, unsigned int level, unsigned int *width, unsigned int *height)
{
    unsigned int w = d.width;
    unsigned int h = d.height;
    if (level == 0)
    {
        if (width)
            *width = w;
        if (height)
            *height = h;
        return;
    }

    for (unsigned int i = 0; i < level; i++)
    {
        w >>= 1;
        h >>= 1;
    }

    if (width)
        *width = w;
    if (height)
        *height = h;
}

unsigned int mip_offset(const TextureData &d, unsigned int level)
{
    unsigned int w = d.width;
    unsigned int h = d.height;
    if (level == 0)
        return 0;

    unsigned int offset = 0;
    for (unsigned int i = 0; i < level; i++)
    {
        offset += w * h;
        w >>= 1;
        h >>= 1;
    }

    return offset;
}

void MetalRenderer::set_textures(const TextureData *data, unsigned int num_textures, const unsigned int *changed)
{
    if (_textures.empty() || _textures.size() != num_textures)
    {
        for (auto &_texture : _textures)
            _texture = nil;
        _textures.clear();

        for (size_t i = 0; i < num_textures; i++)
        {
            MTLTextureDescriptor *desc = [[MTLTextureDescriptor alloc] init];
            const TextureData &d = data[i];
            desc.width = d.width;
            desc.height = d.height;
            desc.pixelFormat = MTLPixelFormatBGRA8Unorm;
            desc.mipmapLevelCount = d.mip_levels;
            desc.sampleCount = 1;
            desc.storageMode = MTLStorageModeManaged;
            desc.textureType = MTLTextureType2D;
            desc.usage = MTLTextureUsageShaderRead;

            id<MTLTexture> texture = [_device newTextureWithDescriptor:desc];

            for (unsigned int m = 0; m < d.mip_levels; m++)
            {
                unsigned int w = d.width;
                unsigned int h = d.height;
                mip_level_width_height(d, m, &w, &h);

                [texture replaceRegion:MTLRegionMake2D(0, 0, w, h)
                           mipmapLevel:m
                             withBytes:d.bytes + mip_offset(d, m)
                           bytesPerRow:w * sizeof(unsigned int)];
            }

            _textures.push_back(texture);
        }
    }
    else
    {
        for (size_t i = 0; i < num_textures; i++)
        {
            if (changed[i] != 1)
                continue;

            const TextureData &d = data[i];
            id<MTLTexture> texture = _textures[i];

            MTLTextureDescriptor *desc = [[MTLTextureDescriptor alloc] init];
            if (d.width != texture.width || d.height != texture.height)
            {
                desc.width = d.width;
                desc.height = d.height;
                desc.pixelFormat = MTLPixelFormatBGRA8Unorm;
                desc.mipmapLevelCount = d.mip_levels;
                desc.sampleCount = 1;
                desc.storageMode = MTLStorageModeManaged;
                desc.textureType = MTLTextureType2D;
                desc.usage = MTLTextureUsageShaderRead;

                texture = [_device newTextureWithDescriptor:desc];

                for (unsigned int m = 0; m < d.mip_levels; m++)
                {
                    unsigned int w = d.width;
                    unsigned int h = d.height;
                    mip_level_width_height(d, m, &w, &h);

                    [texture replaceRegion:MTLRegionMake2D(0, 0, w, h)
                               mipmapLevel:m
                                 withBytes:d.bytes + mip_offset(d, m)
                               bytesPerRow:w * sizeof(unsigned int)];
                }

                _flags |= UpdateTextures;
                _textures[i] = texture;
            }
            else
            {
                const unsigned char *bytes = d.bytes;
                for (unsigned int m = 0; m < d.mip_levels; m++)
                {
                    unsigned int w = d.width;
                    unsigned int h = d.height;
                    mip_level_width_height(d, m, &w, &h);

                    [texture replaceRegion:MTLRegionMake2D(0, 0, w, h)
                               mipmapLevel:m
                                 withBytes:d.bytes + mip_offset(d, m)
                               bytesPerRow:w * sizeof(unsigned int)];
                    bytes += w * h;
                }
            }
        }
    }

    _flags |= Flags::UpdateTextures;
}
