#include "big_star_web.h"

#include "nsmb/project/editable_level.hpp"

#include <exception>
#include <cmath>
#include <limits>
#include <memory>
#include <optional>
#include <sstream>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

#ifdef __EMSCRIPTEN__
#include <emscripten/emscripten.h>
#define BSE_EXPORT EMSCRIPTEN_KEEPALIVE
#else
#define BSE_EXPORT
#endif

namespace {

using nsmb::project::EditableLevel;

struct Session {
    explicit Session(EditableLevel value) : level(std::move(value)) {}
    EditableLevel level;
    std::string snapshot;
    std::vector<std::uint8_t> course;
    std::vector<std::uint8_t> bgdat;
};

thread_local std::string last_error;
// ponytail: The WebAssembly build is single-threaded; add locking only if a
// future build enables pthreads.
std::unordered_map<std::uint32_t, std::unique_ptr<Session>> sessions;
std::uint32_t next_handle{1};

void fail(std::string message) { last_error = std::move(message); }
void succeed() { last_error.clear(); }

Session* find_session(std::uint32_t handle) {
    const auto found = sessions.find(handle);
    if (found == sessions.end()) {
        fail("level session handle is invalid");
        return nullptr;
    }
    return found->second.get();
}

void invalidate(Session& session) {
    session.snapshot.clear();
    session.course.clear();
    session.bgdat.clear();
}

template <typename T>
bool assign_checked(T& target, double value) {
    if (!std::isfinite(value) || std::trunc(value) != value ||
        value < static_cast<double>(std::numeric_limits<T>::min()) ||
        value > static_cast<double>(std::numeric_limits<T>::max())) {
        fail("field value is outside its stored integer range");
        return false;
    }
    target = static_cast<T>(value);
    return true;
}

template <typename T>
void number(std::ostringstream& out, const char* name, T value, bool comma = true) {
    out << '"' << name << "\":" << +value;
    if (comma) out << ',';
}

void write_snapshot(Session& session) {
    const auto& level = session.level;
    const auto& course = level.course();
    std::ostringstream out;
    out << "{\"schema\":\"bigstar.web-level/1\",\"objects\":[";
    for (std::size_t i = 0; i < level.objects().size(); ++i) {
        if (i) out << ',';
        const auto& value = level.objects()[i]; out << '{';
        number(out,"id",value.object_id);number(out,"slot",value.tileset_slot);number(out,"x",value.x);
        number(out,"y",value.y);number(out,"width",value.width);number(out,"height",value.height,false);out << '}';
    }
    out << "],\"sprites\":[";
    for (std::size_t i = 0; i < course.sprites.size(); ++i) {
        if (i) out << ',';
        const auto& value = course.sprites[i]; out << '{';
        number(out,"id",value.id);number(out,"x",value.x);number(out,"y",value.y);
        number(out,"event0",value.events[0]);number(out,"event1",value.events[1]);number(out,"settings",value.settings,false);
        if (const auto hash=level.sprite_hash(i)) out << ",\"hash\":" << *hash;
        out << '}';
    }
    out << "],\"entrances\":[";
    for (std::size_t i = 0; i < course.entrances.size(); ++i) {
        if (i) out << ',';
        const auto& v=course.entrances[i];out << '{';
        number(out,"x",v.x);number(out,"y",v.y);number(out,"cameraX",v.camera_x);number(out,"cameraY",v.camera_y);
        number(out,"id",v.id);number(out,"targetArea",v.target_stage_area);number(out,"targetStage",v.target_stage_index);
        number(out,"targetGroup",v.target_stage_group);number(out,"targetEntrance",v.target_entrance);number(out,"cameraZoom",v.camera_zoom);
        number(out,"type",v.type);number(out,"flags",v.flags);number(out,"unknown10",v.unknown_10);number(out,"unknown11",v.unknown_11);
        number(out,"viewId",v.view_id);number(out,"worldmapNode",v.target_worldmap_node,false);out << '}';
    }
    out << "],\"views\":[";
    for (std::size_t i=0;i<course.views.size();++i){if(i)out<<',';const auto& v=course.views[i];out<<'{';number(out,"x",v.x);number(out,"y",v.y);number(out,"width",v.width);number(out,"height",v.height);number(out,"id",v.id);number(out,"camera",v.camera_settings_id);number(out,"bgm",v.bgm_id);number(out,"background",v.background_id);number(out,"tileset",v.tileset_id);number(out,"foreground",v.foreground_id);number(out,"light",v.light_type);number(out,"progressPath",v.progress_path_id,false);out<<'}';}
    out << "],\"zones\":[";
    for (std::size_t i=0;i<course.zones.size();++i){if(i)out<<',';const auto& v=course.zones[i];out<<'{';number(out,"x",v.x);number(out,"y",v.y);number(out,"width",v.width);number(out,"height",v.height);number(out,"id",v.id);number(out,"unknown9",v.unknown_9);number(out,"unknownA",v.unknown_a);number(out,"unknownB",v.unknown_b,false);out<<'}';}
    auto paths=[&](const char* name,const auto& values){out<<",\""<<name<<"\":[";for(std::size_t i=0;i<values.size();++i){if(i)out<<',';const auto& path=values[i];out<<'{';number(out,"id",path.id);number(out,"unknown",path.unknown);out<<"\"nodes\":[";for(std::size_t n=0;n<path.nodes.size();++n){if(n)out<<',';const auto& node=path.nodes[n];out<<'{';number(out,"x",node.x);number(out,"y",node.y);number(out,"speedRaw",node.speed.raw());number(out,"accelerationRaw",node.acceleration.raw());number(out,"delay",node.delay);number(out,"flags",node.flags,false);out<<'}';}out<<"]}";}out<<']';};
    out << ']'; paths("paths",course.paths);paths("progressPaths",course.progress_paths);
    const auto refs=level.tileset_references();out<<",\"references\":{";number(out,"tileset",refs.tileset_id);number(out,"backgroundGraphics",refs.background_graphics_id);number(out,"backgroundScreen",refs.background_screen_id);number(out,"foregroundGraphics",refs.foreground_graphics_id);number(out,"foregroundScreen",refs.foreground_screen_id,false);out<<'}';
    out<<",\"objectBanks\":[";for(std::size_t i=0;i<course.object_banks.size();++i){if(i)out<<',';out<<+course.object_banks[i];}
    out<<"],\"canUndo\":"<<(level.can_undo()?"true":"false")<<",\"canRedo\":"<<(level.can_redo()?"true":"false")<<'}';
    session.snapshot = std::move(out).str();
}

int result(Session& session, nsmb::core::Result<void> value) {
    if (!value) { fail(value.error().message); return 0; }
    invalidate(session); succeed(); return 1;
}

template <typename F>
int guarded(F&& action) {
    try { return action(); }
    catch (const std::exception& error) { fail(error.what()); return 0; }
    catch (...) { fail("unexpected backend failure"); return 0; }
}

} // namespace

