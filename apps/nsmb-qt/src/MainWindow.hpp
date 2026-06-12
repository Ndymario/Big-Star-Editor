#pragma once
#include <QMainWindow>
#include <QTextEdit>
#include <QTreeView>
#include "ProjectTreeModel.hpp"
class MainWindow final : public QMainWindow {
public:
    MainWindow();
private:
    void createMenus();
    void createDocks();
    void log(const QString& message);
    ProjectTreeModel* projectModel_{};
    QTextEdit* logOutput_{};
};
