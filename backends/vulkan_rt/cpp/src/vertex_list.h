#ifndef VERTEXLIST_H
#define VERTEXLIST_H

#include "vulkan_loader.h"

#include "utils.h"
#include "vkh/buffer.h"

#include <map>
#include <memory>

template <typename T, typename JW> struct RangeDescriptor
{
	RangeDescriptor()
	{
		ptr = nullptr;
		start = 0;
		count = 0;
		capacity = 0;
		jw_ptr = nullptr;
		jw_start = 0;
	}

	const T *ptr;
	unsigned int start;
	unsigned int count;
	unsigned int capacity;
	const JW *jw_ptr;
	unsigned int jw_start;
};

struct DrawDescriptor
{
	unsigned int start;
	unsigned int end;
	unsigned int jw_start;
	unsigned int jw_end;
};

template <typename T, typename JW, size_t ALIGNMENT = 2048> class VertexDataList
{
  public:
	VertexDataList(VmaAllocator allocator)
		: _buffer(allocator, vk::BufferUsageFlagBits::eVertexBuffer,
				  vk::MemoryPropertyFlagBits::eDeviceLocal | vk::MemoryPropertyFlagBits::eHostVisible,
				  VMA_MEMORY_USAGE_GPU_ONLY),
		  _jwBuffer(allocator, vk::BufferUsageFlagBits::eVertexBuffer,
					vk::MemoryPropertyFlagBits::eDeviceLocal | vk::MemoryPropertyFlagBits::eHostVisible,
					VMA_MEMORY_USAGE_GPU_ONLY),
		  _animBuffer(allocator, vk::BufferUsageFlagBits::eStorageBuffer | vk::BufferUsageFlagBits::eVertexBuffer,
					  vk::MemoryPropertyFlagBits::eDeviceLocal | vk::MemoryPropertyFlagBits::eHostVisible,
					  VMA_MEMORY_USAGE_GPU_ONLY),
		  _total_vertices(0), _total_jw(0), _recalculate_ranges(true)
	{
	}

	void add_pointer(unsigned int id, const T *pointer, unsigned int count, const JW *joints_weights = nullptr)
	{
		RangeDescriptor<T, JW> desc = {};
		desc.ptr = pointer;
		desc.start = 0;
		desc.capacity = next_multiple_of(count, ALIGNMENT);
		desc.count = count;
		desc.jw_ptr = joints_weights;
		desc.jw_start = 0;
		_pointers[id] = desc;

		DrawDescriptor draw_desc = {};
		draw_desc.start = 0;
		draw_desc.end = count;
		draw_desc.jw_start = 0;
		draw_desc.jw_end = 0;
		_draw_ranges[id] = draw_desc;

		_recalculate_ranges = true;
	}

	size_t size() const
	{
		if (!_buffer)
			return 0;
		return _buffer.size();
	}

	bool empty() const
	{
		return size() == 0;
	}

	bool has(unsigned int index) const
	{
		return _draw_ranges.find(index) != _draw_ranges.end();
	}

	void set_allocator(VmaAllocator allocator)
	{
		_buffer = vkh::Buffer<T>(allocator, vk::BufferUsageFlagBits::eVertexBuffer,
								 vk::MemoryPropertyFlagBits::eDeviceLocal | vk::MemoryPropertyFlagBits::eHostVisible,
								 VMA_MEMORY_USAGE_GPU_ONLY);
		_jwBuffer = vkh::Buffer<JW>(allocator, vk::BufferUsageFlagBits::eVertexBuffer,
									vk::MemoryPropertyFlagBits::eDeviceLocal | vk::MemoryPropertyFlagBits::eHostVisible,
									VMA_MEMORY_USAGE_GPU_ONLY);
		_animBuffer =
			vkh::Buffer<T>(allocator, vk::BufferUsageFlagBits::eVertexBuffer,
						   vk::MemoryPropertyFlagBits::eDeviceLocal | vk::MemoryPropertyFlagBits::eHostVisible,
						   VMA_MEMORY_USAGE_GPU_ONLY);
	}

	void update_pointer(unsigned int id, const T *pointer, unsigned int count, const JW *joints_weights = nullptr)
	{
		RangeDescriptor<T, JW> &reference = _pointers[id];
		DrawDescriptor draw_range = _draw_ranges[id];

		if (count > reference.capacity)
		{
			_recalculate_ranges = true;
			reference.capacity = next_multiple_of(count, 512);
		}

		reference.ptr = pointer;
		reference.jw_ptr = joints_weights;
		reference.count = count;
		draw_range.end = draw_range.start + count;
	}

	bool remove_pointer(unsigned int id)
	{
		bool has = false;
		auto it = _pointers.find(id);
		if (it != _pointers.end())
		{
			has = true;
			_pointers.erase(it);
		}

		auto it_d = _draw_ranges.find(id);
		if (it_d != _draw_ranges.end())
		{
			has = true;
			_draw_ranges.erase(it_d);
		}

		return has;
	}

	void update_ranges()
	{
		if (!_recalculate_ranges)
			return;

		unsigned int current_offset = 0;
		unsigned int current_offset_jw = 0;

		for (auto &[id, desc] : _pointers)
		{
			desc.start = current_offset;
			auto range = _draw_ranges.find(id);
			if (range == _draw_ranges.end())
				continue;

			range->second.start = current_offset;
			range->second.end = current_offset + desc.count;

			if (desc.jw_ptr)
			{
				desc.jw_start = current_offset_jw;
				range->second.jw_start = current_offset_jw;
				range->second.jw_end = current_offset_jw + desc.count;

				current_offset_jw += desc.capacity;
			}
			else
			{
				desc.jw_start = 0;
				range->second.jw_start = 0;
				range->second.jw_end = 0;
			}

			current_offset += desc.capacity;
		}

		_total_vertices = current_offset;
		_total_jw = current_offset_jw;
		_recalculate_ranges = false;
	}

	void update_data()
	{
		if (_total_vertices == 0)
			return;

		unsigned int total = _total_vertices;
		if (!_buffer || _buffer.size() < total)
			_buffer.reserve(total);

		T *data = reinterpret_cast<T *>(_buffer.map());
		for (const auto &[id, desc] : _pointers)
			memcpy(data + desc.start, desc.ptr, desc.count * sizeof(T));
		_buffer.unmap();

		total = _total_jw;
		if (total == 0)
			return;

		if (!_jwBuffer || _jwBuffer.size() < total)
		{
			_jwBuffer.reserve(next_multiple_of(total, ALIGNMENT));
			_animBuffer.reserve(next_multiple_of(total, ALIGNMENT));
		}

		JW *jw_data = reinterpret_cast<JW *>(_jwBuffer.map());
		for (const auto &[id, desc] : _pointers)
		{
			if (!desc.jw_ptr)
				continue;

			memcpy(jw_data + desc.start, desc.ptr, desc.count * sizeof(JW));
		}
		_jwBuffer.unmap();
	}

	vk::Buffer vertex_buffer() const
	{
		return _buffer.get();
	}

	vk::Buffer jw_buffer() const
	{
		return _jwBuffer.get();
	}

	vk::Buffer anim_buffer() const
	{
		return _animBuffer.get();
	}

	const std::map<unsigned int, DrawDescriptor> &get_draw_ranges() const
	{
		return _draw_ranges;
	}

	void free()
	{
		_buffer.free();
		_jwBuffer.free();
		_animBuffer.free();

		_pointers.clear();
		_draw_ranges.clear();

		_total_vertices = 0;
		_total_jw = 0;
		_recalculate_ranges = true;
	}

  private:
	vkh::Buffer<T> _buffer;
	vkh::Buffer<JW> _jwBuffer;
	vkh::Buffer<T> _animBuffer;

	std::map<unsigned int, RangeDescriptor<T, JW>> _pointers;
	std::map<unsigned int, DrawDescriptor> _draw_ranges;
	unsigned int _total_vertices;
	unsigned int _total_jw;
	bool _recalculate_ranges;
};

#endif // VERTEXLIST_H
