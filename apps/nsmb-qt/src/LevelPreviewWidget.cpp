#include "LevelPreviewWidget.hpp"

#include <QApplication>
#include <QFontMetrics>
#include <QActionGroup>
#include <QKeyEvent>
#include <QMouseEvent>
#include <QPainter>
#include <QPaintEvent>
#include <QPointer>
#include <QResizeEvent>
#include <QStringList>
#include <QToolBar>
#include <QThreadPool>
#include <QWheelEvent>

#include <algorithm>
#include <array>
#include <cmath>
#include <limits>
#include <utility>

namespace {

QString elided(const QFontMetrics& metrics, const QString& text, int width) {
    return metrics.elidedText(text, Qt::ElideMiddle, std::max(0, width));
}

QString countsText(const LevelPreviewWidget::Scene& scene) {
    const auto& course = scene.level->course();
    return QString("Objects %1   Sprites %2   Entrances %3   Views %4   Zones %5")
        .arg(static_cast<qulonglong>(scene.level->objects().size()))
        .arg(static_cast<qulonglong>(course.sprites.size()))
        .arg(static_cast<qulonglong>(course.entrances.size()))
        .arg(static_cast<qulonglong>(course.views.size()))
        .arg(static_cast<qulonglong>(course.zones.size()));
}

QString formatIdsText(const nsmb::formats::NsmbLevelCourse& course) {
    return QString("Tileset %1   BG graphics %2   BG screen %3   FG graphics %4")
        .arg(static_cast<int>(course.tileset_id))
        .arg(static_cast<int>(course.background_graphics_id))
        .arg(static_cast<int>(course.background_screen_id))
        .arg(static_cast<int>(course.foreground_graphics_id));
}

QRectF objectRect(const nsmb::formats::NsmbLevelObject& object) {
    return QRectF(object.x * 16.0,
                  object.y * 16.0,
                  std::max<int>(1, object.width) * 16.0,
                  std::max<int>(1, object.height) * 16.0);
}

QRectF spriteRect(const nsmb::formats::NsmbLevelSprite& sprite) {
    return QRectF(sprite.x * 16.0 - 8.0, sprite.y * 16.0 - 8.0, 16.0, 16.0);
}

QRectF entranceRect(const nsmb::formats::NsmbLevelEntrance& entrance) {
    return QRectF(entrance.x, entrance.y, 16.0, 16.0);
}

QRectF viewRect(const nsmb::formats::NsmbLevelView& view) {
    return QRectF(view.x, view.y, std::max<int>(1, view.width), std::max<int>(1, view.height));
}

QRectF zoneRect(const nsmb::formats::NsmbLevelZone& zone) {
    return QRectF(zone.x, zone.y, std::max<int>(1, zone.width), std::max<int>(1, zone.height));
}

void includeRect(QRectF& bounds, bool& has_bounds, const QRectF& rect) {
    if (!has_bounds) {
        bounds = rect;
        has_bounds = true;
    } else {
        bounds = bounds.united(rect);
    }
}

QRectF sceneBounds(const LevelPreviewWidget::Scene& scene) {
    const auto& course = scene.level->course();
    QRectF bounds;
    bool has_bounds = false;
    for (const auto& zone : course.zones) {
        includeRect(bounds, has_bounds, zoneRect(zone));
    }
    for (const auto& view : course.views) {
        includeRect(bounds, has_bounds, viewRect(view));
    }
    for (const auto& object : scene.level->objects()) {
        includeRect(bounds, has_bounds, objectRect(object));
    }
    for (const auto& sprite : course.sprites) {
        includeRect(bounds, has_bounds, spriteRect(sprite));
    }
    for (const auto& entrance : course.entrances) {
        includeRect(bounds, has_bounds, entranceRect(entrance));
    }
    auto includePaths = [&](const auto& paths) {
        for (const auto& path : paths) {
            for (const auto& node : path.nodes) {
                includeRect(bounds, has_bounds, QRectF(node.x, node.y, 16.0, 16.0));
            }
        }
    };
    includePaths(course.paths);
    includePaths(course.progress_paths);
    if (!has_bounds) {
        bounds = QRectF(0.0, 0.0, 512.0 * 16.0, 256.0 * 16.0);
    }
    return bounds.adjusted(-128.0, -128.0, 128.0, 128.0);
}

QColor objectColor(std::uint8_t tileset_slot) {
    switch (tileset_slot) {
    case 0:
        return QColor(63, 126, 197, 135);
    case 1:
        return QColor(72, 156, 103, 145);
    case 2:
        return QColor(221, 143, 54, 145);
    default:
        return QColor(132, 92, 165, 125);
    }
}

QTransform mapTransform(const QRectF& target,
                        const LevelPreviewWidget::Scene& scene,
                        double zoom,
                        const QPointF& pan) {
    const auto bounds = sceneBounds(scene);
    const auto scale = std::min(target.width() / bounds.width(), target.height() / bounds.height()) * zoom;
    const auto left = target.left() + (target.width() - bounds.width() * scale) / 2.0 + pan.x();
    const auto top = target.top() + (target.height() - bounds.height() * scale) / 2.0 + pan.y();
    QTransform transform;
    transform.translate(left, top);
    transform.scale(scale, scale);
    transform.translate(-bounds.left(), -bounds.top());
    return transform;
}

bool selected(const std::vector<LevelPreviewWidget::Selection>& selections, int kind, int item, int node = -1) {
    return std::ranges::any_of(selections, [=](const auto& selection) {
        return selection.kind == kind && selection.item == item && (node < 0 || selection.node == node);
    });
}

struct EntityRect {
    LevelPreviewWidget::Selection selection;
    QRectF rect;
};

std::vector<EntityRect> entityRects(const LevelPreviewWidget::Scene& scene) {
    std::vector<EntityRect> entities;
    const auto& course = scene.level->course();
    for (std::size_t i = 0; i < scene.level->objects().size(); ++i) {
        entities.push_back({{1, static_cast<int>(i), -1}, objectRect(scene.level->objects()[i])});
    }
    for (std::size_t i = 0; i < course.sprites.size(); ++i) {
        entities.push_back({{2, static_cast<int>(i), -1}, spriteRect(course.sprites[i])});
    }
    for (std::size_t i = 0; i < course.entrances.size(); ++i) {
        entities.push_back({{3, static_cast<int>(i), -1}, entranceRect(course.entrances[i])});
    }
    for (std::size_t i = 0; i < course.zones.size(); ++i) {
        entities.push_back({{4, static_cast<int>(i), -1}, zoneRect(course.zones[i])});
    }
    auto add_paths = [&](const auto& paths, int kind) {
        for (std::size_t path = 0; path < paths.size(); ++path) {
            for (std::size_t node = 0; node < paths[path].nodes.size(); ++node) {
                const auto& value = paths[path].nodes[node];
                entities.push_back({{kind, static_cast<int>(path), static_cast<int>(node)}, QRectF(value.x, value.y, 16.0, 16.0)});
            }
        }
    };
    add_paths(course.paths, 5);
    add_paths(course.progress_paths, 6);
    return entities;
}

void drawMap(QPainter& painter,
             const QRectF& target,
             const LevelPreviewWidget::Scene& scene,
             const std::vector<LevelPreviewWidget::Selection>& selections,
             const QPointF& drag_delta,
             double zoom,
             const QPointF& pan,
             const std::array<std::shared_ptr<nsmb::project::DecodedTileset>, 3>& tilesets,
             const QImage& background,
             const QImage& foreground,
             std::unordered_map<std::uint64_t, QImage>& object_images,
             const std::unordered_map<std::uint16_t, QImage>& actor_images,
             const std::unordered_map<std::uint16_t, QString>& actor_labels,
             bool resizing) {
    const auto& course = scene.level->course();
    painter.save();
    painter.setPen(QPen(QColor(190, 198, 208)));
    painter.setBrush(QColor(248, 250, 252));
    painter.drawRect(target);

    const auto bounds = sceneBounds(scene);
    const auto transform = mapTransform(target, scene, zoom, pan);
    const auto scale = transform.m11();

    painter.setClipRect(target);
    painter.setTransform(transform, true);

    painter.setRenderHint(QPainter::Antialiasing, false);
    if (!background.isNull()) {
        painter.setOpacity(0.55);
        painter.drawImage(bounds, background);
        painter.setOpacity(1.0);
    }
    painter.setPen(QPen(QColor(226, 231, 237), 0));
    const auto grid = 256.0;
    const auto x_start = std::floor(bounds.left() / grid) * grid;
    const auto y_start = std::floor(bounds.top() / grid) * grid;
    for (double x = x_start; x <= bounds.right(); x += grid) {
        painter.drawLine(QPointF(x, bounds.top()), QPointF(x, bounds.bottom()));
    }
    for (double y = y_start; y <= bounds.bottom(); y += grid) {
        painter.drawLine(QPointF(bounds.left(), y), QPointF(bounds.right(), y));
    }

    painter.setRenderHint(QPainter::Antialiasing, true);
    for (std::size_t i = 0; i < course.zones.size(); ++i) {
        const auto& zone = course.zones[i];
        auto rect = zoneRect(zone);
        if (!resizing && selected(selections, 4, static_cast<int>(i))) rect.translate(drag_delta);
        if (resizing && selected(selections, 4, static_cast<int>(i))) rect.setBottomRight(rect.bottomRight() + drag_delta);
        painter.setPen(QPen(QColor(84, 154, 110), 0));
        painter.setBrush(QColor(84, 154, 110, 34));
        painter.drawRect(rect);
        if (selected(selections, 4, static_cast<int>(i))) {
            painter.setPen(QPen(QColor(24, 111, 58), 3.0 / scale));
            painter.setBrush(Qt::NoBrush);
            painter.drawRect(rect);
            painter.setBrush(Qt::white);
            painter.drawEllipse(QRectF(rect.bottomRight() - QPointF(7.0 / scale, 7.0 / scale), QSizeF(14.0 / scale, 14.0 / scale)));
        }
    }
    for (const auto& view : course.views) {
        painter.setPen(QPen(QColor(65, 122, 180), 0));
        painter.setBrush(QColor(65, 122, 180, 25));
        painter.drawRect(viewRect(view));
    }
    for (std::size_t i = 0; i < scene.level->objects().size(); ++i) {
        const auto& object = scene.level->objects()[i];
        auto rect = objectRect(object);
        if (!resizing && selected(selections, 1, static_cast<int>(i))) rect.translate(drag_delta);
        if (resizing && selected(selections, 1, static_cast<int>(i))) rect.setBottomRight(rect.bottomRight() + drag_delta);
        painter.setPen(QPen(QColor(38, 46, 58), 0));
        bool rendered = false;
        if (object.tileset_slot < tilesets.size() && tilesets[object.tileset_slot]) {
            const auto image_key = (static_cast<std::uint64_t>(object.tileset_slot) << 56U) |
                                   (static_cast<std::uint64_t>(object.object_id) << 32U) |
                                   (static_cast<std::uint64_t>(object.width) << 16U) | object.height;
            auto image = object_images.find(image_key);
            if (image == object_images.end()) {
                auto decoded = nsmb::project::render_tileset_object(*tilesets[object.tileset_slot],
                                                                    object.object_id,
                                                                    std::max<std::size_t>(1, object.width),
                                                                    std::max<std::size_t>(1, object.height));
                if (decoded) {
                    const auto& value = decoded.value();
                    QImage converted(value.pixels.data(),
                                     static_cast<int>(value.width),
                                     static_cast<int>(value.height),
                                     static_cast<int>(value.width * 4U),
                                     QImage::Format_RGBA8888);
                    image = object_images.emplace(image_key, converted.copy()).first;
                }
            }
            if (image != object_images.end()) {
                painter.drawImage(rect, image->second);
                rendered = true;
            }
        }
        if (!rendered) {
            painter.setBrush(objectColor(object.tileset_slot));
            painter.drawRoundedRect(rect, 2.0 / scale, 2.0 / scale);
        }
        if (selected(selections, 1, static_cast<int>(i))) {
            painter.setPen(QPen(Qt::white, 3.0 / scale));
            painter.setBrush(Qt::NoBrush);
            painter.drawRect(rect);
            painter.setBrush(Qt::white);
            painter.drawEllipse(QRectF(rect.bottomRight() - QPointF(7.0 / scale, 7.0 / scale), QSizeF(14.0 / scale, 14.0 / scale)));
        }
    }
    for (std::size_t i = 0; i < course.sprites.size(); ++i) {
        const auto& sprite = course.sprites[i];
        auto rect = spriteRect(sprite);
        if (selected(selections, 2, static_cast<int>(i))) rect.translate(drag_delta);
        const QPointF center = rect.center();
        if (const auto actor = actor_images.find(sprite.type); actor != actor_images.end()) {
            const QSizeF size(actor->second.width(), actor->second.height());
            painter.drawImage(QRectF(center - QPointF(size.width() / 2.0, size.height() / 2.0), size), actor->second);
        } else {
            QPolygonF diamond;
            diamond << QPointF(center.x(), rect.top()) << QPointF(rect.right(), center.y()) << QPointF(center.x(), rect.bottom())
                    << QPointF(rect.left(), center.y());
            painter.setPen(QPen(QColor(135, 78, 29), 0));
            painter.setBrush(QColor(238, 178, 75, 190));
            painter.drawPolygon(diamond);
            const auto label = actor_labels.contains(sprite.type) ? actor_labels.at(sprite.type) : QString("Actor %1").arg(sprite.type);
            painter.setPen(QPen(QColor(100, 55, 20), 0));
            painter.drawText(QRectF(rect.right() + 2.0, rect.top() - 2.0, 120.0, 20.0), label);
        }
        if (selected(selections, 2, static_cast<int>(i))) {
            painter.setPen(QPen(Qt::white, 3.0 / scale));
            painter.setBrush(Qt::NoBrush);
            painter.drawRect(rect.adjusted(-3.0, -3.0, 3.0, 3.0));
        }
    }
    for (std::size_t i = 0; i < course.entrances.size(); ++i) {
        const auto& entrance = course.entrances[i];
        auto rect = entranceRect(entrance);
        if (selected(selections, 3, static_cast<int>(i))) rect.translate(drag_delta);
        painter.setPen(QPen(QColor(34, 94, 122), 0));
        painter.setBrush(QColor(92, 190, 214, 190));
        painter.drawEllipse(rect);
        if (selected(selections, 3, static_cast<int>(i))) {
            painter.setPen(QPen(Qt::white, 3.0 / scale));
            painter.setBrush(Qt::NoBrush);
            painter.drawEllipse(rect.adjusted(-3.0, -3.0, 3.0, 3.0));
        }
    }

    auto drawPaths = [&](const auto& paths, bool progress) {
        for (std::size_t path_index = 0; path_index < paths.size(); ++path_index) {
            const auto& path = paths[path_index];
            painter.setPen(QPen(progress ? QColor(155, 92, 180) : QColor(199, 76, 76), 2.0 / scale));
            for (std::size_t node_index = 1; node_index < path.nodes.size(); ++node_index) {
                const auto& before = path.nodes[node_index - 1U];
                const auto& node = path.nodes[node_index];
                auto before_point = QPointF(before.x + 8.0, before.y + 8.0);
                auto node_point = QPointF(node.x + 8.0, node.y + 8.0);
                if (selected(selections, progress ? 6 : 5, static_cast<int>(path_index), static_cast<int>(node_index - 1U))) {
                    before_point += drag_delta;
                }
                if (selected(selections, progress ? 6 : 5, static_cast<int>(path_index), static_cast<int>(node_index))) {
                    node_point += drag_delta;
                }
                painter.drawLine(before_point, node_point);
            }
            for (std::size_t node_index = 0; node_index < path.nodes.size(); ++node_index) {
                const auto& node = path.nodes[node_index];
                QRectF rect(node.x, node.y, 16.0, 16.0);
                if (selected(selections, progress ? 6 : 5, static_cast<int>(path_index), static_cast<int>(node_index))) {
                    rect.translate(drag_delta);
                }
                painter.setBrush(node_index == 0 ? QColor(79, 178, 98) : QColor(235, 105, 95));
                painter.drawEllipse(rect);
                if (selected(selections, progress ? 6 : 5, static_cast<int>(path_index), static_cast<int>(node_index))) {
                    painter.setPen(QPen(Qt::white, 3.0 / scale));
                    painter.setBrush(Qt::NoBrush);
                    painter.drawEllipse(rect.adjusted(-3.0, -3.0, 3.0, 3.0));
                }
            }
        }
    };
    drawPaths(course.paths, false);
    drawPaths(course.progress_paths, true);

    if (!foreground.isNull()) {
        painter.setOpacity(0.35);
        painter.drawImage(bounds, foreground);
        painter.setOpacity(1.0);
    }

    painter.restore();
}

} // namespace

