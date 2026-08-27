#pragma once

#include <QString>

#include <cstdint>
#include <span>
#include <string_view>
#include <vector>

class QWidget;

// One entry point for "open this file in whichever editor understands it", so nothing in the
// interface has to ask the user which kind of resource they are about to edit.
namespace bse::editors {

enum class Kind { None, Messages, UiResource, Graphics };

[[nodiscard]] Kind classify(std::span<const std::uint8_t> bytes,
                            std::string_view path,
                            bool romUserMetadata = false);
[[nodiscard]] QString editorName(Kind kind);

struct Outcome {
    enum class Status { Unsupported, Cancelled, Edited, Failed };
    Status status{Status::Unsupported};
    std::vector<std::uint8_t> bytes;  // only meaningful when status is Edited
    QString error;                    // only meaningful when status is Failed
};

[[nodiscard]] Outcome edit(QWidget* parent,
                           std::string_view path,
                           std::span<const std::uint8_t> bytes,
                           bool romUserMetadata = false);

} // namespace bse::editors
