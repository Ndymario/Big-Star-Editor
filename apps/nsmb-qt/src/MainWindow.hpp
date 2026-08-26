#pragma once

#include <QLabel>
#include <QMainWindow>
#include <QPointer>
#include <QSet>
#include <QStringList>
#include <QTextEdit>
#include <QTreeView>

#include <memory>
#include <string>
#include <unordered_map>

#include "LevelPropertiesWidget.hpp"
#include "LevelPreviewWidget.hpp"
#include "ProjectTreeModel.hpp"
#include "nsmb/project/project.hpp"

class QAction;
class QCloseEvent;
class QDockWidget;
class QFileSystemWatcher;
class QLineEdit;
class QListWidget;
class QMenu;
class QTabWidget;
class QTimer;
class QtEditorTests;

class MainWindow final : public QMainWindow {
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
    void buildRom();
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
    [[nodiscard]] QModelIndex findLevelIndex(const QString& course_path, const QString& bgdat_path) const;
    nsmb::core::Result<nsmb::project::EditableLevel*> levelSession(const QString& course_path, const QString& bgdat_path);
    void undo();
    void redo();
    void updateEditState();
    [[nodiscard]] bool hasUnsavedLevelEdits() const;
    [[nodiscard]] bool confirmDiscardLevelEdits();
    void log(const QString& message);

protected:
    void closeEvent(QCloseEvent* event) override;

private:
    nsmb::project::Project currentProject_;
    ProjectTreeModel* projectModel_{};
    QTreeView* levelTree_{};
    QTabWidget* tabs_{};
    LevelPreviewWidget* preview_{};
    LevelPropertiesWidget* levelProperties_{};
    QLabel* propertiesLabel_{};
    QTextEdit* logOutput_{};
    QAction* undoAction_{};
    QAction* redoAction_{};
    QAction* previewAction_{};
    QMenu* viewMenu_{};
    QDockWidget* levelsDock_{};
    QDockWidget* propertiesDock_{};
    QDockWidget* paletteDock_{};
    QDockWidget* logDock_{};
    QLineEdit* paletteSearch_{};
    QListWidget* paletteList_{};
    QFileSystemWatcher* graphicsWatcher_{};
    QTimer* graphicsRefreshTimer_{};
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
