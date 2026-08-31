#include "LevelPreviewWidget.hpp"

#include <QApplication>
#include <QClipboard>
#include <QEvent>
#include <QFontMetrics>
#include <QFileDialog>
#include <QActionGroup>
#include <QKeyEvent>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMouseEvent>
#include <QPainter>
#include <QPaintEvent>
#include <QPalette>
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

#include "nsmb/formats/graphics.hpp"

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
    return QString("Tileset %1   BG graphics %2   BG screen %3   FG graphics %4   FG screen %5")
        .arg(static_cast<int>(course.header.tileset_theme[0]))
        .arg(static_cast<int>(course.header.background_theme[0]))
        .arg(static_cast<int>(course.backgrounds.empty() ? 0 : course.backgrounds[0].screen_id))
        .arg(static_cast<int>(course.header.foreground_theme[0]))
        .arg(static_cast<int>(course.foregrounds.empty() ? 0 : course.foregrounds[0].screen_id));
}

QRectF objectRect(const nsmb::formats::NsmbLevelObject& object) {
    return QRectF(object.x * 16.0,
                  object.y * 16.0,
                  std::max<int>(1, object.width) * 16.0,
                  std::max<int>(1, object.height) * 16.0);
}

using SpriteDefinitions = std::unordered_map<std::uint64_t, nsmb::project::SpriteDefinition>;
using ActorObjectIds = std::unordered_map<std::uint16_t, std::uint32_t>;

std::uint64_t spriteDefinitionKey(std::uint16_t type, std::optional<std::uint32_t> hash) {
    return hash ? (std::uint64_t{1} << 32U) | *hash : type;
}

const nsmb::project::SpriteDefinition* spriteDefinition(const nsmb::formats::NsmbLevelSprite& sprite,
                                                        std::optional<std::uint32_t> hash,
                                                        const SpriteDefinitions& definitions) {
    const auto found = definitions.find(spriteDefinitionKey(sprite.id, hash));
    return found == definitions.end() ? nullptr : &found->second;
}

const nsmb::project::LuaActorDefinition* actorDefinition(const nsmb::formats::NsmbLevelSprite& sprite,
                                                         std::optional<std::uint32_t> hash,
                                                         const nsmb::project::LuaActorCatalog& definitions,
                                                         const ActorObjectIds& object_ids) {
    if (hash) return definitions.by_hash(*hash);
    const auto object = object_ids.find(sprite.id);
    return object == object_ids.end() ? nullptr : definitions.by_object_id(object->second);
}

nsmb::project::LuaActorRenderInput actorInput(const nsmb::formats::NsmbLevelSprite& sprite,
                                              std::optional<std::uint32_t> hash,
                                              std::uint32_t object_id,
                                              const QPointF& offset = {}) {
    nsmb::project::LuaActorRenderInput input;
    input.x = sprite.x;
    input.y = sprite.y;
    input.pixel_x = sprite.x * 16.0 + offset.x();
    input.pixel_y = sprite.y * 16.0 + offset.y();
    input.object_id = object_id;
    input.type = sprite.id;
    input.hash = hash;
    input.settings = sprite.settings;
    input.data = {sprite.events[0], sprite.events[1],
                  static_cast<std::uint8_t>(sprite.settings >> 24U),
                  static_cast<std::uint8_t>(sprite.settings >> 16U),
                  static_cast<std::uint8_t>(sprite.settings >> 8U),
                  static_cast<std::uint8_t>(sprite.settings)};
    return input;
}

QRectF spriteRect(const nsmb::formats::NsmbLevelSprite& sprite,
                  std::optional<std::uint32_t> hash,
                  const SpriteDefinitions& definitions,
                  const nsmb::project::LuaActorCatalog& actor_definitions,
                  const ActorObjectIds& object_ids,
                  bool render = false) {
    const auto* definition = spriteDefinition(sprite, hash, definitions);
    auto bounds = definition ? (render ? definition->render_bounds : definition->grab_bounds)
                             : nsmb::project::SpriteBounds{};
    if (const auto* actor = actorDefinition(sprite, hash, actor_definitions, object_ids)) {
        const auto dynamic = nsmb::project::lua_actor_bounds(
            *actor, actorInput(sprite, hash, actor->sprite.object_id), render);
        // A bounds function returns the box in scene pixels; the cached
        // definition bounds are the same box evaluated at the origin.
        if (dynamic && dynamic.value())
            return QRectF(dynamic.value()->x, dynamic.value()->y, dynamic.value()->width, dynamic.value()->height);
    }
    return QRectF(sprite.x * 16.0 + bounds.x, sprite.y * 16.0 + bounds.y, bounds.width, bounds.height);
}

