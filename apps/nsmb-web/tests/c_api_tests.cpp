#include "big_star_web.h"

#include "nsmb/core/json.hpp"
#include "nsmb/formats/level.hpp"

#include <cassert>
#include <cstdint>
#include <string>
#include <vector>

namespace {

void put32(std::vector<std::uint8_t>& data,std::size_t offset,std::uint32_t value){for(int shift=0;shift<32;shift+=8)data[offset+shift/8]=static_cast<std::uint8_t>(value>>shift);}

std::vector<std::uint8_t> empty_course(){
    std::vector<std::uint8_t> data(nsmb::formats::nsmb_level_block_count*8U);
    for(std::size_t i=0;i<nsmb::formats::nsmb_level_block_count;++i){put32(data,i*8U,static_cast<std::uint32_t>(data.size()));const std::size_t size=i==0?32U:0U;put32(data,i*8U+4U,static_cast<std::uint32_t>(size));data.resize(data.size()+size);}
    return data;
}

} // namespace

int main(){
    assert(bse_api_version()==0x00010000U);
    const std::vector<std::uint8_t> invalid{1,2,3};
    assert(bse_level_open(invalid.data(),invalid.size(),invalid.data(),invalid.size())==0);
    assert(std::string(bse_last_error()).find("block table")!=std::string::npos);

    auto course=empty_course();const std::vector<std::uint8_t> bgdat{0xff,0xff};
    const auto handle=bse_level_open(course.data(),course.size(),bgdat.data(),bgdat.size());assert(handle);
    auto snapshot=nsmb::core::parse_json(bse_level_snapshot(handle));assert(snapshot&&snapshot.value()["schema"].as_string()=="bigstar.web-level/1");
    assert(snapshot.value()["objects"].size()==0);

    assert(bse_level_add(handle,BSE_OBJECT));
    assert(bse_level_set(handle,BSE_OBJECT,0,0,37));
    assert(bse_level_set(handle,BSE_OBJECT,0,1,2));
    assert(bse_level_set(handle,BSE_OBJECT,0,2,-16));
    assert(!bse_level_set(handle,BSE_OBJECT,0,1,16));
    assert(std::string(bse_last_error()).find("range")!=std::string::npos);
    assert(bse_level_undo(handle));
    assert(bse_level_redo(handle));

    assert(bse_level_serialize(handle));
    assert(bse_level_course_data(handle)&&bse_level_course_size(handle)>=course.size());
    const auto* encoded=bse_level_bgdat_data(handle);const auto size=bse_level_bgdat_size(handle);assert(encoded&&size==12);
    auto objects=nsmb::formats::parse_nsmb_bgdat_objects({encoded,size});assert(objects&&objects.value().size()==1);
    assert(objects.value()[0].object_id==37&&objects.value()[0].tileset_slot==2&&objects.value()[0].x==-16);

    bse_level_close(handle);
    assert(!bse_level_snapshot(handle));
    assert(std::string(bse_last_error()).find("handle")!=std::string::npos);
}
