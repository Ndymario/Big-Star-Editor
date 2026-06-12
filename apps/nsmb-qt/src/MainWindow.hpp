#pragma once

#include <QLabel>
#include <QMainWindow>
#include <QTextEdit>
#include <QTreeView>

#include "ProjectTreeModel.hpp"
#include "nsmb/project/project.hpp"

class MainWindow final : public QMainWindow {
public:
    MainWindow();

private:
    void createMenus();
    void createDocks();
    void openProject();
    void saveProject();
    void setProject(const nsmb::project::Project& project);
    void updateProjectSummary();
    void updateSelectedLevel(const QModelIndex& index);
    void log(const QString& message);

    nsmb::project::Project currentProject_;
    ProjectTreeModel* projectModel_{};
    QTreeView* levelTree_{};
    QLabel* canvasLabel_{};
    QLabel* propertiesLabel_{};
    QTextEdit* logOutput_{};
};
