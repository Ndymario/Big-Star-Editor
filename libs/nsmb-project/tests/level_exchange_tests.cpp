#include "nsmb/project/level_exchange.hpp"

#include <iostream>

namespace { int failures{}; void check(bool ok, const char* text) { if (!ok) { std::cerr << "FAIL: " << text << '\n'; ++failures; } } }

int main() {
    nsmb::project::LevelPackage level{12, 13, {1,2,3,4}, {5,6}};
    auto nml = nsmb::project::encode_nml(level);
    auto decoded = nml ? nsmb::project::decode_nml(nml.value()) : nsmb::core::Result<nsmb::project::LevelPackage>(nml.error());
    check(decoded && decoded.value().course == level.course && decoded.value().bgdat_file_id == 13, "NML round trip");
    if (nml) { nml.value().push_back(0); check(!nsmb::project::decode_nml(nml.value()), "NML trailing bytes rejected"); }

    const std::vector entries{nsmb::project::NsmbPatchEntry{"root/course/A01_1.bin", 42, {1,1,1,2,3,5,8}},
                              nsmb::project::NsmbPatchEntry{"root/course/A01_1_bgdat.bin", -1, {9,8,7}}};
    auto patch = nsmb::project::encode_nsmbpatch(entries);
    auto patch_decoded = patch ? nsmb::project::decode_nsmbpatch(patch.value()) : nsmb::core::Result<std::vector<nsmb::project::NsmbPatchEntry>>(patch.error());
    check(patch_decoded && patch_decoded.value().size() == 2 && patch_decoded.value()[0].contents == entries[0].contents, "patch compresses and round trips");
    check(!nsmb::project::encode_nsmbpatch(std::vector{nsmb::project::NsmbPatchEntry{"../escape", -1, {1}}}), "unsafe patch path rejected");
    check(!nsmb::project::encode_nsmbpatch(std::vector{entries[0], entries[0]}), "duplicate patch path rejected");

    nsmb::project::LevelRecoverySnapshot recovery{"extracted/root/course/A.bin", "extracted/root/course/A_bgdat.bin", level};
    auto snapshot = nsmb::project::encode_level_recovery(recovery);
    auto recovered = snapshot ? nsmb::project::decode_level_recovery(snapshot.value()) : nsmb::core::Result<nsmb::project::LevelRecoverySnapshot>(snapshot.error());
    check(recovered && recovered.value().course_path == recovery.course_path && recovered.value().package.course == level.course, "recovery snapshot round trip");
    return failures ? 1 : 0;
}
