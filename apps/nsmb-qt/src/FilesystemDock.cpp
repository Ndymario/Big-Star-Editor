#include "FilesystemDock.hpp"
#include "ThemeColors.hpp"
#include "FileEditors.hpp"

#include "nsmb/core/file_io.hpp"
#include "nsmb/formats/identify.hpp"
#include "nsmb/formats/ui_resources.hpp"

#include <QAction>
#include <QComboBox>
#include <QColor>
#include <QDialog>
#include <QDialogButtonBox>
#include <QFile>
#include <QFileDialog>
#include <QFileIconProvider>
#include <QFileInfo>
#include <QIcon>
#include <QEvent>
#include <QHeaderView>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QLabel>
#include <QLineEdit>
#include <QLocale>
#include <QMenu>
#include <QMessageBox>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QSettings>
#include <QSignalBlocker>
#include <QTreeWidget>
#include <QTreeWidgetItemIterator>
#include <QVBoxLayout>

#include <algorithm>
#include <limits>
#include <map>
#include <set>
#include <vector>

namespace {
constexpr auto path_role = Qt::UserRole;
// Set on a container row whose members have not been listed from the ROM yet.
// Expanding one clears it, which is what keeps a game's hundreds of archives
// from being parsed to open a project.
constexpr auto unlisted_archive_role = Qt::UserRole + 1;

// Path and Status are the list; everything else about a file lives in the footer, and is only
// promoted to a column when the user asks for it through the header menu.
enum Column { PathColumn, StatusColumn, ProvenanceColumn, VariantColumn, FileIdColumn, SizeColumn, ColumnCount };

const QStringList& column_names() {
    static const QStringList names{"Path", "Status", "Provenance", "Variant", "File ID", "Size"};
    return names;
}

int default_column_width(int column) {
    switch (column) {
    case PathColumn: return 280;
    case StatusColumn: return 150;
    case FileIdColumn: return 80;
    default: return 110;
    }
}

QString provenance(const nsmb::project::FilesystemEntry& entry) {
    if (entry.module.empty()) return entry.source.empty() ? QString{} : QString("project");
    return QString::fromStdString(entry.module + (entry.component.empty() ? "" : " / " + entry.component));
}

int compare_names(QStringView left, QStringView right) {
    // Natural, case-insensitive ordering: "w2map" before "w10map", and "bomthrow.narc"
    // next to "Dat_2D.narc" instead of after every upper-case name in the directory.
    qsizetype l = 0, r = 0;
    while (l < left.size() && r < right.size()) {
        if (left[l].isDigit() && right[r].isDigit()) {
            const auto leftStart = l, rightStart = r;
            while (l < left.size() && left[l].isDigit()) ++l;
            while (r < right.size() && right[r].isDigit()) ++r;
            auto leftDigits = left.sliced(leftStart, l - leftStart);
            auto rightDigits = right.sliced(rightStart, r - rightStart);
            qsizetype leftZeros = 0, rightZeros = 0;
            while (leftZeros + 1 < leftDigits.size() && leftDigits[leftZeros] == u'0') ++leftZeros;
            while (rightZeros + 1 < rightDigits.size() && rightDigits[rightZeros] == u'0') ++rightZeros;
            leftDigits = leftDigits.sliced(leftZeros); rightDigits = rightDigits.sliced(rightZeros);
            if (leftDigits.size() != rightDigits.size()) return leftDigits.size() < rightDigits.size() ? -1 : 1;
            if (const auto order = leftDigits.compare(rightDigits); order != 0) return order < 0 ? -1 : 1;
            continue;
        }
        const auto leftChar = left[l].toLower(), rightChar = right[r].toLower();
        if (leftChar != rightChar) return leftChar < rightChar ? -1 : 1;
        ++l; ++r;
    }
    if (l < left.size()) return 1;
    if (r < right.size()) return -1;
    const auto exact = left.compare(right, Qt::CaseSensitive);  // stable tie-break
    return exact == 0 ? 0 : (exact < 0 ? -1 : 1);
}

enum class EntryIcon { File, Folder, Archive, Image, Palette, ScreenMap, Model, Animation, Audio, Level, Text, Particle, TouchHitbox, OamPositions };

// Icons come from the desktop's own theme, so the tree matches every other file list on the
// system. Each category names the theme icons it prefers; the first one installed wins, and
// a category with nothing suitable falls back to the system's own generic file icon -- which
// is also what platforms without a theme (Windows, macOS) get for everything but folders.
// The cache lives for one rebuild rather than the session, so icons follow a theme change.
QIcon entry_icon(EntryIcon kind, std::map<EntryIcon, QIcon>& resolved) {
    const auto cached = resolved.find(kind);
    if (cached != resolved.end()) return cached->second;

    const QFileIconProvider provider;
    if (kind == EntryIcon::Folder) return resolved.emplace(kind, provider.icon(QFileIconProvider::Folder)).first->second;

    static const std::map<EntryIcon, QStringList> preferred{
        {EntryIcon::Archive, {"package-x-generic", "application-x-archive"}},
        {EntryIcon::Image, {"image-x-generic"}},
        {EntryIcon::Palette, {"color-picker", "preferences-desktop-color", "image-x-generic"}},
        {EntryIcon::ScreenMap, {"x-office-drawing", "image-x-generic"}},
        {EntryIcon::Model, {"applications-graphics", "image-x-generic"}},
        {EntryIcon::Animation, {"video-x-generic"}},
        {EntryIcon::Audio, {"audio-x-generic"}},
        {EntryIcon::Level, {"applications-games", "input-gaming"}},
        {EntryIcon::Text, {"text-x-generic"}},
        {EntryIcon::Particle, {"preferences-desktop-effects"}},
        {EntryIcon::TouchHitbox, {"input-touchpad"}},
        {EntryIcon::OamPositions, {"snap-bounding-box", "transform-move"}},
    };
    QIcon icon;
    if (const auto names = preferred.find(kind); names != preferred.end())
        for (const auto& name : names->second) if (icon = QIcon::fromTheme(name); !icon.isNull()) break;
    if (icon.isNull()) icon = provider.icon(QFileIconProvider::File);
    return resolved.emplace(kind, std::move(icon)).first->second;
}

bool has_magic(std::span<const std::uint8_t> header, std::string_view magic, std::size_t offset) {
    return header.size() >= offset + magic.size() && std::equal(magic.begin(), magic.end(), header.begin() + offset);
}

// Where a magic can still be read in a compressed file. NSMB stores plenty of NSBCA and
// player data LZ-compressed, and the first block flag is usually zero, so the header bytes
// survive verbatim right after it -- offset 5 for a bare LZ stream, 9 behind an LZ77 header.
// Anything whose first block is actually compressed stays unidentified, as it does in NSMBe.
std::vector<std::size_t> magic_offsets(std::span<const std::uint8_t> header) {
    std::vector<std::size_t> offsets{0};
    if (!header.empty() && header[0] == 0x10) offsets.push_back(5);
    if (has_magic(header, "LZ77", 0)) offsets.push_back(9);
    return offsets;
}

// Classifies an entry for its icon only. Formats the shared identifier knows come from there.
// Magic wins over the file name throughout: the identifier's own name hints treat any path
// containing "course" as a level, which would otherwise mislabel script/course.bmg and friends.
EntryIcon classify_entry(const nsmb::project::FilesystemEntry& entry, nsmb::project::EffectiveFilesystem* files) {
    // The head is enough to recognise a format, and a dock that read two
    // thousand entries whole to draw its icons would read a game's worth of
    // bytes to do it.
    std::vector<std::uint8_t> header;
    if (files) if (auto prefix = files->read_prefix(entry.path, 32)) header = std::move(prefix.value());

    using nsmb::formats::FormatKind;
    using nsmb::formats::GraphicsFormatKind;
    const auto graphics = [](GraphicsFormatKind kind) -> std::optional<EntryIcon> {
        switch (kind) {
        case GraphicsFormatKind::NitroPalette:
        case GraphicsFormatKind::RawPaletteCandidate: return EntryIcon::Palette;
        case GraphicsFormatKind::NitroScreenMap:
        case GraphicsFormatKind::RawScreenMapCandidate: return EntryIcon::ScreenMap;
        case GraphicsFormatKind::NitroModel: return EntryIcon::Model;
        case GraphicsFormatKind::NitroCellAnimation: return EntryIcon::Animation;
        case GraphicsFormatKind::NitroCharacterGraphics:
        case GraphicsFormatKind::RawCharacterGraphicsCandidate:
        case GraphicsFormatKind::NitroCellLayout:
        case GraphicsFormatKind::NitroTexture: return EntryIcon::Image;
        case GraphicsFormatKind::Unknown: break;
        }
        return std::nullopt;
    };

    for (const auto offset : magic_offsets(header)) {
        const auto body = std::span(header).subspan(std::min(offset, header.size()));
        if (nsmb::formats::identify_format(body) == FormatKind::NarcArchive) return EntryIcon::Archive;

        // NSMB's own uiStudio formats: BNBL holds touch hit boxes, BNCL holds OAM positions,
        // and BNCD holds OAM plus the graphics themselves. ENPG and the banner need the whole
        // file to confirm, which a header prefix cannot give, so those fall to the name below.
        switch (nsmb::formats::identify_ui_resource(body, entry.path)) {
        case nsmb::formats::UiResourceKind::Bnbl: return EntryIcon::TouchHitbox;
        case nsmb::formats::UiResourceKind::Bncl: return EntryIcon::OamPositions;
        case nsmb::formats::UiResourceKind::Bncd: return EntryIcon::Image;
        case nsmb::formats::UiResourceKind::Enpg:
        case nsmb::formats::UiResourceKind::Banner: return EntryIcon::Image;
        case nsmb::formats::UiResourceKind::Unknown: break;
        }
        // BNLL is undocumented -- nothing models it, and it is recognised here only so its
        // name cannot fall through to the level hint below.
        if (has_magic(body, "JNLL", 0)) return EntryIcon::File;

        if (const auto icon = graphics(nsmb::formats::identify_graphics_format(body))) return *icon;
        // Other NSMB formats the shared identifier does not model yet.
        if (has_magic(body, "SDAT", 0)) return EntryIcon::Audio;
        if (has_magic(body, "MESG", 0)) return EntryIcon::Text;
        if (has_magic(body, " APS22_1", 0)) return EntryIcon::Particle;
        for (const auto* animation : {"BCA0", "BTP0", "BTA0", "BMA0", "BVA0"})
            if (has_magic(body, animation, 0)) return EntryIcon::Animation;
    }

    // Nothing identified the bytes, so fall back to the name.
    if (QString::fromStdString(entry.path).endsWith(".enpg", Qt::CaseInsensitive)) return EntryIcon::Image;
    if (const auto icon = graphics(nsmb::formats::identify_graphics_format({}, entry.path))) return *icon;
    const auto named = nsmb::formats::identify_format({}, entry.path);
    if (named == FormatKind::BgdatCandidate || named == FormatKind::LevelCandidate) return EntryIcon::Level;
    return EntryIcon::File;
}

// A plan predicts the ID a new file would take, so a pending insertion sorts where it
// will land rather than at the end. Without one -- no NCPatcher to ask -- there is no ID
// to sort by and the file trails its siblings.
constexpr auto unassigned_id = std::numeric_limits<qint64>::max();

// A ROM address as tree segments. `a/b.narc!c/d` becomes a, b.narc, c, d: the
// container is a node like any other and its members hang beneath it, so the
// `!` shows up only in the keys, which is what keeps a member row's key equal
// to the address the view knows it by.
struct Segments {
    QStringList names;
    QStringList keys;  // the address of everything up to and including that name
};

Segments split_address(const QString& path) {
    const auto bang = path.indexOf('!');
    Segments split; QString key;
    for (const auto& part : (bang < 0 ? path : path.left(bang)).split('/')) {
        if (!key.isEmpty()) key += '/';
        key += part; split.names << part; split.keys << key;
    }
    if (bang < 0) return split;
    bool first = true;
    for (const auto& part : path.mid(bang + 1).split('/')) {
        key += first ? '!' : '/'; first = false;
        key += part; split.names << part; split.keys << key;
    }
    return split;
}

struct TreeRow {
    QStringList parts;
    QList<qint64> order;  // per segment: the lowest file ID at or beneath that prefix
    const nsmb::project::FilesystemEntry* entry;
};

// NitroFS lists files in file-ID order, which is the order the FNT defines and the order
// the game sees, so the tree follows it rather than sorting names. Every directory owns a
// contiguous ID range, so a directory sorts by the lowest ID it contains -- which puts
// 00DUMMY above ARCHIVE. Names only break ties between entries with no ID (a project
// NCPatcher has not built yet has none at all, and then this is a plain natural sort).
bool path_precedes(const TreeRow& left, const TreeRow& right) {
    for (qsizetype i = 0; i < left.parts.size() && i < right.parts.size(); ++i) {
        if (left.order[i] != right.order[i]) return left.order[i] < right.order[i];
        if (const auto order = compare_names(left.parts[i], right.parts[i]); order != 0) return order < 0;
    }
    return left.parts.size() < right.parts.size();
}

}

