#include "nsmb/project/editable_level.hpp"

#include <memory>
#include <algorithm>
#include <limits>
#include <string_view>
#include <tuple>
#include <unordered_set>
#include <utility>

namespace nsmb::project {

namespace {

constexpr std::uint32_t stage_objects_hash = formats::fnv1a_32("glue.stageObjects");

std::uint32_t read_u32(std::span<const std::uint8_t> data, std::size_t offset) {
    return static_cast<std::uint32_t>(data[offset]) |
           (static_cast<std::uint32_t>(data[offset + 1]) << 8U) |
           (static_cast<std::uint32_t>(data[offset + 2]) << 16U) |
           (static_cast<std::uint32_t>(data[offset + 3]) << 24U);
}

core::Result<std::vector<std::uint32_t>> read_registration(const formats::GlueBlock& glue) {
    const auto found=std::ranges::find(glue.values,stage_objects_hash,&formats::GlueValue::hash);
    if(found==glue.values.end())return std::vector<std::uint32_t>{};
    if(found->payload.size()<4U)return core::make_error(core::ErrorCode::ParseError,"glue.stageObjects is missing its count");
    const auto count=static_cast<std::uint64_t>(read_u32(found->payload,0));
    if(4ULL+count*4ULL!=found->payload.size())return core::make_error(core::ErrorCode::ParseError,"glue.stageObjects size does not match its count");
    std::vector<std::uint32_t> hashes;hashes.reserve(static_cast<std::size_t>(count));
    for(std::size_t offset=4;offset<found->payload.size();offset+=4)hashes.push_back(read_u32(found->payload,offset));
    return hashes;
}

std::vector<std::uint8_t> write_registration(const std::vector<std::uint32_t>& hashes) {
    std::vector<std::uint8_t> payload(4U+hashes.size()*4U);
    auto write=[&](std::size_t offset,std::uint32_t value){payload[offset]=value;payload[offset+1]=value>>8;payload[offset+2]=value>>16;payload[offset+3]=value>>24;};
    write(0,static_cast<std::uint32_t>(hashes.size()));for(std::size_t i=0;i<hashes.size();++i)write(4U+i*4U,hashes[i]);return payload;
}

void put_u16(std::vector<std::uint8_t>& data, std::size_t offset, std::uint16_t value) {
    data[offset] = static_cast<std::uint8_t>(value & 0xFFU);
    data[offset + 1U] = static_cast<std::uint8_t>((value >> 8U) & 0xFFU);
}

void put_u32(std::vector<std::uint8_t>& data, std::size_t offset, std::uint32_t value) {
    data[offset] = static_cast<std::uint8_t>(value & 0xFFU);
    data[offset + 1U] = static_cast<std::uint8_t>((value >> 8U) & 0xFFU);
    data[offset + 2U] = static_cast<std::uint8_t>((value >> 16U) & 0xFFU);
    data[offset + 3U] = static_cast<std::uint8_t>((value >> 24U) & 0xFFU);
}

core::Result<void> encode_paths(const std::vector<formats::NsmbLevelPath>& paths,
                                std::vector<std::uint8_t>& headers,
                                std::vector<std::uint8_t>& nodes) {
    std::size_t node_count = 0;
    for (const auto& path : paths) {
        if (path.nodes.size() > std::numeric_limits<std::uint16_t>::max() ||
            node_count > std::numeric_limits<std::uint16_t>::max() - path.nodes.size()) {
            return core::make_error(core::ErrorCode::InvalidArgument, "path node table exceeds the NSMB format limit");
        }
        node_count += path.nodes.size();
    }
    if (paths.size() > std::numeric_limits<std::size_t>::max() / 8U || node_count > std::numeric_limits<std::size_t>::max() / 16U) {
        return core::make_error(core::ErrorCode::InvalidArgument, "path data is too large to save");
    }

    headers.assign(paths.size() * 8U, 0);
    nodes.assign(node_count * 16U, 0);
    std::size_t first_node = 0;
    for (std::size_t path_index = 0; path_index < paths.size(); ++path_index) {
        const auto& path = paths[path_index];
        const auto header_offset = path_index * 8U;
        headers[header_offset] = path.id;
        put_u16(headers, header_offset + 2U, static_cast<std::uint16_t>(first_node));
        put_u16(headers, header_offset + 4U, static_cast<std::uint16_t>(path.nodes.size()));
        put_u16(headers, header_offset + 6U, path.unknown);
        for (const auto& node : path.nodes) {
            const auto node_offset = first_node++ * 16U;
            put_u16(nodes, node_offset, static_cast<std::uint16_t>(node.x));
            put_u16(nodes, node_offset + 2U, static_cast<std::uint16_t>(node.y));
            put_u32(nodes, node_offset + 4U, static_cast<std::uint32_t>(node.speed.raw()));
            put_u32(nodes, node_offset + 8U, static_cast<std::uint32_t>(node.acceleration.raw()));
            put_u16(nodes, node_offset + 12U, node.delay);
            put_u16(nodes, node_offset + 14U, node.flags);
        }
    }
    return {};
}

template <typename T>
class ReplaceCommand final : public core::Command {
public:
    ReplaceCommand(std::vector<T>& values, std::size_t index, T replacement, std::string name)
        : values_(&values), index_(index), before_(values[index]), after_(std::move(replacement)), name_(std::move(name)) {}

