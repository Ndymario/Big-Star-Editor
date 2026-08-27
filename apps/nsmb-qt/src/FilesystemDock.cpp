#include "FilesystemDock.hpp"
#include "GraphicsEditorDialog.hpp"
#include "UiResourceEditorDialog.hpp"

#include "nsmb/core/file_io.hpp"

#include <QComboBox>
#include <QColor>
#include <QDialog>
#include <QDialogButtonBox>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QHeaderView>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QLabel>
#include <QLineEdit>
#include <QMessageBox>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QSignalBlocker>
#include <QTreeWidget>
#include <QVBoxLayout>

#include <map>

namespace {
constexpr auto path_role = Qt::UserRole;

QString provenance(const nsmb::project::FilesystemEntry& entry) {
    if (entry.module.empty()) return entry.source.empty() ? QString{} : QString("project");
    return QString::fromStdString(entry.module + (entry.component.empty() ? "" : " / " + entry.component));
}

QString hex(std::span<const std::uint8_t> bytes) {
    return QByteArray(reinterpret_cast<const char*>(bytes.data()), static_cast<qsizetype>(bytes.size())).toHex(' ').toUpper();
}

std::optional<std::vector<std::uint8_t>> unhex(QString text) {
    text.remove(' '); text.remove('\t'); text.remove('\r'); text.remove('\n');
    if (text.size() % 2 != 0) return std::nullopt;
    for (const auto c : text) if (!c.isDigit() && (c.toLower() < 'a' || c.toLower() > 'f')) return std::nullopt;
    const auto data = QByteArray::fromHex(text.toLatin1());
    return std::vector<std::uint8_t>(data.begin(), data.end());
}
}

FilesystemDock::FilesystemDock(QWidget* parent) : QWidget(parent) {
    auto* layout = new QVBoxLayout(this);
    auto* target = new QHBoxLayout;
    target->addWidget(new QLabel("Module", this));
    module_ = new QComboBox(this); module_->setObjectName("ActiveModule"); target->addWidget(module_, 1);
    target->addWidget(new QLabel("Variant", this));
    variant_ = new QComboBox(this); variant_->setObjectName("ActiveVariant"); target->addWidget(variant_, 1);
    layout->addLayout(target);

    auto* banner = new QHBoxLayout;
    pending_ = new QLabel(this); pending_->setWordWrap(true); pending_->setStyleSheet("color:#9a5b00"); banner->addWidget(pending_, 1);
    run_ = new QPushButton("Run NCPatcher", this); run_->setObjectName("RunPendingPatcher"); banner->addWidget(run_);
    layout->addLayout(banner);

    tree_ = new QTreeWidget(this); tree_->setObjectName("FilesystemTree");
    tree_->setHeaderLabels({"Path", "Status", "Provenance", "Variant", "File ID", "Size"});
    tree_->header()->setSectionResizeMode(0, QHeaderView::Stretch);
    for (int column = 1; column < 6; ++column) tree_->header()->setSectionResizeMode(column, QHeaderView::ResizeToContents);
    layout->addWidget(tree_, 1);

    auto* actions = new QHBoxLayout;
    add_ = new QPushButton("Add file", this); replace_ = new QPushButton("Replace", this); remove_ = new QPushButton("Remove", this);
    extract_ = new QPushButton("Extract", this); graphics_ = new QPushButton("Graphics edit", this); graphics_->setObjectName("GraphicsEdit"); uiResource_ = new QPushButton("UI resource edit", this); uiResource_->setObjectName("UiResourceEdit"); hex_ = new QPushButton("Hex edit", this);
    for (auto* button : {add_, replace_, remove_, extract_, graphics_, uiResource_, hex_}) actions->addWidget(button);
    layout->addLayout(actions);

    auto changed = [this] { emit activeTargetChanged(activeModule(), activeVariant()); };
    connect(module_, &QComboBox::currentIndexChanged, this, changed);
    connect(variant_, &QComboBox::currentIndexChanged, this, changed);
    connect(run_, &QPushButton::clicked, this, &FilesystemDock::runPatcherRequested);
    connect(add_, &QPushButton::clicked, this, &FilesystemDock::addFile);
    connect(replace_, &QPushButton::clicked, this, &FilesystemDock::replaceFile);
    connect(remove_, &QPushButton::clicked, this, &FilesystemDock::removeFile);
    connect(extract_, &QPushButton::clicked, this, &FilesystemDock::extractFile);
    connect(graphics_, &QPushButton::clicked, this, &FilesystemDock::graphicsEditFile);
    connect(uiResource_, &QPushButton::clicked, this, &FilesystemDock::uiResourceEditFile);
    connect(hex_, &QPushButton::clicked, this, &FilesystemDock::hexEditFile);
    connect(tree_, &QTreeWidget::itemSelectionChanged, this, [this] {
        const auto* entry = selectedEntry(); replace_->setEnabled(entry); extract_->setEnabled(entry); hex_->setEnabled(entry);
        graphics_->setEnabled(false); uiResource_->setEnabled(false);
        if (entry) { const auto source = !entry->source.empty() ? entry->source : entry->extracted; const auto data = nsmb::core::read_file(source); const bool metadata=entry->id&&*entry->id==131; graphics_->setEnabled(data && GraphicsEditorDialog::supports(data.value(), entry->path)); uiResource_->setEnabled(data&&UiResourceEditorDialog::supports(data.value(),entry->path,metadata)); }
        remove_->setEnabled(entry && !entry->source.empty() && !entry->module.empty());
    });
}

