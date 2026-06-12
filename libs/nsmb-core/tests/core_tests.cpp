#include "nsmb/core/byte_buffer.hpp"
#include "nsmb/core/endian.hpp"
#include <cassert>
#include <vector>
int main() {
    using namespace nsmb::core;
    std::vector<std::uint8_t> data(4);
    LittleEndianWriter writer(data);
    assert(writer.u16(0, 0x1234));
    assert(writer.u32(0, 0x89abcdef));
    LittleEndianReader reader(data);
    assert(reader.u16(0).value() == 0xcdef);
    assert(reader.u32(0).value() == 0x89abcdef);
    assert(!reader.u32(1));
    ByteBuffer buffer({1, 2, 3});
    assert(buffer.at(1).value() == 2);
    assert(!buffer.at(3));
    assert(buffer.slice(1, 2).value().size() == 2);
    std::uint8_t patch[] = {9, 8};
    assert(buffer.write(1, patch));
    assert(buffer.at(1).value() == 9);
    assert(!buffer.write(2, patch));
    return 0;
}
