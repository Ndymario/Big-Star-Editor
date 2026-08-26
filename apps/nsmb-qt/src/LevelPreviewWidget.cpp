#include "LevelPreviewWidget.hpp"

#include <QFontMetrics>
#include <QPainter>
#include <QPaintEvent>

#include <algorithm>
#include <cmath>
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

void drawMap(QPainter& painter, const QRectF& target, const LevelPreviewWidget::Scene& scene) {
    const auto& course = scene.level->course();
    painter.save();
    painter.setPen(QPen(QColor(190, 198, 208)));
    painter.setBrush(QColor(248, 250, 252));
    painter.drawRect(target);

    const auto bounds = sceneBounds(scene);
    const auto scale = std::min(target.width() / bounds.width(), target.height() / bounds.height());
    const auto left = target.left() + (target.width() - bounds.width() * scale) / 2.0;
    const auto top = target.top() + (target.height() - bounds.height() * scale) / 2.0;

    painter.setClipRect(target);
    painter.translate(left, top);
    painter.scale(scale, scale);
    painter.translate(-bounds.left(), -bounds.top());

    painter.setRenderHint(QPainter::Antialiasing, false);
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
        painter.setPen(QPen(QColor(84, 154, 110), 0));
        painter.setBrush(QColor(84, 154, 110, 34));
        painter.drawRect(zoneRect(zone));
        if (scene.selected_kind == 4 && scene.selected_item == static_cast<int>(i)) {
            painter.setPen(QPen(QColor(24, 111, 58), 3.0 / scale));
            painter.setBrush(Qt::NoBrush);
            painter.drawRect(zoneRect(zone));
        }
    }
    for (const auto& view : course.views) {
        painter.setPen(QPen(QColor(65, 122, 180), 0));
        painter.setBrush(QColor(65, 122, 180, 25));
        painter.drawRect(viewRect(view));
    }
    for (std::size_t i = 0; i < scene.level->objects().size(); ++i) {
        const auto& object = scene.level->objects()[i];
        painter.setPen(QPen(QColor(38, 46, 58), 0));
        painter.setBrush(objectColor(object.tileset_slot));
        painter.drawRoundedRect(objectRect(object), 2.0 / scale, 2.0 / scale);
        if (scene.selected_kind == 1 && scene.selected_item == static_cast<int>(i)) {
            painter.setPen(QPen(Qt::white, 3.0 / scale));
            painter.setBrush(Qt::NoBrush);
            painter.drawRect(objectRect(object));
        }
    }
    for (std::size_t i = 0; i < course.sprites.size(); ++i) {
        const auto& sprite = course.sprites[i];
        const auto rect = spriteRect(sprite);
        const QPointF center = rect.center();
        QPolygonF diamond;
        diamond << QPointF(center.x(), rect.top()) << QPointF(rect.right(), center.y()) << QPointF(center.x(), rect.bottom())
                << QPointF(rect.left(), center.y());
        painter.setPen(QPen(QColor(135, 78, 29), 0));
        painter.setBrush(QColor(238, 178, 75, 190));
        painter.drawPolygon(diamond);
        if (scene.selected_kind == 2 && scene.selected_item == static_cast<int>(i)) {
            painter.setPen(QPen(Qt::white, 3.0 / scale));
            painter.setBrush(Qt::NoBrush);
            painter.drawRect(rect.adjusted(-3.0, -3.0, 3.0, 3.0));
        }
    }
    for (std::size_t i = 0; i < course.entrances.size(); ++i) {
        const auto& entrance = course.entrances[i];
        painter.setPen(QPen(QColor(34, 94, 122), 0));
        painter.setBrush(QColor(92, 190, 214, 190));
        painter.drawEllipse(entranceRect(entrance));
        if (scene.selected_kind == 3 && scene.selected_item == static_cast<int>(i)) {
            painter.setPen(QPen(Qt::white, 3.0 / scale));
            painter.setBrush(Qt::NoBrush);
            painter.drawEllipse(entranceRect(entrance).adjusted(-3.0, -3.0, 3.0, 3.0));
        }
    }

    auto drawPaths = [&](const auto& paths, bool progress) {
        for (std::size_t path_index = 0; path_index < paths.size(); ++path_index) {
            const auto& path = paths[path_index];
            painter.setPen(QPen(progress ? QColor(155, 92, 180) : QColor(199, 76, 76), 2.0 / scale));
            for (std::size_t node_index = 1; node_index < path.nodes.size(); ++node_index) {
                const auto& before = path.nodes[node_index - 1U];
                const auto& node = path.nodes[node_index];
                painter.drawLine(QPointF(before.x + 8.0, before.y + 8.0), QPointF(node.x + 8.0, node.y + 8.0));
            }
            for (std::size_t node_index = 0; node_index < path.nodes.size(); ++node_index) {
                const auto& node = path.nodes[node_index];
                const QRectF rect(node.x, node.y, 16.0, 16.0);
                painter.setBrush(node_index == 0 ? QColor(79, 178, 98) : QColor(235, 105, 95));
                painter.drawEllipse(rect);
                if (scene.selected_kind == (progress ? 6 : 5) && scene.selected_item == static_cast<int>(path_index) &&
                    scene.selected_node == static_cast<int>(node_index)) {
                    painter.setPen(QPen(Qt::white, 3.0 / scale));
                    painter.setBrush(Qt::NoBrush);
                    painter.drawEllipse(rect.adjusted(-3.0, -3.0, 3.0, 3.0));
                }
            }
        }
    };
    drawPaths(course.paths, false);
    drawPaths(course.progress_paths, true);

    painter.restore();
}

} // namespace

LevelPreviewWidget::LevelPreviewWidget(QWidget* parent) : QWidget(parent) {
    setMinimumSize(640, 420);
    setAutoFillBackground(false);
}

void LevelPreviewWidget::showProjectSummary(const nsmb::project::Project& project, int level_count, int available_level_count) {
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
    message_.clear();
    update();
}

void LevelPreviewWidget::paintEvent(QPaintEvent*) {
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing, true);
    painter.fillRect(rect(), QColor(242, 245, 248));

    const QRectF outer = QRectF(rect()).adjusted(18, 18, -18, -18);
    if (!scene_) {
        painter.setPen(QColor(63, 72, 86));
        painter.drawText(outer, Qt::AlignCenter | Qt::TextWordWrap, message_);
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
        drawMap(painter, map_rect, *scene_);
    }
}

void LevelPreviewWidget::setSelection(int kind, int item_index, int node_index) {
    if (!scene_) {
        return;
    }
    scene_->selected_kind = kind;
    scene_->selected_item = item_index;
    scene_->selected_node = node_index;
    update();
}