extern "C" BSE_EXPORT std::uint32_t bse_api_version(void) { return 0x00010000U; }
extern "C" BSE_EXPORT const char* bse_last_error(void) { return last_error.c_str(); }

extern "C" BSE_EXPORT std::uint32_t bse_level_open(const std::uint8_t* course, std::uint32_t course_size,
                                                     const std::uint8_t* bgdat, std::uint32_t bgdat_size) {
    try {
        if ((!course && course_size) || (!bgdat && bgdat_size)) { fail("level input pointer is null"); return 0; }
        auto loaded=EditableLevel::load({course,course_size},{bgdat,bgdat_size});
        if(!loaded){fail(loaded.error().message);return 0;}
        while(next_handle==0||sessions.contains(next_handle))++next_handle;
        const auto handle=next_handle++;sessions.emplace(handle,std::make_unique<Session>(std::move(loaded.value())));succeed();return handle;
    } catch (const std::exception& error) { fail(error.what()); return 0; }
    catch (...) { fail("unexpected backend failure"); return 0; }
}

extern "C" BSE_EXPORT void bse_level_close(std::uint32_t handle) { sessions.erase(handle); }

extern "C" BSE_EXPORT const char* bse_level_snapshot(std::uint32_t handle) {
    try { auto* session=find_session(handle);if(!session)return nullptr;if(session->snapshot.empty())write_snapshot(*session);succeed();return session->snapshot.c_str(); }
    catch(const std::exception& error){fail(error.what());return nullptr;}catch(...){fail("unexpected backend failure");return nullptr;}
}

