#include "renderer.hpp"

#include <cassert>
#include <cstring>
#include <filesystem>
#include <iostream>

#include <glm/ext.hpp>
#include <glm/glm.hpp>

#include "device.h"
#include "shaders.h"

#include "../../shaders/minimal.frag.spv.h"
#include "../../shaders/minimal.vert.spv.h"

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

VulkanRenderer *VulkanRenderer::create_instance(vk::UniqueInstance instance, vk::UniqueSurfaceKHR surface,
												unsigned int width, unsigned int height, double scale)
{
	assert(instance);
	assert(surface);
	return new VulkanRenderer(std::move(instance), std::move(surface), width, height, scale);
}

VulkanRenderer::~VulkanRenderer()
{
	try
	{
		_device->waitIdle();

		_vertexList3D.reset();
		_vertexList2D.reset();

		_materials.free();
		_uniformBuffers.clear();

		vmaDestroyAllocator(_allocator);
		_commandBuffers.clear();
	}
	catch (const std::exception &e)
	{
		std::cerr << "Exception occurred: " << e.what() << std::endl;
	}
}

VulkanRenderer::VulkanRenderer(vk::UniqueInstance instance, vk::UniqueSurfaceKHR surface, unsigned int width,
							   unsigned int height, double scale)
	: _instance(std::move(instance))
{
	std::cout << "Received Vulkan instance: " << _instance.get() << ", surface: " << surface.get() << std::endl;

	_physicalDevice = vkh::pickPhysicalDevice(_instance.get(), "NVIDIA");
	if (!_physicalDevice)
		_physicalDevice = vkh::pickPhysicalDevice(_instance.get(), "AMD");
	if (!_physicalDevice)
		_physicalDevice = vkh::pickPhysicalDevice(_instance.get(), "Intel");

	if (!_physicalDevice)
	{
		std::cerr << "Could not find a suitable Vulkan device.";
		exit(-1);
	}

	vk::PhysicalDeviceProperties deviceProperties = _physicalDevice.getProperties();
	std::cout << "Picked Vulkan device: " << deviceProperties.deviceName.data() << std::endl;

	uint32_t graphicsQueue = 0, presentQueue = 0;
	std::set<uint32_t> uniqueQueueFamilyIndices =
		vkh::findQueueFamilyIndices(_physicalDevice, *surface, &graphicsQueue, &presentQueue);
	std::vector<vk::DeviceQueueCreateInfo> queueCreateInfos;

	_queueFamilyIndices = std::vector<uint32_t>(uniqueQueueFamilyIndices.begin(), uniqueQueueFamilyIndices.end());
	queueCreateInfos.reserve(_queueFamilyIndices.size());

	const float queuePriority = 1.0f;
	for (uint32_t queueFamilyIndex : _queueFamilyIndices)
	{
		queueCreateInfos.push_back(vk::DeviceQueueCreateInfo({}, queueFamilyIndex, 1, &queuePriority));
	}

	std::vector<const char *> deviceExtensions = {VK_KHR_SWAPCHAIN_EXTENSION_NAME,
												  VK_KHR_GET_MEMORY_REQUIREMENTS_2_EXTENSION_NAME,
												  VK_KHR_BIND_MEMORY_2_EXTENSION_NAME};
#if MACOS || IOS
	deviceExtensions.push_back(VK_KHR_PORTABILITY_SUBSET_EXTENSION_NAME);
#endif

	_device = _physicalDevice.createDeviceUnique(vk::DeviceCreateInfo({}, queueCreateInfos, {}, deviceExtensions, {}));

	// Final initialization of dynamic dispatcher
	VULKAN_HPP_DEFAULT_DISPATCHER.init(_instance.get(), _device.get());

	uint32_t *familyIndices = !_queueFamilyIndices.empty() ? _queueFamilyIndices.data() : nullptr;

	SM sharingModeUtil = (graphicsQueue != presentQueue) ? SM(vk::SharingMode::eConcurrent, 2u, familyIndices)
														 : SM(vk::SharingMode::eExclusive);

	_graphicsQueue = _device->getQueue(graphicsQueue, 0);
	_presentQueue = _device->getQueue(presentQueue, 0);

	VmaAllocatorCreateInfo allocatorCreateInfo = {};
	allocatorCreateInfo.instance = *_instance;
	allocatorCreateInfo.physicalDevice = _physicalDevice;
	allocatorCreateInfo.device = *_device;
	allocatorCreateInfo.frameInUseCount = 2;
	allocatorCreateInfo.vulkanApiVersion = VK_API_VERSION_1_2;
	allocatorCreateInfo.pVulkanFunctions = &_allocatorFunctions;
	_allocatorFunctions.vkGetPhysicalDeviceProperties = vk::defaultDispatchLoaderDynamic.vkGetPhysicalDeviceProperties;
	_allocatorFunctions.vkGetPhysicalDeviceMemoryProperties =
		vk::defaultDispatchLoaderDynamic.vkGetPhysicalDeviceMemoryProperties;
	_allocatorFunctions.vkAllocateMemory = vk::defaultDispatchLoaderDynamic.vkAllocateMemory;
	_allocatorFunctions.vkFreeMemory = vk::defaultDispatchLoaderDynamic.vkFreeMemory;
	_allocatorFunctions.vkMapMemory = vk::defaultDispatchLoaderDynamic.vkMapMemory;
	_allocatorFunctions.vkUnmapMemory = vk::defaultDispatchLoaderDynamic.vkUnmapMemory;
	_allocatorFunctions.vkFlushMappedMemoryRanges = vk::defaultDispatchLoaderDynamic.vkFlushMappedMemoryRanges;
	_allocatorFunctions.vkInvalidateMappedMemoryRanges =
		vk::defaultDispatchLoaderDynamic.vkInvalidateMappedMemoryRanges;
	_allocatorFunctions.vkBindBufferMemory = vk::defaultDispatchLoaderDynamic.vkBindBufferMemory;
	_allocatorFunctions.vkBindImageMemory = vk::defaultDispatchLoaderDynamic.vkBindImageMemory;
	_allocatorFunctions.vkGetBufferMemoryRequirements = vk::defaultDispatchLoaderDynamic.vkGetBufferMemoryRequirements;
	_allocatorFunctions.vkGetImageMemoryRequirements = vk::defaultDispatchLoaderDynamic.vkGetImageMemoryRequirements;
	_allocatorFunctions.vkCreateBuffer = vk::defaultDispatchLoaderDynamic.vkCreateBuffer;
	_allocatorFunctions.vkDestroyBuffer = vk::defaultDispatchLoaderDynamic.vkDestroyBuffer;
	_allocatorFunctions.vkCreateImage = vk::defaultDispatchLoaderDynamic.vkCreateImage;
	_allocatorFunctions.vkDestroyImage = vk::defaultDispatchLoaderDynamic.vkDestroyImage;
	_allocatorFunctions.vkCmdCopyBuffer = vk::defaultDispatchLoaderDynamic.vkCmdCopyBuffer;
#if VMA_DEDICATED_ALLOCATION || VMA_VULKAN_VERSION >= 1001000
	_allocatorFunctions.vkGetBufferMemoryRequirements2KHR =
		vk::defaultDispatchLoaderDynamic.vkGetBufferMemoryRequirements2KHR;
	_allocatorFunctions.vkGetImageMemoryRequirements2KHR =
		vk::defaultDispatchLoaderDynamic.vkGetImageMemoryRequirements2KHR;
#endif
#if VMA_BIND_MEMORY2 || VMA_VULKAN_VERSION >= 1001000
	_allocatorFunctions.vkBindBufferMemory2KHR = vk::defaultDispatchLoaderDynamic.vkBindBufferMemory2KHR;
	_allocatorFunctions.vkBindImageMemory2KHR = vk::defaultDispatchLoaderDynamic.vkBindImageMemory2KHR;
#endif
#if VMA_MEMORY_BUDGET || VMA_VULKAN_VERSION >= 1001000
	_allocatorFunctions.vkGetPhysicalDeviceMemoryProperties2KHR =
		vk::defaultDispatchLoaderDynamic.vkGetPhysicalDeviceMemoryProperties2KHR;
#endif

	vmaCreateAllocator(&allocatorCreateInfo, &_allocator);

	vk::ShaderModuleCreateInfo vertShaderCreateInfo = vk::ShaderModuleCreateInfo(
		{}, __shaders_minimal_vert_spv_len, reinterpret_cast<const uint32_t *>(__shaders_minimal_vert_spv));
	_vertModule = _device->createShaderModuleUnique(vertShaderCreateInfo);

	vk::ShaderModuleCreateInfo fragShaderCreateInfo = vk::ShaderModuleCreateInfo(
		{}, __shaders_minimal_frag_spv_len, reinterpret_cast<const uint32_t *>(__shaders_minimal_frag_spv));
	_fragModule = _device->createShaderModuleUnique(fragShaderCreateInfo);

	_commandPool = _device->createCommandPoolUnique({{}, static_cast<uint32_t>(graphicsQueue)});

	_swapchain = vkh::Swapchain::create(
		_device.get(), _physicalDevice, std::move(surface), vk::Format::eB8G8R8A8Unorm, 2, sharingModeUtil.sharingMode,
		sharingModeUtil.familyIndices,
		vk::ImageViewCreateInfo({}, {}, vk::ImageViewType::e2D, {}, {},
								vk::ImageSubresourceRange(vk::ImageAspectFlagBits::eColor, 0, 1, 0, 1)),
		width, height);

	resize(width, height, scale);

	_vertexList2D = std::make_unique<VertexDataList<Vertex2D, int>>(_allocator);
	_vertexList3D = std::make_unique<VertexDataList<Vertex3D, JointData>>(_allocator);

	_instanceList2D = std::make_unique<InstanceDataList<glm::mat4>>(_allocator);
	_instanceList3D = std::make_unique<InstanceDataList<glm::mat4>>(_allocator);

	_materials = vkh::Buffer<DeviceMaterial>(_allocator);
}

