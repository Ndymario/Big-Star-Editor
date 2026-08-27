#pragma once

#include <QColor>
#include <QPalette>

// Status accents are picked per theme: the desktop shell follows the system palette, so a colour
// tuned for a light window (dark ambers, deep reds) turns unreadable once the window goes dark.
namespace bse::theme {

[[nodiscard]] inline bool isDark(const QPalette& palette) {
    return palette.color(QPalette::Window).lightness() < 128;
}

[[nodiscard]] inline QColor pick(const QPalette& palette, const QColor& light, const QColor& dark) {
    return isDark(palette) ? dark : light;
}

[[nodiscard]] inline QColor warning(const QPalette& palette) {
    return pick(palette, QColor(0x9a, 0x5b, 0x00), QColor(0xf0, 0xb6, 0x4a));
}

[[nodiscard]] inline QColor danger(const QPalette& palette) {
    return pick(palette, QColor(0xb4, 0x2d, 0x2d), QColor(0xf2, 0x83, 0x7c));
}

// Pixel grids are drawn over QPalette::Base, so they follow the text colour instead of pure black.
[[nodiscard]] inline QColor pixelGrid(const QPalette& palette) {
    auto color = palette.color(QPalette::Text);
    color.setAlpha(45);
    return color;
}

} // namespace bse::theme
