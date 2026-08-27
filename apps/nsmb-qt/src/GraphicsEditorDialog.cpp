#include "GraphicsEditorDialog.hpp"

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
#include <QVBoxLayout>

#include <algorithm>
#include <cmath>
#include <functional>
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

class PixelCanvas final : public QWidget {
public:
    explicit PixelCanvas(QWidget* parent = nullptr) : QWidget(parent) {
        setObjectName("GraphicsCanvas");
        setCursor(Qt::CrossCursor);
    }

    void setImage(QImage value, bool editable = false) {
        image_ = std::move(value); editable_ = editable;
        const auto longest = std::max(image_.width(), image_.height());
        zoom_ = std::clamp(longest ? 512 / longest : 1, 1, 16);
        setFixedSize(std::max(1, image_.width() * zoom_), std::max(1, image_.height() * zoom_));
        update();
    }

    std::function<void(int, int)> clicked;

protected:
    void paintEvent(QPaintEvent*) override {
        QPainter painter(this);
        painter.fillRect(rect(), palette().brush(QPalette::Base));
        painter.setRenderHint(QPainter::SmoothPixmapTransform, false);
        if (!image_.isNull()) painter.drawImage(rect(), image_);
        if (editable_ && zoom_ >= 8) {
            painter.setPen(QColor(0, 0, 0, 45));
            for (int x = 0; x <= image_.width(); ++x) painter.drawLine(x * zoom_, 0, x * zoom_, height());
            for (int y = 0; y <= image_.height(); ++y) painter.drawLine(0, y * zoom_, width(), y * zoom_);
        }
    }

    void mousePressEvent(QMouseEvent* event) override {
        if (editable_ && clicked && !image_.isNull()) {
            const auto x = static_cast<int>(event->position().x()) / zoom_;
            const auto y = static_cast<int>(event->position().y()) / zoom_;
            if (x >= 0 && y >= 0 && x < image_.width() && y < image_.height()) clicked(x, y);
        }
    }

private:
    QImage image_;
    int zoom_{1};
    bool editable_{};
};

std::size_t sheet_width(std::size_t tile_count, std::size_t declared) {
    if (declared && declared != 0xffffU && declared <= tile_count) return declared;
    return std::max<std::size_t>(1, static_cast<std::size_t>(std::ceil(std::sqrt(static_cast<double>(tile_count)))));
}

} // namespace

struct GraphicsEditorDialog::Impl {
    enum class Mode { Unsupported, Nclr, Ncgr, Nscr, Ncer, Nanr, Tex0, RawPalette, RawCharacters, RawScreen };