FilesystemDock::FilesystemDock(QWidget* parent) : QWidget(parent) {
    auto* layout = new QVBoxLayout(this);
    auto* target = new QHBoxLayout;
    // This picks where add/replace/extract write, not what the tree shows -- it
    // used to sit next to a variant combo and read like a second view filter.
    target->addWidget(new QLabel("Save to module", this));
    module_ = new QComboBox(this); module_->setObjectName("ActiveModule"); target->addWidget(module_, 1);
    layout->addLayout(target);

    auto* banner = new QHBoxLayout;
    pending_ = new QLabel(this); pending_->setWordWrap(true); applyThemeColors(); banner->addWidget(pending_, 1);
    run_ = new QPushButton("Run NCPatcher", this); run_->setObjectName("RunPendingPatcher"); banner->addWidget(run_);
    layout->addLayout(banner);

    tree_ = new QTreeWidget(this); tree_->setObjectName("FilesystemTree");
    tree_->setHeaderLabels(column_names());
    auto* header = tree_->header();
    // Every column is hand-resizable, so no automatic mode may fight the width the user drags.
    header->setSectionResizeMode(QHeaderView::Interactive);
    header->setStretchLastSection(false);
    header->setContextMenuPolicy(Qt::CustomContextMenu);
    for (int column = 0; column < ColumnCount; ++column) tree_->setColumnWidth(column, default_column_width(column));
    for (int column = ProvenanceColumn; column < ColumnCount; ++column) tree_->setColumnHidden(column, true);
    layout->addWidget(tree_, 1);

    columnMenu_ = new QMenu(this); columnMenu_->setObjectName("FilesystemColumns");
    for (int column = 0; column < ColumnCount; ++column) {
        auto* action = columnMenu_->addAction(column_names()[column]);
        action->setCheckable(true);
        action->setChecked(!tree_->isColumnHidden(column));
        action->setEnabled(column != PathColumn);  // without the path a row names nothing
        connect(action, &QAction::toggled, this, [this, column](bool visible) {
            tree_->setColumnHidden(column, !visible);
            if (visible && tree_->columnWidth(column) <= 0) tree_->setColumnWidth(column, default_column_width(column));
            saveColumnState();
        });
    }
    connect(header, &QWidget::customContextMenuRequested, this, &FilesystemDock::showColumnMenu);

    // Everything the hidden columns would say about the selected file, the way NSMB Editor
    // reports it: one line above the buttons that act on that file.
    details_ = new QLabel(this); details_->setObjectName("FilesystemDetails");
    details_->setWordWrap(true); details_->setTextInteractionFlags(Qt::TextSelectableByMouse);
    layout->addWidget(details_);

    restoreColumnState();
    // Connected after the restore so replaying stored widths does not write them straight back.
    connect(header, &QHeaderView::sectionResized, this, [this] { saveColumnState(); });

    auto* actions = new QHBoxLayout;
    add_ = new QPushButton("Add file", this); replace_ = new QPushButton("Replace", this); remove_ = new QPushButton("Remove", this);
    extract_ = new QPushButton("Extract", this); open_ = new QPushButton("Open", this); open_->setObjectName("OpenFile"); hex_ = new QPushButton("Hex edit", this);
    for (auto* button : {add_, replace_, remove_, extract_, open_, hex_}) actions->addWidget(button);
    layout->addLayout(actions);

    auto changed = [this] { emit activeTargetChanged(activeModule(), activeVariant()); };
    connect(module_, &QComboBox::currentIndexChanged, this, changed);
    connect(run_, &QPushButton::clicked, this, &FilesystemDock::runPatcherRequested);
    connect(add_, &QPushButton::clicked, this, &FilesystemDock::addFile);
    connect(replace_, &QPushButton::clicked, this, &FilesystemDock::replaceFile);
    connect(remove_, &QPushButton::clicked, this, &FilesystemDock::removeFile);
    connect(extract_, &QPushButton::clicked, this, &FilesystemDock::extractFile);
    connect(open_, &QPushButton::clicked, this, &FilesystemDock::openFile);
    connect(hex_, &QPushButton::clicked, this, &FilesystemDock::hexEditFile);
    // Double-clicking a file opens it in whichever editor understands it, the way NSMB Editor does.
    connect(tree_, &QTreeWidget::itemDoubleClicked, this, [this](QTreeWidgetItem* item, int) {
        if (item->data(0, path_role).toString().isEmpty()) return;  // a directory row only expands
        openFile();
    });
    connect(tree_, &QTreeWidget::itemExpanded, this, &FilesystemDock::listArchive);
    connect(tree_, &QTreeWidget::itemSelectionChanged, this, [this] {
        const auto* entry = selectedEntry(); replace_->setEnabled(entry); extract_->setEnabled(entry); hex_->setEnabled(entry);
        selectedEditor_ = editorFor(entry);
        updateDetails();
        open_->setEnabled(selectedEditor_ != bse::editors::Kind::None);
        // A member cannot be removed: the archive codec replaces in place,
        // because dropping a member renumbers every member after it and the
        // game addresses them by number.
        remove_->setEnabled(entry && !entry->source.empty() && !entry->module.empty() && !entry->inside_archive());
    });
}

