#include "renderer.hpp"

#include <cassert>
#include <cstring>
#include <filesystem>
#include <iostream>

#include <glm/ext.hpp>
#include <glm/glm.hpp>

#include "shaders.h"

VULKAN_HPP_DEFAULT_DISPATCH_LOADER_DYNAMIC_STORAGE

using namespace glm;

vk::Result _CheckVK(vk::Result result, const char *command, const char *file, const int line)
{
	if (result != vk::Result::eSuccess)
	{
		std::cerr << file << ":" << line << " :: " << command << "; error: " << vk::to_string(result) << std::endl;
		exit(-1);
	}
	return result;
}

mat4 get_rh_matrix(const CameraView3D &view)
{
	const float width = 1.0f / view.inv_width;
	const float height = 1.0f / view.inv_height;

	const vec3 pos = vec3(view.pos.x, view.pos.y, view.pos.z);
	const vec3 direction = vec3(view.direction.x, view.direction.y, view.direction.z);
	const vec3 up = vec3(0, 1, 0);
	const mat4 projection = perspectiveFovRH(view.fov, width, height, view.near_plane, view.far_plane);
	const mat4 v = lookAtRH(pos, pos + direction, up);
	return projection * v;
}

mat4 get_rh_projection_matrix(const CameraView3D &view)
{
	const float width = 1.0f / view.inv_width;
	const float height = 1.0f / view.inv_height;
	return perspectiveFovRH(view.fov, width, height, view.near_plane, view.far_plane);
}

mat4 get_rh_view_matrix(const CameraView3D &view)
{
	const vec3 pos = vec3(view.pos.x, view.pos.y, view.pos.z);
	const vec3 direction = vec3(view.direction.x, view.direction.y, view.direction.z);
	const vec3 up = vec3(0, 1, 0);
	return lookAtRH(pos, pos + direction, up);
}

VulkanRenderer *VulkanRenderer::create_instance(vk::Instance instance, vk::SurfaceKHR surface, unsigned int width,
												unsigned int height, double scale)
{
	return new VulkanRenderer(instance, surface, width, height, scale);
}

VulkanRenderer::~VulkanRenderer()
{
	_instance.destroySurfaceKHR(_surface);
	_instance.destroy();
}

VulkanRenderer::VulkanRenderer(vk::Instance instance, vk::SurfaceKHR surface, unsigned int width, unsigned int height,
							   double scale)
	: _instance(instance), _surface(surface)
{
	std::cout << "Received Vulkan instance: " << instance << ", surface: " << surface << std::endl;
}

void VulkanRenderer::set_2d_mesh(unsigned int id, MeshData2D data)
{
}

void VulkanRenderer::set_2d_instances(unsigned int id, InstancesData2D data)
{
}

void VulkanRenderer::set_3d_mesh(unsigned int id, MeshData3D data)
{
}

void VulkanRenderer::set_3d_instances(unsigned int id, InstancesData3D data)
{
}

void VulkanRenderer::unload_3d_meshes(const unsigned int *ids, unsigned int num)
{
}

void VulkanRenderer::set_materials(const DeviceMaterial *materials, unsigned int num_materials)
{
}

void VulkanRenderer::synchronize()
{
}

void VulkanRenderer::render(mat4 matrix_2d, CameraView3D view_3d)
{
}

void VulkanRenderer::resize(unsigned int width, unsigned int height, double scale)
{
}

void VulkanRenderer::set_textures(const TextureData *data, unsigned int num_textures, const unsigned int *changed)
{
}
