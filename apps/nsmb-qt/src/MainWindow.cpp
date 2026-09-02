#include "MainWindow.hpp"

#include "nsmb/core/file_io.hpp"
#include "nsmb/nds/rom.hpp"
#include "PatcherPanel.hpp"
#include "FilesystemDock.hpp"
#include "TilesetEditorDialog.hpp"
#include "FileEditors.hpp"
#include "UiResourceEditorDialog.hpp"
#include "nsmb/patching/ncpatcher.hpp"
#include "nsmb/project/patch_apply.hpp"
#include "nsmb/project/manifest.hpp"
#include "nsmb/project/graphics_resources.hpp"
#include "nsmb/project/level_exchange.hpp"
#include "nsmb/patching/process.hpp"

#include <QApplication>
#include <QActionGroup>
#include <QByteArray>
#include <QDateTime>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDir>
#include <QDockWidget>
#include <QCloseEvent>
#include <QFileDialog>
#include <QFileInfo>
#include <QFileSystemWatcher>
#include <QFormLayout>
#include <QHeaderView>
#include <QIcon>
#include <QItemSelectionModel>
#include <QInputDialog>
#include <QKeySequence>
#include <QLineEdit>
#include <QComboBox>
#include <QMenu>
#include <QMenuBar>
#include <QMessageBox>
#include <QListWidget>
#include <QPlainTextEdit>
#include <QPixmap>
#include <QPushButton>
#include <QSaveFile>
#include <QScrollArea>
#include <QSettings>
#include <QStandardPaths>
#include <QSpinBox>
#include <QTabBar>
#include <QTabWidget>
#include <QTimer>
#include <QTemporaryDir>
#include <QTreeWidget>
#include <QToolBox>
#include <QToolButton>
#include <QToolBar>
#include <QGridLayout>
#include <QVBoxLayout>

#include <algorithm>
#include <array>
#include <filesystem>
#include <limits>
#include <map>
#include <span>
#include <utility>
#include <unordered_set>
#include <vector>

