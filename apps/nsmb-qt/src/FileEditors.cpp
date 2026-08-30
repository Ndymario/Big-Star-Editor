#include "FileEditors.hpp"

#include "GraphicsEditorDialog.hpp"
#include "MessageEditorDialog.hpp"

#include "nsmb/formats/graphics.hpp"

#include <QDialog>
#include <QPlainTextEdit>
#include <QVBoxLayout>

namespace bse::editors {
namespace {

// NitroFS stores plenty of files behind an LZ wrapper regardless of what they hold, so the
// wrapper is stripped here rather than in each editor: every editor sees plain bytes, and
// whatever it hands back goes to disk compressed exactly the way it arrived.
struct Wrapper {
    std::uint8_t compression{};
    std::vector<std::uint8_t> body;
};

Wrapper unwrap(std::span<const std::uint8_t> bytes) {
    Wrapper wrapper{0, std::vector<std::uint8_t>(bytes.begin(), bytes.end())};
    if (bytes.empty() || (bytes[0] != 0x10 && bytes[0] != 0x11)) return wrapper;
    auto decoded = nsmb::formats::decompress_nitro_lz(bytes);
    // A file that only looks compressed stays exactly as it was read.
    if (!decoded) return wrapper;
    wrapper.compression = bytes[0];
    wrapper.body = std::move(decoded.value());
    return wrapper;
}

nsmb::core::Result<std::vector<std::uint8_t>> rewrap(std::uint8_t compression, std::vector<std::uint8_t> body) {
    if (!compression) return body;
    return compression == 0x10 ? nsmb::formats::compress_nitro_lz10(body) : nsmb::formats::compress_nitro_lz11(body);
}

} // namespace

std::vector<std::uint8_t> unwrapped(std::span<const std::uint8_t> bytes) { return unwrap(bytes).body; }

nsmb::core::Result<std::vector<std::uint8_t>> rewrapped(std::span<const std::uint8_t> original,
                                                        std::vector<std::uint8_t> edited) {
    return rewrap(unwrap(original).compression, std::move(edited));
}

HexEditorPanel::HexEditorPanel(std::string_view path, std::span<const std::uint8_t> bytes, QWidget* parent)
    : EditorPanel(parent), path_(QString::fromUtf8(path.data(), static_cast<qsizetype>(path.size()))) {
    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    editor_ = new QPlainTextEdit(this);
    editor_->setObjectName("HexEditor");
    editor_->setLineWrapMode(QPlainTextEdit::NoWrap);
    editor_->setPlainText(QByteArray(reinterpret_cast<const char*>(bytes.data()),
                                     static_cast<qsizetype>(bytes.size())).toHex(' ').toUpper());
    layout->addWidget(editor_);
}

QString HexEditorPanel::title() const { return "Hex — " + path_; }

nsmb::core::Result<std::vector<std::uint8_t>> HexEditorPanel::editedBytes() const {
    auto text = editor_->toPlainText();
    text.remove(' '); text.remove('\t'); text.remove('\r'); text.remove('\n');
    const auto invalid = [] {
        return nsmb::core::make_error(nsmb::core::ErrorCode::InvalidArgument,
                                      "hex data contains an invalid or incomplete byte");
    };
    if (text.size() % 2 != 0) return invalid();
    for (const auto character : text)
        if (!character.isDigit() && (character.toLower() < 'a' || character.toLower() > 'f')) return invalid();
    const auto data = QByteArray::fromHex(text.toLatin1());
    return std::vector<std::uint8_t>(data.begin(), data.end());
}

EditorPanel* makePanel(QWidget* parent, std::string_view path, std::span<const std::uint8_t> bytes,
                       bool romUserMetadata, Companions companions) {
    const auto plain = unwrap(bytes);
    switch (classify(bytes, path, romUserMetadata)) {
    case Kind::Messages: return new MessageEditorPanel(path, plain.body, parent);
    case Kind::Graphics:
        return new GraphicsEditorPanel(path, plain.body, parent, std::move(companions), romUserMetadata);
    case Kind::None: break;
    }
    return nullptr;
}

Kind classify(std::span<const std::uint8_t> bytes, std::string_view path, bool romUserMetadata) {
    const auto plain = unwrap(bytes);
    // Ordered by how certain each editor's claim is. BMG is settled by a magic; the graphics
    // editor also accepts raw files on a name hint alone, so it goes last and never steals a
    // file the message editor already recognised for certain.
    if (MessageEditorDialog::supports(plain.body, path)) return Kind::Messages;
    if (GraphicsEditorDialog::supports(plain.body, path, romUserMetadata)) return Kind::Graphics;
    return Kind::None;
}

QString editorName(Kind kind) {
    switch (kind) {
    case Kind::Messages: return "Messages";
    case Kind::Graphics: return "Graphics";
    case Kind::None: break;
    }
    return "None";
}

Outcome edit(QWidget* parent, std::string_view path, std::span<const std::uint8_t> bytes, bool romUserMetadata,
             Companions companions) {
    const auto plain = unwrap(bytes);
    const auto open = [&](QDialog& dialog, auto&& edited) {
        Outcome outcome;
        if (dialog.exec() != QDialog::Accepted) { outcome.status = Outcome::Status::Cancelled; return outcome; }
        auto result = edited();
        if (result) result = rewrap(plain.compression, std::move(result.value()));
        if (!result) {
            outcome.status = Outcome::Status::Failed;
            outcome.error = QString::fromStdString(result.error().message);
            return outcome;
        }
        outcome.status = Outcome::Status::Edited;
        outcome.bytes = std::move(result.value());
        return outcome;
    };

    switch (classify(bytes, path, romUserMetadata)) {
    case Kind::Messages: {
        MessageEditorDialog dialog(path, plain.body, parent);
        return open(dialog, [&] { return dialog.editedBytes(); });
    }
    case Kind::Graphics: {
        GraphicsEditorDialog dialog(path, plain.body, parent, std::move(companions), romUserMetadata);
        return open(dialog, [&] { return dialog.editedBytes(); });
    }
    case Kind::None: break;
    }
    return {};
}

} // namespace bse::editors
