#include "GraphicsEditorDialog.hpp"
#include "GraphicsCanvas.hpp"
#include "UiResourceEditorDialog.hpp"

#include "nsmb/formats/g2d.hpp"
#include "nsmb/formats/graphics.hpp"
#include "nsmb/formats/tex0.hpp"

#include <QColorDialog>
#include <QComboBox>
#include <QDialogButtonBox>
#include <QFormLayout>
#include <QLabel>
#include <QListWidget>
#include <QMouseEvent>
#include <QPainter>
#include <QPushButton>
#include <QScrollArea>
#include <QSettings>
#include <QSpinBox>
#include <QVBoxLayout>

#include <algorithm>
#include <cmath>
#include <functional>
#include <memory>
#include <optional>

namespace {

using namespace nsmb::formats;

QColor color(const Bgr555Color& value) {
    return QColor(value.red, value.green, value.blue, value.transparent ? 0 : 255);
}

Bgr555Color unpack(std::span<const std::uint8_t> bytes, std::size_t index) {
    if (index * 2U + 1U >= bytes.size()) return {};
    const auto value = static_cast<std::uint16_t>(bytes[index * 2U] | (bytes[index * 2U + 1U] << 8U));
    const auto expand = [](std::uint16_t channel) { return static_cast<std::uint8_t>((channel << 3U) | (channel >> 2U)); };
    return {expand(value & 31U), expand((value >> 5U) & 31U), expand((value >> 10U) & 31U), false};
}

void pack(std::vector<std::uint8_t>& bytes, std::size_t index, const QColor& value) {
    if (index * 2U + 1U >= bytes.size()) return;
    const auto five = [](int channel) { return static_cast<std::uint16_t>((channel * 31 + 127) / 255); };
    const auto packed = static_cast<std::uint16_t>(five(value.red()) | (five(value.green()) << 5U) |
                                                   (five(value.blue()) << 10U));
    bytes[index * 2U] = static_cast<std::uint8_t>(packed);
    bytes[index * 2U + 1U] = static_cast<std::uint8_t>(packed >> 8U);
}

QImage image(const RgbaImage& source) {
    QImage output(static_cast<int>(source.width), static_cast<int>(source.height), QImage::Format_RGBA8888);
    if (!source.pixels.empty()) std::copy(source.pixels.begin(), source.pixels.end(), output.bits());
    return output;
}

std::size_t sheet_width(std::size_t tile_count, std::size_t declared) {
    if (declared && declared != 0xffffU && declared <= tile_count) return declared;
    return std::max<std::size_t>(1, static_cast<std::size_t>(std::ceil(std::sqrt(static_cast<double>(tile_count)))));
}

} // namespace

struct GraphicsEditorPanel::Impl {
    enum class Mode { Unsupported, Nclr, Ncgr, Nscr, Ncer, Nanr, Tex0, RawPalette, RawCharacters, RawScreen };

    Impl(GraphicsEditorPanel* owner, std::string_view path, std::span<const std::uint8_t> source,
         bse::editors::Companions companions = {}, bool rom_user_metadata = false)
        : owner(owner), path(path), body(source.begin(), source.end()), companions(std::move(companions)) {
        // The uiStudio layouts, ENPG images and ROM metadata are pages of this
        // same editor. Only when none of them claims the file does it go down
        // the NNS G2D path.
        if (bse::editors::UiResourcePages::supports(body, path, rom_user_metadata)) {
            pages = std::make_unique<bse::editors::UiResourcePages>(owner, path, body, rom_user_metadata);
            return;
        }
        kind = identify_graphics_format(body, path);
        parse();
    }

    // The palette the picture is drawn with: the paired NCLR when one has been
    // chosen, and otherwise a grey ramp standing in for "index N". A grey ramp
    // is a debugging view, not a picture, which is why pairing matters.
    [[nodiscard]] std::vector<std::uint8_t> activePalette(std::size_t colours) const {
        if (!companion_palette.empty()) return companion_palette;
        std::vector<std::uint8_t> gray(colours * 2U);
        for (std::size_t i = 0; i < colours; ++i) {
            const auto shade = static_cast<int>(i * 255U / std::max<std::size_t>(1, colours - 1U));
            pack(gray, i, QColor(shade, shade, shade));
        }
        return gray;
    }

