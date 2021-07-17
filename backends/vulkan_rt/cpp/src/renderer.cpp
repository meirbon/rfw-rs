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

vk::Result _CheckVK(vk::Result result, const char *command, const char *file, const int line)
{
	if (result != vk::Result::eSuccess)
	{
		std::cerr << file << ":" << line << " :: " << command << "; error: " << vk::to_string(result) << std::endl;
		exit(-1);
	}
	return result;
}

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

	std::vector<const char *> deviceExtensions = {VK_KHR_SWAPCHAIN_EXTENSION_NAME};
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
}

void VulkanRenderer::set_2d_mesh(unsigned int /*id*/, MeshData2D /*data*/)
{
}

void VulkanRenderer::set_2d_instances(unsigned int /*id*/, InstancesData2D /*data*/)
{
}

void VulkanRenderer::set_3d_mesh(unsigned int /*id*/, MeshData3D /*data*/)
{
}

void VulkanRenderer::set_3d_instances(unsigned int /*id*/, InstancesData3D /*data*/)
{
}

void VulkanRenderer::unload_3d_meshes(const unsigned int * /*ids*/, unsigned int /*num*/)
{
}

void VulkanRenderer::set_materials(const DeviceMaterial * /*materials*/, unsigned int /*num_materials*/)
{
}

void VulkanRenderer::synchronize()
{
}

void VulkanRenderer::render(const mat4 /*matrix_2d*/, const CameraView3D /*view_3d*/)
{
	const vk::ResultValue<uint32_t> value =
		_swapchain->acquire_next_image(std::numeric_limits<uint64_t>::max(), *_imageAvailableSemaphores[_currentFrame]);
	if (value.result == vk::Result::eErrorOutOfDateKHR)
		return;
	const uint32_t imageIndex = value.value;

	if (_imagesInFlight[imageIndex])
		CheckVK(_device->waitForFences(1, &_imagesInFlight[imageIndex], 1, std::numeric_limits<uint64_t>::max()));

	_imagesInFlight[imageIndex] = _inFlightFences[_currentFrame].get();

	vk::PipelineStageFlags waitStageMask = vk::PipelineStageFlagBits::eColorAttachmentOutput;
	vk::SubmitInfo submitInfo =
		vk::SubmitInfo(1, &*_imageAvailableSemaphores[_currentFrame], &waitStageMask, 1, &*_commandBuffers[imageIndex],
					   1, &*_renderFinishedSemaphores[_currentFrame]);

	CheckVK(_device->resetFences(1, &_imagesInFlight[imageIndex]));
	if (_graphicsQueue.submit(1, &submitInfo, _imagesInFlight[imageIndex]) != vk::Result::eSuccess)
		return;

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
	//	create_swapchain(width, height);

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

void VulkanRenderer::set_textures(const TextureData * /*data*/, unsigned int /*num_textures*/,
								  const unsigned int * /*changed*/)
{
}