extern "C" BSE_EXPORT int bse_level_set(std::uint32_t handle,std::uint32_t collection,std::uint32_t index,std::uint32_t field,double value){return guarded([&]{
    auto* s=find_session(handle);if(!s)return 0;auto& level=s->level;
    if(collection==BSE_OBJECT){if(index>=level.objects().size()){fail("object index is out of range");return 0;}if((field==0&&value>0x0fff)||(field==1&&value>0x0f)){fail("object field is outside its format range");return 0;}auto v=level.objects()[index];bool ok=field==0?assign_checked(v.object_id,value):field==1?assign_checked(v.tileset_slot,value):field==2?assign_checked(v.x,value):field==3?assign_checked(v.y,value):field==4?assign_checked(v.width,value):field==5?assign_checked(v.height,value):false;if(!ok){if(field>5)fail("object field is invalid");return 0;}return result(*s,level.update_object(index,v));}
    if(collection==BSE_SPRITE){if(index>=level.course().sprites.size()){fail("sprite index is out of range");return 0;}auto v=level.course().sprites[index];bool ok=field==0?assign_checked(v.id,value):field==1?assign_checked(v.x,value):field==2?assign_checked(v.y,value):field==3?assign_checked(v.events[0],value):field==4?assign_checked(v.events[1],value):field==5?assign_checked(v.settings,value):false;if(!ok){if(field>5)fail("sprite field is invalid");return 0;}return result(*s,level.update_sprite(index,v));}
    if(collection==BSE_ENTRANCE){if(index>=level.course().entrances.size()){fail("entrance index is out of range");return 0;}auto v=level.course().entrances[index];bool ok=false;switch(field){case 0:ok=assign_checked(v.x,value);break;case 1:ok=assign_checked(v.y,value);break;case 2:ok=assign_checked(v.camera_x,value);break;case 3:ok=assign_checked(v.camera_y,value);break;case 4:ok=assign_checked(v.id,value);break;case 5:ok=assign_checked(v.target_stage_area,value);break;case 6:ok=assign_checked(v.target_stage_index,value);break;case 7:ok=assign_checked(v.target_stage_group,value);break;case 8:ok=assign_checked(v.target_entrance,value);break;case 9:ok=assign_checked(v.camera_zoom,value);break;case 10:ok=assign_checked(v.type,value);break;case 11:ok=assign_checked(v.flags,value);break;case 12:ok=assign_checked(v.unknown_10,value);break;case 13:ok=assign_checked(v.unknown_11,value);break;case 14:ok=assign_checked(v.view_id,value);break;case 15:ok=assign_checked(v.target_worldmap_node,value);break;default:fail("entrance field is invalid");return 0;}if(!ok)return 0;return result(*s,level.update_entrance(index,v));}
    if(collection==BSE_VIEW){if(index>=level.course().views.size()){fail("view index is out of range");return 0;}auto v=level.course().views[index];bool ok=false;switch(field){case 0:ok=assign_checked(v.x,value);break;case 1:ok=assign_checked(v.y,value);break;case 2:ok=assign_checked(v.width,value);break;case 3:ok=assign_checked(v.height,value);break;case 4:ok=assign_checked(v.id,value);break;case 5:ok=assign_checked(v.camera_settings_id,value);break;case 6:ok=assign_checked(v.bgm_id,value);break;case 7:ok=assign_checked(v.background_id,value);break;case 8:ok=assign_checked(v.tileset_id,value);break;case 9:ok=assign_checked(v.foreground_id,value);break;case 10:ok=assign_checked(v.light_type,value);break;case 11:ok=assign_checked(v.progress_path_id,value);break;default:fail("view field is invalid");return 0;}if(!ok)return 0;return result(*s,level.update_view(index,v));}
    if(collection==BSE_ZONE){if(index>=level.course().zones.size()){fail("zone index is out of range");return 0;}auto v=level.course().zones[index];bool ok=field==0?assign_checked(v.x,value):field==1?assign_checked(v.y,value):field==2?assign_checked(v.width,value):field==3?assign_checked(v.height,value):field==4?assign_checked(v.id,value):field==5?assign_checked(v.unknown_9,value):field==6?assign_checked(v.unknown_a,value):field==7?assign_checked(v.unknown_b,value):false;if(!ok){if(field>7)fail("zone field is invalid");return 0;}return result(*s,level.update_zone(index,v));}
    if(collection==BSE_PATH||collection==BSE_PROGRESS_PATH){const bool progress=collection==BSE_PROGRESS_PATH;const auto& values=progress?level.course().progress_paths:level.course().paths;if(index>=values.size()){fail("path index is out of range");return 0;}auto v=values[index];bool ok=field==0?assign_checked(v.id,value):field==1?assign_checked(v.unknown,value):false;if(!ok){if(field>1)fail("path field is invalid");return 0;}return result(*s,level.update_path(progress,index,std::move(v)));}
    if(collection==BSE_OBJECT_BANK){if(field!=0){fail("object-bank field is invalid");return 0;}std::uint8_t v{};if(!assign_checked(v,value))return 0;return result(*s,level.set_object_bank(index,v));}
    if(collection==BSE_REFERENCES){auto v=level.tileset_references();bool ok=field==0?assign_checked(v.tileset_id,value):field==1?assign_checked(v.background_graphics_id,value):field==2?assign_checked(v.background_screen_id,value):field==3?assign_checked(v.foreground_graphics_id,value):field==4?assign_checked(v.foreground_screen_id,value):false;if(!ok){if(field>4)fail("reference field is invalid");return 0;}return result(*s,level.set_tileset_references(v));}
    fail("collection is invalid");return 0;});}

