#include "GraphicsCanvas.hpp"
#include "ThemeColors.hpp"

#include <QColor>
#include <QMouseEvent>
#include <QPainter>

#include <algorithm>

GraphicsCanvas::GraphicsCanvas(QWidget* parent) : QWidget(parent) {
    setObjectName("GraphicsCanvas");
    setCursor(Qt::CrossCursor);
}

void GraphicsCanvas::setImage(QImage value, GraphicsCanvasGrid grid, int fit, int maximum_zoom) {
    image_ = std::move(value);
    grid_ = grid;
    const auto longest = std::max(image_.width(), image_.height());
    zoom_ = std::clamp(longest ? fit / longest : 1, 1, maximum_zoom);
    setFixedSize(std::max(1, image_.width() * zoom_), std::max(1, image_.height() * zoom_));
    update();
}

void GraphicsCanvas::paintEvent(QPaintEvent*) {
    QPainter painter(this);
    painter.fillRect(rect(), palette().brush(QPalette::Base));
    painter.setRenderHint(QPainter::SmoothPixmapTransform, false);
    if (!image_.isNull()) painter.drawImage(rect(), image_);
    if (grid_.cell <= 0 || grid_.cell * zoom_ < grid_.minimum_pixels) return;
    painter.setPen(bse::theme::pixelGrid(palette()));
    for (int x = 0; x <= image_.width(); x += grid_.cell) painter.drawLine(x * zoom_, 0, x * zoom_, height());
    for (int y = 0; y <= image_.height(); y += grid_.cell) painter.drawLine(0, y * zoom_, width(), y * zoom_);
}

void GraphicsCanvas::mousePressEvent(QMouseEvent* event) {
    if (!clicked || image_.isNull()) return;
    const auto x = static_cast<int>(event->position().x()) / zoom_;
    const auto y = static_cast<int>(event->position().y()) / zoom_;
    if (x >= 0 && y >= 0 && x < image_.width() && y < image_.height()) clicked(x, y);
}

namespace bse::graphics {

QColor fromBgr555(std::uint16_t value) {
    const auto expand = [](std::uint16_t channel) { return static_cast<int>((channel << 3U) | (channel >> 2U)); };
    return {expand(value & 31U), expand((value >> 5U) & 31U), expand((value >> 10U) & 31U)};
}

std::uint16_t toBgr555(const QColor& value) {
    const auto five = [](int channel) { return static_cast<std::uint16_t>((channel * 31 + 127) / 255); };
    return static_cast<std::uint16_t>(five(value.red()) | (five(value.green()) << 5U) | (five(value.blue()) << 10U));
}

} // namespace bse::graphics
