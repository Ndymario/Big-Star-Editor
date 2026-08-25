#include "nsmb/project/editable_level.hpp"

#include <memory>
#include <algorithm>
#include <limits>
#include <string_view>
#include <tuple>
#include <utility>

namespace nsmb::project {

namespace {

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

core::Result<void> patch_byte(std::vector<std::uint8_t>& block, std::size_t offset, std::uint8_t value, std::string_view name) {
    if (offset >= block.size()) {
        if (value == 0) {
            return {};
        }
        return core::make_error(core::ErrorCode::ParseError, std::string(name) + " block is too small to save");
    }
    block[offset] = value;
    return {};
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
        put_u16(headers, header_offset, path.id);
        put_u16(headers, header_offset + 2U, static_cast<std::uint16_t>(first_node));
        put_u16(headers, header_offset + 4U, static_cast<std::uint16_t>(path.nodes.size()));
        put_u16(headers, header_offset + 6U, path.unknown);
        for (const auto& node : path.nodes) {
            const auto node_offset = first_node++ * 16U;
            put_u16(nodes, node_offset, node.x);
            put_u16(nodes, node_offset + 2U, node.y);
            for (std::size_t value_index = 0; value_index < node.values.size(); ++value_index) {
                put_u16(nodes, node_offset + 4U + value_index * 2U, node.values[value_index]);
            }
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
        : course_(&course), before_(before), after_(after) {}

    std::string_view name() const override { return "Edit tileset references"; }
    core::Result<void> execute() override {
        apply(after_);
        return {};
    }
    core::Result<void> undo() override {
        apply(before_);
        return {};
    }

private:
    void apply(LevelTilesetReferences value) {
        course_->tileset_id = value.tileset_id;
        course_->background_graphics_id = value.background_graphics_id;
        course_->background_screen_id = value.background_screen_id;
        course_->foreground_graphics_id = value.foreground_graphics_id;
    }

    formats::NsmbLevelCourse* course_{};
    LevelTilesetReferences before_;
    LevelTilesetReferences after_;
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
                             std::array<std::vector<std::uint8_t>, formats::nsmb_level_block_count> raw_blocks)
    : course_(std::move(course)), objects_(std::move(objects)), raw_blocks_(std::move(raw_blocks)) {}

core::Result<EditableLevel> EditableLevel::load(std::span<const std::uint8_t> course_data,
                                                std::span<const std::uint8_t> bgdat_data) {
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
    return EditableLevel(std::move(course.value()), std::move(objects.value()), std::move(raw_blocks));
}

const formats::NsmbLevelCourse& EditableLevel::course() const noexcept { return course_; }
const std::vector<formats::NsmbLevelObject>& EditableLevel::objects() const noexcept { return objects_; }

LevelTilesetReferences EditableLevel::tileset_references() const noexcept {
    return {course_.tileset_id, course_.background_graphics_id, course_.background_screen_id, course_.foreground_graphics_id};
}

core::Result<std::vector<std::uint8_t>> EditableLevel::serialize_course() const {
    auto blocks = raw_blocks_;
    for (const auto& [block, offset, value, name] : std::array{
             std::tuple<std::size_t, std::size_t, std::uint8_t, std::string_view>{0U, 0x0CU, course_.tileset_id, "tileset"},
             std::tuple<std::size_t, std::size_t, std::uint8_t, std::string_view>{0U, 0x06U, course_.background_graphics_id, "background graphics"},
             std::tuple<std::size_t, std::size_t, std::uint8_t, std::string_view>{0U, 0x12U, course_.foreground_graphics_id, "foreground graphics"},
             std::tuple<std::size_t, std::size_t, std::uint8_t, std::string_view>{2U, 0x02U, course_.background_screen_id, "background screen"},
         }) {
        auto result = patch_byte(blocks[block], offset, value, name);
        if (!result) {
            return result.error();
        }
    }

    blocks[5].assign(course_.entrances.size() * 20U, 0);
    for (std::size_t i = 0; i < course_.entrances.size(); ++i) {
        const auto& entrance = course_.entrances[i];
        const auto offset = i * 20U;
        std::copy(entrance.raw.begin(), entrance.raw.end(), blocks[5].begin() + static_cast<std::ptrdiff_t>(offset));
        if (std::all_of(entrance.raw.begin(), entrance.raw.end(), [](std::uint8_t value) { return value == 0; })) {
            blocks[5][offset + 13U] = 0x10U;
        }
        put_u16(blocks[5], offset, entrance.x);
        put_u16(blocks[5], offset + 2U, entrance.y);
        put_u16(blocks[5], offset + 4U, entrance.camera_x);
        put_u16(blocks[5], offset + 6U, entrance.camera_y);
        blocks[5][offset + 8U] = entrance.number;
        blocks[5][offset + 9U] = entrance.destination_area;
        blocks[5][offset + 10U] = entrance.connected_pipe_id;
        blocks[5][offset + 12U] = entrance.destination_entrance;
        blocks[5][offset + 14U] = entrance.type;
        blocks[5][offset + 15U] = entrance.settings;
        blocks[5][offset + 16U] = entrance.unknown1;
        blocks[5][offset + 18U] = entrance.entry_view;
        blocks[5][offset + 19U] = entrance.unknown2;
    }

    blocks[6].assign(course_.sprites.size() * 12U + 4U, 0xFFU);
    for (std::size_t i = 0; i < course_.sprites.size(); ++i) {
        const auto& sprite = course_.sprites[i];
        const auto offset = i * 12U;
        put_u16(blocks[6], offset, sprite.type);
        put_u16(blocks[6], offset + 2U, sprite.x);
        put_u16(blocks[6], offset + 4U, sprite.y);
        std::copy(sprite.data.begin(), sprite.data.end(), blocks[6].begin() + static_cast<std::ptrdiff_t>(offset + 6U));
    }

    blocks[8].assign(course_.zones.size() * 12U, 0);
    for (std::size_t i = 0; i < course_.zones.size(); ++i) {
        const auto& zone = course_.zones[i];
        const auto offset = i * 12U;
        put_u16(blocks[8], offset, zone.x);
        put_u16(blocks[8], offset + 2U, zone.y);
        put_u16(blocks[8], offset + 4U, zone.width);
        put_u16(blocks[8], offset + 6U, zone.height);
        blocks[8][offset + 8U] = zone.number;
        std::copy(zone.trailing.begin(), zone.trailing.end(), blocks[8].begin() + static_cast<std::ptrdiff_t>(offset + 9U));
    }

    auto paths = encode_paths(course_.paths, blocks[10], blocks[12]);
    if (!paths) {
        return paths.error();
    }
    auto progress_paths = encode_paths(course_.progress_paths, blocks[9], blocks[11]);
    if (!progress_paths) {
        return progress_paths.error();
    }

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

core::Result<void> EditableLevel::update_object(std::size_t index, formats::NsmbLevelObject object) {
    return replace_at(history_, objects_, index, std::move(object), "Edit object");
}
core::Result<std::size_t> EditableLevel::add_object(formats::NsmbLevelObject object) {
    return append(history_, objects_, std::move(object), "Add object");
}
core::Result<void> EditableLevel::remove_object(std::size_t index) { return erase_at(history_, objects_, index, "Delete object"); }

core::Result<void> EditableLevel::update_sprite(std::size_t index, formats::NsmbLevelSprite sprite) {
    return replace_at(history_, course_.sprites, index, std::move(sprite), "Edit sprite");
}
core::Result<std::size_t> EditableLevel::add_sprite(formats::NsmbLevelSprite sprite) {
    return append(history_, course_.sprites, std::move(sprite), "Add sprite");
}
core::Result<void> EditableLevel::remove_sprite(std::size_t index) { return erase_at(history_, course_.sprites, index, "Delete sprite"); }

core::Result<void> EditableLevel::update_entrance(std::size_t index, formats::NsmbLevelEntrance entrance) {
    return replace_at(history_, course_.entrances, index, std::move(entrance), "Edit entrance");
}
core::Result<std::size_t> EditableLevel::add_entrance(formats::NsmbLevelEntrance entrance) {
    return append(history_, course_.entrances, std::move(entrance), "Add entrance");
}
core::Result<void> EditableLevel::remove_entrance(std::size_t index) {
    return erase_at(history_, course_.entrances, index, "Delete entrance");
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

core::Result<void> EditableLevel::undo() { return history_.undo(); }
core::Result<void> EditableLevel::redo() { return history_.redo(); }
bool EditableLevel::can_undo() const noexcept { return history_.can_undo(); }
bool EditableLevel::can_redo() const noexcept { return history_.can_redo(); }
bool EditableLevel::dirty() const noexcept { return history_.dirty(); }
std::string EditableLevel::undo_name() const { return history_.undo_name(); }
std::string EditableLevel::redo_name() const { return history_.redo_name(); }
void EditableLevel::mark_clean() noexcept { history_.mark_clean(); }

} // namespace nsmb::project
