#ifndef VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_DEVICE_H
#define VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_DEVICE_H

#include "vulkan_loader.h"

#include <set>

namespace vkh
{
vk::PhysicalDevice pickPhysicalDevice(vk::Instance instance, const char *vendorName);

std::set<uint32_t> findQueueFamilyIndices(vk::PhysicalDevice physicalDevice, vk::SurfaceKHR surface,
										  uint32_t *graphicsQueue, uint32_t *presentQueue);
} // namespace vkh

#endif // VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_DEVICE_H