namespace {

QString projectTitle(const nsmb::project::Project& project) {
    if (project.name().empty()) {
        return "Big Star Editor";
    }
    return QString::fromStdString(project.name()) + " - Big Star Editor";
}

nsmb::core::Result<std::string> readTextFile(const std::filesystem::path& path) {
    auto bytes = nsmb::core::read_file(path);
    if (!bytes) return bytes.error();
    return std::string(reinterpret_cast<const char*>(bytes.value().data()), bytes.value().size());
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

// A new backup directory: what made it, then when, then which process. The
// kind leads because that is what a sweep groups on; milliseconds are there
// because two level saves in one second are ordinary, and the process id
// because two editors open on one project are not unheard of.
std::filesystem::path newBackupDirectory(const std::filesystem::path& project_dir, const QString& kind) {
    return nsmb::project::patch_backup_root(project_dir) /
           (kind + "-" + QDateTime::currentDateTimeUtc().toString("yyyyMMdd-HHmmss-zzz") + "-" +
            QString::number(QApplication::applicationPid()))
               .toStdString();
}

// Backups accumulate for as long as somebody keeps working, and nothing ever
// asked them to leave. Swept whenever one is added and once when a project
// opens, so a project that has been edited for a year is the size of one that
// was edited last week.
void sweepBackups(const std::filesystem::path& project_dir) {
    if (project_dir.empty()) return;
    nsmb::project::prune_patch_backups(
        project_dir, static_cast<std::size_t>(std::clamp(QSettings().value("BackupsKept", 10).toInt(), 1, 100)));
}

QString manifestPathText(const nsmb::project::Project& project) {
    if (project.directory().empty()) {
        return {};
    }
    return QString::fromStdString((project.directory() / nsmb::project::manifest_file_name).string());
}

std::filesystem::path projectRomPath(const nsmb::project::Project& project) {
    if (!project.rom()) {
        return {};
    }
    auto path = project.rom()->rom_path;
    return path.is_absolute() ? path : project.directory() / path;
}

QString hexBytes(std::span<const std::uint8_t> bytes) {
    return QByteArray(reinterpret_cast<const char*>(bytes.data()), static_cast<qsizetype>(bytes.size())).toHex(' ').toUpper();
}

std::optional<std::vector<std::uint8_t>> unhexBytes(QString text) {
    text.remove(' '); text.remove('\t'); text.remove('\r'); text.remove('\n');
    if (text.size() % 2) return std::nullopt;
    for (const auto c : text) if (!c.isDigit() && (c.toLower() < 'a' || c.toLower() > 'f')) return std::nullopt;
    const auto bytes = QByteArray::fromHex(text.toLatin1()); return std::vector<std::uint8_t>(bytes.begin(), bytes.end());
}

QString workspaceKey(const nsmb::project::Project& project) {
    if (project.directory().empty()) {
        return {};
    }
    const auto path = QString::fromStdString(project.directory().lexically_normal().generic_string()).toUtf8();
    return path.toBase64(QByteArray::Base64UrlEncoding | QByteArray::OmitTrailingEquals);
}

constexpr int max_recent_projects = 10;
constexpr auto recent_projects_key = "RecentProjects";

QStringList recentProjects() {
    return QSettings().value(recent_projects_key).toStringList();
}

void storeRecentProjects(const QStringList& projects) {
    QSettings().setValue(recent_projects_key, projects);
}

QString uiText(std::string_view english) {
    struct Translation { std::string_view english, nl, pt, es, fr; };
    static constexpr std::array translations{
        Translation{"Project Hub","Projecthub","Central do projeto","Centro del proyecto","Centre du projet"},
        Translation{"Level Listing","Leveloverzicht","Lista de níveis","Lista de niveles","Liste des niveaux"},
        Translation{"Tilesets","Tegelsets","Conjuntos de blocos","Conjuntos de bloques","Jeux de tuiles"},
        Translation{"Backgrounds","Achtergronden","Fundos","Fondos","Arrière-plans"},
        Translation{"Filesystem","Bestandssysteem","Sistema de arquivos","Sistema de archivos","Système de fichiers"},
        Translation{"Tools","Gereedschap","Ferramentas","Herramientas","Outils"},
        Translation{"Open Editor","Editor openen","Abrir editor","Abrir editor","Ouvrir l’éditeur"},
        Translation{"Hex Editor","Hex-editor","Editor hexadecimal","Editor hexadecimal","Éditeur hexadécimal"},
        Translation{"File","Bestand","Arquivo","Archivo","Fichier"},
        Translation{"Edit","Bewerken","Editar","Editar","Édition"},
        Translation{"View","Beeld","Exibir","Ver","Affichage"},
        Translation{"Distribution","Distributie","Distribuição","Distribución","Distribution"},
        Translation{"Save Project","Project opslaan","Salvar projeto","Guardar proyecto","Enregistrer le projet"},
        Translation{"Cut","Knippen","Recortar","Cortar","Couper"},
        Translation{"Copy","Kopiëren","Copiar","Copiar","Copier"},
        Translation{"Paste","Plakken","Colar","Pegar","Coller"},
        Translation{"Language","Taal","Idioma","Idioma","Langue"},
    };
    const auto language = QSettings().value("Language", "en").toString();
    const auto found = std::ranges::find(translations, english, &Translation::english);
    if (found == translations.end()) return QString::fromUtf8(english.data(), static_cast<qsizetype>(english.size()));
    const auto value = language == "nl" ? found->nl : language == "pt" ? found->pt : language == "es" ? found->es : language == "fr" ? found->fr : found->english;
    return QString::fromUtf8(value.data(), static_cast<qsizetype>(value.size()));
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

    // The ROM a build replaces. It is one file, so watching it costs one
    // handle where the extracted tree cost thousands.
    romWatcher_ = new QFileSystemWatcher(this);
    connect(romWatcher_, &QFileSystemWatcher::fileChanged, this, [this] {
        reloadRom();
        moduleRefreshTimer_->start();
    });

    moduleWatcher_ = new QFileSystemWatcher(this);
    moduleRefreshTimer_ = new QTimer(this);
    moduleRefreshTimer_->setSingleShot(true);
    moduleRefreshTimer_->setInterval(250);
    connect(moduleWatcher_, &QFileSystemWatcher::directoryChanged, moduleRefreshTimer_, qOverload<>(&QTimer::start));
    connect(moduleRefreshTimer_, &QTimer::timeout, this, &MainWindow::reloadFilesystemView);
    autosaveTimer_ = new QTimer(this);
    autosaveTimer_->setInterval(std::clamp(QSettings().value("AutosaveMinutes", 1).toInt(), 1, 60) * 60000);
    autosaveTimer_->start();
    connect(autosaveTimer_, &QTimer::timeout, this, &MainWindow::writeRecoverySnapshots);

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
    connect(preview_, &LevelPreviewWidget::graphicsLoaded, this, [this] { if (preview_->level() == currentLevel_) refreshPalette(); });
    connect(qApp, &QApplication::focusChanged, this, [this](QWidget*, QWidget* now) {
        if (!now) {
            return;
        }
        for (const auto& [key, document] : documents_) {
            (void)key;
            if (document.window && document.window == now->window()) {
                currentLevel_ = levelSessions_.contains(key) ? levelSessions_.at(key).get() : nullptr;
                levelProperties_->setLevel(currentLevel_);
                levelData_->setLevel(currentLevel_, &moduleData_);
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

    // One toolbar for the things done constantly. Undo and redo used to be the
    // only ones with a button; saving, opening a file and starting a build were
    // menu-only, which is a lot of digging for a rebuild-and-test loop.
    auto* main_bar = addToolBar("Main");
    main_bar->setObjectName("MainToolbar");
    main_bar->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);

    auto* file = bar->addMenu("&" + uiText("File"));
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
    recentMenu_ = file->addMenu("Open &Recent");
    updateRecentMenu();
    auto* saveProjectAction = file->addAction(uiText("Save Project"), QKeySequence::Save, this, [this] {
        log("File > Save Project selected.");
        saveProject();
    });
    saveProjectAction->setObjectName("SaveProjectAction");
    auto* openFile = file->addAction("Open File...", QKeySequence(Qt::CTRL | Qt::SHIFT | Qt::Key_O), this, &MainWindow::openFileInEditor);
    openFile->setObjectName("OpenFileAction");
    file->addAction("Export Level (.nml)…", this, &MainWindow::exportLevelPackage);
    file->addAction("Import Level (.nml)…", this, &MainWindow::importLevelPackage);
    file->addSeparator();
    auto* runPatcherAction = file->addAction("Run Patcher...", this, [this] {
        log("File > Run Patcher selected.");
        runPatcher();
    });
    runPatcherAction->setObjectName("RunPatcherAction");
    // Repacking a tree the editor unpacked was never a legitimate way to
    // produce a ROM: it went through a second builder that had to agree with
    // NCPatcher's and could not. Exporting one, on the other hand, is somebody
    // wanting the files -- and NCPatcher's own extractor writes a directory it
    // can read back, so that is the one that runs.
    auto* extractRomAction = file->addAction("Extract ROM...", this, [this] {
        log("File > Extract ROM selected.");
        extractRom();
    });
    extractRomAction->setObjectName("ExtractRomAction");
    file->addSeparator();
    file->addAction("Exit", this, [this] {
        log("File > Exit selected.");
        close();
    });

    auto* edit = bar->addMenu("&" + uiText("Edit"));
    undoAction_ = edit->addAction("Undo", QKeySequence::Undo, this, [this] { undo(); });
    redoAction_ = edit->addAction("Redo", QKeySequence::Redo, this, [this] { redo(); });
    main_bar->addAction(saveProjectAction);
    main_bar->addAction(openFile);
    main_bar->addSeparator();
    undoButton_ = new QToolButton(main_bar);
    redoButton_ = new QToolButton(main_bar);
    for (auto* button : {undoButton_, redoButton_}) {
        button->setPopupMode(QToolButton::MenuButtonPopup);
        button->setMenu(new QMenu(button));
        button->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
        main_bar->addWidget(button);
    }
    undoButton_->setDefaultAction(undoAction_);
    redoButton_->setDefaultAction(redoAction_);
    connect(undoButton_->menu(),&QMenu::aboutToShow,this,[this]{auto* menu=undoButton_->menu();menu->clear();if(!currentLevel_)return;const auto names=currentLevel_->undo_names();for(std::size_t i=0;i<names.size();++i)menu->addAction(QString::fromStdString(names[i]),this,[this,count=i+1]{for(std::size_t n=0;n<count&&currentLevel_&&currentLevel_->can_undo();++n)undo();});});
    connect(redoButton_->menu(),&QMenu::aboutToShow,this,[this]{auto* menu=redoButton_->menu();menu->clear();if(!currentLevel_)return;const auto names=currentLevel_->redo_names();for(std::size_t i=0;i<names.size();++i)menu->addAction(QString::fromStdString(names[i]),this,[this,count=i+1]{for(std::size_t n=0;n<count&&currentLevel_&&currentLevel_->can_redo();++n)redo();});});
    edit->addSeparator();
    auto* cut = edit->addAction(uiText("Cut"), QKeySequence::Cut, this, [this] { if (auto* view=activeLevelView()) view->cutSelection(); });
    auto* copy = edit->addAction(uiText("Copy"), QKeySequence::Copy, this, [this] { if (auto* view=activeLevelView()) view->copySelection(); });
    auto* paste = edit->addAction(uiText("Paste"), QKeySequence::Paste, this, [this] { if (auto* view=activeLevelView()) view->pasteSelection(); });
    auto* duplicate = edit->addAction("Duplicate", QKeySequence(Qt::CTRL | Qt::Key_D), this, [this] { if (auto* view=activeLevelView()) view->cloneSelection(); });
    auto* forward = edit->addAction("Bring Forward", QKeySequence(Qt::Key_PageUp), this, [this] { if (auto* view=activeLevelView()) view->reorderSelection(true); });
    auto* backward = edit->addAction("Send Backward", QKeySequence(Qt::Key_PageDown), this, [this] { if (auto* view=activeLevelView()) view->reorderSelection(false); });
    main_bar->addSeparator();
    for (auto* action : {cut, copy, paste, duplicate}) main_bar->addAction(action);
    main_bar->addSeparator();
    for (auto* action : {forward, backward}) main_bar->addAction(action);
    main_bar->addSeparator();
    main_bar->addAction(runPatcherAction);
    // One variant selector for the whole window. There used to be two, in the
    // Filesystem and Build Output docks, kept in sync by hand and each showing
    // only part of what a variant governs.
    main_bar->addSeparator();
    main_bar->addWidget(new QLabel(" Variant: ", main_bar));
    variantSelector_ = new QComboBox(main_bar);
    variantSelector_->setObjectName("VariantSelector");
    variantSelector_->setMinimumContentsLength(12);
    variantSelector_->setToolTip(
        "Which configured build variant the editor is working in.\n"
        "Switching reloads levels and graphics from that variant's module layers.");
    variantSelector_->addItem("(none)", QString());
    main_bar->addWidget(variantSelector_);
    connect(variantSelector_, &QComboBox::currentIndexChanged, this,
            [this](int) { setActiveVariant(variantSelector_->currentData().toString()); });
    edit->addSeparator();
    auto* preferences = edit->addAction("Preferences…", QKeySequence::Preferences, this, &MainWindow::editPreferences);
    preferences->setObjectName("PreferencesAction");
    auto* tools = bar->addMenu("&" + uiText("Tools"));
    auto* tilesets = tools->addAction("Tilesets and Backgrounds…", this, &MainWindow::openTilesetEditor);
    tilesets->setObjectName("TilesetEditorAction");
    tools->addAction("Level Hex Editor…", this, &MainWindow::editLevelHex);
    tools->addAction("Find Object…", this, [this] { findEntity(false); });
    tools->addAction("Find Sprite…", this, [this] { findEntity(true); });
    tools->addAction("Sprite Event Viewer…", this, &MainWindow::showSpriteEvents);
    auto* bannerAction = tools->addAction("ROM Banner…", this, &MainWindow::editRomBanner);
    bannerAction->setObjectName("RomBannerAction");
    auto* metadataAction = tools->addAction("ROM User Metadata…", this, &MainWindow::editRomUserMetadata);
    metadataAction->setObjectName("RomUserMetadataAction");
    auto* distribution = bar->addMenu("&" + uiText("Distribution"));
    distribution->addAction("Export .nsmbpatch…", this, &MainWindow::exportPatch);
    distribution->addAction("Import .nsmbpatch…", this, &MainWindow::importPatch);
    distribution->addAction("Create xdelta…", this, [this] { runXdelta(true); });
    distribution->addAction("Apply xdelta…", this, [this] { runXdelta(false); });
    viewMenu_ = bar->addMenu("&" + uiText("View"));
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
    levelTree_->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(levelTree_, &QTreeView::customContextMenuRequested, this, &MainWindow::showLevelTreeMenu);

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
    levelData_ = new LevelDataWidget(propertiesContainer);
    auto* propertiesTabs = new QTabWidget(propertiesContainer);
    propertiesTabs->addTab(levelProperties_, "Selection");
    propertiesTabs->addTab(levelData_, "Level Data");
    propertiesLayout->addWidget(propertiesLabel_);
    propertiesLayout->addWidget(propertiesTabs, 1);
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
    // With no tool modes on the canvas, the panel's kind tab is what right-click
    // creates. Selecting on the canvas switches the tab, which comes back here
    // and keeps the two in step in both directions.
    connect(levelProperties_, &LevelPropertiesWidget::currentKindChanged, this, [this](int kind) {
        if (kind < 1 || kind > 7) return;
        preview_->setCreateKind(kind);
        for (auto& [key, document] : documents_) { (void)key; document.view->setCreateKind(kind); }
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
    connect(levelData_, &LevelDataWidget::levelChanged, this, [this] {
        preview_->update();
        for (auto& [key, document] : documents_) { (void)key; document.view->refreshLevelState(); }
        updateEditState();
    });
    connect(levelData_, &LevelDataWidget::errorOccurred, this, [this](const QString& message) {
        log("Level data edit failed: " + message);
        QMessageBox::warning(this, "Level Data Edit Failed", message);
    });

    paletteDock_ = new QDockWidget("Palette", this);
    paletteDock_->setObjectName("PaletteDock");
    auto* paletteContainer = new QWidget(paletteDock_);
    auto* paletteLayout = new QVBoxLayout(paletteContainer);
    paletteLayout->setContentsMargins(4, 4, 4, 4);
    paletteSearch_ = new QLineEdit(paletteContainer);
    paletteSearch_->setPlaceholderText("Filter palette...");
    auto* paletteTabs = new QTabWidget(paletteContainer);
    paletteTabs->setObjectName("PaletteTabs");
    // Objects and sprites are different things picked in different ways, and
    // flattening them into one alphabetised accordion made both hard to find.
    objectTabs_ = new QTabWidget(paletteTabs);
    objectTabs_->setObjectName("ObjectPickerTabs");
    objectTabs_->setDocumentMode(true);
    spriteToolbox_ = new QToolBox(paletteTabs);
    spriteToolbox_->setObjectName("SpritePicker");
    customPaletteTree_ = new QTreeWidget(paletteTabs);
    customPaletteTree_->setObjectName("CustomActorTree");
    customPaletteTree_->setHeaderHidden(true);
    paletteTabs->addTab(objectTabs_, "Objects");
    paletteTabs->addTab(spriteToolbox_, "Sprites");
    paletteTabs->addTab(customPaletteTree_, "Custom actors");
    paletteLayout->addWidget(paletteSearch_);
    paletteLayout->addWidget(paletteTabs, 1);
    paletteDock_->setWidget(paletteContainer);
    addDockWidget(Qt::LeftDockWidgetArea, paletteDock_);
    connect(paletteSearch_, &QLineEdit::textChanged, this, [this](const QString& text) {
        for (auto* button : paletteButtons_) button->setVisible(button->text().contains(text,Qt::CaseInsensitive)||button->property("category").toString().contains(text,Qt::CaseInsensitive));
        for (int module = 0; module < customPaletteTree_->topLevelItemCount(); ++module) {
            auto* moduleItem = customPaletteTree_->topLevelItem(module); bool moduleVisible = false;
            for (int component = 0; component < moduleItem->childCount(); ++component) {
                auto* componentItem = moduleItem->child(component); bool componentVisible = false;
                for (int actor = 0; actor < componentItem->childCount(); ++actor) {
                    auto* actorItem = componentItem->child(actor);
                    const bool visible = actorItem->text(0).contains(text, Qt::CaseInsensitive);
                    actorItem->setHidden(!visible); componentVisible |= visible;
                }
                componentItem->setHidden(!componentVisible); moduleVisible |= componentVisible;
            }
            moduleItem->setHidden(!moduleVisible);
        }
    });
    connect(customPaletteTree_, &QTreeWidget::currentItemChanged, this, [this](QTreeWidgetItem* item, QTreeWidgetItem*) {
        if (!item || !item->data(0, Qt::UserRole).isValid()) return;
        LevelPreviewWidget* view = nullptr;
        for (auto& [key, document] : documents_) { (void)key; if (document.view->level() == currentLevel_) view = document.view; }
        if (!view) return;
        view->setPlacementCustomSprite(static_cast<std::uint32_t>(item->data(0, Qt::UserRole).toULongLong()));
        view->setFocus();
    });

    filesystemDock_ = new QDockWidget("Filesystem", this);
    filesystemDock_->setObjectName("FilesystemDock");
    filesystemPanel_ = new FilesystemDock(filesystemDock_);
    filesystemDock_->setWidget(filesystemPanel_);
    addDockWidget(Qt::LeftDockWidgetArea, filesystemDock_);
    tabifyDockWidget(levelsDock_, filesystemDock_);
    connect(filesystemPanel_, &FilesystemDock::activeTargetChanged, this, [this](const QString& module, const QString& variant) {
        (void)variant;
        if (activeModule_ == module) return;
        activeModule_ = module;
        reloadFilesystemView();
    });
    connect(filesystemPanel_, &FilesystemDock::openInEditorRequested, this, [this](const QString& romPath) {
        const auto* entry = filesystemPanel_->entryFor(romPath);
        if (!entry) return;
        auto data = effectiveFiles_.read(romPath.toStdString());
        if (!data) { log("Open file: " + QString::fromStdString(data.error().message)); return; }
        const auto metadata = entry->id && *entry->id == rom_user_metadata_id;
        openFileDocument(activeVariant_ + "\n" + romPath, romPath, std::move(data.value()), metadata,
                         filesystemPanel_->companionsFor(romPath),
                         [this, romPath](const std::vector<std::uint8_t>& bytes) {
                             return filesystemPanel_->saveEditedFile(romPath, bytes);
                         });
    });
    connect(filesystemPanel_, &FilesystemDock::hexEditRequested, this, [this](const QString& romPath) {
        if (!filesystemPanel_->entryFor(romPath)) return;
        auto data = effectiveFiles_.read(romPath.toStdString());
        if (!data) { log("Hex edit: " + QString::fromStdString(data.error().message)); return; }
        const auto key = activeVariant_ + "\nhex\n" + romPath;
        if (raiseFileDocument(key)) return;
        auto* panel = new bse::editors::HexEditorPanel(romPath.toStdString(), data.value());
        hostFileDocument(key, romPath, panel, std::move(data.value()),
                         [this, romPath](const std::vector<std::uint8_t>& bytes) {
                             return filesystemPanel_->saveEditedFile(romPath, bytes);
                         },
                         false);
    });
    connect(filesystemPanel_, &FilesystemDock::filesChanged, this, &MainWindow::reloadFilesystemView);
    connect(filesystemPanel_, &FilesystemDock::runPatcherRequested, this, &MainWindow::runPatcher);
    connect(filesystemPanel_, &FilesystemDock::errorOccurred, this, [this](const QString& message) {
        log("Filesystem: " + message); QMessageBox::warning(this, "Filesystem Operation Failed", message);
    });

    // One place output goes. There used to be two bottom docks tabbed over each
    // other, and the patcher forwarded its log into the other one, so the same
    // lines appeared twice and you could only ever read one copy.
    buildDock_ = new QDockWidget("Output", this);
    buildDock_->setObjectName("BuildOutputDock");
    patcherPanel_ = new PatcherPanel(buildDock_);
    buildDock_->setWidget(patcherPanel_);
    addDockWidget(Qt::BottomDockWidgetArea, buildDock_);
    // Remembered per project, alongside the rest of the workspace state.
    connect(patcherPanel_, &PatcherPanel::diagnosticActivated, this,
            &MainWindow::openDiagnosticLocation);
    connect(patcherPanel_, &PatcherPanel::buildFinished, this, &MainWindow::onPatcherFinished);
    connect(patcherPanel_, &PatcherPanel::romApplied, this,
            [this](const QString& builtRom) { log("Built ROM: " + builtRom); });

    for (auto* dock : {levelsDock_, propertiesDock_, paletteDock_, filesystemDock_, buildDock_}) {
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
    const auto imported_rom = project_dir / nsmb::project::input_rom_file_name;
    std::error_code ec;
    if (std::filesystem::exists(manifest)) {
        QMessageBox::warning(this, "Import ROM Failed", "The selected folder already contains a Big Star project.");
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
    // The ROM is read where it lies, so importing one is copying it in and
    // reading its table -- and a manifest path has to stay inside the project,
    // so a ROM from elsewhere is copied rather than pointed at.
    if (std::filesystem::weakly_canonical(rom_path, ec) != std::filesystem::weakly_canonical(imported_rom, ec)) {
        WaitCursor cursor;
        std::filesystem::copy_file(rom_path, imported_rom, std::filesystem::copy_options::overwrite_existing, ec);
        if (ec) {
            QMessageBox::warning(this, "Import ROM Failed",
                                 "The ROM could not be copied into the project: " + QString::fromStdString(ec.message()));
            return false;
        }
    }

    auto name = project_dir.filename().string();
    if (name.empty()) {
        name = header.value().game_title.empty() ? "Imported ROM" : header.value().game_title;
    }
    auto project = nsmb::project::create_empty_project(name, project_dir).value();
    project.attach_rom(nsmb::project::input_rom_file_name, header.value());
    auto indexed = nsmb::project::index_rom_files(project);
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
    QMessageBox::information(this, "ROM Imported", "The ROM is in the project and ready to read.");
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

    openProjectPath(std::filesystem::path(directory.toStdString()));
}

bool MainWindow::openProjectPath(const std::filesystem::path& directory) {
    const auto manifest_path = directory / nsmb::project::manifest_file_name;
    auto loaded = nsmb::project::load_manifest(manifest_path);
    if (!loaded) {
        QMessageBox::warning(this, "Open Project Failed", QString::fromStdString(loaded.error().message));
        log("Could not open project: " + QString::fromStdString(loaded.error().message));
        return false;
    }

    setProject(loaded.value());
    sweepBackups(directory);
    log("Opened project: " + QString::fromStdString(manifest_path.string()));
    return true;
}

void MainWindow::openRecentProject(const QString& directory) {
    if (!confirmDiscardLevelEdits()) {
        return;
    }
    log("File > Open Recent: " + QDir::toNativeSeparators(directory));
    if (!openProjectPath(std::filesystem::path(directory.toStdString()))) {
        forgetRecentProject(directory);
    }
}

void MainWindow::rememberRecentProject(const std::filesystem::path& directory) {
    if (directory.empty()) {
        return;
    }
    const auto path = QDir(QString::fromStdString(directory.string())).absolutePath();
    auto projects = recentProjects();
    projects.removeAll(path);
    projects.prepend(path);
    while (projects.size() > max_recent_projects) {
        projects.removeLast();
    }
    storeRecentProjects(projects);
    updateRecentMenu();
}

void MainWindow::forgetRecentProject(const QString& directory) {
    auto projects = recentProjects();
    if (projects.removeAll(directory) == 0) {
        return;
    }
    storeRecentProjects(projects);
    updateRecentMenu();
    log("Removed a project that no longer opens from the recent list: " + QDir::toNativeSeparators(directory));
}

void MainWindow::updateRecentMenu() {
    if (!recentMenu_) {
        return;
    }
    recentMenu_->clear();
    const auto projects = recentProjects();
    recentMenu_->setEnabled(!projects.isEmpty());
    int number = 0;
    for (const auto& path : projects) {
        const QDir directory(path);
        // Menu text swallows a lone '&', and project folders may legitimately contain one.
        auto label = directory.dirName() + " - " + QDir::toNativeSeparators(directory.absolutePath());
        label.replace("&", "&&");
        ++number;
        if (number < 10) {
            label = QString("&%1  %2").arg(number).arg(label);
        }
        auto* action = recentMenu_->addAction(label, this, [this, path] { openRecentProject(path); });
        action->setStatusTip(QDir::toNativeSeparators(path));
    }
    recentMenu_->addSeparator();
    auto* clear = recentMenu_->addAction("Clear Recent Projects", this, [this] {
        storeRecentProjects({});
        updateRecentMenu();
        log("Cleared the recent project list.");
    });
    clear->setEnabled(!projects.isEmpty());
}

namespace {

// documentKey() is "variant\ncourse\nbgdat". Everything that needs the two paths
// back out of a session or document key goes through here rather than counting
// separators for itself.
struct DocumentKeyPaths { std::string course, bgdat; };

std::optional<DocumentKeyPaths> documentKeyPaths(const std::string& key) {
    const auto variant_end = key.find('\n');
    if (variant_end == std::string::npos) return std::nullopt;
    const auto separator = key.find('\n', variant_end + 1);
    if (separator == std::string::npos) return std::nullopt;
    return DocumentKeyPaths{key.substr(variant_end + 1, separator - variant_end - 1), key.substr(separator + 1)};
}

} // namespace

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
    const auto backup_root = newBackupDirectory(currentProject_.directory(), "levels");

    for (const auto& [key, level] : levelSessions_) {
        if (!level->dirty()) {
            continue;
        }
        const auto paths = documentKeyPaths(key);
        if (!paths) {
            return nsmb::core::make_error(nsmb::core::ErrorCode::InvalidArgument, "level session path is invalid");
        }
        auto course_target = moduleSaveTarget(paths->course);
        auto bgdat_target = moduleSaveTarget(paths->bgdat);
        if (!course_target) return course_target.error();
        if (!bgdat_target) return bgdat_target.error();

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
        auto course_target_relative = nsmb::project::make_project_relative(currentProject_.directory(), course_target.value());
        auto bgdat_target_relative = nsmb::project::make_project_relative(currentProject_.directory(), bgdat_target.value());
        if (!course_target_relative) return course_target_relative.error();
        if (!bgdat_target_relative) return bgdat_target_relative.error();
        files.push_back({course_target.value(),
                         course_target_relative.value(),
                         backup_root / course_target_relative.value(),
                         std::move(course.value())});
        files.push_back({bgdat_target.value(),
                         bgdat_target_relative.value(),
                         backup_root / bgdat_target_relative.value(),
                         std::move(bgdat.value())});
    }
    if (files.empty()) {
        return {};
    }

    std::error_code ec;
    for (const auto& file : files) {
        std::filesystem::create_directories(file.target.parent_path(), ec);
        if (ec) {
            return nsmb::core::make_error(nsmb::core::ErrorCode::IoError, "could not create the module target directory");
        }
        if (std::filesystem::is_regular_file(file.target, ec)) {
            std::filesystem::create_directories(file.backup.parent_path(), ec);
            if (ec) return nsmb::core::make_error(nsmb::core::ErrorCode::IoError, "could not create the level backup directory");
            std::filesystem::copy_file(file.target, file.backup, std::filesystem::copy_options::none, ec);
            if (ec) return nsmb::core::make_error(nsmb::core::ErrorCode::IoError, "could not back up " + file.target.string());
        }
    }

    std::size_t written = 0;
    for (; written < files.size(); ++written) {
        auto result = writeBinaryFileAtomically(files[written].target, files[written].bytes);
        if (!result) {
            for (std::size_t rollback = 0; rollback < written; ++rollback) {
                if (std::filesystem::is_regular_file(files[rollback].backup, ec))
                    std::filesystem::copy_file(files[rollback].backup, files[rollback].target, std::filesystem::copy_options::overwrite_existing, ec);
                else std::filesystem::remove(files[rollback].target, ec);
            }
            return result.error();
        }
    }

    for (const auto& level : levels) {
        level.level->mark_clean();
    }
    writeRecoverySnapshots();
    sweepBackups(currentProject_.directory());
    log("Saved level edits to the active module tree. Run NCPatcher to build them into the ROM.");
    reloadFilesystemView();
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
    if (patcherPanel_->running()) {
        log("NCPatcher is already running.");
        return;
    }
    if (!saveProject()) {
        return;
    }

    const auto config = nsmb::patching::detect_project_config(currentProject_.directory());
    if (config.kind == nsmb::patching::ProjectConfigKind::None) {
        QMessageBox::information(
            this, "Run Patcher",
            "This project has no ncpatcher.yaml, so there is nothing for NCPatcher to build.");
        log("Run Patcher: no ncpatcher.yaml in the project directory.");
        return;
    }
    if (config.kind == nsmb::patching::ProjectConfigKind::V1Json) {
        // Offered rather than done silently: migrating rewrites how the project
        // builds, and the old file stays put until a build has proved the new
        // one works.
        if (!offerNcpatcherMigration(config.file)) {
            return;
        }
    }

    auto executable = QStandardPaths::findExecutable("ncpatcher");
    if (executable.isEmpty()) {
        executable = QFileDialog::getOpenFileName(this, "Select NCPatcher Executable");
        if (executable.isEmpty()) {
            log("Run Patcher cancelled before choosing NCPatcher.");
            return;
        }
    }

    patcherPanel_->setProjectDirectory(currentProject_.directory());
    // A successful build becomes the ROM the editor reads, but only after the
    // one it produced in staging has been validated.
    patcherPanel_->setApplyToProject(true);
    patcherPanel_->setExecutable(std::filesystem::path(executable.toStdString()));
    patcherPanel_->setSelectedVariant(ncpatcherVariant_);

    buildDock_->show();
    buildDock_->raise();

    if (!patcherPanel_->startBuild(true)) {
        log("NCPatcher could not be started.");
        return;
    }
    log("Running NCPatcher: " + executable);
    updateEditState();
}

bool MainWindow::offerNcpatcherMigration(const std::filesystem::path& legacyConfig) {
    const auto answer = QMessageBox::question(
        this, "Legacy NCPatcher Project",
        "This project still uses the legacy ncpatcher.json:\n\n" +
            QString::fromStdString(legacyConfig.string()) +
            "\n\nNCPatcher 2 builds from ncpatcher.yaml. Convert it now?\n"
            "The original ncpatcher.json is left in place.",
        QMessageBox::Yes | QMessageBox::No, QMessageBox::Yes);
    if (answer != QMessageBox::Yes) {
        log("Run Patcher cancelled: the project still uses ncpatcher.json.");
        return false;
    }

    nsmb::patching::NCPatcherClient client;
    nsmb::core::Result<nsmb::patching::RunSummary> migrated =
        nsmb::core::make_error(nsmb::core::ErrorCode::ExternalToolFailed, "migrate did not run");
    {
        WaitCursor cursor;
        migrated = client.migrate(currentProject_.directory(), true);
    }
    if (!migrated || !migrated.value().succeeded()) {
        const QString reason = migrated
                                   ? QString::fromStdString(std::string(
                                         nsmb::patching::describe_stage(migrated.value().stage)))
                                   : QString::fromStdString(migrated.error().message);
        QMessageBox::warning(this, "Migration Failed",
                             "NCPatcher could not convert the project:\n" + reason);
        log("NCPatcher migration failed: " + reason);
        return false;
    }
    log("Converted ncpatcher.json to ncpatcher.yaml. The original file was left in place.");
    return true;
}

void MainWindow::openDiagnosticLocation(const QString& file, int line, int column) {
    // Big Star has no YAML editor of its own yet, so the honest thing is to say
    // exactly where the problem is rather than pretend to navigate there.
    log(QString("Diagnostic at %1:%2:%3").arg(file).arg(line).arg(column));
    QMessageBox::information(this, "Diagnostic Location",
                             QString("%1\n\nLine %2, column %3").arg(file).arg(line).arg(column));
}

void MainWindow::onPatcherFinished(bool ok, int exitCode, const QString& stage, int errors,
                                   int warnings) {
    updateEditState();

    if (!ok) {
        const QString detail = QString("%1 (exit code %2, %3 error(s), %4 warning(s))")
                                   .arg(stage)
                                   .arg(exitCode)
                                   .arg(errors)
                                   .arg(warnings);
        log("NCPatcher failed: " + detail);
        QMessageBox::warning(this, "NCPatcher Failed",
                             "NCPatcher did not finish:\n\n" + detail +
                                 "\n\nSee the Build Output dock for the diagnostics.");
        return;
    }

    // The apply replaced the ROM wholesale, so every cached level session now
    // holds bytes from a ROM that no longer exists.
    clearDocuments();
    levelSessions_.clear();
    currentLevel_ = nullptr;

    auto refreshed = currentProject_;
    // A build writes `big-star-build.nds`, which is not necessarily what the
    // project was reading before its first one.
    refreshed.attach_rom(nsmb::project::built_rom_file_name, refreshed.rom() ? refreshed.rom()->header : std::nullopt);
    auto indexed = nsmb::project::index_rom_files(refreshed);
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

    const QString summary = warnings > 0
                                ? QString("NCPatcher completed with %1 warning(s).").arg(warnings)
                                : QString("NCPatcher completed successfully.");
    log(summary);
    QMessageBox::information(this, "NCPatcher Complete", summary);
}

LevelPreviewWidget* MainWindow::activeLevelView() const {
    if (auto* view = qobject_cast<LevelPreviewWidget*>(tabs_->currentWidget()); view && view != preview_) return view;
    for (const auto& [key, document] : documents_) { (void)key; if (document.view->level() == currentLevel_) return document.view; }
    return nullptr;
}

void MainWindow::exportLevelPackage() {
    if (!currentLevel_) { QMessageBox::information(this, "Export Level", "Open an editable level first."); return; }
    auto course=currentLevel_->serialize_course(),bgdat=currentLevel_->serialize_bgdat();if(!course||!bgdat){QMessageBox::warning(this,"Export Level",QString::fromStdString(!course?course.error().message:bgdat.error().message));return;}
    nsmb::project::LevelPackage package; package.course=std::move(course.value()); package.bgdat=std::move(bgdat.value());
    QString suggested="level.nml";
    for(const auto& [key,document]:documents_)if(document.view->level()==currentLevel_){suggested=document.name+".nml";for(auto [path,id]:{std::pair{document.course_path,&package.course_file_id},std::pair{document.bgdat_path,&package.bgdat_file_id}})if(const auto* entry=filesystemView_.find(path.toStdString());entry&&entry->id&&*entry->id>=0&&*entry->id<=65535)*id=static_cast<std::uint16_t>(*entry->id);break;}
    auto encoded=nsmb::project::encode_nml(package);if(!encoded){QMessageBox::warning(this,"Export Level",QString::fromStdString(encoded.error().message));return;}
    const auto path=QFileDialog::getSaveFileName(this,"Export Level",suggested,"NSMB levels (*.nml)");if(path.isEmpty())return;auto result=nsmb::core::write_file_atomically(path.toStdString(),encoded.value());if(!result)QMessageBox::warning(this,"Export Level",QString::fromStdString(result.error().message));
}

void MainWindow::importLevelPackage() {
    if(!currentLevel_){QMessageBox::information(this,"Import Level","Open an editable level first.");return;}const auto path=QFileDialog::getOpenFileName(this,"Import Level",{},"NSMB levels (*.nml)");if(path.isEmpty())return;auto bytes=nsmb::core::read_file(path.toStdString());if(!bytes){QMessageBox::warning(this,"Import Level",QString::fromStdString(bytes.error().message));return;}auto package=nsmb::project::decode_nml(bytes.value());if(!package){QMessageBox::warning(this,"Import Level",QString::fromStdString(package.error().message));return;}auto result=currentLevel_->replace_serialized(package.value().course,package.value().bgdat,&moduleData_,"Import level");if(!result){QMessageBox::warning(this,"Import Level",QString::fromStdString(result.error().message));return;}if(auto* view=activeLevelView())view->refreshLevelState();levelProperties_->refresh();levelData_->refresh();updateEditState();
}

void MainWindow::editLevelHex() {
    if(!currentLevel_){QMessageBox::information(this,"Level Hex Editor","Open an editable level first.");return;}auto course=currentLevel_->serialize_course(),bgdat=currentLevel_->serialize_bgdat();if(!course||!bgdat)return;
    QDialog dialog(this);dialog.setWindowTitle("Level Hex Editor");dialog.resize(900,600);auto* layout=new QVBoxLayout(&dialog);auto* pages=new QTabWidget(&dialog);auto editor=[&](QString name,const auto& data){auto* text=new QPlainTextEdit(pages);text->setLineWrapMode(QPlainTextEdit::NoWrap);text->setPlainText(hexBytes(data));pages->addTab(text,name);return text;};auto* course_editor=editor("Course",course.value());auto* bgdat_editor=editor("BGDat",bgdat.value());layout->addWidget(pages);auto* buttons=new QDialogButtonBox(QDialogButtonBox::Save|QDialogButtonBox::Cancel,&dialog);layout->addWidget(buttons);connect(buttons,&QDialogButtonBox::accepted,&dialog,&QDialog::accept);connect(buttons,&QDialogButtonBox::rejected,&dialog,&QDialog::reject);if(dialog.exec()!=QDialog::Accepted)return;auto edited_course=unhexBytes(course_editor->toPlainText()),edited_bgdat=unhexBytes(bgdat_editor->toPlainText());if(!edited_course||!edited_bgdat){QMessageBox::warning(this,"Level Hex Editor","Hex data contains an invalid or incomplete byte.");return;}auto result=currentLevel_->replace_serialized(*edited_course,*edited_bgdat,&moduleData_,"Hex edit level");if(!result){QMessageBox::warning(this,"Level Hex Editor",QString::fromStdString(result.error().message));return;}if(auto* view=activeLevelView())view->refreshLevelState();levelProperties_->refresh();levelData_->refresh();updateEditState();
}

void MainWindow::findEntity(bool sprites) {
    if(currentProject_.directory().empty())return;bool ok=false;const auto id=QInputDialog::getInt(this,sprites?"Find Sprite":"Find Object",sprites?"Placement type":"Object ID",0,0,sprites?65535:4095,1,&ok);if(!ok)return;
    QDialog dialog(this);dialog.setWindowTitle(sprites?"Sprite Finder":"Object Finder");dialog.resize(760,480);auto* layout=new QVBoxLayout(&dialog);auto* results=new QTreeWidget(&dialog);results->setHeaderLabels({"Level","Index","X","Y","Settings"});layout->addWidget(results);auto* close=new QDialogButtonBox(QDialogButtonBox::Close,&dialog);layout->addWidget(close);connect(close,&QDialogButtonBox::rejected,&dialog,&QDialog::reject);
    WaitCursor wait;std::function<void(const QModelIndex&)> scan=[&](const QModelIndex& parent){for(int row=0;row<projectModel_->rowCount(parent);++row){const auto index=projectModel_->index(row,0,parent);const auto kind=projectModel_->data(index,ProjectTreeModel::NodeKindRole).toInt();if(kind==ProjectTreeModel::AreaNode&&projectModel_->data(index,ProjectTreeModel::AvailableRole).toBool()){const auto course_path=projectModel_->data(index,ProjectTreeModel::CoursePathRole).toString(),bgdat_path=projectModel_->data(index,ProjectTreeModel::BgdatPathRole).toString();auto level=levelSession(course_path,bgdat_path);if(level){if(sprites){const auto& values=level.value()->course().sprites;for(std::size_t i=0;i<values.size();++i)if(values[i].id==id){auto* item=new QTreeWidgetItem(results,{projectModel_->data(index,Qt::DisplayRole).toString(),QString::number(i),QString::number(values[i].x),QString::number(values[i].y),QString("0x%1").arg(values[i].settings,8,16,QLatin1Char('0'))});item->setData(0,Qt::UserRole,course_path);item->setData(0,Qt::UserRole+1,bgdat_path);item->setData(0,Qt::UserRole+2,static_cast<int>(i));}}else{const auto& values=level.value()->objects();for(std::size_t i=0;i<values.size();++i)if(values[i].object_id==id){auto* item=new QTreeWidgetItem(results,{projectModel_->data(index,Qt::DisplayRole).toString(),QString::number(i),QString::number(values[i].x),QString::number(values[i].y),{}});item->setData(0,Qt::UserRole,course_path);item->setData(0,Qt::UserRole+1,bgdat_path);item->setData(0,Qt::UserRole+2,static_cast<int>(i));}}}}scan(index);}};scan({});
    connect(results,&QTreeWidget::itemDoubleClicked,&dialog,[&](QTreeWidgetItem* item){const auto index=findLevelIndex(item->data(0,Qt::UserRole).toString(),item->data(0,Qt::UserRole+1).toString());if(index.isValid()){openLevelEditor(index);levelProperties_->select(sprites?LevelPropertiesWidget::Kind::Sprite:LevelPropertiesWidget::Kind::Object,item->data(0,Qt::UserRole+2).toInt());if(auto* view=activeLevelView())view->setSelection(sprites?2:1,item->data(0,Qt::UserRole+2).toInt(),-1);}dialog.accept();});dialog.exec();
}

void MainWindow::showSpriteEvents() {
    if(!currentLevel_){QMessageBox::information(this,"Sprite Events","Open an editable level first.");return;}std::unordered_map<std::uint16_t,QString> names;auto resources=nsmb::project::resolve_level_graphics(effectiveFiles_,currentLevel_->tileset_references());if(resources)for(const auto& actor:resources.value().actors){const auto* definition=actorDefinitions_.by_object_id(actor.object_id);names.emplace(actor.type,QString::fromStdString(definition?definition->sprite.name:actor.name));}
    QDialog dialog(this);dialog.setWindowTitle("Sprite Event Viewer");dialog.resize(620,420);auto* layout=new QVBoxLayout(&dialog);auto* table=new QTreeWidget(&dialog);table->setHeaderLabels({"Event","Sprite","Type","Name"});for(std::size_t i=0;i<currentLevel_->course().sprites.size();++i){const auto& sprite=currentLevel_->course().sprites[i];for(const auto event:sprite.events)if(event){auto* item=new QTreeWidgetItem(table,{QString::number(event),QString::number(i),QString::number(sprite.id),names.contains(sprite.id)?names.at(sprite.id):QString("Actor %1").arg(sprite.id)});item->setData(0,Qt::UserRole,static_cast<int>(i));}}table->sortItems(0,Qt::AscendingOrder);layout->addWidget(table);auto* close=new QDialogButtonBox(QDialogButtonBox::Close,&dialog);layout->addWidget(close);connect(close,&QDialogButtonBox::rejected,&dialog,&QDialog::reject);connect(table,&QTreeWidget::itemDoubleClicked,&dialog,[&](QTreeWidgetItem* item){const auto index=item->data(0,Qt::UserRole).toInt();levelProperties_->select(LevelPropertiesWidget::Kind::Sprite,index);if(auto* view=activeLevelView())view->setSelection(2,index,-1);});dialog.exec();
}

void MainWindow::exportPatch() {
    if(currentProject_.directory().empty()||!saveProject())return;std::vector<nsmb::project::NsmbPatchEntry> entries;for(const auto& file:filesystemView_.entries)if(file.status!=nsmb::project::FilesystemStatus::Vanilla&&file.status!=nsmb::project::FilesystemStatus::Orphaned&&!file.source.empty()){auto data=nsmb::core::read_file(file.source);if(!data){QMessageBox::warning(this,"Export Patch",QString::fromStdString(data.error().message));return;}entries.push_back({file.path,file.id?static_cast<std::int32_t>(*file.id):-1,std::move(data.value())});}if(entries.empty()){QMessageBox::information(this,"Export Patch","There are no module changes to distribute.");return;}auto encoded=nsmb::project::encode_nsmbpatch(entries);if(!encoded){QMessageBox::warning(this,"Export Patch",QString::fromStdString(encoded.error().message));return;}const auto path=QFileDialog::getSaveFileName(this,"Export Patch",QString::fromStdString(currentProject_.name())+".nsmbpatch","Big Star patches (*.nsmbpatch)");if(path.isEmpty())return;auto written=nsmb::core::write_file_atomically(path.toStdString(),encoded.value());if(!written)QMessageBox::warning(this,"Export Patch",QString::fromStdString(written.error().message));
}

void MainWindow::importPatch() {
    if(currentProject_.directory().empty())return;const auto path=QFileDialog::getOpenFileName(this,"Import Patch",{},"Big Star patches (*.nsmbpatch)");if(path.isEmpty())return;auto bytes=nsmb::core::read_file(path.toStdString());if(!bytes){QMessageBox::warning(this,"Import Patch",QString::fromStdString(bytes.error().message));return;}auto patch=nsmb::project::decode_nsmbpatch(bytes.value());if(!patch){QMessageBox::warning(this,"Import Patch",QString::fromStdString(patch.error().message));return;}
    struct Write{std::filesystem::path target,backup;const nsmb::project::NsmbPatchEntry* entry{};bool existed{};};std::vector<Write> writes;bool id_warning=false;const auto backup_root=newBackupDirectory(currentProject_.directory(),"import");
    for(const auto& entry:patch.value()){const auto* current=filesystemView_.find(entry.path);if(current&&current->id&&entry.expected_file_id>=0&&*current->id!=entry.expected_file_id)id_warning=true;const auto module_name=current&&!current->module.empty()?current->module:activeModule_.toStdString();const auto* module=moduleGraph_.find(module_name);if(!module){QMessageBox::warning(this,"Import Patch","Choose an active module in the Filesystem dock first.");return;}auto target=nsmb::project::module_file_target(*module,activeVariant_.toStdString(),entry.path,activeModuleMappings());if(!target){QMessageBox::warning(this,"Import Patch",QString::fromStdString(target.error().message));return;}writes.push_back({target.value(),backup_root/std::filesystem::path(entry.path),&entry});}
    if(id_warning&&QMessageBox::warning(this,"Import Patch","One or more file IDs differ from the patch's source ROM. Apply by path anyway?",QMessageBox::Yes|QMessageBox::No,QMessageBox::No)!=QMessageBox::Yes)return;
    std::error_code ec;for(auto& write:writes){std::filesystem::create_directories(write.target.parent_path(),ec);if(ec){QMessageBox::warning(this,"Import Patch","Could not create a module directory.");return;}write.existed=std::filesystem::is_regular_file(write.target,ec);if(write.existed){std::filesystem::create_directories(write.backup.parent_path(),ec);std::filesystem::copy_file(write.target,write.backup,std::filesystem::copy_options::overwrite_existing,ec);if(ec){QMessageBox::warning(this,"Import Patch","Could not back up an existing module file.");return;}}}
    std::size_t written=0;for(;written<writes.size();++written){auto result=nsmb::core::write_file_atomically(writes[written].target,writes[written].entry->contents);if(!result){for(std::size_t i=0;i<written;++i){if(writes[i].existed)std::filesystem::copy_file(writes[i].backup,writes[i].target,std::filesystem::copy_options::overwrite_existing,ec);else std::filesystem::remove(writes[i].target,ec);}QMessageBox::warning(this,"Import Patch",QString::fromStdString(result.error().message));return;}}
    sweepBackups(currentProject_.directory());
    reloadFilesystemView();log(QString("Imported %1 patch file(s) transactionally.").arg(patch.value().size()));
}

void MainWindow::runXdelta(bool create) {
    auto executable=QStandardPaths::findExecutable("xdelta3");if(executable.isEmpty())executable=QFileDialog::getOpenFileName(this,"Select xdelta3 Executable");if(executable.isEmpty())return;QStringList inputs;if(create){inputs<<QFileDialog::getOpenFileName(this,"Select Clean ROM",{},"Nintendo DS ROMs (*.nds)")<<QFileDialog::getOpenFileName(this,"Select Modified ROM",{},"Nintendo DS ROMs (*.nds)")<<QFileDialog::getSaveFileName(this,"Save xdelta Patch",{},"xdelta patches (*.xdelta)");}else{inputs<<QFileDialog::getOpenFileName(this,"Select Clean ROM",{},"Nintendo DS ROMs (*.nds)")<<QFileDialog::getOpenFileName(this,"Select xdelta Patch",{},"xdelta patches (*.xdelta)")<<QFileDialog::getSaveFileName(this,"Save Patched ROM",{},"Nintendo DS ROMs (*.nds)");}if(std::ranges::any_of(inputs,[](const auto& value){return value.isEmpty();}))return;nsmb::patching::ProcessRequest request;request.executable=executable.toStdString();request.arguments=create?std::vector<std::string>{"-e","-f","-s",inputs[0].toStdString(),inputs[1].toStdString(),inputs[2].toStdString()}:std::vector<std::string>{"-d","-f","-s",inputs[0].toStdString(),inputs[1].toStdString(),inputs[2].toStdString()};QString errors;WaitCursor wait;auto result=nsmb::patching::run_process(request,{},[&](std::string_view line){errors+=QString::fromUtf8(line.data(),static_cast<qsizetype>(line.size()))+'\n';});if(!result||result.value().exit_code!=0)QMessageBox::warning(this,"xdelta3",result?errors:QString::fromStdString(result.error().message));else QMessageBox::information(this,"xdelta3",create?"Patch created successfully.":"Patched ROM created successfully.");
}

void MainWindow::writeRecoverySnapshots() {
    if (currentProject_.directory().empty()) return;
    const auto root = currentProject_.directory() / ".big-star-autosave";
    std::error_code ec; std::filesystem::create_directories(root, ec);
    if (ec) { log("Autosave: could not create recovery directory."); return; }
    std::unordered_set<std::string> keep;
    for (const auto& [key, level] : levelSessions_) {
        if (!level->dirty()) continue;
        // A snapshot records the two paths; restoring re-keys them against
        // whatever variant is active then.
        const auto paths = documentKeyPaths(key); if (!paths) continue;
        auto course = level->serialize_course(), bgdat = level->serialize_bgdat(); if (!course || !bgdat) continue;
        nsmb::project::LevelRecoverySnapshot snapshot{paths->course, paths->bgdat,
            {0, 0, std::move(course.value()), std::move(bgdat.value())}};
        auto encoded = nsmb::project::encode_level_recovery(snapshot); if (!encoded) continue;
        const auto key_bytes = QByteArray(key.data(), static_cast<qsizetype>(key.size()));
        const auto name = key_bytes.toBase64(QByteArray::Base64UrlEncoding | QByteArray::OmitTrailingEquals).toStdString() + ".bsrecover";
        keep.insert(name); auto result = nsmb::core::write_file_atomically(root / name, encoded.value());
        if (!result) log("Autosave: " + QString::fromStdString(result.error().message));
    }
    for (const auto& entry : std::filesystem::directory_iterator(root, ec))
        if (entry.is_regular_file() && entry.path().extension() == ".bsrecover" && !keep.contains(entry.path().filename().string()))
            std::filesystem::remove(entry.path(), ec);
}

void MainWindow::restoreRecoverySnapshots() {
    if (currentProject_.directory().empty()) return;
    const auto root = currentProject_.directory() / ".big-star-autosave"; std::error_code ec;
    if (!std::filesystem::is_directory(root, ec)) return;
    std::vector<nsmb::project::LevelRecoverySnapshot> snapshots;
    for (const auto& entry : std::filesystem::directory_iterator(root, ec)) if (entry.is_regular_file() && entry.path().extension() == ".bsrecover") {
        auto bytes = nsmb::core::read_file(entry.path()); if (!bytes) { log("Recovery: " + QString::fromStdString(bytes.error().message)); continue; }
        auto decoded = nsmb::project::decode_level_recovery(bytes.value()); if (!decoded) { log("Recovery: " + QString::fromStdString(decoded.error().message)); continue; }
        snapshots.push_back(std::move(decoded.value()));
    }
    if (snapshots.empty()) return;
    if (QMessageBox::question(this, "Recover Autosaved Levels",
                              QString("Big Star found %1 autosaved level(s) from an interrupted or unsaved session. Recover them?").arg(snapshots.size()),
                              QMessageBox::Yes | QMessageBox::No, QMessageBox::Yes) == QMessageBox::No) { clearRecoverySnapshots(); return; }
    int restored = 0;
    for (auto& snapshot : snapshots) {
        auto session = levelSession(QString::fromStdString(snapshot.course_path), QString::fromStdString(snapshot.bgdat_path));
        if (!session) { log("Recovery: " + QString::fromStdString(session.error().message)); continue; }
        auto result = session.value()->replace_serialized(snapshot.package.course, snapshot.package.bgdat, &moduleData_, "Recover autosave");
        if (!result) { log("Recovery: " + QString::fromStdString(result.error().message)); continue; }
        ++restored;
    }
    if (restored) log(QString("Recovered %1 autosaved level(s). Save the project to keep them.").arg(restored));
    updateEditState();
}

void MainWindow::clearRecoverySnapshots() {
    if (currentProject_.directory().empty()) return;
    const auto root = currentProject_.directory() / ".big-star-autosave"; std::error_code ec;
    if (!std::filesystem::is_directory(root, ec)) return;
    for (const auto& entry : std::filesystem::directory_iterator(root, ec))
        if (entry.is_regular_file() && entry.path().extension() == ".bsrecover") std::filesystem::remove(entry.path(), ec);
}

void MainWindow::openTilesetEditor() {
    if (!currentLevel_) {
        QMessageBox::information(this, "Tilesets and Backgrounds", "Open an editable level first.");
        return;
    }
    auto resources = nsmb::project::resolve_level_graphics(effectiveFiles_, currentLevel_->tileset_references());
    if (!resources) { QMessageBox::warning(this, "Tilesets and Backgrounds", QString::fromStdString(resources.error().message)); return; }
    // The resources arrive with their bytes: resolving them is one read each,
    // through whatever supplies them, rather than a path the caller re-opens.
    std::array<nsmb::formats::TilesetResourceData, 3> source_tilesets;
    std::array<nsmb::formats::EditableTileset, 3> tilesets;
    for (std::size_t slot = 0; slot < tilesets.size(); ++slot) {
        const auto& files = resources.value().tilesets[slot];
        source_tilesets[slot]={files.graphics.data,files.palette.data,files.map16.data,files.objects.data,files.object_index.data,files.behaviors.data};
        auto parsed=nsmb::formats::parse_tileset(source_tilesets[slot],files.tile_offset,files.palette_offset,files.tile_page_offset);
        if(!parsed){QMessageBox::warning(this,"Tilesets and Backgrounds",QString::fromStdString(parsed.error().message));return;}tilesets[slot]=std::move(parsed.value());
    }
    auto load_map=[&](const nsmb::project::BackgroundResourceFiles& files,nsmb::formats::TilemapResourceData& source)->nsmb::core::Result<nsmb::formats::EditableTilemap>{source={files.graphics.data,files.palette.data,files.screen.data};return nsmb::formats::parse_tilemap(source);};
    nsmb::formats::TilemapResourceData source_background,source_foreground;
    auto background=load_map(resources.value().background,source_background),foreground=load_map(resources.value().foreground,source_foreground);
    if(!background||!foreground){QMessageBox::warning(this,"Tilesets and Backgrounds",QString::fromStdString(!background?background.error().message:foreground.error().message));return;}
    TilesetEditorDialog dialog(std::move(tilesets),std::move(background.value()),std::move(foreground.value()),currentLevel_->tileset_references(),tileOverlays_,this);
    if(dialog.exec()!=QDialog::Accepted)return;
    std::array<nsmb::formats::TilesetResourceData,3> encoded_tilesets;
    for(std::size_t slot=0;slot<encoded_tilesets.size();++slot){auto value=dialog.tilesetData(slot);if(!value){QMessageBox::warning(this,"Save tileset",QString::fromStdString(value.error().message));return;}encoded_tilesets[slot]=std::move(value.value());}
    auto encoded_background=dialog.backgroundData(),encoded_foreground=dialog.foregroundData();
    if(!encoded_background||!encoded_foreground){QMessageBox::warning(this,"Save backgrounds",QString::fromStdString(!encoded_background?encoded_background.error().message:encoded_foreground.error().message));return;}
    struct Write { std::string rom_path; std::vector<std::uint8_t> bytes; std::filesystem::path target; };
    std::vector<Write> writes;
    auto queue=[&](const nsmb::project::ResourceFile& resource,const std::vector<std::uint8_t>& before,std::vector<std::uint8_t> after)->nsmb::core::Result<void>{if(before==after)return{};if(resource.empty())return nsmb::core::make_error(nsmb::core::ErrorCode::InvalidArgument,"edited resource has no ROM file target");auto target=moduleSaveTarget(resource.rom_path);if(!target)return target.error();writes.push_back({resource.rom_path,std::move(after),std::move(target.value())});return{};};
    auto queue_tileset=[&](std::size_t slot)->nsmb::core::Result<void>{const auto& paths=resources.value().tilesets[slot];const auto& before=source_tilesets[slot];auto& after=encoded_tilesets[slot];if(auto r=queue(paths.graphics,before.graphics,std::move(after.graphics));!r)return r;if(auto r=queue(paths.palette,before.palette,std::move(after.palette));!r)return r;if(auto r=queue(paths.map16,before.map16,std::move(after.map16));!r)return r;if(auto r=queue(paths.objects,before.objects,std::move(after.objects));!r)return r;if(auto r=queue(paths.object_index,before.object_index,std::move(after.object_index));!r)return r;return queue(paths.behaviors,before.behaviors,std::move(after.behaviors));};
    for(std::size_t slot=0;slot<3;++slot)if(auto result=queue_tileset(slot);!result){QMessageBox::warning(this,"Save tileset",QString::fromStdString(result.error().message));return;}
    auto queue_map=[&](const nsmb::project::BackgroundResourceFiles& paths,const nsmb::formats::TilemapResourceData& before,nsmb::formats::TilemapResourceData after)->nsmb::core::Result<void>{if(auto r=queue(paths.graphics,before.graphics,std::move(after.graphics));!r)return r;if(auto r=queue(paths.palette,before.palette,std::move(after.palette));!r)return r;return queue(paths.screen,before.screen,std::move(after.screen));};
    if(auto result=queue_map(resources.value().background,source_background,std::move(encoded_background.value()));!result){QMessageBox::warning(this,"Save background",QString::fromStdString(result.error().message));return;}
    if(auto result=queue_map(resources.value().foreground,source_foreground,std::move(encoded_foreground.value()));!result){QMessageBox::warning(this,"Save foreground",QString::fromStdString(result.error().message));return;}
    for(auto& write:writes){std::error_code ec;std::filesystem::create_directories(write.target.parent_path(),ec);if(ec){QMessageBox::warning(this,"Save graphics","Could not create the module target directory.");return;}auto result=nsmb::core::write_file_atomically(write.target,write.bytes);if(!result){QMessageBox::warning(this,"Save graphics",QString::fromStdString(result.error().message));return;}}
    if(dialog.references()!=currentLevel_->tileset_references())if(auto result=currentLevel_->set_tileset_references(dialog.references());!result){QMessageBox::warning(this,"Save slots",QString::fromStdString(result.error().message));return;}
    reloadFilesystemView();refreshGraphics();levelProperties_->refresh();updateEditState();log(QString("Saved %1 edited graphics resource(s) to module targets.").arg(writes.size()));
}

// Opens any file the editors understand straight off disk, the way NSMB Editor's own file
// list does, and writes it back where it came from. Files inside the project's NitroFS are
// better opened from the filesystem dock, which routes the save through the module target.
// A level row names two files the Filesystem tab also lists, so it can hand the
// user over to them rather than making them go and find the name themselves.
void MainWindow::showLevelTreeMenu(const QPoint& position) {
    const auto index = levelTree_->indexAt(position);
    if (!index.isValid()) return;
    if (projectModel_->data(index, ProjectTreeModel::NodeKindRole).toInt() != ProjectTreeModel::AreaNode) return;

    QMenu menu(levelTree_);
    menu.setObjectName("LevelTreeMenu");
    auto* open = menu.addAction("Open Level", this, [this, index] { openLevelEditor(index); });
    open->setObjectName("LevelTreeOpen");
    open->setEnabled(projectModel_->data(index, ProjectTreeModel::AvailableRole).toBool());
    menu.addSeparator();

    for (const auto& [label, name, role] :
         std::array{std::tuple{"Show Course in Filesystem", "LevelTreeShowCourse", ProjectTreeModel::CoursePathRole},
                    std::tuple{"Show BGDat in Filesystem", "LevelTreeShowBgdat", ProjectTreeModel::BgdatPathRole}}) {
        const auto path = projectModel_->data(index, role).toString();
        auto* action = menu.addAction(label, this, [this, path] { showInFilesystem(path); });
        action->setObjectName(name);
        action->setEnabled(!path.isEmpty());
    }
    menu.exec(levelTree_->viewport()->mapToGlobal(position));
}

void MainWindow::showInFilesystem(const QString& romPath) {
    if (!filesystemDock_ || !filesystemPanel_) return;
    filesystemDock_->show();
    filesystemDock_->raise();
    if (!filesystemPanel_->revealPath(romPath))
        log("Filesystem: " + romPath + " is not listed in the " +
            (activeVariant_.isEmpty() ? QString("base") : activeVariant_) + " variant.");
}

void MainWindow::openFileInEditor() {
    const auto start = currentProject_.directory().empty() ? QString{} : QString::fromStdString(currentProject_.directory().string());
    const auto source = QFileDialog::getOpenFileName(this, "Open File", start);
    if (source.isEmpty()) return;
    auto data = nsmb::core::read_file(source.toStdString());
    if (!data) { log("Open File: " + QString::fromStdString(data.error().message)); return; }

    // Palettes, graphics and tilemaps are separate files, so the editor is
    // offered this file's neighbours to pair it with.
    bse::editors::Companions companions;
    const QFileInfo chosen(source);
    for (const auto& sibling : QDir(chosen.absolutePath()).entryInfoList(QDir::Files, QDir::Name))
        if (sibling.absoluteFilePath() != chosen.absoluteFilePath())
            companions.paths << sibling.absoluteFilePath();
    companions.load = [](const QString& file) -> nsmb::core::Result<std::vector<std::uint8_t>> {
        auto bytes = nsmb::core::read_file(file.toStdString());
        if (!bytes) return bytes.error();
        return bse::editors::unwrapped(bytes.value());
    };

    const auto path = source.toStdString();
    openFileDocument("file\n" + source, chosen.fileName(), std::move(data.value()), false, std::move(companions),
                     [this, path](const std::vector<std::uint8_t>& bytes) -> nsmb::core::Result<void> {
                         auto written = nsmb::core::write_file_atomically(path, bytes);
                         if (!written) return written.error();
                         refreshGraphics();
                         return {};
                     });
}

void MainWindow::openFileDocument(const QString& key, const QString& subtitle, std::vector<std::uint8_t> bytes,
                                  bool romUserMetadata, bse::editors::Companions companions,
                                  std::function<nsmb::core::Result<void>(const std::vector<std::uint8_t>&)> save) {
    if (raiseFileDocument(key)) return;
    auto* panel = bse::editors::makePanel(nullptr, subtitle.toStdString(), bytes, romUserMetadata, std::move(companions));
    if (!panel) {
        QMessageBox::information(this, "Open File", "No editor understands " + subtitle + ".");
        return;
    }
    hostFileDocument(key, subtitle, panel, std::move(bytes), std::move(save));
}

// True when the file already has a tab, which is then brought forward.
bool MainWindow::raiseFileDocument(const QString& key) {
    const auto found = fileDocuments_.find(key.toStdString());
    if (found == fileDocuments_.end()) return false;
    if (const auto index = tabs_->indexOf(found->second.page); index >= 0) { tabs_->setCurrentIndex(index); return true; }
    fileDocuments_.erase(found);
    return false;
}

void MainWindow::hostFileDocument(const QString& key, const QString& subtitle, bse::editors::EditorPanel* panel,
                                  std::vector<std::uint8_t> bytes,
                                  std::function<nsmb::core::Result<void>(const std::vector<std::uint8_t>&)> save,
                                  bool rewrap) {
    const auto identifier = key.toStdString();
    auto* page = new QWidget(tabs_);
    auto* layout = new QVBoxLayout(page);
    layout->setContentsMargins(0, 0, 0, 0);
    panel->setParent(page);
    layout->addWidget(panel, 1);
    auto* actions = new QHBoxLayout;
    auto* saveButton = new QPushButton("Save", page);
    saveButton->setObjectName("FileDocumentSave");
    saveButton->setEnabled(panel->usable());
    actions->addStretch(1);
    actions->addWidget(saveButton);
    layout->addLayout(actions);
    connect(saveButton, &QPushButton::clicked, this, [this, identifier] { saveFileDocument(identifier); });

    fileDocuments_[identifier] = FileDocument{panel->title(), page, panel, std::move(bytes), rewrap, std::move(save)};
    tabs_->setCurrentIndex(tabs_->addTab(page, subtitle));
    log("Opened " + subtitle + " in " + panel->title() + ".");
}

void MainWindow::saveFileDocument(const std::string& key) {
    const auto found = fileDocuments_.find(key);
    if (found == fileDocuments_.end()) return;
    auto edited = found->second.panel->editedBytes();
    if (edited && found->second.rewrap) edited = bse::editors::rewrapped(found->second.original, std::move(edited.value()));
    if (!edited) { QMessageBox::warning(this, "Save file", QString::fromStdString(edited.error().message)); return; }
    if (auto written = found->second.save(edited.value()); !written) {
        QMessageBox::warning(this, "Save file", QString::fromStdString(written.error().message));
        return;
    }
    found->second.original = std::move(edited.value());
    log("Saved " + found->second.title + ".");
}

void MainWindow::editRomBanner() {
    if (!currentProject_.rom()) {
        QMessageBox::information(this, "ROM Banner", "Open or import a ROM project first.");
        return;
    }

    std::filesystem::path target;
    if (patcherConfig_) {
        const auto variant = std::ranges::find(
            patcherConfig_->variants, activeVariant_.toStdString(), &nsmb::patching::BuildVariant::name);
        target = variant != patcherConfig_->variants.end() && !variant->banner.empty()
            ? variant->banner : patcherConfig_->rom_banner;
        if (target.empty()) {
            // `config dump` reports both the project banner and the variant's
            // override, so an empty answer means the project configures no
            // banner at all. Asking the user to point at a file would invent a
            // source no build reads; the fix belongs in ncpatcher.yaml.
            QMessageBox::information(this, "ROM Banner",
                "This project configures no rom.banner, so there is nowhere for an edited "
                "banner to go. Add one to ncpatcher.yaml and the editor will write to it.");
            return;
        }
        if (target.is_relative()) target = currentProject_.directory() / target;
    } else {
        QMessageBox::information(this, "ROM Banner",
            "NCPatcher has not reported this project's configuration yet, so there is nowhere "
            "for an edited banner to go.");
        return;
    }

    // The configured banner once it exists, and otherwise the one in the ROM,
    // which is what the game is showing today.
    auto bytes = std::filesystem::is_regular_file(target)
                     ? nsmb::core::read_file(target)
                     : (effectiveFiles_.rom() ? effectiveFiles_.rom()->read_banner()
                                              : nsmb::core::Result<std::vector<std::uint8_t>>(nsmb::core::make_error(
                                                    nsmb::core::ErrorCode::NotFound, "no built ROM is open")));
    if (!bytes) {
        QMessageBox::warning(this, "ROM Banner", QString::fromStdString(bytes.error().message));
        return;
    }
    UiResourceEditorDialog dialog("banner.bin", bytes.value(), false, this);
    if (dialog.exec() != QDialog::Accepted) return;
    auto encoded = dialog.editedBytes();
    if (!encoded) {
        QMessageBox::warning(this, "ROM Banner", QString::fromStdString(encoded.error().message));
        return;
    }
    std::error_code ec;
    std::filesystem::create_directories(target.parent_path(), ec);
    if (ec) {
        QMessageBox::warning(this, "ROM Banner", "Could not create the configured banner directory.");
        return;
    }
    auto saved = writeBinaryFileAtomically(target, encoded.value());
    if (!saved) {
        QMessageBox::warning(this, "ROM Banner", QString::fromStdString(saved.error().message));
        return;
    }
    log("Saved ROM banner: " + QString::fromStdString(target.string()));
}

void MainWindow::editRomUserMetadata() {
    const auto found = std::ranges::find_if(filesystemView_.entries, [](const auto& entry) {
        return entry.id && *entry.id == 131;
    });
    if (found == filesystemView_.entries.end()) {
        QMessageBox::information(this, "ROM User Metadata", "NitroFS file ID 131 is not present in this ROM.");
        return;
    }
    auto bytes = effectiveFiles_.read(found->path);
    if (!bytes) {
        QMessageBox::warning(this, "ROM User Metadata", QString::fromStdString(bytes.error().message));
        return;
    }
    UiResourceEditorDialog dialog(found->path, bytes.value(), true, this);
    if (dialog.exec() != QDialog::Accepted) return;
    auto encoded = dialog.editedBytes();
    if (!encoded) {
        QMessageBox::warning(this, "ROM User Metadata", QString::fromStdString(encoded.error().message));
        return;
    }

    std::filesystem::path target;
    if (!moduleGraph_.modules.empty()) {
        std::string key = found->module.empty() ? activeModule_.toStdString() : found->module;
        const auto* module = moduleGraph_.find(key);
        if (!module) {
            QMessageBox::information(this, "ROM User Metadata", "Choose an active module in the Filesystem dock before saving.");
            return;
        }
        auto resolved = nsmb::project::module_file_target(
            *module, activeVariant_.toStdString(), found->path, activeModuleMappings());
        if (!resolved) {
            QMessageBox::warning(this, "ROM User Metadata", QString::fromStdString(resolved.error().message));
            return;
        }
        target = std::move(resolved.value());
    }
    std::error_code ec;
    std::filesystem::create_directories(target.parent_path(), ec);
    if (ec) {
        QMessageBox::warning(this, "ROM User Metadata", "Could not create the metadata target directory.");
        return;
    }
    auto saved = nsmb::core::write_file_atomically(target, encoded.value());
    if (!saved) {
        QMessageBox::warning(this, "ROM User Metadata", QString::fromStdString(saved.error().message));
        return;
    }
    reloadFilesystemView();
    log("Saved ROM user metadata: " + QString::fromStdString(target.string()));
}

// Delegated on purpose. NCPatcher's `rom extract` writes a directory NCPatcher
// itself can read back; a second extractor in the editor produced a tree that
// looked right and that the patcher could not open.
void MainWindow::extractRom() {
    if (currentProject_.directory().empty() || !currentProject_.rom()) {
        QMessageBox::information(this, "Extract ROM", "Open or import a ROM project first.");
        return;
    }
    const auto rom = projectRomPath(currentProject_);
    std::error_code ec;
    if (!std::filesystem::is_regular_file(rom, ec)) {
        QMessageBox::information(this, "Extract ROM",
            "This project has no built ROM yet. Run NCPatcher first.");
        return;
    }
    auto executable = patcherPanel_ && !patcherPanel_->executable().empty()
        ? QString::fromStdString(patcherPanel_->executable().string())
        : QStandardPaths::findExecutable("ncpatcher");
    if (executable.isEmpty()) {
        executable = QFileDialog::getOpenFileName(this, "Select NCPatcher Executable");
        if (executable.isEmpty()) return;
    }
    const auto directory = QFileDialog::getExistingDirectory(this, "Extract ROM Into");
    if (directory.isEmpty()) {
        log("Extract ROM cancelled before choosing a folder.");
        return;
    }

    nsmb::patching::NCPatcherClient client(executable.toStdString());
    nsmb::core::Result<nsmb::patching::RunSummary> extracted =
        nsmb::core::make_error(nsmb::core::ErrorCode::IoError, "extraction did not start");
    {
        WaitCursor cursor;
        extracted = client.extract_rom(currentProject_.directory(), std::filesystem::path(directory.toStdString()), rom);
    }
    if (!extracted || !extracted.value().succeeded()) {
        const auto message = !extracted ? extracted.error().message
                                        : (extracted.value().diagnostics.empty() ? "NCPatcher refused the extraction."
                                                                                 : extracted.value().diagnostics.front().message);
        QMessageBox::warning(this, "Extract ROM Failed", QString::fromStdString(message));
        log("ROM extraction failed: " + QString::fromStdString(message));
        return;
    }
    log("Extracted ROM into " + directory);
    QMessageBox::information(this, "ROM Extracted", "NCPatcher wrote the ROM into " + directory + ".");
}

// One ROM, opened once and kept open. Everything that reads a ROM address goes
// through it, so re-pointing it is all a rebuild has to do here.
void MainWindow::reloadRom() {
    effectiveFiles_.set_rom({});
    const auto rom = projectRomPath(currentProject_);
    if (rom.empty()) return;
    std::error_code ec;
    if (!std::filesystem::is_regular_file(rom, ec)) {
        log("No built ROM at " + QString::fromStdString(rom.string()) + " yet.");
        return;
    }
    auto opened = nsmb::project::RomReader::open(rom);
    if (!opened) { log("ROM: " + QString::fromStdString(opened.error().message)); return; }
    effectiveFiles_.set_rom(std::move(opened.value()));
    if (romWatcher_) {
        if (!romWatcher_->files().isEmpty()) romWatcher_->removePaths(romWatcher_->files());
        romWatcher_->addPath(QString::fromStdString(rom.string()));
    }
}

void MainWindow::setProject(const nsmb::project::Project& project) {
    saveWorkspaceState();
    if (graphicsWatcher_ && !graphicsWatcher_->files().empty()) {
        graphicsWatcher_->removePaths(graphicsWatcher_->files());
    }
    levelProperties_->setLevel(nullptr);
    levelData_->setLevel(nullptr, &moduleData_);
    currentLevel_ = nullptr;
    clearDocuments();
    graphicsFiles_.clear();
    changedGraphicsFiles_.clear();
    levelSessions_.clear();
    currentProject_ = project;
    moduleGraph_ = {};
    patcherConfig_.reset();
    fileManifest_.reset();
    filePlan_.reset();
    filesystemView_ = {};
    effectiveFiles_.clear();
    activeModule_.clear();
    activeVariant_.clear();
    moduleData_={};
    spriteDefinitions_={};
    const auto generated=QSettings().value("NCPatcherGeneratedPath","build/generated").toString().toStdString();
    auto module_data=nsmb::project::load_module_data_catalog(currentProject_.directory(),generated);
    if(module_data){moduleData_=std::move(module_data.value());if(moduleData_.stage_object_id_base!=326)log(QString("Stage-object catalog base %1 disagrees with Big Star's vanilla count 326.").arg(moduleData_.stage_object_id_base));}
    else log("Module data: "+QString::fromStdString(module_data.error().message));
    levelProperties_->setSpriteDefinitions(spriteDefinitions_, {});
    levelData_->setLevel(nullptr, &moduleData_);
    levelProperties_->setObjectBankOverlays(std::nullopt);
    reloadRom();
    if(auto* rom=effectiveFiles_.rom()){
        const auto arm9=rom->read_arm9();
        if(arm9){const auto decoded=nsmb::formats::decompress_blz_overlay(arm9.value());if(decoded){auto table=nsmb::project::find_object_bank_overlays(decoded.value());if(table)levelProperties_->setObjectBankOverlays(table.value());}}
    }
    projectModel_->setProject(currentProject_);
    reloadFilesystemView();
    restoreRecoverySnapshots();
    if (levelTree_) {
        levelTree_->expandToDepth(1);
    }
    setWindowTitle(projectTitle(currentProject_));
    rememberRecentProject(currentProject_.directory());
    updateProjectSummary();
    restoreWorkspaceState();
    updateEditState();
}

std::vector<nsmb::project::ModuleVariantMapping> MainWindow::activeModuleMappings() const {
    // Straight out of `config dump`. The editor used to read these out of
    // ncpatcher.yaml with a hand-written YAML scanner, which is exactly the
    // kind of second opinion that can disagree with the build.
    if (!patcherConfig_) return {};
    const auto found = std::ranges::find(patcherConfig_->variants, activeVariant_.toStdString(), &nsmb::patching::BuildVariant::name);
    if (found == patcherConfig_->variants.end()) return {};
    std::vector<nsmb::project::ModuleVariantMapping> mappings;
    mappings.reserve(found->module_variants.size());
    for (const auto& [module, layer] : found->module_variants) mappings.push_back({module, layer});
    return mappings;
}

void MainWindow::setActiveVariant(const QString& variant) {
    if (activeVariant_ == variant) return;
    // Everything loaded through the old variant's layers is now wrong: the
    // sessions, their tabs, and the tileset graphics resolved alongside them.
    // Leaving them open is what made switching look like it did nothing.
    if (!documents_.empty() && !confirmDiscardLevelEdits()) {
        if (variantSelector_) {
            const QSignalBlocker blocker(variantSelector_);
            const auto index = variantSelector_->findData(activeVariant_);
            variantSelector_->setCurrentIndex(index >= 0 ? index : 0);
        }
        return;
    }
    const auto reopen = tabs_ && tabs_->currentWidget() ? activeLevelView() : nullptr;
    QString course, bgdat;
    for (auto& [key, document] : documents_) {
        (void)key;
        if (document.view == reopen) { course = document.course_path; bgdat = document.bgdat_path; }
    }

    activeVariant_ = variant;
    ncpatcherVariant_ = variant;
    if (patcherPanel_) patcherPanel_->setSelectedVariant(variant);
    clearDocuments();
    levelSessions_.clear();
    currentLevel_ = nullptr;
    levelProperties_->setLevel(nullptr);
    levelData_->setLevel(nullptr, &moduleData_);
    reloadFilesystemView();
    refreshPalette();
    updateEditState();

    if (!course.isEmpty()) {
        const auto index = findLevelIndex(course, bgdat);
        if (index.isValid()) openLevelEditor(index);
    }
}

void MainWindow::reloadFilesystemView() {
    if (!filesystemPanel_) return;
    if (moduleWatcher_ && !moduleWatcher_->directories().empty()) moduleWatcher_->removePaths(moduleWatcher_->directories());
    if (currentProject_.directory().empty() || !currentProject_.rom()) {
        moduleGraph_ = {}; fileManifest_.reset(); filePlan_.reset(); filesystemView_ = {};
        actorDefinitions_ = {}; spriteDefinitions_ = {};
        filesystemPanel_->setData({}, nullptr, {}, {}, {}, {}, {});
        return;
    }

    const auto configured = std::filesystem::path(QSettings().value("NCPatcherGeneratedPath", "build/generated").toString().toStdString());
    const auto generated = configured.is_absolute() ? configured : currentProject_.directory() / configured;
    const auto modulesFile = generated / "modules.json";
    const auto filesFile = generated / "files.json";
    const auto executable = patcherPanel_ && !patcherPanel_->executable().empty()
        ? QString::fromStdString(patcherPanel_->executable().string())
        : QStandardPaths::findExecutable("ncpatcher");
    if (moduleGraph_.modules.empty()) {
        std::error_code ec;
        if (std::filesystem::is_regular_file(modulesFile, ec)) {
            auto text = readTextFile(modulesFile);
            auto parsed = text ? nsmb::patching::parse_module_graph(text.value())
                               : nsmb::core::Result<nsmb::patching::ModuleGraph>(text.error());
            if (parsed) moduleGraph_ = std::move(parsed.value()); else log("Filesystem modules: " + QString::fromStdString(parsed.error().message));
        } else if (!executable.isEmpty()) {
            nsmb::patching::NCPatcherClient client(executable.toStdString());
            auto parsed = client.dump_modules(currentProject_.directory());
            if (parsed) moduleGraph_ = std::move(parsed.value()); else log("Filesystem modules: " + QString::fromStdString(parsed.error().message));
        }
        const auto localModules = currentProject_.directory() / "modules";
        if (std::filesystem::is_directory(localModules, ec)) moduleGraph_.dir = localModules;
        for (auto& module : moduleGraph_.modules) {
            const auto local = localModules / module.key;
            if (std::filesystem::is_directory(local, ec)) {
                module.dir = local; module.file = local / "module.yaml";
                if (module.nitrofs) module.nitrofs->dir = local / "nitrofs";
            }
        }
    }

    patcherConfig_.reset();
    if (!executable.isEmpty()) {
        nsmb::patching::NCPatcherClient client(executable.toStdString());
        auto parsed = client.dump_config(currentProject_.directory());
        if (parsed) patcherConfig_ = std::move(parsed.value());
        else log("Filesystem config: " + QString::fromStdString(parsed.error().message));
    }

    fileManifest_.reset();
    std::error_code ec;
    if (std::filesystem::is_regular_file(filesFile, ec)) {
        auto text = readTextFile(filesFile);
        auto parsed = text ? nsmb::patching::parse_file_manifest(text.value())
                           : nsmb::core::Result<nsmb::patching::FileManifest>(text.error());
        if (parsed) fileManifest_ = std::move(parsed.value());
        else log("Filesystem manifest: " + QString::fromStdString(parsed.error().message));
    }

    QStringList variants;
    if (patcherConfig_) for (const auto& variant : patcherConfig_->variants) variants.push_back(QString::fromStdString(variant.name));
    if (activeVariant_.isEmpty()) {
        activeVariant_ = !ncpatcherVariant_.isEmpty() ? ncpatcherVariant_
            : fileManifest_ && !fileManifest_->variant.empty() ? QString::fromStdString(fileManifest_->variant)
            : variants.isEmpty() ? QString{} : variants.front();
    }
    if (activeModule_.isEmpty()) for (const auto& module : moduleGraph_.modules) if (module.enabled && module.nitrofs) {
        activeModule_ = QString::fromStdString(module.key); break;
    }
    if (variantSelector_) {
        // Repopulating is bookkeeping, not a choice: blocked so it cannot come
        // back through setActiveVariant and reload everything again.
        const QSignalBlocker blocker(variantSelector_);
        variantSelector_->clear();
        variantSelector_->addItem("(none)", QString());
        for (const auto& name : variants) variantSelector_->addItem(name, name);
        auto index = variantSelector_->findData(activeVariant_);
        if (index < 0 && !activeVariant_.isEmpty()) {
            // A variant the project no longer lists is kept, labelled, rather
            // than quietly switching the work to a different language.
            variantSelector_->addItem(activeVariant_ + "  (not configured)", activeVariant_);
            index = variantSelector_->count() - 1;
        }
        variantSelector_->setCurrentIndex(std::max(0, index));
    }

    // Asked for once the variant is settled, because a plan is per variant.
    // This is the whole of what the view used to work out for itself: module
    // order, layering, `into:`, component subtraction, the archive-folder
    // convention. NCPatcher decides all of it, so the dock and the build can
    // no longer disagree.
    filePlan_.reset();
    if (!executable.isEmpty()) {
        nsmb::patching::NCPatcherClient client(executable.toStdString());
        auto planned = client.plan_files(currentProject_.directory(), activeVariant_.toStdString());
        if (planned) filePlan_ = std::move(planned.value());
        else log("Filesystem plan: " + QString::fromStdString(planned.error().message));
    }

    effectiveFiles_.set_plan(currentProject_.directory(), filePlan_);

    nsmb::project::FilesystemViewRequest request;
    request.rom = effectiveFiles_.rom();
    request.project_dir = currentProject_.directory();
    request.modules = moduleGraph_;
    request.plan = filePlan_;
    request.manifest = fileManifest_;
    request.variant = activeVariant_.toStdString();
    auto built = nsmb::project::build_filesystem_view(std::move(request));
    if (!built) { log("Filesystem view: " + QString::fromStdString(built.error().message)); filesystemView_ = {}; }
    else filesystemView_ = std::move(built.value());
    reloadActorDefinitions();
    filesystemPanel_->setData(currentProject_.directory(), &effectiveFiles_, moduleGraph_, filesystemView_, variants, activeModule_, activeVariant_, activeModuleMappings());
    if (moduleWatcher_) {
        QStringList watch; for (const auto& path : filesystemView_.watched_directories) if (std::filesystem::is_directory(path, ec)) watch.push_back(QString::fromStdString(path.string()));
        if (!watch.isEmpty()) moduleWatcher_->addPaths(watch);
    }
}

void MainWindow::reloadActorDefinitions() {
    // A module's Big Star contribution lives under `bigstar/`, one directory per
    // kind. The bare `actors/` beside it is the older spelling and is still read,
    // after it, so a project that predates `bigstar/` keeps working. There is no
    // layer in the user's config directory: an edit that lives there never
    // reaches anybody else on the project.
    std::vector<std::filesystem::path> roots{currentProject_.directory()};
    std::vector<std::string> names{"project"};
    for (const auto& module : moduleGraph_.modules)
        if (module.enabled && !module.dir.empty()) { roots.push_back(module.dir); names.push_back("module " + module.key); }

    std::vector<nsmb::project::LuaActorLayer> layers;
    std::vector<nsmb::project::LuaTileLayer> tileLayers;
    for (std::size_t i = 0; i < roots.size(); ++i) {
        layers.push_back({names[i], roots[i] / "bigstar" / "actors"});
        layers.push_back({names[i] + " (legacy)", roots[i] / "actors"});
        tileLayers.push_back({names[i], roots[i] / "bigstar" / "tiles"});
    }
    layers.push_back({"shipped", std::filesystem::path(BSE_SOURCE_DIR) / "data" / "actors"});
    tileLayers.push_back({"shipped", std::filesystem::path(BSE_SOURCE_DIR) / "data" / "tiles"});

    // Scripts name a ROM file by its full path under the filesystem root --
    // `polygon_unit/I_dokan.nsbtx` -- because two directories hold files of the
    // same name and a bare name cannot say which is meant.
    auto resolve = [this](auto, std::string_view name) -> nsmb::core::Result<nsmb::project::ActorResource> {
        const auto* found = filesystemView_.find(name);
        if (!found)
            return nsmb::core::make_error(nsmb::core::ErrorCode::NotFound, "resource file is missing: " + std::string(name));
        // Read through the same object the canvas reads through, which is what
        // finally makes an actor's artwork follow a pending module edit.
        auto data = effectiveFiles_.read(found->path);
        if (!data) return data.error();
        return nsmb::project::ActorResource{found->path, found->source, std::move(data.value())};
    };

    // A module actor spells its identity as the name its own build generates --
    // `coop.CoopFlagActor.Default` -- rather than repeating a hash that moves
    // whenever the object is renamed.
    auto stageObject = [this](std::string_view name) -> std::optional<nsmb::project::StageObjectIdentity> {
        const auto found = std::ranges::find(moduleData_.stage_objects, name, &nsmb::project::StageObjectDeclaration::name);
        if (found == moduleData_.stage_objects.end()) return std::nullopt;
        return nsmb::project::StageObjectIdentity{found->hash, found->object_id};
    };

    auto overlays = nsmb::project::load_lua_tile_layers(tileLayers, resolve);
    if (!overlays) {
        log("Tile scripts: " + QString::fromStdString(overlays.error().message));
        tileOverlays_.reset();
    } else {
        for (const auto& script : overlays.value().scripts)
            for (const auto& problem : script.errors) log("Tile scripts: " + QString::fromStdString(problem));
        tileOverlays_ = std::make_shared<const nsmb::project::TileOverlayCatalog>(std::move(overlays.value()));
    }

    auto definitions = nsmb::project::load_lua_actor_layers(layers, resolve, [](std::span<const std::uint8_t> bytes) -> nsmb::core::Result<nsmb::formats::RgbaImage> {
        // nsmb-project carries no image codec, so the Qt layer supplies one for
        // the stills that ship beside actor scripts.
        QImage decoded;
        if (!decoded.loadFromData(QByteArray(reinterpret_cast<const char*>(bytes.data()),
                                             static_cast<qsizetype>(bytes.size()))))
            return nsmb::core::make_error(nsmb::core::ErrorCode::ParseError, "could not decode the actor image");
        decoded = decoded.convertToFormat(QImage::Format_RGBA8888);
        nsmb::formats::RgbaImage image;
        image.width = static_cast<std::size_t>(decoded.width());
        image.height = static_cast<std::size_t>(decoded.height());
        image.pixels.resize(image.width * image.height * 4U);
        for (std::size_t y = 0; y < image.height; ++y)
            std::copy_n(decoded.constScanLine(static_cast<int>(y)), image.width * 4U,
                        image.pixels.begin() + static_cast<std::ptrdiff_t>(y * image.width * 4U));
        return image;
    }, stageObject);
    if (!definitions) {
        log("Lua actors: " + QString::fromStdString(definitions.error().message));
        actorDefinitions_ = {}; spriteDefinitions_ = {};
    } else {
        actorDefinitions_ = std::move(definitions.value());
        spriteDefinitions_ = actorDefinitions_.sprite_definitions();
    }
    levelProperties_->setSpriteDefinitions(spriteDefinitions_, {});
    if (preview_) { preview_->setActorDefinitions(actorDefinitions_); preview_->setTileOverlays(tileOverlays_); }
    for (auto& [key, document] : documents_) {
        (void)key;
        document.view->setActorDefinitions(actorDefinitions_);
        document.view->setTileOverlays(tileOverlays_);
    }
}

nsmb::core::Result<std::filesystem::path> MainWindow::moduleSaveTarget(const std::string& romPath) const {
    std::string moduleKey = activeModule_.toStdString();
    if (const auto* entry = filesystemView_.find(romPath); entry && !entry->module.empty()) moduleKey = entry->module;
    const auto* module = moduleGraph_.find(moduleKey);
    if (!module) return nsmb::core::make_error(nsmb::core::ErrorCode::InvalidArgument, "choose an active module in the Filesystem dock before saving");
    return nsmb::project::module_file_target(*module, activeVariant_.toStdString(), romPath, activeModuleMappings());
}

void MainWindow::updateProjectSummary() {
    if (!preview_ || !propertiesLabel_) {
        return;
    }

    preview_->showProjectSummary(currentProject_, projectModel_->levelCount(), projectModel_->availableLevelCount());
    currentLevel_ = nullptr;
    levelProperties_->setLevel(nullptr);
    levelData_->setLevel(nullptr, &moduleData_);

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
    if (kind != ProjectTreeModel::AreaNode) {
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
    levelData_->setLevel(nullptr, &moduleData_);
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
    if (kind != ProjectTreeModel::AreaNode) {
        return;
    }
    const auto course_path = projectModel_->data(index, ProjectTreeModel::CoursePathRole).toString();
    const auto bgdat_path = projectModel_->data(index, ProjectTreeModel::BgdatPathRole).toString();
    const auto key = documentKey(course_path, bgdat_path);
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
        if ((tabs_->currentWidget() == view || view->window()->isActiveWindow()) && kind >= 0 && kind <= 7) {
            levelProperties_->select(static_cast<LevelPropertiesWidget::Kind>(kind), item, node);
        }
    });
    connect(view, &LevelPreviewWidget::multiSelectionRequested, this, [this, view](int kind, const QList<int>& items) {
        if ((tabs_->currentWidget() == view || view->window()->isActiveWindow()) && kind >= 0 && kind <= 7 && !items.empty()) {
            levelProperties_->selectMany(static_cast<LevelPropertiesWidget::Kind>(kind), items);
        }
    });
    connect(view, &LevelPreviewWidget::interactionPreviewRequested, this, [this, view](const QPointF& delta, int handle) {
        if (tabs_->currentWidget() == view || view->window()->isActiveWindow()) levelProperties_->previewInteraction(delta, handle);
    });
    connect(view, &LevelPreviewWidget::kindShortcutRequested, this, [this, view](int kind) {
        if (view->level() != currentLevel_) return;
        levelProperties_->select(static_cast<LevelPropertiesWidget::Kind>(kind), 0);
        propertiesDock_->show(); propertiesDock_->raise();
    });
    connect(view, &LevelPreviewWidget::levelChanged, this, [this] {
        levelProperties_->refresh();
        levelData_->refresh();
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
    for (auto it = fileDocuments_.begin(); it != fileDocuments_.end(); ++it) {
        if (it->second.page == widget) {
            tabs_->removeTab(index);
            delete widget;
            fileDocuments_.erase(it);
            activateCurrentDocument();
            return;
        }
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
        levelData_->setLevel(nullptr, &moduleData_);
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
        levelData_->setLevel(currentLevel_, &moduleData_);
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
    levelData_->setLevel(currentLevel_, &moduleData_);
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
    // Removing a tab makes the tab bar pick a new current widget, which runs
    // activateCurrentDocument() while this loop is still deleting views. Move
    // the maps out first so that pass never sees a document whose view has
    // already been deleted.
    auto documents = std::move(documents_);
    documents_.clear();
    currentLevel_ = nullptr;
    for (auto& [key, document] : documents) {
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
    // File editors are keyed by variant too, so a switch has to take them with it.
    auto file_documents = std::move(fileDocuments_);
    fileDocuments_.clear();
    for (auto& [key, document] : file_documents) {
        (void)key;
        if (const auto index = tabs_->indexOf(document.page); index >= 0) tabs_->removeTab(index);
        delete document.page;
    }
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
    settings.setValue("ncpatcherVariant", ncpatcherVariant_);
    settings.setValue("activeModule", activeModule_);
    settings.setValue("activeVariant", activeVariant_);
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
    ncpatcherVariant_ = settings.value("ncpatcherVariant").toString();
    activeModule_ = settings.value("activeModule", activeModule_).toString();
    activeVariant_ = settings.value("activeVariant", ncpatcherVariant_).toString();
    if (patcherPanel_) {
        patcherPanel_->setSelectedVariant(ncpatcherVariant_);
    }
    reloadFilesystemView();
    const auto detached = settings.value("detached").toStringList();
    for (const auto& document_key : settings.value("documents").toStringList()) {
        const auto paths = documentKeyPaths(document_key.toStdString());
        if (!paths) {
            continue;
        }
        const auto index = findLevelIndex(QString::fromStdString(paths->course), QString::fromStdString(paths->bgdat));
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
    }
    if (active == "preview" && preview_visible) {
        showPreview();
    } else if (const auto found = documents_.find(active.toStdString()); found != documents_.end() && !found->second.window) {
        tabs_->setCurrentWidget(found->second.view);
    }
    settings.endGroup();
}

void MainWindow::resetWorkspaceLayout() {
    restoreState(defaultDockState_);
    for (auto* dock : {levelsDock_, propertiesDock_, paletteDock_, filesystemDock_, buildDock_}) {
        dock->show();
    }
    showPreview();
}

void MainWindow::configureGraphics(LevelPreviewWidget* view) {
    if (!view || !view->level()) {
        return;
    }
    view->setCustomActors(moduleData_);
    view->setActorDefinitions(actorDefinitions_);
    // Before the graphicsMatch below, because the tileset decode reads it: a
    // level opened after the catalog loaded gets it here rather than from the
    // reload that only walks the documents already open.
    view->setTileOverlays(tileOverlays_);
    const auto references = view->level()->tileset_references();
    if (view->graphicsMatch(references)) return;
    auto resources = nsmb::project::resolve_level_graphics(effectiveFiles_, references);
    if (!resources) {
        log("Graphics fallback: " + QString::fromStdString(resources.error().message));
        return;
    }
    for (auto& actor : resources.value().actors) {
        const auto* definition = actorDefinitions_.by_object_id(actor.object_id);
        if (!definition) continue;
        actor.name = definition->sprite.name;
    }
    for (const auto& definition : actorDefinitions_.actors) if (definition.hash) {
        nsmb::project::ActorResourceFiles actor;
        actor.hash = definition.hash;
        actor.object_id = definition.sprite.object_id;
        actor.name = definition.sprite.name;
        resources.value().actors.push_back(std::move(actor));
    }
    for (const auto& definition : actorDefinitions_.actors) {
        resources.value().watched_files.push_back(definition.source);
        for (const auto& path : definition.render_resources)
            if (!path.empty()) resources.value().watched_files.push_back(path);
    }
    std::unordered_map<std::uint16_t, std::uint32_t> actor_object_ids;
    for (const auto& actor : resources.value().actors) actor_object_ids.emplace(actor.type, actor.object_id);
    if (view->level() == currentLevel_) levelProperties_->setSpriteDefinitions(spriteDefinitions_, std::move(actor_object_ids));
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
    reloadActorDefinitions();
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
    if (!objectTabs_ || !spriteToolbox_) return;
    paletteButtons_.clear();
    while (objectTabs_->count()) { auto* widget = objectTabs_->widget(0); objectTabs_->removeTab(0); delete widget; }
    while (spriteToolbox_->count()) { auto* widget = spriteToolbox_->widget(0); spriteToolbox_->removeItem(0); delete widget; }
    customPaletteTree_->clear();

    LevelPreviewWidget* view = nullptr;
    if (currentLevel_) {
        for (auto& [key, document] : documents_) {
            (void)key;
            if (document.view->level() == currentLevel_) view = document.view;
        }
    }

    // A grid of preview buttons, shared by the object and sprite pickers.
    const auto grid = [this](QWidget* parent, const std::vector<LevelPreviewWidget::PaletteEntry>& entries,
                             const QString& category) {
        auto* page = new QScrollArea(parent);
        page->setWidgetResizable(true);
        page->setFrameShape(QFrame::NoFrame);
        auto* content = new QWidget(page);
        auto* layout = new QGridLayout(content);
        layout->setAlignment(Qt::AlignTop);
        int cell = 0;
        for (const auto& entry : entries) {
            auto* button = new QToolButton(content);
            button->setText(entry.label);
            button->setToolTip(entry.tooltip.isEmpty() ? entry.label : entry.tooltip);
            button->setToolButtonStyle(Qt::ToolButtonTextUnderIcon);
            button->setIconSize({40, 40});
            button->setMinimumSize(82, 72);
            button->setProperty("category", category);
            if (!entry.image.isNull()) {
                button->setIcon(QIcon(QPixmap::fromImage(
                    entry.image.scaled(40, 40, Qt::KeepAspectRatio, Qt::SmoothTransformation))));
            }
            connect(button, &QToolButton::clicked, this, [this, kind = entry.kind, id = entry.id, slot = entry.slot] {
                auto* target = activeLevelView();
                if (!target) return;
                if (kind == 1) target->setPlacementObject(slot, id);
                else if (kind == 2) target->setPlacementSprite(id);
                else target->setCreateKind(kind);
                target->setFocus();
            });
            layout->addWidget(button, cell / 3, cell % 3);
            paletteButtons_.push_back(button);
            ++cell;
        }
        page->setWidget(content);
        return page;
    };

    std::array<std::vector<LevelPreviewWidget::PaletteEntry>, 3> objects;
    std::map<std::string, std::vector<LevelPreviewWidget::PaletteEntry>> sprites;
    if (view) {
        for (auto& entry : view->paletteEntries()) {
            if (entry.kind == 1 && entry.slot >= 0 && entry.slot < static_cast<int>(objects.size())) {
                objects[static_cast<std::size_t>(entry.slot)].push_back(std::move(entry));
            } else if (entry.kind == 2) {
                sprites[entry.category.toStdString()].push_back(std::move(entry));
            }
        }
        std::unordered_map<std::string, QTreeWidgetItem*> modules;
        std::unordered_map<std::string, QTreeWidgetItem*> components;
        for (const auto& actor : moduleData_.stage_objects) {
            auto*& module = modules[actor.module];
            if (!module) module = new QTreeWidgetItem(customPaletteTree_, {QString::fromStdString(actor.module)});
            const auto componentKey = actor.module + "\n" + actor.component;
            auto*& component = components[componentKey];
            if (!component) component = new QTreeWidgetItem(module, {QString::fromStdString(actor.component)});
            auto* item = new QTreeWidgetItem(component, {QString::fromStdString(actor.name) +
                QString("  [object %1]").arg(actor.object_id)});
            item->setData(0, Qt::UserRole, static_cast<qulonglong>(actor.hash));
            const auto image = view->customActorImage(actor.hash);
            if (!image.isNull()) item->setIcon(0, QIcon(QPixmap::fromImage(image)));
        }
        customPaletteTree_->expandAll();
    }

    // Only slots that actually have a tileset loaded get a page, so the picker
    // never offers three tabs where the level uses one.
    for (std::size_t slot = 0; slot < objects.size(); ++slot) {
        if (objects[slot].empty()) continue;
        auto* page = grid(objectTabs_, objects[slot], QString("Tileset %1").arg(slot));
        page->setObjectName(QString("ObjectPicker%1").arg(slot));
        objectTabs_->addTab(page, QString("Tileset %1").arg(slot));
    }
    if (objectTabs_->count() == 0) {
        auto* empty = new QLabel("Open a level to list its tileset objects.", objectTabs_);
        empty->setAlignment(Qt::AlignCenter);
        empty->setWordWrap(true);
        objectTabs_->addTab(empty, "Objects");
    }

    for (const auto& [category, entries] : sprites) {
        spriteToolbox_->addItem(grid(spriteToolbox_, entries, QString::fromStdString(category)),
                                QString::fromStdString(category));
    }
    if (spriteToolbox_->count() == 0) {
        auto* empty = new QLabel("Open a level to list its actors.", spriteToolbox_);
        empty->setAlignment(Qt::AlignCenter);
        empty->setWordWrap(true);
        spriteToolbox_->addItem(empty, "Sprites");
    }

    emit paletteSearch_->textChanged(paletteSearch_->text());
}

std::string MainWindow::documentKey(const QString& course_path, const QString& bgdat_path) const {
    // The variant is part of the identity. Two variants can supply different
    // bytes for the same project-relative path, so a session cached without it
    // would be handed back for the wrong layer.
    return activeVariant_.toStdString() + "\n" + course_path.toStdString() + "\n" + bgdat_path.toStdString();
}

nsmb::core::Result<nsmb::project::EditableLevel*> MainWindow::levelSession(const QString& course_path, const QString& bgdat_path) {
    if (course_path.isEmpty() || bgdat_path.isEmpty()) {
        return nsmb::core::make_error(nsmb::core::ErrorCode::NotFound, "course or BGDat file is missing");
    }
    const auto key = documentKey(course_path, bgdat_path);
    if (const auto found = levelSessions_.find(key); found != levelSessions_.end()) {
        return found->second.get();
    }

    auto course_data = effectiveFiles_.read(course_path.toStdString());
    if (!course_data) {
        return course_data.error();
    }
    auto bgdat_data = effectiveFiles_.read(bgdat_path.toStdString());
    if (!bgdat_data) {
        return bgdat_data.error();
    }
    auto level = nsmb::project::EditableLevel::load(course_data.value(), bgdat_data.value(), &moduleData_);
    if (!level) {
        return level.error();
    }
    const auto registration = level.value().registration_hashes();
    if (registration.size() > moduleData_.stage_objects.size())
        log(QString("Glue warning: level registers %1 custom objects, but this build declares only %2.")
                .arg(registration.size()).arg(moduleData_.stage_objects.size()));
    for (const auto hash : registration) if (!moduleData_.stage_object_by_hash(hash))
        log(QString("Glue warning: custom object 0x%1 is not declared by this build; it will spawn nothing.")
                .arg(hash, 8, 16, QLatin1Char('0')));
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
    levelData_->refresh();
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
    levelData_->refresh();
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
    if (choice == QMessageBox::Save) return saveProject();
    if (choice == QMessageBox::Discard) { clearRecoverySnapshots(); return true; }
    return false;
}

void MainWindow::closeEvent(QCloseEvent* event) {
    if (confirmDiscardLevelEdits()) {
        saveWorkspaceState();
        event->accept();
    } else {
        event->ignore();
    }
}

void MainWindow::editPreferences() {
    QDialog dialog(this);
    dialog.setWindowTitle("Preferences");
    dialog.setObjectName("PreferencesDialog");
    auto* layout = new QVBoxLayout(&dialog);
    auto* form = new QFormLayout;

    auto* language = new QComboBox(&dialog);
    language->setObjectName("PreferenceLanguage");
    for (const auto& [code, name] : std::array{std::pair{"en", "English"}, std::pair{"nl", "Nederlands"},
                                               std::pair{"pt", "Portugues"}, std::pair{"es", "Espanol"},
                                               std::pair{"fr", "Francais"}}) {
        language->addItem(QString::fromUtf8(name), QString::fromLatin1(code));
    }
    const auto current = QSettings().value("Language", "en").toString();
    if (const auto index = language->findData(current); index >= 0) language->setCurrentIndex(index);
    form->addRow("Language", language);
    auto* restart = new QLabel("Applied the next time Big Star starts.", &dialog);
    restart->setWordWrap(true);
    form->addRow(QString{}, restart);

    auto* autosave = new QSpinBox(&dialog);
    autosave->setObjectName("PreferenceAutosaveMinutes");
    autosave->setRange(1, 60);
    autosave->setSuffix(" min");
    autosave->setValue(QSettings().value("AutosaveMinutes", 1).toInt());
    autosave->setToolTip("How often unsaved level edits are written to a recovery snapshot.");
    form->addRow("Recovery snapshots", autosave);

    auto* backups = new QSpinBox(&dialog);
    backups->setObjectName("PreferenceBackupsKept");
    backups->setRange(1, 100);
    backups->setValue(QSettings().value("BackupsKept", 10).toInt());
    backups->setToolTip("How many level-save and patch-import backups are kept, of each kind. "
                        "Older ones are removed when a project opens and whenever one is added.");
    form->addRow("Backups kept", backups);

    layout->addLayout(form);
    auto* buttons = new QDialogButtonBox(QDialogButtonBox::Save | QDialogButtonBox::Cancel, &dialog);
    layout->addWidget(buttons);
    connect(buttons, &QDialogButtonBox::accepted, &dialog, &QDialog::accept);
    connect(buttons, &QDialogButtonBox::rejected, &dialog, &QDialog::reject);
    if (dialog.exec() != QDialog::Accepted) return;

    QSettings settings;
    settings.setValue("Language", language->currentData().toString());
    settings.setValue("AutosaveMinutes", autosave->value());
    settings.setValue("BackupsKept", backups->value());
    // The snapshot interval is the one preference that can take effect now.
    if (autosaveTimer_) autosaveTimer_->setInterval(autosave->value() * 60000);
}

void MainWindow::log(const QString& message) {
    if (patcherPanel_) {
        patcherPanel_->appendLog(message);
    }
}
