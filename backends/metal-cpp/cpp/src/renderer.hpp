#ifndef RENDERER_HPP
#define RENDERER_HPP

#define GLM_FORCE_DEPTH_ZERO_TO_ONE

#import <Metal/Metal.h>
#import <MetalKit/MetalKit.h>

#import <QuartzCore/QuartzCore.h>
#import <simd/simd.h>

#import "buffer.hpp"
#include "instance_list.h"
#include "library.h"
#include "vertex_list.h"

#include <memory>
#include <vector>

#include <glm/ext.hpp>
#include <glm/glm.hpp>

struct Uniforms
{
    matrix_float4x4 projection;
    matrix_float4x4 view_matrix;
    matrix_float4x4 combined;
    matrix_float4x4 matrix_2d;
    CameraView3D view;
};

struct Matrices
{
    glm::mat4 transform;
    glm::mat4 normal_transform;
};

class MetalRenderer
{
  public:
    enum Flags : unsigned int
    {
        None = 0,
        Update3D = 1,
        UpdateInstances3D = 2,
        Update2D = 4,
        UpdateInstances2D = 8,
        UpdateMaterials = 16,
        UpdateTextures = 32
    };

    ~MetalRenderer();

    static MetalRenderer *create_instance(void *ns_window, void *ns_view, unsigned int width, unsigned int height,
                                          double scale);

    void set_2d_mesh(unsigned int id, MeshData2D data);
    void set_2d_instances(unsigned int id, InstancesData2D data);

    void set_3d_mesh(unsigned int id, MeshData3D data);
    void set_3d_instances(unsigned int id, InstancesData3D data);
    void unload_3d_meshes(const unsigned int *ids, unsigned int num);

    void set_materials(const DeviceMaterial *materials, unsigned int num_materials);
    void set_textures(const TextureData *data, unsigned int num_textures, const unsigned int *changed);

    void synchronize();
    void render(glm::mat4 matrix_2d, CameraView3D view_3d);

    void resize(unsigned int width, unsigned int height, double scale);

  private:
    MetalRenderer(id<MTLDevice> device, void *ns_window, void *ns_view, unsigned int width, unsigned int height,
                  double scale);

    id<MTLDevice> _device;
    id<MTLCommandQueue> _queue;
    CAMetalLayer *_layer;

    id<MTLLibrary> _library;
    dispatch_semaphore_t _sem;
    id<MTLRenderPipelineState> _state;
    id<MTLRenderPipelineState> _state_2d;

    id<MTLBuffer> _args_buffer = nil;
    id<MTLBuffer> _textures_buffer = nil;

    Buffer<Uniforms> _uniforms;
    Buffer<DeviceMaterial> _materials;
    Buffer<Uniforms> _camera;

    id<MTLTexture> _depth_texture;
    id<MTLDepthStencilState> _depth_state;
    id<MTLDepthStencilState> _depth_state_2d;

    VertexList<Vertex3D, JointData> _vertex_3d_list;
    VertexList<Vertex2D, unsigned int> _vertex_2d_list;

    //    vertex_2d_list: VertexList<Vertex2D>,
    //        mesh_2d_textures: Vec<Option<usize>>,
    //        instance_2d_list: InstanceList<Mat4>,

    std::vector<std::shared_ptr<std::vector<Matrices>>> _instance_3d_matrices;
    InstanceList<Matrices> _instance_3d_list;
    InstanceList<glm::mat4> _instance_2d_list;

    std::vector<id<MTLTexture>> _textures;

    unsigned int _flags = Flags::None;
};
#endif