#include "MainWindow.hpp"

#include "nsmb/core/file_io.hpp"
#include "nsmb/nds/rom.hpp"
#include "nsmb/patching/external_tools.hpp"
#include "nsmb/project/manifest.hpp"
#include "nsmb/project/graphics_resources.hpp"

#include <QApplication>
#include <QByteArray>
#include <QDateTime>
#include <QDockWidget>
#include <QCloseEvent>
#include <QFileDialog>
#include <QFileInfo>
#include <QFileSystemWatcher>
#include <QHeaderView>
#include <QIcon>
#include <QItemSelectionModel>
#include <QInputDialog>
#include <QKeySequence>
#include <QLineEdit>
#include <QMenu>
#include <QMenuBar>
#include <QMessageBox>
#include <QListWidget>
#include <QPixmap>
#include <QSaveFile>
#include <QSettings>
#include <QStandardPaths>
#include <QTabBar>
#include <QTabWidget>
#include <QTimer>
#include <QVBoxLayout>

#include <algorithm>
#include <array>
#include <filesystem>
#include <limits>
#include <span>
#include <utility>
#include <vector>

namespace {

QString projectTitle(const nsmb::project::Project& project) {
    if (project.name().empty()) {
        return "Big Star Editor";
    }
    return QString::fromStdString(project.name()) + " - Big Star Editor";
}

nsmb::core::Result<std::vector<std::uint8_t>> readBinaryFile(const std::filesystem::path& path) {
    return nsmb::core::read_file(path);
}

nsmb::core::Result<std::vector<std::uint8_t>> readBinaryFilePrefix(const std::filesystem::path& path, std::size_t maximum_size) {
    return nsmb::core::read_file_prefix(path, maximum_size);
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

std::filesystem::path extractedProjectPath(const nsmb::project::Project& project) {
    if (!project.rom()) {
        return {};
    }
    auto path = project.rom()->extracted_path;
    return path.is_absolute() ? path : project.directory() / path;
}

QString workspaceKey(const nsmb::project::Project& project) {
    if (project.directory().empty()) {
        return {};
    }
    const auto path = QString::fromStdString(project.directory().lexically_normal().generic_string()).toUtf8();
    return path.toBase64(QByteArray::Base64UrlEncoding | QByteArray::OmitTrailingEquals);
}

} // namespace

MainWindow::MainWindow() {
    setWindowTitle("Big Star Editor");
    resize(1200, 800);

    projectModel_ = new ProjectTreeModel(this);
    createWorkspace();
    createMenus();
    createDocks();
    defaultDockState_ = saveState();

    graphicsWatcher_ = new QFileSystemWatcher(this);
    graphicsRefreshTimer_ = new QTimer(this);
    graphicsRefreshTimer_->setSingleShot(true);
    graphicsRefreshTimer_->setInterval(250);
    connect(graphicsWatcher_, &QFileSystemWatcher::fileChanged, this, [this](const QString& path) {
        changedGraphicsFiles_.insert(path);
        graphicsRefreshTimer_->start();
    });
    connect(graphicsRefreshTimer_, &QTimer::timeout, this, &MainWindow::refreshChangedGraphics);

    auto project = nsmb::project::create_empty_project("Empty Project", {}).value();
    setProject(project);
}

MainWindow::~MainWindow() { clearDocuments(); }

void MainWindow::createWorkspace() {
    tabs_ = new QTabWidget(this);
    tabs_->setObjectName("DocumentTabs");
    tabs_->setTabsClosable(true);
    tabs_->setMovable(true);
    tabs_->setDocumentMode(true);
    tabs_->tabBar()->setContextMenuPolicy(Qt::CustomContextMenu);
    setCentralWidget(tabs_);

    preview_ = new LevelPreviewWidget(tabs_);
    preview_->setEditable(false);
    tabs_->addTab(preview_, "Preview");
    connect(tabs_, &QTabWidget::tabCloseRequested, this, &MainWindow::closeTab);
    connect(tabs_, &QTabWidget::currentChanged, this, [this] { activateCurrentDocument(); });
    connect(tabs_->tabBar(), &QTabBar::customContextMenuRequested, this, [this](const QPoint& position) {
        const auto index = tabs_->tabBar()->tabAt(position);
        auto* view = index >= 0 ? qobject_cast<LevelPreviewWidget*>(tabs_->widget(index)) : nullptr;
        if (!view || view == preview_) {
            return;
        }
        for (const auto& [key, document] : documents_) {
            if (document.view == view) {
                QMenu menu(this);
                menu.addAction("Move into New Window", this, [this, key] { detachEditor(key); });
                menu.exec(tabs_->tabBar()->mapToGlobal(position));
                return;
            }
        }
    });
    connect(preview_, &LevelPreviewWidget::graphicsWarning, this, [this](const QString& warning) { log("Graphics: " + warning); });
    connect(qApp, &QApplication::focusChanged, this, [this](QWidget*, QWidget* now) {
        if (!now) {
            return;
        }
        for (const auto& [key, document] : documents_) {
            (void)key;
            if (document.window && document.window == now->window()) {
                currentLevel_ = levelSessions_.contains(key) ? levelSessions_.at(key).get() : nullptr;
                levelProperties_->setLevel(currentLevel_);
                refreshPalette();
                updateEditState();
                return;
            }
        }
    });
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
    viewMenu_ = bar->addMenu("&View");
    updateEditState();
}

void MainWindow::createDocks() {
    levelsDock_ = new QDockWidget("Levels", this);
    levelsDock_->setObjectName("LevelsDock");
    levelTree_ = new QTreeView(levelsDock_);
    levelTree_->setModel(projectModel_);
    levelTree_->setRootIsDecorated(true);
    levelTree_->setAlternatingRowColors(true);
    levelTree_->setSelectionBehavior(QAbstractItemView::SelectRows);
    levelTree_->setSelectionMode(QAbstractItemView::SingleSelection);
    levelTree_->header()->setStretchLastSection(true);
    levelTree_->header()->setSectionResizeMode(0, QHeaderView::ResizeToContents);
    levelTree_->header()->setSectionResizeMode(1, QHeaderView::ResizeToContents);
    levelsDock_->setWidget(levelTree_);
    addDockWidget(Qt::LeftDockWidgetArea, levelsDock_);
    levelTree_->expandToDepth(1);

    connect(levelTree_->selectionModel(), &QItemSelectionModel::currentChanged, this, [this](const QModelIndex& current, const QModelIndex&) {
        updateSelectedLevel(current);
    });
    connect(levelTree_, &QTreeView::doubleClicked, this, &MainWindow::openLevelEditor);

    propertiesDock_ = new QDockWidget("Properties", this);
    propertiesDock_->setObjectName("PropertiesDock");
    auto* propertiesContainer = new QWidget(propertiesDock_);
    auto* propertiesLayout = new QVBoxLayout(propertiesContainer);
    propertiesLayout->setContentsMargins(0, 0, 0, 0);
    propertiesLabel_ = new QLabel(propertiesContainer);
    propertiesLabel_->setAlignment(Qt::AlignTop | Qt::AlignLeft);
    propertiesLabel_->setTextInteractionFlags(Qt::TextSelectableByMouse);
    propertiesLabel_->setMargin(8);
    levelProperties_ = new LevelPropertiesWidget(propertiesContainer);
    propertiesLayout->addWidget(propertiesLabel_);
    propertiesLayout->addWidget(levelProperties_, 1);
    propertiesDock_->setWidget(propertiesContainer);
    addDockWidget(Qt::RightDockWidgetArea, propertiesDock_);

    connect(levelProperties_, &LevelPropertiesWidget::levelChanged, this, [this] {
        preview_->update();
        for (auto& [key, document] : documents_) {
            (void)key;
            document.view->update();
        }
        for (auto& [key, document] : documents_) {
            (void)key;
            if (document.view->level() == currentLevel_) configureGraphics(document.view);
        }
        updateEditState();
    });
    connect(levelProperties_, &LevelPropertiesWidget::selectionChanged, this, [this](int kind, int item, int node) {
        for (auto& [key, document] : documents_) {
            (void)key;
            if (document.view->level() == currentLevel_) document.view->setSelection(kind, item, node);
        }
    });
    connect(levelProperties_, &LevelPropertiesWidget::errorOccurred, this, [this](const QString& message) {
        log("Edit failed: " + message);
        QMessageBox::warning(this, "Level Edit Failed", message);
    });

    paletteDock_ = new QDockWidget("Palette", this);
    paletteDock_->setObjectName("PaletteDock");
    auto* paletteContainer = new QWidget(paletteDock_);
    auto* paletteLayout = new QVBoxLayout(paletteContainer);
    paletteLayout->setContentsMargins(4, 4, 4, 4);
    paletteSearch_ = new QLineEdit(paletteContainer);
    paletteSearch_->setPlaceholderText("Filter palette...");
    paletteList_ = new QListWidget(paletteContainer);
    paletteLayout->addWidget(paletteSearch_);
    paletteLayout->addWidget(paletteList_, 1);
    paletteDock_->setWidget(paletteContainer);
    addDockWidget(Qt::LeftDockWidgetArea, paletteDock_);
    tabifyDockWidget(levelsDock_, paletteDock_);
    levelsDock_->raise();
    connect(paletteSearch_, &QLineEdit::textChanged, this, [this](const QString& text) {
        for (int row = 0; row < paletteList_->count(); ++row) {
            paletteList_->item(row)->setHidden(!paletteList_->item(row)->text().contains(text, Qt::CaseInsensitive));
        }
    });
    connect(paletteList_, &QListWidget::currentItemChanged, this, [this](QListWidgetItem* item, QListWidgetItem*) {
        LevelPreviewWidget* view = nullptr;
        for (auto& [key, document] : documents_) {
            (void)key;
            if (document.view->level() == currentLevel_) view = document.view;
        }
        if (!view) return;
        if (!item) {
            view->clearPlacement();
            return;
        }
        const auto tool = static_cast<LevelPreviewWidget::Tool>(item->data(Qt::UserRole).toInt());
        if (tool == LevelPreviewWidget::Tool::Object) {
            view->setPlacementObject(item->data(Qt::UserRole + 2).toInt(), item->data(Qt::UserRole + 1).toInt());
        } else if (tool == LevelPreviewWidget::Tool::Sprite) {
            view->setPlacementSprite(item->data(Qt::UserRole + 1).toInt());
        } else {
            view->setPlacementTool(tool);
        }
        view->setFocus();
    });

    logDock_ = new QDockWidget("Log", this);
    logDock_->setObjectName("LogDock");
    logOutput_ = new QTextEdit(logDock_);
    logOutput_->setReadOnly(true);
    logDock_->setWidget(logOutput_);
    addDockWidget(Qt::BottomDockWidgetArea, logDock_);

    for (auto* dock : {levelsDock_, propertiesDock_, paletteDock_, logDock_}) {
        viewMenu_->addAction(dock->toggleViewAction());
    }
    viewMenu_->addSeparator();
    previewAction_ = viewMenu_->addAction("Preview");
    previewAction_->setObjectName("PreviewAction");
    previewAction_->setCheckable(true);
    previewAction_->setChecked(true);
    connect(previewAction_, &QAction::triggered, this, [this](bool visible) {
        if (visible) {
            showPreview();
        } else if (const auto index = tabs_->indexOf(preview_); index >= 0) {
            tabs_->removeTab(index);
        }
    });
    viewMenu_->addAction("Reset Layout", this, &MainWindow::resetWorkspaceLayout);
    viewMenu_->addAction("Refresh Graphics", this, &MainWindow::refreshGraphics);

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

    auto rom_data = readBinaryFilePrefix(rom_path, nsmb::nds::default_header_region_size);
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
    auto indexed = nsmb::project::index_project_files(project, extracted_dir);
    if (!indexed) {
        QMessageBox::warning(this, "Import ROM Failed", QString::fromStdString(indexed.error().message));
        log("ROM indexing failed: " + QString::fromStdString(indexed.error().message));
        return false;
    }
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
    auto refreshed = currentProject_;
    auto indexed = nsmb::project::index_project_files(refreshed, extractedProjectPath(refreshed));
    if (!indexed) {
        QMessageBox::warning(this,
                             "NCPatcher Refresh Failed",
                             "NCPatcher completed, but the project file index could not be refreshed:\n" +
                                 QString::fromStdString(indexed.error().message));
        log("NCPatcher completed, but project refresh failed: " + QString::fromStdString(indexed.error().message));
        return;
    }
    const auto manifest = refreshed.directory() / nsmb::project::manifest_file_name;
    auto saved = nsmb::project::save_manifest(refreshed, manifest);
    if (!saved) {
        QMessageBox::warning(this,
                             "NCPatcher Refresh Failed",
                             "NCPatcher completed, but the refreshed project manifest could not be saved:\n" +
                                 QString::fromStdString(saved.error().message));
        log("NCPatcher completed, but manifest refresh failed: " + QString::fromStdString(saved.error().message));
        return;
    }
    refreshed.mark_clean();
    setProject(refreshed);
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

    const auto extracted_path = extractedProjectPath(currentProject_);
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
    saveWorkspaceState();
    if (graphicsWatcher_ && !graphicsWatcher_->files().empty()) {
        graphicsWatcher_->removePaths(graphicsWatcher_->files());
    }
    levelProperties_->setLevel(nullptr);
    currentLevel_ = nullptr;
    clearDocuments();
    graphicsFiles_.clear();
    changedGraphicsFiles_.clear();
    levelSessions_.clear();
    currentProject_ = project;
    projectModel_->setProject(currentProject_);
    if (levelTree_) {
        levelTree_->expandToDepth(1);
    }
    setWindowTitle(projectTitle(currentProject_));
    updateProjectSummary();
    restoreWorkspaceState();
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
    currentLevel_ = nullptr;
    levelProperties_->setLevel(nullptr);
    if (preview_) {
        preview_->showLevel(name, world, area_name, course_path, bgdat_path, source_token, session ? session.value() : nullptr);
        configureGraphics(preview_);
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
    showPreview();
    updateEditState();
}

void MainWindow::showPreview() {
    auto index = tabs_->indexOf(preview_);
    if (index < 0) {
        index = tabs_->insertTab(0, preview_, "Preview");
    }
    tabs_->setCurrentIndex(index);
    if (previewAction_) {
        previewAction_->setChecked(true);
    }
}

void MainWindow::openLevelEditor(const QModelIndex& source_index) {
    if (!source_index.isValid()) {
        return;
    }
    const auto index = source_index.sibling(source_index.row(), 0);
    const auto kind = projectModel_->data(index, ProjectTreeModel::NodeKindRole).toInt();
    if (kind != ProjectTreeModel::AreaNode && kind != ProjectTreeModel::OtherNode) {
        return;
    }
    const auto course_path = projectModel_->data(index, ProjectTreeModel::CoursePathRole).toString();
    const auto bgdat_path = projectModel_->data(index, ProjectTreeModel::BgdatPathRole).toString();
    const auto key = course_path.toStdString() + "\n" + bgdat_path.toStdString();
    if (auto found = documents_.find(key); found != documents_.end()) {
        if (found->second.window) {
            found->second.window->show();
            found->second.window->raise();
            found->second.window->activateWindow();
        } else {
            tabs_->setCurrentWidget(found->second.view);
        }
        return;
    }

    auto session = levelSession(course_path, bgdat_path);
    if (!session) {
        log("Could not open level editor: " + QString::fromStdString(session.error().message));
        QMessageBox::warning(this, "Open Level Failed", QString::fromStdString(session.error().message));
        return;
    }
    LevelDocument document;
    document.name = projectModel_->data(index, Qt::DisplayRole).toString();
    document.world = projectModel_->data(index, ProjectTreeModel::WorldRole).toString();
    document.area_name = projectModel_->data(index, ProjectTreeModel::AreaNameRole).toString();
    document.source_token = projectModel_->data(index, ProjectTreeModel::SourceTokenRole).toString();
    document.course_path = course_path;
    document.bgdat_path = bgdat_path;
    document.view = new LevelPreviewWidget(tabs_);
    document.view->setEditable(true);
    document.view->showLevel(document.name,
                             document.world,
                             document.area_name,
                             document.course_path,
                             document.bgdat_path,
                             document.source_token,
                             session.value());
    configureGraphics(document.view);
    auto* view = document.view;
    documents_.emplace(key, std::move(document));
    connect(view, &LevelPreviewWidget::selectionRequested, this, [this, view](int kind, int item, int node) {
        if ((tabs_->currentWidget() == view || view->window()->isActiveWindow()) && kind >= 0 && kind <= 6) {
            levelProperties_->select(static_cast<LevelPropertiesWidget::Kind>(kind), item, node);
        }
    });
    connect(view, &LevelPreviewWidget::multiSelectionRequested, this, [this, view](int kind, const QList<int>& items) {
        if ((tabs_->currentWidget() == view || view->window()->isActiveWindow()) && !items.empty()) {
            levelProperties_->selectMany(static_cast<LevelPropertiesWidget::Kind>(kind), items);
        }
    });
    connect(view, &LevelPreviewWidget::levelChanged, this, [this] {
        levelProperties_->refresh();
        preview_->update();
        updateEditState();
    });
    connect(view, &LevelPreviewWidget::errorOccurred, this, [this](const QString& message) {
        log("Canvas edit failed: " + message);
        QMessageBox::warning(this, "Level Edit Failed", message);
    });
    connect(view, &LevelPreviewWidget::graphicsWarning, this, [this](const QString& warning) { log("Graphics: " + warning); });
    connect(view, &LevelPreviewWidget::graphicsLoaded, this, [this, view] {
        if (view->level() == currentLevel_) refreshPalette();
    });
    tabs_->addTab(view, documents_.at(key).name);
    tabs_->setCurrentWidget(view);
    activateCurrentDocument();
}

void MainWindow::closeTab(int index) {
    auto* widget = tabs_->widget(index);
    if (widget == preview_) {
        tabs_->removeTab(index);
        previewAction_->setChecked(false);
        return;
    }
    for (auto it = documents_.begin(); it != documents_.end(); ++it) {
        if (it->second.view == widget) {
            tabs_->removeTab(index);
            delete widget;
            documents_.erase(it);
            activateCurrentDocument();
            return;
        }
    }
}

void MainWindow::activateCurrentDocument() {
    auto* view = qobject_cast<LevelPreviewWidget*>(tabs_->currentWidget());
    currentLevel_ = nullptr;
    if (!view || view == preview_) {
        if (levelProperties_) {
            levelProperties_->setLevel(nullptr);
        }
        updateEditState();
        refreshPalette();
        return;
    }
    for (const auto& [key, document] : documents_) {
        if (document.view != view) {
            continue;
        }
        if (const auto found = levelSessions_.find(key); found != levelSessions_.end()) {
            currentLevel_ = found->second.get();
        }
        levelProperties_->setLevel(currentLevel_);
        propertiesLabel_->setText("Level: " + document.name + "\nWorld: " + document.world + "\nCourse: " + document.course_path +
                                  "\nBGDat: " + document.bgdat_path);
        break;
    }
    updateEditState();
    refreshPalette();
}

void MainWindow::detachEditor(const std::string& key) {
    auto found = documents_.find(key);
    if (found == documents_.end() || found->second.window) {
        return;
    }
    const auto index = tabs_->indexOf(found->second.view);
    if (index >= 0) {
        tabs_->removeTab(index);
    }
    auto* window = new QMainWindow(this, Qt::Window);
    window->setAttribute(Qt::WA_DeleteOnClose);
    window->setWindowTitle(found->second.name + " - Big Star Editor");
    window->resize(1000, 700);
    window->setCentralWidget(found->second.view);
    auto* document_menu = window->menuBar()->addMenu("Document");
    document_menu->addAction("Move Back to Main Window", this, [this, key] { reattachEditor(key); });
    found->second.window = window;
    connect(window, &QObject::destroyed, this, [this, key] {
        const auto document = documents_.find(key);
        if (document != documents_.end() && tabs_->indexOf(document->second.view) < 0) {
            documents_.erase(document);
        }
        activateCurrentDocument();
    });
    window->show();
    found->second.view->show();
    currentLevel_ = levelSessions_.contains(key) ? levelSessions_.at(key).get() : nullptr;
    levelProperties_->setLevel(currentLevel_);
    updateEditState();
}

void MainWindow::reattachEditor(const std::string& key) {
    auto found = documents_.find(key);
    if (found == documents_.end() || !found->second.window) {
        return;
    }
    auto* window = found->second.window.data();
    auto* view = qobject_cast<LevelPreviewWidget*>(window->takeCentralWidget());
    found->second.window.clear();
    tabs_->addTab(view, found->second.name);
    tabs_->setCurrentWidget(view);
    view->show();
    QObject::disconnect(window, nullptr, this, nullptr);
    delete window;
    activateCurrentDocument();
}

void MainWindow::clearDocuments() {
    for (auto& [key, document] : documents_) {
        (void)key;
        if (document.window) {
            document.window->takeCentralWidget();
            QObject::disconnect(document.window, nullptr, this, nullptr);
            delete document.window.data();
        } else if (const auto index = tabs_->indexOf(document.view); index >= 0) {
            tabs_->removeTab(index);
        }
        delete document.view;
        graphicsFiles_.erase(document.view);
    }
    documents_.clear();
}

QModelIndex MainWindow::findLevelIndex(const QString& course_path, const QString& bgdat_path) const {
    const auto find = [&](const auto& self, const QModelIndex& parent) -> QModelIndex {
        for (int row = 0; row < projectModel_->rowCount(parent); ++row) {
            const auto index = projectModel_->index(row, 0, parent);
            if (projectModel_->data(index, ProjectTreeModel::CoursePathRole).toString() == course_path &&
                projectModel_->data(index, ProjectTreeModel::BgdatPathRole).toString() == bgdat_path) {
                return index;
            }
            if (const auto nested = self(self, index); nested.isValid()) {
                return nested;
            }
        }
        return {};
    };
    return find(find, {});
}

void MainWindow::saveWorkspaceState() {
    const auto key = workspaceKey(currentProject_);
    if (key.isEmpty() || !tabs_) {
        return;
    }
    QSettings settings;
    settings.beginGroup("workspaces/" + key);
    settings.setValue("geometry", saveGeometry());
    settings.setValue("docks", saveState());
    settings.setValue("previewVisible", tabs_->indexOf(preview_) >= 0);
    settings.setValue("previewZoom", preview_->zoom());
    settings.setValue("previewPan", preview_->pan());
    QStringList open;
    QStringList detached;
    QString active = tabs_->currentWidget() == preview_ ? "preview" : QString{};
    for (const auto& [document_key, document] : documents_) {
        const auto text = QString::fromStdString(document_key);
        open.push_back(text);
        const auto document_group = "documents/" + text.toUtf8().toBase64(QByteArray::Base64UrlEncoding | QByteArray::OmitTrailingEquals);
        settings.setValue(document_group + "/zoom", document.view->zoom());
        settings.setValue(document_group + "/pan", document.view->pan());
        if (document.window) {
            detached.push_back(text);
            settings.setValue("windows/" + text.toUtf8().toBase64(QByteArray::Base64UrlEncoding | QByteArray::OmitTrailingEquals) + "/geometry",
                              document.window->saveGeometry());
        }
        if (!document.window && tabs_->currentWidget() == document.view) {
            active = text;
        } else if (document.window && document.window->isActiveWindow()) {
            active = text;
        }
    }
    settings.setValue("documents", open);
    settings.setValue("detached", detached);
    settings.setValue("active", active);
    settings.endGroup();
}

void MainWindow::restoreWorkspaceState() {
    const auto key = workspaceKey(currentProject_);
    if (key.isEmpty()) {
        return;
    }
    QSettings settings;
    settings.beginGroup("workspaces/" + key);
    restoreGeometry(settings.value("geometry").toByteArray());
    restoreState(settings.value("docks").toByteArray());
    preview_->setZoom(settings.value("previewZoom", 1.0).toDouble());
    preview_->setPan(settings.value("previewPan", QPointF{}).toPointF());
    const auto detached = settings.value("detached").toStringList();
    for (const auto& document_key : settings.value("documents").toStringList()) {
        const auto separator = document_key.indexOf('\n');
        if (separator < 0) {
            continue;
        }
        const auto index = findLevelIndex(document_key.left(separator), document_key.mid(separator + 1));
        if (!index.isValid()) {
            continue;
        }
        openLevelEditor(index);
        if (const auto found = documents_.find(document_key.toStdString()); found != documents_.end()) {
            const auto document_group = "documents/" +
                                        document_key.toUtf8().toBase64(QByteArray::Base64UrlEncoding | QByteArray::OmitTrailingEquals);
            found->second.view->setZoom(settings.value(document_group + "/zoom", 1.0).toDouble());
            found->second.view->setPan(settings.value(document_group + "/pan", QPointF{}).toPointF());
        }
        if (detached.contains(document_key)) {
            detachEditor(document_key.toStdString());
            const auto found = documents_.find(document_key.toStdString());
            if (found != documents_.end() && found->second.window) {
                const auto geometry_key = "windows/" +
                                          document_key.toUtf8().toBase64(QByteArray::Base64UrlEncoding | QByteArray::OmitTrailingEquals) +
                                          "/geometry";
                found->second.window->restoreGeometry(settings.value(geometry_key).toByteArray());
            }
        }
    }
    const auto active = settings.value("active", "preview").toString();
    const auto preview_visible = settings.value("previewVisible", true).toBool();
    if (!preview_visible && tabs_->indexOf(preview_) >= 0) {
        tabs_->removeTab(tabs_->indexOf(preview_));
        previewAction_->setChecked(false);
    } else if (active == "preview") {
        showPreview();
    } else if (const auto found = documents_.find(active.toStdString()); found != documents_.end() && !found->second.window) {
        tabs_->setCurrentWidget(found->second.view);
    }
    settings.endGroup();
}

void MainWindow::resetWorkspaceLayout() {
    restoreState(defaultDockState_);
    for (auto* dock : {levelsDock_, propertiesDock_, paletteDock_, logDock_}) {
        dock->show();
    }
    showPreview();
}

void MainWindow::configureGraphics(LevelPreviewWidget* view) {
    if (!view || !view->level()) {
        return;
    }
    const auto references = view->level()->tileset_references();
    if (view->graphicsMatch(references)) return;
    auto resources = nsmb::project::resolve_level_graphics(currentProject_, references);
    if (!resources) {
        log("Graphics fallback: " + QString::fromStdString(resources.error().message));
        return;
    }
    QStringList watch;
    QStringList add;
    for (const auto& path : resources.value().watched_files) {
        const auto text = QString::fromStdString(path.string());
        watch.push_back(text);
        if (QFileInfo::exists(text) && !graphicsWatcher_->files().contains(text)) add.push_back(text);
    }
    graphicsFiles_.insert_or_assign(view, watch);
    if (!add.empty()) graphicsWatcher_->addPaths(add);
    view->setGraphicsResources(std::move(resources.value()), references);
}

void MainWindow::refreshGraphics() {
    if (!graphicsWatcher_) return;
    const auto watched = graphicsWatcher_->files();
    if (!watched.empty()) graphicsWatcher_->removePaths(watched);
    preview_->invalidateGraphics();
    configureGraphics(preview_);
    for (auto& [key, document] : documents_) {
        (void)key;
        document.view->invalidateGraphics();
        configureGraphics(document.view);
    }
}

void MainWindow::refreshChangedGraphics() {
    if (changedGraphicsFiles_.isEmpty()) return;
    const auto changed = std::exchange(changedGraphicsFiles_, {});
    auto refresh = [&](LevelPreviewWidget* view) {
        const auto files = graphicsFiles_.find(view);
        if (files == graphicsFiles_.end() || !std::ranges::any_of(files->second, [&](const QString& path) { return changed.contains(path); })) {
            return;
        }
        view->invalidateGraphics();
        configureGraphics(view);
    };
    refresh(preview_);
    for (auto& [key, document] : documents_) {
        (void)key;
        refresh(document.view);
    }
}

void MainWindow::refreshPalette() {
    if (!paletteList_) return;
    paletteList_->clear();
    LevelPreviewWidget* view = nullptr;
    for (auto& [key, document] : documents_) {
        (void)key;
        if (document.view->level() == currentLevel_) view = document.view;
    }
    if (view) {
        for (auto& entry : view->paletteEntries()) {
            auto* item = new QListWidgetItem(entry.label, paletteList_);
            item->setData(Qt::UserRole, static_cast<int>(entry.tool));
            item->setData(Qt::UserRole + 1, entry.id);
            item->setData(Qt::UserRole + 2, entry.slot);
            if (!entry.image.isNull()) item->setIcon(QIcon(QPixmap::fromImage(entry.image.scaled(32, 32, Qt::KeepAspectRatio))));
        }
    }
    const bool has_objects = paletteList_->findItems("Object ", Qt::MatchStartsWith).size() > 0;
    const bool has_sprites = paletteList_->count() > 0 && paletteList_->count() !=
        paletteList_->findItems("Object ", Qt::MatchStartsWith).size();
    const std::array utilities{
        std::pair{LevelPreviewWidget::Tool::Entrance, "Entrance"},
        std::pair{LevelPreviewWidget::Tool::Zone, "Zone"},
        std::pair{LevelPreviewWidget::Tool::Path, "Path"},
        std::pair{LevelPreviewWidget::Tool::ProgressPath, "Progress Path"},
    };
    if (!has_objects) {
        auto* item = new QListWidgetItem("Object", paletteList_);
        item->setData(Qt::UserRole, static_cast<int>(LevelPreviewWidget::Tool::Object));
    }
    if (!has_sprites) {
        auto* item = new QListWidgetItem("Sprite", paletteList_);
        item->setData(Qt::UserRole, static_cast<int>(LevelPreviewWidget::Tool::Sprite));
    }
    for (const auto& [tool, name] : utilities) {
        auto* item = new QListWidgetItem(name, paletteList_);
        item->setData(Qt::UserRole, static_cast<int>(tool));
    }
    const auto filter = paletteSearch_->text();
    for (int row = 0; row < paletteList_->count(); ++row) {
        auto* item = paletteList_->item(row);
        item->setHidden(!item->text().contains(filter, Qt::CaseInsensitive));
    }
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
    preview_->refreshLevelState();
    for (auto& [key, document] : documents_) {
        (void)key;
        document.view->refreshLevelState();
    }
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
    preview_->refreshLevelState();
    for (auto& [key, document] : documents_) {
        (void)key;
        document.view->refreshLevelState();
    }
    updateEditState();
}

void MainWindow::updateEditState() {
    if (undoAction_) {
        undoAction_->setEnabled(currentLevel_ && currentLevel_->can_undo());
        undoAction_->setText(currentLevel_ && currentLevel_->can_undo() ? "Undo " + QString::fromStdString(currentLevel_->undo_name()) : "Undo");
    }
    for (const auto& [key, document] : documents_) {
        const auto dirty = levelSessions_.contains(key) && levelSessions_.at(key)->dirty();
        const auto title = document.name + (dirty ? " *" : "");
        if (document.window) {
            document.window->setWindowTitle(title + " - Big Star Editor");
        } else if (const auto index = tabs_->indexOf(document.view); index >= 0) {
            tabs_->setTabText(index, title);
        }
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
        saveWorkspaceState();
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
