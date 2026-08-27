#pragma once

#include <QDialog>

#include <cstdint>
#include <memory>
#include <span>
#include <string_view>
#include <vector>

#include "nsmb/core/result.hpp"

class GraphicsEditorDialog final : public QDialog {
public:
    GraphicsEditorDialog(std::string_view path,
                         std::span<const std::uint8_t> bytes,
                         QWidget* parent = nullptr);
    ~GraphicsEditorDialog() override;

    [[nodiscard]] static bool supports(std::span<const std::uint8_t> bytes,
                                       std::string_view path = {});
    [[nodiscard]] nsmb::core::Result<std::vector<std::uint8_t>> editedBytes() const;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};