void FilesystemDock::setData(std::filesystem::path projectDir,
                             nsmb::project::EffectiveFilesystem* files,
                             nsmb::patching::ModuleGraph modules,
                             nsmb::project::FilesystemView view,
                             QStringList variants,
                             QString activeModule,
                             QString activeVariant,
                             std::vector<nsmb::project::ModuleVariantMapping> mappings) {
    projectDir_ = std::move(projectDir); files_ = files; modules_ = std::move(modules); view_ = std::move(view); mappings_ = std::move(mappings);
    const QSignalBlocker moduleBlock(module_);
    module_->clear();
    for (const auto& entry : modules_.modules) if (entry.enabled && entry.nitrofs)
        module_->addItem(QString::fromStdString(entry.name.empty() ? entry.key : entry.name), QString::fromStdString(entry.key));
    auto moduleIndex = module_->findData(activeModule); if (moduleIndex < 0 && module_->count()) moduleIndex = 0; module_->setCurrentIndex(moduleIndex);
    variants.removeDuplicates();
    variants_ = std::move(variants);
    activeVariant_ = activeVariant;
    rebuildTree();
}

QString FilesystemDock::activeModule() const { return module_->currentData().toString(); }
QString FilesystemDock::activeVariant() const { return activeVariant_; }

void FilesystemDock::rebuildTree() {
    tree_->clear();
    std::map<EntryIcon, QIcon> icons;
    auto* root = new QTreeWidgetItem(tree_, {"root"});
    root->setIcon(0, entry_icon(EntryIcon::Folder, icons));
    std::map<QString, QTreeWidgetItem*> directories{{{}, root}};
    std::vector<TreeRow> rows; rows.reserve(view_.entries.size());
    std::vector<Segments> addresses; addresses.reserve(view_.entries.size());
    for (const auto& entry : view_.entries) {
        addresses.push_back(split_address(QString::fromStdString(entry.path)));
        rows.push_back({addresses.back().names, {}, &entry});
    }

    std::map<QString, qint64> lowestId;
    for (std::size_t i = 0; i < rows.size(); ++i) {
        if (!rows[i].entry->id) continue;
        const auto id = static_cast<qint64>(*rows[i].entry->id);
        for (const auto& key : addresses[i].keys) {
            const auto slot = lowestId.try_emplace(key, id).first;
            slot->second = std::min(slot->second, id);
        }
    }
    for (std::size_t i = 0; i < rows.size(); ++i) {
        rows[i].order.reserve(addresses[i].keys.size());
        for (const auto& key : addresses[i].keys) {
            const auto found = lowestId.find(key);
            rows[i].order.push_back(found == lowestId.end() ? unassigned_id : found->second);
        }
    }
    // A container sorts above its own members: they share every segment it has
    // and then it runs out of them first.
    std::ranges::sort(rows, path_precedes);

    for (const auto& [parts, order, entryPointer] : rows) {
        const auto& entry = *entryPointer; const auto address = split_address(QString::fromStdString(entry.path));
        const auto& path = address.keys.back(); auto* parent = root;
        for (int i = 0; i + 1 < parts.size(); ++i) {
            const auto& directoryPath = address.keys[i];
            if (!directories.contains(directoryPath)) {
                directories[directoryPath] = new QTreeWidgetItem(parent, {parts[i]});
                directories[directoryPath]->setIcon(0, entry_icon(EntryIcon::Folder, icons));
            }
            parent = directories[directoryPath];
        }
        const auto statusName = entry.status == nsmb::project::FilesystemStatus::Pending
            ? (entry.id && !entry.id_is_planned ? QString("⏳ Pending rebuild") : QString("⏳ Pending insertion"))
            : entry.status == nsmb::project::FilesystemStatus::Orphaned ? QString("Orphaned — reverting")
            : QString::fromUtf8(nsmb::project::filesystem_status_name(entry.status).data(), static_cast<qsizetype>(nsmb::project::filesystem_status_name(entry.status).size()));
        // A planned id is what the next build would assign, not an address the
        // game can use yet, and the two must never read alike.
        const auto fileId = !entry.id ? QString("—")
            : entry.id_is_planned ? QString::number(*entry.id) + "?"
            : QString::number(*entry.id);
        auto* item = new QTreeWidgetItem(parent, {parts.back(), statusName,
            provenance(entry), QString::fromStdString(entry.variant), fileId, QString::number(entry.size)});
        if (entry.id_is_planned) item->setToolTip(FileIdColumn, "The ID the next build would assign. Nothing holds it yet.");
        item->setData(0, path_role, path);
        const auto icon = classify_entry(entry, files_);
        item->setIcon(0, entry_icon(icon, icons));
        // A container is a file and a folder at once: it keeps its own row, and
        // whatever the plan says about its members hangs beneath it. The rest
        // of them are listed when somebody opens it.
        if (icon == EntryIcon::Archive && !entry.inside_archive()) {
            directories.emplace(path, item);
            item->setData(0, unlisted_archive_role, true);
            item->setChildIndicatorPolicy(QTreeWidgetItem::ShowIndicator);
        }
        if (entry.status == nsmb::project::FilesystemStatus::Pending) item->setForeground(1, bse::theme::warning(palette()));
        if (entry.status == nsmb::project::FilesystemStatus::Orphaned) item->setForeground(1, bse::theme::danger(palette()));
    }
    root->setExpanded(true);
    const bool hasPending = view_.has_pending();
    pending_->setVisible(hasPending); run_->setVisible(hasPending);
    pending_->setText(hasPending ? "⏳ Module files differ from the last build. Run NCPatcher to assign IDs and rebuild the ROM." : QString{});
    const bool hasTarget = targetModule(); add_->setEnabled(hasTarget); replace_->setEnabled(false); remove_->setEnabled(false); extract_->setEnabled(false); open_->setEnabled(false); hex_->setEnabled(false);
    selectedEditor_ = bse::editors::Kind::None;
    updateDetails();
}

