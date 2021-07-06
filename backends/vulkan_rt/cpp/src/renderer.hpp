#ifndef RENDERER_HPP
#define RENDERER_HPP

#define GLM_FORCE_DEPTH_ZERO_TO_ONE
#include "vulkan_loader.h"

#include "structs.h"

#include <memory>
#include <vector>

#include <glm/ext.hpp>
#include <glm/glm.hpp>
vk::Result _CheckVK(vk::Result result, const char *command, const char *file, int line);
#define CheckVK(x) _CheckVK((x), #x, __FILE__, __LINE__)

class VulkanRenderer
{
  public:
	enum Flags : unsigned int
	{
		Empty = 0,
		Update3D = 1,
		UpdateInstances3D = 2,
		Update2D = 4,
		UpdateInstances2D = 8,
		UpdateMaterials = 16,
		UpdateTextures = 32
	};

	VulkanRenderer(const VulkanRenderer &other) = delete;
	~VulkanRenderer();

	static VulkanRenderer *create_instance(vk::UniqueInstance, vk::UniqueSurfaceKHR surface, unsigned int width,
										   unsigned int height, double scale);

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
	VulkanRenderer(vk::UniqueInstance, vk::UniqueSurfaceKHR surface, unsigned int width, unsigned int height,
				   double scale);

	vk::UniqueInstance _instance;
	vk::UniqueSurfaceKHR _surface;
	vk::UniqueDevice _device;
};
#endif