void VulkanRenderer::set_2d_mesh(unsigned int id, MeshData2D data)
{
	if (_vertexList2D->has(id))
		_vertexList2D->update_pointer(id, data.vertices, data.num_vertices);
	else
		_vertexList2D->add_pointer(id, data.vertices, data.num_vertices);

	_updateFlags |= Flags::Update2D;
}

void VulkanRenderer::set_2d_instances(unsigned int id, InstancesData2D data)
{
	if (_instanceList2D->has(id))
		_instanceList2D->update_instances_list(id, reinterpret_cast<const mat4 *>(data.matrices), data.num_matrices);
	else
		_instanceList2D->add_instances_list(id, reinterpret_cast<const mat4 *>(data.matrices), data.num_matrices);

	_updateFlags |= Flags::UpdateInstances2D;
}

void VulkanRenderer::set_3d_mesh(unsigned int id, MeshData3D data)
{
	if (_vertexList3D->has(id))
		_vertexList3D->update_pointer(id, data.vertices, data.num_vertices, data.skin_data);
	else
		_vertexList3D->add_pointer(id, data.vertices, data.num_vertices, data.skin_data);

	_updateFlags |= Flags::Update3D;
}

void VulkanRenderer::set_3d_instances(unsigned int id, InstancesData3D data)
{
	if (_instanceList3D->has(id))
		_instanceList3D->update_instances_list(id, reinterpret_cast<const mat4 *>(data.matrices), data.num_matrices);
	else
		_instanceList3D->add_instances_list(id, reinterpret_cast<const mat4 *>(data.matrices), data.num_matrices);

	_updateFlags |= Flags::UpdateInstances2D;
}