    std::string_view name() const override { return name_; }
    core::Result<void> execute() override {
        if (index_ >= values_->size()) {
            return core::make_error(core::ErrorCode::OutOfBounds, "item to edit no longer exists");
        }
        (*values_)[index_] = after_;
        return {};
    }
    core::Result<void> undo() override {
        if (index_ >= values_->size()) {
            return core::make_error(core::ErrorCode::OutOfBounds, "edited item no longer exists");
        }
        (*values_)[index_] = before_;
        return {};
    }

private:
    std::vector<T>* values_{};
    std::size_t index_{};
    T before_;
    T after_;
    std::string name_;
};

template <typename T>
class ValueCommand final : public core::Command {
public:
    ValueCommand(T& value, T replacement, std::string name)
        : value_(&value), before_(value), after_(std::move(replacement)), name_(std::move(name)) {}
    std::string_view name() const override { return name_; }
    core::Result<void> execute() override { *value_ = after_; return {}; }
    core::Result<void> undo() override { *value_ = before_; return {}; }
private:
    T* value_; T before_, after_; std::string name_;
};

template <typename T>
class InsertCommand final : public core::Command {
public:
    InsertCommand(std::vector<T>& values, std::size_t index, T value, std::string name)
        : values_(&values), index_(index), value_(std::move(value)), name_(std::move(name)) {}

    std::string_view name() const override { return name_; }
    core::Result<void> execute() override {
        if (index_ > values_->size()) {
            return core::make_error(core::ErrorCode::OutOfBounds, "insert position is outside the collection");
        }
        values_->insert(values_->begin() + static_cast<std::ptrdiff_t>(index_), value_);
        return {};
    }
    core::Result<void> undo() override {
        if (index_ >= values_->size()) {
            return core::make_error(core::ErrorCode::OutOfBounds, "inserted item no longer exists");
        }
        values_->erase(values_->begin() + static_cast<std::ptrdiff_t>(index_));
        return {};
    }

private:
    std::vector<T>* values_{};
    std::size_t index_{};
    T value_;
    std::string name_;
};

template <typename T>
class EraseCommand final : public core::Command {
public:
    EraseCommand(std::vector<T>& values, std::size_t index, std::string name)
        : values_(&values), index_(index), value_(values[index]), name_(std::move(name)) {}

    std::string_view name() const override { return name_; }
    core::Result<void> execute() override {
        if (index_ >= values_->size()) {
            return core::make_error(core::ErrorCode::OutOfBounds, "item to delete no longer exists");
        }
        values_->erase(values_->begin() + static_cast<std::ptrdiff_t>(index_));
        return {};
    }
    core::Result<void> undo() override {
        if (index_ > values_->size()) {
            return core::make_error(core::ErrorCode::OutOfBounds, "delete position is outside the collection");
        }
        values_->insert(values_->begin() + static_cast<std::ptrdiff_t>(index_), value_);
        return {};
    }

private:
    std::vector<T>* values_{};
    std::size_t index_{};
    T value_;
    std::string name_;
};

class TilesetCommand final : public core::Command {
public:
    TilesetCommand(formats::NsmbLevelCourse& course, LevelTilesetReferences before, LevelTilesetReferences after)
        : course_(&course), before_(before), after_(after), had_background_(!course.backgrounds.empty()), had_foreground_(!course.foregrounds.empty()) {}

    std::string_view name() const override { return "Edit tileset references"; }
    core::Result<void> execute() override {
        apply(after_);
        return {};
    }
    core::Result<void> undo() override {
        apply(before_);
        if (!had_background_) course_->backgrounds.clear();
        if (!had_foreground_) course_->foregrounds.clear();
        return {};
    }

private:
    void apply(LevelTilesetReferences value) {
        course_->header.tileset_theme[0] = value.tileset_id;
        course_->header.background_theme[0] = value.background_graphics_id;
        if (course_->backgrounds.empty()) course_->backgrounds.push_back({});
        course_->backgrounds[0].screen_id = value.background_screen_id;
        course_->header.foreground_theme[0] = value.foreground_graphics_id;
        if (course_->foregrounds.empty()) course_->foregrounds.push_back({});
        course_->foregrounds[0].screen_id = value.foreground_screen_id;
    }

    formats::NsmbLevelCourse* course_{};
    LevelTilesetReferences before_;
    LevelTilesetReferences after_;
    bool had_background_{};
    bool had_foreground_{};
};

class GlueCommand final : public core::Command {
public:
    GlueCommand(formats::GlueBlock& target, formats::GlueBlock replacement, std::string name)
        : target_(&target), before_(target), after_(std::move(replacement)), name_(std::move(name)) {}

    std::string_view name() const override { return name_; }
    core::Result<void> execute() override { *target_ = after_; return {}; }
    core::Result<void> undo() override { *target_ = before_; return {}; }

private:
    formats::GlueBlock* target_{};
    formats::GlueBlock before_, after_;
    std::string name_;
};

class CompositeCommand final : public core::Command {
public:
    CompositeCommand(std::vector<std::unique_ptr<core::Command>> commands, std::string name)
        : commands_(std::move(commands)), name_(std::move(name)) {}

