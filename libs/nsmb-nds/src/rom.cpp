#include "nsmb/nds/rom.hpp"
#include "nsmb/core/endian.hpp"
#include <algorithm>
#include <fstream>
namespace nsmb::nds {
static std::string ascii_field(std::span<const std::uint8_t> data, std::size_t offset, std::size_t length) {
    std::string out(reinterpret_cast<const char*>(data.data() + offset), length);
    while (!out.empty() && (out.back() == '\0' || out.back() == ' ')) out.pop_back();
    return out;
}
core::Result<RomHeader> parse_rom_header(std::span<const std::uint8_t> data) {
    if (data.size() < 0x40) return core::make_error(core::ErrorCode::ParseError, "NDS header needs at least 0x40 bytes");
    core::LittleEndianReader reader(data);
    auto arm9 = reader.u32(0x20); if (!arm9) return arm9.error();
    auto arm7 = reader.u32(0x30); if (!arm7) return arm7.error();
    return RomHeader{ascii_field(data, 0, 12), ascii_field(data, 12, 4), arm9.value(), arm7.value()};
}
core::Result<FileTreeNode> scan_file_tree(const std::filesystem::path& root) {
    std::error_code ec;
    if (!std::filesystem::exists(root, ec)) return core::make_error(core::ErrorCode::NotFound, "file tree root does not exist");
    FileTreeNode node{root.filename().string(), root, std::filesystem::is_directory(root, ec), {}};
    if (node.directory) {
        for (const auto& entry : std::filesystem::directory_iterator(root, ec)) {
            auto child = scan_file_tree(entry.path());
            if (!child) return child.error();
            node.children.push_back(child.value());
        }
        std::sort(node.children.begin(), node.children.end(), [](const auto& a, const auto& b){ return a.name < b.name; });
    }
    return node;
}
}
