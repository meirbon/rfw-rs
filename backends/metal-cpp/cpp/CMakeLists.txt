cmake_minimum_required(VERSION 3.10)
project(MetalCpp)

set(CMAKE_CXX_STANDARD 17)

add_library(${PROJECT_NAME}
		src/library.mm
		src/renderer.mm
		)

set_property(TARGET MetalCpp APPEND_STRING PROPERTY COMPILE_FLAGS "-fobjc-arc")

target_link_libraries(${PROJECT_NAME}
		"-framework Metal"
		"-framework MetalKit"
		"-framework QuartzCore"
		)
target_include_directories(${PROJECT_NAME} PRIVATE ./deps)