extern "C" BSE_EXPORT int bse_level_add(std::uint32_t handle,std::uint32_t collection){return guarded([&]{auto* s=find_session(handle);if(!s)return 0;nsmb::core::Result<std::size_t> added=nsmb::core::make_error(nsmb::core::ErrorCode::InvalidArgument,"collection cannot add rows");switch(collection){case BSE_OBJECT:added=s->level.add_object({});break;case BSE_SPRITE:added=s->level.add_sprite({});break;case BSE_ENTRANCE:added=s->level.add_entrance({});break;case BSE_VIEW:added=s->level.add_view({});break;case BSE_ZONE:added=s->level.add_zone({});break;case BSE_PATH:added=s->level.add_path(false,{});break;case BSE_PROGRESS_PATH:added=s->level.add_path(true,{});break;}if(!added){fail(added.error().message);return 0;}invalidate(*s);succeed();return 1;});}

extern "C" BSE_EXPORT int bse_level_remove(std::uint32_t handle,std::uint32_t collection,std::uint32_t index){return guarded([&]{auto* s=find_session(handle);if(!s)return 0;nsmb::core::Result<void> removed=nsmb::core::make_error(nsmb::core::ErrorCode::InvalidArgument,"collection cannot remove rows");switch(collection){case BSE_OBJECT:removed=s->level.remove_object(index);break;case BSE_SPRITE:removed=s->level.remove_sprite(index);break;case BSE_ENTRANCE:removed=s->level.remove_entrance(index);break;case BSE_VIEW:removed=s->level.remove_view(index);break;case BSE_ZONE:removed=s->level.remove_zone(index);break;case BSE_PATH:removed=s->level.remove_path(false,index);break;case BSE_PROGRESS_PATH:removed=s->level.remove_path(true,index);break;}return result(*s,std::move(removed));});}
extern "C" BSE_EXPORT int bse_level_undo(std::uint32_t handle){return guarded([&]{auto* s=find_session(handle);return s?result(*s,s->level.undo()):0;});}
extern "C" BSE_EXPORT int bse_level_redo(std::uint32_t handle){return guarded([&]{auto* s=find_session(handle);return s?result(*s,s->level.redo()):0;});}

extern "C" BSE_EXPORT int bse_level_serialize(std::uint32_t handle){return guarded([&]{auto* s=find_session(handle);if(!s)return 0;auto course=s->level.serialize_course();if(!course){fail(course.error().message);return 0;}auto bgdat=s->level.serialize_bgdat();if(!bgdat){fail(bgdat.error().message);return 0;}if(course.value().size()>std::numeric_limits<std::uint32_t>::max()||bgdat.value().size()>std::numeric_limits<std::uint32_t>::max()){fail("serialized level exceeds the web ABI size limit");return 0;}s->course=std::move(course.value());s->bgdat=std::move(bgdat.value());succeed();return 1;});}
extern "C" BSE_EXPORT const std::uint8_t* bse_level_course_data(std::uint32_t handle){auto* s=find_session(handle);return s&& !s->course.empty()?s->course.data():nullptr;}
extern "C" BSE_EXPORT std::uint32_t bse_level_course_size(std::uint32_t handle){auto* s=find_session(handle);return s?static_cast<std::uint32_t>(s->course.size()):0;}
extern "C" BSE_EXPORT const std::uint8_t* bse_level_bgdat_data(std::uint32_t handle){auto* s=find_session(handle);return s&&!s->bgdat.empty()?s->bgdat.data():nullptr;}
extern "C" BSE_EXPORT std::uint32_t bse_level_bgdat_size(std::uint32_t handle){auto* s=find_session(handle);return s?static_cast<std::uint32_t>(s->bgdat.size()):0;}