LevelPreviewWidget::LevelPreviewWidget(QWidget* parent) : QWidget(parent) {
    setMinimumSize(640, 420);
    setAutoFillBackground(false);
    setFocusPolicy(Qt::StrongFocus);
    setMouseTracking(true);

    toolbar_ = new QToolBar(this);
    toolbar_->setMovable(false);
    toolbar_->setIconSize(QSize(16, 16));
    auto* tools = new QActionGroup(toolbar_);
    tools->setExclusive(true);
    const std::array names{"Select", "Pan", "Object", "Sprite", "Entrance", "Zone", "Path", "Progress Path"};
    for (int i = 0; i < static_cast<int>(names.size()); ++i) {
        auto* action = toolbar_->addAction(names[static_cast<std::size_t>(i)]);
        action->setCheckable(true);
        action->setData(i);
        tools->addAction(action);
        if (i == 0) action->setChecked(true);
    }
    connect(tools, &QActionGroup::triggered, this, [this](QAction* action) {
        setTool(static_cast<Tool>(action->data().toInt()));
    });
    toolbar_->addSeparator();
    auto* snap = toolbar_->addAction("Snap");
    snap->setCheckable(true);
    snap->setChecked(true);
    connect(snap, &QAction::toggled, this, [this](bool enabled) { grid_snap_ = enabled; });
    toolbar_->hide();
}

