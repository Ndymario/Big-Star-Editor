#pragma once

#include <QList>
#include <QImage>
#include <QPointF>
#include <QRectF>
#include <QString>
#include <QTransform>
#include <QWidget>

#include <optional>
#include <vector>
#include <array>
#include <memory>
#include <unordered_map>

#include "nsmb/formats/level.hpp"
#include "nsmb/project/editable_level.hpp"
#include "nsmb/project/graphics_resources.hpp"
#include "nsmb/project/lua_actors.hpp"
#include "nsmb/project/project.hpp"
#include "nsmb/project/sprite_definitions.hpp"

class QtEditorTests;

class LevelPreviewWidget final : public QWidget {
    Q_OBJECT

public:
    // Which of the eight resize knobs a drag has hold of.
    enum class Handle { None, TopLeft, Top, TopRight, Right, BottomRight, Bottom, BottomLeft, Left };

    // How far each edge of a resized rectangle has moved, in scene pixels.
    // Grabbing a corner moves two edges and grabbing the top or left moves the
    // origin, neither of which a lone bottom-right handle could express.
    struct ResizeEdges {
        double left{}, top{}, right{}, bottom{};
    };

    explicit LevelPreviewWidget(QWidget* parent = nullptr);

    friend class QtEditorTests;

    void showProjectSummary(const nsmb::project::Project& project, int level_count, int available_level_count);
    void showLevel(const QString& level_name,
                   const QString& world,
                   const QString& area_name,
                   const QString& course_path,
                   const QString& bgdat_path,
                   const QString& source_token,
                   nsmb::project::EditableLevel* level);
    void setSelection(int kind, int item_index, int node_index);
    void setEditable(bool editable);
    // The entity kind right-click creates, in the numbering shared with
    // LevelPropertiesWidget::Kind: 1 object, 2 sprite, 3 entrance, 4 zone,
    // 5 path, 6 progress path, 7 view. Selection is never filtered by it --
    // anything on the canvas can be grabbed whatever this is set to.
    void setCreateKind(int kind);
    [[nodiscard]] int createKind() const noexcept;
    [[nodiscard]] double zoom() const noexcept;
    // The scene-to-widget scale: 1.0 is 1:1, where tile art is pixel exact.
    void setZoom(double zoom);
    // The zoom that fits the whole course in the viewport. Where a level starts.
    [[nodiscard]] double fitZoom() const;
    void zoomToFit();
    [[nodiscard]] QPointF pan() const noexcept;
    void setPan(QPointF pan);
    void setGraphicsResources(nsmb::project::LevelGraphicsResources resources, nsmb::project::LevelTilesetReferences references);
    void setCustomActors(const nsmb::project::ModuleDataCatalog& catalog);
    void setActorDefinitions(const nsmb::project::LuaActorCatalog& catalog);
    void setSpriteDefinitions(const nsmb::project::SpriteDefinitionCatalog& catalog);
    void clearGraphics();
    void invalidateGraphics();
    [[nodiscard]] bool graphicsMatch(nsmb::project::LevelTilesetReferences references) const noexcept;
    [[nodiscard]] nsmb::project::EditableLevel* level() const noexcept;

