#include "nsmb/formats/tex0.hpp"
#include "nsmb/core/file_io.hpp"

#include <cassert>
#include <filesystem>
#include <iostream>

int main(){
    using namespace nsmb::formats;
    // The reference module is a private cross-check and is never committed, so these
    // round trips run only where it happens to be checked out beside the repository.
    // Everything below this block is synthetic and always runs.
    const auto fixture=std::filesystem::path(__FILE__).parent_path()/"../../../deps/nsmb-coop-module/modules/coop/nitrofs/en/enemy/koopa_new.nsbmd";
    if(std::filesystem::exists(fixture)){
        auto bytes=nsmb::core::read_file(fixture);assert(bytes);auto tex0=parse_tex0(bytes.value());assert(tex0&&tex0.value().textures.size()==6&&!tex0.value().palettes.empty());
        assert(tex0.value().textures[0].name=="koopa_all");assert(write_tex0(tex0.value()).value()==bytes.value());
        auto edited=tex0.value();edited.textures[0].texels[0]^=1;auto written=write_tex0(edited);assert(written&&written.value()!=bytes.value());auto reparsed=parse_tex0(written.value());assert(reparsed&&reparsed.value().textures[0].texels[0]==edited.textures[0].texels[0]);
        edited.textures[0].width*=2;assert(!write_tex0(edited));

        const auto assets=std::filesystem::path(__FILE__).parent_path()/"../../../deps/nsmb-coop-module/modules";std::size_t checked=0;
        for(const auto& entry:std::filesystem::recursive_directory_iterator(assets))if(entry.path().extension()==".nsbmd"){auto asset=nsmb::core::read_file(entry.path());assert(asset);auto parsed=parse_tex0(asset.value());if(parsed){assert(write_tex0(parsed.value()).value()==asset.value());++checked;}}
        assert(checked>=10);
    }else{
        std::cerr<<"tex0: reference module absent, skipping ROM-asset round trips\n";
    }

    RgbaImage image{8,4,std::vector<std::uint8_t>(8*4*4,255)};
    for(std::size_t y=0;y<4;++y)for(std::size_t x=0;x<8;++x){const auto pixel=(y*8+x)*4;image.pixels[pixel]=static_cast<std::uint8_t>(x*31);image.pixels[pixel+1]=static_cast<std::uint8_t>(y*63);image.pixels[pixel+2]=static_cast<std::uint8_t>((x+y)*20);image.pixels[pixel+3]=(x==0&&y==0)?0:255;}
    auto compressed=compress_tex4x4(image);assert(compressed&&compressed.value().texels.size()==8&&compressed.value().palette_indices.size()==4&&!compressed.value().palette.empty());
    Tex0Texture texture{"test",TextureFormat::Compressed4x4,8,4,8,4,false,compressed.value().texels,compressed.value().palette_indices};Tex0Palette palette{"test",0,compressed.value().palette};auto rendered=render_tex0_texture(texture,palette);assert(rendered&&rendered.value().width==8&&rendered.value().pixels[3]==0&&rendered.value().pixels[(7*4)+3]==255);

    Tex0Texture direct{"direct",TextureFormat::Direct,8,8,1,1,false,std::vector<std::uint8_t>(128),{}};direct.texels[0]=0x1f;direct.texels[1]=0x80;auto directImage=render_tex0_texture(direct,{});assert(directImage&&directImage.value().pixels[0]==255&&directImage.value().pixels[3]==255);
    assert(!compress_tex4x4(RgbaImage{3,4,std::vector<std::uint8_t>(48)}));
    return 0;
}
