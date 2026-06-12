#include "nsmb/patching/external_tools.hpp"
#include <cassert>
#include <filesystem>
int main() {
    namespace fs = std::filesystem;
    auto dir = fs::temp_directory_path() / "big-star-patching-test";
    fs::remove_all(dir); fs::create_directories(dir);
    nsmb::patching::NCPatcherExternalTool patcher;
    auto cfg = patcher.validate_or_create_config(dir);
    assert(cfg); assert(fs::exists(cfg.value()));
    auto cmd = patcher.build_command({dir, "ncpatcher", true});
    assert(cmd); assert(cmd.value().working_directory == dir); assert(cmd.value().arguments.size() == 1);
    nsmb::patching::ExternalToolRomBuildService rom("nds-extract", "nds-build");
    auto extract = rom.build_extract_command("game.nds", dir / "out");
    assert(extract); assert(extract.value().arguments.size() == 2);
    auto build = rom.build_rom_command(dir / "out", "patched.nds");
    assert(build); assert(build.value().arguments.size() == 2);
    fs::remove_all(dir);
    return 0;
}
