#pragma once

#include <QDialog>

#include <cstdint>
#include <memory>
#include <span>
#include <string_view>
#include <vector>

#include "FileEditors.hpp"
#include "nsmb/core/result.hpp"

// The BMG message editor as a widget the document workspace can host in a tab.
class MessageEditorPanel final : public bse::editors::EditorPanel {
public:
    MessageEditorPanel(std::string_view path,
                       std::span<const std::uint8_t> bytes,
                       QWidget* parent = nullptr);
    ~MessageEditorPanel() override;

    [[nodiscard]] static bool supports(std::span<const std::uint8_t> bytes,
                                       std::string_view path = {});
    [[nodiscard]] QString title() const override;
    [[nodiscard]] bool usable() const override;
    [[nodiscard]] nsmb::core::Result<std::vector<std::uint8_t>> editedBytes() const override;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

// The same editor in a modal frame, for the callers that still want one.
class MessageEditorDialog final : public QDialog {
public:
    MessageEditorDialog(std::string_view path,
                        std::span<const std::uint8_t> bytes,
                        QWidget* parent = nullptr);

    [[nodiscard]] static bool supports(std::span<const std::uint8_t> bytes,
                                       std::string_view path = {});
    [[nodiscard]] nsmb::core::Result<std::vector<std::uint8_t>> editedBytes() const;

private:
    MessageEditorPanel* panel_{};
};