void VulkanRenderer::unload_3d_meshes(const unsigned int *ids, unsigned int num)
{
	for (size_t i = 0; i < num; i++)
	{
		const unsigned int id = ids[i];
		_vertexList3D->remove_pointer(id);
		_instanceList3D->remove_instances_list(id);
	}

	_updateFlags |= Flags::UpdateCommandBuffers;
}

void VulkanRenderer::set_materials(const DeviceMaterial *materials, unsigned int num_materials)
{
	_materials.set_data(_allocator, vk::BufferUsageFlagBits::eStorageBuffer,
						vk::MemoryPropertyFlagBits::eDeviceLocal | vk::MemoryPropertyFlagBits::eHostVisible,
						VMA_MEMORY_USAGE_GPU_ONLY, materials, num_materials);
	_updateFlags |= Flags::UpdateMaterials;
}

void VulkanRenderer::set_textures(const TextureData * /*data*/, unsigned int /*num_textures*/,
								  const unsigned int * /*changed*/)
{
	_updateFlags |= Flags::UpdateTextures;
}

void VulkanRenderer::synchronize()
{
	if (_updateFlags & Flags::Update3D)
	{
		_vertexList3D->update_ranges();
		_vertexList3D->update_data();
	}

	if (_updateFlags & Flags::Update2D)
	{
		_vertexList2D->update_ranges();
		_vertexList2D->update_data();
	}

	if (_updateFlags & Flags::UpdateInstances2D)
	{
		_instanceList2D->update_ranges();
		_instanceList2D->update_data();
	}

	if (_updateFlags & Flags::UpdateInstances3D)
	{
		_instanceList3D->update_ranges();
		_instanceList3D->update_data();
	}

	_updateFlags = Flags::Empty;

	VmaStats stats = {};
	vmaCalculateStats(_allocator, &stats);

	//	fmt::print("Allocations: (count: {}, usedBytes: {}, unusedBytes: {})\n", stats.total.allocationCount,
	//			   stats.total.usedBytes, stats.total.unusedBytes);
}

