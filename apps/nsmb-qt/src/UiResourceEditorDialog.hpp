#pragma once

#include <memory>
#include <span>
#include <string_view>
#include <vector>

#include <QDialog>

#include "nsmb/core/result.hpp"

class UiResourceEditorDialog final : public QDialog {
public:
    UiResourceEditorDialog(std::string_view path,
                           std::span<const std::uint8_t> bytes,
                           bool rom_user_metadata = false,
                           QWidget* parent = nullptr);
    ~UiResourceEditorDialog() override;

    [[nodiscard]] static bool supports(std::span<const std::uint8_t> bytes,
                                       std::string_view path = {},
                                       bool rom_user_metadata = false);
    [[nodiscard]] nsmb::core::Result<std::vector<std::uint8_t>> editedBytes() const;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};
