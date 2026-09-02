#pragma once

#include <QWidget>

#include "FileEditors.hpp"

#include "nsmb/project/effective_filesystem.hpp"
#include "nsmb/project/filesystem_view.hpp"

class QComboBox;
class QLabel;
class QMenu;
class QPoint;
class QPushButton;
class QTreeWidget;
class QTreeWidgetItem;

// NSMB keeps its ROM user metadata in NitroFS file 131; that one file needs the metadata
// reader rather than the uiStudio one, so the ID travels with the dispatch call.
inline constexpr std::uint32_t rom_user_metadata_id = 131;

class FilesystemDock final : public QWidget {
    Q_OBJECT

public:
    explicit FilesystemDock(QWidget* parent = nullptr);

    // `files` is the window's own reader, borrowed rather than copied: the dock
    // reads bytes to classify, extract and open, and there is one ROM open.
    void setData(std::filesystem::path projectDir,
                 nsmb::project::EffectiveFilesystem* files,
                 nsmb::patching::ModuleGraph modules,
                 nsmb::project::FilesystemView view,
                 QStringList variants,
                 QString activeModule,
                 QString activeVariant,
                 std::vector<nsmb::project::ModuleVariantMapping> mappings = {});
    [[nodiscard]] QString activeModule() const;
    [[nodiscard]] QString activeVariant() const;
    // The files a graphics editor may pair the opened one with: its NitroFS
    // siblings, read through the same module layering the dock itself shows.
    [[nodiscard]] bse::editors::Companions companionsFor(const QString& romPath) const;
    // Selects and scrolls to a ROM path, expanding whatever holds it. False
    // when this variant's tree has no such file.
    bool revealPath(const QString& romPath);
    // What the dock knows about a ROM address, including the archive members it
    // has listed since the view was handed to it -- so a caller asking about a
    // member gets an answer rather than the view's silence.
    [[nodiscard]] const nsmb::project::FilesystemEntry* entryFor(const QString& romPath) const;
    // Writes an editor's result back through the active module target.
    nsmb::core::Result<void> saveEditedFile(const QString& romPath, std::span<const std::uint8_t> bytes);

signals:
    // The window hosts editors in document tabs, so the dock asks rather than
    // opening a modal dialog of its own.
    void openInEditorRequested(const QString& romPath);
    void hexEditRequested(const QString& romPath);
    void activeTargetChanged(const QString& module, const QString& variant);
    void filesChanged();
    void runPatcherRequested();
    void errorOccurred(const QString& message);

protected:
    void changeEvent(QEvent* event) override;

private:
    void applyThemeColors();
    void rebuildTree();
    // Fills in an archive's remaining members the first time it is opened. A
    // plan names only the members a build would write; the rest are the
    // container's own business, and a game has hundreds of containers.
    void listArchive(QTreeWidgetItem* item);
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
    nsmb::project::EffectiveFilesystem* files_{};
    nsmb::patching::ModuleGraph modules_;
    nsmb::project::FilesystemView view_;
    std::vector<nsmb::project::ModuleVariantMapping> mappings_;
    QComboBox* module_{};
    // The variant is chosen once, on the window's toolbar. This dock only needs
    // to know which one is active and what the full list is, so it can say which
    // variants already hold a path it is about to write.
    QStringList variants_;
    QString activeVariant_;
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
