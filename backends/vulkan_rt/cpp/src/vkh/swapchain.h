//
// Created by Mèir Noordermeer on 17/07/2021.
//

#ifndef VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_VKH_SWAPCHAIN_H
#define VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_VKH_SWAPCHAIN_H

#include "../vulkan_loader.h"
#include <vector>

namespace vkh
{
class Swapchain
{
  public:
	Swapchain() = default;
	~Swapchain() = default;

	static std::unique_ptr<Swapchain> create(vk::Device device, vk::PhysicalDevice physicalDevice,
											 vk::UniqueSurfaceKHR surface, vk::Format format, uint32_t imageCount,
											 vk::SharingMode sharingMode, std::vector<uint32_t> familyIndices,
											 vk::ImageViewCreateInfo imageCreateInfo, uint32_t width, uint32_t height);
	void resize(uint32_t width, uint32_t height);

	size_t size() const;
	uint32_t width() const;
	uint32_t height() const;
	vk::Extent2D extent() const;
	vk::Format format() const;
	vk::Viewport viewport(float minDepth = 0.0f, float maxDepth = 1.0f) const;

	vk::Image image_at(uint32_t index) const;
	vk::ImageView image_view_at(uint32_t index) const;
	vk::ResultValue<uint32_t> acquire_next_image(uint64_t timeout, vk::Semaphore semaphore = {}, vk::Fence fence = {});

	vk::SwapchainKHR &get()
	{
		return *_swapchain;
	}

	vk::SwapchainKHR &operator*() VULKAN_HPP_NOEXCEPT
	{
		return *_swapchain;
	}

  private:
	vk::Device _device;
	vk::PhysicalDevice _physicalDevice;
	vk::UniqueSurfaceKHR _surface;

	vk::UniqueSwapchainKHR _swapchain;
	std::vector<vk::Image> _swapchainImages;
	std::vector<vk::UniqueImageView> _swapchainImageViews;
	vk::ImageViewCreateInfo _imageCreateInfo;

	vk::SharingMode _sharingMode;
	std::vector<uint32_t> _familyIndices;
	vk::Extent2D _extent;
	vk::Format _format;
	uint32_t _imageCount;
};
} // namespace vkh

#endif // VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_VKH_SWAPCHAIN_H