void LevelPreviewWidget::showProjectSummary(const nsmb::project::Project& project, int level_count, int available_level_count) {
    clearGraphics();
    scene_.reset();
    if (project.directory().empty()) {
        message_ = "Open a project to list and preview levels.";
    } else if (level_count == 0) {
        message_ = "No level entries are available for this project.";
    } else {
        message_ = QString("Select a level. %1 catalog areas, %2 available in project files.")
                       .arg(level_count)
                       .arg(available_level_count);
    }
    update();
}

void LevelPreviewWidget::showLevel(const QString& level_name,
                                   const QString& world,
                                   const QString& area_name,
                                   const QString& course_path,
                                   const QString& bgdat_path,
                                   const QString& source_token,
                                   nsmb::project::EditableLevel* level) {
    scene_.reset();
    clearGraphics();

    if (course_path.isEmpty() || bgdat_path.isEmpty() || !level) {
        message_ = "Level files are not available in this project yet.\nCourse: " + (course_path.isEmpty() ? QString("missing") : course_path) +
                   "\nBGDat: " + (bgdat_path.isEmpty() ? QString("missing") : bgdat_path);
        update();
        return;
    }

    scene_ = Scene{
        level_name,
        world,
        area_name,
        course_path,
        bgdat_path,
        source_token,
        level,
        0,
        0,
        0,
    };
    selections_.clear();
    zoom_ = 1.0;
    pan_ = {};
    message_.clear();
    update();
}