    // Reads a companion and keeps whichever kind of resource it turned out to
    // be. Anything unreadable is reported rather than silently ignored.
    void loadCompanion(const QString& value, GraphicsFormatKind expected) {
        const auto clear = [&] {
            if (expected == GraphicsFormatKind::NitroPalette) companion_palette.clear();
            else companion_characters.clear();
        };
        if (value.isEmpty() || !companions.load) { clear(); return; }
        auto bytes = companions.load(value);
        if (!bytes) { clear(); companion_error = QString::fromStdString(bytes.error().message); return; }
        companion_error.clear();
        if (expected == GraphicsFormatKind::NitroPalette) {
            auto parsed = parse_nclr(bytes.value());
            companion_palette = parsed ? parsed.value().palette : std::move(bytes.value());
        } else {
            auto parsed = parse_ncgr(bytes.value());
            if (parsed) {
                companion_characters = parsed.value().characters;
                companion_characters_8bpp = parsed.value().pixel_format == 4U;
            } else {
                companion_characters = std::move(bytes.value());
                companion_characters_8bpp = false;
            }
        }
    }

    void parse() {
        switch (kind) {
        case GraphicsFormatKind::NitroPalette: {
            auto value = parse_nclr(body); if (!value) return fail(value.error()); nclr = std::move(value.value()); mode = Mode::Nclr; break;
        }
        case GraphicsFormatKind::NitroCharacterGraphics: {
            auto value = parse_ncgr(body); if (!value) return fail(value.error()); ncgr = std::move(value.value()); mode = Mode::Ncgr; break;
        }
        case GraphicsFormatKind::NitroScreenMap: {
            auto value = parse_nscr(body); if (!value) return fail(value.error()); nscr = std::move(value.value()); mode = Mode::Nscr; break;
        }
        case GraphicsFormatKind::NitroCellLayout: {
            auto value = parse_ncer(body); if (!value) return fail(value.error()); ncer = std::move(value.value()); mode = Mode::Ncer; break;
        }
        case GraphicsFormatKind::NitroCellAnimation: {
            auto value = parse_nanr(body); if (!value) return fail(value.error()); nanr = std::move(value.value()); mode = Mode::Nanr; break;
        }
        case GraphicsFormatKind::NitroTexture:
        case GraphicsFormatKind::NitroModel: {
            auto value = parse_tex0(body); if (!value) return fail(value.error()); tex0 = std::move(value.value()); mode = Mode::Tex0; break;
        }
        case GraphicsFormatKind::RawPaletteCandidate:
            if (body.empty() || body.size() % 2U) return fail(nsmb::core::make_error(nsmb::core::ErrorCode::ParseError, "raw palette size must be even"));
            mode = Mode::RawPalette; break;
        case GraphicsFormatKind::RawCharacterGraphicsCandidate:
            if (body.empty() || body.size() % 32U) return fail(nsmb::core::make_error(nsmb::core::ErrorCode::ParseError, "raw 4bpp graphics must contain complete tiles"));
            mode = Mode::RawCharacters; break;
        case GraphicsFormatKind::RawScreenMapCandidate:
            if (body.empty() || body.size() % 2U) return fail(nsmb::core::make_error(nsmb::core::ErrorCode::ParseError, "raw screen map size must be even"));
            mode = Mode::RawScreen; break;
        default: return fail(nsmb::core::make_error(nsmb::core::ErrorCode::Unsupported, "this file is not a supported Nintendo DS graphics resource"));
        }
    }

