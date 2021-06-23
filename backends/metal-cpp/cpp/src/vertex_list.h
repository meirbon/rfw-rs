//
// Created by Mèir Noordermeer on 06/06/2021.
//

#ifndef METALCPP_SRC_VERTEXLIST_H
#define METALCPP_SRC_VERTEXLIST_H

#include "buffer.hpp"
#include "utils.hpp"
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

template <typename T, typename JW> class VertexList
{
  public:
    VertexList() : _total_vertices(0), _total_jw(0), _recalculate_ranges(true)
    {
    }

    void add_pointer(unsigned int id, const T *pointer, unsigned int count, const JW *joints_weights = nullptr)
    {
        RangeDescriptor<T, JW> desc = {};
        desc.ptr = pointer;
        desc.start = 0;
        desc.capacity = next_multiple_of(count, 512);
        desc.count = count;
        desc.jw_ptr = joints_weights;
        desc.jw_start = 0;
        _pointers.insert({id, desc});

        DrawDescriptor draw_desc = {};
        draw_desc.start = 0;
        draw_desc.end = count;
        draw_desc.jw_start = 0;
        draw_desc.jw_end = 0;
        _draw_ranges.insert({id, draw_desc});

        _recalculate_ranges = true;
    }

    size_t size() const
    {
        if (!_buffer)
            return 0;
        return _buffer->size();
    }

    bool empty() const
    {
        return size() == 0;
    }

    bool has(unsigned int index) const
    {
        return _draw_ranges.find(index) != _draw_ranges.end();
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
            std::map<unsigned int, DrawDescriptor>::iterator range = _draw_ranges.find(id);
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

    void update_data(id<MTLDevice> device)
    {
        if (_total_vertices == 0)
            return;

        unsigned int total = _total_vertices;
        if (!_buffer || (_buffer && _buffer->size() < total))
        {
            _buffer = std::make_unique<Buffer<T>>(device, next_multiple_of(total, 2048));
        }

        T *data = reinterpret_cast<T *>(_buffer->data());
        for (const auto &[id, desc] : _pointers)
        {
            memcpy(data + desc.start, desc.ptr, desc.count * sizeof(T));
        }

        // Let Metal know the buffer got updated.
        _buffer->update();

        total = _total_jw;
        if (total == 0)
            return;

        if (!_jw_buffer || (_jw_buffer && _jw_buffer->size() < total))
        {
            _jw_buffer = std::make_unique<Buffer<JW>>(device, next_multiple_of(total, 2048));
            _anim_buffer = std::make_unique<Buffer<T>>(device, next_multiple_of(total, 2048));
        }

        JW *jw_data = reinterpret_cast<JW *>(_jw_buffer->data());
        for (const auto &[id, desc] : _pointers)
        {
            if (!desc.jw_ptr)
                continue;

            memcpy(jw_data + desc.start, desc.ptr, desc.count * sizeof(JW));
        }
        _jw_buffer->update();
    }

    id<MTLBuffer> vertex_buffer() const
    {
        if (_buffer)
            return _buffer->buffer();
        return nil;
    }

    id<MTLBuffer> jw_buffer() const
    {
        if (_jw_buffer)
            return _jw_buffer->buffer();
        return nil;
    }

    id<MTLBuffer> anim_buffer() const
    {
        if (_anim_buffer)
            return _anim_buffer->buffer();
        return nil;
    }

    const std::map<unsigned int, DrawDescriptor> &get_draw_ranges() const
    {
        return _draw_ranges;
    }

  private:
    std::unique_ptr<Buffer<T>> _buffer;
    std::unique_ptr<Buffer<JW>> _jw_buffer;
    std::unique_ptr<Buffer<T>> _anim_buffer;

    std::map<unsigned int, RangeDescriptor<T, JW>> _pointers;
    std::map<unsigned int, DrawDescriptor> _draw_ranges;
    unsigned int _total_vertices;
    unsigned int _total_jw;
    bool _recalculate_ranges;
};

#endif // METALCPP_SRC_VERTEXLIST_H
