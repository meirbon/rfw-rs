#ifndef VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_VULKAN_LOADER_H
#define VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_VULKAN_LOADER_H

#include <iostream>

#define FMT_HEADER_ONLY
#define VMA_STATIC_VULKAN_FUNCTIONS 0

#define VK_ENABLE_BETA_EXTENSIONS
#define VULKAN_HPP_DISPATCH_LOADER_DYNAMIC 1

// Use GLM's defines to detect compiler and silence external warnings
#if WINDOWS
#pragma warning(push, 0)
#else
#pragma GCC diagnostic push
#if MACOS || IOS
#pragma GCC diagnostic ignored "-Wnullability-completeness"
#endif
#pragma GCC diagnostic ignored "-Wunused-parameter"
#pragma GCC diagnostic ignored "-Wmissing-field-initializers"
#pragma GCC diagnostic ignored "-Wunused-variable"
#pragma GCC diagnostic ignored "-Wtype-limits"
#endif
#include <vulkan/vulkan.hpp>

#include <fmt/format.h>
#include <fmt/printf.h>

#include <vk_mem_alloc.h>
#if WINDOWS
#pragma warning(pop)
#else
#pragma GCC diagnostic pop
#endif

vk::Result _CheckVK(VkResult result, const char *command, const char *file, int line);
vk::Result _CheckVK(vk::Result result, const char *command, const char *file, int line);
template <typename T> T _CheckVK(vk::ResultValue<T> result, const char *command, const char *file, const int line)
{
	_CheckVK(result.result, command, file, line);
	return result.value;
}

vk::Device getAllocatorDevice(VmaAllocator allocator);

#define CheckVK(x) _CheckVK((x), #x, __FILE__, __LINE__)

#endif // VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_VULKAN_LOADER_H