void LevelPreviewWidget::paintEvent(QPaintEvent*) {
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing, true);
    painter.fillRect(rect(), QColor(242, 245, 248));

    const QRectF outer = QRectF(rect()).adjusted(18, editable_ ? 52 : 18, -18, -18);
    if (!scene_) {
        painter.setPen(QColor(63, 72, 86));
        painter.drawText(outer, Qt::AlignCenter | Qt::TextWordWrap, message_);
        return;
    }

    if (editable_) {
        drawMap(painter,
                outer,
                *scene_,
                selections_,
                (dragging_ || resizing_) ? drag_delta_ : QPointF{},
                zoom_,
                pan_,
                tilesets_,
                background_,
                foreground_,
                object_images_,
                actor_images_,
                actor_labels_,
                resizing_);
        if (marquee_selecting_) {
            painter.setPen(QPen(QColor(45, 125, 210), 1, Qt::DashLine));
            painter.setBrush(QColor(45, 125, 210, 35));
            painter.drawRect(marquee_.normalized());
        }
        return;
    }

    auto font = painter.font();
    font.setPointSize(font.pointSize() + 4);
    font.setBold(true);
    painter.setFont(font);
    const QFontMetrics title_metrics(font);
    painter.setPen(QColor(32, 38, 49));
    painter.drawText(outer.adjusted(0, 0, 0, -outer.height() + 32),
                     Qt::AlignLeft | Qt::AlignVCenter,
                     elided(title_metrics, scene_->title, static_cast<int>(outer.width())));

    font.setPointSize(std::max(8, font.pointSize() - 4));
    font.setBold(false);
    painter.setFont(font);
    const QFontMetrics body_metrics(font);
    painter.setPen(QColor(72, 83, 99));

    const QString location = scene_->area_name == scene_->title ? scene_->world : scene_->world + " / " + scene_->area_name;
    painter.drawText(outer.adjusted(0, 34, 0, -outer.height() + 56),
                     Qt::AlignLeft | Qt::AlignVCenter,
                     elided(body_metrics, location + "   " + scene_->source_token, static_cast<int>(outer.width())));
    painter.drawText(outer.adjusted(0, 58, 0, -outer.height() + 78),
                     Qt::AlignLeft | Qt::AlignVCenter,
                     elided(body_metrics, countsText(*scene_), static_cast<int>(outer.width())));
    painter.drawText(outer.adjusted(0, 80, 0, -outer.height() + 100),
                     Qt::AlignLeft | Qt::AlignVCenter,
                     elided(body_metrics, formatIdsText(scene_->level->course()), static_cast<int>(outer.width())));

    const QRectF map_rect(outer.left(), outer.top() + 112.0, outer.width(), outer.height() - 112.0);
    if (map_rect.height() > 20.0) {
        drawMap(painter,
                map_rect,
                *scene_,
                selections_,
                (dragging_ || resizing_) ? drag_delta_ : QPointF{},
                zoom_,
                pan_,
                tilesets_,
                background_,
                foreground_,
                object_images_,
                actor_images_,
                actor_labels_,
                resizing_);
    }
    if (marquee_selecting_) {
        painter.setPen(QPen(QColor(45, 125, 210), 1, Qt::DashLine));
        painter.setBrush(QColor(45, 125, 210, 35));
        painter.drawRect(marquee_.normalized());
    }
}

void LevelPreviewWidget::setSelection(int kind, int item_index, int node_index) {
    if (!scene_) {
        return;
    }
    scene_->selected_kind = kind;
    scene_->selected_item = item_index;
    scene_->selected_node = node_index;
    selections_ = {{kind, item_index, node_index}};
    update();
}

void LevelPreviewWidget::setEditable(bool editable) {
    editable_ = editable;
    toolbar_->setVisible(editable);
    if (!editable) {
        selections_.clear();
    }
    update();
}

void LevelPreviewWidget::setTool(Tool tool) {
    tool_ = tool;
    for (auto* action : toolbar_->actions()) {
        if (action->isCheckable() && action->data().isValid()) {
            action->setChecked(action->data().toInt() == static_cast<int>(tool));
        }
    }
    if (tool != Tool::Select && tool != Tool::Pan) {
        const auto removed = std::erase_if(selections_, [this](const auto& selection) { return !selectionMatchesTool(selection); });
        if (removed != 0) publishSelection();
    }
    setCursor(tool == Tool::Pan ? Qt::OpenHandCursor : Qt::ArrowCursor);
    update();
}

LevelPreviewWidget::Tool LevelPreviewWidget::tool() const noexcept { return tool_; }
double LevelPreviewWidget::zoom() const noexcept { return zoom_; }
void LevelPreviewWidget::setZoom(double zoom) {
    zoom_ = std::clamp(zoom, 0.25, 16.0);
    update();
}
QPointF LevelPreviewWidget::pan() const noexcept { return pan_; }
void LevelPreviewWidget::setPan(QPointF pan) {
    pan_ = pan;
    update();
}

