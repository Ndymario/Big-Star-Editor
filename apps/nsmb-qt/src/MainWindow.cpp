#include "MainWindow.hpp"

#include "nsmb/nds/rom.hpp"
#include "nsmb/patching/external_tools.hpp"
#include "nsmb/project/manifest.hpp"

#include <QApplication>
#include <QDateTime>
#include <QDockWidget>
#include <QCloseEvent>
#include <QFileDialog>
#include <QHeaderView>
#include <QItemSelectionModel>
#include <QInputDialog>
#include <QKeySequence>
#include <QLineEdit>
#include <QMenuBar>
#include <QMessageBox>
#include <QSaveFile>
#include <QStandardPaths>
#include <QVBoxLayout>

#include <filesystem>
#include <fstream>
#include <iterator>
#include <limits>
#include <span>
#include <vector>

namespace {

QString projectTitle(const nsmb::project::Project& project) {
    if (project.name().empty()) {
        return "Big Star Editor";
    }
    return QString::fromStdString(project.name()) + " - Big Star Editor";
}

nsmb::core::Result<std::vector<std::uint8_t>> readBinaryFile(const std::filesystem::path& path) {
    std::ifstream in(path, std::ios::binary);
    if (!in) {
        return nsmb::core::make_error(nsmb::core::ErrorCode::IoError, "could not open " + path.string());
    }
    return std::vector<std::uint8_t>(std::istreambuf_iterator<char>(in), {});
}

nsmb::core::Result<void> writeBinaryFileAtomically(const std::filesystem::path& path, std::span<const std::uint8_t> data) {
    if (data.size() > static_cast<std::size_t>(std::numeric_limits<qint64>::max())) {
        return nsmb::core::make_error(nsmb::core::ErrorCode::InvalidArgument, "file is too large to write");
    }
    QSaveFile file(QString::fromStdString(path.string()));
    if (!file.open(QIODevice::WriteOnly)) {
        return nsmb::core::make_error(nsmb::core::ErrorCode::IoError, "could not open " + path.string() + " for writing");
    }
    const auto size = static_cast<qint64>(data.size());
    if (file.write(reinterpret_cast<const char*>(data.data()), size) != size || !file.commit()) {
        return nsmb::core::make_error(nsmb::core::ErrorCode::IoError, "could not atomically replace " + path.string());
    }
    return {};
}

void collectProjectFiles(nsmb::project::Project& project,
                         const std::filesystem::path& project_dir,
                         const std::filesystem::path& extracted_dir) {
    std::error_code ec;
    for (const auto& entry : std::filesystem::recursive_directory_iterator(extracted_dir, ec)) {
        if (ec) {
            break;
        }
        if (!entry.is_regular_file(ec)) {
            continue;
        }
        auto relative = nsmb::project::make_project_relative(project_dir, entry.path());
        if (relative) {
            project.add_game_file({relative.value(), entry.file_size(ec)});
        }
    }
}

class WaitCursor final {
public:
    WaitCursor() {
        QApplication::setOverrideCursor(Qt::WaitCursor);
        QApplication::processEvents();
    }
    ~WaitCursor() { QApplication::restoreOverrideCursor(); }
};

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

    createMenus();
    createDocks();

    auto project = nsmb::project::create_empty_project("Empty Project", {}).value();
    setProject(project);
}