void FilesystemDock::setData(std::filesystem::path projectDir,
                             nsmb::patching::ModuleGraph modules,
                             nsmb::project::FilesystemView view,
                             QStringList variants,
                             QString activeModule,
                             QString activeVariant,
                             std::vector<nsmb::project::ModuleVariantMapping> mappings) {
    projectDir_ = std::move(projectDir); modules_ = std::move(modules); view_ = std::move(view); mappings_ = std::move(mappings);
    const QSignalBlocker moduleBlock(module_), variantBlock(variant_);
    module_->clear();
    for (const auto& entry : modules_.modules) if (entry.enabled && entry.nitrofs)
        module_->addItem(QString::fromStdString(entry.name.empty() ? entry.key : entry.name), QString::fromStdString(entry.key));
    auto moduleIndex = module_->findData(activeModule); if (moduleIndex < 0 && module_->count()) moduleIndex = 0; module_->setCurrentIndex(moduleIndex);
    variant_->clear(); variant_->addItem("(base)", QString{});
    variants.removeDuplicates(); for (const auto& value : variants) if (!value.isEmpty()) variant_->addItem(value, value);
    auto variantIndex = variant_->findData(activeVariant); if (variantIndex < 0) variantIndex = 0; variant_->setCurrentIndex(variantIndex);
    rebuildTree();
}

QString FilesystemDock::activeModule() const { return module_->currentData().toString(); }
QString FilesystemDock::activeVariant() const { return variant_->currentData().toString(); }