void paintActorCommand(QPainter& painter, const nsmb::project::LuaActorDrawCommand& command) {
    const QRectF destination(command.x, command.y, command.width, command.height);
    painter.save();
    painter.setOpacity(command.opacity);
    const auto center = destination.center();
    painter.translate(center);
    painter.rotate(command.rotation);
    painter.scale(command.flip_x ? -1.0 : 1.0, command.flip_y ? -1.0 : 1.0);
    painter.translate(-center);
    const auto colorOf = [](std::uint32_t packed) {
        return QColor(static_cast<int>(packed >> 24U & 0xFFU), static_cast<int>(packed >> 16U & 0xFFU),
                      static_cast<int>(packed >> 8U & 0xFFU), static_cast<int>(packed & 0xFFU));
    };
    switch (command.kind) {
    case nsmb::project::LuaActorDrawKind::FillRect:
        painter.fillRect(destination, colorOf(command.color));
        break;
    case nsmb::project::LuaActorDrawKind::StrokeRect:
        painter.setPen(QPen(colorOf(command.color), command.stroke_width));
        painter.setBrush(Qt::NoBrush);
        painter.drawRect(destination);
        break;
    case nsmb::project::LuaActorDrawKind::FillEllipse:
        painter.setPen(Qt::NoPen);
        painter.setBrush(colorOf(command.color));
        painter.drawEllipse(destination);
        break;
    case nsmb::project::LuaActorDrawKind::Line:
        painter.setPen(QPen(colorOf(command.color), command.stroke_width));
        painter.drawLine(QPointF(command.x, command.y), QPointF(command.x2, command.y2));
        break;
    case nsmb::project::LuaActorDrawKind::Text:
    case nsmb::project::LuaActorDrawKind::Image:
        break;
    }
    if (!command.text.empty()) {
        painter.setPen(Qt::white);
        painter.drawText(destination.topLeft() + QPointF(0.0, 8.0), QString::fromStdString(command.text));
    } else if (command.image && !command.image->pixels.empty()) {
        const QImage image(command.image->pixels.data(),
                           static_cast<int>(command.image->width),
                           static_cast<int>(command.image->height),
                           static_cast<int>(command.image->width * 4U),
                           QImage::Format_RGBA8888);
        painter.drawImage(destination, image,
                          QRectF(command.source_x, command.source_y, command.source_width, command.source_height));
    }
    painter.restore();
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

QRectF sceneBounds(const LevelPreviewWidget::Scene& scene,
                   const SpriteDefinitions& definitions,
                   const nsmb::project::LuaActorCatalog& actor_definitions,
                   const ActorObjectIds& object_ids) {
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
    for (std::size_t i = 0; i < course.sprites.size(); ++i) {
        includeRect(bounds, has_bounds, spriteRect(course.sprites[i], scene.level->sprite_hash(i), definitions,
                                                   actor_definitions, object_ids, true));
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

// The canvas paints itself instead of relying on widget styling, so every chrome colour is
// derived from the active palette to keep the editor legible in light and dark themes alike.
struct CanvasColors {
    bool dark{};
    QColor page;
    QColor surface;
    QColor frame;
    QColor coarse_grid;
    QColor fine_grid;
    QColor title_text;
    QColor body_text;
    QColor outline;
    QColor label_text;
    QColor marker;
    QColor highlight;
};

QColor blend(const QColor& from, const QColor& to, double ratio) {
    return QColor::fromRgbF(from.redF() + (to.redF() - from.redF()) * ratio,
                            from.greenF() + (to.greenF() - from.greenF()) * ratio,
                            from.blueF() + (to.blueF() - from.blueF()) * ratio);
}

QColor withAlpha(QColor color, int alpha) {
    color.setAlpha(alpha);
    return color;
}

CanvasColors canvasColors(const QPalette& palette) {
    const auto window = palette.color(QPalette::Window);
    const auto window_text = palette.color(QPalette::WindowText);
    const auto base = palette.color(QPalette::Base);
    const auto text = palette.color(QPalette::Text);
    CanvasColors colors;
    colors.dark = window.lightness() < 128;
    colors.page = window;
    colors.surface = base;
    colors.frame = palette.color(QPalette::Mid);
    colors.coarse_grid = blend(base, text, colors.dark ? 0.24 : 0.12);
    colors.fine_grid = withAlpha(blend(base, text, 0.55), 70);
    colors.title_text = window_text;
    colors.body_text = blend(window, window_text, 0.78);
    colors.outline = blend(base, text, 0.82);
    colors.label_text = text;
    colors.marker = colors.dark ? QColor(246, 248, 251) : QColor(Qt::white);
    colors.highlight = palette.color(QPalette::Highlight);
    return colors;
}

// Semantic entity colours keep their hue; on dark canvases they are lifted so the strokes stay visible.
QColor entityStroke(const QColor& color, const CanvasColors& colors) {
    return colors.dark ? color.lighter(140) : color.darker(160);
}

QTransform mapTransform(const QRectF& target,
                        const LevelPreviewWidget::Scene& scene,
                        const SpriteDefinitions& definitions,
                        const nsmb::project::LuaActorCatalog& actor_definitions,
                        const ActorObjectIds& object_ids,
                        double zoom,
                        const QPointF& pan) {
    const auto bounds = sceneBounds(scene, definitions, actor_definitions, object_ids);
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

// Ordered back to front for picking: `hitTest` walks this list in reverse, so
// whatever is pushed last is grabbed first. Views and zones lead precisely
// because they are the big rectangles everything else sits inside -- were they
// picked first, a click anywhere in a view could never reach the object under
// the cursor, and the enclosing container would be the only thing selectable.
std::vector<EntityRect> entityRects(const LevelPreviewWidget::Scene& scene,
                                    const SpriteDefinitions& definitions,
                                    const nsmb::project::LuaActorCatalog& actor_definitions,
                                    const ActorObjectIds& object_ids) {
    std::vector<EntityRect> entities;
    const auto& course = scene.level->course();
    // Views are the largest, so they yield to everything, zones included.
    for (std::size_t i = 0; i < course.views.size(); ++i) {
        entities.push_back({{7, static_cast<int>(i), -1}, viewRect(course.views[i])});
    }
    for (std::size_t i = 0; i < course.zones.size(); ++i) {
        entities.push_back({{4, static_cast<int>(i), -1}, zoneRect(course.zones[i])});
    }
    for (std::size_t i = 0; i < scene.level->objects().size(); ++i) {
        entities.push_back({{1, static_cast<int>(i), -1}, objectRect(scene.level->objects()[i])});
    }
    for (std::size_t i = 0; i < course.sprites.size(); ++i) {
        entities.push_back({{2, static_cast<int>(i), -1},
                            spriteRect(course.sprites[i], scene.level->sprite_hash(i), definitions,
                                       actor_definitions, object_ids)});
    }
    for (std::size_t i = 0; i < course.entrances.size(); ++i) {
        entities.push_back({{3, static_cast<int>(i), -1}, entranceRect(course.entrances[i])});
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

// Only rectangular entities can be resized: tile objects, zones and views.
bool resizableKind(int kind) { return kind == 1 || kind == 4 || kind == 7; }

// Paths and progress paths are the only kinds selected down to a node.
bool pathKind(int kind) { return kind == 5 || kind == 6; }

// The eight knob centres of a rectangle, in the order Handle enumerates them
// after None: clockwise from the top left, corners alternating with edge
// midpoints.
std::array<QPointF, 8> handleCentres(const QRectF& rect) {
    const auto middle_x = rect.center().x();
    const auto middle_y = rect.center().y();
    return {rect.topLeft(),
            QPointF(middle_x, rect.top()),
            rect.topRight(),
            QPointF(rect.right(), middle_y),
            rect.bottomRight(),
            QPointF(middle_x, rect.bottom()),
            rect.bottomLeft(),
            QPointF(rect.left(), middle_y)};
}

Qt::CursorShape handleCursor(LevelPreviewWidget::Handle handle) {
    using Handle = LevelPreviewWidget::Handle;
    switch (handle) {
    case Handle::TopLeft:
    case Handle::BottomRight: return Qt::SizeFDiagCursor;
    case Handle::TopRight:
    case Handle::BottomLeft: return Qt::SizeBDiagCursor;
    case Handle::Top:
    case Handle::Bottom: return Qt::SizeVerCursor;
    case Handle::Left:
    case Handle::Right: return Qt::SizeHorCursor;
    case Handle::None: break;
    }
    return Qt::ArrowCursor;
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
             const std::unordered_map<std::uint32_t, QImage>& custom_actor_images,
             const std::unordered_map<std::uint16_t, QString>& actor_labels,
             const std::unordered_map<std::uint32_t, QString>& custom_actor_labels,
             const SpriteDefinitions& sprite_definitions,
             const nsmb::project::LuaActorCatalog& actor_definitions,
             const ActorObjectIds& actor_object_ids,
             bool resizing,
             const LevelPreviewWidget::ResizeEdges& resize,
             const CanvasColors& colors) {
    const auto& course = scene.level->course();
    painter.save();
    painter.setPen(QPen(colors.frame));
    painter.setBrush(colors.surface);
    painter.drawRect(target);

    const auto bounds = sceneBounds(scene, sprite_definitions, actor_definitions, actor_object_ids);
    const auto transform = mapTransform(target, scene, sprite_definitions, actor_definitions, actor_object_ids, zoom, pan);
    const auto scale = transform.m11();
    // Knobs are a screen-space affordance, so their size is divided back out of
    // the scene scale to stay grabbable at any zoom.
    const auto drawKnobs = [&](const QRectF& rect) {
        const auto size = 9.0 / scale;
        painter.setPen(QPen(colors.outline, 1.0 / scale));
        painter.setBrush(colors.marker);
        for (const auto& centre : handleCentres(rect)) {
            painter.drawRect(QRectF(centre - QPointF(size / 2.0, size / 2.0), QSizeF(size, size)));
        }
    };
    // A resize adjusts the edges the grabbed knob owns; anything else moves.
    const auto preview = [&](QRectF& rect, int kind, int index) {
        if (!selected(selections, kind, index)) return;
        if (resizing && resizableKind(kind)) rect.adjust(resize.left, resize.top, resize.right, resize.bottom);
        else if (!resizing) rect.translate(drag_delta);
    };

    painter.setClipRect(target);
    painter.setTransform(transform, true);

    painter.setRenderHint(QPainter::Antialiasing, false);
    if (!background.isNull()) {
        painter.setOpacity(0.55);
        painter.drawImage(bounds, background);
        painter.setOpacity(1.0);
    }
    painter.setPen(QPen(colors.coarse_grid, 0));
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
    // Zones and views are containers, so they are drawn in two passes: the tint
    // goes underneath, where it cannot wash out what it holds, and the boundary
    // goes over everything at the end. An outline buried under the objects it
    // encloses is not a boundary you can follow.
    const auto zoneColor = QColor(84, 154, 110);
    const auto viewColor = QColor(65, 122, 180);
    painter.setPen(Qt::NoPen);
    for (std::size_t i = 0; i < course.zones.size(); ++i) {
        auto rect = zoneRect(course.zones[i]);
        preview(rect, 4, static_cast<int>(i));
        painter.setBrush(withAlpha(zoneColor, 34));
        painter.drawRect(rect);
    }
    for (std::size_t i = 0; i < course.views.size(); ++i) {
        auto rect = viewRect(course.views[i]);
        preview(rect, 7, static_cast<int>(i));
        painter.setBrush(withAlpha(viewColor, 25));
        painter.drawRect(rect);
    }
    const auto drawContainerOutlines = [&] {
        painter.setBrush(Qt::NoBrush);
        const auto outline = [&](const QRectF& rect, int kind, int index, const QColor& color, const QColor& accent) {
            if (selected(selections, kind, index)) {
                painter.setPen(QPen(accent, 3.0 / scale));
                painter.drawRect(rect);
                drawKnobs(rect);
                painter.setBrush(Qt::NoBrush);
            } else {
                painter.setPen(QPen(color, 0));
                painter.drawRect(rect);
            }
        };
        for (std::size_t i = 0; i < course.zones.size(); ++i) {
            auto rect = zoneRect(course.zones[i]);
            preview(rect, 4, static_cast<int>(i));
            outline(rect, 4, static_cast<int>(i), zoneColor, QColor(24, 111, 58));
        }
        for (std::size_t i = 0; i < course.views.size(); ++i) {
            auto rect = viewRect(course.views[i]);
            preview(rect, 7, static_cast<int>(i));
            outline(rect, 7, static_cast<int>(i), viewColor, QColor(35, 88, 145));
        }
    };
    for (std::size_t i = 0; i < scene.level->objects().size(); ++i) {
        const auto& object = scene.level->objects()[i];
        auto rect = objectRect(object);
        preview(rect, 1, static_cast<int>(i));
        painter.setPen(QPen(colors.outline, 0));
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
            painter.setPen(QPen(colors.marker, 3.0 / scale));
            painter.setBrush(Qt::NoBrush);
            painter.drawRect(rect);
            drawKnobs(rect);
        }
    }
    for (std::size_t i = 0; i < course.sprites.size(); ++i) {
        const auto& sprite = course.sprites[i];
        const auto custom_hash = scene.level->sprite_hash(i);
        auto rect = spriteRect(sprite, custom_hash, sprite_definitions, actor_definitions, actor_object_ids);
        auto render_rect = spriteRect(sprite, custom_hash, sprite_definitions, actor_definitions, actor_object_ids, true);
        const auto moving = selected(selections, 2, static_cast<int>(i));
        if (moving) rect.translate(drag_delta);
        if (moving) render_rect.translate(drag_delta);

        bool rendered = false;
        if (const auto* actor = actorDefinition(sprite, custom_hash, actor_definitions, actor_object_ids);
            actor && nsmb::project::has_lua_renderer(*actor)) {
            bool custom_rendered = false;
            const auto commands = nsmb::project::render_lua_actor(
                *actor, actorInput(sprite, custom_hash, actor->sprite.object_id, moving ? drag_delta : QPointF{}),
                &custom_rendered);
            if (commands) {
                for (const auto& command : commands.value()) paintActorCommand(painter, command);
                rendered = custom_rendered;
            }
        }

        if (!rendered && !custom_hash) {
            if (const auto image = actor_images.find(sprite.id); image != actor_images.end()) {
                painter.drawImage(render_rect, image->second);
                rendered = true;
            }
        } else if (!rendered) {
            if (const auto image = custom_actor_images.find(*custom_hash); image != custom_actor_images.end()) {
                painter.drawImage(render_rect, image->second);
                rendered = true;
            }
        }

        // An actor with no rendering program of its own gets the stock marker and
        // its number, the way NSMBe draws an actor it has no case for.
        if (!rendered) {
            const auto* definition = spriteDefinition(sprite, custom_hash, sprite_definitions);
            painter.setPen(QPen(entityStroke(colors.label_text, colors), 0));
            painter.setBrush(QColor(110, 110, 110, 190));
            painter.drawRoundedRect(render_rect, 2, 2);
            painter.setPen(QPen(colors.label_text, 0));
            painter.drawText(render_rect, Qt::AlignCenter, QString::number(sprite.id));
            const auto label = definition ? QString::fromStdString(definition->name)
                : custom_hash ? (custom_actor_labels.contains(*custom_hash)
                                     ? custom_actor_labels.at(*custom_hash)
                                     : QString("0x%1 (foreign)").arg(*custom_hash, 8, 16, QLatin1Char('0')))
                : (actor_labels.contains(sprite.id) ? actor_labels.at(sprite.id) : QString("Actor %1").arg(sprite.id));
            painter.drawText(QRectF(render_rect.right() + 2.0, render_rect.top() - 2.0, 180.0, 20.0), label);
        }
        if (selected(selections, 2, static_cast<int>(i))) {
            painter.setPen(QPen(colors.marker, 3.0 / scale));
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
            painter.setPen(QPen(colors.marker, 3.0 / scale));
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
                    painter.setPen(QPen(colors.marker, 3.0 / scale));
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

    // Last, so nothing -- not even the foreground overlay -- dims the edges you
    // size a zone or a view against.
    drawContainerOutlines();

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
    // No tool modes. Everything on the canvas is selectable at all times, and
    // what right-click creates is whatever the properties panel is showing --
    // the same arrangement NSMB Editor uses.
    auto* snap = toolbar_->addAction("Snap");
    snap->setCheckable(true);
    snap->setChecked(true);
    connect(snap, &QAction::toggled, this, [this](bool enabled) { grid_snap_ = enabled; });
    auto* minimap = toolbar_->addAction("Minimap");
    minimap->setCheckable(true); minimap->setChecked(true);
    connect(minimap, &QAction::toggled, this, [this](bool enabled) { show_minimap_ = enabled; update(); });
    auto* grid = toolbar_->addAction("Grid"); grid->setCheckable(true); grid->setChecked(true); connect(grid,&QAction::toggled,this,[this](bool enabled){show_grid_=enabled;update();});
    auto* snap8 = toolbar_->addAction("8 px"); snap8->setCheckable(true); connect(snap8,&QAction::toggled,this,[this](bool enabled){snap_size_=enabled?8:16;});
    auto* screens = toolbar_->addAction("DS Screen"); screens->setCheckable(true); connect(screens,&QAction::toggled,this,[this](bool enabled){show_ds_screen_=enabled;update();});
    toolbar_->addAction("Reference…",this,[this]{const auto path=QFileDialog::getOpenFileName(this,"Reference Background",{},"Images (*.png *.jpg *.bmp)");if(!path.isEmpty()){reference_image_.load(path);reference_offset_={};update();}});
    for(const auto& [name,zoom]:std::array{std::pair{"50%",0.5},std::pair{"100%",1.0},std::pair{"200%",2.0}})toolbar_->addAction(name,this,[this,zoom]{setZoom(zoom);});
    toolbar_->addAction("Fullscreen",this,[this]{window()->isFullScreen()?window()->showNormal():window()->showFullScreen();});
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
    const auto colors = canvasColors(palette());
    painter.fillRect(rect(), colors.page);

    const QRectF outer = QRectF(rect()).adjusted(18, editable_ ? 52 : 18, -18, -18);
    if (!scene_) {
        painter.setPen(colors.body_text);
        painter.drawText(outer, Qt::AlignCenter | Qt::TextWordWrap, message_);
        return;
    }

    if (editable_) {
        drawMap(painter,
                outer,
                *scene_,
                selections_,
                (dragging_ || resize_handle_ != Handle::None) ? drag_delta_ : QPointF{},
                zoom_,
                pan_,
                tilesets_,
                background_,
                foreground_,
                object_images_,
                actor_images_,
                custom_actor_images_,
                actor_labels_,
                custom_actor_labels_,
                sprite_definitions_,
                actor_definition_catalog_,
                actor_object_ids_,
                resize_handle_ != Handle::None,
                resizeEdges(drag_delta_),
                colors);
        if (show_grid_ || show_ds_screen_ || !reference_image_.isNull()) {
            painter.save(); painter.setClipRect(outer); painter.setTransform(sceneTransform(), true);
            if (!reference_image_.isNull()) { painter.setOpacity(0.35); painter.drawImage(QRectF(reference_offset_, reference_image_.size()), reference_image_); painter.setOpacity(1.0); }
            if (show_grid_) { bool ok=false;const auto visible=sceneTransform().inverted(&ok).mapRect(outer);if(ok){painter.setPen(QPen(colors.fine_grid,0));const auto left=std::floor(visible.left()/16)*16,top=std::floor(visible.top()/16)*16;for(double x=left;x<=visible.right();x+=16)painter.drawLine(QPointF(x,visible.top()),QPointF(x,visible.bottom()));for(double y=top;y<=visible.bottom();y+=16)painter.drawLine(QPointF(visible.left(),y),QPointF(visible.right(),y));}}
            if (show_ds_screen_) { painter.setPen(QPen(QColor(220,45,55),0));painter.setBrush(QColor(220,45,55,18));painter.drawRect(QRectF(0,0,256,192));painter.drawRect(QRectF(0,192,256,192)); }
            painter.restore();
        }
        if (marquee_selecting_) {
            painter.setPen(QPen(colors.highlight, 1, Qt::DashLine));
            painter.setBrush(withAlpha(colors.highlight, 35));
            painter.drawRect(marquee_.normalized());
        }
        if (show_minimap_) {
            const QRectF mini(outer.right() - 190.0, outer.bottom() - 130.0, 180.0, 120.0);
            drawMap(painter, mini, *scene_, {}, {}, 1.0, {}, tilesets_, background_, foreground_, object_images_,
                    actor_images_, custom_actor_images_, actor_labels_, custom_actor_labels_, sprite_definitions_,
                    actor_definition_catalog_, actor_object_ids_, false, {}, colors);
            bool invertible = false; const auto visible = mapTransform(outer, *scene_, sprite_definitions_, actor_definition_catalog_, actor_object_ids_, zoom_, pan_).inverted(&invertible).mapRect(outer);
            if (invertible) { painter.setPen(QPen(colors.highlight, 2)); painter.setBrush(withAlpha(colors.highlight, 25)); painter.drawRect(mapTransform(mini, *scene_, sprite_definitions_, actor_definition_catalog_, actor_object_ids_, 1.0, {}).mapRect(visible)); }
            painter.setPen(QPen(colors.outline, 2)); painter.setBrush(Qt::NoBrush); painter.drawRect(mini);
        }
        return;
    }

    auto font = painter.font();
    font.setPointSize(font.pointSize() + 4);
    font.setBold(true);
    painter.setFont(font);
    const QFontMetrics title_metrics(font);
    painter.setPen(colors.title_text);
    painter.drawText(outer.adjusted(0, 0, 0, -outer.height() + 32),
                     Qt::AlignLeft | Qt::AlignVCenter,
                     elided(title_metrics, scene_->title, static_cast<int>(outer.width())));

    font.setPointSize(std::max(8, font.pointSize() - 4));
    font.setBold(false);
    painter.setFont(font);
    const QFontMetrics body_metrics(font);
    painter.setPen(colors.body_text);

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
                (dragging_ || resize_handle_ != Handle::None) ? drag_delta_ : QPointF{},
                zoom_,
                pan_,
                tilesets_,
                background_,
                foreground_,
                object_images_,
                actor_images_,
                custom_actor_images_,
                actor_labels_,
                custom_actor_labels_,
                sprite_definitions_,
                actor_definition_catalog_,
                actor_object_ids_,
                resize_handle_ != Handle::None,
                resizeEdges(drag_delta_),
                colors);
    }
    if (marquee_selecting_) {
        painter.setPen(QPen(colors.highlight, 1, Qt::DashLine));
        painter.setBrush(withAlpha(colors.highlight, 35));
        painter.drawRect(marquee_.normalized());
    }
}

void LevelPreviewWidget::setSelection(int kind, int item_index, int node_index) {
    if (!scene_) {
        return;
    }
    // Only paths carry a node; everything else is addressed as -1 on the canvas.
    // The properties panel's node spin box floors at 0, so it hands back 0 for
    // nodeless kinds, and an unnormalised 0 would never compare equal to the
    // entity's own selection -- which is what the resize knobs are looked up by.
    const auto node = pathKind(kind) ? node_index : -1;
    scene_->selected_kind = kind;
    scene_->selected_item = item_index;
    scene_->selected_node = node;
    selections_ = {{kind, item_index, node}};
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

void LevelPreviewWidget::setCreateKind(int kind) {
    // Deliberately does not touch the selection. Changing what you are about to
    // create is not a reason to drop what you already had hold of.
    create_kind_ = std::clamp(kind, 1, 7);
}

int LevelPreviewWidget::createKind() const noexcept { return create_kind_; }
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

void LevelPreviewWidget::changeEvent(QEvent* event) {
    QWidget::changeEvent(event);
    if (event->type() == QEvent::PaletteChange || event->type() == QEvent::ApplicationPaletteChange ||
        event->type() == QEvent::ThemeChange || event->type() == QEvent::StyleChange) {
        update();
    }
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
    return scene_ ? mapTransform(mapRect(), *scene_, sprite_definitions_, actor_definition_catalog_, actor_object_ids_, zoom_, pan_)
                  : QTransform{};
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
    const auto entities = entityRects(*scene_, sprite_definitions_, actor_definition_catalog_, actor_object_ids_);
    // Front to back, across every layer. Nothing is filtered out.
    for (auto entity = entities.rbegin(); entity != entities.rend(); ++entity) {
        if (entity->rect.adjusted(-3, -3, 3, 3).contains(scene_point)) {
            return entity->selection;
        }
    }
    return std::nullopt;
}

std::optional<QRectF> LevelPreviewWidget::selectedResizableRect() const {
    if (!scene_ || selections_.size() != 1U || !resizableKind(selections_.front().kind)) return std::nullopt;
    const auto entities = entityRects(*scene_, sprite_definitions_, actor_definition_catalog_, actor_object_ids_);
    const auto entity = std::find_if(entities.begin(), entities.end(), [this](const auto& value) {
        return value.selection == selections_.front();
    });
    if (entity == entities.end()) return std::nullopt;
    return entity->rect;
}

LevelPreviewWidget::Handle LevelPreviewWidget::handleAt(const QPointF& widget_point) const {
    const auto rect = selectedResizableRect();
    if (!rect) return Handle::None;
    const auto transform = sceneTransform();
    const auto centres = handleCentres(transform.mapRect(*rect));
    for (std::size_t i = 0; i < centres.size(); ++i) {
        if (QRectF(centres[i] - QPointF(6, 6), QSizeF(12, 12)).contains(widget_point)) {
            return static_cast<Handle>(i + 1);
        }
    }
    return Handle::None;
}

LevelPreviewWidget::ResizeEdges LevelPreviewWidget::resizeEdges(const QPointF& delta) const {
    // Each knob owns the edges it sits on; the rest of the rectangle stays put.
    const auto left = resize_handle_ == Handle::TopLeft || resize_handle_ == Handle::Left ||
                      resize_handle_ == Handle::BottomLeft;
    const auto right = resize_handle_ == Handle::TopRight || resize_handle_ == Handle::Right ||
                       resize_handle_ == Handle::BottomRight;
    const auto top = resize_handle_ == Handle::TopLeft || resize_handle_ == Handle::Top ||
                     resize_handle_ == Handle::TopRight;
    const auto bottom = resize_handle_ == Handle::BottomLeft || resize_handle_ == Handle::Bottom ||
                        resize_handle_ == Handle::BottomRight;
    return {left ? delta.x() : 0.0, top ? delta.y() : 0.0, right ? delta.x() : 0.0, bottom ? delta.y() : 0.0};
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
    if (!reference_image_.isNull() && event->button()==Qt::LeftButton && event->modifiers().testFlag(Qt::AltModifier) && event->modifiers().testFlag(Qt::ShiftModifier)) { dragging_reference_=true; return; }
    if (event->button() == Qt::MiddleButton || space_pan_ || event->modifiers().testFlag(Qt::AltModifier)) {
        panning_ = true;
        setCursor(Qt::ClosedHandCursor);
        return;
    }
    if (event->button() == Qt::RightButton) {
        placeEntity(toScene(event->position()));
        return;
    }
    if (event->button() != Qt::LeftButton) {
        return;
    }

    if (const auto handle = handleAt(event->position()); handle != Handle::None) {
        resize_handle_ = handle;
        setCursor(handleCursor(handle));
        return;
    }

    const auto hit = hitTest(toScene(event->position()));
    if (hit) {
        if (event->modifiers().testFlag(Qt::ControlModifier) && !event->modifiers().testFlag(Qt::ShiftModifier)) {
            selections_ = {*hit}; cloneSelection(); dragging_ = !selections_.empty(); publishSelection(); update(); return;
        }
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
    if (dragging_reference_) { reference_offset_ += toScene(event->position()) - toScene(last_position_); last_position_=event->position(); update(); return; }
    if (panning_) {
        pan_ += event->position() - last_position_;
        last_position_ = event->position();
        update();
        return;
    }
    if (dragging_ || resize_handle_ != Handle::None) {
        drag_delta_ = toScene(event->position()) - toScene(press_position_);
        if (grid_snap_ && !event->modifiers().testFlag(Qt::AltModifier)) {
            const auto grid = std::ranges::all_of(selections_, [](const auto& selection){return selection.kind==1||selection.kind==2;}) ? 16 : snap_size_;
            drag_delta_.setX(std::round(drag_delta_.x() / grid) * grid);
            drag_delta_.setY(std::round(drag_delta_.y() / grid) * grid);
        }
        emit interactionPreviewRequested(drag_delta_, static_cast<int>(resize_handle_));
        update();
        return;
    }
    if (marquee_selecting_) {
        marquee_.setBottomRight(event->position());
        update();
        return;
    }
    // Hover feedback: a knob if one is under the cursor, otherwise a move cursor
    // over anything grabbable at all.
    if (const auto handle = handleAt(event->position()); handle != Handle::None) {
        setCursor(handleCursor(handle));
    } else {
        setCursor(hitTest(toScene(event->position())) ? Qt::SizeAllCursor : Qt::ArrowCursor);
    }
    QWidget::mouseMoveEvent(event);
}

void LevelPreviewWidget::mouseReleaseEvent(QMouseEvent* event) {
    if (dragging_reference_) { dragging_reference_=false; return; }
    if (panning_) {
        panning_ = false;
        setCursor(Qt::ArrowCursor);
        return;
    }
    if (dragging_) {
        dragging_ = false;
        commitMove(drag_delta_);
        drag_delta_ = {};
        update();
        return;
    }
    if (resize_handle_ != Handle::None) {
        const auto edges = resizeEdges(drag_delta_);
        resize_handle_ = Handle::None;
        commitResize(edges);
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
        for (const auto& entity : entityRects(*scene_, sprite_definitions_, actor_definition_catalog_, actor_object_ids_)) {
            if (scene_rect.normalized().intersects(entity.rect) &&
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
    if (event->matches(QKeySequence::Copy)) { copySelection(); return; }
    if (event->matches(QKeySequence::Cut)) { cutSelection(); return; }
    if (event->matches(QKeySequence::Paste)) { pasteSelection(); return; }
    if (event->modifiers() == Qt::ControlModifier && event->key() == Qt::Key_D) { cloneSelection(); return; }
    if (event->key() == Qt::Key_PageUp || event->key() == Qt::Key_PageDown) { reorderSelection(event->key() == Qt::Key_PageUp); return; }
    if (event->modifiers() == Qt::NoModifier && (event->key() == Qt::Key_Left || event->key() == Qt::Key_Right || event->key() == Qt::Key_Up || event->key() == Qt::Key_Down)) {
        const auto step = std::ranges::all_of(selections_, [](const auto& selection){return selection.kind==1||selection.kind==2;}) ? 16.0 : static_cast<double>(snap_size_);
        const auto x = event->key() == Qt::Key_Left ? -step : event->key() == Qt::Key_Right ? step : 0.0;
        const auto y = event->key() == Qt::Key_Up ? -step : event->key() == Qt::Key_Down ? step : 0.0;
        commitMove({x, y}); update(); return;
    }
    if (event->modifiers() == Qt::NoModifier && event->key() >= Qt::Key_A && event->key() <= Qt::Key_Z && QStringLiteral("COSEVZPG").contains(QChar(event->key()))) {
        const auto key = QChar(event->key());
        const auto kind = key == 'C' ? 0 : key == 'O' ? 1 : key == 'S' ? 2 : key == 'E' ? 3 : key == 'Z' ? 4 : key == 'P' ? 5 : key == 'G' ? 6 : 7;
        emit kindShortcutRequested(kind); return;
    }
    if (event->modifiers() == Qt::NoModifier && event->key() >= Qt::Key_A && event->key() <= Qt::Key_Z && QStringLiteral("WASD").contains(QChar(event->key()))) {
        pan_ += event->key() == Qt::Key_A ? QPointF(32, 0) : event->key() == Qt::Key_D ? QPointF(-32, 0)
                : event->key() == Qt::Key_W ? QPointF(0, 32) : QPointF(0, -32);
        update(); return;
    }
    if (event->key() == Qt::Key_Escape) {
        dragging_ = false;
        resize_handle_ = Handle::None;
        marquee_selecting_ = false;
        drag_delta_ = {};
        marquee_ = {};
        emit interactionPreviewRequested({}, 0);
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

void LevelPreviewWidget::copySelection() {
    if (!scene_ || !scene_->level || selections_.empty()) return;
    QJsonArray entities; const auto& course = scene_->level->course();
    auto values = [](auto... fields) { QJsonArray result; (result.append(static_cast<qint64>(fields)), ...); return result; };
    for (const auto& selection : selections_) {
        const auto index = static_cast<std::size_t>(selection.item); QJsonObject item; item["kind"] = selection.kind;
        if (selection.kind == 1 && index < scene_->level->objects().size()) { const auto& v=scene_->level->objects()[index]; item["v"]=values(v.object_id,v.tileset_slot,v.x,v.y,v.width,v.height); }
        else if (selection.kind == 2 && index < course.sprites.size()) { const auto& v=course.sprites[index]; item["v"]=values(v.id,v.x,v.y,v.events[0],v.events[1],v.settings); if (const auto hash=scene_->level->sprite_hash(index)) item["hash"]=QString("%1").arg(*hash,8,16,QLatin1Char('0')); }
        else if (selection.kind == 3 && index < course.entrances.size()) { const auto& v=course.entrances[index]; item["v"]=values(v.x,v.y,v.camera_x,v.camera_y,v.id,v.target_stage_area,v.target_stage_index,v.target_stage_group,v.target_entrance,v.camera_zoom,v.type,v.flags,v.unknown_10,v.unknown_11,v.view_id,v.target_worldmap_node); }
        else if (selection.kind == 4 && index < course.zones.size()) { const auto& v=course.zones[index]; item["v"]=values(v.x,v.y,v.width,v.height,v.id,v.unknown_9,v.unknown_a,v.unknown_b); }
        else if ((selection.kind == 5 || selection.kind == 6)) { const auto& paths=selection.kind==6?course.progress_paths:course.paths; if(index>=paths.size())continue; const auto& v=paths[index]; item["v"]=values(v.id,v.unknown); QJsonArray nodes; for(const auto& n:v.nodes)nodes.append(values(n.x,n.y,n.speed.raw(),n.acceleration.raw(),n.delay,n.flags)); item["nodes"]=nodes; }
        else if (selection.kind == 7 && index < course.views.size()) { const auto& v=course.views[index]; item["v"]=values(v.x,v.y,v.width,v.height,v.id,v.camera_settings_id,v.bgm_id,v.background_id,v.tileset_id,v.foreground_id,v.light_type,v.progress_path_id); }
        else continue;
        entities.append(item);
    }
    QJsonObject root{{"schema","bigstar.entity-clipboard/1"},{"entities",entities}};
    const auto json = QJsonDocument(root).toJson(QJsonDocument::Compact);
    auto compressed = nsmb::formats::compress_nitro_lz10(std::span<const std::uint8_t>(reinterpret_cast<const std::uint8_t*>(json.data()), static_cast<std::size_t>(json.size())));
    if (!compressed) { emit errorOccurred(QString::fromStdString(compressed.error().message)); return; }
    QApplication::clipboard()->setText("BigStar-Entities/1\n" + QByteArray(reinterpret_cast<const char*>(compressed.value().data()), static_cast<qsizetype>(compressed.value().size())).toBase64());
}

void LevelPreviewWidget::cutSelection() { copySelection(); deleteSelection(); }

void LevelPreviewWidget::pasteSelection() {
    if (!scene_ || !scene_->level) return;
    const auto text = QApplication::clipboard()->text();
    if (!text.startsWith("BigStar-Entities/1\n")) { emit errorOccurred("Clipboard does not contain Big Star entities"); return; }
    const auto payload = text.sliced(19).toLatin1();
    if (payload.size() > 24 * 1024 * 1024) { emit errorOccurred("Clipboard entity payload is too large"); return; }
    const auto encoded = QByteArray::fromBase64(payload);
    if (encoded.toBase64() != payload) { emit errorOccurred("Clipboard entity payload is not valid Base64"); return; }
    auto json = nsmb::formats::decompress_nitro_lz(std::span<const std::uint8_t>(reinterpret_cast<const std::uint8_t*>(encoded.data()), static_cast<std::size_t>(encoded.size())));
    if (!json) { emit errorOccurred(QString::fromStdString(json.error().message)); return; }
    QJsonParseError parse_error; const auto document=QJsonDocument::fromJson(QByteArray(reinterpret_cast<const char*>(json.value().data()),static_cast<qsizetype>(json.value().size())),&parse_error);
    const auto root=document.object(); const auto entities=root["entities"].toArray();
    if(parse_error.error!=QJsonParseError::NoError||root["schema"].toString()!="bigstar.entity-clipboard/1"||entities.size()>10000){emit errorOccurred("Clipboard entity payload is invalid");return;}
    nsmb::project::LevelEntityInsertions insertions;
    auto number=[](const QJsonArray& v,int i,int low,int high)->std::optional<int>{if(i>=v.size()||!v[i].isDouble())return{};const auto n=v[i].toInteger(std::numeric_limits<qint64>::min());if(n<low||n>high)return{};return static_cast<int>(n);};
    bool valid=true;
    for(const auto entry:entities){const auto item=entry.toObject();const auto v=item["v"].toArray();const auto kind=item["kind"].toInt(-1);auto n=[&](int i,int low=-32768,int high=65535){auto result=number(v,i,low,high);valid&=result.has_value();return result.value_or(0);};
        if(kind==1&&v.size()==6)insertions.objects.push_back({static_cast<std::uint16_t>(n(0,0,4095)),static_cast<std::uint8_t>(n(1,0,15)),static_cast<std::int16_t>(n(2,-32768,32766)+1),static_cast<std::int16_t>(n(3,-32768,32766)+1),static_cast<std::uint16_t>(n(4,1)),static_cast<std::uint16_t>(n(5,1))});
        else if(kind==2&&v.size()==6){
            const auto settings=v[5].isDouble()?v[5].toInteger(-1):-1;
            valid&=settings>=0&&settings<=std::numeric_limits<std::uint32_t>::max();
            nsmb::project::SpriteInsertion sprite;sprite.value={static_cast<std::uint16_t>(n(0,0)),static_cast<std::uint16_t>(n(1,0,65534)+1),static_cast<std::uint16_t>(n(2,0,65534)+1),{static_cast<std::uint8_t>(n(3,0,255)),static_cast<std::uint8_t>(n(4,0,255))},static_cast<std::uint32_t>(settings)};
            if(item["hash"].isString()){bool ok=false;const auto text=item["hash"].toString();const auto hash=text.toUInt(&ok,16);valid&=ok&&text.size()==8;sprite.hash=hash;}insertions.sprites.push_back(sprite);
        }
        else if(kind==3&&v.size()==16){nsmb::formats::NsmbLevelEntrance e;e.x=n(0,0,65519)+16;e.y=n(1,0,65519)+16;e.camera_x=n(2,0);e.camera_y=n(3,0);e.id=n(4,0,255);e.target_stage_area=n(5,0,255);e.target_stage_index=n(6,0,255);e.target_stage_group=n(7,0,255);e.target_entrance=n(8,0,255);e.camera_zoom=n(9,0,255);e.type=n(10,0,255);e.flags=n(11,0,255);e.unknown_10=n(12,0,255);e.unknown_11=n(13,0,255);e.view_id=n(14,0,255);e.target_worldmap_node=n(15,0,255);insertions.entrances.push_back(e);}
        else if(kind==4&&v.size()==8){nsmb::formats::NsmbLevelZone z;z.x=n(0,0,65519)+16;z.y=n(1,0,65519)+16;z.width=n(2,1);z.height=n(3,1);z.id=n(4,0,255);z.unknown_9=n(5,0,255);z.unknown_a=n(6,0,255);z.unknown_b=n(7,0,255);insertions.zones.push_back(z);}
        else if((kind==5||kind==6)&&v.size()==2){
            nsmb::formats::NsmbLevelPath p;p.id=n(0,0,255);p.unknown=n(1,0);
            if(!item["nodes"].isArray()||item["nodes"].toArray().size()>10000)valid=false;
            for(const auto node:item["nodes"].toArray()){
                const auto a=node.toArray();if(a.size()!=6){valid=false;break;}
                const auto x=number(a,0,-32768,32751),y=number(a,1,-32768,32751),speed=number(a,2,std::numeric_limits<int>::min(),std::numeric_limits<int>::max()),acceleration=number(a,3,std::numeric_limits<int>::min(),std::numeric_limits<int>::max()),delay=number(a,4,0,65535),flags=number(a,5,0,65535);
                if(!x||!y||!speed||!acceleration||!delay||!flags){valid=false;break;}
                p.nodes.push_back({static_cast<std::int16_t>(*x+16),static_cast<std::int16_t>(*y+16),nsmb::core::Fx32::from_raw(*speed),nsmb::core::Fx32::from_raw(*acceleration),static_cast<std::uint16_t>(*delay),static_cast<std::uint16_t>(*flags)});
            }
            (kind==6?insertions.progress_paths:insertions.paths).push_back(std::move(p));
        }
        else if(kind==7&&v.size()==12){nsmb::formats::NsmbLevelView x;x.x=n(0,0,65519)+16;x.y=n(1,0,65519)+16;x.width=n(2,1);x.height=n(3,1);x.id=n(4,0,255);x.camera_settings_id=n(5,0,255);x.bgm_id=n(6,0,255);x.background_id=n(7,0,255);x.tileset_id=n(8,0,255);x.foreground_id=n(9,0,255);x.light_type=n(10,0,255);x.progress_path_id=n(11,0,255);insertions.views.push_back(x);}
        else valid=false;
    }
    if(!valid){emit errorOccurred("Clipboard entity values are invalid");return;}
    auto added=scene_->level->add_entities(std::move(insertions));if(!added){emit errorOccurred(QString::fromStdString(added.error().message));return;}
    selections_.clear();auto select=[&](const auto& indices,int kind){for(const auto index:indices)selections_.push_back({kind,static_cast<int>(index),-1});};select(added.value().objects,1);select(added.value().sprites,2);select(added.value().entrances,3);select(added.value().zones,4);select(added.value().paths,5);select(added.value().progress_paths,6);select(added.value().views,7);publishSelection();emit levelChanged();update();
}

void LevelPreviewWidget::cloneSelection() { const auto before=QApplication::clipboard()->text();copySelection();pasteSelection();QApplication::clipboard()->setText(before); }

void LevelPreviewWidget::reorderSelection(bool forward) {
    if(!scene_||!scene_->level||selections_.size()!=1)return;auto& selection=selections_.front();auto result=scene_->level->reorder_entity(selection.kind,static_cast<std::size_t>(selection.item),forward);if(!result){emit errorOccurred(QString::fromStdString(result.error().message));return;}selection.item=static_cast<int>(result.value());publishSelection();emit levelChanged();update();
}

void LevelPreviewWidget::keyReleaseEvent(QKeyEvent* event) {
    if (event->key() == Qt::Key_Space && !event->isAutoRepeat()) {
        space_pan_ = false;
        if (!panning_) setCursor(Qt::ArrowCursor);
        event->accept();
        return;
    }
    QWidget::keyReleaseEvent(event);
}

void LevelPreviewWidget::placeEntity(const QPointF& point) {
    if (!scene_ || !scene_->level) return;
    auto placement = point;
    if (grid_snap_ && !QApplication::keyboardModifiers().testFlag(Qt::AltModifier)) {
        const auto grid = (create_kind_ == 1 || create_kind_ == 2) ? 16 : snap_size_;
        placement.setX(std::round(placement.x() / grid) * grid);
        placement.setY(std::round(placement.y() / grid) * grid);
    }
    const auto pixel_x = std::clamp(qRound(placement.x()), 0, 65535);
    const auto pixel_y = std::clamp(qRound(placement.y()), 0, 65535);
    nsmb::core::Result<std::size_t> result =
        nsmb::core::make_error(nsmb::core::ErrorCode::Unsupported, "this entity kind cannot be created on the canvas");
    const int kind = create_kind_;
    switch (kind) {
    case 1:
        result = scene_->level->add_object({static_cast<std::uint16_t>(placement_object_id_),
                                            static_cast<std::uint8_t>(placement_tileset_slot_),
                                            static_cast<std::int16_t>(pixel_x / 16),
                                            static_cast<std::int16_t>(pixel_y / 16),
                                            1,
                                            1});
        break;
    case 2:
        {
            nsmb::formats::NsmbLevelSprite sprite{static_cast<std::uint16_t>(placement_sprite_type_),
                                                  static_cast<std::uint16_t>(pixel_x / 16),
                                                  static_cast<std::uint16_t>(pixel_y / 16), {}};
            result = placement_sprite_hash_ ? scene_->level->add_custom_sprite(*placement_sprite_hash_, sprite)
                                            : scene_->level->add_sprite(sprite);
        }
        break;
    case 3: {
        nsmb::formats::NsmbLevelEntrance entrance;
        entrance.x = static_cast<std::uint16_t>(pixel_x);
        entrance.y = static_cast<std::uint16_t>(pixel_y);
        result = scene_->level->add_entrance(entrance);
        break;
    }
    case 4:
        result = scene_->level->add_zone({static_cast<std::uint16_t>(pixel_x), static_cast<std::uint16_t>(pixel_y), 256, 192, 0});
        break;
    case 5:
    case 6: {
        const bool progress = kind == 6;
        nsmb::formats::NsmbLevelPath path;
        path.nodes.push_back({static_cast<std::int16_t>(pixel_x), static_cast<std::int16_t>(pixel_y)});
        result = scene_->level->add_path(progress, std::move(path));
        break;
    }
    case 7: {
        nsmb::formats::NsmbLevelView view{};
        view.x = static_cast<std::uint16_t>(pixel_x);
        view.y = static_cast<std::uint16_t>(pixel_y);
        view.width = 256;
        view.height = 192;
        result = scene_->level->add_view(view);
        break;
    }
    default:
        return;
    }
    if (!result) {
        emit errorOccurred(QString::fromStdString(result.error().message));
        return;
    }
    selections_ = {{kind, static_cast<int>(result.value()), pathKind(kind) ? 0 : -1}};
    publishSelection();
    emit levelChanged();
    update();
}

void LevelPreviewWidget::commitMove(const QPointF& delta) {
    if (!scene_ || !scene_->level || (qFuzzyIsNull(delta.x()) && qFuzzyIsNull(delta.y()))) return;
    auto clamp16 = [](int value) { return static_cast<std::uint16_t>(std::clamp(value, 0, 65535)); };
    auto clampSigned16 = [](int value) { return static_cast<std::int16_t>(std::clamp(value, -32768, 32767)); };
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
        case 7: {
            if (index >= scene_->level->course().views.size()) break;
            auto value = scene_->level->course().views[index];
            value.x = clamp16(static_cast<int>(value.x) + qRound(delta.x()));
            value.y = clamp16(static_cast<int>(value.y) + qRound(delta.y()));
            edits.views.push_back({index, value});
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
                node.x = clampSigned16(static_cast<int>(node.x) + qRound(delta.x()));
                node.y = clampSigned16(static_cast<int>(node.y) + qRound(delta.y()));
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

void LevelPreviewWidget::commitResize(const ResizeEdges& edges) {
    if (!scene_ || !scene_->level || selections_.size() != 1U) return;
    const auto selection = selections_.front();
    const auto index = static_cast<std::size_t>(selection.item);

    // Grabbing a top or left knob moves the origin and shrinks the extent by the
    // same amount, so both are adjusted together and the opposite edge holds
    // still. `scale` converts scene pixels into the entity's own units.
    const auto resized = [&](int origin, int extent, double near_edge, double far_edge, double scale, int minimum) {
        const auto near_delta = qRound(near_edge / scale);
        const auto far_delta = qRound(far_edge / scale);
        auto value = std::make_pair(origin + near_delta, extent - near_delta + far_delta);
        if (value.second < minimum) {
            // Dragged past the far edge: pin the size and stop the origin there.
            value.first -= minimum - value.second;
            value.second = minimum;
        }
        return value;
    };

    nsmb::project::LevelEntityUpdates edits;
    if (selection.kind == 1 && index < scene_->level->objects().size()) {
        auto object = scene_->level->objects()[index];
        // Objects are placed and sized in tiles, so the scene delta is 16-scaled.
        const auto [x, width] = resized(object.x, object.width, edges.left, edges.right, 16.0, 1);
        const auto [y, height] = resized(object.y, object.height, edges.top, edges.bottom, 16.0, 1);
        object.x = static_cast<std::int16_t>(std::clamp(x, -32768, 32767));
        object.y = static_cast<std::int16_t>(std::clamp(y, -32768, 32767));
        object.width = static_cast<std::uint16_t>(std::clamp(width, 1, 65535));
        object.height = static_cast<std::uint16_t>(std::clamp(height, 1, 65535));
        edits.objects.push_back({index, object});
    } else if (selection.kind == 4 && index < scene_->level->course().zones.size()) {
        auto zone = scene_->level->course().zones[index];
        const auto [x, width] = resized(zone.x, zone.width, edges.left, edges.right, 1.0, 1);
        const auto [y, height] = resized(zone.y, zone.height, edges.top, edges.bottom, 1.0, 1);
        zone.x = static_cast<std::uint16_t>(std::clamp(x, 0, 65535));
        zone.y = static_cast<std::uint16_t>(std::clamp(y, 0, 65535));
        zone.width = static_cast<std::uint16_t>(std::clamp(width, 1, 65535));
        zone.height = static_cast<std::uint16_t>(std::clamp(height, 1, 65535));
        edits.zones.push_back({index, zone});
    } else if (selection.kind == 7 && index < scene_->level->course().views.size()) {
        // A view is a pixel-space rectangle exactly like a zone, so it resizes
        // the same way. It only ever looked different because nothing offered a
        // handle for it.
        auto view = scene_->level->course().views[index];
        const auto [x, width] = resized(view.x, view.width, edges.left, edges.right, 1.0, 1);
        const auto [y, height] = resized(view.y, view.height, edges.top, edges.bottom, 1.0, 1);
        view.x = static_cast<std::uint16_t>(std::clamp(x, 0, 65535));
        view.y = static_cast<std::uint16_t>(std::clamp(y, 0, 65535));
        view.width = static_cast<std::uint16_t>(std::clamp(width, 1, 65535));
        view.height = static_cast<std::uint16_t>(std::clamp(height, 1, 65535));
        edits.views.push_back({index, view});
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
                       : item.kind == 6 ? &selection.progress_paths
                                        : &selection.views;
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
    actor_object_ids_.clear();
    for (const auto& actor : resources.actors) if (!actor.hash) actor_object_ids_.emplace(actor.type, actor.object_id);
    rebuildSpriteDefinitions();
    rebuildActorThumbnails();
    QPointer<LevelPreviewWidget> weak(this);
    QThreadPool::globalInstance()->start([weak, generation, resources = std::move(resources)]() mutable {
        std::array<std::shared_ptr<nsmb::project::DecodedTileset>, 3> tilesets;
        QImage background;
        QImage foreground;
        std::unordered_map<std::uint16_t, QString> actor_labels;
        std::unordered_map<std::uint32_t, QString> custom_actor_labels;
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
            const auto label = QString::fromUtf8(actor.name.data(), static_cast<qsizetype>(actor.name.size()));
            if (actor.hash) custom_actor_labels.emplace(*actor.hash, label); else actor_labels.emplace(actor.type, label);
        }
        if (!weak) return;
        QMetaObject::invokeMethod(
            weak,
            [weak, generation, tilesets = std::move(tilesets), background = std::move(background), foreground = std::move(foreground),
             actor_labels = std::move(actor_labels), custom_actor_labels = std::move(custom_actor_labels), warnings = warnings.join("\n")]() mutable {
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
                for (auto& [hash, label] : custom_actor_labels) weak->custom_actor_labels_.insert_or_assign(hash, std::move(label));
                if (tileset_changed) weak->object_images_.clear();
                weak->update();
                if (!warnings.isEmpty()) emit weak->graphicsWarning(warnings);
                emit weak->graphicsLoaded();
            },
            Qt::QueuedConnection);
    });
}

void LevelPreviewWidget::setCustomActors(const nsmb::project::ModuleDataCatalog& catalog) {
    custom_actor_labels_.clear();
    for (const auto& actor : catalog.stage_objects)
        custom_actor_labels_.emplace(actor.hash, QString::fromStdString(actor.name));
    update();
}

void LevelPreviewWidget::setActorDefinitions(const nsmb::project::LuaActorCatalog& catalog) {
    actor_definition_catalog_ = catalog;
    setSpriteDefinitions(catalog.sprite_definitions());
    rebuildActorThumbnails();
}

// The picker and minimap show what the canvas shows, so a thumbnail is the
// actor's own program run at zero settings rather than a separate preview path.
void LevelPreviewWidget::rebuildActorThumbnails() {
    actor_images_.clear();
    custom_actor_images_.clear();
    std::unordered_map<std::uint32_t, std::uint16_t> types;
    for (const auto& [type, object_id] : actor_object_ids_) types.emplace(object_id, type);
    for (const auto& actor : actor_definition_catalog_.actors) {
        if (!nsmb::project::has_lua_renderer(actor)) continue;
        nsmb::project::LuaActorRenderInput input;
        input.object_id = actor.sprite.object_id;
        input.hash = actor.hash;
        const auto commands = nsmb::project::render_lua_actor(actor, input);
        if (!commands || commands.value().empty()) continue;
        QRectF box;
        for (const auto& command : commands.value())
            box = box.isNull() ? QRectF(command.x, command.y, command.width, command.height)
                               : box.united(QRectF(command.x, command.y, command.width, command.height));
        if (box.width() < 1.0 || box.height() < 1.0) continue;
        QImage thumbnail(qCeil(box.width()), qCeil(box.height()), QImage::Format_RGBA8888);
        thumbnail.fill(Qt::transparent);
        QPainter painter(&thumbnail);
        painter.translate(-box.topLeft());
        for (const auto& command : commands.value()) paintActorCommand(painter, command);
        painter.end();
        if (thumbnail.width() > 64 || thumbnail.height() > 64)
            thumbnail = thumbnail.scaled(64, 64, Qt::KeepAspectRatio, Qt::SmoothTransformation);
        if (actor.hash) custom_actor_images_.emplace(*actor.hash, std::move(thumbnail));
        else if (const auto type = types.find(actor.sprite.object_id); type != types.end())
            actor_images_.emplace(type->second, std::move(thumbnail));
    }
}

void LevelPreviewWidget::setSpriteDefinitions(const nsmb::project::SpriteDefinitionCatalog& catalog) {
    sprite_definition_catalog_ = catalog;
    rebuildSpriteDefinitions();
    update();
}

void LevelPreviewWidget::rebuildSpriteDefinitions() {
    sprite_definitions_.clear();
    for (const auto& [type, object_id] : actor_object_ids_)
        if (const auto* definition = sprite_definition_catalog_.by_object_id(object_id))
            sprite_definitions_.emplace(type, *definition);
    for (const auto& definition : sprite_definition_catalog_.definitions)
        if (definition.hash) sprite_definitions_.emplace(spriteDefinitionKey(0, definition.hash), definition);
}

void LevelPreviewWidget::clearGraphics() {
    ++graphics_generation_;
    tilesets_ = {};
    background_ = {};
    foreground_ = {};
    object_images_.clear();
    actor_images_.clear();
    custom_actor_images_.clear();
    actor_labels_.clear();
    actor_object_ids_.clear();
    sprite_definitions_.clear();
    graphics_references_.reset();
}

void LevelPreviewWidget::invalidateGraphics() { graphics_references_.reset(); }

bool LevelPreviewWidget::graphicsMatch(nsmb::project::LevelTilesetReferences references) const noexcept {
    return graphics_references_ && graphics_references_->tileset_id == references.tileset_id &&
           graphics_references_->background_graphics_id == references.background_graphics_id &&
           graphics_references_->background_screen_id == references.background_screen_id &&
           graphics_references_->foreground_graphics_id == references.foreground_graphics_id &&
           graphics_references_->foreground_screen_id == references.foreground_screen_id;
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
        if (selection.kind == 7) return item >= course.views.size();
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
            // Preview an object at its own size, not at one tile. A pipe drawn
            // 1x1 is a corner, which tells you nothing about what you are about
            // to place. Repeat-marked rows would tile forever, so the natural
            // extent is capped at a size that still fits a palette button.
            const auto& layout = tilesets_[slot]->objects[object];
            std::size_t natural_width = 1;
            for (const auto& row : layout.rows) natural_width = std::max(natural_width, row.size());
            const auto natural_height = std::max<std::size_t>(1, layout.rows.size());
            const auto preview_width = std::clamp<std::size_t>(natural_width, 1, 8);
            const auto preview_height = std::clamp<std::size_t>(natural_height, 1, 8);
            auto rendered = nsmb::project::render_tileset_object(
                *tilesets_[slot], static_cast<std::uint16_t>(object), preview_width, preview_height);
            if (rendered) {
                const auto& value = rendered.value();
                image = QImage(value.pixels.data(),
                               static_cast<int>(value.width),
                               static_cast<int>(value.height),
                               static_cast<int>(value.width * 4U),
                               QImage::Format_RGBA8888)
                            .copy();
            }
            entries.push_back({1,
                               static_cast<int>(object),
                               static_cast<int>(slot),
                               QString("Object %1:%2").arg(slot).arg(object),
                               QString("Tileset %1").arg(slot),
                               QString("Object %1 of tileset slot %2 - %3x%4 tiles")
                                   .arg(object).arg(slot).arg(natural_width).arg(natural_height),
                               std::move(image)});
        }
    }
    for (const auto& [type, label] : actor_labels_) {
        const auto image = actor_images_.contains(type) ? actor_images_.at(type) : QImage{};
        const auto definition = sprite_definitions_.find(type);
        const auto name = definition == sprite_definitions_.end() ? label
                                                                 : QString::fromStdString(definition->second.name);
        entries.push_back({2, type, 0,
                           name + QString(" (%1)").arg(type),
                           definition == sprite_definitions_.end() ? QString("Other actors")
                                                                   : QString::fromStdString(definition->second.category),
                           QString("%1 - actor type %2").arg(name).arg(type),
                           image});
    }
    std::ranges::sort(entries, [](const auto& left, const auto& right) {
        if (left.kind != right.kind) return left.kind < right.kind;
        if (left.slot != right.slot) return left.slot < right.slot;
        return left.id < right.id;
    });
    return entries;
}

QImage LevelPreviewWidget::customActorImage(std::uint32_t hash) const {
    const auto found = custom_actor_images_.find(hash);
    return found == custom_actor_images_.end() ? QImage{} : found->second;
}

void LevelPreviewWidget::setPlacementObject(int slot, int object_id) {
    placement_tileset_slot_ = std::clamp(slot, 0, 2);
    placement_object_id_ = std::clamp(object_id, 0, 4095);
    setCreateKind(1);
}

void LevelPreviewWidget::setPlacementSprite(int sprite_type) {
    placement_sprite_type_ = std::clamp(sprite_type, 0, 65535);
    placement_sprite_hash_.reset();
    setCreateKind(2);
}

void LevelPreviewWidget::setPlacementCustomSprite(std::uint32_t hash) {
    placement_sprite_hash_ = hash;
    placement_sprite_type_ = 0;
    setCreateKind(2);
}

void LevelPreviewWidget::clearPlacement() {
    placement_object_id_ = 0;
    placement_tileset_slot_ = 0;
    placement_sprite_type_ = 0;
    placement_sprite_hash_.reset();
}
