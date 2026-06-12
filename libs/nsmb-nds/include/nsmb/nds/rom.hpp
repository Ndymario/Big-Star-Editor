#pragma once
#include <cstdint>
#include <filesystem>
#include <span>
#include <string>
#include <vector>
#include "nsmb/core/result.hpp"
namespace nsmb::nds {
struct RomHeader { std::string game_title; std::string game_code; std::uint32_t arm9_rom_offset{}; std::uint32_t arm7_rom_offset{}; };
struct FileTreeNode { std::string name; std::filesystem::path path; bool directory{}; std::vector<FileTreeNode> children; };
struct ExtractedRom { std::filesystem::path root; FileTreeNode files; };
class RomExtractionService { public: virtual ~RomExtractionService() = default; virtual core::Result<ExtractedRom> extract(const std::filesystem::path& rom, const std::filesystem::path& output_dir) = 0; };
class RomBuildService { public: virtual ~RomBuildService() = default; virtual core::Result<void> build(const std::filesystem::path& extracted_dir, const std::filesystem::path& output_rom) = 0; };
core::Result<RomHeader> parse_rom_header(std::span<const std::uint8_t> data);
core::Result<FileTreeNode> scan_file_tree(const std::filesystem::path& root);
}