void FilesystemDock::rebuildTree() {
    tree_->clear();
    auto* root = new QTreeWidgetItem(tree_, {"root"});
    std::map<QString, QTreeWidgetItem*> directories{{{}, root}};
    for (const auto& entry : view_.entries) {
        const auto path = QString::fromStdString(entry.path); const auto parts = path.split('/'); QString parentPath; auto* parent = root;
        for (int i = 0; i + 1 < parts.size(); ++i) {
            const auto directoryPath = parentPath.isEmpty() ? parts[i] : parentPath + "/" + parts[i];
            if (!directories.contains(directoryPath)) directories[directoryPath] = new QTreeWidgetItem(parent, {parts[i]});
            parent = directories[directoryPath]; parentPath = directoryPath;
        }
        const auto statusName = entry.status == nsmb::project::FilesystemStatus::Pending
            ? (entry.id ? QString("⏳ Pending rebuild") : QString("⏳ Pending insertion"))
            : entry.status == nsmb::project::FilesystemStatus::Orphaned ? QString("Orphaned — reverting")
            : QString::fromUtf8(nsmb::project::filesystem_status_name(entry.status).data(), static_cast<qsizetype>(nsmb::project::filesystem_status_name(entry.status).size()));
        auto* item = new QTreeWidgetItem(parent, {parts.back(), statusName,
            provenance(entry), QString::fromStdString(entry.variant), entry.id ? QString::number(*entry.id) : QString("—"), QString::number(entry.size)});
        item->setData(0, path_role, path);
        if (entry.status == nsmb::project::FilesystemStatus::Pending) item->setForeground(1, QColor(190, 116, 0));
        if (entry.status == nsmb::project::FilesystemStatus::Orphaned) item->setForeground(1, QColor(180, 45, 45));
    }
    root->setExpanded(true);
    const bool hasPending = view_.has_pending();
    pending_->setVisible(hasPending); run_->setVisible(hasPending);
    pending_->setText(hasPending ? "⏳ Module files differ from the last build. Run NCPatcher to assign IDs and refresh the extracted ROM." : QString{});
    const bool hasTarget = targetModule(); add_->setEnabled(hasTarget); replace_->setEnabled(false); remove_->setEnabled(false); extract_->setEnabled(false); graphics_->setEnabled(false); uiResource_->setEnabled(false); hex_->setEnabled(false);
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
    if (const auto* module = targetModule()) for (int i = 0; i < variant_->count(); ++i) {
        auto target = nsmb::project::module_file_target(*module, variant_->itemData(i).toString().toStdString(), romPath.toStdString(), mappings_);
        if (target && std::filesystem::exists(target.value())) existing.push_back(variant_->itemText(i));
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
    const auto* entry = selectedEntry(); if (!entry) return; const auto source = !entry->source.empty() ? entry->source : entry->extracted;
    auto data = nsmb::core::read_file(source); if (!data) { emit errorOccurred(QString::fromStdString(data.error().message)); return; }
    const auto destination = QFileDialog::getSaveFileName(this, "Extract file", QString::fromStdString(entry->path)); if (destination.isEmpty()) return;
    auto result = nsmb::core::write_file_atomically(destination.toStdString(), data.value()); if (!result) emit errorOccurred(QString::fromStdString(result.error().message));
}

void FilesystemDock::hexEditFile() {
    const auto* entry = selectedEntry(); if (!entry) return; const auto source = !entry->source.empty() ? entry->source : entry->extracted;
    auto data = nsmb::core::read_file(source); if (!data) { emit errorOccurred(QString::fromStdString(data.error().message)); return; }
    QDialog dialog(this); dialog.setWindowTitle("Hex edit " + QString::fromStdString(entry->path)); dialog.resize(760, 520);
    auto* layout = new QVBoxLayout(&dialog); auto* editor = new QPlainTextEdit(&dialog); editor->setLineWrapMode(QPlainTextEdit::NoWrap); editor->setPlainText(hex(data.value())); layout->addWidget(editor);
    auto* buttons = new QDialogButtonBox(QDialogButtonBox::Save | QDialogButtonBox::Cancel, &dialog); layout->addWidget(buttons); connect(buttons, &QDialogButtonBox::accepted, &dialog, &QDialog::accept); connect(buttons, &QDialogButtonBox::rejected, &dialog, &QDialog::reject);
    if (dialog.exec() != QDialog::Accepted) return; auto decoded = unhex(editor->toPlainText()); if (!decoded) { emit errorOccurred("Hex data contains an invalid or incomplete byte."); return; }
    auto result = writeTarget(entry->path, *decoded, entry); if (!result) emit errorOccurred(QString::fromStdString(result.error().message)); else emit filesChanged();
}

void FilesystemDock::graphicsEditFile() {
    const auto* entry = selectedEntry(); if (!entry) return; const auto source = !entry->source.empty() ? entry->source : entry->extracted;
    auto data = nsmb::core::read_file(source); if (!data) { emit errorOccurred(QString::fromStdString(data.error().message)); return; }
    GraphicsEditorDialog dialog(entry->path, data.value(), this); if (dialog.exec() != QDialog::Accepted) return;
    auto encoded = dialog.editedBytes(); if (!encoded) { emit errorOccurred(QString::fromStdString(encoded.error().message)); return; }
    auto result = writeTarget(entry->path, encoded.value(), entry); if (!result) emit errorOccurred(QString::fromStdString(result.error().message)); else emit filesChanged();
}

void FilesystemDock::uiResourceEditFile() {
    const auto* entry=selectedEntry();if(!entry)return;const auto source=!entry->source.empty()?entry->source:entry->extracted;auto data=nsmb::core::read_file(source);if(!data){emit errorOccurred(QString::fromStdString(data.error().message));return;}
    const bool metadata=entry->id&&*entry->id==131;UiResourceEditorDialog dialog(entry->path,data.value(),metadata,this);if(dialog.exec()!=QDialog::Accepted)return;auto encoded=dialog.editedBytes();if(!encoded){emit errorOccurred(QString::fromStdString(encoded.error().message));return;}auto result=writeTarget(entry->path,encoded.value(),entry);if(!result)emit errorOccurred(QString::fromStdString(result.error().message));else emit filesChanged();
}