// Everything an archive holds that the plan had no reason to mention. The plan
// carries provenance, which is the one thing that cannot be derived; the member
// list is the container's own, so it is read from the container -- once, when
// somebody opens it, rather than for every archive in the game on project open.
void FilesystemDock::listArchive(QTreeWidgetItem* item) {
    if (!item || !item->data(0, unlisted_archive_role).toBool()) return;
    item->setData(0, unlisted_archive_role, {});
    const auto container = item->data(0, path_role).toString();
    if (container.isEmpty() || !files_ || !files_->rom()) return;
    auto listed = files_->rom()->list_archive(container.toStdString());
    if (!listed) {
        // Not an archive after all, or not a readable one. The row goes back to
        // being an ordinary file rather than an expander that opens onto nothing.
        item->setChildIndicatorPolicy(QTreeWidgetItem::DontShowIndicatorWhenChildless);
        return;
    }

    // What the plan already put here. Those rows carry provenance this listing
    // does not have, so they stay exactly as they are.
    std::map<QString, QTreeWidgetItem*> directories{{container, item}};
    std::set<QString> present;
    const auto seed = [&](auto&& self, QTreeWidgetItem* node, const QString& key) -> void {
        for (int i = 0; i < node->childCount(); ++i) {
            auto* child = node->child(i);
            const auto childKey = key + (key == container ? '!' : '/') + child->text(0);
            if (child->childCount()) directories.emplace(childKey, child);
            else present.insert(child->data(0, path_role).toString());
            self(self, child, childKey);
        }
    };
    seed(seed, item, container);

    std::map<EntryIcon, QIcon> icons;
    std::map<QString, qint64> indices;
    const auto depth = container.split('/').size();
    for (std::size_t index = 0; index < listed.value().size(); ++index) {
        const auto& member = listed.value()[index];
        // An index the container does not name has no address to open it by,
        // and inventing one would name a member the next build cannot find.
        if (member.path.empty()) continue;
        const auto key = container + "!" + QString::fromStdString(member.path);
        indices.emplace(key, static_cast<qint64>(index));
        if (present.contains(key)) continue;

        nsmb::project::FilesystemEntry vanilla;
        vanilla.path = key.toStdString();
        vanilla.member_index = static_cast<std::int64_t>(index);
        vanilla.size = member.size;
        view_.entries.push_back(std::move(vanilla));
        const auto& entry = view_.entries.back();

        const auto address = split_address(key);
        auto* parent = item;
        for (qsizetype i = depth; i + 1 < address.names.size(); ++i) {
            auto found = directories.find(address.keys[i]);
            if (found == directories.end()) {
                auto* node = new QTreeWidgetItem(parent, {address.names[i]});
                node->setIcon(0, entry_icon(EntryIcon::Folder, icons));
                found = directories.emplace(address.keys[i], node).first;
            }
            parent = found->second;
        }
        auto* leaf = new QTreeWidgetItem(parent, {address.names.back(),
            QString::fromUtf8(nsmb::project::filesystem_status_name(entry.status).data(),
                              static_cast<qsizetype>(nsmb::project::filesystem_status_name(entry.status).size())),
            {}, {}, QString("—"), QString::number(entry.size)});
        leaf->setData(0, path_role, key);
        leaf->setIcon(0, entry_icon(classify_entry(entry, files_), icons));
    }

    // Members sort by index the way the tree above sorts by file ID, so a
    // folder inside an archive takes the lowest index it holds and the order
    // is the container's own rather than alphabetical.
    std::map<const QTreeWidgetItem*, qint64> order;
    const auto rank = [&](auto&& self, QTreeWidgetItem* node) -> qint64 {
        qint64 lowest = unassigned_id;
        if (!node->childCount()) {
            const auto found = indices.find(node->data(0, path_role).toString());
            if (found != indices.end()) lowest = found->second;
        }
        for (int i = 0; i < node->childCount(); ++i) lowest = std::min(lowest, self(self, node->child(i)));
        return order[node] = lowest;
    };
    rank(rank, item);
    const auto reorder = [&](auto&& self, QTreeWidgetItem* node) -> void {
        auto children = node->takeChildren();
        std::stable_sort(children.begin(), children.end(), [&](const auto* left, const auto* right) {
            if (order[left] != order[right]) return order[left] < order[right];
            return compare_names(left->text(0), right->text(0)) < 0;
        });
        node->addChildren(children);
        for (auto* child : children) self(self, child);
    };
    reorder(reorder, item);
}

