#pragma once

#include <QImage>
#include <QWidget>

#include <cstdint>
#include <functional>

class QColor;

// One zoom-to-fit indexed-pixel canvas for every editor that shows DS graphics.
// The graphics editor, the UI resource pages and the tileset editor each grew
// their own copy of this; they now share this one.
// A grid is drawn every `cell` pixels, but only once a cell is at least
// `minimum_pixels` across on screen — below that it is noise.
struct GraphicsCanvasGrid {
    int cell{};
    int minimum_pixels{8};
};

class GraphicsCanvas final : public QWidget {
public:
    using Grid = GraphicsCanvasGrid;

    explicit GraphicsCanvas(QWidget* parent = nullptr);

    void setImage(QImage value, GraphicsCanvasGrid grid = {}, int fit = 512, int maximum_zoom = 16);
    [[nodiscard]] int zoom() const noexcept { return zoom_; }

    // Called with image coordinates when a pixel inside the image is clicked.
    std::function<void(int, int)> clicked;

protected:
    void paintEvent(QPaintEvent*) override;
    void mousePressEvent(QMouseEvent* event) override;

private:
    QImage image_;
    GraphicsCanvasGrid grid_;
    int zoom_{1};
};

namespace bse::graphics {

// BGR555 is how every DS palette stores a colour, and every editor here has to
// cross that boundary in both directions.
[[nodiscard]] QColor fromBgr555(std::uint16_t value);
[[nodiscard]] std::uint16_t toBgr555(const QColor& value);

} // namespace bse::graphics