    void buildUi() {
        auto* root = new QVBoxLayout(owner);
        root->setContentsMargins(0, 0, 0, 0);
        if (pages) { pages->build(root); return; }
        info = new QLabel(owner); info->setObjectName("GraphicsInfo"); info->setWordWrap(true); root->addWidget(info);
        auto* form = new QFormLayout;
        primary = new QComboBox(owner); primary->setObjectName("GraphicsPrimarySelector");
        secondary = new QComboBox(owner); secondary->setObjectName("GraphicsPaletteSelector");
        form->addRow("Image/cell", primary); form->addRow("Palette", secondary);

        // A palette lives in a different file from the graphics it colours, and
        // a screen map in a third. Pairing them here is the whole point of one
        // graphics editor rather than a viewer per file.
        palette_source = new QComboBox(owner); palette_source->setObjectName("GraphicsPaletteSource");
        characters_source = new QComboBox(owner); characters_source->setObjectName("GraphicsCharacterSource");
        form->addRow("Palette file", palette_source);
        form->addRow("Graphics file", characters_source);

        // A headerless _ncg.bin carries no width or pixel format, so the values
        // its NCGR header would have supplied are asked for instead of guessed.
        raw_width = new QSpinBox(owner); raw_width->setObjectName("GraphicsRawWidth");
        raw_width->setRange(0, 256); raw_width->setSpecialValueText("auto"); raw_width->setSuffix(" tiles");
        form->addRow("Width", raw_width);
        root->addLayout(form);
        auto* scroll = new QScrollArea(owner); scroll->setWidgetResizable(false); scroll->setAlignment(Qt::AlignCenter);
        canvas = new GraphicsCanvas(scroll); scroll->setWidget(canvas); root->addWidget(scroll, 1);
        swatches = new QListWidget(owner); swatches->setObjectName("GraphicsPalette"); swatches->setViewMode(QListView::IconMode);
        swatches->setFlow(QListView::LeftToRight); swatches->setWrapping(true); swatches->setMaximumHeight(120);
        swatches->setIconSize(QSize(22, 22)); swatches->setGridSize(QSize(48, 42)); root->addWidget(swatches);
        QObject::connect(primary, &QComboBox::currentIndexChanged, owner, [this] { refresh(); });
        QObject::connect(secondary, &QComboBox::currentIndexChanged, owner, [this] { refresh(); });
        QObject::connect(palette_source, &QComboBox::currentIndexChanged, owner, [this] {
            loadCompanion(palette_source->currentData().toString(), GraphicsFormatKind::NitroPalette);
            rememberChoices();
            refresh();
        });
        QObject::connect(characters_source, &QComboBox::currentIndexChanged, owner, [this] {
            loadCompanion(characters_source->currentData().toString(), GraphicsFormatKind::NitroCharacterGraphics);
            rememberChoices();
            refresh();
        });
        QObject::connect(raw_width, &QSpinBox::valueChanged, owner, [this] { rememberChoices(); refresh(); });
        QObject::connect(swatches, &QListWidget::currentRowChanged, owner, [this](int row) { if (row >= 0) selected_color = static_cast<std::size_t>(row); });
        QObject::connect(swatches, &QListWidget::itemDoubleClicked, owner, [this](QListWidgetItem* item) { editColor(item); });
        canvas->clicked = [this](int x, int y) { editPixel(x, y); };

        populateSelectors();
        populateCompanions();
        if (error) info->setText(QString::fromStdString(error->message));
        else refresh();
    }

    // A file's settings are worth remembering: a sheet only has to be paired up
    // and dialled in once.
    [[nodiscard]] QString settingsKey() const {
        return "graphicsEditor/" + QString::fromStdString(path).toUtf8().toBase64(
            QByteArray::Base64UrlEncoding | QByteArray::OmitTrailingEquals);
    }

    void rememberChoices() {
        if (loading_choices) return;
        QSettings settings;
        settings.setValue(settingsKey() + "/palette", palette_source->currentData().toString());
        settings.setValue(settingsKey() + "/characters", characters_source->currentData().toString());
        settings.setValue(settingsKey() + "/width", raw_width->value());
    }

