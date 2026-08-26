#pragma once

#include <cstddef>
#include <cstdint>
#include <array>
#include <span>
#include <string>
#include <vector>

#include "nsmb/core/command.hpp"
#include "nsmb/formats/level.hpp"

namespace nsmb::project {

struct LevelTilesetReferences {
    std::uint8_t tileset_id{};
    std::uint8_t background_graphics_id{};
    std::uint8_t background_screen_id{};
    std::uint8_t foreground_graphics_id{};
};

template <typename T>
struct IndexedLevelValue {
    std::size_t index{};
    T value;
};

struct LevelEntityUpdates {
    std::vector<IndexedLevelValue<formats::NsmbLevelObject>> objects;
    std::vector<IndexedLevelValue<formats::NsmbLevelSprite>> sprites;
    std::vector<IndexedLevelValue<formats::NsmbLevelEntrance>> entrances;
    std::vector<IndexedLevelValue<formats::NsmbLevelZone>> zones;
    std::vector<IndexedLevelValue<formats::NsmbLevelPath>> paths;
    std::vector<IndexedLevelValue<formats::NsmbLevelPath>> progress_paths;
};

struct LevelEntitySelection {
    std::vector<std::size_t> objects;
    std::vector<std::size_t> sprites;
    std::vector<std::size_t> entrances;
    std::vector<std::size_t> zones;
    std::vector<std::size_t> paths;
    std::vector<std::size_t> progress_paths;
};

class EditableLevel {
public:
    EditableLevel(EditableLevel&&) noexcept = default;
    EditableLevel& operator=(EditableLevel&&) noexcept = default;
    EditableLevel(const EditableLevel&) = delete;
    EditableLevel& operator=(const EditableLevel&) = delete;

    static core::Result<EditableLevel> load(std::span<const std::uint8_t> course_data,
                                            std::span<const std::uint8_t> bgdat_data);

    [[nodiscard]] const formats::NsmbLevelCourse& course() const noexcept;
    [[nodiscard]] const std::vector<formats::NsmbLevelObject>& objects() const noexcept;
    [[nodiscard]] LevelTilesetReferences tileset_references() const noexcept;
    [[nodiscard]] core::Result<std::vector<std::uint8_t>> serialize_course() const;
    [[nodiscard]] core::Result<std::vector<std::uint8_t>> serialize_bgdat() const;

    core::Result<void> set_tileset_references(LevelTilesetReferences references);
    core::Result<void> update_object(std::size_t index, formats::NsmbLevelObject object);
    core::Result<std::size_t> add_object(formats::NsmbLevelObject object);
    core::Result<void> remove_object(std::size_t index);
    core::Result<void> update_sprite(std::size_t index, formats::NsmbLevelSprite sprite);
    core::Result<std::size_t> add_sprite(formats::NsmbLevelSprite sprite);
    core::Result<void> remove_sprite(std::size_t index);
    core::Result<void> update_entrance(std::size_t index, formats::NsmbLevelEntrance entrance);
    core::Result<std::size_t> add_entrance(formats::NsmbLevelEntrance entrance);
    core::Result<void> remove_entrance(std::size_t index);
    core::Result<void> update_zone(std::size_t index, formats::NsmbLevelZone zone);
    core::Result<std::size_t> add_zone(formats::NsmbLevelZone zone);
    core::Result<void> remove_zone(std::size_t index);
    core::Result<void> update_path(bool progress, std::size_t index, formats::NsmbLevelPath path);
    core::Result<std::size_t> add_path(bool progress, formats::NsmbLevelPath path);
    core::Result<void> remove_path(bool progress, std::size_t index);
    core::Result<void> update_entities(LevelEntityUpdates updates, std::string name = "Edit selection");
    core::Result<void> remove_entities(LevelEntitySelection selection, std::string name = "Delete selection");

    core::Result<void> undo();
    core::Result<void> redo();
    [[nodiscard]] bool can_undo() const noexcept;
    [[nodiscard]] bool can_redo() const noexcept;
    [[nodiscard]] bool dirty() const noexcept;
    [[nodiscard]] std::string undo_name() const;
    [[nodiscard]] std::string redo_name() const;
    void mark_clean() noexcept;

private:
    EditableLevel(formats::NsmbLevelCourse course,
                  std::vector<formats::NsmbLevelObject> objects,
                  std::array<std::vector<std::uint8_t>, formats::nsmb_level_block_count> raw_blocks);

    formats::NsmbLevelCourse course_;
    std::vector<formats::NsmbLevelObject> objects_;
    std::array<std::vector<std::uint8_t>, formats::nsmb_level_block_count> raw_blocks_;
    core::CommandHistory history_;
};

} // namespace nsmb::project
