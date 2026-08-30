#pragma once

#include <memory>
#include <span>
#include <string_view>
#include <vector>

#include <QDialog>

#include "nsmb/core/result.hpp"

class QVBoxLayout;

namespace bse::editors {

// The BNBL/BNCL/BNCD/ENPG/banner/ROM-metadata surfaces, built into whatever
// window opened the file. They are pages of the one graphics editor rather than
// a second editor of their own.
class UiResourcePages {
public:
    UiResourcePages(QWidget* owner,
                    std::string_view path,
                    std::span<const std::uint8_t> bytes,
                    bool rom_user_metadata);
    ~UiResourcePages();

    [[nodiscard]] static bool supports(std::span<const std::uint8_t> bytes,
                                       std::string_view path = {},
                                       bool rom_user_metadata = false);
    [[nodiscard]] bool valid() const;
    [[nodiscard]] QString title() const;
    void build(QVBoxLayout* root);
    [[nodiscard]] nsmb::core::Result<std::vector<std::uint8_t>> output() const;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace bse::editors

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
    std::unique_ptr<bse::editors::UiResourcePages> pages_;
};