    void populateCompanions() {
        const auto pairs_palette = mode == Mode::Ncgr || mode == Mode::RawCharacters ||
                                   mode == Mode::Nscr || mode == Mode::RawScreen ||
                                   mode == Mode::Ncer || mode == Mode::Nanr;
        const auto pairs_characters = mode == Mode::Nscr || mode == Mode::RawScreen ||
                                      mode == Mode::Ncer || mode == Mode::Nanr;
        palette_source->setVisible(pairs_palette && companions.usable());
        characters_source->setVisible(pairs_characters && companions.usable());
        raw_width->setVisible(mode == Mode::RawCharacters || mode == Mode::RawScreen);
        if (auto* form = owner ? owner->findChild<QFormLayout*>() : nullptr) {
            if (auto* label = form->labelForField(palette_source)) label->setVisible(palette_source->isVisible());
            if (auto* label = form->labelForField(characters_source)) label->setVisible(characters_source->isVisible());
            if (auto* label = form->labelForField(raw_width)) label->setVisible(raw_width->isVisible());
        }
        if (!companions.usable()) return;

        loading_choices = true;
        for (auto* box : {palette_source, characters_source}) {
            box->clear();
            box->addItem("(none)", QString());
        }
        for (const auto& candidate : companions.paths) {
            const auto candidate_kind = identify_graphics_format({}, candidate.toStdString());
            if (candidate_kind == GraphicsFormatKind::NitroPalette ||
                candidate_kind == GraphicsFormatKind::RawPaletteCandidate) {
                palette_source->addItem(candidate, candidate);
            } else if (candidate_kind == GraphicsFormatKind::NitroCharacterGraphics ||
                       candidate_kind == GraphicsFormatKind::RawCharacterGraphicsCandidate) {
                characters_source->addItem(candidate, candidate);
            }
        }

        QSettings settings;
        const auto restore = [&](QComboBox* box, const QString& name) {
            const auto stored = settings.value(settingsKey() + "/" + name).toString();
            if (stored.isEmpty()) return;
            if (const auto index = box->findData(stored); index >= 0) box->setCurrentIndex(index);
        };
        restore(palette_source, "palette");
        restore(characters_source, "characters");
        raw_width->setValue(settings.value(settingsKey() + "/width", 0).toInt());
        loading_choices = false;

        loadCompanion(palette_source->currentData().toString(), GraphicsFormatKind::NitroPalette);
        loadCompanion(characters_source->currentData().toString(), GraphicsFormatKind::NitroCharacterGraphics);
    }

    void populateSelectors() {
        primary->clear(); secondary->clear();
        primary->setVisible(mode == Mode::Ncer || mode == Mode::Nanr || mode == Mode::Tex0 || mode == Mode::RawCharacters);
        secondary->setVisible(mode == Mode::Tex0);
        if (mode == Mode::Ncer) for (std::size_t i = 0; i < ncer->cells.size(); ++i) primary->addItem(QString("Cell %1").arg(i));
        if (mode == Mode::Nanr) for (std::size_t i = 0; i < nanr->sequences.size(); ++i) primary->addItem(QString("Sequence %1").arg(i));
        if (mode == Mode::RawCharacters) { primary->addItem("4bpp"); primary->addItem("8bpp"); }
        if (mode == Mode::Tex0) {
            for (const auto& texture : tex0->textures) primary->addItem(QString::fromStdString(texture.name));
            for (const auto& palette : tex0->palettes) secondary->addItem(QString::fromStdString(palette.name));
        }
    }

    void refresh() {
        switch (mode) {
        case Mode::Nclr: setPalette(nclr->palette, true); info->setText(QString("NCLR palette — %1 colours").arg(nclr->palette.size() / 2U)); paletteImage(nclr->palette); break;
        case Mode::RawPalette: setPalette(body, true); info->setText(QString("Raw BGR555 palette — %1 colours").arg(body.size() / 2U)); paletteImage(body); break;
        case Mode::Ncgr: {
            // NNSG2dCharacterData.characterFmt bits 0-7 are NNSG2dCharacterFmt:
            // 0 is CHAR, 8x8 tiled; 1 is BMP, a linear bitmap -- the NCBR
            // variant, which shares the NCGR signature and would otherwise be
            // de-tiled into nonsense.
            const auto linear = (ncgr->character_format & 0xffU) == 1U;
            // The CPOS block records the real source dimensions. When it is
            // there it beats guessing a square root.
            const auto declared = ncgr->position && ncgr->position->width >= 8U
                                      ? static_cast<std::size_t>(ncgr->position->width) / 8U
                                      : ncgr->width;
            characters(ncgr->characters, declared, ncgr->pixel_format == 4U, linear);
            break;
        }
        case Mode::RawCharacters:
            characters(body, static_cast<std::size_t>(raw_width->value()), primary->currentIndex() == 1, false);
            break;
        case Mode::Nscr: screen(nscr->screen, nscr->width, nscr->height); break;
        case Mode::RawScreen: screen(body, static_cast<std::size_t>(raw_width->value()) * 8U, 0); break;
        case Mode::Ncer: cells(); break;
        case Mode::Nanr: animation(); break;
        case Mode::Tex0: texture(); break;
        default: break;
        }
    }

