#ifndef BUFFER_H
#define BUFFER_H

#import <Metal/Metal.h>

#include <cxxabi.h>
#include <string>
#include <typeinfo>

template <typename T> class Buffer
{
  public:
    Buffer(id<MTLDevice> device, size_t count, MTLResourceOptions options = MTLResourceStorageModeManaged)
        : _device(device), _count(count)
    {
        const size_t bytes = count * sizeof(T);

        _buffer = [_device newBufferWithLength:bytes options:options];
        _buffer.label = [NSString stringWithCString:typeid(T).name() encoding:NSASCIIStringEncoding];
    }

    Buffer(id<MTLDevice> device, const T *data, size_t count,
           MTLResourceOptions options = MTLResourceStorageModeManaged)
        : _device(device), _count(count)
    {
        const size_t bytes = count * sizeof(T);
        _buffer = [_device newBufferWithLength:bytes options:options];

        memcpy([_buffer contents], data, count * sizeof(T));

        [_buffer didModifyRange:NSMakeRange(0, byte_size())];
    }

    Buffer(const Buffer<T> &buffer)
    {
        _buffer = nil;
        _device = nil;

        _buffer = std::move(buffer._buffer);
        _device = std::move(buffer._device);
    }

    ~Buffer()
    {
        _buffer = nil;
        _device = nil;
    }

    const void *data() const
    {
        reinterpret_cast<const T *>([_buffer contents]);
    }

    void *data()
    {
        return reinterpret_cast<T *>([_buffer contents]);
    }

    void update(unsigned int start = 0, unsigned int end = 0)
    {
        if (start == 0 && end == 0)
            [_buffer didModifyRange:NSMakeRange(0, byte_size())];
        else
            [_buffer didModifyRange:NSMakeRange(start * sizeof(T), end * sizeof(T))];
    }

    size_t size() const
    {
        return _count;
    }

    size_t byte_size() const
    {
        return _count * sizeof(T);
    }

    id<MTLDevice> device() const
    {
        return _device;
    }

    id<MTLBuffer> buffer() const
    {
        return _buffer;
    }

  private:
    id<MTLDevice> _device;
    id<MTLBuffer> _buffer;
    size_t _count;
};

#endif // BUFFER_H