void VulkanRenderer::render(const mat4 matrix_2d, const CameraView3D view_3d)
{
	const vk::ResultValue<uint32_t> value =
		_swapchain->acquire_next_image(std::numeric_limits<uint64_t>::max(), *_imageAvailableSemaphores[_currentFrame]);
	if (value.result == vk::Result::eErrorOutOfDateKHR)
		return;
	const uint32_t imageIndex = value.value;

	if (_imagesInFlight[imageIndex])
		CheckVK(_device->waitForFences(1, &_imagesInFlight[imageIndex], 1, std::numeric_limits<uint64_t>::max()));

	_imagesInFlight[imageIndex] = _inFlightFences[_currentFrame].get();

	// Update uniform data
	Uniforms data = {};
	data.matrix_2d = matrix_2d;
	data.view = get_rh_view_matrix(view_3d);
	data.projection = get_rh_projection_matrix(view_3d);
	data.combined = get_rh_matrix(view_3d);
	data.cameraPosition = glm::vec4(view_3d.pos.x, view_3d.pos.y, view_3d.pos.z, 1.0f);
	data.cameraDirection = glm::vec4(view_3d.direction.x, view_3d.direction.y, view_3d.direction.z, 1.0f);
	_uniformBuffers[imageIndex].set_data(&data, 1);

	// Wait till previous frame finished presenting
	vk::PipelineStageFlags waitStageMask = vk::PipelineStageFlagBits::eColorAttachmentOutput;
	vk::SubmitInfo submitInfo =
		vk::SubmitInfo(1, &*_imageAvailableSemaphores[_currentFrame], &waitStageMask, 1, &*_commandBuffers[imageIndex],
					   1, &*_renderFinishedSemaphores[_currentFrame]);

	// Reset fence for this frame
	CheckVK(_device->resetFences(1, &_imagesInFlight[imageIndex]));
	if (_graphicsQueue.submit(1, &submitInfo, _imagesInFlight[imageIndex]) != vk::Result::eSuccess)
		return;

	// Present image
	vk::PresentInfoKHR presentInfoKhr =
		vk::PresentInfoKHR(1, &*_renderFinishedSemaphores[_currentFrame], 1, &_swapchain->get(), &imageIndex);
	if (_presentQueue.presentKHR(&presentInfoKhr) != vk::Result::eSuccess)
		return;

	_currentFrame = (_currentFrame + 1) % _imageAvailableSemaphores.size();
}

