#define VMA_IMPLEMENTATION

#include "vulkan_loader.h"

#include <iostream>

#ifdef VULKAN_HPP_DEFAULT_DISPATCH_LOADER_DYNAMIC_STORAGE
VULKAN_HPP_DEFAULT_DISPATCH_LOADER_DYNAMIC_STORAGE
#endif

vk::Result _CheckVK(VkResult result, const char *command, const char *file, int line)
{
	return _CheckVK(static_cast<vk::Result>(result), command, file, line);
}

vk::Result _CheckVK(vk::Result result, const char *command, const char *file, const int line)
{
	if (result != vk::Result::eSuccess)
	{
		std::cerr << file << ":" << line << " :: " << command << "; error: " << vk::to_string(result) << std::endl;
		exit(-1);
	}
	return result;
}

vk::Device getAllocatorDevice(VmaAllocator allocator)
{
	if (allocator)
		return static_cast<vk::Device>(allocator->m_hDevice);
	return VK_NULL_HANDLE;
}