#include "nsmb/project/filesystem_view.hpp"
#include "nsmb/formats/graphics.hpp"
#include "nsmb/formats/narc.hpp"

#include <cassert>
#include <filesystem>
#include <fstream>

namespace {
using nsmb::patching::FileAction;

void write(const std::filesystem::path& path,std::string_view value){std::filesystem::create_directories(path.parent_path());std::ofstream(path,std::ios::binary)<<value;}
void write(const std::filesystem::path& path,const std::vector<std::uint8_t>& value){std::filesystem::create_directories(path.parent_path());std::ofstream output(path,std::ios::binary);output.write(reinterpret_cast<const char*>(value.data()),value.size());}

nsmb::patching::ModuleInfo module(std::string key,const std::filesystem::path& root,bool layered,std::string base={},std::string into={}){
    nsmb::patching::ModuleInfo result;result.key=key;result.id=key;result.enabled=true;
    result.nitrofs=nsmb::patching::ModuleNitroFs{root,layered,std::move(base),std::move(into)};return result;
}

// One NARC holding one member, so a container's bytes can be compared against
// what a plan says its members should be.
std::vector<std::uint8_t> one_member_narc(std::string_view contents){
    nsmb::formats::NarcArchive narc;narc.name_table={8,0,0,0,0,0,1,0,10,'m','e','m','b','e','r','.','b','i','n',0};
    narc.files={{contents.begin(),contents.end()}};return nsmb::formats::write_narc(narc).value();
}
}