void MainWindow::createMenus() {
    auto* bar = menuBar();
    bar->setNativeMenuBar(false);

    auto* file = bar->addMenu("&File");
    file->addAction("New Project...", QKeySequence::New, this, [this] {
        log("File > New Project selected.");
        newProject();
    });
    file->addAction("Import ROM...", this, [this] {
        log("File > Import ROM selected.");
        importRom();
    });
    file->addAction("Open Project...", QKeySequence::Open, this, [this] {
        log("File > Open Project selected.");
        openProject();
    });
    file->addAction("Save Project", QKeySequence::Save, this, [this] {
        log("File > Save Project selected.");
        saveProject();
    });
    file->addSeparator();
    file->addAction("Run Patcher...", this, [this] {
        log("File > Run Patcher selected.");
        runPatcher();
    });
    file->addAction("Build ROM...", this, [this] {
        log("File > Build ROM selected.");
        buildRom();
    });
    file->addSeparator();
    file->addAction("Exit", this, [this] {
        log("File > Exit selected.");
        close();
    });

    auto* edit = bar->addMenu("&Edit");
    undoAction_ = edit->addAction("Undo", QKeySequence::Undo, this, [this] { undo(); });
    redoAction_ = edit->addAction("Redo", QKeySequence::Redo, this, [this] { redo(); });
    updateEditState();
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
    auto* propertiesContainer = new QWidget(props);
    auto* propertiesLayout = new QVBoxLayout(propertiesContainer);
    propertiesLayout->setContentsMargins(0, 0, 0, 0);
    propertiesLabel_ = new QLabel(propertiesContainer);
    propertiesLabel_->setAlignment(Qt::AlignTop | Qt::AlignLeft);
    propertiesLabel_->setTextInteractionFlags(Qt::TextSelectableByMouse);
    propertiesLabel_->setMargin(8);
    levelProperties_ = new LevelPropertiesWidget(propertiesContainer);
    propertiesLayout->addWidget(propertiesLabel_);
    propertiesLayout->addWidget(levelProperties_, 1);
    props->setWidget(propertiesContainer);
    addDockWidget(Qt::RightDockWidgetArea, props);

    connect(levelProperties_, &LevelPropertiesWidget::levelChanged, this, [this] {
        preview_->update();
        updateEditState();
    });
    connect(levelProperties_, &LevelPropertiesWidget::selectionChanged, preview_, &LevelPreviewWidget::setSelection);
    connect(levelProperties_, &LevelPropertiesWidget::errorOccurred, this, [this](const QString& message) {
        log("Edit failed: " + message);
        QMessageBox::warning(this, "Level Edit Failed", message);
    });

    auto* logDock = new QDockWidget("Log", this);
    logOutput_ = new QTextEdit(logDock);
    logOutput_->setReadOnly(true);
    logDock->setWidget(logOutput_);
    addDockWidget(Qt::BottomDockWidgetArea, logDock);

    updateProjectSummary();
    log("Qt shell launched.");
}

bool MainWindow::newProject() {
    if (!confirmDiscardLevelEdits()) {
        return false;
    }
    const auto directory = QFileDialog::getExistingDirectory(this, "Create Big Star Project");
    if (directory.isEmpty()) {
        log("New Project cancelled before choosing a folder.");
        return false;
    }
    const auto project_dir = std::filesystem::path(directory.toStdString());
    const auto manifest = project_dir / nsmb::project::manifest_file_name;
    if (std::filesystem::exists(manifest)) {
        QMessageBox::warning(this, "Project Already Exists", "This folder already contains a Big Star project. Use Open Project instead.");
        return false;
    }

    bool accepted = false;
    auto default_name = QString::fromStdString(project_dir.filename().string());
    if (default_name.isEmpty()) {
        default_name = "New Project";
    }
    const auto name = QInputDialog::getText(this, "New Project", "Project name", QLineEdit::Normal, default_name, &accepted).trimmed();
    if (!accepted || name.isEmpty()) {
        log("New Project cancelled before choosing a name.");
        return false;
    }

    auto project = nsmb::project::create_empty_project(name.toStdString(), project_dir);
    if (!project) {
        QMessageBox::warning(this, "New Project Failed", QString::fromStdString(project.error().message));
        return false;
    }
    auto saved = nsmb::project::save_manifest(project.value(), manifest);
    if (!saved) {
        QMessageBox::warning(this, "New Project Failed", QString::fromStdString(saved.error().message));
        return false;
    }
    setProject(project.value());
    log("Created project: " + QString::fromStdString(manifest.string()));
    return true;
}

