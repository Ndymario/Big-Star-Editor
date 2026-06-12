#include "nsmb/patching/external_tools.hpp"

#include <cassert>
#include <filesystem>
#include <fstream>
#include <sstream>

int main() {
    namespace fs = std::filesystem;
    const auto dir = fs::temp_directory_path() / "big-star-patching-test";
    fs::remove_all(dir);
    fs::create_directories(dir / "extracted-rom/root");

    nsmb::patching::NCPatcherExternalTool patcher;
    auto cfg = patcher.validate_or_create_config(dir);
    assert(cfg);
    assert(fs::exists(cfg.value()));

    auto cmd = patcher.build_command({dir, "ncpatcher", true});
    assert(cmd);
    assert(cmd.value().working_directory == dir);
    assert(cmd.value().arguments.size() == 1);

    nsmb::patching::ExternalToolRomBuildService rom("nds-extract", "nds-build");
    auto extract = rom.build_extract_command("game.nds", dir / "extracted-rom");
    assert(extract);
    assert(extract.value().arguments.size() == 2);

    auto rules = rom.write_fireflower_build_rules(dir / "extracted-rom", dir / "buildrules.txt");
    assert(rules);
    std::ifstream rules_input(rules.value());
    std::stringstream rules_text;
    rules_text << rules_input.rdbuf();
    assert(rules_text.str().find("rom_header") != std::string::npos);
    assert(rules_text.str().find("data ") != std::string::npos);

    auto build = rom.build_rom_command(dir / "buildrules.txt", "patched.nds");
    assert(build);
    assert(build.value().arguments.size() == 2);
    assert(build.value().arguments[0].find("buildrules.txt") != std::string::npos);

    fs::remove_all(dir);
    return 0;
}
