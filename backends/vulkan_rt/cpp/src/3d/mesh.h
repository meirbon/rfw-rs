#ifndef VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_MESH_H
#define VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_MESH_H

#include "../structs.h"
#include "../vulkan_loader.h"

#include <glm/glm.hpp>

class Mesh3D
{
  public:
	Mesh3D() = default;
	Mesh3D(VmaAllocator allocator);
	Mesh3D(VmaAllocator allocator, MeshData3D data);
	~Mesh3D();

	Mesh3D(const Mesh3D &mesh);

	void set_data(VmaAllocator allocator, MeshData3D data);
	void set_data(MeshData3D data);
	void free();

  private:
	void allocate(vk::DeviceSize size);

	vk::DeviceSize _bufferSize = 0;
	vk::Buffer _buffer = VK_NULL_HANDLE;
	VmaAllocation _allocation = VK_NULL_HANDLE;
	VmaAllocationInfo _allocationInfo;
	VmaAllocator _allocator = VK_NULL_HANDLE;
};

#endif // VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_MESH_H
