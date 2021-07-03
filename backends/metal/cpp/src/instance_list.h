//
// Created by Mèir Noordermeer on 07/06/2021.
//

#ifndef METALCPP_SRC_INSTANCE_LIST_H
#define METALCPP_SRC_INSTANCE_LIST_H

#include "buffer.hpp"
#include "utils.hpp"

#import <Metal/Metal.h>

#include <map>

template <typename T> struct InstanceRange
{
    const T *ptr;
    unsigned int start;
    unsigned int end;
    unsigned int count;
    unsigned int capacity;
};

template <typename T> class InstanceList
{
  public:
    InstanceList(id<MTLDevice> device)
        : _buffer(std::make_unique<Buffer<T>>(device, 2048)), _total(0), _recalculate_ranges(true)
    {
    }

    bool has(unsigned int id) const
    {
        return _lists.find(id) != _lists.end();
    }

    void add_instances_list(unsigned int id, const T *ptr, unsigned int count)
    {
        InstanceRange<T> desc = {};
        desc.ptr = ptr;
        desc.start = 0;
        desc.end = 0;
        desc.count = count;
        desc.capacity = next_multiple_of(count, 128);

        _lists.insert({id, desc});
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

    id<MTLBuffer> buffer()
    {
        return _buffer->buffer();
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

    void update_data(id<MTLDevice> device)
    {
        if (_total == 0)
            return;

        if (_buffer->size() < _total)
        {
            _buffer = std::make_unique<Buffer<T>>(device, next_multiple_of(_total, 512));
        }

        std::byte *data = reinterpret_cast<std::byte *>(_buffer->data());
        for (const auto &[id, desc] : _lists)
        {
            memcpy(data + (desc.start * sizeof(T)), desc.ptr, desc.count * sizeof(T));
        }

        _buffer->update();
    }

    const std::map<unsigned int, InstanceRange<T>> &get_ranges() const
    {
        return _lists;
    }

  private:
    std::unique_ptr<Buffer<T>> _buffer;
    std::map<unsigned int, InstanceRange<T>> _lists;
    unsigned int _total;
    bool _recalculate_ranges;
};

#endif // METALCPP_SRC_INSTANCE_LIST_H
