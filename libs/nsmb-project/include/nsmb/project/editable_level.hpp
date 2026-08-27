#pragma once

#include <cstddef>
#include <cstdint>
#include <array>
#include <span>
#include <optional>
#include <string>
#include <vector>

#include "nsmb/core/command.hpp"
#include "nsmb/formats/level.hpp"
#include "nsmb/formats/glue.hpp"
#include "nsmb/project/module_data.hpp"

namespace nsmb::project {

struct LevelTilesetReferences {
    std::uint16_t tileset_id{};
    std::uint16_t background_graphics_id{};
    std::uint16_t background_screen_id{};
    std::uint16_t foreground_graphics_id{};
    std::uint16_t foreground_screen_id{};
    bool operator==(const LevelTilesetReferences&) const = default;
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
    std::vector<IndexedLevelValue<formats::NsmbLevelView>> views;
    std::vector<IndexedLevelValue<formats::NsmbLevelZone>> zones;
    std::vector<IndexedLevelValue<formats::NsmbLevelPath>> paths;
    std::vector<IndexedLevelValue<formats::NsmbLevelPath>> progress_paths;
};

struct LevelEntitySelection {
    std::vector<std::size_t> objects;
    std::vector<std::size_t> sprites;
    std::vector<std::size_t> entrances;
    std::vector<std::size_t> views;
    std::vector<std::size_t> zones;
    std::vector<std::size_t> paths;
    std::vector<std::size_t> progress_paths;
};

struct SpriteInsertion {
    formats::NsmbLevelSprite value;
    std::optional<std::uint32_t> hash;
};

struct LevelEntityInsertions {
    std::vector<formats::NsmbLevelObject> objects;
    std::vector<SpriteInsertion> sprites;
    std::vector<formats::NsmbLevelEntrance> entrances;
    std::vector<formats::NsmbLevelView> views;
    std::vector<formats::NsmbLevelZone> zones;
    std::vector<formats::NsmbLevelPath> paths;
    std::vector<formats::NsmbLevelPath> progress_paths;
};

class EditableLevel {
public:
    EditableLevel(EditableLevel&&) noexcept = default;
    EditableLevel& operator=(EditableLevel&&) noexcept = default;
    EditableLevel(const EditableLevel&) = delete;
    EditableLevel& operator=(const EditableLevel&) = delete;

    static core::Result<EditableLevel> load(std::span<const std::uint8_t> course_data,
                                            std::span<const std::uint8_t> bgdat_data,
                                            const ModuleDataCatalog* catalog = nullptr);

    [[nodiscard]] const formats::NsmbLevelCourse& course() const noexcept;
    [[nodiscard]] const std::vector<formats::NsmbLevelObject>& objects() const noexcept;
    [[nodiscard]] LevelTilesetReferences tileset_references() const noexcept;
    [[nodiscard]] const formats::GlueBlock& glue() const noexcept;
    [[nodiscard]] const std::optional<std::string>& glue_error() const noexcept;
    [[nodiscard]] std::uint16_t stage_object_id_base() const noexcept;
    [[nodiscard]] std::optional<std::uint32_t> sprite_hash(std::size_t index) const;
    [[nodiscard]] std::vector<std::uint32_t> registration_hashes() const;
    [[nodiscard]] int resolve_sprite_object_id(std::size_t index,
                                               std::span<const std::uint16_t> vanilla_object_ids,
                                               const ModuleDataCatalog& catalog) const;
    [[nodiscard]] core::Result<std::vector<std::uint8_t>> serialize_course() const;
    [[nodiscard]] core::Result<std::vector<std::uint8_t>> serialize_bgdat() const;

    core::Result<void> set_tileset_references(LevelTilesetReferences references);
    core::Result<void> replace_serialized(std::span<const std::uint8_t> course_data,
                                          std::span<const std::uint8_t> bgdat_data,
                                          const ModuleDataCatalog* catalog = nullptr,
                                          std::string name = "Import level");
    core::Result<void> set_glue(formats::GlueBlock glue, std::string name = "Edit level data");
    core::Result<void> update_object(std::size_t index, formats::NsmbLevelObject object);
    core::Result<std::size_t> add_object(formats::NsmbLevelObject object);
    core::Result<void> remove_object(std::size_t index);
    core::Result<void> update_sprite(std::size_t index, formats::NsmbLevelSprite sprite);
    core::Result<std::size_t> add_sprite(formats::NsmbLevelSprite sprite);
    core::Result<std::size_t> add_custom_sprite(std::uint32_t hash, formats::NsmbLevelSprite sprite);
    core::Result<void> remove_sprite(std::size_t index);
    core::Result<void> update_entrance(std::size_t index, formats::NsmbLevelEntrance entrance);
    core::Result<std::size_t> add_entrance(formats::NsmbLevelEntrance entrance);
    core::Result<void> remove_entrance(std::size_t index);
    core::Result<void> update_view(std::size_t index, formats::NsmbLevelView view);
    core::Result<std::size_t> add_view(formats::NsmbLevelView view);
    core::Result<void> remove_view(std::size_t index);
    core::Result<void> set_object_bank(std::size_t slot, std::uint8_t value);
    core::Result<void> update_zone(std::size_t index, formats::NsmbLevelZone zone);
    core::Result<std::size_t> add_zone(formats::NsmbLevelZone zone);
    core::Result<void> remove_zone(std::size_t index);
    core::Result<void> update_path(bool progress, std::size_t index, formats::NsmbLevelPath path);
    core::Result<std::size_t> add_path(bool progress, formats::NsmbLevelPath path);
    core::Result<void> remove_path(bool progress, std::size_t index);
    core::Result<void> update_entities(LevelEntityUpdates updates, std::string name = "Edit selection");
    core::Result<void> remove_entities(LevelEntitySelection selection, std::string name = "Delete selection");
    core::Result<LevelEntitySelection> add_entities(LevelEntityInsertions insertions,
                                                    std::string name = "Paste selection");
    core::Result<std::size_t> reorder_entity(int kind, std::size_t index, bool forward);

    core::Result<void> undo();
    core::Result<void> redo();
    [[nodiscard]] bool can_undo() const noexcept;
    [[nodiscard]] bool can_redo() const noexcept;
    [[nodiscard]] bool dirty() const noexcept;
    [[nodiscard]] std::string undo_name() const;
    [[nodiscard]] std::string redo_name() const;
    [[nodiscard]] std::vector<std::string> undo_names() const;
    [[nodiscard]] std::vector<std::string> redo_names() const;
    void mark_clean() noexcept;

private:
    EditableLevel(formats::NsmbLevelCourse course,
                  std::vector<formats::NsmbLevelObject> objects,
                  std::array<std::vector<std::uint8_t>, formats::nsmb_level_block_count> raw_blocks,
                  formats::GlueBlock glue,
                  std::optional<std::string> glue_error,
                  std::vector<std::optional<std::uint32_t>> sprite_hashes,
                  std::vector<std::uint32_t> loaded_registration,
                  std::uint16_t stage_object_id_base);

    void refresh_custom_ids();

    formats::NsmbLevelCourse course_;
    std::vector<formats::NsmbLevelObject> objects_;
    std::array<std::vector<std::uint8_t>, formats::nsmb_level_block_count> raw_blocks_;
    formats::GlueBlock glue_;
    std::optional<std::string> glue_error_;
    std::vector<std::optional<std::uint32_t>> sprite_hashes_;
    std::vector<std::uint32_t> loaded_registration_;
    std::uint16_t stage_object_id_base_{326};
    core::CommandHistory history_;
};

} // namespace nsmb::project
