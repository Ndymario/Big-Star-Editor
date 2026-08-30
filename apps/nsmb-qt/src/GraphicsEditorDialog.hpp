#pragma once

#include <QDialog>

#include <cstdint>
#include <memory>
#include <span>
#include <string_view>
#include <vector>

#include "FileEditors.hpp"
#include "nsmb/core/result.hpp"

// The graphics editor proper: every NNS G2D format plus the uiStudio resources,
// as a widget the document workspace can host in a tab.
class GraphicsEditorPanel final : public bse::editors::EditorPanel {
public:
    GraphicsEditorPanel(std::string_view path,
                        std::span<const std::uint8_t> bytes,
                        QWidget* parent = nullptr,
                        bse::editors::Companions companions = {},
                        bool rom_user_metadata = false);
    ~GraphicsEditorPanel() override;

    [[nodiscard]] static bool supports(std::span<const std::uint8_t> bytes,
                                       std::string_view path = {},
                                       bool rom_user_metadata = false);
    [[nodiscard]] QString title() const override;
    [[nodiscard]] bool usable() const override;
    [[nodiscard]] nsmb::core::Result<std::vector<std::uint8_t>> editedBytes() const override;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

// The same editor in a modal frame, for the callers that still want one.
class GraphicsEditorDialog final : public QDialog {
public:
    GraphicsEditorDialog(std::string_view path,
                         std::span<const std::uint8_t> bytes,
                         QWidget* parent = nullptr,
                         bse::editors::Companions companions = {},
                         bool rom_user_metadata = false);

    [[nodiscard]] static bool supports(std::span<const std::uint8_t> bytes,
                                       std::string_view path = {},
                                       bool rom_user_metadata = false);
    [[nodiscard]] nsmb::core::Result<std::vector<std::uint8_t>> editedBytes() const;

private:
    GraphicsEditorPanel* panel_{};
};
