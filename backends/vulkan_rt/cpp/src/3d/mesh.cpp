#include "mesh.h"

Mesh3D::Mesh3D(VmaAllocator allocator) : _allocator(allocator)
{
}

Mesh3D::Mesh3D(VmaAllocator allocator, MeshData3D data) : _allocator(allocator)
{
	_bufferSize = static_cast<vk::DeviceSize>(data.num_vertices * sizeof(Vertex3D));
	allocate(_bufferSize);

	void *mappedMemory = nullptr;
	if (CheckVK(vmaMapMemory(_allocator, _allocation, &mappedMemory)) == vk::Result::eSuccess)
	{
		if (mappedMemory)
			memcpy(mappedMemory, data.vertices, static_cast<size_t>(_bufferSize));
		vmaUnmapMemory(_allocator, _allocation);
	}
}

Mesh3D::Mesh3D(const Mesh3D &mesh)
{
	_allocator = mesh._allocator;
	_bufferSize = mesh._bufferSize;
	if (_allocator && _bufferSize > 0 && mesh._buffer && mesh._allocation)
	{
		allocate(_bufferSize);

		void *destinationMemory = nullptr;
		void *sourceMemory = nullptr;
		if (CheckVK(vmaMapMemory(_allocator, _allocation, &destinationMemory)) == vk::Result::eSuccess)
		{
			if (destinationMemory)
			{
				if (CheckVK(vmaMapMemory(_allocator, mesh._allocation, &sourceMemory)) == vk::Result::eSuccess)
				{
					if (sourceMemory)
						memcpy(destinationMemory, sourceMemory, _bufferSize);
					vmaUnmapMemory(mesh._allocator, mesh._allocation);
				}
			}
			vmaUnmapMemory(_allocator, _allocation);
		}
	}
}

Mesh3D::~Mesh3D()
{
	free();
}

void Mesh3D::set_data(VmaAllocator allocator, MeshData3D data)
{
	free();
	_allocator = allocator;
	set_data(data);
}

void Mesh3D::set_data(MeshData3D data)
{
	assert(_allocator);
	if (!_allocator)
		return;

	const vk::DeviceSize requiredSize = static_cast<vk::DeviceSize>(data.num_vertices * sizeof(Vertex3D));
	if (requiredSize > _bufferSize)
	{
		free();
		allocate(requiredSize);
	}

	void *mappedMemory = nullptr;
	if (CheckVK(vmaMapMemory(_allocator, _allocation, &mappedMemory)) == vk::Result::eSuccess)
	{
		if (mappedMemory)
			memcpy(mappedMemory, data.vertices, static_cast<size_t>(_bufferSize));
		vmaUnmapMemory(_allocator, _allocation);
	}
}

void Mesh3D::free()
{
	if (_allocator && _buffer && _allocation)
	{
		vmaDestroyBuffer(_allocator, _buffer, _allocation);
		_buffer = nullptr;
		_allocation = nullptr;
	}
}

void Mesh3D::allocate(vk::DeviceSize size)
{
	vk::BufferCreateInfo bufferCreateInfo = vk::BufferCreateInfo(
		{}, size, vk::BufferUsageFlagBits::eVertexBuffer | vk::BufferUsageFlagBits::eTransferDst, {});
	VmaAllocationCreateInfo allocInfo = {};
	allocInfo.usage = VmaMemoryUsage::VMA_MEMORY_USAGE_GPU_ONLY;
	allocInfo.requiredFlags = static_cast<VkMemoryPropertyFlags>(vk::MemoryPropertyFlagBits::eDeviceLocal |
																 vk::MemoryPropertyFlagBits::eHostVisible);

	vmaCreateBuffer(_allocator, reinterpret_cast<VkBufferCreateInfo *>(&bufferCreateInfo), &allocInfo,
					reinterpret_cast<VkBuffer *>(&_buffer), &_allocation, &_allocationInfo);
	_bufferSize = size;
}
