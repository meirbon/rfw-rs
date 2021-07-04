#define API extern "C"
#if WINDOWS
#include <Windows.h>
#define VK_USE_PLATFORM_WIN32_KHR
#endif

#include "library.h"
#include "renderer.hpp"

#include <iostream>

// Storage for dynamic Vulkan library loader
using Renderer = VulkanRenderer;

const std::vector<const char *> validationLayers = {"VK_LAYER_KHRONOS_validation"};
constexpr bool enableValidationLayers = true;

std::vector<const char *> getRequiredExtensions()
{
	if constexpr (enableValidationLayers)
	{
		return {VK_KHR_WIN32_SURFACE_EXTENSION_NAME, VK_KHR_SURFACE_EXTENSION_NAME, VK_EXT_DEBUG_UTILS_EXTENSION_NAME};
	}
	else
	{
		return {VK_KHR_WIN32_SURFACE_EXTENSION_NAME, VK_KHR_SURFACE_EXTENSION_NAME};
	}
}

bool checkValidationLayerSupport()
{
	uint32_t layerCount;
	CheckVK(vk::enumerateInstanceLayerProperties(&layerCount, nullptr));

	std::vector<vk::LayerProperties> availableLayers(layerCount);
	CheckVK(vk::enumerateInstanceLayerProperties(&layerCount, availableLayers.data()));

	for (const char *layerName : validationLayers)
	{
		bool layerFound = false;

		for (const auto &layerProperties : availableLayers)
		{
			if (strcmp(layerName, layerProperties.layerName) == 0)
			{
				layerFound = true;
				break;
			}
		}

		if (!layerFound)
		{
			return false;
		}
	}

	return false;
}

VKAPI_ATTR VkBool32 VKAPI_CALL debugCallback(VkDebugUtilsMessageSeverityFlagBitsEXT messageSeverity,
											 VkDebugUtilsMessageTypeFlagsEXT messageType,
											 const VkDebugUtilsMessengerCallbackDataEXT *pCallbackData, void *pUserData)
{
	std::cerr << "Validation layer: " << pCallbackData->pMessage << std::endl;
	return VK_FALSE;
}

#if WINDOWS
extern "C" void *create_instance(void *hwnd, void *hinstance, unsigned int width, unsigned int height, double scale)
{
	const std::vector<const char *> extensions = getRequiredExtensions();

	vk::ApplicationInfo applicationInfo = vk::ApplicationInfo("", 0, "rfw", 2, VK_API_VERSION_1_2);
	vk::InstanceCreateInfo createInfo = {};
	createInfo.pApplicationInfo = &applicationInfo;
	createInfo.enabledExtensionCount = extensions.size();
	createInfo.ppEnabledExtensionNames = extensions.data();

	vk::DebugUtilsMessengerCreateInfoEXT debugCreateInfo = vk::DebugUtilsMessengerCreateInfoEXT(
		{},
		vk::DebugUtilsMessageSeverityFlagBitsEXT::eVerbose | vk::DebugUtilsMessageSeverityFlagBitsEXT::eWarning |
			vk::DebugUtilsMessageSeverityFlagBitsEXT::eError,
		vk::DebugUtilsMessageTypeFlagBitsEXT::eGeneral | vk::DebugUtilsMessageTypeFlagBitsEXT::ePerformance |
			vk::DebugUtilsMessageTypeFlagBitsEXT::eValidation,
		debugCallback);
	if constexpr (enableValidationLayers)
	{
		createInfo.enabledLayerCount = validationLayers.size();
		createInfo.ppEnabledLayerNames = validationLayers.data();
		createInfo.pNext = &debugCreateInfo;
	}
	else
	{
		createInfo.enabledLayerCount = 0;
		createInfo.pNext = nullptr;
	}

	vk::DynamicLoader dl;
	auto vkGetInstanceProcAddrPtr = dl.getProcAddress<PFN_vkGetInstanceProcAddr>("vkGetInstanceProcAddr");
	VULKAN_HPP_DEFAULT_DISPATCHER.init(vkGetInstanceProcAddrPtr);

	vk::Instance instance;
	CheckVK(vk::createInstance(&createInfo, nullptr, &instance));

	VULKAN_HPP_DEFAULT_DISPATCHER.init(instance);

	vk::Win32SurfaceCreateInfoKHR surfaceCreateInfo =
		vk::Win32SurfaceCreateInfoKHR({}, reinterpret_cast<HINSTANCE>(hinstance), reinterpret_cast<HWND>(hwnd));
	vk::SurfaceKHR surface;
	CheckVK(instance.createWin32SurfaceKHR(&surfaceCreateInfo, nullptr, &surface));

	return VulkanRenderer::create_instance(instance, surface, width, height, scale);
}
#endif

extern "C" void destroy_instance(void *instance)
{
	delete reinterpret_cast<Renderer *>(instance);
}

extern "C" void set_2d_mesh(void *instance, unsigned int id, MeshData2D data)
{
	Renderer *renderer = reinterpret_cast<Renderer *>(instance);
	renderer->set_2d_mesh(id, data);
}
extern "C" void set_2d_instances(void *instance, unsigned int id, InstancesData2D data)
{
	Renderer *renderer = reinterpret_cast<Renderer *>(instance);
	renderer->set_2d_instances(id, data);
}

extern "C" void set_3d_mesh(void *instance, unsigned int id, MeshData3D data)
{
	Renderer *renderer = reinterpret_cast<Renderer *>(instance);
	renderer->set_3d_mesh(id, data);
}
extern "C" void unload_3d_meshes(void *instance, const unsigned int *ids, unsigned int num)
{
	Renderer *renderer = reinterpret_cast<Renderer *>(instance);
	renderer->unload_3d_meshes(ids, num);
}
extern "C" void set_3d_instances(void *instance, unsigned int id, InstancesData3D data)
{
	Renderer *renderer = reinterpret_cast<Renderer *>(instance);
	renderer->set_3d_instances(id, data);
}

extern "C" void set_materials(void *instance, const DeviceMaterial *materials, unsigned int num_materials)
{
	Renderer *renderer = reinterpret_cast<Renderer *>(instance);
	renderer->set_materials(materials, num_materials);
}
extern "C" void set_textures(void *instance, const TextureData *const data, unsigned int num_textures,
							 const unsigned int *changed)
{
	Renderer *renderer = reinterpret_cast<Renderer *>(instance);
	renderer->set_textures(data, num_textures, changed);
}

extern "C" void render(void *instance, Vector4x4 matrix_2d, CameraView3D view_3d)
{
	glm::mat4 matrix;
	std::memcpy(glm::value_ptr(matrix), &matrix_2d, sizeof(glm::mat4));

	Renderer *renderer = reinterpret_cast<Renderer *>(instance);
	renderer->render(matrix, view_3d);
}

extern "C" void synchronize(void *instance)
{
	Renderer *renderer = reinterpret_cast<Renderer *>(instance);
	renderer->synchronize();
}

extern "C" void resize(void *instance, unsigned int width, unsigned int height, double scale_factor)
{
	Renderer *renderer = reinterpret_cast<Renderer *>(instance);
	renderer->resize(width, height, scale_factor);
}