    void setPalette(std::span<const std::uint8_t> bytes, bool editable) {
        swatches->clear(); swatches_editable = editable;
        for (std::size_t i = 0; i < bytes.size() / 2U; ++i) {
            auto* item = new QListWidgetItem(QString::number(i), swatches);
            QPixmap icon(22, 22); icon.fill(color(unpack(bytes, i))); item->setIcon(icon);
        }
        if (swatches->count()) swatches->setCurrentRow(static_cast<int>(std::min(selected_color, bytes.size() / 2U - 1U)));
        swatches->setVisible(!bytes.empty());
    }

    void paletteImage(std::span<const std::uint8_t> bytes) {
        const auto count = bytes.size() / 2U, width = std::min<std::size_t>(16, std::max<std::size_t>(1, count));
        QImage output(static_cast<int>(width), static_cast<int>((count + width - 1U) / width), QImage::Format_RGBA8888);
        output.fill(Qt::transparent);
        for (std::size_t i = 0; i < count; ++i) output.setPixelColor(static_cast<int>(i % width), static_cast<int>(i / width), color(unpack(bytes, i)));
        canvas->setImage(output);
    }

    void characters(std::vector<std::uint8_t>& bytes, std::size_t declared_width, bool eight_bpp, bool linear) {
        character_data = &bytes; character_8bpp = eight_bpp; character_linear = linear;
        const std::size_t tile_size = eight_bpp ? 64U : 32U;
        const auto tiles = bytes.size() / tile_size;
        character_width_tiles = sheet_width(tiles, declared_width);
        const auto rows = (tiles + character_width_tiles - 1U) / character_width_tiles;
        const auto width_pixels = character_width_tiles * 8U;
        QImage output(static_cast<int>(width_pixels), static_cast<int>(rows * 8U), QImage::Format_RGBA8888);
        output.fill(Qt::transparent);
        const auto colours = eight_bpp ? 256U : 16U;
        const auto palette = activePalette(colours);
        for (std::size_t index = 0; index < tiles * 64U; ++index) {
            const auto value = eight_bpp
                ? bytes[index]
                : static_cast<std::uint8_t>((bytes[index / 2U] >> ((index & 1U) * 4U)) & 15U);
            std::size_t x = 0, y = 0;
            if (linear) {
                x = index % width_pixels;
                y = index / width_pixels;
            } else {
                const auto tile = index / 64U, pixel = index % 64U;
                x = (tile % character_width_tiles) * 8U + pixel % 8U;
                y = (tile / character_width_tiles) * 8U + pixel / 8U;
            }
            if (y >= static_cast<std::size_t>(output.height())) break;
            output.setPixelColor(static_cast<int>(x), static_cast<int>(y), color(unpack(palette, value)));
        }
        setPalette(palette, companion_palette.empty() ? false : true);
        canvas->setImage(output, {1});
        info->setText(QString("%1 — %2 tiles, %3bpp, %4.%5 Choose an index, then click pixels to paint.")
                          .arg(mode == Mode::Ncgr ? "NCGR" : "Raw graphics")
                          .arg(tiles)
                          .arg(eight_bpp ? 8 : 4)
                          .arg(linear ? "bitmap arrangement (NCBR)" : "8x8 tiled")
                          .arg(companion_palette.empty()
                                   ? QString(" Indices are shown as greys until a palette file is chosen.")
                                   : QString()));
    }