void FilesystemDock::showColumnMenu(const QPoint& position) {
    const auto actions = columnMenu_->actions();
    for (int column = 0; column < ColumnCount && column < actions.size(); ++column) {
        const QSignalBlocker block(actions[column]);
        actions[column]->setChecked(!tree_->isColumnHidden(column));
    }
    columnMenu_->exec(tree_->header()->mapToGlobal(position));
}

void FilesystemDock::saveColumnState() {
    QSettings().setValue("FilesystemDock/columns", tree_->header()->saveState());
}

void FilesystemDock::restoreColumnState() {
    const auto state = QSettings().value("FilesystemDock/columns").toByteArray();
    if (!state.isEmpty()) tree_->header()->restoreState(state);
}

void FilesystemDock::updateDetails() {
    const auto* entry = selectedEntry();
    if (!entry) { details_->setText("No file selected."); return; }
    QStringList fields{QString::fromStdString(entry->path)};
    fields << "File ID: " + (!entry->id ? QString("unassigned")
        : entry->id_is_planned ? QString::number(*entry->id) + " (planned — assigned by the next build)"
        : QString::number(*entry->id));
    fields << "Size: " + QLocale().toString(static_cast<qlonglong>(entry->size)) + " bytes";
    if (selectedEditor_ != bse::editors::Kind::None) fields << "Opens in: " + bse::editors::editorName(selectedEditor_);
    if (const auto from = provenance(*entry); !from.isEmpty()) fields << "From: " + from;
    if (!entry->variant.empty()) fields << "Variant: " + QString::fromStdString(entry->variant);
    details_->setText(fields.join("   \u00b7   "));
}

