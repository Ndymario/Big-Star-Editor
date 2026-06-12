#include "LevelPreviewWidget.hpp"

#include "nsmb/core/result.hpp"
#include "nsmb/formats/graphics.hpp"

#include <QFontMetrics>
#include <QPainter>
#include <QPaintEvent>

#include <algorithm>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <unordered_map>
#include <utility>

namespace {

namespace fs = std::filesystem;

nsmb::core::Result<std::vector<std::uint8_t>> readBinaryFile(const fs::path& path) {
    std::ifstream in(path, std::ios::binary);
    if (!in) {
        return nsmb::core::make_error(nsmb::core::ErrorCode::IoError, "could not open " + path.string());
    }
    return std::vector<std::uint8_t>(std::istreambuf_iterator<char>(in), {});
}

fs::path absoluteProjectPath(const nsmb::project::Project& project, const QString& path_text) {
    if (path_text.isEmpty()) {
        return {};
    }

    auto path = fs::path(path_text.toStdString());
    if (path.is_absolute() || project.directory().empty()) {
        return path;
    }
    return project.directory() / path;
}

QString elided(const QFontMetrics& metrics, const QString& text, int width) {
    return metrics.elidedText(text, Qt::ElideMiddle, std::max(0, width));
}

QString countsText(const LevelPreviewWidget::Scene& scene) {
    return QString("Objects %1   Sprites %2   Entrances %3   Views %4   Zones %5")
        .arg(static_cast<qulonglong>(scene.objects.size()))
        .arg(static_cast<qulonglong>(scene.course.sprites.size()))
        .arg(static_cast<qulonglong>(scene.course.entrances.size()))
        .arg(static_cast<qulonglong>(scene.course.views.size()))
        .arg(static_cast<qulonglong>(scene.course.zones.size()));
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
    QRectF bounds;
    bool has_bounds = false;
    for (const auto& zone : scene.course.zones) {
        includeRect(bounds, has_bounds, zoneRect(zone));
    }
    for (const auto& view : scene.course.views) {
        includeRect(bounds, has_bounds, viewRect(view));
    }
    for (const auto& object : scene.objects) {
        includeRect(bounds, has_bounds, objectRect(object));
    }
    for (const auto& sprite : scene.course.sprites) {
        includeRect(bounds, has_bounds, spriteRect(sprite));
    }
    for (const auto& entrance : scene.course.entrances) {
        includeRect(bounds, has_bounds, entranceRect(entrance));
    }
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
    for (const auto& zone : scene.course.zones) {
        painter.setPen(QPen(QColor(84, 154, 110), 0));
        painter.setBrush(QColor(84, 154, 110, 34));
        painter.drawRect(zoneRect(zone));
    }
    for (const auto& view : scene.course.views) {
        painter.setPen(QPen(QColor(65, 122, 180), 0));
        painter.setBrush(QColor(65, 122, 180, 25));
        painter.drawRect(viewRect(view));
    }
    for (const auto& object : scene.objects) {
        painter.setPen(QPen(QColor(38, 46, 58), 0));
        painter.setBrush(objectColor(object.tileset_slot));
        painter.drawRoundedRect(objectRect(object), 2.0 / scale, 2.0 / scale);
    }
    for (const auto& sprite : scene.course.sprites) {
        const auto rect = spriteRect(sprite);
        const QPointF center = rect.center();
        QPolygonF diamond;
        diamond << QPointF(center.x(), rect.top()) << QPointF(rect.right(), center.y()) << QPointF(center.x(), rect.bottom())
                << QPointF(rect.left(), center.y());
        painter.setPen(QPen(QColor(135, 78, 29), 0));
        painter.setBrush(QColor(238, 178, 75, 190));
        painter.drawPolygon(diamond);
    }
    for (const auto& entrance : scene.course.entrances) {
        painter.setPen(QPen(QColor(34, 94, 122), 0));
        painter.setBrush(QColor(92, 190, 214, 190));
        painter.drawEllipse(entranceRect(entrance));
    }

    painter.restore();
}

std::string paletteNameForCharacterName(std::string name) {
    const auto ncg = name.find("_ncg");
    if (ncg != std::string::npos) {
        name.replace(ncg, 4, "_ncl");
        return name;
    }
    const auto ncgr = name.find(".ncgr");
    if (ncgr != std::string::npos) {
        name.replace(ncgr, 5, ".nclr");
        return name;
    }
    return {};
}

QImage renderTileSheet(std::span<const nsmb::formats::IndexedTile4Bpp> tiles, std::span<const nsmb::formats::Bgr555Color> palette) {
    constexpr int tile_size = 8;
    constexpr int columns = 16;
    const auto tile_count = static_cast<int>(std::min<std::size_t>(tiles.size(), 128U));
    if (tile_count == 0 || palette.empty()) {
        return {};
    }

    const auto rows = (tile_count + columns - 1) / columns;
    QImage image(columns * tile_size, rows * tile_size, QImage::Format_ARGB32);
    image.fill(QColor(224, 229, 236));

    for (int tile_index = 0; tile_index < tile_count; ++tile_index) {
        const auto tile_x = (tile_index % columns) * tile_size;
        const auto tile_y = (tile_index / columns) * tile_size;
        const auto& tile = tiles[static_cast<std::size_t>(tile_index)];
        for (int y = 0; y < tile_size; ++y) {
            for (int x = 0; x < tile_size; ++x) {
                const auto palette_index = tile.pixels[static_cast<std::size_t>(y * tile_size + x)];
                if (palette_index >= palette.size()) {
                    continue;
                }
                const auto& color = palette[palette_index];
                image.setPixelColor(tile_x + x,
                                    tile_y + y,
                                    QColor(color.red, color.green, color.blue, color.transparent ? 0 : 255));
            }
        }
    }
    return image;
}

std::optional<LevelPreviewWidget::GraphicsSample> findGraphicsSample(const nsmb::project::Project& project) {
    std::unordered_map<std::string, const nsmb::project::GameFile*> files_by_name;
    std::vector<const nsmb::project::GameFile*> character_files;
    std::vector<const nsmb::project::GameFile*> palette_files;

    for (const auto& file : project.game_files()) {
        files_by_name.emplace(file.path.filename().string(), &file);
        const auto hint = file.path.generic_string();
        const auto kind = nsmb::formats::identify_graphics_format({}, hint);
        if (kind == nsmb::formats::GraphicsFormatKind::RawCharacterGraphicsCandidate) {
            character_files.push_back(&file);
        } else if (kind == nsmb::formats::GraphicsFormatKind::RawPaletteCandidate) {
            palette_files.push_back(&file);
        }
    }

    std::sort(character_files.begin(), character_files.end(), [](const auto* lhs, const auto* rhs) {
        return lhs->path.generic_string() < rhs->path.generic_string();
    });
    std::sort(palette_files.begin(), palette_files.end(), [](const auto* lhs, const auto* rhs) {
        return lhs->path.generic_string() < rhs->path.generic_string();
    });

    auto decode = [&](const nsmb::project::GameFile& character_file,
                      const nsmb::project::GameFile& palette_file) -> std::optional<LevelPreviewWidget::GraphicsSample> {
        auto character_data = readBinaryFile(absoluteProjectPath(project, QString::fromStdString(character_file.path.generic_string())));
        auto palette_data = readBinaryFile(absoluteProjectPath(project, QString::fromStdString(palette_file.path.generic_string())));
        if (!character_data || !palette_data) {
            return std::nullopt;
        }
        auto tiles = nsmb::formats::decode_4bpp_tiles(character_data.value());
        auto palette = nsmb::formats::decode_bgr555_palette(palette_data.value());
        if (!tiles || !palette || tiles.value().empty() || palette.value().empty()) {
            return std::nullopt;
        }

        auto image = renderTileSheet(tiles.value(), palette.value());
        if (image.isNull()) {
            return std::nullopt;
        }

        return LevelPreviewWidget::GraphicsSample{
            QString::fromStdString(character_file.path.generic_string()),
            QString::fromStdString(palette_file.path.generic_string()),
            image,
        };
    };

    for (const auto* character_file : character_files) {
        const auto palette_name = paletteNameForCharacterName(character_file->path.filename().string());
        if (palette_name.empty()) {
            continue;
        }
        if (const auto found = files_by_name.find(palette_name); found != files_by_name.end()) {
            if (auto sample = decode(*character_file, *found->second)) {
                return sample;
            }
        }
    }

    if (!character_files.empty() && !palette_files.empty()) {
        return decode(*character_files.front(), *palette_files.front());
    }
    return std::nullopt;
}

void drawGraphicsSample(QPainter& painter, const QRectF& target, const LevelPreviewWidget::GraphicsSample& sample) {
    painter.save();
    painter.setPen(QPen(QColor(190, 198, 208)));
    painter.setBrush(QColor(248, 250, 252));
    painter.drawRect(target);

    auto font = painter.font();
    font.setBold(true);
    painter.setFont(font);
    painter.setPen(QColor(42, 49, 60));
    painter.drawText(target.adjusted(12, 8, -12, -8), Qt::AlignLeft | Qt::AlignTop, "Graphics sample");

    font.setBold(false);
    painter.setFont(font);
    const QFontMetrics metrics(font);
    const auto text_rect = target.adjusted(12, 30, -12, -8);
    painter.setPen(QColor(77, 86, 101));
    painter.drawText(text_rect,
                     Qt::AlignLeft | Qt::AlignTop,
                     elided(metrics, sample.character_path, static_cast<int>(text_rect.width())) + "\n" +
                         elided(metrics, sample.palette_path, static_cast<int>(text_rect.width())));

    const auto image_rect = target.adjusted(target.width() * 0.52, 10, -12, -10);
    painter.setRenderHint(QPainter::SmoothPixmapTransform, false);
    painter.drawImage(image_rect, sample.image);
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

void LevelPreviewWidget::showLevel(const nsmb::project::Project& project,
                                   const QString& level_name,
                                   const QString& world,
                                   const QString& area_name,
                                   const QString& course_path,
                                   const QString& bgdat_path,
                                   const QString& source_token) {
    scene_.reset();

    if (course_path.isEmpty() || bgdat_path.isEmpty()) {
        message_ = "Level files are not available in this project yet.\nCourse: " + (course_path.isEmpty() ? QString("missing") : course_path) +
                   "\nBGDat: " + (bgdat_path.isEmpty() ? QString("missing") : bgdat_path);
        update();
        return;
    }

    auto course_data = readBinaryFile(absoluteProjectPath(project, course_path));
    if (!course_data) {
        message_ = QString::fromStdString(course_data.error().message);
        update();
        return;
    }
    auto bgdat_data = readBinaryFile(absoluteProjectPath(project, bgdat_path));
    if (!bgdat_data) {
        message_ = QString::fromStdString(bgdat_data.error().message);
        update();
        return;
    }

    auto course = nsmb::formats::parse_nsmb_level_course(course_data.value());
    if (!course) {
        message_ = QString::fromStdString(course.error().message);
        update();
        return;
    }
    auto objects = nsmb::formats::parse_nsmb_bgdat_objects(bgdat_data.value());
    if (!objects) {
        message_ = QString::fromStdString(objects.error().message);
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
        std::move(course.value()),
        std::move(objects.value()),
        findGraphicsSample(project),
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
                     elided(body_metrics, formatIdsText(scene_->course), static_cast<int>(outer.width())));

    const auto graphics_height = scene_->graphics ? std::min(160.0, outer.height() * 0.28) : 0.0;
    const QRectF map_rect(outer.left(), outer.top() + 112.0, outer.width(), outer.height() - 112.0 - graphics_height - (graphics_height > 0 ? 14.0 : 0.0));
    if (map_rect.height() > 20.0) {
        drawMap(painter, map_rect, *scene_);
    }

    if (scene_->graphics) {
        const QRectF graphics_rect(outer.left(), outer.bottom() - graphics_height, outer.width(), graphics_height);
        drawGraphicsSample(painter, graphics_rect, scene_->graphics.value());
    }
}