void LevelPreviewWidget::resizeEvent(QResizeEvent* event) {
    toolbar_->setGeometry(0, 0, event->size().width(), 36);
    QWidget::resizeEvent(event);
}

QRectF LevelPreviewWidget::mapRect() const {
    const QRectF outer = QRectF(rect()).adjusted(18, editable_ ? 52 : 18, -18, -18);
    if (editable_) return outer;
    return QRectF(outer.left(), outer.top() + 112.0, outer.width(), outer.height() - 112.0);
}

QTransform LevelPreviewWidget::sceneTransform() const {
    return scene_ ? mapTransform(mapRect(), *scene_, zoom_, pan_) : QTransform{};
}

QPointF LevelPreviewWidget::toScene(const QPointF& point) const {
    bool invertible = false;
    const auto inverse = sceneTransform().inverted(&invertible);
    return invertible ? inverse.map(point) : QPointF{};
}

std::optional<LevelPreviewWidget::Selection> LevelPreviewWidget::hitTest(const QPointF& scene_point) const {
    if (!scene_) {
        return std::nullopt;
    }
    const auto entities = entityRects(*scene_);
    for (auto entity = entities.rbegin(); entity != entities.rend(); ++entity) {
        if (selectionMatchesTool(entity->selection) && entity->rect.adjusted(-3, -3, 3, 3).contains(scene_point)) {
            return entity->selection;
        }
    }
    return std::nullopt;
}

bool LevelPreviewWidget::selectionMatchesTool(const Selection& selection) const {
    if (tool_ == Tool::Select) return true;
    const auto kind = tool_ == Tool::Object         ? 1
                      : tool_ == Tool::Sprite       ? 2
                      : tool_ == Tool::Entrance     ? 3
                      : tool_ == Tool::Zone         ? 4
                      : tool_ == Tool::Path         ? 5
                      : tool_ == Tool::ProgressPath ? 6
                                                   : -1;
    return selection.kind == kind;
}

void LevelPreviewWidget::publishSelection() {
    if (selections_.empty()) {
        emit selectionRequested(0, 0, 0);
        return;
    }
    const auto kind = selections_.front().kind;
    if (!std::ranges::all_of(selections_, [kind](const auto& selection) { return selection.kind == kind; })) {
        return;
    }
    QList<int> items;
    for (const auto& selection : selections_) {
        if (!items.contains(selection.item)) items.push_back(selection.item);
    }
    if (items.size() == 1) {
        emit selectionRequested(kind, items.front(), selections_.front().node);
    } else {
        emit multiSelectionRequested(kind, items);
    }
}

void LevelPreviewWidget::mousePressEvent(QMouseEvent* event) {
    if (!editable_ || !scene_ || !mapRect().contains(event->position())) {
        QWidget::mousePressEvent(event);
        return;
    }
    setFocus();
    press_position_ = event->position();
    last_position_ = event->position();
    drag_delta_ = {};
    if (event->button() == Qt::MiddleButton || tool_ == Tool::Pan || space_pan_) {
        panning_ = true;
        setCursor(Qt::ClosedHandCursor);
        return;
    }
    if (event->button() == Qt::RightButton) {
        if (placement_selected_ && placement_tool_ == tool_) placeEntity(toScene(event->position()));
        return;
    }
    if (event->button() != Qt::LeftButton) {
        return;
    }

    if (selections_.size() == 1U && selectionMatchesTool(selections_.front()) &&
        (selections_.front().kind == 1 || selections_.front().kind == 4)) {
        const auto entities = entityRects(*scene_);
        const auto selected_entity = std::find_if(entities.begin(), entities.end(), [this](const auto& entity) {
            return entity.selection == selections_.front();
        });
        if (selected_entity != entities.end()) {
            const auto handle = sceneTransform().map(selected_entity->rect.bottomRight());
            if (QRectF(handle - QPointF(9, 9), QSizeF(18, 18)).contains(event->position())) {
                resizing_ = true;
                setCursor(Qt::SizeFDiagCursor);
                return;
            }
        }
    }

    const auto hit = hitTest(toScene(event->position()));
    if (hit) {
        const auto existing = std::find(selections_.begin(), selections_.end(), *hit);
        if (event->modifiers().testFlag(Qt::ShiftModifier)) {
            if (existing == selections_.end()) {
                selections_.push_back(*hit);
            } else {
                selections_.erase(existing);
            }
        } else if (existing == selections_.end()) {
            selections_ = {*hit};
        }
        dragging_ = !selections_.empty();
        publishSelection();
    } else {
        if (!event->modifiers().testFlag(Qt::ShiftModifier)) selections_.clear();
        marquee_selecting_ = true;
        marquee_ = QRectF(event->position(), QSizeF{});
        publishSelection();
    }
    update();
}

void LevelPreviewWidget::mouseMoveEvent(QMouseEvent* event) {
    if (panning_) {
        pan_ += event->position() - last_position_;
        last_position_ = event->position();
        update();
        return;
    }
    if (dragging_ || resizing_) {
        drag_delta_ = toScene(event->position()) - toScene(press_position_);
        if (grid_snap_ && !event->modifiers().testFlag(Qt::AltModifier)) {
            drag_delta_.setX(std::round(drag_delta_.x() / 16.0) * 16.0);
            drag_delta_.setY(std::round(drag_delta_.y() / 16.0) * 16.0);
        }
        update();
        return;
    }
    if (marquee_selecting_) {
        marquee_.setBottomRight(event->position());
        update();
        return;
    }
    if (selections_.size() == 1U && selectionMatchesTool(selections_.front()) &&
        (selections_.front().kind == 1 || selections_.front().kind == 4)) {
        const auto entities = entityRects(*scene_);
        const auto selected_entity = std::find_if(entities.begin(), entities.end(), [this](const auto& entity) {
            return entity.selection == selections_.front();
        });
        if (selected_entity != entities.end()) {
            const auto handle = sceneTransform().map(selected_entity->rect.bottomRight());
            setCursor(QRectF(handle - QPointF(9, 9), QSizeF(18, 18)).contains(event->position()) ? Qt::SizeFDiagCursor
                                                                                                 : Qt::ArrowCursor);
        }
    }
    QWidget::mouseMoveEvent(event);
}

