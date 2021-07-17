cmake_minimum_required(VERSION 3.10)
project(VulkanRtCpp)

set(CMAKE_CXX_STANDARD 17)

if (APPLE)
	set(SOURCES
			src/library.cpp
			src/renderer.cpp
			src/device.cpp
			src/vulkan_loader.cpp
			src/create_metal_layer.mm
			src/vkh/swapchain.cpp
			src/vk_mem_alloc.cpp
			)
else (APPLE)
	set(SOURCES src/library.cpp
			src/renderer.cpp
			src/device.cpp
			src/vulkan_loader.cpp
			src/vkh/swapchain.cpp
			src/vk_mem_alloc.cpp
			)
endif (APPLE)

add_library(${PROJECT_NAME} STATIC ${SOURCES})

if (WIN32)
	target_compile_definitions(${PROJECT_NAME} PUBLIC -DWINDOWS=1)
endif (WIN32)

if (UNIX AND NOT APPLE)
	target_compile_definitions(${PROJECT_NAME} PUBLIC -DLINUX=1)
else (UNIX AND NOT APPLE)
	target_compile_definitions(${PROJECT_NAME} PUBLIC -DMACOS=1)
endif (UNIX AND NOT APPLE)


find_package(Vulkan REQUIRED)

target_link_libraries(${PROJECT_NAME} PRIVATE Vulkan::Vulkan)
target_include_directories(${PROJECT_NAME} PRIVATE ./deps Vulkan::Headers)