#include "nsmb/project/module_data.hpp"

#include <cassert>
#include <filesystem>

int main() {
    using namespace nsmb::project;
    constexpr auto level_json=R"({
      "schema":"nsmbtool.leveldata/1",
      "levelData":[
        {"key":"coop.canFly","module":"coop","dataType":"flag","hash":"0xc8dfeb91"},
        {"key":"glue.stageObjects","module":"glue","dataType":"sizedArray","hash":"0x6db68f3c","typeName":"u32"}
      ]})";
    auto levels=parse_level_data_catalog(level_json);assert(levels&&levels.value().size()==2);
    assert(levels.value()[0].kind==LevelDataKind::Flag&&levels.value()[1].type_name=="u32");
    assert(!parse_level_data_catalog(R"({"schema":"nsmbtool.leveldata/2","levelData":[]})"));
    assert(!parse_level_data_catalog(R"({"schema":"nsmbtool.leveldata/1","levelData":[{"key":"x","module":"x","dataType":"array","hash":"0xfd0c5087","typeName":"u8","arraySize":0}]})"));

    constexpr auto objects_json=R"({
      "schema":"nsmbtool.stageobjects/1","stageObjectIdBase":326,
      "stageObjects":[
        {"name":"coop.CoopFlagActor.Default","hash":"0xf46a44b9","module":"coop","component":"CustomWorldUnlock","object":"CoopFlagActor","variant":"Default","objectId":387,"info":"CoopFlagActor::ObjectInfo_Default","header":"coop_flag.hpp"},
        {"name":"coop.CoopFlagActor.Big","hash":"0x27e61112","module":"coop","component":"CustomWorldUnlock","object":"CoopFlagActor","variant":"Big","objectId":387,"info":"CoopFlagActor::ObjectInfo_Big","header":"coop_flag.hpp"}
      ]})";
    auto objects=parse_stage_object_catalog(objects_json);assert(objects&&objects.value().first==326&&objects.value().second.size()==2);
    assert(objects.value().second[0].object_id==objects.value().second[1].object_id);
    assert(!parse_stage_object_catalog(R"({"schema":"nsmbtool.stageobjects/2","stageObjectIdBase":326,"stageObjects":[]})"));
    assert(!parse_stage_object_catalog(R"({"schema":"nsmbtool.stageobjects/1","stageObjectIdBase":325,"stageObjects":[]})"));

    const auto missing=load_module_data_catalog(std::filesystem::temp_directory_path()/"big-star-no-generated-data-anywhere");
    assert(missing&&missing.value().level_data.empty()&&missing.value().stage_objects.empty());
    return 0;
}
