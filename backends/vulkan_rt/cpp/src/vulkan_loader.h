#ifndef VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_VULKAN_LOADER_H
#define VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_VULKAN_LOADER_H

#define VK_ENABLE_BETA_EXTENSIONS
#define VULKAN_HPP_DISPATCH_LOADER_DYNAMIC 1
#define VMA_DYNAMIC_VULKAN_FUNCTIONS 1

// Use GLM's defines to detect compiler and silence external warnings
#if WINDOWS
#pragma warning(push, 0)
#else
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wnullability-completeness"
#pragma GCC diagnostic ignored "-Wunused-parameter"
#pragma GCC diagnostic ignored "-Wmissing-field-initializers"
#pragma GCC diagnostic ignored "-Wunused-variable"
#endif
#include <vulkan/vulkan.hpp>

#include <vk_mem_alloc.h>
#if WINDOWS
#pragma warning(pop)
#else
#pragma GCC diagnostic pop
#endif

#endif // VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_VULKAN_LOADER_H
