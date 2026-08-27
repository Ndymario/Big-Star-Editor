#include "nsmb/formats/glue.hpp"

#include <cassert>
#include <cstdint>
#include <vector>

namespace {
void put(std::vector<std::uint8_t>& data, std::size_t offset, std::uint32_t value) {
    data[offset]=value;data[offset+1]=value>>8;data[offset+2]=value>>16;data[offset+3]=value>>24;
}
std::vector<std::uint8_t> words(std::initializer_list<std::uint32_t> values) {
    std::vector<std::uint8_t> out(values.size()*4);std::size_t offset=0;
    for(const auto value:values){put(out,offset,value);offset+=4;}return out;
}
}

int main() {
    using namespace nsmb::formats;
    static_assert(fnv1a_32("coop.canFly")==0xc8dfeb91U);
    static_assert(fnv1a_32("coop.CoopFlagActor.Default")==0xf46a44b9U);

    auto ordered=words({0,0x11111111,32,0x22222222,28,0,0,0xbbbbbbbb,0xaaaaaaaa});
    auto parsed=parse_glue_block(ordered);assert(parsed&&parsed.value().values.size()==2);
    assert(parsed.value().values[0].payload[0]==0xaa&&parsed.value().values[1].payload[0]==0xbb);
    auto rewritten=write_glue_block(parsed.value());assert(rewritten&&rewritten.value()==ordered);

    assert(!parse_glue_block(std::vector<std::uint8_t>(13)));
    assert(!parse_glue_block(std::vector<std::uint8_t>(8)));
    assert(!parse_glue_block(words({0xffffffff,0,0})));
    assert(!parse_glue_block(words({0,1,12})));
    assert(!parse_glue_block(words({0,1,16,0,0,0})));
    assert(!parse_glue_block(words({0,1,28,1,32,0,0,0,0})));
    assert(!parse_glue_block(words({0,1,28,2,28,0,0,0})));

    GlueBlock invalid;invalid.flags={0};assert(!write_glue_block(invalid));
    invalid.flags.clear();invalid.values={{0,{1},0}};assert(!write_glue_block(invalid));
    invalid.values={{1,{},0}};assert(!write_glue_block(invalid));

    GlueBlock registration;
    registration.values.push_back({fnv1a_32("glue.stageObjects"),
        words({2,fnv1a_32("coop.CoopFlagActor.Default"),fnv1a_32("coop.CoopFlagActor.Big")}),0});
    const auto worked=words({0,0x6db68f3c,0x14,0,0,2,0xf46a44b9,0x27e61112});
    auto encoded=write_glue_block(registration);assert(encoded&&encoded.value()==worked);
    return 0;
}
