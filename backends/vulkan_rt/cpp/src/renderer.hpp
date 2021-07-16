#ifndef RENDERER_HPP
#define RENDERER_HPP

#define GLM_FORCE_SILENT_WARNINGS
#define GLM_FORCE_DEPTH_ZERO_TO_ONE
#include "vulkan_loader.h"

#include "structs.h"

#include <memory>
#include <utility>
#include <vector>

#include <glm/ext.hpp>
#include <glm/glm.hpp>
vk::Result _CheckVK(vk::Result result, const char *command, const char *file, int line);
template <typename T> T _CheckVK(vk::ResultValue<T> result, const char *command, const char *file, const int line)
{
	_CheckVK(result.result, command, file, line);
	return result.value;
}

#define CheckVK(x) _CheckVK((x), #x, __FILE__, __LINE__)

class VulkanRenderer
{
  public:
	struct SM
	{
		SM() = default;

		SM(vk::SharingMode mode, uint32_t indicesCount = 0, uint32_t *indices = nullptr) : sharingMode(mode)
		{
			if (indicesCount > 0 && indices)
			{
				familyIndices = std::vector<uint32_t>(indices, indices + indicesCount);
			}
		}

		SM(vk::SharingMode mode, std::vector<uint32_t> indices) : sharingMode(mode), familyIndices(std::move(indices))
		{
		}

		vk::SharingMode sharingMode = vk::SharingMode::eConcurrent;
		std::vector<uint32_t> familyIndices;
	};

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

	/**
	 * This function assumes _sharingModeUtil was filled correctly.
	 * @param fromOldSwapchain Whether to (re)create the swapchain from an old swapchain.
	 */
	void create_swapchain(unsigned int width, unsigned int height);

	vk::UniqueInstance _instance;
	vk::UniqueSurfaceKHR _surface;
	vk::UniqueDevice _device;
	vk::PhysicalDevice _physicalDevice;

	std::vector<uint32_t> _queueFamilyIndices;
	SM _sharingModeUtil;

	vk::UniqueSwapchainKHR _swapchain;
	std::vector<vk::Image> _swapchainImages;
	std::vector<vk::UniqueImageView> _swapchainImageViews;

	vk::UniqueCommandPool _commandPool;
	std::vector<vk::UniqueCommandBuffer> _commandBuffers;

	vk::Queue _graphicsQueue;
	vk::Queue _presentQueue;

	vk::UniquePipelineLayout _pipelineLayout;
	vk::UniquePipeline _pipeline;
	vk::UniqueShaderModule _vertModule;
	vk::UniqueShaderModule _fragModule;
	vk::UniqueRenderPass _renderPass;
	vk::UniqueSemaphore _imageAvailableSemaphore;
	vk::UniqueSemaphore _renderFinishedSemaphore;

	vk::Extent2D _extent;
	double _scale = 1.0;
	std::vector<vk::UniqueFramebuffer> _framebuffers;
};
#endif