    void screen(std::span<const std::uint8_t> bytes, std::size_t declared_width, std::size_t declared_height) {
        const auto entries = bytes.size() / 2U;
        auto width = declared_width >= 8U ? declared_width / 8U : sheet_width(entries, 0);
        if (!width) width = 1;
        const auto height = declared_height >= 8U ? declared_height / 8U : (entries + width - 1U) / width;
        QImage output(static_cast<int>(width * 8U), static_cast<int>(height * 8U), QImage::Format_RGBA8888);
        output.fill(Qt::transparent);
        const auto composed = !companion_characters.empty();
        const auto colours = companion_characters_8bpp ? 256U : 16U;
        const auto palette = activePalette(colours);
        QPainter painter(&output);
        for (std::size_t i = 0; i < entries; ++i) {
            const auto value = static_cast<std::uint16_t>(bytes[i * 2U] | (bytes[i * 2U + 1U] << 8U));
            const auto left = static_cast<int>(i % width * 8U), top = static_cast<int>(i / width * 8U);
            if (!composed) {
                // Nothing to compose with yet: colour each entry by its tile id
                // so the arrangement is at least legible.
                painter.fillRect(QRect(left, top, 8, 8),
                                 QColor::fromHsv(static_cast<int>((value & 0x3ffU) * 47U % 360U), 150, 220));
                continue;
            }
            // NSCR entry: bits 0-9 character, 10 H-flip, 11 V-flip, 12-15 palette row.
            const std::size_t tile = value & 0x3ffU;
            const bool hflip = (value & 0x400U) != 0, vflip = (value & 0x800U) != 0;
            const std::size_t row = companion_characters_8bpp ? 0U : static_cast<std::size_t>(value >> 12U) * 16U;
            const auto tile_size = companion_characters_8bpp ? 64U : 32U;
            for (std::size_t pixel = 0; pixel < 64U; ++pixel) {
                const auto sx = hflip ? 7U - pixel % 8U : pixel % 8U;
                const auto sy = vflip ? 7U - pixel / 8U : pixel / 8U;
                const auto within = sy * 8U + sx;
                const auto offset = tile * tile_size + (companion_characters_8bpp ? within : within / 2U);
                if (offset >= companion_characters.size()) continue;
                const auto index = companion_characters_8bpp
                    ? companion_characters[offset]
                    : static_cast<std::uint8_t>((companion_characters[offset] >> ((within & 1U) * 4U)) & 15U);
                if (index == 0) continue;
                output.setPixelColor(left + static_cast<int>(pixel % 8U), top + static_cast<int>(pixel / 8U),
                                     color(unpack(palette, row + index)));
            }
        }
        painter.end();
        setPalette(composed ? palette : std::vector<std::uint8_t>{}, false);
        canvas->setImage(output);
        info->setText(QString("%1 — %2 map entries.%3")
                          .arg(mode == Mode::Nscr ? "NSCR" : "Raw screen map")
                          .arg(entries)
                          .arg(composed ? QString() : QString(" Tile colours are identifiers; choose a graphics file to see the picture.")));
    }

    void cells() {
        const auto index = static_cast<std::size_t>(std::max(0, primary->currentIndex()));
        if (!companion_characters.empty() && index < ncer->cells.size()) {
            const auto colours = companion_characters_8bpp ? 256U : 16U;
            const auto packed = activePalette(colours);
            auto decoded = decode_bgr555_palette(packed);
            if (decoded) {
                auto rendered = render_ncer_cell(*ncer, index, companion_characters, decoded.value());
                if (rendered) {
                    setPalette(packed, false);
                    canvas->setImage(image(rendered.value()));
                    info->setText(QString("NCER cell %1 — %2 OAM objects, composed with %3.")
                                      .arg(index)
                                      .arg(ncer->cells[index].oam.size())
                                      .arg(characters_source->currentData().toString()));
                    return;
                }
                info->setText(QString::fromStdString(rendered.error().message));
            }
        }
        setPalette({}, false); QImage output(256, 256, QImage::Format_RGBA8888); output.fill(Qt::transparent); QPainter painter(&output); painter.translate(128, 128);
        if (!ncer->cells.empty()) {
            const auto& cell = ncer->cells[index];
            painter.setPen(QPen(QColor(35, 125, 220), 2));
            for (const auto& oam : cell.oam) {
                int x = oam.attr1 & 0x1ffU; if (x >= 256) x -= 512; int y = oam.attr0 & 0xffU; if (y >= 128) y -= 256;
                static constexpr int dimensions[3][4][2] = {{{8,8},{16,16},{32,32},{64,64}},{{16,8},{32,8},{32,16},{64,32}},{{8,16},{8,32},{16,32},{32,64}}};
                const auto shape = std::min<int>((oam.attr0 >> 14U) & 3U, 2);
                const auto size = (oam.attr1 >> 14U) & 3U;
                painter.drawRect(x, y, dimensions[shape][size][0], dimensions[shape][size][1]);
            }
            info->setText(QString("NCER cell %1 — %2 OAM objects. Outlines show placement; choose a graphics file to draw the cell.").arg(index).arg(cell.oam.size()));
        }
        painter.end();
        canvas->setImage(output);
    }

