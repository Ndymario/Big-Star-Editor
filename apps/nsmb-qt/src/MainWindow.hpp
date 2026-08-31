#pragma once

#include <QLabel>
#include <QMainWindow>
#include <QPointer>
#include <QSet>
#include <QStringList>
#include <QTextEdit>
#include <QTreeView>

#include <filesystem>
#include <functional>
#include <memory>
#include <optional>
#include <string>
#include <unordered_map>

#include "FileEditors.hpp"
#include "LevelPropertiesWidget.hpp"
#include "LevelDataWidget.hpp"
#include "nsmb/project/module_data.hpp"
#include "nsmb/project/lua_actors.hpp"
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
    bool openProjectPath(const std::filesystem::path& directory);
    void openRecentProject(const QString& directory);
    void rememberRecentProject(const std::filesystem::path& directory);
    void forgetRecentProject(const QString& directory);
    void updateRecentMenu();
    bool saveProject();
    nsmb::core::Result<void> saveEditedLevels();
    void runPatcher();
    bool offerNcpatcherMigration(const std::filesystem::path& legacyConfig);
    void openDiagnosticLocation(const QString& file, int line, int column);
    void onPatcherFinished(bool ok, int exitCode, const QString& stage, int errors, int warnings);
    void buildRom();
    void showLevelTreeMenu(const QPoint& position);
    // Brings the Filesystem tab forward with `romPath` selected.
    void showInFilesystem(const QString& romPath);
    void openFileInEditor();
    // Opens any file the editors understand in a document tab. `save` is how
    // that tab writes its result back, which differs between a NitroFS file and
    // a loose one on disk.
    void openFileDocument(const QString& key,
                          const QString& subtitle,
                          std::vector<std::uint8_t> bytes,
                          bool romUserMetadata,
                          bse::editors::Companions companions,
                          std::function<nsmb::core::Result<void>(const std::vector<std::uint8_t>&)> save);
    [[nodiscard]] bool raiseFileDocument(const QString& key);
    void hostFileDocument(const QString& key,
                          const QString& subtitle,
                          bse::editors::EditorPanel* panel,
                          std::vector<std::uint8_t> bytes,
                          std::function<nsmb::core::Result<void>(const std::vector<std::uint8_t>&)> save,
                          bool rewrap = true);
    void saveFileDocument(const std::string& key);
    void openTilesetEditor();
    void editRomBanner();
    void editRomUserMetadata();
    void editPreferences();
    void exportLevelPackage();
    void importLevelPackage();
    void editLevelHex();
    void findEntity(bool sprites);
    void showSpriteEvents();
    void exportPatch();
    void importPatch();
    void runXdelta(bool create);
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
    void reloadActorDefinitions();
    [[nodiscard]] std::vector<nsmb::project::ModuleVariantMapping> activeModuleMappings() const;
    [[nodiscard]] nsmb::core::Result<std::string> romPathForProjectFile(const std::filesystem::path& projectRelative) const;
    // Where a project-relative file actually lives once the active variant's
    // module layers have had their say, falling back to the extracted tree.
    [[nodiscard]] std::filesystem::path effectiveFile(const std::filesystem::path& projectRelative) const;
    [[nodiscard]] std::string documentKey(const QString& course_path, const QString& bgdat_path) const;
    void setActiveVariant(const QString& variant);
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
    nsmb::project::LuaActorCatalog actorDefinitions_;
    nsmb::patching::ModuleGraph moduleGraph_;
    std::optional<nsmb::patching::ProjectConfig> patcherConfig_;
    std::optional<nsmb::patching::FileManifest> fileManifest_;
    nsmb::project::FilesystemView filesystemView_;
    std::vector<nsmb::project::ProjectVariantLayers> projectVariants_;
    ProjectTreeModel* projectModel_{};
    QTreeView* levelTree_{};
    QTabWidget* tabs_{};
    LevelPreviewWidget* preview_{};
    LevelPropertiesWidget* levelProperties_{};
    LevelDataWidget* levelData_{};
    QLabel* propertiesLabel_{};
    QAction* undoAction_{};
    QAction* redoAction_{};
    QAction* previewAction_{};
    QToolButton* undoButton_{};
    QToolButton* redoButton_{};
    QMenu* viewMenu_{};
    QMenu* recentMenu_{};
    QDockWidget* levelsDock_{};
    QDockWidget* propertiesDock_{};
    QDockWidget* paletteDock_{};
    QDockWidget* buildDock_{};
    QDockWidget* filesystemDock_{};
    PatcherPanel* patcherPanel_{};
    FilesystemDock* filesystemPanel_{};
    QString ncpatcherVariant_;
    QString activeModule_;
    QString activeVariant_;
    class QComboBox* variantSelector_{};
    QLineEdit* paletteSearch_{};
    // One page per loaded tileset slot, the way NSMB Editor splits its object
    // picker; sprites get their own categorised page beside it.
    QTabWidget* objectTabs_{};
    QToolBox* spriteToolbox_{};
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

    // File editors live in the same tab bar as levels rather than behind a
    // modal dialog, so the rest of the shell stays usable while one is open.
    struct FileDocument {
        QString title;
        QWidget* page{};
        bse::editors::EditorPanel* panel{};
        std::vector<std::uint8_t> original;
        // The hex editor shows the file exactly as it sits on disk, wrapper and
        // all, so its bytes must not be compressed again on the way back.
        bool rewrap{true};
        std::function<nsmb::core::Result<void>(const std::vector<std::uint8_t>&)> save;
    };
    std::unordered_map<std::string, FileDocument> fileDocuments_;
};
