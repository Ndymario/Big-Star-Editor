#include "nsmb/formats/narc.hpp"

#include "nsmb/nds/rom.hpp"

#include <algorithm>
#include <limits>

namespace nsmb::formats {
namespace {
std::uint16_t u16(std::span<const std::uint8_t> b,std::size_t o){return b[o]|(b[o+1]<<8U);}
std::uint32_t u32(std::span<const std::uint8_t> b,std::size_t o){return u16(b,o)|(static_cast<std::uint32_t>(u16(b,o+2))<<16U);}
void add16(std::vector<std::uint8_t>& b,std::uint16_t v){b.push_back(v);b.push_back(v>>8);}
void add32(std::vector<std::uint8_t>& b,std::uint32_t v){add16(b,v);add16(b,v>>16);}
void magic(std::vector<std::uint8_t>& b,const char* s){b.insert(b.end(),s,s+4);}
bool at(std::span<const std::uint8_t>b,std::size_t o,const char*s){return o+4<=b.size()&&std::equal(s,s+4,b.begin()+static_cast<std::ptrdiff_t>(o));}
}

bool is_narc(std::span<const std::uint8_t> data){return at(data,0,"NARC");}

core::Result<NarcArchive> parse_narc(std::span<const std::uint8_t> data){
    if(data.size()<16||!is_narc(data)||u16(data,4)!=0xfffe||u16(data,12)!=16||u32(data,8)!=data.size())
        return core::make_error(core::ErrorCode::ParseError,"invalid NARC header");
    std::size_t fat=0,fnt=0,img=0,fnt_size=0,img_size=0,cursor=16;
    std::size_t chunks=0;
    while(cursor<data.size()){
        if(cursor+8>data.size())return core::make_error(core::ErrorCode::ParseError,"truncated NARC chunk header");
        const auto size=static_cast<std::size_t>(u32(data,cursor+4));
        if(size<8||size>data.size()-cursor)return core::make_error(core::ErrorCode::ParseError,"NARC chunk extends past the file");
        if(at(data,cursor,"BTAF")){if(fat)return core::make_error(core::ErrorCode::ParseError,"duplicate NARC FAT");fat=cursor;}
        else if(at(data,cursor,"BTNF")){if(fnt)return core::make_error(core::ErrorCode::ParseError,"duplicate NARC FNT");fnt=cursor;fnt_size=size;}
        else if(at(data,cursor,"GMIF")){if(img)return core::make_error(core::ErrorCode::ParseError,"duplicate NARC image");img=cursor;img_size=size;}
        else return core::make_error(core::ErrorCode::Unsupported,"unknown NARC chunk");
        cursor+=size;++chunks;
    }
    if(cursor!=data.size()||chunks!=u16(data,14)||!fat||!fnt||!img)return core::make_error(core::ErrorCode::ParseError,"NARC is missing a required chunk");
    const auto count=static_cast<std::size_t>(u16(data,fat+8));
    if(12+count*8>u32(data,fat+4))return core::make_error(core::ErrorCode::ParseError,"NARC FAT is truncated");
    NarcArchive out;out.name_table.assign(data.begin()+static_cast<std::ptrdiff_t>(fnt+8),data.begin()+static_cast<std::ptrdiff_t>(fnt+fnt_size));
    auto names=nds::parse_fnt(out.name_table);if(!names)return names.error();
    if(!names.value().empty()&&names.value().back().id>=count)return core::make_error(core::ErrorCode::ParseError,"NARC name table refers past its FAT");
    const auto base=img+8,available=img_size-8;
    for(std::size_t i=0;i<count;++i){const auto start=u32(data,fat+12+i*8),end=u32(data,fat+16+i*8);if(end<start||end>available)return core::make_error(core::ErrorCode::OutOfBounds,"NARC member lies outside its image");out.files.emplace_back(data.begin()+static_cast<std::ptrdiff_t>(base+start),data.begin()+static_cast<std::ptrdiff_t>(base+end));}
    return out;
}

core::Result<std::vector<std::uint8_t>> write_narc(const NarcArchive& a){
    if(a.files.size()>65535)return core::make_error(core::ErrorCode::InvalidArgument,"NARC has more than 65535 members");
    auto names=nds::parse_fnt(a.name_table);if(!names)return names.error();if(!names.value().empty()&&names.value().back().id>=a.files.size())return core::make_error(core::ErrorCode::InvalidArgument,"NARC name table refers past its members");
    std::vector<std::uint8_t> allocation,image;add16(allocation,static_cast<std::uint16_t>(a.files.size()));add16(allocation,0);
    for(const auto& file:a.files){if(image.size()>std::numeric_limits<std::uint32_t>::max()-file.size()-3)return core::make_error(core::ErrorCode::InvalidArgument,"NARC image is too large");add32(allocation,image.size());image.insert(image.end(),file.begin(),file.end());add32(allocation,image.size());image.resize((image.size()+3)&~std::size_t(3),0xff);}
    const auto fat=8+allocation.size(),fnt=8+a.name_table.size(),img=8+image.size(),total=16+fat+fnt+img;if(total>std::numeric_limits<std::uint32_t>::max())return core::make_error(core::ErrorCode::InvalidArgument,"NARC is too large");
    std::vector<std::uint8_t> out;magic(out,"NARC");add16(out,0xfffe);add16(out,0x0100);add32(out,total);add16(out,16);add16(out,3);
    magic(out,"BTAF");add32(out,fat);out.insert(out.end(),allocation.begin(),allocation.end());magic(out,"BTNF");add32(out,fnt);out.insert(out.end(),a.name_table.begin(),a.name_table.end());magic(out,"GMIF");add32(out,img);out.insert(out.end(),image.begin(),image.end());return out;
}

core::Result<std::size_t> NarcArchive::find(const std::filesystem::path& path)const{auto entries=nds::parse_fnt(name_table);if(!entries)return entries.error();const auto found=std::ranges::find(entries.value(),path,&nds::NitroFileEntry::path);if(found==entries.value().end())return core::make_error(core::ErrorCode::NotFound,"NARC member was not found");return found->id;}

} // namespace nsmb::formats
