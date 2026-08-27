#include "FileEditors.hpp"

#include "GraphicsEditorDialog.hpp"
#include "MessageEditorDialog.hpp"
#include "UiResourceEditorDialog.hpp"

#include "nsmb/formats/graphics.hpp"

#include <QDialog>

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

Kind classify(std::span<const std::uint8_t> bytes, std::string_view path, bool romUserMetadata) {
    const auto plain = unwrap(bytes);
    // Ordered by how certain each editor's claim is. BMG and the uiStudio formats are settled by
    // a magic; the graphics editor also accepts raw files on a name hint alone, so it goes last
    // and never steals a file another editor already recognised for certain.
    if (MessageEditorDialog::supports(plain.body, path)) return Kind::Messages;
    if (UiResourceEditorDialog::supports(plain.body, path, romUserMetadata)) return Kind::UiResource;
    if (GraphicsEditorDialog::supports(plain.body, path)) return Kind::Graphics;
    return Kind::None;
}

QString editorName(Kind kind) {
    switch (kind) {
    case Kind::Messages: return "Messages";
    case Kind::UiResource: return "UI resource";
    case Kind::Graphics: return "Graphics";
    case Kind::None: break;
    }
    return "None";
}

Outcome edit(QWidget* parent, std::string_view path, std::span<const std::uint8_t> bytes, bool romUserMetadata) {
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
    case Kind::UiResource: {
        UiResourceEditorDialog dialog(path, plain.body, romUserMetadata, parent);
        return open(dialog, [&] { return dialog.editedBytes(); });
    }
    case Kind::Graphics: {
        GraphicsEditorDialog dialog(path, plain.body, parent);
        return open(dialog, [&] { return dialog.editedBytes(); });
    }
    case Kind::None: break;
    }
    return {};
}

} // namespace bse::editors
