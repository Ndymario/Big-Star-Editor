#include "nsmb/project/module_data.hpp"

#include "nsmb/core/file_io.hpp"
#include "nsmb/core/json.hpp"
#include "nsmb/formats/glue.hpp"

#include <algorithm>
#include <charconv>
#include <limits>
#include <initializer_list>

namespace nsmb::project {
namespace {

core::Result<core::JsonValue> document(std::string_view text, std::string_view name) {
    auto parsed=core::parse_json(text);
    if(!parsed)return parsed.error();
    if(!parsed.value().is_object())return core::make_error(core::ErrorCode::ParseError,std::string(name)+" root is not an object");
    return parsed;
}

core::Result<void> schema(const core::JsonValue& root,std::string_view expected,std::string_view name) {
    if(!root["schema"].is_string()||root["schema"].as_string()!=expected)
        return core::make_error(core::ErrorCode::Unsupported,std::string(name)+" has an unsupported schema");
    return {};
}

core::Result<std::uint32_t> hash_value(const core::JsonValue& node,std::string_view context) {
    if(!node.is_string())return core::make_error(core::ErrorCode::ParseError,std::string(context)+" hash is not a string");
    const auto& text=node.as_string();
    if(text.size()!=10||text[0]!='0'||text[1]!='x')return core::make_error(core::ErrorCode::ParseError,std::string(context)+" hash is not 0x plus eight hex digits");
    if(std::ranges::any_of(std::string_view(text).substr(2),[](char c){return !(c>='0'&&c<='9')&&!(c>='a'&&c<='f');}))
        return core::make_error(core::ErrorCode::ParseError,std::string(context)+" hash must use lowercase hexadecimal");
    std::uint32_t value{};const auto [end,error]=std::from_chars(text.data()+2,text.data()+10,value,16);
    if(error!=std::errc{}||end!=text.data()+10)return core::make_error(core::ErrorCode::ParseError,std::string(context)+" hash is invalid");
    return value;
}

core::Result<std::string> required_string(const core::JsonValue& node,std::string_view key,std::string_view context) {
    if(!node[key].is_string()||node[key].as_string().empty())return core::make_error(core::ErrorCode::ParseError,std::string(context)+" is missing "+std::string(key));
    return node[key].as_string();
}

core::Result<std::string> read_text(const std::filesystem::path& path) {
    auto bytes=core::read_file(path);if(!bytes)return bytes.error();
    return std::string(reinterpret_cast<const char*>(bytes.value().data()),bytes.value().size());
}

core::Result<void> only(const core::JsonValue& node,std::initializer_list<std::string_view> allowed,std::string_view context) {
    for(const auto& [key,value]:node.members()){
        (void)value;
        if(std::ranges::find(allowed,key)==allowed.end())return core::make_error(core::ErrorCode::ParseError,std::string(context)+" contains unknown property "+key);
    }
    return {};
}

} // namespace

core::Result<std::vector<LevelDataDeclaration>> parse_level_data_catalog(std::string_view json) {
    auto root=document(json,"level-data catalog");if(!root)return root.error();
    if(auto ok=only(root.value(),{"schema","levelData"},"level-data catalog");!ok)return ok.error();
    if(auto ok=schema(root.value(),"nsmbtool.leveldata/1","level-data catalog");!ok)return ok.error();
    if(!root.value()["levelData"].is_array())return core::make_error(core::ErrorCode::ParseError,"level-data catalog has no levelData array");
    std::vector<LevelDataDeclaration> out;
    for(const auto& node:root.value()["levelData"].elements()){
        if(!node.is_object())return core::make_error(core::ErrorCode::ParseError,"level-data entry is not an object");
        if(auto ok=only(node,{"key","module","dataType","hash","typeName","arraySize"},"level-data entry");!ok)return ok.error();
        LevelDataDeclaration entry;
        auto key=required_string(node,"key","level-data entry");if(!key)return key.error();entry.key=std::move(key.value());
        auto module=required_string(node,"module",entry.key);if(!module)return module.error();entry.module=std::move(module.value());
        auto hash=hash_value(node["hash"],entry.key);if(!hash)return hash.error();entry.hash=hash.value();
        if(entry.hash!=formats::fnv1a_32(entry.key))return core::make_error(core::ErrorCode::ParseError,entry.key+" does not match its hash");
        auto kind=required_string(node,"dataType",entry.key);if(!kind)return kind.error();
        if(kind.value()=="flag")entry.kind=LevelDataKind::Flag;
        else if(kind.value()=="value")entry.kind=LevelDataKind::Value;
        else if(kind.value()=="array")entry.kind=LevelDataKind::Array;
        else if(kind.value()=="sizedArray")entry.kind=LevelDataKind::SizedArray;
        else return core::make_error(core::ErrorCode::ParseError,entry.key+" has an unknown dataType");
        if(entry.kind!=LevelDataKind::Flag){auto type=required_string(node,"typeName",entry.key);if(!type)return type.error();entry.type_name=std::move(type.value());}
        if(entry.kind==LevelDataKind::Array){const auto count=node["arraySize"].as_int64();if(!node["arraySize"].is_number()||count<1)return core::make_error(core::ErrorCode::ParseError,entry.key+" has an invalid arraySize");entry.array_size=static_cast<std::size_t>(count);}
        out.push_back(std::move(entry));
    }
    return out;
}

core::Result<std::pair<std::uint16_t,std::vector<StageObjectDeclaration>>> parse_stage_object_catalog(std::string_view json) {
    auto root=document(json,"stage-object catalog");if(!root)return root.error();
    if(auto ok=only(root.value(),{"schema","stageObjectIdBase","stageObjects"},"stage-object catalog");!ok)return ok.error();
    if(auto ok=schema(root.value(),"nsmbtool.stageobjects/1","stage-object catalog");!ok)return ok.error();
    const auto base=root.value()["stageObjectIdBase"].as_int64(-1);
    if(!root.value()["stageObjectIdBase"].is_number()||base!=326)return core::make_error(core::ErrorCode::ParseError,"stage-object catalog has an invalid ID base");
    if(!root.value()["stageObjects"].is_array())return core::make_error(core::ErrorCode::ParseError,"stage-object catalog has no stageObjects array");
    std::vector<StageObjectDeclaration> out;
    for(const auto& node:root.value()["stageObjects"].elements()){
        if(!node.is_object())return core::make_error(core::ErrorCode::ParseError,"stage-object entry is not an object");
        if(auto ok=only(node,{"name","hash","module","component","object","variant","objectId","info","header"},"stage-object entry");!ok)return ok.error();
        StageObjectDeclaration entry;
        auto name=required_string(node,"name","stage-object entry");if(!name)return name.error();entry.name=std::move(name.value());
        auto hash=hash_value(node["hash"],entry.name);if(!hash)return hash.error();entry.hash=hash.value();
        if(entry.hash!=formats::fnv1a_32(entry.name))return core::make_error(core::ErrorCode::ParseError,entry.name+" does not match its hash");
        for(auto [key,target]:{std::pair{"module",&entry.module},std::pair{"component",&entry.component},std::pair{"object",&entry.object},std::pair{"variant",&entry.variant},std::pair{"info",&entry.info},std::pair{"header",&entry.header}}){auto value=required_string(node,key,entry.name);if(!value)return value.error();*target=std::move(value.value());}
        const auto object_id=node["objectId"].as_int64(-1);if(!node["objectId"].is_number()||object_id<386||object_id>std::numeric_limits<std::uint32_t>::max())return core::make_error(core::ErrorCode::ParseError,entry.name+" has an invalid objectId");entry.object_id=static_cast<std::uint32_t>(object_id);
        out.push_back(std::move(entry));
    }
    return std::pair{static_cast<std::uint16_t>(base),std::move(out)};
}

core::Result<ModuleDataCatalog> load_module_data_catalog(const std::filesystem::path& project_dir,const std::filesystem::path& generated_dir) {
    ModuleDataCatalog catalog;const auto root=generated_dir.is_absolute()?generated_dir:project_dir/generated_dir;
    std::error_code ec;const auto level=root/"level_data.json",objects=root/"stageobjects.json";
    const bool has_level=std::filesystem::exists(level,ec);if(ec)return core::make_error(core::ErrorCode::IoError,"could not inspect "+level.string());
    if(has_level){auto text=read_text(level);if(!text)return text.error();auto parsed=parse_level_data_catalog(text.value());if(!parsed)return parsed.error();catalog.level_data=std::move(parsed.value());}
    ec.clear();const bool has_objects=std::filesystem::exists(objects,ec);if(ec)return core::make_error(core::ErrorCode::IoError,"could not inspect "+objects.string());
    if(has_objects){auto text=read_text(objects);if(!text)return text.error();auto parsed=parse_stage_object_catalog(text.value());if(!parsed)return parsed.error();catalog.stage_object_id_base=parsed.value().first;catalog.stage_objects=std::move(parsed.value().second);}
    return catalog;
}

const LevelDataDeclaration* ModuleDataCatalog::level_data_by_hash(std::uint32_t hash)const{const auto found=std::ranges::find(level_data,hash,&LevelDataDeclaration::hash);return found==level_data.end()?nullptr:&*found;}
const StageObjectDeclaration* ModuleDataCatalog::stage_object_by_hash(std::uint32_t hash)const{const auto found=std::ranges::find(stage_objects,hash,&StageObjectDeclaration::hash);return found==stage_objects.end()?nullptr:&*found;}

} // namespace nsmb::project