void FilesystemDock::applyThemeColors() {
    if (pending_) {
        auto label = pending_->palette();
        label.setColor(QPalette::WindowText, bse::theme::warning(palette()));
        pending_->setPalette(label);
    }
    if (!tree_) return;
    for (QTreeWidgetItemIterator item(tree_); *item; ++item) {
        const auto path = (*item)->data(0, path_role).toString();
        if (path.isEmpty()) continue;
        const auto* entry = view_.find(path.toStdString());
        if (!entry) continue;
        if (entry->status == nsmb::project::FilesystemStatus::Pending) (*item)->setForeground(1, bse::theme::warning(palette()));
        if (entry->status == nsmb::project::FilesystemStatus::Orphaned) (*item)->setForeground(1, bse::theme::danger(palette()));
    }
}

void FilesystemDock::changeEvent(QEvent* event) {
    QWidget::changeEvent(event);
    switch (event->type()) {
    case QEvent::PaletteChange:
    case QEvent::ApplicationPaletteChange:
    case QEvent::ThemeChange:
    case QEvent::StyleChange:
        applyThemeColors();
        break;
    default:
        break;
    }
}

const nsmb::project::FilesystemEntry* FilesystemDock::entryFor(const QString& romPath) const {
    return view_.find(romPath.toStdString());
}

