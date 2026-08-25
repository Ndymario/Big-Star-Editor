#pragma once

#include <QLabel>
#include <QMainWindow>
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

class MainWindow final : public QMainWindow {
public:
    MainWindow();

private:
    void createMenus();
    void createDocks();
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
    LevelPreviewWidget* preview_{};
    LevelPropertiesWidget* levelProperties_{};
    QLabel* propertiesLabel_{};
    QTextEdit* logOutput_{};
    QAction* undoAction_{};
    QAction* redoAction_{};
    nsmb::project::EditableLevel* currentLevel_{};
    std::unordered_map<std::string, std::unique_ptr<nsmb::project::EditableLevel>> levelSessions_;
};
