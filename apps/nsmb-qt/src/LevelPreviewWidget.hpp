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
#include "nsmb/project/project.hpp"

class QtEditorTests;

class LevelPreviewWidget final : public QWidget {
    Q_OBJECT

public:
    enum class Tool { Select, Pan, Object, Sprite, Entrance, Zone, Path, ProgressPath };

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
    void setTool(Tool tool);
    [[nodiscard]] Tool tool() const noexcept;
    [[nodiscard]] double zoom() const noexcept;
    void setZoom(double zoom);
    [[nodiscard]] QPointF pan() const noexcept;
    void setPan(QPointF pan);
    void setGraphicsResources(nsmb::project::LevelGraphicsResources resources, nsmb::project::LevelTilesetReferences references);
    void clearGraphics();
    void invalidateGraphics();
    [[nodiscard]] bool graphicsMatch(nsmb::project::LevelTilesetReferences references) const noexcept;
    [[nodiscard]] nsmb::project::EditableLevel* level() const noexcept;

    struct PaletteEntry {
        Tool tool{};
        int id{};
        int slot{};
        QString label;
        QImage image;
    };
    [[nodiscard]] std::vector<PaletteEntry> paletteEntries();
    void setPlacementObject(int slot, int object_id);
    void setPlacementSprite(int sprite_type);
    void setPlacementTool(Tool tool);
    void clearPlacement();
    void refreshLevelState();

signals:
    void selectionRequested(int kind, int item_index, int node_index);
    void multiSelectionRequested(int kind, const QList<int>& item_indices);
    void levelChanged();
    void errorOccurred(const QString& message);
    void graphicsLoaded();
    void graphicsWarning(const QString& message);

protected:
    void paintEvent(QPaintEvent* event) override;
    void resizeEvent(QResizeEvent* event) override;
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
    [[nodiscard]] QRectF mapRect() const;
    [[nodiscard]] QTransform sceneTransform() const;
    [[nodiscard]] QPointF toScene(const QPointF& point) const;
    [[nodiscard]] std::optional<Selection> hitTest(const QPointF& scene_point) const;
    [[nodiscard]] bool selectionMatchesTool(const Selection& selection) const;
    void publishSelection();
    void placeEntity(const QPointF& scene_point);
    void commitMove(const QPointF& delta);
    void commitResize(const QPointF& delta);
    void deleteSelection();

    std::optional<Scene> scene_;
    QString message_;
    std::vector<Selection> selections_;
    class QToolBar* toolbar_{};
    Tool tool_{Tool::Select};
    double zoom_{1.0};
    QPointF pan_;
    QPointF press_position_;
    QPointF last_position_;
    QPointF drag_delta_;
    QRectF marquee_;
    bool editable_{};
    bool panning_{};
    bool dragging_{};
    bool resizing_{};
    bool marquee_selecting_{};
    bool space_pan_{};
    bool grid_snap_{true};
    int placement_object_id_{};
    int placement_tileset_slot_{};
    int placement_sprite_type_{};
    Tool placement_tool_{Tool::Select};
    bool placement_selected_{};
    std::array<std::shared_ptr<nsmb::project::DecodedTileset>, 3> tilesets_;
    QImage background_;
    QImage foreground_;
    std::unordered_map<std::uint64_t, QImage> object_images_;
    std::unordered_map<std::uint16_t, QImage> actor_images_;
    std::unordered_map<std::uint16_t, QString> actor_labels_;
    std::uint64_t graphics_generation_{};
    std::optional<nsmb::project::LevelTilesetReferences> graphics_references_;
};