const nsmb::project::FilesystemEntry* FilesystemDock::selectedEntry() const {
    const auto selected = tree_->selectedItems(); if (selected.isEmpty()) return nullptr;
    const auto path = selected.front()->data(0, path_role).toString(); return path.isEmpty() ? nullptr : view_.find(path.toStdString());
}

const nsmb::patching::ModuleInfo* FilesystemDock::targetModule(const nsmb::project::FilesystemEntry* entry) const {
    const auto key = entry && !entry->module.empty() ? entry->module : activeModule().toStdString(); return modules_.find(key);
}

nsmb::core::Result<std::filesystem::path> FilesystemDock::targetPath(std::string_view romPath,const nsmb::project::FilesystemEntry* entry) const {
    const auto* module = targetModule(entry); if (!module) return nsmb::core::make_error(nsmb::core::ErrorCode::InvalidArgument, "choose an active module target first");
    return nsmb::project::module_file_target(*module, activeVariant().toStdString(), romPath, mappings_);
}

nsmb::core::Result<void> FilesystemDock::writeTarget(std::string_view romPath,std::span<const std::uint8_t> bytes,const nsmb::project::FilesystemEntry* entry) {
    auto target = targetPath(romPath, entry); if (!target) return target.error();
    std::error_code ec; std::filesystem::create_directories(target.value().parent_path(), ec);
    if (ec) return nsmb::core::make_error(nsmb::core::ErrorCode::IoError, "could not create module target directory");
    return nsmb::core::write_file_atomically(target.value(), bytes);
}

void FilesystemDock::addFile() {
    const auto source = QFileDialog::getOpenFileName(this, "Add file to module"); if (source.isEmpty()) return;
    const auto defaultPath = "z_new/" + QFileInfo(source).fileName(); bool accepted = false;
    const auto romPath = QInputDialog::getText(this, "ROM destination", "ROM path (new files must be under z_new/)", QLineEdit::Normal, defaultPath, &accepted).trimmed();
    if (!accepted || romPath.isEmpty()) return;
    if (!view_.find(romPath.toStdString()) && !romPath.startsWith("z_new/")) { emit errorOccurred("New NitroFS files must be under z_new/."); return; }
    QStringList existing;
    if (const auto* module = targetModule()) {
        // The base layer plus every configured variant, so the prompt can name
        // all of them rather than only the one in view.
        auto candidates = variants_;
        candidates.prepend(QString{});
        for (const auto& value : candidates) {
            auto target = nsmb::project::module_file_target(*module, value.toStdString(), romPath.toStdString(), mappings_);
            if (target && std::filesystem::exists(target.value())) existing.push_back(value.isEmpty() ? "(base)" : value);
        }
    }
    if (!existing.isEmpty() && QMessageBox::question(this, "Replace existing module file", "This path already exists in: " + existing.join(", ") + ". Replace the active variant?") != QMessageBox::Yes) return;
    auto data = nsmb::core::read_file(source.toStdString()); if (!data) { emit errorOccurred(QString::fromStdString(data.error().message)); return; }
    auto result = writeTarget(romPath.toStdString(), data.value()); if (!result) emit errorOccurred(QString::fromStdString(result.error().message)); else emit filesChanged();
}