void LevelPreviewWidget::mouseReleaseEvent(QMouseEvent* event) {
    if (panning_) {
        panning_ = false;
        setCursor(tool_ == Tool::Pan ? Qt::OpenHandCursor : Qt::ArrowCursor);
        return;
    }
    if (dragging_) {
        dragging_ = false;
        commitMove(drag_delta_);
        drag_delta_ = {};
        update();
        return;
    }
    if (resizing_) {
        resizing_ = false;
        commitResize(drag_delta_);
        drag_delta_ = {};
        setCursor(Qt::ArrowCursor);
        update();
        return;
    }
    if (marquee_selecting_) {
        marquee_selecting_ = false;
        const auto screen_rect = marquee_.normalized();
        const auto top_left = toScene(screen_rect.topLeft());
        const auto bottom_right = toScene(screen_rect.bottomRight());
        const QRectF scene_rect(top_left, bottom_right);
        for (const auto& entity : entityRects(*scene_)) {
            if (selectionMatchesTool(entity.selection) && scene_rect.normalized().intersects(entity.rect) &&
                std::find(selections_.begin(), selections_.end(), entity.selection) == selections_.end()) {
                selections_.push_back(entity.selection);
            }
        }
        marquee_ = {};
        publishSelection();
        update();
        return;
    }
    QWidget::mouseReleaseEvent(event);
}

void LevelPreviewWidget::wheelEvent(QWheelEvent* event) {
    if (!editable_ || !scene_) {
        QWidget::wheelEvent(event);
        return;
    }
    const auto before = toScene(event->position());
    setZoom(zoom_ * std::pow(1.0015, event->angleDelta().y()));
    const auto after = sceneTransform().map(before);
    pan_ += event->position() - after;
    update();
    event->accept();
}

void LevelPreviewWidget::keyPressEvent(QKeyEvent* event) {
    if (!editable_) {
        QWidget::keyPressEvent(event);
        return;
    }
    if (event->key() == Qt::Key_Delete || event->key() == Qt::Key_Backspace) {
        deleteSelection();
        return;
    }
    if (event->key() == Qt::Key_Escape) {
        dragging_ = false;
        resizing_ = false;
        marquee_selecting_ = false;
        drag_delta_ = {};
        marquee_ = {};
        update();
        return;
    }
    if (event->key() == Qt::Key_Space && !event->isAutoRepeat()) {
        space_pan_ = true;
        setCursor(Qt::OpenHandCursor);
        event->accept();
        return;
    }
    QWidget::keyPressEvent(event);
}

void LevelPreviewWidget::keyReleaseEvent(QKeyEvent* event) {
    if (event->key() == Qt::Key_Space && !event->isAutoRepeat()) {
        space_pan_ = false;
        if (!panning_) setCursor(tool_ == Tool::Pan ? Qt::OpenHandCursor : Qt::ArrowCursor);
        event->accept();
        return;
    }
    QWidget::keyReleaseEvent(event);
}

void LevelPreviewWidget::placeEntity(const QPointF& point) {
    if (!scene_ || !scene_->level) return;
    auto placement = point;
    if (grid_snap_ && !QApplication::keyboardModifiers().testFlag(Qt::AltModifier)) {
        placement.setX(std::round(placement.x() / 16.0) * 16.0);
        placement.setY(std::round(placement.y() / 16.0) * 16.0);
    }
    const auto pixel_x = std::clamp(qRound(placement.x()), 0, 65535);
    const auto pixel_y = std::clamp(qRound(placement.y()), 0, 65535);
    nsmb::core::Result<std::size_t> result = nsmb::core::make_error(nsmb::core::ErrorCode::Unsupported, "unsupported placement tool");
    int kind = 0;
    switch (tool_) {
    case Tool::Object:
        kind = 1;
        result = scene_->level->add_object({static_cast<std::uint16_t>(placement_object_id_),
                                            static_cast<std::uint8_t>(placement_tileset_slot_),
                                            static_cast<std::int16_t>(pixel_x / 16),
                                            static_cast<std::int16_t>(pixel_y / 16),
                                            1,
                                            1});
        break;
    case Tool::Sprite:
        kind = 2;
        result = scene_->level->add_sprite({static_cast<std::uint16_t>(placement_sprite_type_),
                                            static_cast<std::uint16_t>(pixel_x / 16),
                                            static_cast<std::uint16_t>(pixel_y / 16),
                                            {}});
        break;
    case Tool::Entrance: {
        kind = 3;
        nsmb::formats::NsmbLevelEntrance entrance;
        entrance.x = static_cast<std::uint16_t>(pixel_x);
        entrance.y = static_cast<std::uint16_t>(pixel_y);
        result = scene_->level->add_entrance(entrance);
        break;
    }
    case Tool::Zone:
        kind = 4;
        result = scene_->level->add_zone({static_cast<std::uint16_t>(pixel_x), static_cast<std::uint16_t>(pixel_y), 256, 192, 0});
        break;
    case Tool::Path:
    case Tool::ProgressPath: {
        const bool progress = tool_ == Tool::ProgressPath;
        kind = progress ? 6 : 5;
        nsmb::formats::NsmbLevelPath path;
        path.nodes.push_back({static_cast<std::uint16_t>(pixel_x), static_cast<std::uint16_t>(pixel_y), {}});
        result = scene_->level->add_path(progress, std::move(path));
        break;
    }
    default:
        return;
    }
    if (!result) {
        emit errorOccurred(QString::fromStdString(result.error().message));
        return;
    }
    selections_ = {{kind, static_cast<int>(result.value()), kind >= 5 ? 0 : -1}};
    publishSelection();
    emit levelChanged();
    update();
}

