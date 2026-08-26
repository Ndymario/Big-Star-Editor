#include "nsmb/project/editable_level.hpp"
#include "nsmb/project/level_catalog.hpp"
#include "nsmb/project/manifest.hpp"

#include <cassert>
#include <filesystem>
#include <fstream>
#include <sstream>
#include <vector>

namespace {
void put_u32(std::vector<std::uint8_t>& data, std::size_t offset, std::uint32_t value) {
    data[offset] = static_cast<std::uint8_t>(value & 0xFFU);
    data[offset + 1U] = static_cast<std::uint8_t>((value >> 8U) & 0xFFU);
    data[offset + 2U] = static_cast<std::uint8_t>((value >> 16U) & 0xFFU);
    data[offset + 3U] = static_cast<std::uint8_t>((value >> 24U) & 0xFFU);
}

void append_block(std::vector<std::uint8_t>& course, std::size_t index, const std::vector<std::uint8_t>& block) {
    put_u32(course, index * 8U, static_cast<std::uint32_t>(course.size()));
    put_u32(course, index * 8U + 4U, static_cast<std::uint32_t>(block.size()));
    course.insert(course.end(), block.begin(), block.end());
}
} // namespace

int main() {
    namespace fs = std::filesystem;
    const auto dir = fs::temp_directory_path() / "big-star-project-test";
    fs::remove_all(dir);
    fs::create_directories(dir / "extracted-rom/root/course");

    auto project = nsmb::project::create_empty_project("Test \"Project\"\nName", dir).value();
    nsmb::nds::RomHeader project_header;
    project_header.game_title = "TEST \"ROM\"";
    project_header.game_code = "TST1";
    project.attach_extracted_rom(dir / "extracted-rom", project_header);
    project.add_game_file({dir / "extracted-rom/root/course/course1.bin", 12});

    const auto manifest = dir / nsmb::project::manifest_file_name;
    assert(nsmb::project::save_manifest(project, manifest));

    std::ifstream manifest_input(manifest);
    std::stringstream manifest_text;
    manifest_text << manifest_input.rdbuf();
    const auto text = manifest_text.str();
    assert(text.find("format: big-star-editor-project") != std::string::npos);
    assert(text.find("root:") == std::string::npos);
    assert(text.find(dir.string()) == std::string::npos);
    assert(text.find("extracted_path: \"extracted-rom\"") != std::string::npos);
    assert(text.find("name: \"Test \\\"Project\\\"\\nName\"") != std::string::npos);

    auto loaded = nsmb::project::load_manifest(manifest);
    assert(loaded);
    assert(loaded.value().name() == "Test \"Project\"\nName");
    assert(loaded.value().rom());
    assert(loaded.value().rom()->extracted_path == "extracted-rom");
    assert(loaded.value().rom()->header);
    assert(loaded.value().rom()->header->game_title == "TEST \"ROM\"");
    assert(loaded.value().game_files().size() == 1);
    assert(loaded.value().game_files()[0].path == "extracted-rom/root/course/course1.bin");
    assert(!loaded.value().dirty());
    loaded.value().update_game_file_size("extracted-rom/root/course/course1.bin", 99);
    assert(loaded.value().game_files()[0].size == 99);
    assert(loaded.value().dirty());
    loaded.value().mark_clean();

    {
        std::ofstream bad(dir / "bad-size.yaml");
        bad << "format: big-star-editor-project\nversion: 1\nname: \"Bad\"\nrom:\n"
               "  extracted_path: \"extracted-rom\"\nfiles:\n"
               "  - path: \"extracted-rom/root/file.bin\"\n    size: nope\n";
    }
    assert(!nsmb::project::load_manifest(dir / "bad-size.yaml"));
    {
        std::ofstream bad(dir / "bad-path.yaml");
        bad << "format: big-star-editor-project\nversion: 1\nname: \"Bad\"\nrom:\n"
               "  extracted_path: \"../outside\"\n";
    }
    assert(!nsmb::project::load_manifest(dir / "bad-path.yaml"));
    {
        std::ofstream bad(dir / "bad-drive-path.yaml");
        bad << "format: big-star-editor-project\nversion: 1\nname: \"Bad\"\nrom:\n"
               "  extracted_path: \"C:/outside\"\n";
    }
    assert(!nsmb::project::load_manifest(dir / "bad-drive-path.yaml"));
    {
        std::ofstream bad(dir / "bad-version.yaml");
        bad << "format: big-star-editor-project\nversion: 2\nname: \"Bad\"\n";
    }
    assert(!nsmb::project::load_manifest(dir / "bad-version.yaml"));
    assert(!nsmb::project::make_project_relative(dir, {}));

    auto invalid_project = nsmb::project::create_empty_project("Invalid", dir).value();
    invalid_project.attach_extracted_rom("extracted-rom");
    invalid_project.add_game_file({"outside.bin", 1});
    assert(!nsmb::project::save_manifest(invalid_project, dir / "invalid-save.yaml"));

    {
        std::ofstream first(dir / "extracted-rom/root/course/a.bin", std::ios::binary);
        first << "abc";
        std::ofstream second(dir / "extracted-rom/root/course/z.bin", std::ios::binary);
        second << "z";
    }
    assert(nsmb::project::index_project_files(loaded.value(), dir / "extracted-rom"));
    assert(loaded.value().game_files().size() == 2);
    assert(loaded.value().game_files()[0].path == "extracted-rom/root/course/a.bin");
    assert(loaded.value().game_files()[0].size == 3);
    assert(loaded.value().game_files()[1].path == "extracted-rom/root/course/z.bin");
    assert(!nsmb::project::index_project_files(loaded.value(), dir / "missing"));

    const auto& catalog = nsmb::project::nsmb_level_catalog();
    assert(!catalog.empty());
    assert(catalog.front().world == "World 1");
    assert(catalog.front().display_name == "1-1");
    assert(catalog.front().areas.size() == 3);
    assert(catalog.front().areas[0].display_name == "Area 1");
    assert(catalog.front().areas[0].course_token == "A01_1");
    bool saw_download_play_level = false;
    for (const auto& entry : catalog) {
        for (const auto& area : entry.areas) {
            if (area.source_token == "J03_1@Dat_Ice") {
                saw_download_play_level = true;
                assert(area.course_token == "J03_1");
                assert(area.narc_name);
                assert(area.narc_name.value() == "Dat_Ice");
            }
        }
    }
    assert(saw_download_play_level);

    std::vector<std::uint8_t> empty_course(nsmb::formats::nsmb_level_block_count * 8U);
    for (std::size_t i = 0; i < nsmb::formats::nsmb_level_block_count; ++i) {
        put_u32(empty_course, i * 8U, static_cast<std::uint32_t>(empty_course.size()));
    }
    append_block(empty_course, 0, std::vector<std::uint8_t>(0x13U));
    append_block(empty_course, 2, std::vector<std::uint8_t>(3U));
    auto editable = nsmb::project::EditableLevel::load(empty_course, {});
    assert(editable);
    auto object_index = editable.value().add_object({12, 1, -2, 3, 4, 5});
    assert(object_index && object_index.value() == 0);
    assert(editable.value().objects()[0].x == -2);
    auto object = editable.value().objects()[0];
    object.x = 24;
    assert(editable.value().update_object(0, object));
    assert(editable.value().objects()[0].x == 24);
    assert(editable.value().undo());
    assert(editable.value().objects()[0].x == -2);
    assert(editable.value().undo());
    assert(editable.value().objects().empty());
    assert(editable.value().redo());
    assert(editable.value().objects().size() == 1);
    object = editable.value().objects()[0];
    object.x = 42;
    assert(editable.value().update_object(0, object));
    for (int i = 0; i < 32; ++i) {
        assert(editable.value().add_object({}));
    }
    for (int i = 0; i < 32; ++i) {
        assert(editable.value().undo());
    }
    assert(editable.value().undo());
    assert(editable.value().objects()[0].x == -2);

    auto sprite_index = editable.value().add_sprite({3, 10, 20, {1, 2, 3, 4, 5, 6}});
    assert(sprite_index && editable.value().course().sprites[0].type == 3);
    auto entrance_index = editable.value().add_entrance({});
    assert(entrance_index && editable.value().course().entrances.size() == 1);
    auto zone_index = editable.value().add_zone({16, 32, 256, 192, 2});
    assert(zone_index && editable.value().course().zones[0].number == 2);

    nsmb::formats::NsmbLevelPath path;
    path.id = 7;
    path.nodes.push_back({32, 48, {1, 2, 3, 4, 5, 6}});
    assert(editable.value().add_path(false, path));
    assert(editable.value().course().paths[0].nodes[0].values[5] == 6);
    assert(editable.value().set_tileset_references({9, 8, 7, 6}));
    assert(editable.value().course().tileset_id == 9);
    assert(editable.value().dirty());
    auto saved_course = editable.value().serialize_course();
    auto saved_bgdat = editable.value().serialize_bgdat();
    assert(saved_course && saved_bgdat);
    assert(saved_bgdat.value().size() == editable.value().objects().size() * nsmb::formats::nsmb_bgdat_object_size + 2U);
    assert(saved_bgdat.value()[saved_bgdat.value().size() - 2U] == 0xFFU);
    auto round_trip = nsmb::project::EditableLevel::load(saved_course.value(), saved_bgdat.value());
    assert(round_trip);
    assert(round_trip.value().course().tileset_id == 9);
    assert(round_trip.value().course().sprites.size() == 1);
    assert(round_trip.value().course().entrances.size() == 1);
    assert(round_trip.value().course().zones.size() == 1);
    assert(round_trip.value().course().paths[0].nodes[0].values[5] == 6);
    assert(round_trip.value().objects()[0].x == -2);
    editable.value().mark_clean();
    assert(!editable.value().dirty());

    auto outside = nsmb::project::make_project_relative(dir, fs::temp_directory_path() / "outside-file.bin");
    assert(!outside);

    fs::remove_all(dir);
    return 0;
}
