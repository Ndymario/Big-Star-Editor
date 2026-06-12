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
    preview_ = new LevelPreviewWidget(this);
    setCentralWidget(preview_);

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
    levelTree_->setRootIsDecorated(true);
    levelTree_->setAlternatingRowColors(true);
    levelTree_->setSelectionBehavior(QAbstractItemView::SelectRows);
    levelTree_->setSelectionMode(QAbstractItemView::SingleSelection);
    levelTree_->header()->setStretchLastSection(true);
    levelTree_->header()->setSectionResizeMode(0, QHeaderView::ResizeToContents);
    levelTree_->header()->setSectionResizeMode(1, QHeaderView::ResizeToContents);
    levelDock->setWidget(levelTree_);
    addDockWidget(Qt::LeftDockWidgetArea, levelDock);
    levelTree_->expandToDepth(1);

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
    if (levelTree_) {
        levelTree_->expandToDepth(1);
    }
    setWindowTitle(projectTitle(currentProject_));
    updateProjectSummary();
}

void MainWindow::updateProjectSummary() {
    if (!preview_ || !propertiesLabel_) {
        return;
    }

    preview_->showProjectSummary(currentProject_, projectModel_->levelCount(), projectModel_->availableLevelCount());

    QString text;
    text += "Project: " + QString::fromStdString(currentProject_.name()) + "\n";
    const auto manifest = manifestPathText(currentProject_);
    if (!manifest.isEmpty()) {
        text += "Manifest: " + manifest + "\n";
    }
    text += "Files: " + QString::number(static_cast<qulonglong>(currentProject_.game_files().size())) + "\n";
    text += "Catalog areas: " + QString::number(projectModel_->levelCount()) + "\n";
    text += "Available areas: " + QString::number(projectModel_->availableLevelCount());
    propertiesLabel_->setText(text);
}

void MainWindow::updateSelectedLevel(const QModelIndex& index) {
    if (!index.isValid()) {
        updateProjectSummary();
        return;
    }

    const auto level_index = index.sibling(index.row(), 0);
    const auto kind = projectModel_->data(level_index, ProjectTreeModel::NodeKindRole).toInt();
    if (kind != ProjectTreeModel::AreaNode && kind != ProjectTreeModel::OtherNode) {
        updateProjectSummary();
        return;
    }

    const auto name = projectModel_->data(level_index, Qt::DisplayRole).toString();
    const auto world = projectModel_->data(level_index, ProjectTreeModel::WorldRole).toString();
    const auto area_name = projectModel_->data(level_index, ProjectTreeModel::AreaNameRole).toString();
    const auto course_path = projectModel_->data(level_index, ProjectTreeModel::CoursePathRole).toString();
    const auto bgdat_path = projectModel_->data(level_index, ProjectTreeModel::BgdatPathRole).toString();
    const auto source_token = projectModel_->data(level_index, ProjectTreeModel::SourceTokenRole).toString();
    const auto size = projectModel_->data(level_index, ProjectTreeModel::SizeRole).toULongLong();
    const auto available = projectModel_->data(level_index, ProjectTreeModel::AvailableRole).toBool();

    if (preview_) {
        preview_->showLevel(currentProject_, name, world, area_name, course_path, bgdat_path, source_token);
    }
    if (propertiesLabel_) {
        QString text;
        text += "Level: " + name + "\n";
        if (!world.isEmpty()) {
            text += "World: " + world + "\n";
        }
        if (!area_name.isEmpty() && area_name != name) {
            text += "Area: " + area_name + "\n";
        }
        if (!source_token.isEmpty()) {
            text += "Token: " + source_token + "\n";
        }
        text += "Course: " + (course_path.isEmpty() ? QString("missing") : course_path) + "\n";
        text += "BGDat: " + (bgdat_path.isEmpty() ? QString("missing") : bgdat_path) + "\n";
        text += "Available: " + QString(available ? "yes" : "no") + "\n";
        text += "Size: " + QString::number(size) + " bytes";
        propertiesLabel_->setText(text);
    }
}

void MainWindow::log(const QString& message) {
    if (logOutput_) {
        logOutput_->append(message);
    }
}
