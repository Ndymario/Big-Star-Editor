#pragma once

#include <QWidget>

#include "FileEditors.hpp"

#include "nsmb/project/filesystem_view.hpp"

class QComboBox;
class QLabel;
class QMenu;
class QPoint;
class QPushButton;
class QTreeWidget;

// NSMB keeps its ROM user metadata in NitroFS file 131; that one file needs the metadata
// reader rather than the uiStudio one, so the ID travels with the dispatch call.
inline constexpr std::uint32_t rom_user_metadata_id = 131;

class FilesystemDock final : public QWidget {
    Q_OBJECT

public:
    explicit FilesystemDock(QWidget* parent = nullptr);

    void setData(std::filesystem::path projectDir,
                 nsmb::patching::ModuleGraph modules,
                 nsmb::project::FilesystemView view,
                 QStringList variants,
                 QString activeModule,
                 QString activeVariant,
                 std::vector<nsmb::project::ModuleVariantMapping> mappings = {});
    [[nodiscard]] QString activeModule() const;
    [[nodiscard]] QString activeVariant() const;

signals:
    void activeTargetChanged(const QString& module, const QString& variant);
    void filesChanged();
    void runPatcherRequested();
    void errorOccurred(const QString& message);

protected:
    void changeEvent(QEvent* event) override;

private:
    void applyThemeColors();
    void rebuildTree();
    void showColumnMenu(const QPoint& position);
    void saveColumnState();
    void restoreColumnState();
    void updateDetails();
    void addFile();
    void replaceFile();
    void removeFile();
    void extractFile();
    void hexEditFile();
    void openFile();
    [[nodiscard]] bse::editors::Kind editorFor(const nsmb::project::FilesystemEntry* entry) const;
    [[nodiscard]] const nsmb::project::FilesystemEntry* selectedEntry() const;
    [[nodiscard]] const nsmb::patching::ModuleInfo* targetModule(const nsmb::project::FilesystemEntry* entry = nullptr) const;
    [[nodiscard]] nsmb::core::Result<std::filesystem::path> targetPath(std::string_view romPath,
                                                                      const nsmb::project::FilesystemEntry* entry = nullptr) const;
    nsmb::core::Result<void> writeTarget(std::string_view romPath,
                                         std::span<const std::uint8_t> bytes,
                                         const nsmb::project::FilesystemEntry* entry = nullptr);

    std::filesystem::path projectDir_;
    nsmb::patching::ModuleGraph modules_;
    nsmb::project::FilesystemView view_;
    std::vector<nsmb::project::ModuleVariantMapping> mappings_;
    QComboBox* module_{};
    QComboBox* variant_{};
    QLabel* pending_{};
    QTreeWidget* tree_{};
    QMenu* columnMenu_{};
    QLabel* details_{};
    QPushButton* add_{};
    QPushButton* replace_{};
    QPushButton* remove_{};
    QPushButton* extract_{};
    QPushButton* hex_{};
    QPushButton* open_{};
    bse::editors::Kind selectedEditor_{bse::editors::Kind::None};
    QPushButton* run_{};
};
