#include "nsmb/patching/external_tools.hpp"

#include <cassert>
#include <filesystem>

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
#ifdef _WIN32
    assert(nsmb::patching::command_to_string({"patcher$(false)", {"it's safe"}, {}}) == "\"patcher$(false)\" \"it's safe\"");
#else
    assert(nsmb::patching::command_to_string({"patcher$(false)", {"it's safe"}, {}}) == "'patcher$(false)' 'it'\\''s safe'");
#endif

    fs::remove_all(dir);
    return 0;
}
