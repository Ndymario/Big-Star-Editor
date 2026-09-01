#include "nsmb/project/filesystem_view.hpp"

#include "nsmb/core/file_io.hpp"
#include "nsmb/formats/graphics.hpp"
#include "nsmb/formats/narc.hpp"

#include <algorithm>
#include <array>
#include <fstream>
#include <map>
#include <set>

namespace nsmb::project {
namespace {

bool safe_path(std::string_view path) {
    if(path.empty()||path.front()=='/'||path.back()=='/'||path.find('\\')!=path.npos)return false;
    std::size_t start=0;while(start<path.size()){const auto slash=path.find('/',start);const auto end=slash==path.npos?path.size():slash;const auto part=path.substr(start,end-start);
        if(part.empty()||part=="."||part==".."||part.size()>0x7f)return false;if(slash==path.npos)break;start=slash+1;}return true;
}

core::Result<std::vector<std::filesystem::path>> files_under(const std::filesystem::path& root){
    std::vector<std::filesystem::path> files;std::error_code ec;
    std::filesystem::recursive_directory_iterator it(root,std::filesystem::directory_options::skip_permission_denied,ec),end;
    if(ec)return core::make_error(core::ErrorCode::IoError,"could not read "+root.string());
    while(it!=end){if(it->is_regular_file(ec))files.push_back(it->path());if(ec)return core::make_error(core::ErrorCode::IoError,"could not inspect "+root.string());it.increment(ec);if(ec)return core::make_error(core::ErrorCode::IoError,"could not continue reading "+root.string());}
    std::ranges::sort(files);return files;
}

bool same_bytes(const std::filesystem::path& left,const std::filesystem::path& right){
    std::error_code ec;if(!std::filesystem::is_regular_file(left,ec)||!std::filesystem::is_regular_file(right,ec))return false;
    if(std::filesystem::file_size(left,ec)!=std::filesystem::file_size(right,ec)||ec)return false;
    std::ifstream a(left,std::ios::binary),b(right,std::ios::binary);std::array<char,65536> x{},y{};
    if(!a||!b)return false;
    while(a&&b){a.read(x.data(),x.size());b.read(y.data(),y.size());if(a.gcount()!=b.gcount()||!std::equal(x.begin(),x.begin()+a.gcount(),y.begin()))return false;}return true;
}

std::filesystem::path source_path(const std::filesystem::path& project_dir,std::string_view source){auto path=std::filesystem::path(source);return path.is_absolute()?path:project_dir/path;}

// A container may have arrived compressed -- Mario Kart DS names those `.carc`
// -- and is still an ordinary NARC underneath. The wrapper is decided by what
// the bytes decode to, never by the extension: `dwc/utility.bin` starts with
// 0x10 and is not compressed at all.
core::Result<formats::NarcArchive> open_archive(std::span<const std::uint8_t> data){
    if(formats::is_narc(data))return formats::parse_narc(data);
    auto raw=formats::decompress_nitro_lz(data);if(!raw)return raw.error();
    return formats::parse_narc(raw.value());
}

// Does the container the ROM holds already carry the members the plan would
// write into it? Addressed by index, which is how the game addresses a member
// and how the plan reports one; a member path is a label, not an address.
bool archive_members_match(const std::filesystem::path& container,const std::filesystem::path& project_dir,const std::vector<patching::RomArchiveMember>& members){
    auto bytes=core::read_file(container);if(!bytes)return false;
    auto archive=open_archive(bytes.value());if(!archive)return false;
    for(const auto& member:members){
        // A hook writes this one, so there is nothing on disk to compare and
        // nothing the user can do about it either way.
        if(member.source_missing)continue;
        if(member.index<0||std::size_t(member.index)>=archive.value().files.size())return false;
        auto expected=core::read_file(source_path(project_dir,member.source));
        if(!expected||archive.value().files[std::size_t(member.index)]!=expected.value())return false;
    }
    return true;
}

// The set of members a run wrote is part of what it did. A build that replaced
// three members and a plan that would replace two disagree even if those two
// match byte for byte.
bool same_members(const std::vector<patching::RomArchiveMember>& built,const std::vector<patching::RomArchiveMember>& planned){
    return std::ranges::equal(built,planned,[](const auto& a,const auto& b){
        return a.index==b.index&&a.path==b.path&&a.module==b.module&&a.component==b.component&&a.from_variant==b.from_variant;});
}

} // namespace

core::Result<std::filesystem::path> module_file_target(const patching::ModuleInfo& module,std::string_view variant,std::string_view rom_path,std::span<const ModuleVariantMapping> mappings){
    if(!module.enabled||!module.nitrofs)return core::make_error(core::ErrorCode::InvalidArgument,"target module has no enabled NitroFS tree");
    const auto& tree=*module.nitrofs;std::string relative(rom_path);
    if(const auto bang=relative.find('!');bang!=relative.npos){
        // `a/b.narc!c/d` is written as `a/b_narc/c/d`, and the same for any
        // other extension a game gives its archives. NCPatcher decides which
        // way to read such a segment by asking the ROM whether an archive is
        // there; writing is unambiguous, because the address already said so.
        auto archive=relative.substr(0,bang);const auto dot=archive.rfind('.');const auto slash=archive.rfind('/');
        if(dot==archive.npos||dot+1>=archive.size()||(slash!=archive.npos&&dot<slash))
            return core::make_error(core::ErrorCode::InvalidArgument,"archive destination has no extension to spell as a folder: "+archive);
        archive[dot]='_';relative=archive+"/"+relative.substr(bang+1);
    }
    if(!tree.into.empty()){const auto prefix=tree.into+"/";if(!relative.starts_with(prefix))return core::make_error(core::ErrorCode::InvalidArgument,"ROM path is outside the module's into prefix");relative.erase(0,prefix.size());}
    if(!safe_path(relative))return core::make_error(core::ErrorCode::InvalidArgument,"ROM path is unsafe");
    auto root=tree.dir;
    if(tree.layered){
        const auto found=std::ranges::find_if(mappings,[&](const auto& mapping){return mapping.module==module.key||mapping.module==module.id;});
        const auto* mapped=found==mappings.end()?nullptr:&*found;
        const auto layer=mapped?mapped->layer:std::string(variant.empty()?tree.base_variant:variant);
        if(layer.empty())return core::make_error(core::ErrorCode::InvalidArgument,"a variant is required for this layered module");
        root/=layer;
        if(mapped){std::error_code ec;if(!std::filesystem::is_directory(root,ec))return core::make_error(core::ErrorCode::NotFound,"mapped module layer does not exist: "+root.string());}
    }
    return root/std::filesystem::path(relative);
}

core::Result<FilesystemView> build_filesystem_view(FilesystemViewRequest request){
    FilesystemView view;
    // Three records of the same ROM, none of them derived here: what a build
    // would place, what the last build did place, and what is on disk now.
    std::map<std::string,const patching::RomFileEntry*> planned,manifested;
    if(request.plan)for(const auto& file:request.plan->files)planned[file.path]=&file;
    if(request.manifest)for(const auto& file:request.manifest->files)manifested[file.path]=&file;

    std::map<std::string,std::filesystem::path> extracted;const auto root=request.extracted_rom/"root";std::error_code ec;
    if(std::filesystem::is_directory(root,ec)){auto files=files_under(root);if(!files)return files.error();
        for(const auto& file:files.value()){auto relative=std::filesystem::relative(file,root,ec);if(ec)return core::make_error(core::ErrorCode::IoError,"could not resolve extracted ROM path");extracted[relative.generic_string()]=file;}}

    std::set<std::string> paths;
    for(const auto& [path,file]:extracted)paths.insert(path);
    for(const auto& [path,file]:manifested)paths.insert(path);
    for(const auto& [path,file]:planned)paths.insert(path);

    for(const auto& path:paths){
        FilesystemEntry entry;entry.path=path;
        const auto disk=extracted.find(path);if(disk!=extracted.end())entry.extracted=disk->second;
        const auto found_manifest=manifested.find(path);const auto* manifest=found_manifest==manifested.end()?nullptr:found_manifest->second;
        const auto found_plan=planned.find(path);const auto* plan=found_plan==planned.end()?nullptr:found_plan->second;

        if(manifest){
            entry.id=manifest->id;entry.size=std::uintmax_t(manifest->size);
            entry.module=manifest->module;entry.component=manifest->component;entry.variant=manifest->from_variant;
        }else if(plan){
            // Only a file the ROM does not hold yet can have an id the ROM
            // cannot confirm, and only a plan can predict one.
            entry.id=plan->id;entry.size=std::uintmax_t(plan->size);
            entry.id_is_planned=request.plan->planned&&plan->action==patching::FileAction::Created;
        }

        // A build writes a file when a module supplies it, and also when it
        // creates one out of nothing: z_new/reserved has no source at all.
        const bool supplied=plan&&(plan->has_provenance()||plan->action!=patching::FileAction::Unchanged);
        if(!supplied){
            entry.status=manifest&&manifest->has_provenance()?FilesystemStatus::Orphaned:FilesystemStatus::Vanilla;
            view.entries.push_back(std::move(entry));continue;
        }

        entry.module=plan->module;entry.component=plan->component;entry.variant=plan->from_variant;
        if(!plan->source.empty())entry.source=source_path(request.project_dir,plan->source);
        if(!plan->source_missing)entry.size=std::uintmax_t(plan->size);

        bool agrees=manifest!=nullptr;
        if(agrees)agrees=manifest->module==plan->module&&manifest->component==plan->component&&manifest->from_variant==plan->from_variant;
        if(agrees&&!plan->members.empty())
            agrees=same_members(manifest->members,plan->members)&&archive_members_match(entry.extracted,request.project_dir,plan->members);
        // A source a pre-build hook writes is not on disk to be compared, so
        // there is no honest way to call it stale. Reporting the last build is
        // better than a Pending mark nothing the user does can clear.
        else if(agrees&&!entry.source.empty()&&!plan->source_missing)agrees=same_bytes(entry.source,entry.extracted);

        entry.status=!agrees?FilesystemStatus::Pending
            :(path.starts_with("z_new/")||manifest->action==patching::FileAction::Created?FilesystemStatus::Added:FilesystemStatus::Replaced);
        view.entries.push_back(std::move(entry));
    }

    for(const auto& module:request.modules.modules)if(module.enabled&&module.nitrofs){view.watched_directories.push_back(module.nitrofs->dir);std::error_code walk_error;for(std::filesystem::recursive_directory_iterator it(module.nitrofs->dir,std::filesystem::directory_options::skip_permission_denied,walk_error),end;!walk_error&&it!=end;it.increment(walk_error))if(it->is_directory())view.watched_directories.push_back(it->path());}
    std::ranges::sort(view.watched_directories);view.watched_directories.erase(std::unique(view.watched_directories.begin(),view.watched_directories.end()),view.watched_directories.end());return view;
}

bool FilesystemView::has_pending()const{return std::ranges::any_of(entries,[](const auto& entry){return entry.status==FilesystemStatus::Pending||entry.status==FilesystemStatus::Orphaned;});}
const FilesystemEntry* FilesystemView::find(std::string_view path)const{const auto found=std::ranges::find(entries,path,&FilesystemEntry::path);return found==entries.end()?nullptr:&*found;}
std::string_view filesystem_status_name(FilesystemStatus status){switch(status){case FilesystemStatus::Vanilla:return "Vanilla";case FilesystemStatus::Replaced:return "Replaced";case FilesystemStatus::Added:return "Added";case FilesystemStatus::Pending:return "Pending";case FilesystemStatus::Orphaned:return "Orphaned";}return "Unknown";}

} // namespace nsmb::project