    explicit Impl(GraphicsEditorDialog* owner, std::string_view path, std::span<const std::uint8_t> source)
        : owner(owner), path(path), body(source.begin(), source.end()) {
        if (!body.empty() && (body[0] == 0x10 || body[0] == 0x11)) {
            compression = body[0];
            auto decoded = decompress_nitro_lz(body);
            if (!decoded) { error = decoded.error(); return; }
            body = std::move(decoded.value());
        }
        kind = identify_graphics_format(body, path);
        parse();
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
        owner->setWindowTitle(QString("Graphics — %1").arg(QString::fromStdString(path)));
        owner->resize(900, 650);
        auto* root = new QVBoxLayout(owner);
        info = new QLabel(owner); info->setObjectName("GraphicsInfo"); info->setWordWrap(true); root->addWidget(info);
        auto* form = new QFormLayout;
        primary = new QComboBox(owner); primary->setObjectName("GraphicsPrimarySelector");
        secondary = new QComboBox(owner); secondary->setObjectName("GraphicsPaletteSelector");
        form->addRow("Image/cell", primary); form->addRow("Palette", secondary); root->addLayout(form);
        auto* scroll = new QScrollArea(owner); scroll->setWidgetResizable(false); scroll->setAlignment(Qt::AlignCenter);
        canvas = new PixelCanvas(scroll); scroll->setWidget(canvas); root->addWidget(scroll, 1);
        swatches = new QListWidget(owner); swatches->setObjectName("GraphicsPalette"); swatches->setViewMode(QListView::IconMode);
        swatches->setFlow(QListView::LeftToRight); swatches->setWrapping(true); swatches->setMaximumHeight(120);
        swatches->setIconSize(QSize(22, 22)); swatches->setGridSize(QSize(48, 42)); root->addWidget(swatches);
        buttons = new QDialogButtonBox(QDialogButtonBox::Save | QDialogButtonBox::Cancel, owner); root->addWidget(buttons);
        QObject::connect(buttons, &QDialogButtonBox::accepted, owner, &QDialog::accept);
        QObject::connect(buttons, &QDialogButtonBox::rejected, owner, &QDialog::reject);
        QObject::connect(primary, &QComboBox::currentIndexChanged, owner, [this] { refresh(); });
        QObject::connect(secondary, &QComboBox::currentIndexChanged, owner, [this] { refresh(); });
        QObject::connect(swatches, &QListWidget::currentRowChanged, owner, [this](int row) { if (row >= 0) selected_color = static_cast<std::size_t>(row); });
        QObject::connect(swatches, &QListWidget::itemDoubleClicked, owner, [this](QListWidgetItem* item) { editColor(item); });
        canvas->clicked = [this](int x, int y) { editPixel(x, y); };

        populateSelectors();
        if (error) {
            info->setText(QString::fromStdString(error->message));
            buttons->button(QDialogButtonBox::Save)->setEnabled(false);
        } else refresh();
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
        case Mode::Ncgr: characters(ncgr->characters, ncgr->width, ncgr->pixel_format == 4U); break;
        case Mode::RawCharacters: characters(body, 0, primary->currentIndex() == 1); break;
        case Mode::Nscr: screen(nscr->screen, nscr->width, nscr->height); break;
        case Mode::RawScreen: screen(body, 0, 0); break;
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

    void characters(std::vector<std::uint8_t>& bytes, std::size_t declared_width, bool eight_bpp) {
        character_data = &bytes; character_8bpp = eight_bpp;
        const std::size_t tile_size = eight_bpp ? 64U : 32U;
        const auto tiles = bytes.size() / tile_size;
        character_width_tiles = sheet_width(tiles, declared_width);
        const auto rows = (tiles + character_width_tiles - 1U) / character_width_tiles;
        QImage output(static_cast<int>(character_width_tiles * 8U), static_cast<int>(rows * 8U), QImage::Format_RGBA8888); output.fill(Qt::transparent);
        const auto colours = eight_bpp ? 256U : 16U;
        for (std::size_t tile = 0; tile < tiles; ++tile) for (std::size_t pixel = 0; pixel < 64U; ++pixel) {
            const auto value = eight_bpp ? bytes[tile * 64U + pixel] : static_cast<std::uint8_t>((bytes[tile * 32U + pixel / 2U] >> ((pixel & 1U) * 4U)) & 15U);
            const auto shade = static_cast<int>(value * 255U / (colours - 1U));
            output.setPixelColor(static_cast<int>((tile % character_width_tiles) * 8U + pixel % 8U), static_cast<int>((tile / character_width_tiles) * 8U + pixel / 8U), QColor(shade, shade, shade));
        }
        std::vector<std::uint8_t> gray(colours * 2U);
        for (std::size_t i = 0; i < colours; ++i) pack(gray, i, QColor(static_cast<int>(i * 255U / (colours - 1U)), static_cast<int>(i * 255U / (colours - 1U)), static_cast<int>(i * 255U / (colours - 1U))));
        setPalette(gray, false); canvas->setImage(output, true);
        info->setText(QString("%1 — %2 tiles, %3bpp. Choose an index, then click pixels to paint.").arg(mode == Mode::Ncgr ? "NCGR" : "Raw graphics").arg(tiles).arg(eight_bpp ? 8 : 4));
    }

    void screen(std::span<const std::uint8_t> bytes, std::size_t declared_width, std::size_t declared_height) {
        const auto entries = bytes.size() / 2U;
        auto width = declared_width >= 8U ? declared_width / 8U : sheet_width(entries, 0);
        if (!width) width = 1;
        const auto height = declared_height >= 8U ? declared_height / 8U : (entries + width - 1U) / width;
        QImage output(static_cast<int>(width * 8U), static_cast<int>(height * 8U), QImage::Format_RGBA8888); output.fill(Qt::transparent);
        QPainter painter(&output);
        for (std::size_t i = 0; i < entries; ++i) {
            const auto value = static_cast<std::uint16_t>(bytes[i * 2U] | (bytes[i * 2U + 1U] << 8U));
            const QColor fill = QColor::fromHsv(static_cast<int>((value & 0x3ffU) * 47U % 360U), 150, 220);
            painter.fillRect(QRect(static_cast<int>(i % width * 8U), static_cast<int>(i / width * 8U), 8, 8), fill);
        }
        setPalette({}, false); canvas->setImage(output); info->setText(QString("%1 — %2 map entries (tile colours are identifiers; load the NCGR/NCER resource to view pixels).").arg(mode == Mode::Nscr ? "NSCR" : "Raw screen map").arg(entries));
    }

    void cells() {
        setPalette({}, false); QImage output(256, 256, QImage::Format_RGBA8888); output.fill(Qt::transparent); QPainter painter(&output); painter.translate(128, 128);
        if (!ncer->cells.empty()) {
            const auto& cell = ncer->cells[static_cast<std::size_t>(std::max(0, primary->currentIndex()))];
            painter.setPen(QPen(QColor(35, 125, 220), 2));
            for (const auto& oam : cell.oam) {
                int x = oam.attr1 & 0x1ffU; if (x >= 256) x -= 512; int y = oam.attr0 & 0xffU; if (y >= 128) y -= 256;
                static constexpr int dimensions[3][4][2] = {{{8,8},{16,16},{32,32},{64,64}},{{16,8},{32,8},{32,16},{64,32}},{{8,16},{8,32},{16,32},{32,64}}};
                const auto shape = std::min<int>((oam.attr0 >> 14U) & 3U, 2);
                const auto size = (oam.attr1 >> 14U) & 3U;
                painter.drawRect(x, y, dimensions[shape][size][0], dimensions[shape][size][1]);
            }
            info->setText(QString("NCER cell %1 — %2 OAM objects. Outlines show placement; actor previews combine these cells with their character and palette resources.").arg(primary->currentIndex()).arg(cell.oam.size()));
        }
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
        const auto tile = static_cast<std::size_t>(y / 8) * character_width_tiles + static_cast<std::size_t>(x / 8);
        const auto pixel = static_cast<std::size_t>(y % 8) * 8U + static_cast<std::size_t>(x % 8);
        if (character_8bpp) {
            const auto offset = tile * 64U + pixel; if (offset >= character_data->size()) return; (*character_data)[offset] = static_cast<std::uint8_t>(selected_color);
        } else {
            const auto offset = tile * 32U + pixel / 2U; if (offset >= character_data->size()) return;
            const auto shift = static_cast<unsigned>((pixel & 1U) * 4U);
            const auto mask = static_cast<std::uint8_t>(15U << shift);
            (*character_data)[offset] = static_cast<std::uint8_t>(((*character_data)[offset] & ~mask) | ((selected_color & 15U) << shift));
        }
        refresh();
    }

    nsmb::core::Result<std::vector<std::uint8_t>> output() const {
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
        if (!result || !compression) return result;
        return compression == 0x10 ? compress_nitro_lz10(result.value()) : compress_nitro_lz11(result.value());
    }

    void fail(nsmb::core::Error value) { error = std::move(value); mode = Mode::Unsupported; }

    GraphicsEditorDialog* owner;
    std::string path;
    std::vector<std::uint8_t> body;
    std::uint8_t compression{};
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
    PixelCanvas* canvas{};
    QListWidget* swatches{};
    QDialogButtonBox* buttons{};
    std::vector<std::uint8_t>* character_data{};
    std::size_t character_width_tiles{};
    std::size_t selected_color{};
    bool character_8bpp{};
    bool swatches_editable{};
};

GraphicsEditorDialog::GraphicsEditorDialog(std::string_view path, std::span<const std::uint8_t> bytes, QWidget* parent)
    : QDialog(parent), impl_(std::make_unique<Impl>(this, path, bytes)) { impl_->buildUi(); }

GraphicsEditorDialog::~GraphicsEditorDialog() = default;

bool GraphicsEditorDialog::supports(std::span<const std::uint8_t> bytes, std::string_view path) {
    Impl probe(nullptr, path, bytes);
    return !probe.error;
}

nsmb::core::Result<std::vector<std::uint8_t>> GraphicsEditorDialog::editedBytes() const { return impl_->output(); }