bool MainWindow::importRom() {
    if (!confirmDiscardLevelEdits()) {
        return false;
    }
    const auto rom_text = QFileDialog::getOpenFileName(this, "Import Nintendo DS ROM", {}, "Nintendo DS ROMs (*.nds);;All Files (*)");
    if (rom_text.isEmpty()) {
        log("Import ROM cancelled before choosing a ROM.");
        return false;
    }
    const auto directory = QFileDialog::getExistingDirectory(this, "Choose an Empty Project Folder");
    if (directory.isEmpty()) {
        log("Import ROM cancelled before choosing a project folder.");
        return false;
    }

    const auto rom_path = std::filesystem::path(rom_text.toStdString());
    const auto project_dir = std::filesystem::path(directory.toStdString());
    const auto manifest = project_dir / nsmb::project::manifest_file_name;
    const auto extracted_dir = project_dir / "extracted-rom";
    std::error_code ec;
    if (std::filesystem::exists(manifest)) {
        QMessageBox::warning(this, "Import ROM Failed", "The selected folder already contains a Big Star project.");
        return false;
    }
    if (std::filesystem::exists(extracted_dir) && !std::filesystem::is_empty(extracted_dir, ec)) {
        QMessageBox::warning(this, "Import ROM Failed", "The selected folder already contains a non-empty extracted-rom directory.");
        return false;
    }

    auto rom_data = readBinaryFile(rom_path);
    if (!rom_data) {
        QMessageBox::warning(this, "Import ROM Failed", QString::fromStdString(rom_data.error().message));
        return false;
    }
    auto header = nsmb::nds::parse_rom_header(rom_data.value());
    if (!header) {
        QMessageBox::warning(this, "Import ROM Failed", QString::fromStdString(header.error().message));
        return false;
    }

    log("Importing ROM: " + rom_text);
    nsmb::nds::NitroRomService service;
    nsmb::core::Result<nsmb::nds::ExtractedRom> extracted = nsmb::core::make_error(nsmb::core::ErrorCode::IoError, "import did not start");
    {
        WaitCursor cursor;
        extracted = service.extract(rom_path, extracted_dir);
    }
    if (!extracted) {
        QMessageBox::warning(this, "Import ROM Failed", QString::fromStdString(extracted.error().message));
        log("ROM import failed: " + QString::fromStdString(extracted.error().message));
        return false;
    }

    auto name = project_dir.filename().string();
    if (name.empty()) {
        name = header.value().game_title.empty() ? "Imported ROM" : header.value().game_title;
    }
    auto project = nsmb::project::create_empty_project(name, project_dir).value();
    project.attach_extracted_rom("extracted-rom", header.value());
    collectProjectFiles(project, project_dir, extracted_dir);
    auto saved = nsmb::project::save_manifest(project, manifest);
    if (!saved) {
        QMessageBox::warning(this, "Import ROM Failed", QString::fromStdString(saved.error().message));
        return false;
    }
    project.mark_clean();
    setProject(project);
    log(QString("Imported %1 files from %2.").arg(static_cast<qulonglong>(project.game_files().size())).arg(rom_text));
    QMessageBox::information(this, "ROM Imported", "The ROM was extracted and the project is ready.");
    return true;
}