    void animation() {
        setPalette({}, false); canvas->setImage({});
        if (nanr->sequences.empty()) { info->setText("NANR contains no animation sequences."); return; }
        const auto& sequence = nanr->sequences[static_cast<std::size_t>(std::max(0, primary->currentIndex()))];
        std::size_t duration = 0; for (const auto& frame : sequence.frames) duration += frame.duration;
        info->setText(QString("NANR sequence %1 — %2 frames, %3 ticks, loop starts at frame %4, play mode %5.").arg(primary->currentIndex()).arg(sequence.frames.size()).arg(duration).arg(sequence.loop_start).arg(sequence.play_mode));
    }

    void texture() {
        setPalette({}, false);
        if (tex0->textures.empty()) { canvas->setImage({}); info->setText("TEX0 contains no textures."); return; }
        const auto texture_index = static_cast<std::size_t>(std::max(0, primary->currentIndex()));
        const auto palette_index = static_cast<std::size_t>(std::max(0, secondary->currentIndex()));
        Tex0Palette empty;
        auto& palette = tex0->palettes.empty() ? empty : tex0->palettes[std::min(palette_index, tex0->palettes.size() - 1U)];
        auto rendered = render_tex0_texture(tex0->textures[texture_index], palette);
        if (!rendered) { info->setText(QString::fromStdString(rendered.error().message)); canvas->setImage({}); return; }
        if (!tex0->palettes.empty()) setPalette(palette.colors, true);
        canvas->setImage(image(rendered.value()));
        info->setText(QString("TEX0 texture %1 — %2×%3, format %4. Double-click a palette colour to edit it.").arg(QString::fromStdString(tex0->textures[texture_index].name)).arg(tex0->textures[texture_index].original_width).arg(tex0->textures[texture_index].original_height).arg(static_cast<int>(tex0->textures[texture_index].format)));
    }

    void editColor(QListWidgetItem* item) {
        if (!swatches_editable || !item) return;
        const auto index = static_cast<std::size_t>(swatches->row(item));
        std::vector<std::uint8_t>* bytes = nullptr;
        if (mode == Mode::Nclr) bytes = &nclr->palette;
        else if (mode == Mode::RawPalette) bytes = &body;
        else if (mode == Mode::Tex0 && !tex0->palettes.empty()) bytes = &tex0->palettes[static_cast<std::size_t>(std::max(0, secondary->currentIndex()))].colors;
        if (!bytes) return;
        const auto chosen = QColorDialog::getColor(color(unpack(*bytes, index)), owner, "Choose BGR555 colour");
        if (!chosen.isValid()) return;
        pack(*bytes, index, chosen); refresh();
    }

    void editPixel(int x, int y) {
        if (!character_data || !character_width_tiles) return;
        // A BMP-format sheet is one long row-major bitmap; a CHAR one is 8x8
        // tiles laid out in reading order.
        std::size_t index = 0;
        if (character_linear) {
            index = static_cast<std::size_t>(y) * character_width_tiles * 8U + static_cast<std::size_t>(x);
        } else {
            const auto tile = static_cast<std::size_t>(y / 8) * character_width_tiles + static_cast<std::size_t>(x / 8);
            index = tile * 64U + static_cast<std::size_t>(y % 8) * 8U + static_cast<std::size_t>(x % 8);
        }
        if (character_8bpp) {
            const auto offset = index; if (offset >= character_data->size()) return; (*character_data)[offset] = static_cast<std::uint8_t>(selected_color);
        } else {
            const auto offset = index / 2U; if (offset >= character_data->size()) return;
            const auto shift = static_cast<unsigned>((index & 1U) * 4U);
            const auto mask = static_cast<std::uint8_t>(15U << shift);
            (*character_data)[offset] = static_cast<std::uint8_t>(((*character_data)[offset] & ~mask) | ((selected_color & 15U) << shift));
        }
        refresh();
    }

    nsmb::core::Result<std::vector<std::uint8_t>> output() const {
        if (pages) return pages->output();
        if (error) return *error;
        nsmb::core::Result<std::vector<std::uint8_t>> result = body;
        switch (mode) {
        case Mode::Nclr: result = write_nclr(*nclr); break;
        case Mode::Ncgr: result = write_ncgr(*ncgr); break;
        case Mode::Nscr: result = write_nscr(*nscr); break;
        case Mode::Ncer: result = write_ncer(*ncer); break;
        case Mode::Nanr: result = write_nanr(*nanr); break;
        case Mode::Tex0: result = write_tex0(*tex0); break;
        default: break;
        }
        return result;
    }