    std::string_view name() const override { return name_; }
    core::Result<void> execute() override {
        std::size_t completed = 0;
        for (; completed < commands_.size(); ++completed) {
            auto result = commands_[completed]->execute();
            if (!result) {
                while (completed > 0) {
                    --completed;
                    (void)commands_[completed]->undo();
                }
                return result.error();
            }
        }
        return {};
    }
    core::Result<void> undo() override {
        for (auto command = commands_.rbegin(); command != commands_.rend(); ++command) {
            auto result = (*command)->undo();
            if (!result) {
                return result.error();
            }
        }
        return {};
    }

private:
    std::vector<std::unique_ptr<core::Command>> commands_;
    std::string name_;
};

template <typename T>
core::Result<void> replace_at(core::CommandHistory& history,
                              std::vector<T>& values,
                              std::size_t index,
                              T replacement,
                              std::string name) {
    if (index >= values.size()) {
        return core::make_error(core::ErrorCode::OutOfBounds, "item index is outside the collection");
    }
    return history.execute(std::make_unique<ReplaceCommand<T>>(values, index, std::move(replacement), std::move(name)));
}

template <typename T>
core::Result<std::size_t> append(core::CommandHistory& history, std::vector<T>& values, T value, std::string name) {
    const auto index = values.size();
    auto result = history.execute(std::make_unique<InsertCommand<T>>(values, index, std::move(value), std::move(name)));
    if (!result) {
        return result.error();
    }
    return index;
}

template <typename T>
core::Result<void> erase_at(core::CommandHistory& history, std::vector<T>& values, std::size_t index, std::string name) {
    if (index >= values.size()) {
        return core::make_error(core::ErrorCode::OutOfBounds, "item index is outside the collection");
    }
    return history.execute(std::make_unique<EraseCommand<T>>(values, index, std::move(name)));
}

} // namespace

EditableLevel::EditableLevel(formats::NsmbLevelCourse course,
                             std::vector<formats::NsmbLevelObject> objects,
                             std::array<std::vector<std::uint8_t>, formats::nsmb_level_block_count> raw_blocks,
                             formats::GlueBlock glue,
                             std::optional<std::string> glue_error,
                             std::vector<std::optional<std::uint32_t>> sprite_hashes,
                             std::vector<std::uint32_t> loaded_registration,
                             std::uint16_t stage_object_id_base)
    : course_(std::move(course)), objects_(std::move(objects)), raw_blocks_(std::move(raw_blocks)),
      glue_(std::move(glue)), glue_error_(std::move(glue_error)), sprite_hashes_(std::move(sprite_hashes)),
      loaded_registration_(std::move(loaded_registration)), stage_object_id_base_(stage_object_id_base) {}

core::Result<EditableLevel> EditableLevel::load(std::span<const std::uint8_t> course_data,
                                                std::span<const std::uint8_t> bgdat_data,
                                                const ModuleDataCatalog* catalog) {
    auto course = formats::parse_nsmb_level_course(course_data);
    if (!course) {
        return course.error();
    }
    auto objects = formats::parse_nsmb_bgdat_objects(bgdat_data);
    if (!objects) {
        return objects.error();
    }
    std::array<std::vector<std::uint8_t>, formats::nsmb_level_block_count> raw_blocks;
    for (std::size_t i = 0; i < raw_blocks.size(); ++i) {
        const auto offset = static_cast<std::size_t>(course.value().blocks[i].offset);
        const auto size = static_cast<std::size_t>(course.value().blocks[i].size);
        raw_blocks[i].assign(course_data.begin() + static_cast<std::ptrdiff_t>(offset),
                             course_data.begin() + static_cast<std::ptrdiff_t>(offset + size));
    }
    formats::GlueBlock glue;
    std::optional<std::string> glue_error;
    std::vector<std::uint32_t> registration;
    if(raw_blocks[0].size()>formats::nsmb_header_size){
        auto parsed=formats::parse_glue_block(std::span<const std::uint8_t>(raw_blocks[0]).subspan(formats::nsmb_header_size));
        if(!parsed)glue_error=parsed.error().message;
        else{glue=std::move(parsed.value());auto decoded=read_registration(glue);if(!decoded)glue_error=decoded.error().message;else registration=std::move(decoded.value());}
    }
    const auto base=catalog?catalog->stage_object_id_base:static_cast<std::uint16_t>(326);
    std::vector<std::optional<std::uint32_t>> sprite_hashes(course.value().sprites.size());
    if(!glue_error){for(std::size_t i=0;i<course.value().sprites.size();++i){const auto id=course.value().sprites[i].id;if(id>=base&&static_cast<std::size_t>(id-base)<registration.size())sprite_hashes[i]=registration[id-base];}}
    return EditableLevel(std::move(course.value()), std::move(objects.value()), std::move(raw_blocks),
                         std::move(glue),std::move(glue_error),std::move(sprite_hashes),
                         std::move(registration),base);
}

const formats::NsmbLevelCourse& EditableLevel::course() const noexcept { return course_; }
const std::vector<formats::NsmbLevelObject>& EditableLevel::objects() const noexcept { return objects_; }
const formats::GlueBlock& EditableLevel::glue() const noexcept { return glue_; }
const std::optional<std::string>& EditableLevel::glue_error() const noexcept { return glue_error_; }
std::uint16_t EditableLevel::stage_object_id_base() const noexcept { return stage_object_id_base_; }
std::optional<std::uint32_t> EditableLevel::sprite_hash(std::size_t index) const { return index<sprite_hashes_.size()?sprite_hashes_[index]:std::nullopt; }

std::vector<std::uint32_t> EditableLevel::registration_hashes() const {
    std::unordered_set<std::uint32_t> used;
    for(const auto hash:sprite_hashes_)if(hash)used.insert(*hash);
    std::vector<std::uint32_t> result;
    for(const auto hash:loaded_registration_)if(used.erase(hash))result.push_back(hash);
    for(const auto hash:sprite_hashes_)if(hash&&used.erase(*hash))result.push_back(*hash);
    return result;
}

int EditableLevel::resolve_sprite_object_id(std::size_t index,std::span<const std::uint16_t> vanilla_object_ids,const ModuleDataCatalog& catalog) const {
    if(index>=course_.sprites.size())return -1;
    if(index<sprite_hashes_.size()&&sprite_hashes_[index]){const auto* declaration=catalog.stage_object_by_hash(*sprite_hashes_[index]);return declaration?static_cast<int>(declaration->object_id):-1;}
    const auto id=course_.sprites[index].id;return id<vanilla_object_ids.size()?vanilla_object_ids[id]:-1;
}

LevelTilesetReferences EditableLevel::tileset_references() const noexcept {
    return {course_.header.tileset_theme[0], course_.header.background_theme[0],
            static_cast<std::uint16_t>(course_.backgrounds.empty() ? 0 : course_.backgrounds[0].screen_id),
            course_.header.foreground_theme[0],
            static_cast<std::uint16_t>(course_.foregrounds.empty() ? 0 : course_.foregrounds[0].screen_id)};
}

core::Result<void> EditableLevel::set_glue(formats::GlueBlock glue, std::string name) {
    if (glue_error_) return core::make_error(core::ErrorCode::ParseError, *glue_error_);
    const auto before_registration = read_registration(glue_);
    const auto after_registration = read_registration(glue);
    if (!before_registration || !after_registration || before_registration.value() != after_registration.value())
        return core::make_error(core::ErrorCode::InvalidArgument, "stage-object registration is derived and cannot be edited directly");
    if (auto encoded = formats::write_glue_block(glue); !encoded) return encoded.error();
    return history_.execute(std::make_unique<GlueCommand>(glue_, std::move(glue), std::move(name)));
}

core::Result<std::vector<std::uint8_t>> EditableLevel::serialize_course() const {
    auto blocks = raw_blocks_;
    if (blocks[0].size() < 0x20) return core::make_error(core::ErrorCode::ParseError, "level header is too small to save");
    auto sprites=course_.sprites;
    if(!glue_error_){
        const auto registration=registration_hashes();
        const bool unresolved=std::ranges::any_of(sprites,[&](const auto& sprite){const auto index=&sprite-sprites.data();return sprite.id>=stage_object_id_base_&&!sprite_hashes_[index];});
        if(unresolved&&registration!=loaded_registration_)return core::make_error(core::ErrorCode::InvalidArgument,"custom stage-object registration changed while an unresolved placement still uses it");
        for(std::size_t i=0;i<sprites.size();++i)if(sprite_hashes_[i]){const auto found=std::ranges::find(registration,*sprite_hashes_[i]);if(found==registration.end())return core::make_error(core::ErrorCode::InvalidArgument,"custom stage-object placement has no registration");const auto id=static_cast<std::size_t>(stage_object_id_base_)+static_cast<std::size_t>(found-registration.begin());if(id>std::numeric_limits<std::uint16_t>::max())return core::make_error(core::ErrorCode::InvalidArgument,"custom stage-object IDs exceed the level format");sprites[i].id=static_cast<std::uint16_t>(id);}

        auto glue=glue_;const auto existing=std::ranges::find(glue.values,stage_objects_hash,&formats::GlueValue::hash);
        if(registration.empty()){if(existing!=glue.values.end())glue.values.erase(existing);}
        else if(existing!=glue.values.end())existing->payload=write_registration(registration);
        else{std::size_t order=0;for(const auto& value:glue.values)order=std::max(order,value.payload_order+1U);glue.values.push_back({stage_objects_hash,write_registration(registration),order});}
        auto tail=formats::write_glue_block(glue);if(!tail)return tail.error();
        blocks[0].resize(formats::nsmb_header_size);blocks[0].insert(blocks[0].end(),tail.value().begin(),tail.value().end());
        blocks[0][0]=course_.header.entrances[0]; blocks[0][1]=course_.header.entrances[1];
        put_u16(blocks[0],2,course_.header.settings); put_u16(blocks[0],4,course_.header.timer);
        for(std::size_t i=0;i<3;++i){put_u16(blocks[0],6+i*2,course_.header.background_theme[i]);put_u16(blocks[0],12+i*2,course_.header.tileset_theme[i]);put_u16(blocks[0],18+i*2,course_.header.foreground_theme[i]);}
        put_u16(blocks[0],24,course_.header.events);put_u16(blocks[0],26,course_.header.sound_group);put_u16(blocks[0],28,course_.header.sprite_priority);put_u16(blocks[0],30,course_.header.mega_drop_chance);
    }
    if (!course_.backgrounds.empty()) { if(blocks[2].size()<20) blocks[2].resize(20); put_u16(blocks[2],2,course_.backgrounds[0].screen_id); }
    if (!course_.foregrounds.empty()) { if(blocks[4].size()<20) blocks[4].resize(20); put_u16(blocks[4],2,course_.foregrounds[0].screen_id); }

    blocks[5].assign(course_.entrances.size() * 20U, 0);
    for (std::size_t i = 0; i < course_.entrances.size(); ++i) {
        const auto& entrance = course_.entrances[i];
        const auto offset = i * 20U;
        put_u16(blocks[5], offset, entrance.x);
        put_u16(blocks[5], offset + 2U, entrance.y);
        put_u16(blocks[5], offset + 4U, entrance.camera_x);
        put_u16(blocks[5], offset + 6U, entrance.camera_y);
        blocks[5][offset + 8U] = entrance.id;
        blocks[5][offset + 9U] = entrance.target_stage_area;
        blocks[5][offset + 10U] = entrance.target_stage_index;
        blocks[5][offset + 11U] = entrance.target_stage_group;
        blocks[5][offset + 12U] = entrance.target_entrance;
        blocks[5][offset + 13U] = entrance.camera_zoom;
        blocks[5][offset + 14U] = entrance.type;
        blocks[5][offset + 15U] = entrance.flags;
        blocks[5][offset + 16U] = entrance.unknown_10;
        blocks[5][offset + 17U] = entrance.unknown_11;
        blocks[5][offset + 18U] = entrance.view_id;
        blocks[5][offset + 19U] = entrance.target_worldmap_node;
    }

    blocks[6].assign(sprites.size() * 12U + 4U, 0xFFU);
    for (std::size_t i = 0; i < sprites.size(); ++i) {
        const auto& sprite = sprites[i];
        const auto offset = i * 12U;
        put_u16(blocks[6], offset, sprite.id);
        put_u16(blocks[6], offset + 2U, sprite.x);
        put_u16(blocks[6], offset + 4U, sprite.y);
        blocks[6][offset+6]=sprite.events[0]; blocks[6][offset+7]=sprite.events[1];
        put_u32(blocks[6],offset+8,sprite.settings);
    }

    blocks[7].assign(course_.views.size()*16U,0);
    for(std::size_t i=0;i<course_.views.size();++i){const auto& v=course_.views[i];const auto o=i*16;put_u16(blocks[7],o,v.x);put_u16(blocks[7],o+2,v.y);put_u16(blocks[7],o+4,v.width);put_u16(blocks[7],o+6,v.height);blocks[7][o+8]=v.id;blocks[7][o+9]=v.camera_settings_id;blocks[7][o+10]=v.bgm_id;blocks[7][o+11]=v.background_id;blocks[7][o+12]=v.tileset_id;blocks[7][o+13]=v.foreground_id;blocks[7][o+14]=v.light_type;blocks[7][o+15]=v.progress_path_id;}

    blocks[8].assign(course_.zones.size() * 12U, 0);
    for (std::size_t i = 0; i < course_.zones.size(); ++i) {
        const auto& zone = course_.zones[i];
        const auto offset = i * 12U;
        put_u16(blocks[8], offset, zone.x);
        put_u16(blocks[8], offset + 2U, zone.y);
        put_u16(blocks[8], offset + 4U, zone.width);
        put_u16(blocks[8], offset + 6U, zone.height);
        blocks[8][offset + 8U] = zone.id;
        blocks[8][offset+9]=zone.unknown_9;blocks[8][offset+10]=zone.unknown_a;blocks[8][offset+11]=zone.unknown_b;
    }

    auto paths = encode_paths(course_.paths, blocks[10], blocks[12]);
    if (!paths) {
        return paths.error();
    }
    auto progress_paths = encode_paths(course_.progress_paths, blocks[9], blocks[11]);
    if (!progress_paths) {
        return progress_paths.error();
    }
    if(blocks[13].size()<formats::nsmb_object_bank_slot_count) blocks[13].resize(formats::nsmb_object_bank_slot_count);
    std::copy(course_.object_banks.begin(),course_.object_banks.end(),blocks[13].begin());

    std::size_t file_size = formats::nsmb_level_block_count * 8U;
    for (const auto& block : blocks) {
        if (block.size() > std::numeric_limits<std::uint32_t>::max() || file_size > std::numeric_limits<std::uint32_t>::max() - block.size()) {
            return core::make_error(core::ErrorCode::InvalidArgument, "course file exceeds the NSMB format limit");
        }
        file_size += block.size();
    }
    std::vector<std::uint8_t> data(file_size, 0);
    std::size_t block_offset = formats::nsmb_level_block_count * 8U;
    for (std::size_t i = 0; i < blocks.size(); ++i) {
        put_u32(data, i * 8U, static_cast<std::uint32_t>(block_offset));
        put_u32(data, i * 8U + 4U, static_cast<std::uint32_t>(blocks[i].size()));
        std::copy(blocks[i].begin(), blocks[i].end(), data.begin() + static_cast<std::ptrdiff_t>(block_offset));
        block_offset += blocks[i].size();
    }
    return data;
}

core::Result<std::vector<std::uint8_t>> EditableLevel::serialize_bgdat() const {
    if (objects_.size() > (std::numeric_limits<std::size_t>::max() - 2U) / formats::nsmb_bgdat_object_size) {
        return core::make_error(core::ErrorCode::InvalidArgument, "BGDat object table is too large to save");
    }
    std::vector<std::uint8_t> data(objects_.size() * formats::nsmb_bgdat_object_size + 2U, 0);
    for (std::size_t i = 0; i < objects_.size(); ++i) {
        const auto& object = objects_[i];
        if (object.object_id > 0x0FFFU || object.tileset_slot > 0x0FU) {
            return core::make_error(core::ErrorCode::InvalidArgument, "object ID or tileset slot exceeds the BGDat format limit");
        }
        const auto offset = i * formats::nsmb_bgdat_object_size;
        put_u16(data, offset, static_cast<std::uint16_t>(object.object_id | (object.tileset_slot << 12U)));
        put_u16(data, offset + 2U, static_cast<std::uint16_t>(object.x));
        put_u16(data, offset + 4U, static_cast<std::uint16_t>(object.y));
        put_u16(data, offset + 6U, object.width);
        put_u16(data, offset + 8U, object.height);
    }
    data[data.size() - 2U] = 0xFFU;
    data[data.size() - 1U] = 0xFFU;
    return data;
}

core::Result<void> EditableLevel::set_tileset_references(LevelTilesetReferences references) {
    return history_.execute(std::make_unique<TilesetCommand>(course_, tileset_references(), references));
}

core::Result<void> EditableLevel::replace_serialized(std::span<const std::uint8_t> course_data,
                                                     std::span<const std::uint8_t> bgdat_data,
                                                     const ModuleDataCatalog* catalog,
                                                     std::string name) {
    auto replacement = load(course_data, bgdat_data, catalog); if (!replacement) return replacement.error();
    std::vector<std::unique_ptr<core::Command>> commands;
    commands.push_back(std::make_unique<ValueCommand<formats::NsmbLevelCourse>>(course_, std::move(replacement.value().course_), name));
    commands.push_back(std::make_unique<ValueCommand<std::vector<formats::NsmbLevelObject>>>(objects_, std::move(replacement.value().objects_), name));
    commands.push_back(std::make_unique<ValueCommand<decltype(raw_blocks_)>>(raw_blocks_, std::move(replacement.value().raw_blocks_), name));
    commands.push_back(std::make_unique<ValueCommand<formats::GlueBlock>>(glue_, std::move(replacement.value().glue_), name));
    commands.push_back(std::make_unique<ValueCommand<std::optional<std::string>>>(glue_error_, std::move(replacement.value().glue_error_), name));
    commands.push_back(std::make_unique<ValueCommand<decltype(sprite_hashes_)>>(sprite_hashes_, std::move(replacement.value().sprite_hashes_), name));
    commands.push_back(std::make_unique<ValueCommand<decltype(loaded_registration_)>>(loaded_registration_, std::move(replacement.value().loaded_registration_), name));
    commands.push_back(std::make_unique<ValueCommand<std::uint16_t>>(stage_object_id_base_, replacement.value().stage_object_id_base_, name));
    auto result = history_.execute(std::make_unique<CompositeCommand>(std::move(commands), std::move(name)));
    if (result) refresh_custom_ids(); return result;
}

core::Result<void> EditableLevel::update_object(std::size_t index, formats::NsmbLevelObject object) {
    return replace_at(history_, objects_, index, std::move(object), "Edit object");
}
core::Result<std::size_t> EditableLevel::add_object(formats::NsmbLevelObject object) {
    return append(history_, objects_, std::move(object), "Add object");
}
core::Result<void> EditableLevel::remove_object(std::size_t index) { return erase_at(history_, objects_, index, "Delete object"); }

core::Result<void> EditableLevel::update_sprite(std::size_t index, formats::NsmbLevelSprite sprite) {
    auto result=replace_at(history_, course_.sprites, index, std::move(sprite), "Edit sprite");if(result)refresh_custom_ids();return result;
}
core::Result<std::size_t> EditableLevel::add_sprite(formats::NsmbLevelSprite sprite) {
    const auto index=course_.sprites.size();std::vector<std::unique_ptr<core::Command>> commands;
    commands.push_back(std::make_unique<InsertCommand<formats::NsmbLevelSprite>>(course_.sprites,index,std::move(sprite),"Add sprite"));
    commands.push_back(std::make_unique<InsertCommand<std::optional<std::uint32_t>>>(sprite_hashes_,index,std::nullopt,"Add sprite"));
    auto result=history_.execute(std::make_unique<CompositeCommand>(std::move(commands),"Add sprite"));if(!result)return result.error();refresh_custom_ids();return index;
}
core::Result<std::size_t> EditableLevel::add_custom_sprite(std::uint32_t hash,formats::NsmbLevelSprite sprite) {
    if(hash==0)return core::make_error(core::ErrorCode::InvalidArgument,"custom stage object has the reserved zero hash");
    if(glue_error_)return core::make_error(core::ErrorCode::InvalidArgument,"custom stage object cannot be added while glue is unreadable");
    auto registration=registration_hashes();
    if(std::ranges::find(registration,hash)==registration.end())registration.push_back(hash);
    if(static_cast<std::size_t>(stage_object_id_base_)+registration.size()-1U>std::numeric_limits<std::uint16_t>::max())
        return core::make_error(core::ErrorCode::InvalidArgument,"custom stage-object IDs exceed the level format");
    auto dry_glue=glue_;auto existing=std::ranges::find(dry_glue.values,stage_objects_hash,&formats::GlueValue::hash);
    if(existing!=dry_glue.values.end())existing->payload=write_registration(registration);
    else{std::size_t order=0;for(const auto& value:dry_glue.values)order=std::max(order,value.payload_order+1U);dry_glue.values.push_back({stage_objects_hash,write_registration(registration),order});}
    if(auto dry=formats::write_glue_block(dry_glue);!dry)return dry.error();
    const auto index=course_.sprites.size();std::vector<std::unique_ptr<core::Command>> commands;
    commands.push_back(std::make_unique<InsertCommand<formats::NsmbLevelSprite>>(course_.sprites,index,std::move(sprite),"Add custom sprite"));
    commands.push_back(std::make_unique<InsertCommand<std::optional<std::uint32_t>>>(sprite_hashes_,index,hash,"Add custom sprite"));
    auto result=history_.execute(std::make_unique<CompositeCommand>(std::move(commands),"Add custom sprite"));if(!result)return result.error();refresh_custom_ids();return index;
}
core::Result<void> EditableLevel::remove_sprite(std::size_t index) {
    if(index>=course_.sprites.size())return core::make_error(core::ErrorCode::OutOfBounds,"item index is outside the collection");
    std::vector<std::unique_ptr<core::Command>> commands;commands.push_back(std::make_unique<EraseCommand<formats::NsmbLevelSprite>>(course_.sprites,index,"Delete sprite"));commands.push_back(std::make_unique<EraseCommand<std::optional<std::uint32_t>>>(sprite_hashes_,index,"Delete sprite"));
    auto result=history_.execute(std::make_unique<CompositeCommand>(std::move(commands),"Delete sprite"));if(result)refresh_custom_ids();return result;
}

core::Result<void> EditableLevel::update_entrance(std::size_t index, formats::NsmbLevelEntrance entrance) {
    return replace_at(history_, course_.entrances, index, std::move(entrance), "Edit entrance");
}
core::Result<std::size_t> EditableLevel::add_entrance(formats::NsmbLevelEntrance entrance) {
    return append(history_, course_.entrances, std::move(entrance), "Add entrance");
}
core::Result<void> EditableLevel::remove_entrance(std::size_t index) {
    return erase_at(history_, course_.entrances, index, "Delete entrance");
}
core::Result<void> EditableLevel::update_view(std::size_t index, formats::NsmbLevelView view) { return replace_at(history_, course_.views, index, std::move(view), "Edit view"); }
core::Result<std::size_t> EditableLevel::add_view(formats::NsmbLevelView view) { return append(history_, course_.views, std::move(view), "Add view"); }
core::Result<void> EditableLevel::remove_view(std::size_t index) { return erase_at(history_, course_.views, index, "Delete view"); }
core::Result<void> EditableLevel::set_object_bank(std::size_t slot, std::uint8_t value) {
    if(slot>=course_.object_banks.size()) return core::make_error(core::ErrorCode::OutOfBounds,"object-bank slot is outside the table");
    auto before=course_.object_banks; auto after=before; after[slot]=value;
    class BankCommand final : public core::Command { public: BankCommand(std::array<std::uint8_t,16>& v,decltype(before) b,decltype(after) a):v_(&v),b_(b),a_(a){} std::string_view name()const override{return "Edit object bank";} core::Result<void> execute()override{*v_=a_;return{};} core::Result<void> undo()override{*v_=b_;return{};} private:std::array<std::uint8_t,16>*v_;decltype(before)b_,a_;};
    return history_.execute(std::make_unique<BankCommand>(course_.object_banks,before,after));
}

core::Result<void> EditableLevel::update_zone(std::size_t index, formats::NsmbLevelZone zone) {
    return replace_at(history_, course_.zones, index, std::move(zone), "Edit zone");
}
core::Result<std::size_t> EditableLevel::add_zone(formats::NsmbLevelZone zone) {
    return append(history_, course_.zones, std::move(zone), "Add zone");
}
core::Result<void> EditableLevel::remove_zone(std::size_t index) { return erase_at(history_, course_.zones, index, "Delete zone"); }

core::Result<void> EditableLevel::update_path(bool progress, std::size_t index, formats::NsmbLevelPath path) {
    auto& paths = progress ? course_.progress_paths : course_.paths;
    return replace_at(history_, paths, index, std::move(path), progress ? "Edit progress path" : "Edit path");
}
core::Result<std::size_t> EditableLevel::add_path(bool progress, formats::NsmbLevelPath path) {
    auto& paths = progress ? course_.progress_paths : course_.paths;
    return append(history_, paths, std::move(path), progress ? "Add progress path" : "Add path");
}
core::Result<void> EditableLevel::remove_path(bool progress, std::size_t index) {
    auto& paths = progress ? course_.progress_paths : course_.paths;
    return erase_at(history_, paths, index, progress ? "Delete progress path" : "Delete path");
}

core::Result<void> EditableLevel::update_entities(LevelEntityUpdates updates, std::string name) {
    std::vector<std::unique_ptr<core::Command>> commands;
    auto add = [&]<typename T>(std::vector<T>& values,
                               std::vector<IndexedLevelValue<T>>& replacements,
                               std::string command_name) -> core::Result<void> {
        for (auto& replacement : replacements) {
            if (replacement.index >= values.size()) {
                return core::make_error(core::ErrorCode::OutOfBounds, "item index is outside the collection");
            }
            commands.push_back(std::make_unique<ReplaceCommand<T>>(values,
                                                                    replacement.index,
                                                                    std::move(replacement.value),
                                                                    command_name));
        }
        return {};
    };
    if (auto result = add(objects_, updates.objects, "Edit object"); !result) return result;
    if (auto result = add(course_.sprites, updates.sprites, "Edit sprite"); !result) return result;
    if (auto result = add(course_.entrances, updates.entrances, "Edit entrance"); !result) return result;
    if (auto result = add(course_.views, updates.views, "Edit view"); !result) return result;
    if (auto result = add(course_.zones, updates.zones, "Edit zone"); !result) return result;
    if (auto result = add(course_.paths, updates.paths, "Edit path"); !result) return result;
    if (auto result = add(course_.progress_paths, updates.progress_paths, "Edit progress path"); !result) return result;
    if (commands.empty()) {
        return {};
    }
    auto result=history_.execute(std::make_unique<CompositeCommand>(std::move(commands), std::move(name)));if(result)refresh_custom_ids();return result;
}

core::Result<void> EditableLevel::remove_entities(LevelEntitySelection selection, std::string name) {
    std::vector<std::unique_ptr<core::Command>> commands;
    auto add = [&]<typename T>(std::vector<T>& values,
                               std::vector<std::size_t>& indices,
                               std::string command_name) -> core::Result<void> {
        std::sort(indices.begin(), indices.end(), std::greater<>());
        indices.erase(std::unique(indices.begin(), indices.end()), indices.end());
        for (const auto index : indices) {
            if (index >= values.size()) {
                return core::make_error(core::ErrorCode::OutOfBounds, "item index is outside the collection");
            }
            commands.push_back(std::make_unique<EraseCommand<T>>(values, index, command_name));
        }
        return {};
    };
    if (auto result = add(objects_, selection.objects, "Delete object"); !result) return result;
    if (auto result = add(course_.sprites, selection.sprites, "Delete sprite"); !result) return result;
    for(const auto index:selection.sprites)commands.push_back(std::make_unique<EraseCommand<std::optional<std::uint32_t>>>(sprite_hashes_,index,"Delete sprite"));
    if (auto result = add(course_.entrances, selection.entrances, "Delete entrance"); !result) return result;
    if (auto result = add(course_.views, selection.views, "Delete view"); !result) return result;
    if (auto result = add(course_.zones, selection.zones, "Delete zone"); !result) return result;
    if (auto result = add(course_.paths, selection.paths, "Delete path"); !result) return result;
    if (auto result = add(course_.progress_paths, selection.progress_paths, "Delete progress path"); !result) return result;
    if (commands.empty()) {
        return {};
    }
    auto result=history_.execute(std::make_unique<CompositeCommand>(std::move(commands), std::move(name)));if(result)refresh_custom_ids();return result;
}

core::Result<LevelEntitySelection> EditableLevel::add_entities(LevelEntityInsertions insertions, std::string name) {
    std::vector<std::unique_ptr<core::Command>> commands;
    LevelEntitySelection added;
    auto add = [&]<typename T>(std::vector<T>& target, std::vector<T>& values, std::vector<std::size_t>& indices, std::string command_name) {
        for (auto& value : values) {
            const auto index = target.size() + indices.size();
            indices.push_back(index);
            commands.push_back(std::make_unique<InsertCommand<T>>(target, index, std::move(value), command_name));
        }
    };
    add(objects_, insertions.objects, added.objects, "Paste object");
    if (std::ranges::any_of(insertions.sprites, [](const auto& sprite) { return sprite.hash == std::optional<std::uint32_t>{0}; }))
        return core::make_error(core::ErrorCode::InvalidArgument, "custom stage object has the reserved zero hash");
    if (glue_error_ && std::ranges::any_of(insertions.sprites, [](const auto& sprite) { return sprite.hash.has_value(); }))
        return core::make_error(core::ErrorCode::InvalidArgument, "custom stage objects cannot be pasted while glue is unreadable");
    auto registration = registration_hashes();
    for (const auto& sprite : insertions.sprites)
        if (sprite.hash && std::ranges::find(registration, *sprite.hash) == registration.end()) registration.push_back(*sprite.hash);
    if (static_cast<std::size_t>(stage_object_id_base_) + registration.size() > std::numeric_limits<std::uint16_t>::max() + 1ULL)
        return core::make_error(core::ErrorCode::InvalidArgument, "custom stage-object IDs exceed the level format");
    for (auto& sprite : insertions.sprites) {
        const auto index = course_.sprites.size() + added.sprites.size(); added.sprites.push_back(index);
        commands.push_back(std::make_unique<InsertCommand<formats::NsmbLevelSprite>>(course_.sprites, index, std::move(sprite.value), "Paste sprite"));
        commands.push_back(std::make_unique<InsertCommand<std::optional<std::uint32_t>>>(sprite_hashes_, index, sprite.hash, "Paste sprite"));
    }
    add(course_.entrances, insertions.entrances, added.entrances, "Paste entrance");
    add(course_.views, insertions.views, added.views, "Paste view");
    add(course_.zones, insertions.zones, added.zones, "Paste zone");
    add(course_.paths, insertions.paths, added.paths, "Paste path");
    add(course_.progress_paths, insertions.progress_paths, added.progress_paths, "Paste progress path");
    if (commands.empty()) return added;
    auto result = history_.execute(std::make_unique<CompositeCommand>(std::move(commands), std::move(name)));
    if (!result) return result.error(); refresh_custom_ids(); return added;
}

core::Result<std::size_t> EditableLevel::reorder_entity(int kind, std::size_t index, bool forward) {
    std::vector<std::unique_ptr<core::Command>> commands;
    auto reorder = [&]<typename T>(std::vector<T>& values, std::string name) -> core::Result<std::size_t> {
        if (index >= values.size()) return core::make_error(core::ErrorCode::OutOfBounds, "item index is outside the collection");
        if ((!forward && index == 0) || (forward && index + 1 >= values.size())) return index;
        const auto other = forward ? index + 1 : index - 1;
        commands.push_back(std::make_unique<ReplaceCommand<T>>(values, index, values[other], name));
        commands.push_back(std::make_unique<ReplaceCommand<T>>(values, other, values[index], name));
        return other;
    };
    core::Result<std::size_t> target = core::make_error(core::ErrorCode::InvalidArgument, "entity kind cannot be reordered");
    if (kind == 1) target = reorder(objects_, "Reorder object");
    else if (kind == 2) {
        target = reorder(course_.sprites, "Reorder sprite");
        if (target && target.value() != index) {
            const auto other = target.value();
            commands.push_back(std::make_unique<ReplaceCommand<std::optional<std::uint32_t>>>(sprite_hashes_, index, sprite_hashes_[other], "Reorder sprite"));
            commands.push_back(std::make_unique<ReplaceCommand<std::optional<std::uint32_t>>>(sprite_hashes_, other, sprite_hashes_[index], "Reorder sprite"));
        }
    } else if (kind == 3) target = reorder(course_.entrances, "Reorder entrance");
    else if (kind == 4) target = reorder(course_.zones, "Reorder zone");
    else if (kind == 5) target = reorder(course_.paths, "Reorder path");
    else if (kind == 6) target = reorder(course_.progress_paths, "Reorder progress path");
    if (!target || commands.empty()) return target;
    auto result = history_.execute(std::make_unique<CompositeCommand>(std::move(commands), forward ? "Bring forward" : "Send backward"));
    if (!result) return result.error(); refresh_custom_ids(); return target;
}

void EditableLevel::refresh_custom_ids(){const auto registration=registration_hashes();for(std::size_t i=0;i<course_.sprites.size();++i)if(sprite_hashes_[i]){const auto found=std::ranges::find(registration,*sprite_hashes_[i]);if(found!=registration.end())course_.sprites[i].id=static_cast<std::uint16_t>(stage_object_id_base_+(found-registration.begin()));}}
core::Result<void> EditableLevel::undo() { auto result=history_.undo();if(result)refresh_custom_ids();return result; }
core::Result<void> EditableLevel::redo() { auto result=history_.redo();if(result)refresh_custom_ids();return result; }
bool EditableLevel::can_undo() const noexcept { return history_.can_undo(); }
bool EditableLevel::can_redo() const noexcept { return history_.can_redo(); }
bool EditableLevel::dirty() const noexcept { return history_.dirty(); }
std::string EditableLevel::undo_name() const { return history_.undo_name(); }
std::string EditableLevel::redo_name() const { return history_.redo_name(); }
std::vector<std::string> EditableLevel::undo_names() const { return history_.undo_names(); }
std::vector<std::string> EditableLevel::redo_names() const { return history_.redo_names(); }
void EditableLevel::mark_clean() noexcept { history_.mark_clean(); }

} // namespace nsmb::project
