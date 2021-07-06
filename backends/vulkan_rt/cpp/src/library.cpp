#define API extern "C"

#if WINDOWS
#include <Windows.h>
#define VK_USE_PLATFORM_WIN32_KHR
#elif LINUX
#define VK_USE_PLATFORM_XLIB_KHR
#define VK_USE_PLATFORM_XCB_KHR
#define VK_USE_PLATFORM_WAYLAND_KHR
#endif

#include "vulkan_loader.h"

#include "library.h"
#include "renderer.hpp"

#include <iostream>

// Storage for dynamic Vulkan library loader
using Renderer = VulkanRenderer;

constexpr bool enableValidationLayers = true;

std::vector<const char *> getRequiredExtensions(unsigned long long handle)
{
	std::vector<const char *> extensions = {VK_KHR_SURFACE_EXTENSION_NAME};

	if constexpr (enableValidationLayers)
	{
		extensions.push_back(VK_EXT_DEBUG_UTILS_EXTENSION_NAME);
	}
#if WINDOWS
	extensions.push_back(VK_KHR_WIN32_SURFACE_EXTENSION_NAME);
#elif LINUX
	switch (handle)
	{
	case XLIB_HANDLE: {
		extensions.push_back(VK_KHR_XLIB_SURFACE_EXTENSION_NAME);
		break;
	}
	case XCB_HANDLE: {
		extensions.push_back(VK_KHR_XCB_SURFACE_EXTENSION_NAME);
		break;
	}
	case WAYLAND_HANDLE: {
		extensions.push_back(VK_KHR_WAYLAND_SURFACE_EXTENSION_NAME);
		break;
	}
	}
#endif
	return extensions;
}

bool checkValidationLayerSupport()
{
	uint32_t layerCount;
	CheckVK(vk::enumerateInstanceLayerProperties(&layerCount, nullptr));

	std::vector<vk::LayerProperties> availableLayers(layerCount);
	CheckVK(vk::enumerateInstanceLayerProperties(&layerCount, availableLayers.data()));

	const std::array<const char *, 1> validationLayers = {"VK_LAYER_KHRONOS_validation"};
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

API void *create_instance(unsigned long long handle0, unsigned long long handle1, unsigned long long handle2,
						  unsigned int width, unsigned int height, double scale)
{
	const std::vector<const char *> extensions = getRequiredExtensions(handle2);

	vk::ApplicationInfo applicationInfo = vk::ApplicationInfo("", 0, "rfw", 2, VK_API_VERSION_1_0);
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

	const std::array<const char *, 1> validationLayers = {"VK_LAYER_KHRONOS_validation"};
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

	vk::UniqueInstance instance = vk::createInstanceUnique(createInfo);

	VULKAN_HPP_DEFAULT_DISPATCHER.init(instance.get());

	vk::UniqueSurfaceKHR surface;
#if WINDOWS
	vk::Win32SurfaceCreateInfoKHR surfaceCreateInfo =
		vk::Win32SurfaceCreateInfoKHR({}, reinterpret_cast<HINSTANCE>(handle1), reinterpret_cast<HWND>(handle0));
	surface = instance->createWin32SurfaceKHRUnique(surfaceCreateInfo);
#elif LINUX
	switch (handle2)
	{
	case XLIB_HANDLE: {
		std::cout << "Surface type: XLIB" << std::endl;
		Display *display = reinterpret_cast<Display *>(handle0);
		Window window = static_cast<Window>(handle1);
		vk::XlibSurfaceCreateInfoKHR createInfoKhr = vk::XlibSurfaceCreateInfoKHR({}, display, window);
		surface = instance->createXlibSurfaceKHRUnique(createInfoKhr);
		break;
	}
	case XCB_HANDLE: {
		std::cout << "Surface type: XCB" << std::endl;
		xcb_connection_t *connection = reinterpret_cast<xcb_connection_t *>(handle0);
		xcb_window_t window = static_cast<xcb_window_t>(handle1);
		vk::XcbSurfaceCreateInfoKHR createInfoKhr = vk::XcbSurfaceCreateInfoKHR({}, connection, window);
		surface = instance->createXcbSurfaceKHRUnique(createInfoKhr);
		break;
	}
	case WAYLAND_HANDLE: {
		std::cout << "Surface type: WAYLAND" << std::endl;
		wl_surface *wlSurface = reinterpret_cast<wl_surface *>(handle0);
		wl_display *display = reinterpret_cast<wl_display *>(handle1);
		vk::WaylandSurfaceCreateInfoKHR createInfoKhr = vk::WaylandSurfaceCreateInfoKHR({}, display, wlSurface);
		surface = instance->createWaylandSurfaceKHRUnique(createInfoKhr);
		break;
	}
	}
	// TODO
#endif

	return VulkanRenderer::create_instance(std::move(instance), std::move(surface), width, height, scale);
}

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