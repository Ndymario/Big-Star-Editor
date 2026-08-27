#include "nsmb/project/editable_level.hpp"
#include "nsmb/project/graphics_resources.hpp"
#include "nsmb/project/level_catalog.hpp"
#include "nsmb/project/manifest.hpp"
#include "nsmb/project/patch_apply.hpp"

#include <algorithm>
#include <array>
#include <cassert>
#include <cstdint>
#include <span>
#include <string>
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

std::vector<std::uint8_t> make_fnt() {
    std::vector<std::uint8_t> fnt(8, 0);
    put_u32(fnt, 0, 8);
    fnt[6] = 1;
    const std::string name = "graphic.bin";
    fnt.push_back(static_cast<std::uint8_t>(name.size()));
    fnt.insert(fnt.end(), name.begin(), name.end());
    fnt.push_back(0);
    return fnt;
}
} // namespace


// ---------------------------------------------------------------------------
// Transactional patch apply
//
// A build that fails, or that produces something unusable, must leave the
// project exactly as it was. These tests stand in for NCPatcher by re-running
// this test binary, so they need no toolchain and no ROM.
// ---------------------------------------------------------------------------

namespace apply_test {

namespace fs = std::filesystem;

fs::path self_path;

void put_u16(std::vector<std::uint8_t>& data, std::size_t offset, std::uint16_t value) {
    data[offset] = static_cast<std::uint8_t>(value & 0xFF);
    data[offset + 1] = static_cast<std::uint8_t>((value >> 8) & 0xFF);
}

void put_u32(std::vector<std::uint8_t>& data, std::size_t offset, std::uint32_t value) {
    for (int i = 0; i < 4; ++i) {
        data[offset + static_cast<std::size_t>(i)] =
            static_cast<std::uint8_t>((value >> (8 * i)) & 0xFF);
    }
}

std::vector<std::uint8_t> make_fnt() {
    std::vector<std::uint8_t> fnt(8, 0);
    put_u32(fnt, 0, 8);
    put_u16(fnt, 4, 0);
    put_u16(fnt, 6, 1);
    const std::string file_name = "course.bin";
    fnt.push_back(static_cast<std::uint8_t>(file_name.size()));
    fnt.insert(fnt.end(), file_name.begin(), file_name.end());
    fnt.push_back(0);
    return fnt;
}

std::vector<std::uint8_t> make_rom() {
    constexpr std::uint32_t arm9_offset = 0x4000;
    constexpr std::uint32_t arm9_size = 0x20;
    constexpr std::uint32_t arm7_offset = 0x8000;
    constexpr std::uint32_t arm7_size = 0x20;
    constexpr std::uint32_t fnt_offset = 0x8400;
    constexpr std::uint32_t fat_offset = 0x8500;
    constexpr std::uint32_t file_offset = 0x9000;
    const std::vector<std::uint8_t> file_data{'L', 'E', 'V', 'E', 'L'};
    auto fnt = make_fnt();

    std::vector<std::uint8_t> rom(0x9200, 0xFF);
    std::fill_n(rom.begin(), nsmb::nds::ds_cartridge_header_size, 0);
    const std::string title = "TEST ROM";
    std::copy(title.begin(), title.end(), rom.begin());
    rom[0x00C] = 'T'; rom[0x00D] = 'E'; rom[0x00E] = 'S'; rom[0x00F] = 'T';
    rom[0x010] = '0'; rom[0x011] = '1'; rom[0x014] = 7;

    put_u32(rom, 0x020, arm9_offset);
    put_u32(rom, 0x024, 0x02000000);
    put_u32(rom, 0x028, 0x02000000);
    put_u32(rom, 0x02C, arm9_size);
    put_u32(rom, 0x030, arm7_offset);
    put_u32(rom, 0x034, 0x02001000);
    put_u32(rom, 0x038, 0x02001000);
    put_u32(rom, 0x03C, arm7_size);
    put_u32(rom, 0x040, fnt_offset);
    put_u32(rom, 0x044, static_cast<std::uint32_t>(fnt.size()));
    put_u32(rom, 0x048, fat_offset);
    put_u32(rom, 0x04C, 8);
    put_u32(rom, 0x080, static_cast<std::uint32_t>(rom.size()));
    put_u32(rom, 0x084, nsmb::nds::default_header_region_size);

    for (std::uint32_t i = 0; i < arm9_size; ++i) {
        rom[arm9_offset + i] = static_cast<std::uint8_t>(0x90 + i);
    }
    for (std::uint32_t i = 0; i < arm7_size; ++i) {
        rom[arm7_offset + i] = static_cast<std::uint8_t>(0x70 + i);
    }
    std::copy(fnt.begin(), fnt.end(), rom.begin() + fnt_offset);
    put_u32(rom, fat_offset, file_offset);
    put_u32(rom, fat_offset + 4, file_offset + static_cast<std::uint32_t>(file_data.size()));
    std::copy(file_data.begin(), file_data.end(), rom.begin() + file_offset);

    put_u16(rom, 0x06C, nsmb::nds::crc16(std::span<const std::uint8_t>(rom.data() + arm9_offset,
                                                                       0x8000 - arm9_offset)));
    put_u16(rom, 0x15E, nsmb::nds::crc16(std::span<const std::uint8_t>(rom.data(), 0x15E)));
    return rom;
}

// Lays out a project the way the apply expects to find one: an ncpatcher.yaml,
// and a live extracted tree with a marker file that must survive a failure and
// must be gone after a success.
fs::path make_project(const std::string& name) {
    const auto root = fs::temp_directory_path() / ("big-star-apply-" + name);
    fs::remove_all(root);
    fs::create_directories(root);
    { std::ofstream(root / "ncpatcher.yaml") << "version: 2\n"; }
    fs::create_directories(root / "extracted-rom" / "root");
    { std::ofstream(root / "extracted-rom" / "marker.txt") << "the tree that was here before\n"; }
    return root;
}

bool live_tree_is_untouched(const fs::path& project) {
    return fs::exists(project / "extracted-rom" / "marker.txt");
}

nsmb::project::PatchApplyRequest request_for(const fs::path& project, const std::string& mode) {
    nsmb::project::PatchApplyRequest request;
    request.project_dir = project;
    request.extracted_dir = project / "extracted-rom";
    // The stand-in reads its behaviour out of the argument vector NCPatcher
    // would have received, so the real command construction is exercised too.
    request.ncpatcher_executable = self_path;
    request.variant = mode;
    return request;
}

void test_build_failure_changes_nothing() {
    using nsmb::project::PatchApplyOutcome;
    const auto project = make_project("build-failed");
    nsmb::nds::NitroRomService service;

    auto result = nsmb::project::apply_patch_transactionally(
        request_for(project, "fake-fail"), service);
    assert(result);
    assert(result.value().outcome == PatchApplyOutcome::BuildFailed);
    assert(!result.value().summary.succeeded());
    assert(result.value().summary.exit_code == 6);
    assert(result.value().backup_dir.empty());

    // Nothing moved, and no staging directory left to confuse the next run.
    assert(live_tree_is_untouched(project));
    assert(!fs::exists(nsmb::project::patch_staging_dir(project)));
    fs::remove_all(project);
}

void test_success_without_a_rom_is_rejected() {
    using nsmb::project::PatchApplyOutcome;
    const auto project = make_project("no-rom");
    nsmb::nds::NitroRomService service;

    // NCPatcher exits zero but writes nothing. Trusting the exit code alone
    // would replace the project's tree with an extraction of a file that is
    // not there.
    auto result = nsmb::project::apply_patch_transactionally(
        request_for(project, "fake-ok-no-rom"), service);
    assert(result);
    assert(result.value().outcome == PatchApplyOutcome::OutputRejected);
    assert(!result.value().rejection.empty());
    assert(live_tree_is_untouched(project));
    assert(!fs::exists(nsmb::project::patch_staging_dir(project)));
    fs::remove_all(project);
}

void test_unparseable_output_is_rejected() {
    using nsmb::project::PatchApplyOutcome;
    const auto project = make_project("bad-rom");
    nsmb::nds::NitroRomService service;

    // A successful build that produced something that is not an NDS ROM. The
    // live tree must survive that too.
    auto result = nsmb::project::apply_patch_transactionally(
        request_for(project, "fake-ok-bad-rom"), service);
    assert(result);
    assert(result.value().outcome == PatchApplyOutcome::OutputRejected);
    assert(result.value().rejection.find("NDS ROM") != std::string::npos ||
           result.value().rejection.find("could not be") != std::string::npos);
    assert(live_tree_is_untouched(project));
    assert(!fs::exists(nsmb::project::patch_staging_dir(project)));
    fs::remove_all(project);
}

void test_successful_build_swaps_the_tree() {
    using nsmb::project::PatchApplyOutcome;
    const auto project = make_project("applied");
    nsmb::nds::NitroRomService service;

    auto result = nsmb::project::apply_patch_transactionally(
        request_for(project, "fake-ok"), service);
    assert(result);
    assert(result.value().outcome == PatchApplyOutcome::Applied);
    assert(result.value().summary.succeeded());

    // The new tree is in place...
    assert(fs::exists(project / "extracted-rom" / "header.bin"));
    assert(fs::exists(project / "extracted-rom" / "root" / "course.bin"));
    // ...and the old one was moved aside rather than deleted, so a bad build is
    // recoverable by hand.
    assert(!result.value().backup_dir.empty());
    assert(fs::exists(result.value().backup_dir / "marker.txt"));
    assert(!fs::exists(project / "extracted-rom" / "marker.txt"));

    // The built ROM is kept so it can be run without rebuilding.
    assert(fs::exists(result.value().built_rom));
    assert(!fs::exists(nsmb::project::patch_staging_dir(project)));
    fs::remove_all(project);
}

void test_legacy_and_missing_projects_are_refused() {
    nsmb::nds::NitroRomService service;

    const auto legacy = fs::temp_directory_path() / "big-star-apply-legacy";
    fs::remove_all(legacy);
    fs::create_directories(legacy);
    { std::ofstream(legacy / "ncpatcher.json") << "{}\n"; }
    auto request = request_for(legacy, "fake-ok");
    auto refused = nsmb::project::apply_patch_transactionally(request, service);
    assert(!refused && refused.error().code == nsmb::core::ErrorCode::Unsupported);
    fs::remove_all(legacy);

    nsmb::project::PatchApplyRequest missing;
    missing.project_dir = fs::temp_directory_path() / "big-star-apply-nowhere";
    auto absent = nsmb::project::apply_patch_transactionally(missing, service);
    assert(!absent && absent.error().code == nsmb::core::ErrorCode::NotFound);
}

// Stands in for NCPatcher. Reads `--variant` for which behaviour to act out and
// `--out` for where a build would have written, then writes the run result the
// client reads back.
int fake_ncpatcher(int argc, char** argv) {
    std::string mode;
    std::string out;
    std::string result_path;
    for (int i = 1; i < argc; ++i) {
        const std::string argument = argv[i];
        if (argument == "--variant" && i + 1 < argc) {
            mode = argv[++i];
        } else if (argument == "--out" && i + 1 < argc) {
            out = argv[++i];
        } else if (argument == "--result" && i + 1 < argc) {
            result_path = argv[++i];
        }
    }

    const bool ok = mode != "fake-fail";
    const int exit_code = ok ? 0 : 6;

    if (!result_path.empty()) {
        std::ofstream file(result_path);
        file << R"({"schema":"ncpatcher.result/1","status":")" << (ok ? "ok" : "error")
             << R"(","exit-code":)" << exit_code
             << R"(,"duration-ms":1,"errors":)" << (ok ? 0 : 1)
             << R"(,"warnings":0,"diagnostics":[],"artifacts":[]})";
    }

    if (ok && !out.empty() && mode != "fake-ok-no-rom") {
        fs::create_directories(fs::path(out).parent_path());
        std::ofstream rom(out, std::ios::binary);
        if (mode == "fake-ok-bad-rom") {
            rom << "this is not a ROM";
        } else {
            const auto bytes = make_rom();
            rom.write(reinterpret_cast<const char*>(bytes.data()),
                      static_cast<std::streamsize>(bytes.size()));
        }
    }
    return exit_code;
}

