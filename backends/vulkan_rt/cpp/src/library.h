#ifndef CPP_LIBRARY_H
#define CPP_LIBRARY_H

#ifndef API
#define API
#endif

#define XLIB_HANDLE 0
#define XCB_HANDLE 1
#define WAYLAND_HANDLE 2

#include "structs.h"

/**
 * @brief Create a instance object
 *
 * @param handle0 On Windows: hwnd
 * @param handle1 On Windows: hinstance
 * @param handle2
 * @param width
 * @param height
 * @param scale
 * @return API*
 */
API void *vulkan_create_instance(unsigned long long handle0, unsigned long long handle1, unsigned long long handle2,
						  unsigned int width, unsigned int height, double scale);

API void vulkan_destroy_instance(void *instance);

API void vulkan_set_2d_mesh(void *instance, unsigned int id, MeshData2D data);
API void vulkan_set_2d_instances(void *instance, unsigned int id, InstancesData2D data);

API void vulkan_set_3d_mesh(void *instance, unsigned int id, MeshData3D data);
API void vulkan_unload_3d_meshes(void *instance, const unsigned int *ids, unsigned int num);
API void vulkan_set_3d_instances(void *instance, unsigned int id, InstancesData3D data);

API void vulkan_set_materials(void *instance, const DeviceMaterial *materials, unsigned int num_materials);
API void vulkan_set_textures(void *instance, const TextureData *data, unsigned int num_textures, const unsigned int *changed);

API void vulkan_render(void *instance, Vector4x4 matrix_2d, CameraView3D view_3d);
API void vulkan_synchronize(void *instance);

API void vulkan_resize(void *instance, unsigned int width, unsigned int height, double scale_factor);
#endif // CPP_LIBRARY_H
