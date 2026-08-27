#pragma once

#include <QDialog>

#include <array>
#include <memory>

#include "nsmb/formats/tileset.hpp"
#include "nsmb/project/editable_level.hpp"

class TilesetEditorDialog final : public QDialog {
public:
    TilesetEditorDialog(std::array<nsmb::formats::EditableTileset, 3> tilesets,
                        nsmb::formats::EditableTilemap background,
                        nsmb::formats::EditableTilemap foreground,
                        nsmb::project::LevelTilesetReferences references,
                        QWidget* parent = nullptr);
    ~TilesetEditorDialog() override;

    [[nodiscard]] nsmb::core::Result<nsmb::formats::TilesetResourceData> tilesetData(std::size_t slot) const;
    [[nodiscard]] nsmb::core::Result<nsmb::formats::TilemapResourceData> backgroundData() const;
    [[nodiscard]] nsmb::core::Result<nsmb::formats::TilemapResourceData> foregroundData() const;
    [[nodiscard]] nsmb::project::LevelTilesetReferences references() const;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};
