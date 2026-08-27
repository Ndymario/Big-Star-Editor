#include "nsmb/project/filesystem_view.hpp"
#include "nsmb/formats/narc.hpp"

#include <cassert>
#include <filesystem>
#include <fstream>

namespace {
void write(const std::filesystem::path& path,std::string_view value){std::filesystem::create_directories(path.parent_path());std::ofstream(path,std::ios::binary)<<value;}
void write(const std::filesystem::path& path,const std::vector<std::uint8_t>& value){std::filesystem::create_directories(path.parent_path());std::ofstream output(path,std::ios::binary);output.write(reinterpret_cast<const char*>(value.data()),value.size());}

nsmb::patching::ModuleInfo module(std::string key,const std::filesystem::path& root,bool layered,std::string base={},std::string into={}){
    nsmb::patching::ModuleInfo result;result.key=key;result.id=key;result.enabled=true;
    result.nitrofs=nsmb::patching::ModuleNitroFs{root,layered,std::move(base),std::move(into)};return result;
}
}

int main(){
    namespace fs=std::filesystem;using namespace nsmb::project;using nsmb::patching::FileAction;
    const auto root=fs::temp_directory_path()/"big-star-filesystem-view";fs::remove_all(root);
    const auto extracted=root/"extracted";write(extracted/"root/vanilla.bin","V");write(extracted/"root/shared.bin","FR");write(extracted/"root/order.bin","FIRST");write(extracted/"root/explicit.bin","EXPLICIT");write(extracted/"root/legacy-only.bin","LEGACY");write(extracted/"root/base.bin","BASE");write(extracted/"root/z_new/add.bin","ADD");write(extracted/"root/stale.bin","OLD");

    auto first=module("first",root/"modules/first/nitrofs",false);
    write(first.nitrofs->dir/"shared.bin","FIRST");
    write(first.nitrofs->dir/"order.bin","FIRST");
    write(first.nitrofs->dir/"explicit.bin","MODULE");
    write(first.nitrofs->dir/"disabled.bin","NO");
    first.components.push_back({"Disabled",false,"test",{"disabled.bin"},{}});
    auto second=module("second",root/"modules/second/nitrofs",true,"en");
    write(second.nitrofs->dir/"en/base.bin","BASE");
    write(second.nitrofs->dir/"en/order.bin","SECOND");
    write(second.nitrofs->dir/"fr/shared.bin","FR");
    write(second.nitrofs->dir/"fr/z_new/add.bin","ADD");
    write(second.nitrofs->dir/"fr/stale.bin","NEW");
    write(second.nitrofs->dir/"fr/pending.bin","P");
    write(second.nitrofs->dir/"fr/ARCHIVE/menu_narc/member.bin","M");
    second.components.push_back({"Assets",true,{}, {"{shared,base}.bin","z_new/**","ARCHIVE/**"},{}});
    auto prefixed=module("prefixed",root/"modules/prefixed/nitrofs",true,"common","assets");
    write(prefixed.nitrofs->dir/"french/picture.bin","PIC");
    nsmb::patching::ModuleGraph graph;graph.modules={first,second,prefixed};
    nsmb::formats::NarcArchive narc;narc.name_table={8,0,0,0,0,0,1,0,10,'m','e','m','b','e','r','.','b','i','n',0};narc.files={{'M'}};
    write(extracted/"root/ARCHIVE/menu.narc",nsmb::formats::write_narc(narc).value());

    const std::array mappings{ModuleVariantMapping{"prefixed","french"}};
    auto offers=resolve_module_files(graph,"fr",mappings);assert(offers);
    const auto find=[&](std::string_view path){return std::ranges::find(offers.value(),path,&ModuleFileOffer::rom_path);};
    assert(find("shared.bin")!=offers.value().end()&&find("shared.bin")->module=="second");
    assert(find("order.bin")!=offers.value().end()&&find("order.bin")->module=="first");
    assert(find("base.bin")!=offers.value().end()&&find("base.bin")->from_variant=="en");
    assert(find("disabled.bin")==offers.value().end());
    assert(find("ARCHIVE/menu.narc!member.bin")!=offers.value().end());
    assert(find("assets/picture.bin")!=offers.value().end()&&find("assets/picture.bin")->from_variant=="french");

    nsmb::patching::FileManifest manifest;manifest.variant="fr";manifest.files={
        {1,"vanilla.bin",1,FileAction::Unchanged},
        {2,"shared.bin",2,FileAction::Modified,"modules/second/nitrofs/fr/shared.bin","second","Assets","fr"},
        {7,"order.bin",5,FileAction::Modified,"modules/first/nitrofs/order.bin","first",{},{}},
        {8,"explicit.bin",8,FileAction::Modified,"project-assets/explicit.bin"},
        {10,"legacy-only.bin",6,FileAction::Modified,"project-assets/legacy-only.bin"},
        {3,"base.bin",4,FileAction::Modified,"modules/second/nitrofs/en/base.bin","second","Assets","en"},
        {4,"z_new/add.bin",3,FileAction::Created,"modules/second/nitrofs/fr/z_new/add.bin","second","Assets","fr"},
        {5,"stale.bin",3,FileAction::Modified,"modules/second/nitrofs/fr/stale.bin","second",{},"fr"},
        {6,"gone.bin",3,FileAction::Modified,"modules/removed/gone.bin","removed",{},"fr"},
        {9,"ARCHIVE/menu.narc",1,FileAction::Modified,{},"second","Assets","fr"},
    };
    write(root/"project-assets/explicit.bin","EXPLICIT");
    write(root/"project-assets/legacy-only.bin","LEGACY");
    FilesystemViewRequest request;request.extracted_rom=extracted;request.project_dir=root;request.modules=graph;request.manifest=manifest;request.variant="fr";request.module_variants.assign(mappings.begin(),mappings.end());
    auto view=build_filesystem_view(std::move(request));assert(view);
    assert(view.value().find("vanilla.bin")->status==FilesystemStatus::Vanilla);
    assert(view.value().find("shared.bin")->status==FilesystemStatus::Replaced);
    assert(view.value().find("order.bin")->module=="first"&&view.value().find("order.bin")->status==FilesystemStatus::Replaced);
    assert(view.value().find("explicit.bin")->module.empty()&&view.value().find("explicit.bin")->status==FilesystemStatus::Replaced);
    assert(view.value().find("legacy-only.bin")->status==FilesystemStatus::Replaced);
    assert(view.value().find("base.bin")->status==FilesystemStatus::Replaced);
    assert(view.value().find("z_new/add.bin")->status==FilesystemStatus::Added);
    assert(view.value().find("pending.bin")->status==FilesystemStatus::Pending);
    assert(view.value().find("stale.bin")->status==FilesystemStatus::Pending);
    assert(view.value().find("gone.bin")->status==FilesystemStatus::Orphaned);
    assert(view.value().find("ARCHIVE/menu.narc")->status==FilesystemStatus::Replaced);
    assert(view.value().has_pending());

    auto target=module_file_target(second,"fr","ARCHIVE/menu.narc!member.bin");assert(target&&target.value()==second.nitrofs->dir/"fr/ARCHIVE/menu_narc/member.bin");
    auto mapped=module_file_target(prefixed,"fr","assets/picture.bin",mappings);assert(mapped&&mapped.value()==prefixed.nitrofs->dir/"french/picture.bin");
    assert(!module_file_target(prefixed,"fr","outside.bin",mappings));
    assert(!module_file_target(second,"fr","bad//path.bin"));
    assert(!resolve_module_files(graph,"fr",std::array{ModuleVariantMapping{"missing","fr"}}));
    assert(!resolve_module_files(graph,"fr",std::array{ModuleVariantMapping{"first","fr"}}));
    write(root/"project-assets/current.bin","CURRENT");
    FilesystemViewRequest current;current.extracted_rom=extracted;current.project_dir=root;current.modules=graph;current.manifest=manifest;current.variant="fr";current.module_variants.assign(mappings.begin(),mappings.end());current.has_current_config=true;current.explicit_files.push_back({"explicit.bin","project-assets/current.bin",{},{},{}});
    auto currentView=build_filesystem_view(std::move(current));assert(currentView);
    assert(currentView.value().find("explicit.bin")->module.empty()&&currentView.value().find("explicit.bin")->source==root/"project-assets/current.bin"&&currentView.value().find("explicit.bin")->status==FilesystemStatus::Pending);
    assert(currentView.value().find("legacy-only.bin")->status==FilesystemStatus::Orphaned);
    auto parsed=parse_project_variant_layers("variants:\n  en:\n    defines: X\n  fr:\n    module-variants:\n      prefixed: french\n  de:\n    module-variants: { prefixed: german }\ntargets:\n  arm9: {}\n");
    assert(parsed&&parsed.value().size()==3&&parsed.value()[1].mappings[0].layer=="french"&&parsed.value()[2].mappings[0].layer=="german");
    fs::remove_all(root);return 0;
}
