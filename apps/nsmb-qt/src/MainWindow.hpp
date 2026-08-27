#pragma once

#include <QLabel>
#include <QMainWindow>
#include <QPointer>
#include <QSet>
#include <QStringList>
#include <QTextEdit>
#include <QTreeView>

#include <filesystem>
#include <memory>
#include <optional>
#include <string>
#include <unordered_map>

#include "LevelPropertiesWidget.hpp"
#include "LevelDataWidget.hpp"
#include "nsmb/project/module_data.hpp"
#include "nsmb/project/sprite_definitions.hpp"
#include "nsmb/project/filesystem_view.hpp"
#include "LevelPreviewWidget.hpp"
#include "ProjectTreeModel.hpp"
#include "nsmb/project/project.hpp"

class QAction;
class QCloseEvent;
class QDockWidget;
class QFileSystemWatcher;
class QLineEdit;
class QListWidget;
class QToolBox;
class QToolButton;
class QMenu;
class QTabWidget;
class QTreeWidget;
class QTimer;
class QtEditorTests;
class PatcherPanel;
class FilesystemDock;

class MainWindow final : public QMainWindow {
    Q_OBJECT

public:
    MainWindow();
    ~MainWindow() override;

    friend class QtEditorTests;

private:
    void createMenus();
    void createDocks();
    void createWorkspace();
    bool newProject();
    bool importRom();
    void openProject();
    bool saveProject();
    nsmb::core::Result<void> saveEditedLevels();
    void runPatcher();
    bool offerNcpatcherMigration(const std::filesystem::path& legacyConfig);
    void openDiagnosticLocation(const QString& file, int line, int column);
    void onPatcherFinished(bool ok, int exitCode, const QString& stage, int errors, int warnings);
    void buildRom();
    void openTilesetEditor();
    void editRomBanner();
    void editRomUserMetadata();
    void exportLevelPackage();
    void importLevelPackage();
    void editLevelHex();
    void findEntity(bool sprites);
    void showSpriteEvents();
    void exportPatch();
    void importPatch();
    void runXdelta(bool create);
    void runLegacyPatch(bool fireflower);
    void writeRecoverySnapshots();
    void restoreRecoverySnapshots();
    void clearRecoverySnapshots();
    void setProject(const nsmb::project::Project& project);
    void updateProjectSummary();
    void updateSelectedLevel(const QModelIndex& index);
    void openLevelEditor(const QModelIndex& index);
    void showPreview();
    void closeTab(int index);
    void activateCurrentDocument();
    void detachEditor(const std::string& key);
    void reattachEditor(const std::string& key);
    void clearDocuments();
    void saveWorkspaceState();
    void restoreWorkspaceState();
    void resetWorkspaceLayout();
    void configureGraphics(LevelPreviewWidget* view);
    void refreshGraphics();
    void refreshChangedGraphics();
    void refreshPalette();
    void reloadFilesystemView();
    [[nodiscard]] std::vector<nsmb::project::ModuleVariantMapping> activeModuleMappings() const;
    [[nodiscard]] nsmb::core::Result<std::string> romPathForProjectFile(const std::filesystem::path& projectRelative) const;
    [[nodiscard]] nsmb::core::Result<std::filesystem::path> moduleSaveTarget(const std::filesystem::path& projectRelative) const;
    [[nodiscard]] QModelIndex findLevelIndex(const QString& course_path, const QString& bgdat_path) const;
    nsmb::core::Result<nsmb::project::EditableLevel*> levelSession(const QString& course_path, const QString& bgdat_path);
    void undo();
    void redo();
    void updateEditState();
    [[nodiscard]] bool hasUnsavedLevelEdits() const;
    [[nodiscard]] bool confirmDiscardLevelEdits();
    void log(const QString& message);
    [[nodiscard]] LevelPreviewWidget* activeLevelView() const;

protected:
    void closeEvent(QCloseEvent* event) override;

private:
    nsmb::project::Project currentProject_;
    nsmb::project::ModuleDataCatalog moduleData_;
    nsmb::project::SpriteDefinitionCatalog spriteDefinitions_;
    nsmb::patching::ModuleGraph moduleGraph_;
    std::optional<nsmb::patching::ProjectConfig> patcherConfig_;
    std::optional<nsmb::patching::FileManifest> fileManifest_;
    nsmb::project::FilesystemView filesystemView_;
    std::vector<nsmb::project::ProjectVariantLayers> projectVariants_;
    ProjectTreeModel* projectModel_{};
    QTreeView* levelTree_{};
    QTabWidget* tabs_{};
    QWidget* hub_{};
    QTreeView* hubLevelTree_{};
    QTreeWidget* hubFilesystemTree_{};
    LevelPreviewWidget* preview_{};
    LevelPropertiesWidget* levelProperties_{};
    LevelDataWidget* levelData_{};
    QLabel* propertiesLabel_{};
    QTextEdit* logOutput_{};
    QAction* undoAction_{};
    QAction* redoAction_{};
    QAction* previewAction_{};
    QToolButton* undoButton_{};
    QToolButton* redoButton_{};
    QMenu* viewMenu_{};
    QDockWidget* levelsDock_{};
    QDockWidget* propertiesDock_{};
    QDockWidget* paletteDock_{};
    QDockWidget* logDock_{};
    QDockWidget* buildDock_{};
    QDockWidget* filesystemDock_{};
    PatcherPanel* patcherPanel_{};
    FilesystemDock* filesystemPanel_{};
    QString ncpatcherVariant_;
    QString activeModule_;
    QString activeVariant_;
    QLineEdit* paletteSearch_{};
    QToolBox* paletteToolbox_{};
    std::vector<QToolButton*> paletteButtons_;
    QTreeWidget* customPaletteTree_{};
    QFileSystemWatcher* graphicsWatcher_{};
    QTimer* graphicsRefreshTimer_{};
    QFileSystemWatcher* moduleWatcher_{};
    QTimer* moduleRefreshTimer_{};
    QTimer* autosaveTimer_{};
    QSet<QString> changedGraphicsFiles_;
    std::unordered_map<LevelPreviewWidget*, QStringList> graphicsFiles_;
    QByteArray defaultDockState_;
    nsmb::project::EditableLevel* currentLevel_{};
    std::unordered_map<std::string, std::unique_ptr<nsmb::project::EditableLevel>> levelSessions_;

    struct LevelDocument {
        QString name;
        QString world;
        QString area_name;
        QString source_token;
        QString course_path;
        QString bgdat_path;
        LevelPreviewWidget* view{};
        QPointer<QMainWindow> window;
    };
    std::unordered_map<std::string, LevelDocument> documents_;
};
