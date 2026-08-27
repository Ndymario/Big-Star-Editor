#include "nsmb/project/filesystem_view.hpp"

#include "nsmb/core/file_io.hpp"
#include "nsmb/formats/narc.hpp"

#include <algorithm>
#include <array>
#include <fstream>
#include <map>
#include <set>
#include <sstream>

namespace nsmb::project {
namespace {

bool safe_path(std::string_view path) {
    if(path.empty()||path.front()=='/'||path.back()=='/'||path.find('\\')!=path.npos)return false;
    std::size_t start=0;while(start<path.size()){const auto slash=path.find('/',start);const auto end=slash==path.npos?path.size():slash;const auto part=path.substr(start,end-start);
        if(part.empty()||part=="."||part==".."||part.size()>0x7f)return false;if(slash==path.npos)break;start=slash+1;}return true;
}

bool safe_destination(std::string_view path){const auto bang=path.find('!');if(bang!=path.rfind('!'))return false;return safe_path(path.substr(0,bang))&&(bang==path.npos||safe_path(path.substr(bang+1)));}

std::vector<std::string> expand_braces(std::string_view pattern) {
    const auto open=pattern.find('{');if(open==pattern.npos)return {std::string(pattern)};
    const auto close=pattern.find('}',open+1);if(close==pattern.npos)return {std::string(pattern)};
    std::vector<std::string> out;std::size_t start=open+1;
    while(start<=close){const auto comma=pattern.find(',',start);const auto end=comma==pattern.npos||comma>close?close:comma;
        const auto joined=std::string(pattern.substr(0,open))+std::string(pattern.substr(start,end-start))+std::string(pattern.substr(close+1));
        auto nested=expand_braces(joined);out.insert(out.end(),nested.begin(),nested.end());if(end==close)break;start=end+1;}
    return out;
}

bool glob_match_one(std::string_view pattern,std::string_view text,std::size_t p=0,std::size_t t=0) {
    while(p<pattern.size()){
        if(pattern[p]=='*'){
            const bool recursive=p+1<pattern.size()&&pattern[p+1]=='*';p+=recursive?2:1;
            if(recursive&&p<pattern.size()&&pattern[p]=='/')++p;
            for(std::size_t end=t;;++end){if(glob_match_one(pattern,text,p,end))return true;if(end==text.size()||(!recursive&&text[end]=='/'))break;}return false;
        }
        if(t==text.size())return false;
        if(pattern[p]=='?'){if(text[t]=='/')return false;++p;++t;continue;}
        if(pattern[p]=='['){const auto close=pattern.find(']',p+1);if(close==pattern.npos)return false;bool negate=p+1<close&&(pattern[p+1]=='!'||pattern[p+1]=='^');std::size_t i=p+1+negate;bool match=false;
            while(i<close){if(i+2<close&&pattern[i+1]=='-'){match|=text[t]>=pattern[i]&&text[t]<=pattern[i+2];i+=3;}else match|=text[t]==pattern[i++];}
            if(text[t]=='/'||match==negate)return false;p=close+1;++t;continue;}
        if(pattern[p++]!=text[t++])return false;
    }
    return t==text.size();
}

bool glob_match(std::string_view pattern,std::string_view text){for(const auto& expanded:expand_braces(pattern))if(glob_match_one(expanded,text))return true;return false;}

std::string narc_destination(std::string destination) {
    constexpr std::string_view suffix="_narc";std::size_t start=0;
    while(start<destination.size()){
        const auto slash=destination.find('/',start);if(slash==destination.npos)break;
        const auto segment=std::string_view(destination).substr(start,slash-start);
        if(segment.size()>suffix.size()&&segment.ends_with(suffix)){
            const auto stem=start+segment.size()-suffix.size();return destination.substr(0,stem)+".narc!"+destination.substr(slash+1);
        }
        start=slash+1;
    }
    return destination;
}

std::string outer_path(std::string_view path){const auto bang=path.find('!');return std::string(path.substr(0,bang));}

const ModuleVariantMapping* mapping_for(const patching::ModuleInfo& module,std::span<const ModuleVariantMapping> mappings){
    const auto found=std::ranges::find_if(mappings,[&](const auto& mapping){return mapping.module==module.key||mapping.module==module.id;});
    return found==mappings.end()?nullptr:&*found;
}

const patching::ModuleComponent* claiming_component(const patching::ModuleInfo& module,std::string_view destination){
    for(const auto& component:module.components)for(const auto& pattern:component.files)if(glob_match(pattern,destination))return &component;return nullptr;
}

core::Result<std::vector<std::filesystem::path>> files_under(const std::filesystem::path& root){
    std::vector<std::filesystem::path> files;std::error_code ec;
    std::filesystem::recursive_directory_iterator it(root,std::filesystem::directory_options::skip_permission_denied,ec),end;
    if(ec)return core::make_error(core::ErrorCode::IoError,"could not read module tree "+root.string());
    while(it!=end){if(it->is_regular_file(ec))files.push_back(it->path());if(ec)return core::make_error(core::ErrorCode::IoError,"could not inspect module tree "+root.string());it.increment(ec);if(ec)return core::make_error(core::ErrorCode::IoError,"could not continue reading module tree "+root.string());}
    std::ranges::sort(files);return files;
}

bool same_bytes(const std::filesystem::path& left,const std::filesystem::path& right){
    std::error_code ec;if(!std::filesystem::is_regular_file(left,ec)||!std::filesystem::is_regular_file(right,ec))return false;
    if(std::filesystem::file_size(left,ec)!=std::filesystem::file_size(right,ec)||ec)return false;
    std::ifstream a(left,std::ios::binary),b(right,std::ios::binary);std::array<char,65536> x{},y{};
    if(!a||!b)return false;
    while(a&&b){a.read(x.data(),x.size());b.read(y.data(),y.size());if(a.gcount()!=b.gcount()||!std::equal(x.begin(),x.begin()+a.gcount(),y.begin()))return false;}return true;
}

bool archive_matches(const std::filesystem::path& extracted,const std::vector<ModuleFileOffer>& sources){
    auto bytes=core::read_file(extracted);if(!bytes)return false;auto archive=formats::parse_narc(bytes.value());if(!archive)return false;
    for(const auto& source:sources){const auto bang=source.rom_path.find('!');if(bang==source.rom_path.npos)return false;auto member=archive.value().find(source.rom_path.substr(bang+1));if(!member)return false;
        auto expected=core::read_file(source.source);if(!expected||archive.value().files[member.value()]!=expected.value())return false;}
    return true;
}

std::filesystem::path source_path(const FilesystemViewRequest& request,std::string_view source){auto path=std::filesystem::path(source);return path.is_absolute()?path:request.project_dir/path;}

std::string trim(std::string value){const auto first=value.find_first_not_of(" \t\r\n");if(first==value.npos)return{};const auto last=value.find_last_not_of(" \t\r\n");return value.substr(first,last-first+1);}
std::string scalar(std::string value){value=trim(std::move(value));if(value.size()>=2&&((value.front()=='"'&&value.back()=='"')||(value.front()=='\''&&value.back()=='\'')))return value.substr(1,value.size()-2);return value;}

} // namespace

core::Result<std::vector<ModuleFileOffer>> resolve_module_files(const patching::ModuleGraph& graph,std::string_view variant,std::span<const ModuleVariantMapping> mappings){
    struct Winner{ModuleFileOffer offer;int rank{};};std::map<std::string,Winner> chosen;
    std::set<std::string> mapped;
    for(const auto& mapping:mappings){const auto module=std::ranges::find_if(graph.modules,[&](const auto& candidate){return candidate.enabled&&(candidate.key==mapping.module||candidate.id==mapping.module);});
        if(module==graph.modules.end())return core::make_error(core::ErrorCode::InvalidArgument,"variant maps module '"+mapping.module+"', but no such module is enabled");
        if(!module->nitrofs)return core::make_error(core::ErrorCode::InvalidArgument,"variant maps module '"+mapping.module+"', but it has no NitroFS tree");
        if(!module->nitrofs->layered)return core::make_error(core::ErrorCode::InvalidArgument,"variant maps module '"+mapping.module+"', but its NitroFS tree is not layered");
        if(mapping.layer.empty())return core::make_error(core::ErrorCode::InvalidArgument,"variant maps module '"+mapping.module+"' to an empty layer");
        if(!mapped.insert(module->key).second)return core::make_error(core::ErrorCode::InvalidArgument,"variant maps module '"+module->key+"' more than once");}
    for(const auto& module:graph.modules)if(module.enabled&&module.nitrofs){std::error_code ec;if(!std::filesystem::is_directory(module.nitrofs->dir,ec))
        return core::make_error(core::ErrorCode::NotFound,"module NitroFS tree does not exist: "+module.nitrofs->dir.string());}
    for(const int rank:{0,1})for(const auto& module:graph.modules){
        if(!module.enabled||!module.nitrofs)continue;const auto& tree=*module.nitrofs;std::string layer;
        const auto* mapped=mapping_for(module,mappings);
        if(tree.layered)layer=rank==0?tree.base_variant:(mapped?mapped->layer:std::string(variant));else if(rank==1)continue;
        if(tree.layered&&layer.empty())continue;if(rank==1&&layer==tree.base_variant)continue;
        const auto root=tree.layered?tree.dir/layer:tree.dir;std::error_code ec;
        if(!std::filesystem::is_directory(root,ec)){if(rank==1&&mapped)return core::make_error(core::ErrorCode::NotFound,"mapped module layer does not exist: "+root.string());continue;}
        auto files=files_under(root);if(!files)return files.error();
        for(const auto& source:files.value()){
            auto relative=std::filesystem::relative(source,root,ec);if(ec)return core::make_error(core::ErrorCode::IoError,"could not resolve module-tree path");
            auto destination=narc_destination(tree.into.empty()?relative.generic_string():tree.into+"/"+relative.generic_string());
            if(!safe_destination(destination))return core::make_error(core::ErrorCode::ParseError,"module tree produced an unsafe ROM path: "+destination);
            const auto* component=claiming_component(module,destination);if(component&&!component->enabled)continue;
            const auto existing=chosen.find(destination);if(existing!=chosen.end()&&existing->second.rank>=rank)continue;
            chosen[destination]={{destination,source,module.key,component?component->name:std::string{},layer},rank};
        }
    }
    std::vector<ModuleFileOffer> result;result.reserve(chosen.size());for(auto& [path,winner]:chosen)result.push_back(std::move(winner.offer));return result;
}

core::Result<std::filesystem::path> module_file_target(const patching::ModuleInfo& module,std::string_view variant,std::string_view rom_path,std::span<const ModuleVariantMapping> mappings){
    if(!module.enabled||!module.nitrofs)return core::make_error(core::ErrorCode::InvalidArgument,"target module has no enabled NitroFS tree");
    const auto& tree=*module.nitrofs;std::string relative(rom_path);
    if(const auto bang=relative.find('!');bang!=relative.npos){auto archive=relative.substr(0,bang);const auto dot=archive.rfind(".narc");if(dot==archive.npos)return core::make_error(core::ErrorCode::InvalidArgument,"archive destination does not name a NARC");archive.replace(dot,5,"_narc");relative=archive+"/"+relative.substr(bang+1);}
    if(!tree.into.empty()){const auto prefix=tree.into+"/";if(!relative.starts_with(prefix))return core::make_error(core::ErrorCode::InvalidArgument,"ROM path is outside the module's into prefix");relative.erase(0,prefix.size());}
    if(!safe_path(relative))return core::make_error(core::ErrorCode::InvalidArgument,"ROM path is unsafe");
    auto root=tree.dir;if(tree.layered){const auto* mapped=mapping_for(module,mappings);const auto layer=mapped?mapped->layer:std::string(variant.empty()?tree.base_variant:variant);if(layer.empty())return core::make_error(core::ErrorCode::InvalidArgument,"a variant is required for this layered module");root/=layer;if(mapped){std::error_code ec;if(!std::filesystem::is_directory(root,ec))return core::make_error(core::ErrorCode::NotFound,"mapped module layer does not exist: "+root.string());}}
    return root/std::filesystem::path(relative);
}

core::Result<FilesystemView> build_filesystem_view(FilesystemViewRequest request){
    FilesystemView view;auto offers=resolve_module_files(request.modules,request.variant,request.module_variants);if(!offers)return offers.error();
    std::map<std::string,ModuleFileOffer> resolved;for(auto& offer:offers.value())resolved[offer.rom_path]=std::move(offer);
    if(!request.has_current_config&&request.manifest)for(const auto& file:request.manifest->files)if(!file.source.empty()&&file.module.empty())
        resolved[file.path]={file.path,source_path(request,file.source),{}, {},file.from_variant};
    for(auto& file:request.explicit_files){if(!safe_destination(file.rom_path))return core::make_error(core::ErrorCode::ParseError,"explicit file has an unsafe ROM path: "+file.rom_path);file.source=source_path(request,file.source.string());resolved[file.rom_path]=std::move(file);}
    std::map<std::string,std::vector<ModuleFileOffer>> offered;for(auto& [path,offer]:resolved)offered[outer_path(path)].push_back(std::move(offer));
    std::map<std::string,std::filesystem::path> extracted;const auto root=request.extracted_rom/"root";std::error_code ec;
    if(std::filesystem::is_directory(root,ec)){auto files=files_under(root);if(!files)return files.error();for(const auto& file:files.value()){auto relative=std::filesystem::relative(file,root,ec);if(ec)return core::make_error(core::ErrorCode::IoError,"could not resolve extracted ROM path");extracted[relative.generic_string()]=file;}}
    std::map<std::string,const patching::RomFileEntry*> manifested;if(request.manifest)for(const auto& file:request.manifest->files)manifested[file.path]=&file;
    std::set<std::string> paths;for(const auto& [path,file]:extracted)paths.insert(path);for(const auto& [path,file]:manifested)paths.insert(path);for(const auto& [path,file]:offered)paths.insert(path);
    for(const auto& path:paths){FilesystemEntry entry;entry.path=path;const auto disk=extracted.find(path);const auto manifest=manifested.find(path);const auto offer=offered.find(path);if(disk!=extracted.end())entry.extracted=disk->second;
        if(manifest!=manifested.end()){entry.id=manifest->second->id;entry.size=manifest->second->size;entry.module=manifest->second->module;entry.component=manifest->second->component;entry.variant=manifest->second->from_variant;}
        if(offer!=offered.end()){
            const auto& sources=offer->second;const bool archive=sources.size()>1||sources.front().rom_path.find('!')!=std::string::npos;
            if(!archive)entry.source=sources.front().source;
            const auto sameModule=std::ranges::all_of(sources,[&](const auto& source){return source.module==sources.front().module;});
            const auto sameComponent=std::ranges::all_of(sources,[&](const auto& source){return source.component==sources.front().component;});
            const auto sameVariant=std::ranges::all_of(sources,[&](const auto& source){return source.from_variant==sources.front().from_variant;});
            entry.module=sameModule?sources.front().module:std::string{};entry.component=sameComponent?sources.front().component:std::string{};entry.variant=sameVariant?sources.front().from_variant:std::string{};
            if(!archive){std::error_code size_error;entry.size=std::filesystem::file_size(entry.source,size_error);}
            bool agrees=manifest!=manifested.end();
            if(agrees&&(manifest->second->module!=entry.module||manifest->second->component!=entry.component||manifest->second->from_variant!=entry.variant))agrees=false;
            if(agrees&&!archive)agrees=same_bytes(entry.source,entry.extracted);
            if(agrees&&archive)agrees=archive_matches(entry.extracted,sources);
            if(!agrees)entry.status=FilesystemStatus::Pending;else entry.status=(path.starts_with("z_new/")||manifest->second->action==patching::FileAction::Created)?FilesystemStatus::Added:FilesystemStatus::Replaced;
        }else if(manifest!=manifested.end()&&manifest->second->has_provenance()){
            if(manifest->second->module.empty()&&!request.has_current_config){
                entry.source=source_path(request,manifest->second->source);entry.status=path.starts_with("z_new/")?FilesystemStatus::Added:FilesystemStatus::Replaced;
                if(!entry.source.empty()&&!same_bytes(entry.source,entry.extracted))entry.status=FilesystemStatus::Pending;
            }else entry.status=FilesystemStatus::Orphaned;
        }else entry.status=FilesystemStatus::Vanilla;
        view.entries.push_back(std::move(entry));
    }
    for(const auto& module:request.modules.modules)if(module.enabled&&module.nitrofs){view.watched_directories.push_back(module.nitrofs->dir);std::error_code walk_error;for(std::filesystem::recursive_directory_iterator it(module.nitrofs->dir,std::filesystem::directory_options::skip_permission_denied,walk_error),end;!walk_error&&it!=end;it.increment(walk_error))if(it->is_directory())view.watched_directories.push_back(it->path());}
    std::ranges::sort(view.watched_directories);view.watched_directories.erase(std::unique(view.watched_directories.begin(),view.watched_directories.end()),view.watched_directories.end());return view;
}

core::Result<std::vector<ProjectVariantLayers>> parse_project_variant_layers(std::string_view yaml){
    std::vector<ProjectVariantLayers> variants;std::istringstream input{std::string(yaml)};std::string line;bool in_variants=false;ProjectVariantLayers* current=nullptr;bool in_mappings=false;
    while(std::getline(input,line)){
        if(const auto comment=line.find('#');comment!=line.npos)line.erase(comment);if(trim(line).empty())continue;
        const auto indent=line.find_first_not_of(' ');if(indent==line.npos)continue;if(line.find('\t')<indent)return core::make_error(core::ErrorCode::ParseError,"tabs are not supported in variants YAML");
        auto content=trim(line.substr(indent));const auto colon=content.find(':');if(colon==content.npos)continue;const auto key=scalar(content.substr(0,colon));const auto value=trim(content.substr(colon+1));
        if(indent==0){in_variants=key=="variants";current=nullptr;in_mappings=false;continue;}
        if(!in_variants)continue;
        if(indent==2){variants.push_back({key,{}});current=&variants.back();in_mappings=false;continue;}
        if(!current)continue;
        if(indent==4&&key=="module-variants"){
            in_mappings=true;
            if(!value.empty()){
                if(value.front()!='{'||value.back()!='}')return core::make_error(core::ErrorCode::ParseError,"module-variants must be a mapping");
                auto body=value.substr(1,value.size()-2);std::size_t start=0;while(start<body.size()){const auto comma=body.find(',',start);const auto item=trim(body.substr(start,comma==body.npos?body.npos:comma-start));const auto split=item.find(':');if(split==item.npos)return core::make_error(core::ErrorCode::ParseError,"invalid inline module-variants mapping");current->mappings.push_back({scalar(item.substr(0,split)),scalar(item.substr(split+1))});if(comma==body.npos)break;start=comma+1;}
            }
            continue;
        }
        if(in_mappings&&indent==6){if(value.empty())return core::make_error(core::ErrorCode::ParseError,"module variant layer is empty");current->mappings.push_back({key,scalar(value)});continue;}
        if(indent<=4)in_mappings=false;
    }
    return variants;
}

bool FilesystemView::has_pending()const{return std::ranges::any_of(entries,[](const auto& entry){return entry.status==FilesystemStatus::Pending||entry.status==FilesystemStatus::Orphaned;});}
const FilesystemEntry* FilesystemView::find(std::string_view path)const{const auto found=std::ranges::find(entries,path,&FilesystemEntry::path);return found==entries.end()?nullptr:&*found;}
std::string_view filesystem_status_name(FilesystemStatus status){switch(status){case FilesystemStatus::Vanilla:return "Vanilla";case FilesystemStatus::Replaced:return "Replaced";case FilesystemStatus::Added:return "Added";case FilesystemStatus::Pending:return "Pending";case FilesystemStatus::Orphaned:return "Orphaned";}return "Unknown";}

} // namespace nsmb::project