void LevelPreviewWidget::commitMove(const QPointF& delta) {
    if (!scene_ || !scene_->level || (qFuzzyIsNull(delta.x()) && qFuzzyIsNull(delta.y()))) return;
    auto clamp16 = [](int value) { return static_cast<std::uint16_t>(std::clamp(value, 0, 65535)); };
    nsmb::project::LevelEntityUpdates edits;
    for (const auto& selection : selections_) {
        const auto index = static_cast<std::size_t>(selection.item);
        switch (selection.kind) {
        case 1: {
            if (index >= scene_->level->objects().size()) break;
            auto value = scene_->level->objects()[index];
            value.x = static_cast<std::int16_t>(std::clamp(static_cast<int>(value.x) + qRound(delta.x() / 16.0), -32768, 32767));
            value.y = static_cast<std::int16_t>(std::clamp(static_cast<int>(value.y) + qRound(delta.y() / 16.0), -32768, 32767));
            edits.objects.push_back({index, value});
            break;
        }
        case 2: {
            if (index >= scene_->level->course().sprites.size()) break;
            auto value = scene_->level->course().sprites[index];
            value.x = clamp16(static_cast<int>(value.x) + qRound(delta.x() / 16.0));
            value.y = clamp16(static_cast<int>(value.y) + qRound(delta.y() / 16.0));
            edits.sprites.push_back({index, value});
            break;
        }
        case 3: {
            if (index >= scene_->level->course().entrances.size()) break;
            auto value = scene_->level->course().entrances[index];
            value.x = clamp16(static_cast<int>(value.x) + qRound(delta.x()));
            value.y = clamp16(static_cast<int>(value.y) + qRound(delta.y()));
            edits.entrances.push_back({index, value});
            break;
        }
        case 4: {
            if (index >= scene_->level->course().zones.size()) break;
            auto value = scene_->level->course().zones[index];
            value.x = clamp16(static_cast<int>(value.x) + qRound(delta.x()));
            value.y = clamp16(static_cast<int>(value.y) + qRound(delta.y()));
            edits.zones.push_back({index, value});
            break;
        }
        case 5:
        case 6: {
            const bool progress = selection.kind == 6;
            const auto& paths = progress ? scene_->level->course().progress_paths : scene_->level->course().paths;
            if (index >= paths.size()) break;
            auto value = paths[index];
            if (selection.node >= 0 && static_cast<std::size_t>(selection.node) < value.nodes.size()) {
                auto& node = value.nodes[static_cast<std::size_t>(selection.node)];
                node.x = clamp16(static_cast<int>(node.x) + qRound(delta.x()));
                node.y = clamp16(static_cast<int>(node.y) + qRound(delta.y()));
            }
            (progress ? edits.progress_paths : edits.paths).push_back({index, std::move(value)});
            break;
        }
        }
    }
    auto result = scene_->level->update_entities(std::move(edits), "Move selection");
    if (!result) {
        emit errorOccurred(QString::fromStdString(result.error().message));
        return;
    }
    emit levelChanged();
    publishSelection();
}

void LevelPreviewWidget::commitResize(const QPointF& delta) {
    if (!scene_ || !scene_->level || selections_.size() != 1U) return;
    const auto selection = selections_.front();
    const auto index = static_cast<std::size_t>(selection.item);
    nsmb::project::LevelEntityUpdates edits;
    if (selection.kind == 1 && index < scene_->level->objects().size()) {
        auto object = scene_->level->objects()[index];
        object.width = static_cast<std::uint16_t>(std::clamp(static_cast<int>(object.width) + qRound(delta.x() / 16.0), 1, 65535));
        object.height = static_cast<std::uint16_t>(std::clamp(static_cast<int>(object.height) + qRound(delta.y() / 16.0), 1, 65535));
        edits.objects.push_back({index, object});
    } else if (selection.kind == 4 && index < scene_->level->course().zones.size()) {
        auto zone = scene_->level->course().zones[index];
        zone.width = static_cast<std::uint16_t>(std::clamp(static_cast<int>(zone.width) + qRound(delta.x()), 1, 65535));
        zone.height = static_cast<std::uint16_t>(std::clamp(static_cast<int>(zone.height) + qRound(delta.y()), 1, 65535));
        edits.zones.push_back({index, zone});
    } else {
        return;
    }
    auto result = scene_->level->update_entities(std::move(edits), "Resize selection");
    if (!result) emit errorOccurred(QString::fromStdString(result.error().message));
    else emit levelChanged();
}

void LevelPreviewWidget::deleteSelection() {
    if (!scene_ || !scene_->level || selections_.empty()) return;
    nsmb::project::LevelEntitySelection selection;
    for (const auto& item : selections_) {
        auto* target = item.kind == 1   ? &selection.objects
                       : item.kind == 2 ? &selection.sprites
                       : item.kind == 3 ? &selection.entrances
                       : item.kind == 4 ? &selection.zones
                       : item.kind == 5 ? &selection.paths
                                        : &selection.progress_paths;
        target->push_back(static_cast<std::size_t>(item.item));
    }
    auto result = scene_->level->remove_entities(std::move(selection));
    if (!result) {
        emit errorOccurred(QString::fromStdString(result.error().message));
        return;
    }
    selections_.clear();
    emit levelChanged();
    publishSelection();
    update();
}