void MainWindow::openProject() {
    if (!confirmDiscardLevelEdits()) {
        return;
    }
    const auto directory = QFileDialog::getExistingDirectory(this, "Open Big Star Project");
    if (directory.isEmpty()) {
        log("Open Project cancelled before choosing a folder.");
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

nsmb::core::Result<void> MainWindow::saveEditedLevels() {
    struct PendingFile {
        std::filesystem::path target;
        std::filesystem::path relative;
        std::filesystem::path backup;
        std::vector<std::uint8_t> bytes;
    };
    struct PendingLevel {
        nsmb::project::EditableLevel* level{};
    };

    std::vector<PendingFile> files;
    std::vector<PendingLevel> levels;
    const auto backup_root = currentProject_.directory() / ".big-star-backups" /
                             (QDateTime::currentDateTimeUtc().toString("yyyyMMdd-HHmmss-zzz-") + QString::number(QApplication::applicationPid()))
                                 .toStdString();

    for (const auto& [key, level] : levelSessions_) {
        if (!level->dirty()) {
            continue;
        }
        const auto separator = key.find('\n');
        if (separator == std::string::npos) {
            return nsmb::core::make_error(nsmb::core::ErrorCode::InvalidArgument, "level session path is invalid");
        }
        auto course_relative = nsmb::project::make_project_relative(currentProject_.directory(), key.substr(0, separator));
        auto bgdat_relative = nsmb::project::make_project_relative(currentProject_.directory(), key.substr(separator + 1U));
        if (!course_relative) {
            return course_relative.error();
        }
        if (!bgdat_relative) {
            return bgdat_relative.error();
        }

        auto course = level->serialize_course();
        auto bgdat = level->serialize_bgdat();
        if (!course) {
            return course.error();
        }
        if (!bgdat) {
            return bgdat.error();
        }
        auto verified = nsmb::project::EditableLevel::load(course.value(), bgdat.value());
        if (!verified) {
            return nsmb::core::make_error(nsmb::core::ErrorCode::ParseError,
                                          "serialized level failed validation: " + verified.error().message);
        }

        levels.push_back({level.get()});
        files.push_back({currentProject_.directory() / course_relative.value(),
                         course_relative.value(),
                         backup_root / course_relative.value(),
                         std::move(course.value())});
        files.push_back({currentProject_.directory() / bgdat_relative.value(),
                         bgdat_relative.value(),
                         backup_root / bgdat_relative.value(),
                         std::move(bgdat.value())});
    }
    if (files.empty()) {
        return {};
    }

    std::error_code ec;
    for (const auto& file : files) {
        if (!std::filesystem::is_regular_file(file.target, ec)) {
            return nsmb::core::make_error(nsmb::core::ErrorCode::NotFound, "level file does not exist: " + file.target.string());
        }
        std::filesystem::create_directories(file.backup.parent_path(), ec);
        if (ec) {
            return nsmb::core::make_error(nsmb::core::ErrorCode::IoError, "could not create the level backup directory");
        }
        std::filesystem::copy_file(file.target, file.backup, std::filesystem::copy_options::none, ec);
        if (ec) {
            return nsmb::core::make_error(nsmb::core::ErrorCode::IoError, "could not back up " + file.target.string());
        }
    }

    std::size_t written = 0;
    for (; written < files.size(); ++written) {
        auto result = writeBinaryFileAtomically(files[written].target, files[written].bytes);
        if (!result) {
            for (std::size_t rollback = 0; rollback < written; ++rollback) {
                std::filesystem::copy_file(files[rollback].backup,
                                           files[rollback].target,
                                           std::filesystem::copy_options::overwrite_existing,
                                           ec);
            }
            return result.error();
        }
    }

    for (const auto& file : files) {
        currentProject_.update_game_file_size(file.relative, file.bytes.size());
    }
    for (const auto& level : levels) {
        level.level->mark_clean();
    }
    log("Backed up original level files to " + QString::fromStdString(backup_root.string()));
    return {};
}

bool MainWindow::saveProject() {
    if (currentProject_.directory().empty()) {
        return newProject();
    }

    auto levels_saved = saveEditedLevels();
    if (!levels_saved) {
        QMessageBox::warning(this, "Save Project Failed", QString::fromStdString(levels_saved.error().message));
        log("Could not save edited levels: " + QString::fromStdString(levels_saved.error().message));
        return false;
    }

    const auto manifest_path = currentProject_.directory() / nsmb::project::manifest_file_name;
    auto saved = nsmb::project::save_manifest(currentProject_, manifest_path);
    if (!saved) {
        QMessageBox::warning(this, "Save Project Failed", QString::fromStdString(saved.error().message));
        log("Could not save project: " + QString::fromStdString(saved.error().message));
        return false;
    }
    currentProject_.mark_clean();
    log("Saved project: " + QString::fromStdString(manifest_path.string()));
    updateEditState();
    return true;
}

void MainWindow::runPatcher() {
    if (currentProject_.directory().empty() || !currentProject_.rom()) {
        log("Run Patcher requires an imported ROM project.");
        QMessageBox::information(this, "Run Patcher", "Open or import a ROM project first.");
        return;
    }
    if (!saveProject()) {
        return;
    }

    auto executable = QStandardPaths::findExecutable("ncpatcher");
    if (executable.isEmpty()) {
        executable = QFileDialog::getOpenFileName(this, "Select NCPatcher Executable");
        if (executable.isEmpty()) {
            log("Run Patcher cancelled before choosing NCPatcher.");
            return;
        }
    }

    log("Running NCPatcher: " + executable);
    nsmb::patching::NCPatcherExternalTool patcher;
    nsmb::core::Result<nsmb::patching::ExternalToolResult> result =
        nsmb::core::make_error(nsmb::core::ErrorCode::ExternalToolFailed, "patcher did not start");
    {
        WaitCursor cursor;
        result = patcher.run({currentProject_.directory(), std::filesystem::path(executable.toStdString()), true});
    }
    if (!result) {
        QMessageBox::warning(this, "NCPatcher Failed", QString::fromStdString(result.error().message));
        log("NCPatcher failed: " + QString::fromStdString(result.error().message));
        return;
    }
    log("NCPatcher completed successfully.");
    QMessageBox::information(this, "NCPatcher Complete", "NCPatcher completed successfully.");
}

void MainWindow::buildRom() {
    if (currentProject_.directory().empty() || !currentProject_.rom() || currentProject_.rom()->extracted_path.empty()) {
        log("Build ROM requires an imported ROM project.");
        QMessageBox::information(this, "Build ROM", "Open or import a ROM project first.");
        return;
    }
    if (!saveProject()) {
        return;
    }

    const auto default_name = QString::fromStdString(currentProject_.name().empty() ? "rebuilt.nds" : currentProject_.name() + ".nds");
    const auto default_path = QString::fromStdString((currentProject_.directory() / default_name.toStdString()).string());
    const auto output = QFileDialog::getSaveFileName(this, "Build Nintendo DS ROM", default_path, "Nintendo DS ROMs (*.nds)");
    if (output.isEmpty()) {
        log("Build ROM cancelled before choosing an output file.");
        return;
    }

    auto extracted_path = currentProject_.rom()->extracted_path;
    if (!extracted_path.is_absolute()) {
        extracted_path = currentProject_.directory() / extracted_path;
    }
    log("Building ROM: " + output);
    nsmb::nds::NitroRomService service;
    nsmb::core::Result<void> result;
    {
        WaitCursor cursor;
        result = service.build(extracted_path, std::filesystem::path(output.toStdString()));
    }
    if (!result) {
        QMessageBox::warning(this, "Build ROM Failed", QString::fromStdString(result.error().message));
        log("ROM build failed: " + QString::fromStdString(result.error().message));
        return;
    }
    log("Built ROM: " + output);
    QMessageBox::information(this, "ROM Built", "The rebuilt ROM was written successfully.");
}

void MainWindow::setProject(const nsmb::project::Project& project) {
    levelProperties_->setLevel(nullptr);
    currentLevel_ = nullptr;
    levelSessions_.clear();
    currentProject_ = project;
    projectModel_->setProject(currentProject_);
    if (levelTree_) {
        levelTree_->expandToDepth(1);
    }
    setWindowTitle(projectTitle(currentProject_));
    updateProjectSummary();
    updateEditState();
}

void MainWindow::updateProjectSummary() {
    if (!preview_ || !propertiesLabel_) {
        return;
    }

    preview_->showProjectSummary(currentProject_, projectModel_->levelCount(), projectModel_->availableLevelCount());
    currentLevel_ = nullptr;
    levelProperties_->setLevel(nullptr);

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

    auto session = levelSession(course_path, bgdat_path);
    currentLevel_ = session ? session.value() : nullptr;
    levelProperties_->setLevel(currentLevel_);
    if (preview_) {
        preview_->showLevel(currentProject_, name, world, area_name, course_path, bgdat_path, source_token, currentLevel_);
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
        if (!session) {
            text += "\nError: " + QString::fromStdString(session.error().message);
            log("Could not load level: " + QString::fromStdString(session.error().message));
        }
        propertiesLabel_->setText(text);
    }
    updateEditState();
}

nsmb::core::Result<nsmb::project::EditableLevel*> MainWindow::levelSession(const QString& course_path, const QString& bgdat_path) {
    if (course_path.isEmpty() || bgdat_path.isEmpty()) {
        return nsmb::core::make_error(nsmb::core::ErrorCode::NotFound, "course or BGDat file is missing");
    }
    const auto key = course_path.toStdString() + "\n" + bgdat_path.toStdString();
    if (const auto found = levelSessions_.find(key); found != levelSessions_.end()) {
        return found->second.get();
    }

    const auto course_file = currentProject_.directory() / std::filesystem::path(course_path.toStdString());
    const auto bgdat_file = currentProject_.directory() / std::filesystem::path(bgdat_path.toStdString());
    auto course_data = readBinaryFile(course_file);
    if (!course_data) {
        return course_data.error();
    }
    auto bgdat_data = readBinaryFile(bgdat_file);
    if (!bgdat_data) {
        return bgdat_data.error();
    }
    auto level = nsmb::project::EditableLevel::load(course_data.value(), bgdat_data.value());
    if (!level) {
        return level.error();
    }
    auto session = std::make_unique<nsmb::project::EditableLevel>(std::move(level.value()));
    auto* pointer = session.get();
    levelSessions_.emplace(key, std::move(session));
    return pointer;
}

void MainWindow::undo() {
    if (!currentLevel_) {
        return;
    }
    const auto name = currentLevel_->undo_name();
    auto result = currentLevel_->undo();
    if (!result) {
        log("Undo failed: " + QString::fromStdString(result.error().message));
        return;
    }
    log("Undid " + QString::fromStdString(name) + ".");
    levelProperties_->refresh();
    preview_->update();
    updateEditState();
}

void MainWindow::redo() {
    if (!currentLevel_) {
        return;
    }
    const auto name = currentLevel_->redo_name();
    auto result = currentLevel_->redo();
    if (!result) {
        log("Redo failed: " + QString::fromStdString(result.error().message));
        return;
    }
    log("Redid " + QString::fromStdString(name) + ".");
    levelProperties_->refresh();
    preview_->update();
    updateEditState();
}

void MainWindow::updateEditState() {
    if (undoAction_) {
        undoAction_->setEnabled(currentLevel_ && currentLevel_->can_undo());
        undoAction_->setText(currentLevel_ && currentLevel_->can_undo() ? "Undo " + QString::fromStdString(currentLevel_->undo_name()) : "Undo");
    }
    if (redoAction_) {
        redoAction_->setEnabled(currentLevel_ && currentLevel_->can_redo());
        redoAction_->setText(currentLevel_ && currentLevel_->can_redo() ? "Redo " + QString::fromStdString(currentLevel_->redo_name()) : "Redo");
    }
    setWindowTitle(projectTitle(currentProject_) + ((currentProject_.dirty() || hasUnsavedLevelEdits()) ? " *" : ""));
}

bool MainWindow::hasUnsavedLevelEdits() const {
    for (const auto& [key, level] : levelSessions_) {
        (void)key;
        if (level->dirty()) {
            return true;
        }
    }
    return false;
}

bool MainWindow::confirmDiscardLevelEdits() {
    if (!currentProject_.dirty() && !hasUnsavedLevelEdits()) {
        return true;
    }
    const auto choice = QMessageBox::warning(this,
                                             "Unsaved Project Changes",
                                             "Save changes to the current project?",
                                             QMessageBox::Save | QMessageBox::Discard | QMessageBox::Cancel,
                                             QMessageBox::Save);
    if (choice == QMessageBox::Save) {
        return saveProject();
    }
    return choice == QMessageBox::Discard;
}

void MainWindow::closeEvent(QCloseEvent* event) {
    if (confirmDiscardLevelEdits()) {
        event->accept();
    } else {
        event->ignore();
    }
}

void MainWindow::log(const QString& message) {
    if (logOutput_) {
        logOutput_->append(message);
    }
}
