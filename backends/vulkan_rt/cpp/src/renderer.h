#ifndef RENDERER_HPP
#define RENDERER_HPP

#define GLM_FORCE_SILENT_WARNINGS
#define GLM_FORCE_DEPTH_ZERO_TO_ONE
#include "vulkan_loader.h"

#include "structs.h"
#include "vkh/buffer.h"
#include "vkh/swapchain.h"

#include "instance_list.h"
#include "vertex_list.h"

#include <memory>
#include <utility>
#include <vector>

#include <glm/ext.hpp>
#include <glm/glm.hpp>

struct Uniforms
{
	glm::mat4 matrix_2d;
	glm::mat4 view;
	glm::mat4 projection;
	glm::mat4 combined;

	glm::vec4 cameraPosition;
	glm::vec4 cameraDirection;
};

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
		UpdateCommandBuffers = 1,
		Update3D = 2,
		UpdateInstances3D = 4,
		Update2D = 8,
		UpdateInstances2D = 16,
		UpdateMaterials = 32,
		UpdateTextures = 64
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
	void setup_framebuffers();
	void setup_pipelines();
	void update_descriptorsets();
	void record_commandbuffers();

  private:
	VulkanRenderer(vk::UniqueInstance, vk::UniqueSurfaceKHR surface, unsigned int width, unsigned int height,
				   double scale);

	SM _sharingModeUtil;
	vk::UniqueInstance _instance;
	vk::UniqueDevice _device;
	vk::PhysicalDevice _physicalDevice;

	std::vector<uint32_t> _queueFamilyIndices;
	std::unique_ptr<vkh::Swapchain> _swapchain;
	vk::Image _depthImage;
	VmaAllocation _depthImageAllocation;
	vk::UniqueImageView _depthImageView;

	vk::UniqueCommandPool _commandPool;
	std::vector<vk::UniqueCommandBuffer> _commandBuffers;
	std::vector<vk::UniqueFence> _inFlightFences;
	std::vector<vk::Fence> _imagesInFlight;

	vk::Queue _graphicsQueue;
	vk::Queue _presentQueue;

	VmaAllocator _allocator;
	VmaVulkanFunctions _allocatorFunctions;

	std::unique_ptr<VertexDataList<Vertex3D, JointData>> _vertexList3D;
	std::unique_ptr<VertexDataList<Vertex2D, int>> _vertexList2D;
	std::unique_ptr<InstanceDataList<glm::mat4>> _instanceList2D;
	std::unique_ptr<InstanceDataList<glm::mat4>> _instanceList3D;

	//	std::vector<vkh::Buffer<Vertex2D>> _meshes2D;
	//	std::vector<vkh::Buffer<glm::mat4>> _instances2D;
	//
	//	std::vector<vkh::Buffer<Vertex3D>> _meshes3D;
	//	std::vector<vkh::Buffer<glm::mat4>> _instances3D;

	vk::UniqueDescriptorPool _descriptorPool;
	vk::UniqueDescriptorSetLayout _descriptorLayout;
	//	vk::UniqueDescriptorUpdateTemplate _descriptorUpdateTemplate;
	std::vector<vk::UniqueDescriptorSet> _descriptorSets;

	vkh::Buffer<DeviceMaterial> _materials;
	std::vector<vkh::Buffer<Uniforms>> _uniformBuffers;

	vk::UniquePipelineLayout _pipelineLayout;
	vk::UniquePipeline _pipeline;
	vk::UniqueShaderModule _vertModule;
	vk::UniqueShaderModule _fragModule;
	vk::UniqueRenderPass _renderPass;

	std::vector<vk::UniqueSemaphore> _imageAvailableSemaphores;
	std::vector<vk::UniqueSemaphore> _renderFinishedSemaphores;
	size_t _currentFrame = 0;

	//	vk::Extent2D _extent;
	double _scale = 1.0;
	std::vector<vk::UniqueFramebuffer> _framebuffers;

	unsigned int _updateFlags;
};
#endif