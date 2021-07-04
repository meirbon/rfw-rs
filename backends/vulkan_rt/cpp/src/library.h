#ifndef CPP_LIBRARY_H
#define CPP_LIBRARY_H

#ifndef API
#define API
#endif

#include "structs.h"

#if WINDOWS
API void *create_instance(void *hwnd, void *hinstance, unsigned int width, unsigned int height, double scale);
#endif

API void destroy_instance(void *instance);

API void set_2d_mesh(void *instance, unsigned int id, MeshData2D data);
API void set_2d_instances(void *instance, unsigned int id, InstancesData2D data);

API void set_3d_mesh(void *instance, unsigned int id, MeshData3D data);
API void unload_3d_meshes(void *instance, const unsigned int *ids, unsigned int num);
API void set_3d_instances(void *instance, unsigned int id, InstancesData3D data);

API void set_materials(void *instance, const DeviceMaterial *materials, unsigned int num_materials);
API void set_textures(void *instance, const TextureData *data, unsigned int num_textures, const unsigned int *changed);

API void render(void *instance, Vector4x4 matrix_2d, CameraView3D view_3d);
API void synchronize(void *instance);

API void resize(void *instance, unsigned int width, unsigned int height, double scale_factor);
#endif // CPP_LIBRARY_H
