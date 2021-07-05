cmake_minimum_required(VERSION 3.10)
project(VulkanRtCpp)

set(CMAKE_CXX_STANDARD 17)

add_library(${PROJECT_NAME}
		src/library.cpp
		src/renderer.cpp
		)

if (WIN32)
	target_compile_definitions(${PROJECT_NAME} PUBLIC -DWINDOWS=1)
endif (WIN32)

if (UNIX AND NOT APPLE)
	target_compile_definitions(${PROJECT_NAME} PUBLIC -DLINUX=1)
endif (UNIX AND NOT APPLE)


find_package(Vulkan REQUIRED)

target_link_libraries(${PROJECT_NAME} PRIVATE Vulkan::Vulkan)
target_include_directories(${PROJECT_NAME} PRIVATE ./deps Vulkan::Headers)