#include "MainWindow.hpp"

#include "nsmb/project/manifest.hpp"

#include <QDockWidget>
#include <QFileDialog>
#include <QHeaderView>
#include <QItemSelectionModel>
#include <QMenuBar>
#include <QMessageBox>

#include <filesystem>

namespace {

QString projectTitle(const nsmb::project::Project& project) {
    if (project.name().empty()) {
        return "Big Star Editor";
    }
    return QString::fromStdString(project.name()) + " - Big Star Editor";
}

QString manifestPathText(const nsmb::project::Project& project) {
    if (project.directory().empty()) {
        return {};
    }
    return QString::fromStdString((project.directory() / nsmb::project::manifest_file_name).string());
}

} // namespace

MainWindow::MainWindow() {
    setWindowTitle("Big Star Editor");
    resize(1200, 800);

    projectModel_ = new ProjectTreeModel(this);
    canvasLabel_ = new QLabel("Open a project to list levels.", this);
    canvasLabel_->setAlignment(Qt::AlignCenter);
    setCentralWidget(canvasLabel_);

    auto project = nsmb::project::create_empty_project("Empty Project", {}).value();
    setProject(project);

    createMenus();
    createDocks();
}

void MainWindow::createMenus() {
    auto* file = menuBar()->addMenu("&File");
    file->addAction("New Project", this, [this] {
        auto project = nsmb::project::create_empty_project("Empty Project", {}).value();
        setProject(project);
        log("Created a new empty project.");
    });
    file->addAction("Import ROM", this, [this] { log("Import ROM is available from the CLI for now."); });
    file->addAction("Open Project...", this, [this] { openProject(); });
    file->addAction("Save Project", this, [this] { saveProject(); });
    file->addSeparator();
    file->addAction("Run Patcher", this, [this] { log("Run Patcher selected; backend service is ready for async wiring."); });
    file->addAction("Build ROM", this, [this] { log("Build ROM selected; backend service is ready for async wiring."); });
    file->addSeparator();
    file->addAction("Exit", this, &QWidget::close);
}

void MainWindow::createDocks() {
    auto* levelDock = new QDockWidget("Levels", this);
    levelTree_ = new QTreeView(levelDock);
    levelTree_->setModel(projectModel_);
    levelTree_->setRootIsDecorated(false);
    levelTree_->setAlternatingRowColors(true);
    levelTree_->setSelectionBehavior(QAbstractItemView::SelectRows);
    levelTree_->setSelectionMode(QAbstractItemView::SingleSelection);
    levelTree_->header()->setStretchLastSection(true);
    levelTree_->header()->setSectionResizeMode(0, QHeaderView::ResizeToContents);
    levelDock->setWidget(levelTree_);
    addDockWidget(Qt::LeftDockWidgetArea, levelDock);

    connect(levelTree_->selectionModel(), &QItemSelectionModel::currentChanged, this, [this](const QModelIndex& current, const QModelIndex&) {
        updateSelectedLevel(current);
    });

    auto* props = new QDockWidget("Properties", this);
    propertiesLabel_ = new QLabel(props);
    propertiesLabel_->setAlignment(Qt::AlignTop | Qt::AlignLeft);
    propertiesLabel_->setTextInteractionFlags(Qt::TextSelectableByMouse);
    propertiesLabel_->setMargin(8);
    props->setWidget(propertiesLabel_);
    addDockWidget(Qt::RightDockWidgetArea, props);

    auto* logDock = new QDockWidget("Log", this);
    logOutput_ = new QTextEdit(logDock);
    logOutput_->setReadOnly(true);
    logDock->setWidget(logOutput_);
    addDockWidget(Qt::BottomDockWidgetArea, logDock);

    updateProjectSummary();
    log("Qt shell launched.");
}

void MainWindow::openProject() {
    const auto directory = QFileDialog::getExistingDirectory(this, "Open Big Star Project");
    if (directory.isEmpty()) {
        return;
    }

    const auto project_dir = std::filesystem::path(directory.toStdString());
    const auto manifest_path = project_dir / nsmb::project::manifest_file_name;
    auto loaded = nsmb::project::load_manifest(manifest_path);
    if (!loaded) {
        QMessageBox::warning(this, "Open Project Failed", QString::fromStdString(loaded.error().message));
        log("Could not open project: " + QString::fromStdString(loaded.error().message));
        return;
    }

    setProject(loaded.value());
    log("Opened project: " + QString::fromStdString(manifest_path.string()));
}

void MainWindow::saveProject() {
    if (currentProject_.directory().empty()) {
        QMessageBox::information(this, "Save Project", "Choose an existing project directory with Open Project before saving.");
        return;
    }

    const auto manifest_path = currentProject_.directory() / nsmb::project::manifest_file_name;
    auto saved = nsmb::project::save_manifest(currentProject_, manifest_path);
    if (!saved) {
        QMessageBox::warning(this, "Save Project Failed", QString::fromStdString(saved.error().message));
        log("Could not save project: " + QString::fromStdString(saved.error().message));
        return;
    }
    currentProject_.mark_clean();
    log("Saved project: " + QString::fromStdString(manifest_path.string()));
}

void MainWindow::setProject(const nsmb::project::Project& project) {
    currentProject_ = project;
    projectModel_->setProject(currentProject_);
    setWindowTitle(projectTitle(currentProject_));
    updateProjectSummary();
}

void MainWindow::updateProjectSummary() {
    if (!canvasLabel_ || !propertiesLabel_) {
        return;
    }

    if (projectModel_->levelCount() == 0) {
        canvasLabel_->setText("No level candidates found in this project.");
    } else {
        canvasLabel_->setText("Select a level from the sidebar.");
    }

    QString text;
    text += "Project: " + QString::fromStdString(currentProject_.name()) + "\n";
    const auto manifest = manifestPathText(currentProject_);
    if (!manifest.isEmpty()) {
        text += "Manifest: " + manifest + "\n";
    }
    text += "Files: " + QString::number(static_cast<qulonglong>(currentProject_.game_files().size())) + "\n";
    text += "Levels: " + QString::number(projectModel_->levelCount());
    propertiesLabel_->setText(text);
}

void MainWindow::updateSelectedLevel(const QModelIndex& index) {
    if (!index.isValid()) {
        updateProjectSummary();
        return;
    }

    const auto level_index = index.sibling(index.row(), 0);
    const auto name = projectModel_->data(level_index, Qt::DisplayRole).toString();
    const auto path = projectModel_->data(level_index, ProjectTreeModel::PathRole).toString();
    const auto size = projectModel_->data(level_index, ProjectTreeModel::SizeRole).toULongLong();

    if (canvasLabel_) {
        canvasLabel_->setText("Level: " + name + "\n" + path + "\n\nRead-only level preview is not implemented yet.");
    }
    if (propertiesLabel_) {
        propertiesLabel_->setText("Level: " + name + "\nPath: " + path + "\nSize: " + QString::number(size) + " bytes");
    }
}

void MainWindow::log(const QString& message) {
    if (logOutput_) {
        logOutput_->append(message);
    }
}
