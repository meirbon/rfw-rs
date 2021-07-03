#define API extern "C"

#include "library.h"
#include "renderer.hpp"
#import <string.h>

extern "C" void *create_instance(void *ns_window, void *ns_view, unsigned int width, unsigned int height,
                                 double scale_factor)
{
    return reinterpret_cast<void *>(MetalRenderer::create_instance(ns_window, ns_view, width, height, scale_factor));
}

extern "C" void destroy_instance(void *instance)
{
    delete reinterpret_cast<MetalRenderer *>(instance);
}

extern "C" void set_2d_mesh(void *instance, unsigned int id, MeshData2D data)
{
    MetalRenderer *renderer = reinterpret_cast<MetalRenderer *>(instance);
    renderer->set_2d_mesh(id, data);
}
extern "C" void set_2d_instances(void *instance, unsigned int id, InstancesData2D data)
{
    MetalRenderer *renderer = reinterpret_cast<MetalRenderer *>(instance);
    renderer->set_2d_instances(id, data);
}

extern "C" void set_3d_mesh(void *instance, unsigned int id, MeshData3D data)
{
    MetalRenderer *renderer = reinterpret_cast<MetalRenderer *>(instance);
    renderer->set_3d_mesh(id, data);
}
extern "C" void unload_3d_meshes(void *instance, const unsigned int *ids, unsigned int num)
{
    MetalRenderer *renderer = reinterpret_cast<MetalRenderer *>(instance);
    renderer->unload_3d_meshes(ids, num);
}
extern "C" void set_3d_instances(void *instance, unsigned int id, InstancesData3D data)
{
    MetalRenderer *renderer = reinterpret_cast<MetalRenderer *>(instance);
    renderer->set_3d_instances(id, data);
}

extern "C" void set_materials(void *instance, const DeviceMaterial *materials, unsigned int num_materials)
{
    MetalRenderer *renderer = reinterpret_cast<MetalRenderer *>(instance);
    renderer->set_materials(materials, num_materials);
}
extern "C" void set_textures(void *instance, const TextureData *const data, unsigned int num_textures,
                             const unsigned int *changed)
{
    MetalRenderer *renderer = reinterpret_cast<MetalRenderer *>(instance);
    renderer->set_textures(data, num_textures, changed);
}

extern "C" void render(void *instance, simd_float4x4 matrix_2d, CameraView3D view_3d)
{
    glm::mat4 matrix;
    std::memcpy(glm::value_ptr(matrix), &matrix_2d, sizeof(glm::mat4));

    MetalRenderer *renderer = reinterpret_cast<MetalRenderer *>(instance);
    renderer->render(matrix, view_3d);
}

extern "C" void synchronize(void *instance)
{
    MetalRenderer *renderer = reinterpret_cast<MetalRenderer *>(instance);
    renderer->synchronize();
}

extern "C" void resize(void *instance, unsigned int width, unsigned int height, double scale_factor)
{
    MetalRenderer *renderer = reinterpret_cast<MetalRenderer *>(instance);
    renderer->resize(width, height, scale_factor);
}