//
// Created by Mèir Noordermeer on 17/07/2021.
//

#include "swapchain.h"

#include <glm/glm.hpp>

namespace vkh
{

std::unique_ptr<Swapchain> Swapchain::create(vk::Device device, vk::PhysicalDevice physicalDevice,
											 vk::UniqueSurfaceKHR surface, vk::Format format, uint32_t imageCount,
											 vk::SharingMode sharingMode, std::vector<uint32_t> familyIndices,
											 vk::ImageViewCreateInfo imageCreateInfo, uint32_t width, uint32_t height)
{
	std::unique_ptr<Swapchain> instance = std::make_unique<Swapchain>();
	instance->_device = device;
	instance->_physicalDevice = physicalDevice;
	instance->_surface = std::move(surface);
	instance->_imageCreateInfo = imageCreateInfo;
	instance->_imageCreateInfo.format = format;
	instance->_format = format;
	instance->_imageCount = imageCount;
	instance->_sharingMode = sharingMode;
	instance->_familyIndices = std::move(familyIndices);

	const vk::SurfaceCapabilitiesKHR capabilitiesKhr =
		instance->_physicalDevice.getSurfaceCapabilitiesKHR(*instance->_surface);
	width = glm::clamp(capabilitiesKhr.minImageExtent.width, capabilitiesKhr.maxImageExtent.width, width);
	height = glm::clamp(capabilitiesKhr.minImageExtent.height + 1, capabilitiesKhr.maxImageExtent.height + 1, height);

	instance->_extent.width = width;
	instance->_extent.height = height;

	vk::SwapchainCreateInfoKHR swapchainCreateInfoKhr =
		vk::SwapchainCreateInfoKHR({}, *instance->_surface, imageCount, format, vk::ColorSpaceKHR::eSrgbNonlinear,
								   instance->_extent, 1, vk::ImageUsageFlagBits::eColorAttachment,
								   instance->_sharingMode, static_cast<uint32_t>(instance->_familyIndices.size()),
								   instance->_familyIndices.data(), vk::SurfaceTransformFlagBitsKHR::eIdentity,
								   vk::CompositeAlphaFlagBitsKHR::eOpaque, vk::PresentModeKHR::eFifo, true, {});

	auto swapchain = instance->_device.createSwapchainKHRUnique(swapchainCreateInfoKhr);
	instance->_swapchain = std::move(swapchain);

	instance->_swapchainImages = instance->_device.getSwapchainImagesKHR(instance->_swapchain.get());
	instance->_swapchainImageViews.clear();
	instance->_swapchainImageViews.reserve(instance->_swapchainImages.size());

	for (const vk::Image &image : instance->_swapchainImages)
	{
		vk::ImageViewCreateInfo imageViewCreateInfo = instance->_imageCreateInfo;
		imageViewCreateInfo.image = image;
		instance->_swapchainImageViews.push_back(instance->_device.createImageViewUnique(imageViewCreateInfo));
	}

	return instance;
}

void Swapchain::resize(uint32_t width, uint32_t height)
{
	const vk::SurfaceCapabilitiesKHR capabilitiesKhr = _physicalDevice.getSurfaceCapabilitiesKHR(_surface.get());
	width = glm::clamp(capabilitiesKhr.minImageExtent.width, capabilitiesKhr.maxImageExtent.width, width);
	height = glm::clamp(capabilitiesKhr.minImageExtent.height + 1, capabilitiesKhr.maxImageExtent.height + 1, height);

	_extent.width = width;
	_extent.height = height;

	vk::SwapchainCreateInfoKHR swapchainCreateInfoKhr = vk::SwapchainCreateInfoKHR(
		{}, *_surface, _imageCount, _format, vk::ColorSpaceKHR::eSrgbNonlinear, _extent, 1,
		vk::ImageUsageFlagBits::eColorAttachment, _sharingMode, static_cast<uint32_t>(_familyIndices.size()),
		_familyIndices.data(), vk::SurfaceTransformFlagBitsKHR::eIdentity, vk::CompositeAlphaFlagBitsKHR::eOpaque,
		vk::PresentModeKHR::eFifo, true, *_swapchain);

	auto swapchain = _device.createSwapchainKHRUnique(swapchainCreateInfoKhr);
	_swapchain = std::move(swapchain);

	_swapchainImages = _device.getSwapchainImagesKHR(_swapchain.get());
	_swapchainImageViews.clear();
	_swapchainImageViews.reserve(_swapchainImages.size());

	for (const vk::Image &image : _swapchainImages)
	{
		vk::ImageViewCreateInfo imageViewCreateInfo = _imageCreateInfo;
		imageViewCreateInfo.image = image;
		_swapchainImageViews.push_back(_device.createImageViewUnique(imageViewCreateInfo));
	}
}

size_t Swapchain::size() const
{
	return _swapchainImages.size();
}

uint32_t Swapchain::width() const
{
	return _extent.width;
}

uint32_t Swapchain::height() const
{
	return _extent.height;
}

vk::Extent2D Swapchain::extent() const
{
	return _extent;
}

vk::Format Swapchain::format() const
{
	return _format;
}

vk::Viewport Swapchain::viewport(const float minDepth, const float maxDepth) const
{
	return vk::Viewport(0.0f, 0.0f, static_cast<float>(_extent.width), static_cast<float>(_extent.height), minDepth,
						maxDepth);
}

vk::Image Swapchain::image_at(uint32_t index) const
{
	return _swapchainImages[static_cast<size_t>(index)];
}

vk::ImageView Swapchain::image_view_at(uint32_t index) const
{
	return *_swapchainImageViews[static_cast<size_t>(index)];
}

vk::ResultValue<uint32_t> Swapchain::acquire_next_image(uint64_t timeout, vk::Semaphore semaphore, vk::Fence fence)
{
	return _device.acquireNextImageKHR(*_swapchain, timeout, semaphore, fence);
}

} // namespace vkh