    struct PaletteEntry {
        int kind{};
        int id{};
        int slot{};
        QString label;
        QString category;
        // What the picker shows on hover: the identifiers you need to match a
        // placement against the ROM, which do not fit under a 40px icon.
        QString tooltip;
        QImage image;
    };
    [[nodiscard]] std::vector<PaletteEntry> paletteEntries();
    [[nodiscard]] QImage customActorImage(std::uint32_t hash) const;
    void setPlacementObject(int slot, int object_id);
    void setPlacementSprite(int sprite_type);
    void setPlacementCustomSprite(std::uint32_t hash);
    void clearPlacement();
    void refreshLevelState();
    void copySelection();
    void cutSelection();
    void pasteSelection();
    void cloneSelection();
    void reorderSelection(bool forward);

signals:
    void selectionRequested(int kind, int item_index, int node_index);
    void multiSelectionRequested(int kind, const QList<int>& item_indices);
    void interactionPreviewRequested(const QPointF& delta, int resize_handle);
    void levelChanged();
    void errorOccurred(const QString& message);
    void graphicsLoaded();
    void graphicsWarning(const QString& message);
    void kindShortcutRequested(int kind);

protected:
    void paintEvent(QPaintEvent* event) override;
    void resizeEvent(QResizeEvent* event) override;
    void changeEvent(QEvent* event) override;
    void mousePressEvent(QMouseEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;
    void wheelEvent(QWheelEvent* event) override;
    void keyPressEvent(QKeyEvent* event) override;
    void keyReleaseEvent(QKeyEvent* event) override;

public:
    struct Scene {
        QString title;
        QString world;
        QString area_name;
        QString course_path;
        QString bgdat_path;
        QString source_token;
        nsmb::project::EditableLevel* level{};
        int selected_kind{};
        int selected_item{};
    int selected_node{};
    };

    struct Selection {
        int kind{};
        int item{};
        int node{-1};
        friend bool operator==(const Selection&, const Selection&) = default;
    };

private:
    void refitIfUnpinned();
    [[nodiscard]] QRectF mapRect() const;
    [[nodiscard]] QTransform sceneTransform() const;
    [[nodiscard]] QPointF toScene(const QPointF& point) const;
    [[nodiscard]] std::optional<Selection> hitTest(const QPointF& scene_point) const;
    // The scene-space rectangle of the single selected resizable entity, if the
    // selection is exactly one object, zone or view.
    [[nodiscard]] std::optional<QRectF> selectedResizableRect() const;
    [[nodiscard]] Handle handleAt(const QPointF& widget_point) const;
    [[nodiscard]] ResizeEdges resizeEdges(const QPointF& delta) const;
    void publishSelection();
    void placeEntity(const QPointF& scene_point);
    void commitMove(const QPointF& delta);
    void commitResize(const ResizeEdges& edges);
    void deleteSelection();
    void rebuildSpriteDefinitions();
    void rebuildActorThumbnails();

    std::optional<Scene> scene_;
    QString message_;
    std::vector<Selection> selections_;
    class QToolBar* toolbar_{};
    int create_kind_{1};
    double zoom_{1.0};
    bool zoom_pinned_{false};
    QPointF pan_;
    QPointF press_position_;
    QPointF last_position_;
    QPointF drag_delta_;
    QRectF marquee_;
    bool editable_{};
    bool panning_{};
    bool dragging_{};
    Handle resize_handle_{Handle::None};
    bool marquee_selecting_{};
    bool space_pan_{};
    bool grid_snap_{true};
    bool show_minimap_{true};
    bool show_grid_{true};
    bool show_ds_screen_{};
    bool dragging_reference_{};
    int snap_size_{16};
    QImage reference_image_;
    QPointF reference_offset_;
    int placement_object_id_{};
    int placement_tileset_slot_{};
    int placement_sprite_type_{};
    std::optional<std::uint32_t> placement_sprite_hash_;
    std::array<std::shared_ptr<nsmb::project::DecodedTileset>, 3> tilesets_;
    QImage background_;
    QImage foreground_;
    std::unordered_map<std::uint64_t, QImage> object_images_;
    std::unordered_map<std::uint16_t, QImage> actor_images_;
    std::unordered_map<std::uint32_t, QImage> custom_actor_images_;
    std::unordered_map<std::uint16_t, QString> actor_labels_;
    std::unordered_map<std::uint16_t, std::uint32_t> actor_object_ids_;
    std::unordered_map<std::uint64_t, nsmb::project::SpriteDefinition> sprite_definitions_;
    nsmb::project::LuaActorCatalog actor_definition_catalog_;
    nsmb::project::SpriteDefinitionCatalog sprite_definition_catalog_;
    std::unordered_map<std::uint32_t, QString> custom_actor_labels_;
    std::uint64_t graphics_generation_{};
    std::optional<nsmb::project::LevelTilesetReferences> graphics_references_;
};
