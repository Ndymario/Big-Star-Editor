#include "nsmb/formats/g2d.hpp"
#include "nsmb/formats/graphics.hpp"

#include <cassert>

int main(){
    using namespace nsmb::formats;
    const Nitro2dBlock label{{'L','B','A','L'},{1,2,3,4}};

    Nclr nclr;nclr.format=3;nclr.extended_palette=true;nclr.palette={0,0,0x1f,0};nclr.compressed_palette_indices=std::vector<std::uint16_t>{2,7};nclr.file.blocks.push_back(label);
    auto nclrBytes=write_nclr(nclr);assert(nclrBytes&&identify_graphics_format(nclrBytes.value())==GraphicsFormatKind::NitroPalette);
    auto parsedNclr=parse_nclr(nclrBytes.value());assert(parsedNclr&&parsedNclr.value().format==3&&parsedNclr.value().extended_palette&&parsedNclr.value().palette==nclr.palette&&*parsedNclr.value().compressed_palette_indices==*nclr.compressed_palette_indices);
    assert(write_nclr(parsedNclr.value()).value()==nclrBytes.value());

    Ncgr ncgr;ncgr.height=2;ncgr.width=3;ncgr.pixel_format=4;ncgr.mapping_type=1;ncgr.character_format=0x100;ncgr.characters.assign(64,0x5a);ncgr.position=NcgrPosition{1,2,3,4};ncgr.file.blocks.push_back(label);
    auto ncgrBytes=write_ncgr(ncgr);assert(ncgrBytes);auto parsedNcgr=parse_ncgr(ncgrBytes.value());assert(parsedNcgr&&parsedNcgr.value().height==2&&parsedNcgr.value().width==3&&parsedNcgr.value().characters==ncgr.characters&&parsedNcgr.value().position->height==4);assert(write_ncgr(parsedNcgr.value()).value()==ncgrBytes.value());

    Nscr nscr;nscr.width=256;nscr.height=192;nscr.color_mode=1;nscr.screen_format=0;nscr.screen={0,1,2,3};nscr.file.blocks.push_back(label);
    auto nscrBytes=write_nscr(nscr);assert(nscrBytes);auto parsedNscr=parse_nscr(nscrBytes.value());assert(parsedNscr&&parsedNscr.value().width==256&&parsedNscr.value().screen==nscr.screen);assert(write_nscr(parsedNscr.value()).value()==nscrBytes.value());

    Ncer ncer;ncer.mapping_mode=2;ncer.cells={NcerCell{5,{{1,2,3}},NcerBounds{8,9,-8,-9}},NcerCell{6,{{4,5,6},{7,8,9}},std::nullopt}};ncer.maximum_transfer_size=128;ncer.transfers={{0,64},{64,128}};ncer.file.blocks.push_back(label);
    auto ncerBytes=write_ncer(ncer);assert(ncerBytes&&identify_graphics_format(ncerBytes.value())==GraphicsFormatKind::NitroCellLayout);auto parsedNcer=parse_ncer(ncerBytes.value());assert(parsedNcer&&parsedNcer.value().cells.size()==2&&parsedNcer.value().cells[0].bounds->min_x==-8&&parsedNcer.value().cells[1].oam[1].attr2==9&&parsedNcer.value().transfers[1].size==128);assert(write_ncer(parsedNcer.value()).value()==ncerBytes.value());
    Ncer renderCells;renderCells.cells={NcerCell{0,{{0x2000,0,0}},std::nullopt}};std::vector<std::uint8_t> character(64);character[0]=1;std::vector<Bgr555Color> cellPalette(2);cellPalette[1]={255,0,0,false};auto renderedCell=render_ncer_cell(renderCells,0,character,cellPalette);assert(renderedCell&&renderedCell.value().width==8&&renderedCell.value().height==8&&renderedCell.value().pixels[0]==255&&renderedCell.value().pixels[3]==255);

    Nanr nanr;nanr.sequences={
        NanrSequence{0,0x00010000,1,{{2,0xbeef,{3,0}}}},
        NanrSequence{1,0x00010001,2,{{4,0xbeef,std::vector<std::uint8_t>(16,5)}}},
        NanrSequence{0,0x00010002,3,{{6,0xbeef,std::vector<std::uint8_t>(8,7)}}}
    };nanr.file.blocks.push_back(label);
    auto nanrBytes=write_nanr(nanr);assert(nanrBytes&&identify_graphics_format(nanrBytes.value())==GraphicsFormatKind::NitroCellAnimation);auto parsedNanr=parse_nanr(nanrBytes.value());assert(parsedNanr&&parsedNanr.value().sequences.size()==3&&parsedNanr.value().sequences[1].frames[0].content.size()==16&&parsedNanr.value().sequences[2].frames[0].duration==6);assert(write_nanr(parsedNanr.value()).value()==nanrBytes.value());

    auto malformed=nclrBytes.value();malformed[8]=0xff;assert(!parse_nclr(malformed));malformed=ncerBytes.value();malformed[28]=0xff;malformed[29]=0xff;malformed[30]=0xff;malformed[31]=0x7f;assert(!parse_ncer(malformed));
    return 0;
}