void VulkanRenderer::resize(unsigned int width, unsigned int height, double scale)
{
	// Wait till device is idle, a command buffer might still be running
	_device->waitIdle();
	_scale = scale;

	// (Re)create swap chain
	_swapchain->resize(width, height);

	// Reinitialize pipelines
	vk::PipelineShaderStageCreateInfo vertShaderStageInfo =
		vk::PipelineShaderStageCreateInfo{{}, vk::ShaderStageFlagBits::eVertex, *_vertModule, "main"};

	vk::PipelineShaderStageCreateInfo fragShaderStageInfo = {
		{}, vk::ShaderStageFlagBits::eFragment, *_fragModule, "main"};

	std::vector<vk::PipelineShaderStageCreateInfo> pipelineShaderStages = {vertShaderStageInfo, fragShaderStageInfo};

	auto vertexInputInfo = vk::PipelineVertexInputStateCreateInfo{{}, 0u, nullptr, 0u, nullptr};
	auto inputAssembly = vk::PipelineInputAssemblyStateCreateInfo{{}, vk::PrimitiveTopology::eTriangleList, false};
	const vk::Viewport viewport = _swapchain->viewport();
	const vk::Rect2D scissor = vk::Rect2D{{0, 0}, _swapchain->extent()};
	auto viewportState = vk::PipelineViewportStateCreateInfo{{}, 1, &viewport, 1, &scissor};
	auto rasterizer = vk::PipelineRasterizationStateCreateInfo{{},
															   /*depthClamp*/ false,
															   /*rasterizeDiscard*/ false,
															   vk::PolygonMode::eFill,
															   {},
															   /*frontFace*/ vk::FrontFace::eCounterClockwise,
															   {},
															   {},
															   {},
															   {},
															   1.0f};

	auto multisampling = vk::PipelineMultisampleStateCreateInfo{{}, vk::SampleCountFlagBits::e1, false, 1.0};

	auto colorBlendAttachment =
		vk::PipelineColorBlendAttachmentState{{},
											  /*srcCol*/ vk::BlendFactor::eOne,
											  /*dstCol*/ vk::BlendFactor::eZero,
											  /*colBlend*/ vk::BlendOp::eAdd,
											  /*srcAlpha*/ vk::BlendFactor::eOne,
											  /*dstAlpha*/ vk::BlendFactor::eZero,
											  /*alphaBlend*/ vk::BlendOp::eAdd,
											  vk::ColorComponentFlagBits::eR | vk::ColorComponentFlagBits::eG |
												  vk::ColorComponentFlagBits::eB | vk::ColorComponentFlagBits::eA};

	auto colorBlending = vk::PipelineColorBlendStateCreateInfo{{},
															   /*logicOpEnable=*/false,
															   vk::LogicOp::eCopy,
															   /*attachmentCount=*/1,
															   /*colourAttachments=*/&colorBlendAttachment};

	if (!_pipelineLayout)
		_pipelineLayout = _device->createPipelineLayoutUnique({}, nullptr);

	auto colorAttachment = vk::AttachmentDescription{{},
													 _swapchain->format(),
													 vk::SampleCountFlagBits::e1,
													 vk::AttachmentLoadOp::eClear,
													 vk::AttachmentStoreOp::eStore,
													 {},
													 {},
													 {},
													 vk::ImageLayout::ePresentSrcKHR};

	auto colourAttachmentRef = vk::AttachmentReference{0, vk::ImageLayout::eColorAttachmentOptimal};

	auto subpass = vk::SubpassDescription{{},
										  vk::PipelineBindPoint::eGraphics,
										  /*inAttachmentCount*/ 0,
										  nullptr,
										  1,
										  &colourAttachmentRef};

	if (!_renderPass)
	{
		auto subpassDependency =
			vk::SubpassDependency{VK_SUBPASS_EXTERNAL,
								  0,
								  vk::PipelineStageFlagBits::eColorAttachmentOutput,
								  vk::PipelineStageFlagBits::eColorAttachmentOutput,
								  {},
								  vk::AccessFlagBits::eColorAttachmentRead | vk::AccessFlagBits::eColorAttachmentWrite};
		_renderPass = _device->createRenderPassUnique(
			vk::RenderPassCreateInfo{{}, 1, &colorAttachment, 1, &subpass, 1, &subpassDependency});
	}

	// Need to (re)create pipeline if viewport changes
	vk::GraphicsPipelineCreateInfo pipelineCreateInfo = vk::GraphicsPipelineCreateInfo{{},
																					   2,
																					   pipelineShaderStages.data(),
																					   &vertexInputInfo,
																					   &inputAssembly,
																					   nullptr,
																					   &viewportState,
																					   &rasterizer,
																					   &multisampling,
																					   nullptr,
																					   &colorBlending,
																					   nullptr,
																					   *_pipelineLayout,
																					   *_renderPass,
																					   0};

	_pipeline = _device->createGraphicsPipelineUnique({}, pipelineCreateInfo).value;

	_framebuffers = std::vector<vk::UniqueFramebuffer>(_swapchain->size());
	for (size_t i = 0; i < _swapchain->size(); i++)
	{
		vk::ImageView imageView = _swapchain->image_view_at(static_cast<uint32_t>(i));
		_framebuffers[i] = _device->createFramebufferUnique(
			vk::FramebufferCreateInfo{{}, *_renderPass, 1, &imageView, _swapchain->width(), _swapchain->height(), 1});
	}

	_commandBuffers = _device->allocateCommandBuffersUnique(vk::CommandBufferAllocateInfo(
		_commandPool.get(), vk::CommandBufferLevel::ePrimary, static_cast<uint32_t>(_framebuffers.size())));

	_inFlightFences.clear();
	_inFlightFences.resize(_commandBuffers.size());

	_imagesInFlight.clear();
	_imagesInFlight.resize(_commandBuffers.size(), VK_NULL_HANDLE);

	vk::SemaphoreCreateInfo semaphoreCreateInfo = {};
	_imageAvailableSemaphores.clear();
	_imageAvailableSemaphores.resize(_commandBuffers.size());
	_renderFinishedSemaphores.clear();
	_renderFinishedSemaphores.resize(_commandBuffers.size());

	if (_uniformBuffers.size() != _commandBuffers.size())
	{
		_uniformBuffers.clear();
		for (size_t i = 0; i < _commandBuffers.size(); i++)
		{
			_uniformBuffers.emplace_back(_allocator, vk::BufferUsageFlagBits::eUniformBuffer,
										 vk::MemoryPropertyFlagBits::eHostVisible, VMA_MEMORY_USAGE_CPU_TO_GPU);
		}
	}

	for (size_t i = 0; i < _commandBuffers.size(); i++)
	{
		_inFlightFences[i] = _device->createFenceUnique(vk::FenceCreateInfo(vk::FenceCreateFlagBits::eSignaled));
		const std::string objectName = "_inFlightFences[" + std::to_string(i) + "]";
		const auto debugNameInfo = vk::DebugUtilsObjectNameInfoEXT(
			vk::ObjectType::eFence, reinterpret_cast<const uint64_t &>(_inFlightFences[i].get()), objectName.c_str());
		_device->setDebugUtilsObjectNameEXT(debugNameInfo);

		_imageAvailableSemaphores[i] = _device->createSemaphoreUnique(semaphoreCreateInfo);
		_renderFinishedSemaphores[i] = _device->createSemaphoreUnique(semaphoreCreateInfo);

		auto beginInfo = vk::CommandBufferBeginInfo();
		_commandBuffers[i]->begin(beginInfo);
		vk::ClearValue clearValues =
			vk::ClearValue(vk::ClearColorValue(std::array<float, 4>({0.0f, 0.0f, 0.0f, 1.0f})));
		vk::RenderPassBeginInfo renderPassBeginInfo = vk::RenderPassBeginInfo(
			_renderPass.get(), _framebuffers[i].get(), vk::Rect2D{{0, 0}, _swapchain->extent()}, 1, &clearValues);

		_commandBuffers[i]->beginRenderPass(renderPassBeginInfo, vk::SubpassContents::eInline);
		_commandBuffers[i]->bindPipeline(vk::PipelineBindPoint::eGraphics, *_pipeline);
		_commandBuffers[i]->draw(3, 1, 0, 0);
		_commandBuffers[i]->endRenderPass();
		_commandBuffers[i]->end();
	}
}