void LevelPreviewWidget::setGraphicsResources(nsmb::project::LevelGraphicsResources resources,
                                              nsmb::project::LevelTilesetReferences references) {
    const auto generation = ++graphics_generation_;
    graphics_references_ = references;
    actor_images_.clear();
    QPointer<LevelPreviewWidget> weak(this);
    QThreadPool::globalInstance()->start([weak, generation, resources = std::move(resources)]() mutable {
        std::array<std::shared_ptr<nsmb::project::DecodedTileset>, 3> tilesets;
        QImage background;
        QImage foreground;
        std::unordered_map<std::uint16_t, QString> actor_labels;
        QStringList warnings;
        for (std::size_t slot = 0; slot < resources.tilesets.size(); ++slot) {
            auto decoded = nsmb::project::load_tileset_graphics(resources.tilesets[slot]);
            if (decoded) {
                tilesets[slot] = std::make_shared<nsmb::project::DecodedTileset>(std::move(decoded.value()));
            } else {
                warnings.push_back(QString("Tileset slot %1: %2").arg(slot).arg(QString::fromStdString(decoded.error().message)));
            }
        }
        auto convert = [](const nsmb::core::Result<nsmb::formats::RgbaImage>& result, QImage& target, QString name, QStringList& errors) {
            if (!result) {
                errors.push_back(name + ": " + QString::fromStdString(result.error().message));
                return;
            }
            const auto& image = result.value();
            target = QImage(image.pixels.data(),
                            static_cast<int>(image.width),
                            static_cast<int>(image.height),
                            static_cast<int>(image.width * 4U),
                            QImage::Format_RGBA8888)
                         .copy();
        };
        convert(nsmb::project::load_background_graphics(resources.background), background, "Background", warnings);
        convert(nsmb::project::load_background_graphics(resources.foreground), foreground, "Foreground", warnings);
        for (const auto& actor : resources.actors) {
            actor_labels.emplace(actor.type,
                                 QString::fromUtf8(actor.name.data(), static_cast<qsizetype>(actor.name.size())) +
                                     (actor.model.empty() ? QString{} : " (3D)"));
        }
        if (!weak) return;
        QMetaObject::invokeMethod(
            weak,
            [weak, generation, tilesets = std::move(tilesets), background = std::move(background), foreground = std::move(foreground),
             actor_labels = std::move(actor_labels), warnings = warnings.join("\n")]() mutable {
                if (!weak || weak->graphics_generation_ != generation) return;
                bool tileset_changed = false;
                for (std::size_t slot = 0; slot < tilesets.size(); ++slot) {
                    if (!tilesets[slot]) continue;
                    weak->tilesets_[slot] = std::move(tilesets[slot]);
                    tileset_changed = true;
                }
                if (!background.isNull()) weak->background_ = std::move(background);
                if (!foreground.isNull()) weak->foreground_ = std::move(foreground);
                weak->actor_labels_ = std::move(actor_labels);
                if (tileset_changed) weak->object_images_.clear();
                weak->update();
                if (!warnings.isEmpty()) emit weak->graphicsWarning(warnings);
                emit weak->graphicsLoaded();
            },
            Qt::QueuedConnection);
    });
}

void LevelPreviewWidget::clearGraphics() {
    ++graphics_generation_;
    tilesets_ = {};
    background_ = {};
    foreground_ = {};
    object_images_.clear();
    actor_images_.clear();
    actor_labels_.clear();
    graphics_references_.reset();
}

void LevelPreviewWidget::invalidateGraphics() { graphics_references_.reset(); }

bool LevelPreviewWidget::graphicsMatch(nsmb::project::LevelTilesetReferences references) const noexcept {
    return graphics_references_ && graphics_references_->tileset_id == references.tileset_id &&
           graphics_references_->background_graphics_id == references.background_graphics_id &&
           graphics_references_->background_screen_id == references.background_screen_id &&
           graphics_references_->foreground_graphics_id == references.foreground_graphics_id;
}

nsmb::project::EditableLevel* LevelPreviewWidget::level() const noexcept {
    return scene_ ? scene_->level : nullptr;
}

void LevelPreviewWidget::refreshLevelState() {
    if (!scene_ || !scene_->level) return;
    const auto& course = scene_->level->course();
    const auto removed = std::erase_if(selections_, [&](const Selection& selection) {
        const auto item = static_cast<std::size_t>(std::max(0, selection.item));
        if (selection.item < 0) return true;
        if (selection.kind == 1) return item >= scene_->level->objects().size();
        if (selection.kind == 2) return item >= course.sprites.size();
        if (selection.kind == 3) return item >= course.entrances.size();
        if (selection.kind == 4) return item >= course.zones.size();
        const auto* paths = selection.kind == 5   ? &course.paths
                            : selection.kind == 6 ? &course.progress_paths
                                                  : nullptr;
        return !paths || item >= paths->size() || selection.node < 0 ||
               static_cast<std::size_t>(selection.node) >= (*paths)[item].nodes.size();
    });
    if (removed != 0) publishSelection();
    update();
}

std::vector<LevelPreviewWidget::PaletteEntry> LevelPreviewWidget::paletteEntries() {
    std::vector<PaletteEntry> entries;
    for (std::size_t slot = 0; slot < tilesets_.size(); ++slot) {
        if (!tilesets_[slot]) continue;
        entries.reserve(entries.size() + tilesets_[slot]->objects.size());
        for (std::size_t object = 0; object < tilesets_[slot]->objects.size(); ++object) {
            QImage image;
            auto rendered = nsmb::project::render_tileset_object(*tilesets_[slot], static_cast<std::uint16_t>(object), 1, 1);
            if (rendered) {
                const auto& value = rendered.value();
                image = QImage(value.pixels.data(),
                               static_cast<int>(value.width),
                               static_cast<int>(value.height),
                               static_cast<int>(value.width * 4U),
                               QImage::Format_RGBA8888)
                            .copy();
            }
            entries.push_back({Tool::Object,
                               static_cast<int>(object),
                               static_cast<int>(slot),
                               QString("Object %1:%2").arg(slot).arg(object),
                               std::move(image)});
        }
    }
    for (const auto& [type, label] : actor_labels_) {
        const auto image = actor_images_.contains(type) ? actor_images_.at(type) : QImage{};
        entries.push_back({Tool::Sprite, type, 0, label + QString(" (%1)").arg(type), image});
    }
    std::ranges::sort(entries, [](const auto& left, const auto& right) {
        if (left.tool != right.tool) return left.tool < right.tool;
        if (left.slot != right.slot) return left.slot < right.slot;
        return left.id < right.id;
    });
    return entries;
}

void LevelPreviewWidget::setPlacementObject(int slot, int object_id) {
    placement_tileset_slot_ = std::clamp(slot, 0, 2);
    placement_object_id_ = std::clamp(object_id, 0, 4095);
    setTool(Tool::Object);
    placement_tool_ = Tool::Object;
    placement_selected_ = true;
}

void LevelPreviewWidget::setPlacementSprite(int sprite_type) {
    placement_sprite_type_ = std::clamp(sprite_type, 0, 65535);
    setTool(Tool::Sprite);
    placement_tool_ = Tool::Sprite;
    placement_selected_ = true;
}

void LevelPreviewWidget::setPlacementTool(Tool tool) {
    setTool(tool);
    placement_tool_ = tool;
    placement_selected_ = tool != Tool::Select && tool != Tool::Pan;
}

void LevelPreviewWidget::clearPlacement() { placement_selected_ = false; }
