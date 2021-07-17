#ifndef VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_VKH_BUFFER_H
#define VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_VKH_BUFFER_H

#include "../vulkan_loader.h"

namespace vkh
{

enum BufferResult
{
	Error = 0,
	Ok = 1,
	Reallocated = 2,
	NotAllocated = 4,
};

template <typename T> class Buffer
{
  public:
	Buffer(vk::BufferUsageFlags usageFlags,
		   vk::MemoryPropertyFlags flags = vk::MemoryPropertyFlagBits::eDeviceLocal |
										   vk::MemoryPropertyFlagBits::eHostVisible,
		   VmaMemoryUsage usage = VmaMemoryUsage::VMA_MEMORY_USAGE_GPU_ONLY)
		: _usageFlags(usageFlags), _flags(flags), _usage(usage){};

	Buffer(VmaAllocator allocator = VK_NULL_HANDLE, vk::BufferUsageFlags usageFlags = {},
		   vk::MemoryPropertyFlags flags = {}, VmaMemoryUsage usage = VmaMemoryUsage::VMA_MEMORY_USAGE_GPU_ONLY)
		: _allocator(allocator), _usageFlags(usageFlags), _flags(flags), _usage(usage)
	{
	}

	Buffer(Buffer &&buffer)
		: _bufferSize(buffer._bufferSize), _buffer(buffer._buffer), _allocation(buffer._allocation),
		  _allocationInfo(buffer._allocationInfo), _allocator(buffer._allocator), _usageFlags(buffer._usageFlags),
		  _flags(buffer._flags), _usage(buffer._usage)
	{
		buffer._allocator = nullptr;
		buffer._allocation = {};
		buffer._buffer = nullptr;
	}

	Buffer(const Buffer &buffer) = delete;

	Buffer &operator=(Buffer buffer)
	{
		free();
		_bufferSize = buffer._bufferSize;
		_buffer = buffer._buffer;
		_allocation = buffer._allocation;
		_allocationInfo = buffer._allocationInfo;
		_allocator = buffer._allocator;
		_usageFlags = buffer._usageFlags;
		_flags = buffer._flags;
		_usage = buffer._usage;

		buffer._allocator = nullptr;
		buffer._allocation = {};
		buffer._buffer = nullptr;
		return *this;
	}

	Buffer clone() const
	{
		Buffer newBuffer = Buffer(_allocator);

		newBuffer._allocator = _allocator;
		newBuffer._bufferSize = _bufferSize;
		newBuffer._flags = _flags;
		newBuffer._usageFlags = _usageFlags;
		newBuffer._usage = _usage;

		if (_allocator && _bufferSize > 0 && _buffer && _allocation)
		{
			// TODO: Handle case where memory is not HOST_VISIBLE

			newBuffer.allocate(_bufferSize);

			void *destinationMemory = nullptr;
			void *sourceMemory = nullptr;
			if (CheckVK(vmaMapMemory(newBuffer._allocator, newBuffer._allocation, &destinationMemory)) ==
				vk::Result::eSuccess)
			{
				if (destinationMemory)
				{
					if (CheckVK(vmaMapMemory(_allocator, _allocation, &sourceMemory)) == vk::Result::eSuccess)
					{
						if (sourceMemory)
							memcpy(destinationMemory, sourceMemory, _bufferSize);
						vmaUnmapMemory(_allocator, _allocation);
					}
				}
				vmaUnmapMemory(newBuffer._allocator, newBuffer._allocation);
			}
		}

		return newBuffer;
	}

	~Buffer()
	{
		free();
	}

	size_t size() const
	{
		return _bufferSize / sizeof(T);
	}

	vk::DeviceSize buffer_size() const
	{
		return _bufferSize;
	}

	BufferResult set_data(VmaAllocator allocator, vk::BufferUsageFlags usageFlags, vk::MemoryPropertyFlags flags,
						  VmaMemoryUsage usage, const T *data, size_t count)
	{
		_allocator = allocator;
		_usageFlags = usageFlags;
		_flags = flags;
		_usage = usage;
		return set_data(data, count);
	}

	BufferResult set_data(const T *data, size_t count)
	{
		assert(_allocator);
		if (!_allocator)
			return Error;

		BufferResult result = BufferResult::Ok;

		if ((count * sizeof(T)) > _bufferSize)
		{
			free();
			allocate(count * sizeof(T));
			result = BufferResult::Reallocated;
		}

		void *mappedMemory = nullptr;
		if (CheckVK(vmaMapMemory(_allocator, _allocation, &mappedMemory)) == vk::Result::eSuccess)
		{
			if (mappedMemory)
				memcpy(mappedMemory, data, static_cast<size_t>(_bufferSize));
			vmaUnmapMemory(_allocator, _allocation);
			return result;
		}

		return BufferResult::Error;
	}

	BufferResult set_data(void *data, size_t offset, size_t size)
	{
		assert(_allocator);
		if (!_allocator)
			return BufferResult::Error;

		if ((size + offset) * sizeof(T) > _bufferSize)
			return BufferResult::Error;

		void *mappedMemory = nullptr;
		if (CheckVK(vmaMapMemory(_allocator, _allocation, &mappedMemory)) == vk::Result::eSuccess)
		{
			if (mappedMemory)
				memcpy(reinterpret_cast<T *>(mappedMemory) + offset, data, size * sizeof(T));
			vmaUnmapMemory(_allocator, _allocation);
			return BufferResult::Ok;
		}

		return BufferResult::Error;
	}

	T *map()
	{
		if (!(_flags & vk::MemoryPropertyFlagBits::eHostVisible))
			return nullptr;

		if (_allocator && _allocation)
		{
			void *mappedMemory = nullptr;
			if (CheckVK(vmaMapMemory(_allocator, _allocation, &mappedMemory)) == vk::Result::eSuccess)
				return reinterpret_cast<T *>(mappedMemory);
		}
		return nullptr;
	}

	const T *map() const
	{
		if (!(_flags & vk::MemoryPropertyFlagBits::eHostVisible))
			return nullptr;

		if (_allocator && _allocation)
		{
			void *mappedMemory = nullptr;
			if (CheckVK(vmaMapMemory(_allocator, _allocation, &mappedMemory)) == vk::Result::eSuccess)
				return reinterpret_cast<const T *>(mappedMemory);
		}
		return nullptr;
	}

	void unmap() const
	{
		if (!(_flags & vk::MemoryPropertyFlagBits::eHostVisible))
			return;

		if (_allocator && _allocation)
			vmaUnmapMemory(_allocator, _allocation);
	}

	BufferResult reserve(size_t count, bool force = false)
	{
		return allocate(count * sizeof(T), force);
	}

	BufferResult allocate(vk::DeviceSize sizeInBytes, bool force = false)
	{
		if (!force && (sizeInBytes == 0 || sizeInBytes < _bufferSize))
			return BufferResult::Ok;

		vk::BufferCreateInfo bufferCreateInfo = vk::BufferCreateInfo({}, sizeInBytes, _usageFlags, {});
		VmaAllocationCreateInfo allocInfo = {};
		allocInfo.usage = _usage;
		allocInfo.requiredFlags = static_cast<VkMemoryPropertyFlags>(_flags);

		vmaCreateBuffer(_allocator, reinterpret_cast<VkBufferCreateInfo *>(&bufferCreateInfo), &allocInfo,
						reinterpret_cast<VkBuffer *>(&_buffer), &_allocation, &_allocationInfo);
		_bufferSize = sizeInBytes;
		return BufferResult::NotAllocated;
	}

	void free()
	{
		if (_allocator && _allocation && _buffer)
		{
			vmaDestroyBuffer(_allocator, _buffer, _allocation);
			_buffer = nullptr;
			_allocation = nullptr;
		}
	}

	operator bool() const
	{
		return _buffer;
	}

	vk::Buffer get() const
	{
		return _buffer;
	}

	vk::Buffer &operator*()
	{
		return _buffer;
	}

  private:
	vk::DeviceSize _bufferSize = 0;
	vk::Buffer _buffer = VK_NULL_HANDLE;
	VmaAllocation _allocation = VK_NULL_HANDLE;
	VmaAllocationInfo _allocationInfo;
	VmaAllocator _allocator = VK_NULL_HANDLE;
	vk::BufferUsageFlags _usageFlags;
	vk::MemoryPropertyFlags _flags;
	VmaMemoryUsage _usage;
};
} // namespace vkh

#endif // VULKANRTCPP_BACKENDS_VULKAN_RT_CPP_SRC_VKH_BUFFER_H
