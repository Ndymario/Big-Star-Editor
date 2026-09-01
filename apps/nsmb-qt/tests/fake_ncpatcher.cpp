// A stand-in for NCPatcher, so the editor's own behaviour can be tested
// without a toolchain, a ROM, or a real project to build.
//
// It answers a subcommand by printing a file the test wrote next to the
// project: `<project>/fake-ncpatcher/<command>.json`, where <command> is
// `config-dump`, `modules-dump`, `files-plan` or `rom-files`. A command with
// no file behind it exits 3, which is what NCPatcher does when it cannot make
// sense of the project -- so "the editor was not given an answer" is a case
// the tests can set up as easily as any other.

#include <cstdio>
#include <fstream>
#include <string>
#include <vector>

namespace {

bool has(const std::vector<std::string>& args, std::string_view word) {
    for (const auto& arg : args) {
        if (arg == word) return true;
    }
    return false;
}

} // namespace

int main(int argc, char** argv) {
    const std::vector<std::string> args(argv + 1, argv + argc);

    std::string project;
    for (std::size_t i = 0; i + 1 < args.size(); ++i) {
        if (args[i] == "-C") project = args[i + 1];
    }

    std::string command;
    if (has(args, "version")) {
        std::fputs("ncpatcher 2.0.0 (fake)\n", stdout);
        return 0;
    }
    if (has(args, "config") && has(args, "dump")) command = "config-dump";
    else if (has(args, "modules") && has(args, "dump")) command = "modules-dump";
    else if (has(args, "files") && has(args, "plan")) command = "files-plan";
    else if (has(args, "rom") && has(args, "files")) command = "rom-files";
    else {
        std::fputs("fake ncpatcher: unrecognised command\n", stderr);
        return 2;
    }

    std::ifstream input(project + "/fake-ncpatcher/" + command + ".json", std::ios::binary);
    if (!input) {
        std::fprintf(stderr, "fake ncpatcher: nothing staged for `%s`\n", command.c_str());
        return 3;
    }
    std::fputs(std::string(std::istreambuf_iterator<char>(input), {}).c_str(), stdout);
    return 0;
}
