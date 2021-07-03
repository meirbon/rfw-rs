//
// Created by Mèir Noordermeer on 07/06/2021.
//

#ifndef METALCPP_SRC_UTILS_HPP
#define METALCPP_SRC_UTILS_HPP

#include <fstream>
#include <iostream>
#include <random>
#include <string>

inline unsigned int next_multiple_of(unsigned int count, unsigned int multiple_of)
{
    const unsigned int a = (count + multiple_of - 1) / multiple_of;
    return a * multiple_of;
}

inline std::string random_string(size_t length)
{
    auto randchar = []() -> char {
        const char charset[] = "0123456789"
                               "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
                               "abcdefghijklmnopqrstuvwxyz";
        const size_t max_index = (sizeof(charset) - 1);
        return charset[rand() % max_index];
    };
    std::string str(length, 0);
    std::generate_n(str.begin(), length, randchar);
    return str;
}

inline bool write_bytes(const std::string &file, const void *bytes, size_t num_bytes)
{
    std::ofstream fout;
    fout.open(file, std::ios::binary | std::ios::out);

    if (fout.is_open())
    {
        fout.write(reinterpret_cast<const char *>(bytes), static_cast<std::streamsize>(num_bytes));
        fout.close();
        return true;
    }
    else
    {
        return false;
    }
}

#endif // METALCPP_SRC_UTILS_HPP