    void fail(nsmb::core::Error value) { error = std::move(value); mode = Mode::Unsupported; }

    GraphicsEditorPanel* owner;
    std::string path;
    std::vector<std::uint8_t> body;
    GraphicsFormatKind kind{GraphicsFormatKind::Unknown};
    Mode mode{Mode::Unsupported};
    std::optional<nsmb::core::Error> error;
    std::optional<Nclr> nclr;
    std::optional<Ncgr> ncgr;
    std::optional<Nscr> nscr;
    std::optional<Ncer> ncer;
    std::optional<Nanr> nanr;
    std::optional<Tex0> tex0;
    QLabel* info{};
    QComboBox* primary{};
    QComboBox* secondary{};
    GraphicsCanvas* canvas{};
    QListWidget* swatches{};
    std::vector<std::uint8_t>* character_data{};
    std::size_t character_width_tiles{};
    std::size_t selected_color{};
    bool character_8bpp{};
    bool character_linear{};
    bool swatches_editable{};

    // Files this editor may pair the open one with, and whatever has been
    // paired so far.
    bse::editors::Companions companions;
    QComboBox* palette_source{};
    QComboBox* characters_source{};
    QSpinBox* raw_width{};
    std::vector<std::uint8_t> companion_palette;
    std::vector<std::uint8_t> companion_characters;
    bool companion_characters_8bpp{};
    QString companion_error;
    // Set while restoring remembered choices, so writing them back does not
    // fight the restore.
    bool loading_choices{};
    // Non-null when the file turned out to be a uiStudio resource rather than
    // an NNS G2D one.
    std::unique_ptr<bse::editors::UiResourcePages> pages;
};

GraphicsEditorPanel::GraphicsEditorPanel(std::string_view path, std::span<const std::uint8_t> bytes,
                                         QWidget* parent, bse::editors::Companions companions,
                                         bool rom_user_metadata)
    : bse::editors::EditorPanel(parent),
      impl_(std::make_unique<Impl>(this, path, bytes, std::move(companions), rom_user_metadata)) { impl_->buildUi(); }

GraphicsEditorPanel::~GraphicsEditorPanel() = default;

bool GraphicsEditorPanel::supports(std::span<const std::uint8_t> bytes, std::string_view path,
                                   bool rom_user_metadata) {
    if (bse::editors::UiResourcePages::supports(bytes, path, rom_user_metadata)) return true;
    Impl probe(nullptr, path, bytes);
    return !probe.error;
}

QString GraphicsEditorPanel::title() const {
    return impl_->pages ? impl_->pages->title()
                        : QString("Graphics — %1").arg(QString::fromStdString(impl_->path));
}

bool GraphicsEditorPanel::usable() const { return impl_->pages ? impl_->pages->valid() : !impl_->error; }

nsmb::core::Result<std::vector<std::uint8_t>> GraphicsEditorPanel::editedBytes() const { return impl_->output(); }

GraphicsEditorDialog::GraphicsEditorDialog(std::string_view path, std::span<const std::uint8_t> bytes,
                                           QWidget* parent, bse::editors::Companions companions,
                                           bool rom_user_metadata)
    : QDialog(parent) {
    panel_ = new GraphicsEditorPanel(path, bytes, this, std::move(companions), rom_user_metadata);
    setWindowTitle(panel_->title());
    resize(940, 700);
    auto* root = new QVBoxLayout(this);
    root->addWidget(panel_, 1);
    auto* buttons = new QDialogButtonBox(
        panel_->usable() ? (QDialogButtonBox::Save | QDialogButtonBox::Cancel) : QDialogButtonBox::Cancel, this);
    root->addWidget(buttons);
    connect(buttons, &QDialogButtonBox::accepted, this, &QDialog::accept);
    connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);
}

bool GraphicsEditorDialog::supports(std::span<const std::uint8_t> bytes, std::string_view path,
                                    bool rom_user_metadata) {
    return GraphicsEditorPanel::supports(bytes, path, rom_user_metadata);
}

nsmb::core::Result<std::vector<std::uint8_t>> GraphicsEditorDialog::editedBytes() const { return panel_->editedBytes(); }
