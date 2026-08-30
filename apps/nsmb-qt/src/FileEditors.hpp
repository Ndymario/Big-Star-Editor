#pragma once

#include <QString>
#include <QStringList>
#include <QWidget>

class QPlainTextEdit;

#include <cstdint>
#include <functional>
#include <span>
#include <string_view>
#include <vector>

#include "nsmb/core/result.hpp"


// One entry point for "open this file in whichever editor understands it", so nothing in the
// interface has to ask the user which kind of resource they are about to edit.
namespace bse::editors {

enum class Kind { None, Messages, Graphics };

[[nodiscard]] Kind classify(std::span<const std::uint8_t> bytes,
                            std::string_view path,
                            bool romUserMetadata = false);
[[nodiscard]] QString editorName(Kind kind);

// How an editor reaches files other than the one it was opened on. A palette is
// a separate file from the graphics it colours, and a screen map is a third, so
// an editor handed one file in isolation can only ever show part of a picture.
// The host supplies what it can offer and how to read it; an editor given
// nothing simply does without.
struct Companions {
    // Candidate ROM paths, in the order the host wants them listed.
    QStringList paths;
    // Reads one of those paths and returns its bytes with any LZ wrapper already
    // stripped, so editors never repeat that work.
    std::function<nsmb::core::Result<std::vector<std::uint8_t>>(const QString&)> load;

    [[nodiscard]] bool usable() const { return load && !paths.isEmpty(); }
};

// Strips an LZ10/LZ11 wrapper if there is a real one, so a host can implement
// Companions::load without duplicating the rule.
[[nodiscard]] std::vector<std::uint8_t> unwrapped(std::span<const std::uint8_t> bytes);

// A file editor as a widget, so opening one no longer freezes the shell behind
// a modal dialog: the workspace hosts it in a document tab like a level.
class EditorPanel : public QWidget {
public:
    using QWidget::QWidget;
    [[nodiscard]] virtual QString title() const = 0;
    // False when no editor understood the file; the panel is then empty.
    [[nodiscard]] virtual bool usable() const = 0;
    [[nodiscard]] virtual nsmb::core::Result<std::vector<std::uint8_t>> editedBytes() const = 0;
};

// The last-resort editor: raw bytes, for files nothing else understands.
class HexEditorPanel final : public EditorPanel {
public:
    HexEditorPanel(std::string_view path, std::span<const std::uint8_t> bytes, QWidget* parent = nullptr);

    [[nodiscard]] QString title() const override;
    [[nodiscard]] bool usable() const override { return true; }
    [[nodiscard]] nsmb::core::Result<std::vector<std::uint8_t>> editedBytes() const override;

private:
    QString path_;
    QPlainTextEdit* editor_{};
};

// Builds the panel for whatever the file turns out to be, or nullptr when
// nothing understands it.
[[nodiscard]] EditorPanel* makePanel(QWidget* parent,
                                     std::string_view path,
                                     std::span<const std::uint8_t> bytes,
                                     bool romUserMetadata = false,
                                     Companions companions = {});

// Re-applies whatever LZ wrapper the file was read with, so a panel's bytes go
// back to disk the way they arrived.
[[nodiscard]] nsmb::core::Result<std::vector<std::uint8_t>> rewrapped(
    std::span<const std::uint8_t> original, std::vector<std::uint8_t> edited);

struct Outcome {
    enum class Status { Unsupported, Cancelled, Edited, Failed };
    Status status{Status::Unsupported};
    std::vector<std::uint8_t> bytes;  // only meaningful when status is Edited
    QString error;                    // only meaningful when status is Failed
};

[[nodiscard]] Outcome edit(QWidget* parent,
                           std::string_view path,
                           std::span<const std::uint8_t> bytes,
                           bool romUserMetadata = false,
                           Companions companions = {});

} // namespace bse::editors
