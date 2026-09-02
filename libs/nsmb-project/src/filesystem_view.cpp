#include "nsmb/project/filesystem_view.hpp"

#include "nsmb/core/file_io.hpp"

#include <algorithm>
#include <iterator>
#include <map>
#include <set>

namespace nsmb::project {
namespace {

bool safe_path(std::string_view path) {
    if(path.empty()||path.front()=='/'||path.back()=='/'||path.find('\\')!=path.npos)return false;
    std::size_t start=0;while(start<path.size()){const auto slash=path.find('/',start);const auto end=slash==path.npos?path.size():slash;const auto part=path.substr(start,end-start);
        if(part.empty()||part=="."||part==".."||part.size()>0x7f)return false;if(slash==path.npos)break;start=slash+1;}return true;
}

// Does the module's copy of a file already match what the ROM holds? The FAT
// answers the size for nothing, and a size that disagrees settles it without
// reading either.
bool source_matches_rom(const std::filesystem::path& source,RomReader& rom,std::string_view path){
    const auto id=rom.file_id(path);if(!id)return false;
    const auto size=rom.file_size(*id);if(!size)return false;
    std::error_code ec;if(!std::filesystem::is_regular_file(source,ec)||std::filesystem::file_size(source,ec)!=*size||ec)return false;
    auto stored=rom.read(*id);if(!stored)return false;
    auto edited=core::read_file(source);if(!edited)return false;
    return stored.value()==edited.value();
}

std::filesystem::path source_path(const std::filesystem::path& project_dir,std::string_view source){auto path=std::filesystem::path(source);return path.is_absolute()?path:project_dir/path;}

// Does the container the ROM holds already carry this member? Addressed by the
// file id the archive's own name table gives it, which is how the game loads a
// member and how the plan reports one; a member path is a label, not an
// address.
bool member_matches_rom(RomReader& rom,std::string_view container,const std::filesystem::path& project_dir,const patching::RomArchiveMember& member){
    // A hook writes this one, so there is nothing on disk to compare and
    // nothing the user can do about it either way.
    if(member.source_missing)return true;
    if(member.id<0)return false;
    auto stored=rom.read_member(container,std::size_t(member.id));if(!stored)return false;
    auto expected=core::read_file(source_path(project_dir,member.source));
    return expected&&stored.value()==expected.value();
}

bool same_member(const patching::RomArchiveMember& built,const patching::RomArchiveMember& planned){
    return built.path==planned.path&&built.module==planned.module&&built.component==planned.component&&built.from_variant==planned.from_variant;
}

const patching::RomArchiveMember* member_at(const std::vector<patching::RomArchiveMember>& members,std::int64_t id){
    const auto found=std::ranges::find(members,id,&patching::RomArchiveMember::id);return found==members.end()?nullptr:&*found;
}

// The set of members a run wrote is part of what it did. A build that replaced
// three members and a plan that would replace two disagree even if those two
// match byte for byte -- so this is asked of the container, not of a member,
// which is why one stale member never drags its siblings down with it.
bool same_members(const std::vector<patching::RomArchiveMember>& built,const std::vector<patching::RomArchiveMember>& planned){
    return std::ranges::equal(built,planned,[](const auto& a,const auto& b){return a.id==b.id&&same_member(a,b);});
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

    std::set<std::string> paths;
    if(request.rom)for(const auto& file:request.rom->files())paths.insert(file.path.generic_string());
    for(const auto& [path,file]:manifested)paths.insert(path);
    for(const auto& [path,file]:planned)paths.insert(path);

    for(const auto& path:paths){
        FilesystemEntry entry;entry.path=path;
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
        }else if(request.rom){
            // Neither document mentions it, so the ROM is the only record there
            // is -- which is the state before a first build.
            if(const auto id=request.rom->file_id(path)){entry.id=*id;entry.size=request.rom->file_size(*id).value_or(0U);}
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

        bool agrees=manifest!=nullptr&&request.rom!=nullptr;
        if(agrees)agrees=manifest->module==plan->module&&manifest->component==plan->component&&manifest->from_variant==plan->from_variant;

        // An archive is not a file with one answer. Each member the plan would
        // write is judged against the ROM's copy at that id on its own, so
        // the view can say which member is stale instead of only that one is.
        // The container then reports what its members add up to, plus the one
        // thing no member can see: whether the set itself has changed.
        std::vector<FilesystemEntry> member_rows;
        for(const auto& member:plan->members){
            FilesystemEntry row;
            row.path=path+"!"+member.path;row.member_id=member.id;
            row.module=member.module;row.component=member.component;row.variant=member.from_variant;
            if(!member.source.empty())row.source=source_path(request.project_dir,member.source);
            if(!member.source_missing)row.size=std::uintmax_t(member.size);
            const auto* built=manifest?member_at(manifest->members,member.id):nullptr;
            const bool written=built&&same_member(*built,member)&&request.rom&&member_matches_rom(*request.rom,path,request.project_dir,member);
            row.status=written?FilesystemStatus::Replaced:FilesystemStatus::Pending;
            if(!written)agrees=false;
            member_rows.push_back(std::move(row));
        }
        if(agrees&&!plan->members.empty())agrees=same_members(manifest->members,plan->members);
        // A source a pre-build hook writes is not on disk to be compared, so
        // there is no honest way to call it stale. Reporting the last build is
        // better than a Pending mark nothing the user does can clear.
        else if(agrees&&plan->members.empty()&&!entry.source.empty()&&!plan->source_missing)agrees=source_matches_rom(entry.source,*request.rom,path);

        entry.status=!agrees?FilesystemStatus::Pending
            :(path.starts_with("z_new/")||manifest->action==patching::FileAction::Created?FilesystemStatus::Added:FilesystemStatus::Replaced);
        view.entries.push_back(std::move(entry));
        view.entries.insert(view.entries.end(),std::make_move_iterator(member_rows.begin()),std::make_move_iterator(member_rows.end()));
    }

    for(const auto& module:request.modules.modules)if(module.enabled&&module.nitrofs){view.watched_directories.push_back(module.nitrofs->dir);std::error_code walk_error;for(std::filesystem::recursive_directory_iterator it(module.nitrofs->dir,std::filesystem::directory_options::skip_permission_denied,walk_error),end;!walk_error&&it!=end;it.increment(walk_error))if(it->is_directory())view.watched_directories.push_back(it->path());}
    std::ranges::sort(view.watched_directories);view.watched_directories.erase(std::unique(view.watched_directories.begin(),view.watched_directories.end()),view.watched_directories.end());return view;
}

bool FilesystemView::has_pending()const{return std::ranges::any_of(entries,[](const auto& entry){return entry.status==FilesystemStatus::Pending||entry.status==FilesystemStatus::Orphaned;});}
const FilesystemEntry* FilesystemView::find(std::string_view path)const{const auto found=std::ranges::find(entries,path,&FilesystemEntry::path);return found==entries.end()?nullptr:&*found;}
std::string_view filesystem_status_name(FilesystemStatus status){switch(status){case FilesystemStatus::Vanilla:return "Vanilla";case FilesystemStatus::Replaced:return "Replaced";case FilesystemStatus::Added:return "Added";case FilesystemStatus::Pending:return "Pending";case FilesystemStatus::Orphaned:return "Orphaned";}return "Unknown";}

} // namespace nsmb::project
