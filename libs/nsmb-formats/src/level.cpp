#include "nsmb/formats/level.hpp"

#include "nsmb/core/endian.hpp"

#include <algorithm>
#include <cstring>

namespace nsmb::formats {
namespace {

std::uint16_t u16(std::span<const std::uint8_t> b, std::size_t o) { return b[o] | (b[o + 1] << 8U); }
std::int16_t i16(std::span<const std::uint8_t> b, std::size_t o) { return static_cast<std::int16_t>(u16(b, o)); }
std::uint32_t u32(std::span<const std::uint8_t> b, std::size_t o) {
    return u16(b, o) | (static_cast<std::uint32_t>(u16(b, o + 2)) << 16U);
}
std::int32_t i32(std::span<const std::uint8_t> b, std::size_t o) { return static_cast<std::int32_t>(u32(b, o)); }

core::Result<std::span<const std::uint8_t>> block(std::span<const std::uint8_t> data, NsmbLevelBlock b) {
    const auto o = static_cast<std::size_t>(b.offset), n = static_cast<std::size_t>(b.size);
    if (o > data.size() || n > data.size() - o)
        return core::make_error(core::ErrorCode::OutOfBounds, "level block extends past the course file");
    return data.subspan(o, n);
}

core::Result<void> records(std::span<const std::uint8_t> b, std::size_t size, const char* name) {
    if (b.size() % size)
        return core::make_error(core::ErrorCode::ParseError, std::string(name) + " block contains a partial record");
    return {};
}

core::Result<std::vector<NsmbLevelPath>> paths(std::span<const std::uint8_t> headers,
                                               std::span<const std::uint8_t> nodes) {
    if (auto r = records(headers, 8, "path"); !r) return r.error();
    if (auto r = records(nodes, 16, "path node"); !r) return r.error();
    std::vector<NsmbLevelPath> out;
    for (std::size_t o = 0; o < headers.size(); o += 8) {
        NsmbLevelPath p;
        p.id = headers[o];
        const auto first = static_cast<std::size_t>(u16(headers, o + 2));
        const auto count = static_cast<std::size_t>(u16(headers, o + 4));
        p.unknown = u16(headers, o + 6);
        const auto available = nodes.size() / 16;
        if (first > available || count > available - first)
            return core::make_error(core::ErrorCode::OutOfBounds, "path references nodes outside its node block");
        for (std::size_t i = 0; i < count; ++i) {
            const auto n = (first + i) * 16;
            p.nodes.push_back({i16(nodes, n), i16(nodes, n + 2),
                               core::Fx32::from_raw(i32(nodes, n + 4)),
                               core::Fx32::from_raw(i32(nodes, n + 8)),
                               u16(nodes, n + 12), u16(nodes, n + 14)});
        }
        out.push_back(std::move(p));
    }
    return out;
}

} // namespace

core::Result<NsmbLevelCourse> parse_nsmb_level_course(std::span<const std::uint8_t> data) {
    constexpr std::size_t table_size = nsmb_level_block_count * 8;
    if (data.size() < table_size)
        return core::make_error(core::ErrorCode::ParseError, "NSMB course file is missing the block table");
    NsmbLevelCourse c;
    for (std::size_t i = 0; i < c.blocks.size(); ++i) {
        c.blocks[i] = {u32(data, i * 8), u32(data, i * 8 + 4)};
        if (auto b = block(data, c.blocks[i]); !b) return b.error();
    }
    std::array<std::span<const std::uint8_t>, nsmb_level_block_count> b;
    for (std::size_t i = 0; i < b.size(); ++i) b[i] = block(data, c.blocks[i]).value();

    if (!b[0].empty() && b[0].size() < 0x20)
        return core::make_error(core::ErrorCode::ParseError, "level header is shorter than 0x20 bytes");
    if (b[0].size() >= 0x20) {
        c.header.entrances = {b[0][0], b[0][1]};
        c.header.settings = u16(b[0], 2); c.header.timer = u16(b[0], 4);
        for (std::size_t i = 0; i < 3; ++i) {
            c.header.background_theme[i] = u16(b[0], 6 + i * 2);
            c.header.tileset_theme[i] = u16(b[0], 12 + i * 2);
            c.header.foreground_theme[i] = u16(b[0], 18 + i * 2);
        }
        c.header.events = u16(b[0], 24); c.header.sound_group = u16(b[0], 26);
        c.header.sprite_priority = u16(b[0], 28); c.header.mega_drop_chance = u16(b[0], 30);
    }

    if (auto r = records(b[1], 24, "camera"); !r) return r.error();
    for (std::size_t o = 0; o < b[1].size(); o += 24)
        c.cameras.push_back({i32(b[1], o), i32(b[1], o+4), i32(b[1], o+8), i32(b[1], o+12), u16(b[1], o+16), u16(b[1], o+18), u32(b[1], o+20)});

    auto read_screens = [&](std::size_t index, std::vector<NsmbLevelScreen>& out) -> core::Result<void> {
        if (auto r = records(b[index], 20, "screen"); !r) return r;
        for (std::size_t o = 0; o < b[index].size(); o += 20)
            out.push_back({u16(b[index],o),u16(b[index],o+2),u16(b[index],o+4),u16(b[index],o+6),u16(b[index],o+8),u16(b[index],o+10),u16(b[index],o+12),u16(b[index],o+14),u16(b[index],o+16),u16(b[index],o+18)});
        return {};
    };
    if (auto r=read_screens(2,c.backgrounds);!r)return r.error();
    if (auto r=read_screens(3,c.tilesets);!r)return r.error();
    if (auto r=read_screens(4,c.foregrounds);!r)return r.error();

    if (auto r=records(b[5],20,"entrance");!r)return r.error();
    for (std::size_t o=0;o<b[5].size();o+=20)
        c.entrances.push_back({u16(b[5],o),u16(b[5],o+2),u16(b[5],o+4),u16(b[5],o+6),b[5][o+8],b[5][o+9],b[5][o+10],b[5][o+11],b[5][o+12],b[5][o+13],b[5][o+14],b[5][o+15],b[5][o+16],b[5][o+17],b[5][o+18],b[5][o+19]});

    if (!b[6].empty() && b[6].size() < 4)
        return core::make_error(core::ErrorCode::ParseError, "stage object block is missing its terminator");
    const auto object_bytes = b[6].empty() ? 0 : b[6].size()-4;
    if (object_bytes % 12) return core::make_error(core::ErrorCode::ParseError,"stage object block contains a partial record");
    for (std::size_t o=0;o<object_bytes;o+=12)
        c.sprites.push_back({u16(b[6],o),u16(b[6],o+2),u16(b[6],o+4),{b[6][o+6],b[6][o+7]},u32(b[6],o+8)});

    if (auto r=records(b[7],16,"view");!r)return r.error();
    for(std::size_t o=0;o<b[7].size();o+=16)
        c.views.push_back({u16(b[7],o),u16(b[7],o+2),u16(b[7],o+4),u16(b[7],o+6),b[7][o+8],b[7][o+9],b[7][o+10],b[7][o+11],b[7][o+12],b[7][o+13],b[7][o+14],b[7][o+15]});
    if (auto r=records(b[8],12,"zone");!r)return r.error();
    for(std::size_t o=0;o<b[8].size();o+=12)
        c.zones.push_back({u16(b[8],o),u16(b[8],o+2),u16(b[8],o+4),u16(b[8],o+6),b[8][o+8],b[8][o+9],b[8][o+10],b[8][o+11]});

    auto pp=paths(b[9],b[11]); if(!pp)return pp.error(); c.progress_paths=std::move(pp.value());
    auto p=paths(b[10],b[12]); if(!p)return p.error(); c.paths=std::move(p.value());
    std::copy_n(b[13].begin(), std::min(b[13].size(), c.object_banks.size()), c.object_banks.begin());
    return c;
}

core::Result<std::vector<NsmbLevelObject>> parse_nsmb_bgdat_objects(std::span<const std::uint8_t> data) {
    if (data.size()%nsmb_bgdat_object_size==2 && data[data.size()-2]==0xFF && data.back()==0xFF) data=data.first(data.size()-2);
    if (data.size()%nsmb_bgdat_object_size)
        return core::make_error(core::ErrorCode::ParseError,"NSMB BGDat object data must be made of 10-byte records");
    std::vector<NsmbLevelObject> out;
    for(std::size_t o=0;o<data.size();o+=10){const auto id=u16(data,o);out.push_back({static_cast<std::uint16_t>(id&0xFFF),static_cast<std::uint8_t>(id>>12),i16(data,o+2),i16(data,o+4),u16(data,o+6),u16(data,o+8)});}
    return out;
}

} // namespace nsmb::formats
