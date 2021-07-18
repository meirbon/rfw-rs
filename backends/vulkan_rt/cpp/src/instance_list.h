#ifndef INSTANCE_LIST_H
#define INSTANCE_LIST_H

#include "vulkan_loader.h"

#include "vkh/buffer.h"

#include "utils.h"

#include <map>

template <typename T> struct InstanceRange
{
	const T *ptr;
	unsigned int start;
	unsigned int end;
	unsigned int count;
	unsigned int capacity;
};

template <typename T> class InstanceDataList
{
  public:
	InstanceDataList(VmaAllocator allocator)
		: _buffer(allocator, vk::BufferUsageFlagBits::eStorageBuffer,
				  vk::MemoryPropertyFlagBits::eDeviceLocal | vk::MemoryPropertyFlagBits::eHostVisible,
				  VMA_MEMORY_USAGE_CPU_TO_GPU),
		  _total(0), _recalculate_ranges(true)
	{
		_buffer.allocate(1024, true);
	}

	bool has(unsigned int id) const
	{
		return _lists.find(id) != _lists.end();
	}

	size_t size() const
	{
		return _lists.size();
	}

	void add_instances_list(unsigned int id, const T *ptr, unsigned int count)
	{
		InstanceRange<T> desc = {};
		desc.ptr = ptr;
		desc.start = 0;
		desc.end = 0;
		desc.count = count;
		desc.capacity = next_multiple_of(count, 128);

		_lists[id] = desc;
	}

	void update_instances_list(unsigned int id, const T *ptr, unsigned int count)
	{
		auto it = _lists.find(id);
		if (it == _lists.end())
			return;

		if (count > it->second.capacity)
			_recalculate_ranges = true;

		it->second.ptr = ptr;
		it->second.count = count;
		it->second.capacity = next_multiple_of(count, 128);
	}

	bool remove_instances_list(unsigned int id)
	{
		auto it = _lists.find(id);
		if (it != _lists.end())
		{
			_lists.erase(it);
			return true;
		}

		return false;
	}

	vk::Buffer buffer()
	{
		return _buffer.get();
	}

	void update_ranges()
	{
		if (!_recalculate_ranges)
			return;

		unsigned int current_offset = 0;
		for (auto &[id, desc] : _lists)
		{
			desc.start = current_offset;
			desc.end = desc.start + desc.count;
			current_offset += desc.capacity;
		}

		_total = current_offset;
		_recalculate_ranges = false;
	}

	void update_data()
	{
		if (_total == 0)
			return;

		if (_buffer.size() < _total)
		{
			// Need to wait till Device is idle as buffers might be in use in draw calls
			_buffer.device().waitIdle();
			_buffer.reserve(next_multiple_of(_total, 512));
		}

		T *data = _buffer.map();
		for (const auto &[id, desc] : _lists)
			memcpy(data + desc.start, desc.ptr, desc.count * sizeof(T));
		_buffer.unmap();
	}

	const std::map<unsigned int, InstanceRange<T>> &get_ranges() const
	{
		return _lists;
	}

  private:
	VmaAllocator _allocator;
	vkh::Buffer<T> _buffer;
	std::map<unsigned int, InstanceRange<T>> _lists;
	unsigned int _total;
	bool _recalculate_ranges;
};

#endif // INSTANCE_LIST_H