void run_all() {
    test_build_failure_changes_nothing();
    test_success_without_a_rom_is_rejected();
    test_unparseable_output_is_rejected();
    test_successful_build_swaps_the_tree();
    test_legacy_and_missing_projects_are_refused();
}

} // namespace apply_test

int main(int argc, char** argv) {
    // Re-invoked as a stand-in for NCPatcher by the transactional-apply tests.
    if (argc > 1) {
        return apply_test::fake_ncpatcher(argc, argv);
    }
    apply_test::self_path = std::filesystem::absolute(argv[0]);

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
    assert(nsmb::project::nsmb_graphics_profile("A2DE"));
    assert(nsmb::project::nsmb_graphics_profile("A2DE")->file_id_offset == 131);
    assert(nsmb::project::nsmb_graphics_profile("A2DP"));
    assert(nsmb::project::nsmb_graphics_profile("A2DP")->file_id_offset == 135);
    assert(nsmb::project::nsmb_graphics_profile("A2DJ"));
    assert(nsmb::project::nsmb_graphics_profile("A2DK"));
    assert(nsmb::project::nsmb_graphics_profile("A2DC"));
    assert(!nsmb::project::nsmb_graphics_profile("TST1"));
    auto resolved_file = nsmb::project::resolve_nitro_file_id(dir / "extracted-rom", make_fnt(), 0);
    assert(resolved_file && resolved_file.value() == dir / "extracted-rom/root/graphic.bin");
    assert(!nsmb::project::resolve_nitro_file_id(dir / "extracted-rom", make_fnt(), 1));

    std::vector<std::uint8_t> empty_course(nsmb::formats::nsmb_level_block_count * 8U);
    for (std::size_t i = 0; i < nsmb::formats::nsmb_level_block_count; ++i) {
        put_u32(empty_course, i * 8U, static_cast<std::uint32_t>(empty_course.size()));
    }
    append_block(empty_course, 0, std::vector<std::uint8_t>(0x20U));
    append_block(empty_course, 2, std::vector<std::uint8_t>(20U));
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

    auto sprite_index = editable.value().add_sprite({3, 10, 20, {1, 2}, 0x06050403});
    assert(sprite_index && editable.value().course().sprites[0].id == 3);
    auto entrance_index = editable.value().add_entrance({});
    assert(entrance_index && editable.value().course().entrances.size() == 1);
    auto zone_index = editable.value().add_zone({16, 32, 256, 192, 2});
    assert(zone_index && editable.value().course().zones[0].id == 2);
    auto view_index=editable.value().add_view({0,0,256,192,1,2,3,4,5,6,7,8});
    assert(view_index&&editable.value().course().views[0].background_id==4);
    assert(editable.value().set_object_bank(15,3));

    nsmb::project::LevelEntityUpdates compound;
    auto moved_object = editable.value().objects()[0];
    moved_object.x = 100;
    auto moved_zone = editable.value().course().zones[0];
    moved_zone.x = 200;
    compound.objects.push_back({0, moved_object});
    compound.zones.push_back({0, moved_zone});
    assert(editable.value().update_entities(std::move(compound), "Move selection"));
    assert(editable.value().objects()[0].x == 100 && editable.value().course().zones[0].x == 200);
    assert(editable.value().undo_name() == "Move selection");
    assert(editable.value().undo());
    assert(editable.value().objects()[0].x == -2 && editable.value().course().zones[0].x == 16);
    assert(editable.value().redo());
    assert(editable.value().undo());

    nsmb::formats::NsmbLevelPath path;
    path.id = 7;
    path.nodes.push_back({32,48,nsmb::core::Fx32::from_raw(1),nsmb::core::Fx32::from_raw(2),5,6});
    assert(editable.value().add_path(false, path));
    assert(editable.value().course().paths[0].nodes[0].flags == 6);
    assert(editable.value().set_tileset_references({9, 8, 7, 6, 5}));
    assert(editable.value().course().header.tileset_theme[0] == 9);
    assert(editable.value().course().foregrounds[0].screen_id == 5);
    assert(editable.value().dirty());
    auto saved_course = editable.value().serialize_course();
    auto saved_bgdat = editable.value().serialize_bgdat();
    assert(saved_course && saved_bgdat);
    assert(saved_bgdat.value().size() == editable.value().objects().size() * nsmb::formats::nsmb_bgdat_object_size + 2U);
    assert(saved_bgdat.value()[saved_bgdat.value().size() - 2U] == 0xFFU);
    auto round_trip = nsmb::project::EditableLevel::load(saved_course.value(), saved_bgdat.value());
    assert(round_trip);
    assert(round_trip.value().course().header.tileset_theme[0] == 9);
    assert(round_trip.value().course().foregrounds[0].screen_id == 5);
    assert(round_trip.value().course().sprites.size() == 1);
    assert(round_trip.value().course().entrances.size() == 1);
    assert(round_trip.value().course().zones.size() == 1);
    assert(round_trip.value().course().views.size()==1&&round_trip.value().course().views[0].progress_path_id==8);
    assert(round_trip.value().course().object_banks[15]==3);
    assert(round_trip.value().course().paths[0].nodes[0].flags == 6);
    assert(round_trip.value().objects()[0].x == -2);

    nsmb::project::ModuleDataCatalog custom_catalog;
    custom_catalog.stage_objects = {
        {"mod.Actor.First", "mod", "actors", "Actor", "First", "Info", "actor.hpp", 0x11111111U, 400},
        {"mod.Actor.Second", "mod", "actors", "Actor", "Second", "Info", "actor.hpp", 0x22222222U, 401},
    };
    auto custom = nsmb::project::EditableLevel::load(empty_course, {}, &custom_catalog);
    assert(custom);
    nsmb::formats::GlueBlock unknown_glue;
    unknown_glue.flags.push_back(0xabcdef01U);
    unknown_glue.values.push_back({0xabcdef01U, {1, 2, 3, 4}, 0});
    assert(custom.value().set_glue(unknown_glue));
    assert(custom.value().add_sprite({1, 0, 0, {}}));
    assert(custom.value().add_custom_sprite(0x22222222U, {0, 1, 1, {}}));
    assert(custom.value().add_custom_sprite(0x11111111U, {0, 2, 2, {}}));
    assert(custom.value().add_custom_sprite(0x22222222U, {0, 3, 3, {}}));
    assert((custom.value().registration_hashes() == std::vector<std::uint32_t>{0x22222222U, 0x11111111U}));
    const std::array<std::uint16_t, 2> vanilla_objects{100, 101};
    assert(custom.value().resolve_sprite_object_id(0, vanilla_objects, custom_catalog) == 101);
    assert(custom.value().resolve_sprite_object_id(1, vanilla_objects, custom_catalog) == 401);
    auto custom_saved = custom.value().serialize_course(); assert(custom_saved);
    auto custom_loaded = nsmb::project::EditableLevel::load(custom_saved.value(), {}, &custom_catalog); assert(custom_loaded);
    assert(custom_loaded.value().glue().flags == unknown_glue.flags);
    assert(custom_loaded.value().glue().values.front().hash == 0xabcdef01U);
    assert(custom_loaded.value().glue().values.front().payload == unknown_glue.values.front().payload);
    assert(custom_loaded.value().sprite_hash(1) == 0x22222222U);
    assert(custom_loaded.value().sprite_hash(2) == 0x11111111U);
    assert(custom_loaded.value().remove_sprite(3));
    assert(custom_loaded.value().remove_sprite(1));
    assert((custom_loaded.value().registration_hashes() == std::vector<std::uint32_t>{0x11111111U}));
    assert(custom_loaded.value().course().sprites[1].id == 326);

    std::vector<std::uint8_t> malformed_course(nsmb::formats::nsmb_level_block_count * 8U);
    for (std::size_t i = 0; i < nsmb::formats::nsmb_level_block_count; ++i)
        put_u32(malformed_course, i * 8U, static_cast<std::uint32_t>(malformed_course.size()));
    auto malformed_header = std::vector<std::uint8_t>(0x20U + 12U);
    put_u32(malformed_header, 0x20U, 0); put_u32(malformed_header, 0x24U, 1); put_u32(malformed_header, 0x28U, 12);
    append_block(malformed_course, 0, malformed_header);
    append_block(malformed_course, 2, std::vector<std::uint8_t>(20U));
    auto malformed = nsmb::project::EditableLevel::load(malformed_course, {}); assert(malformed && malformed.value().glue_error());
    auto malformed_saved = malformed.value().serialize_course(); assert(malformed_saved);
    const auto malformed_offset = static_cast<std::size_t>(malformed_saved.value()[0]) |
        (static_cast<std::size_t>(malformed_saved.value()[1]) << 8U) |
        (static_cast<std::size_t>(malformed_saved.value()[2]) << 16U) |
        (static_cast<std::size_t>(malformed_saved.value()[3]) << 24U);
    assert(std::equal(malformed_header.begin(), malformed_header.end(), malformed_saved.value().begin() + static_cast<std::ptrdiff_t>(malformed_offset)));
    editable.value().mark_clean();
    assert(!editable.value().dirty());

    std::vector<std::uint8_t> bank_table(8+16*10*4,0);
    const std::array<std::array<std::uint32_t,10>,10> banks{{{{22,23,24,25,26,27,28,29,30,31}},{{32,33,34,35,36,37,38,39,40,41}},{{56,57,58,59,60,61,62,63,64,65}},{{66,67,68,69,70,71,72,73,74,75}},{{42,43,44,45,46,47,48,49,50,51}},{{76,77,78,79,80,81,82,83,84,85}},{{86,87,88,89,90,91,92,93,94,95}},{{96,97,98,99,100,101,102,103,104,105}},{{106,107,108,109,110,111,112,113,114,115}},{{116,117,118,119,120,121,122,123,124,125}}}};
    for(std::size_t s=0;s<10;++s)for(std::size_t b=0;b<10;++b)put_u32(bank_table,8+(s*10+b)*4,banks[s][b]);for(std::size_t b=0;b<10;++b)put_u32(bank_table,8+(15*10+b)*4,12+b);
    auto resolved_banks=nsmb::project::find_object_bank_overlays(bank_table);assert(resolved_banks&&resolved_banks.value()[15][9]==21);

    // Reference module assets are a private cross-check and never committed; run the
    // model-backed actor preview only where that tree happens to sit beside the repo.
    const auto actor_model=fs::path(__FILE__).parent_path()/"../../../deps/nsmb-coop-module/modules/coop/nitrofs/en/enemy/koopa_new.nsbmd";
    if(fs::exists(actor_model)){nsmb::project::ActorResourceFiles actor;actor.model=actor_model;auto actor_image=nsmb::project::load_actor_graphics(actor);assert(actor_image&&actor_image.value().width&&actor_image.value().height);}

    auto outside = nsmb::project::make_project_relative(dir, fs::temp_directory_path() / "outside-file.bin");
    assert(!outside);

    fs::remove_all(dir);
    apply_test::run_all();

    return 0;
}