void FilesystemDock::replaceFile() {
    const auto* entry = selectedEntry(); if (!entry) return; const auto source = QFileDialog::getOpenFileName(this, "Replacement file"); if (source.isEmpty()) return;
    auto data = nsmb::core::read_file(source.toStdString()); if (!data) { emit errorOccurred(QString::fromStdString(data.error().message)); return; }
    auto result = writeTarget(entry->path, data.value(), entry); if (!result) emit errorOccurred(QString::fromStdString(result.error().message)); else emit filesChanged();
}

void FilesystemDock::removeFile() {
    const auto* entry = selectedEntry(); if (!entry || entry->source.empty() || entry->module.empty()) return;
    if (QMessageBox::question(this, "Remove module file", "Move this module source to Trash?\n\n" + QString::fromStdString(entry->source.string())) != QMessageBox::Yes) return;
    if (!QFile::moveToTrash(QString::fromStdString(entry->source.string()))) emit errorOccurred("Could not move the module file to Trash."); else emit filesChanged();
}

void FilesystemDock::extractFile() {
    const auto* entry = selectedEntry(); if (!entry || !files_) return;
    auto data = files_->read(entry->path); if (!data) { emit errorOccurred(QString::fromStdString(data.error().message)); return; }
    const auto destination = QFileDialog::getSaveFileName(this, "Extract file", QString::fromStdString(entry->path)); if (destination.isEmpty()) return;
    auto result = nsmb::core::write_file_atomically(destination.toStdString(), data.value()); if (!result) emit errorOccurred(QString::fromStdString(result.error().message));
}

void FilesystemDock::hexEditFile() {
    const auto* entry = selectedEntry();
    if (entry) emit hexEditRequested(QString::fromStdString(entry->path));
}

// Which editor claims the selected file, if any. Read straight from the bytes so the answer is
// the same one the Open button acts on.
bse::editors::Kind FilesystemDock::editorFor(const nsmb::project::FilesystemEntry* entry) const {
    if (!entry || !files_) return bse::editors::Kind::None;
    const auto data = files_->read(entry->path);
    if (!data) return bse::editors::Kind::None;
    return bse::editors::classify(data.value(), entry->path, entry->id && *entry->id == rom_user_metadata_id);
}

bse::editors::Companions FilesystemDock::companionsFor(const QString& romPath) const {
    // What sits beside this file. Inside a container that means the other
    // members of the same folder of that container, not the container's own
    // neighbours, which is why the address is split rather than the string.
    const auto folder_of = [](const QString& path) {
        const auto address = split_address(path);
        return address.keys.size() < 2 ? QString{} : address.keys[address.keys.size() - 2];
    };
    const auto directory = folder_of(romPath);
    bse::editors::Companions companions;
    for (const auto& candidate : view_.entries) {
        const auto path = QString::fromStdString(candidate.path);
        if (path == romPath || folder_of(path) != directory) continue;
        companions.paths << path;
    }
    companions.paths.sort();
    companions.load = [this](const QString& path) -> nsmb::core::Result<std::vector<std::uint8_t>> {
        if (!view_.find(path.toStdString()) || !files_)
            return nsmb::core::make_error(nsmb::core::ErrorCode::NotFound, "no such file in this variant");
        auto data = files_->read(path.toStdString());
        if (!data) return data.error();
        return bse::editors::unwrapped(data.value());
    };
    return companions;
}

bool FilesystemDock::revealPath(const QString& romPath) {
    if (romPath.isEmpty() || !tree_) return false;
    for (QTreeWidgetItemIterator item(tree_); *item; ++item) {
        if ((*item)->data(0, path_role).toString() != romPath) continue;
        for (auto* parent = (*item)->parent(); parent; parent = parent->parent()) parent->setExpanded(true);
        tree_->setCurrentItem(*item);
        tree_->scrollToItem(*item, QAbstractItemView::PositionAtCenter);
        return true;
    }
    return false;
}

nsmb::core::Result<void> FilesystemDock::saveEditedFile(const QString& romPath, std::span<const std::uint8_t> bytes) {
    const auto* entry = view_.find(romPath.toStdString());
    auto result = writeTarget(romPath.toStdString(), bytes, entry);
    if (result) emit filesChanged();
    return result;
}

void FilesystemDock::openFile() {
    const auto* entry = selectedEntry();
    if (entry) emit openInEditorRequested(QString::fromStdString(entry->path));
}
