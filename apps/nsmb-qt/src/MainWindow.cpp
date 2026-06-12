#include "MainWindow.hpp"
#include "nsmb/project/manifest.hpp"
#include <QDockWidget>
#include <QFileDialog>
#include <QLabel>
#include <QMenuBar>
#include <QMessageBox>
MainWindow::MainWindow() { setWindowTitle("Big Star Editor"); resize(1200, 800); projectModel_ = new ProjectTreeModel(this); auto project = nsmb::project::create_empty_project("Empty Project", {}).value(); projectModel_->setProject(project); setCentralWidget(new QLabel("Placeholder level editor canvas", this)); createMenus(); createDocks(); }
void MainWindow::createMenus() { auto* file = menuBar()->addMenu("&File"); file->addAction("New Project", this, [this]{ log("New Project selected"); }); file->addAction("Import ROM", this, [this]{ log("Import ROM selected; no ROM data is required for the M0 shell"); }); file->addAction("Open Project", this, [this]{ log("Open Project selected"); }); file->addAction("Save Project", this, [this]{ log("Save Project selected"); }); file->addSeparator(); file->addAction("Run Patcher", this, [this]{ log("Run Patcher selected; backend service is ready for async wiring"); }); file->addAction("Build ROM", this, [this]{ log("Build ROM selected; backend service is ready for async wiring"); }); file->addSeparator(); file->addAction("Exit", this, &QWidget::close); }
void MainWindow::createDocks() { auto* treeDock = new QDockWidget("Project Files", this); auto* tree = new QTreeView(treeDock); tree->setModel(projectModel_); treeDock->setWidget(tree); addDockWidget(Qt::LeftDockWidgetArea, treeDock); auto* props = new QDockWidget("Properties", this); props->setWidget(new QLabel("Properties placeholder", props)); addDockWidget(Qt::RightDockWidgetArea, props); auto* logDock = new QDockWidget("Log", this); logOutput_ = new QTextEdit(logDock); logOutput_->setReadOnly(true); logDock->setWidget(logOutput_); addDockWidget(Qt::BottomDockWidgetArea, logDock); log("Qt shell launched with an empty project."); }
void MainWindow::log(const QString& message) { if (logOutput_) logOutput_->append(message); }