int main(){
    namespace fs=std::filesystem;using namespace nsmb::project;
    const auto root=fs::temp_directory_path()/"big-star-filesystem-view";fs::remove_all(root);

    // The ROM as the last build left it.
    const auto extracted=root/"extracted";
    write(extracted/"root/vanilla.bin","V");
    write(extracted/"root/shared.bin","FR");
    write(extracted/"root/moved.bin","SAME");
    write(extracted/"root/explicit.bin","EXPLICIT");
    write(extracted/"root/generated.bin","BUILT");
    write(extracted/"root/stale.bin","OLD");
    write(extracted/"root/gone.bin","LEFTOVER");
    write(extracted/"root/z_new/reserved","");
    write(extracted/"root/z_new/add.bin","ADD");
    write(extracted/"root/ARCHIVE/menu.narc",one_member_narc("M"));
    write(extracted/"root/ARCHIVE/other.narc",one_member_narc("OLD"));
    // A compressed container is an ordinary archive under an LZ10 wrapper, and
    // has to be compared through it rather than written off as unreadable.
    write(extracted/"root/ARCHIVE/menu.carc",nsmb::formats::compress_nitro_lz10(one_member_narc("C")).value());

    // The module trees. Nothing here is walked any more -- they exist so the
    // sources a plan names are on disk, and so there is something to watch.
    auto first=module("first",root/"modules/first/nitrofs",false);
    write(first.nitrofs->dir/"moved.bin","SAME");
    auto second=module("second",root/"modules/second/nitrofs",true,"en");
    write(second.nitrofs->dir/"fr/shared.bin","FR");
    write(second.nitrofs->dir/"fr/stale.bin","NEW");
    write(second.nitrofs->dir/"fr/z_new/add.bin","ADD");
    write(second.nitrofs->dir/"fr/z_new/fresh.bin","FRESH");
    write(second.nitrofs->dir/"fr/ARCHIVE/menu_narc/member.bin","M");
    write(second.nitrofs->dir/"fr/ARCHIVE/menu_carc/member.bin","C");
    write(second.nitrofs->dir/"fr/ARCHIVE/other_narc/member.bin","NEW");
    auto prefixed=module("prefixed",root/"modules/prefixed/nitrofs",true,"common","assets");
    write(prefixed.nitrofs->dir/"french/picture.bin","PIC");
    write(root/"project-assets/explicit.bin","EXPLICIT");
    // A pre-build hook writes this one, and writes it differently every time,
    // so its bytes never match what the last build put in the ROM.
    write(root/"build/generated/generated.bin","REGENERATED");
    nsmb::patching::ModuleGraph graph;graph.modules={first,second,prefixed};

    // What the last build recorded doing.
    nsmb::patching::FileManifest manifest;manifest.variant="fr";manifest.files={
        {1,"vanilla.bin",1,FileAction::Unchanged},
        {2,"shared.bin",2,FileAction::Modified,"modules/second/nitrofs/fr/shared.bin","second","Assets","fr"},
        {3,"moved.bin",4,FileAction::Modified,"modules/second/nitrofs/fr/moved.bin","second",{},"fr"},
        {4,"explicit.bin",8,FileAction::Modified,"project-assets/explicit.bin"},
        {5,"generated.bin",5,FileAction::Modified,"build/generated/generated.bin","glue",{},{}},
        {6,"stale.bin",3,FileAction::Modified,"modules/second/nitrofs/fr/stale.bin","second",{},"fr"},
        {7,"gone.bin",8,FileAction::Modified,"modules/removed/gone.bin","removed",{},"fr"},
        {8,"ARCHIVE/menu.narc",64,FileAction::Modified,{},{},{},{},false,
            {{0,"member.bin",1,FileAction::Modified,"modules/second/nitrofs/fr/ARCHIVE/menu_narc/member.bin","second","Assets","fr"}}},
        {9,"ARCHIVE/menu.carc",48,FileAction::Modified,{},{},{},{},false,
            {{0,"member.bin",1,FileAction::Modified,"modules/second/nitrofs/fr/ARCHIVE/menu_carc/member.bin","second","Assets","fr"}}},
        {10,"ARCHIVE/other.narc",64,FileAction::Modified,{},{},{},{},false,
            {{0,"member.bin",3,FileAction::Modified,"modules/second/nitrofs/fr/ARCHIVE/other_narc/member.bin","second","Assets","fr"}}},
        {11,"z_new/reserved",0,FileAction::Created},
        {12,"z_new/add.bin",3,FileAction::Created,"modules/second/nitrofs/fr/z_new/add.bin","second","Assets","fr"},
    };

    // What a build would do now. `stale.bin` and `ARCHIVE/other.narc` have new
    // bytes behind them, `moved.bin` changed hands, `z_new/fresh.bin` is new,
    // and `gone.bin` is no longer supplied at all.
    nsmb::patching::FileManifest plan=manifest;plan.planned=true;
    const auto at=[&](std::string_view path)->nsmb::patching::RomFileEntry&{
        return *std::ranges::find(plan.files,path,&nsmb::patching::RomFileEntry::path);};
    at("moved.bin")={3,"moved.bin",4,FileAction::Modified,"modules/first/nitrofs/moved.bin","first",{},{}};
    at("generated.bin").source_missing=true;
    at("gone.bin")={7,"gone.bin",8,FileAction::Unchanged};
    plan.files.push_back({13,"z_new/fresh.bin",5,FileAction::Created,"modules/second/nitrofs/fr/z_new/fresh.bin","second","Assets","fr"});

    FilesystemViewRequest request;request.extracted_rom=extracted;request.project_dir=root;
    request.modules=graph;request.manifest=manifest;request.plan=plan;request.variant="fr";
    auto view=build_filesystem_view(std::move(request));assert(view);
    const auto find=[&](std::string_view path){const auto* entry=view.value().find(path);assert(entry);return entry;};

    // Untouched by either document.
    assert(find("vanilla.bin")->status==FilesystemStatus::Vanilla);
    // Supplied, agreed on, and the same bytes the ROM holds.
    assert(find("shared.bin")->status==FilesystemStatus::Replaced&&find("shared.bin")->module=="second");
    // A project-level `files:` entry: a source with no module behind it.
    assert(find("explicit.bin")->status==FilesystemStatus::Replaced&&find("explicit.bin")->module.empty());
    assert(find("explicit.bin")->source==root/"project-assets/explicit.bin");
    // The bytes behind it changed since the build.
    assert(find("stale.bin")->status==FilesystemStatus::Pending);
    // Same bytes, different module. Which module owns a destination is part of
    // what a build did, so a handover is a change even when nothing moves.
    assert(find("moved.bin")->status==FilesystemStatus::Pending&&find("moved.bin")->module=="first");
    // A hook writes this one, so there is nothing on disk to compare it with.
    // Calling it Pending would be a mark the user could never clear.
    assert(find("generated.bin")->status==FilesystemStatus::Replaced);
    // A previous build put a module's file here and nothing supplies it now.
    assert(find("gone.bin")->status==FilesystemStatus::Orphaned);
    // NCPatcher creates the reservation placeholder out of nothing, so it has
    // no source and still is not vanilla.
    assert(find("z_new/reserved")->status==FilesystemStatus::Added);
    assert(find("z_new/add.bin")->status==FilesystemStatus::Added);

    // An id the ROM does not hold yet, marked as the prediction it is.
    const auto* fresh=find("z_new/fresh.bin");
    assert(fresh->status==FilesystemStatus::Pending);
    assert(fresh->id==13&&fresh->id_is_planned);
    // Every other id is read from the ROM, so none of them is a prediction.
    assert(!find("z_new/add.bin")->id_is_planned&&!find("shared.bin")->id_is_planned);

    // Archives, still collapsed into their container here, compared through
    // the members the plan names rather than by re-deriving them.
    assert(find("ARCHIVE/menu.narc")->status==FilesystemStatus::Replaced);
    assert(find("ARCHIVE/menu.carc")->status==FilesystemStatus::Replaced);
    assert(find("ARCHIVE/other.narc")->status==FilesystemStatus::Pending);
    assert(view.value().has_pending());

    // The property the deleted resolution code could never have: when the plan
    // is the document the last build wrote, the view has nothing to report.
    // Two implementations of module resolution could agree on every test and
    // still fail this on the project that matters.
    {
        nsmb::patching::FileManifest built=plan;built.planned=false;
        write(extracted/"root/stale.bin","NEW");
        write(extracted/"root/ARCHIVE/other.narc",one_member_narc("NEW"));
        write(extracted/"root/z_new/fresh.bin","FRESH");
        FilesystemViewRequest agreed;agreed.extracted_rom=extracted;agreed.project_dir=root;
        agreed.modules=graph;agreed.manifest=built;agreed.plan=plan;agreed.variant="fr";
        auto same=build_filesystem_view(std::move(agreed));assert(same);
        assert(!same.value().has_pending());
        assert(same.value().find("gone.bin")->status==FilesystemStatus::Vanilla);
        assert(!same.value().find("z_new/fresh.bin")->id_is_planned);
        write(extracted/"root/stale.bin","OLD");
        write(extracted/"root/ARCHIVE/other.narc",one_member_narc("OLD"));
        fs::remove(extracted/"root/z_new/fresh.bin");
    }

    // A project that has never been built has no manifest, so everything a
    // build would place is pending -- including the files it would only
    // rewrite in place.
    {
        FilesystemViewRequest fresh_project;fresh_project.extracted_rom=extracted;fresh_project.project_dir=root;
        fresh_project.modules=graph;fresh_project.plan=plan;fresh_project.variant="fr";
        auto unbuilt=build_filesystem_view(std::move(fresh_project));assert(unbuilt);
        assert(unbuilt.value().find("shared.bin")->status==FilesystemStatus::Pending);
        assert(unbuilt.value().find("vanilla.bin")->status==FilesystemStatus::Vanilla);
        assert(unbuilt.value().find("z_new/add.bin")->id_is_planned);
    }

    // Without NCPatcher there is no plan, and the view reports the last build
    // rather than inventing an answer of its own.
    {
        FilesystemViewRequest unplanned;unplanned.extracted_rom=extracted;unplanned.project_dir=root;
        unplanned.modules=graph;unplanned.manifest=manifest;unplanned.variant="fr";
        auto reported=build_filesystem_view(std::move(unplanned));assert(reported);
        assert(reported.value().find("shared.bin")->status==FilesystemStatus::Orphaned);
        assert(reported.value().find("vanilla.bin")->status==FilesystemStatus::Vanilla);
    }

    // Watching is the one thing that still needs the module graph.
    assert(std::ranges::find(view.value().watched_directories,second.nitrofs->dir)!=view.value().watched_directories.end());

    // Writing an edit back is the direction no build document answers.
    const std::array mappings{ModuleVariantMapping{"prefixed","french"}};
    auto target=module_file_target(second,"fr","ARCHIVE/menu.narc!member.bin");
    assert(target&&target.value()==second.nitrofs->dir/"fr/ARCHIVE/menu_narc/member.bin");
    // The folder convention is not about NARCs: it is `<stem>_<ext>` for
    // whatever extension a game gives its archives.
    auto compressed=module_file_target(second,"fr","ARCHIVE/menu.carc!member.bin");
    assert(compressed&&compressed.value()==second.nitrofs->dir/"fr/ARCHIVE/menu_carc/member.bin");
    auto mapped=module_file_target(prefixed,"fr","assets/picture.bin",mappings);
    assert(mapped&&mapped.value()==prefixed.nitrofs->dir/"french/picture.bin");
    assert(!module_file_target(prefixed,"fr","outside.bin",mappings));
    assert(!module_file_target(second,"fr","bad//path.bin"));
    assert(!module_file_target(second,"fr","ARCHIVE/menu!member.bin"));

    fs::remove_all(root);return 0;
}
