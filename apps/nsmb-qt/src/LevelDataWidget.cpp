#include "LevelDataWidget.hpp"
#include "ThemeColors.hpp"

#include <QEvent>

#include <QByteArray>
#include <QHeaderView>
#include <QLabel>
#include <QTableWidget>
#include <QTreeWidget>
#include <QVBoxLayout>

#include <algorithm>

namespace {
constexpr auto hash_role = Qt::UserRole;
constexpr auto flag_role = Qt::UserRole + 1;
constexpr auto present_role = Qt::UserRole + 2;
constexpr std::uint32_t registration_hash = nsmb::formats::fnv1a_32("glue.stageObjects");

QString hex(std::span<const std::uint8_t> bytes) {
    return QByteArray(reinterpret_cast<const char*>(bytes.data()), static_cast<qsizetype>(bytes.size())).toHex(' ').toUpper();
}

std::optional<std::vector<std::uint8_t>> bytes(QString text) {
    text.remove(' '); text.remove('\t'); text.remove('\n');
    if (text.isEmpty() || text.size() % 2 != 0) return std::nullopt;
    for (const auto c : text) if (!c.isDigit() && (c.toLower() < 'a' || c.toLower() > 'f')) return std::nullopt;
    const auto decoded = QByteArray::fromHex(text.toLatin1());
    return std::vector<std::uint8_t>(decoded.begin(), decoded.end());
}

QString keyName(std::uint32_t hash, const nsmb::project::ModuleDataCatalog* catalog) {
    if (catalog) if (const auto* declaration = catalog->level_data_by_hash(hash)) return QString::fromStdString(declaration->key);
    return QString("0x%1").arg(hash, 8, 16, QLatin1Char('0'));
}
}

LevelDataWidget::LevelDataWidget(QWidget* parent) : QWidget(parent) {
    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(4, 4, 4, 4);
    error_ = new QLabel(this);
    error_->setWordWrap(true);
    applyThemeColors();
    keys_ = new QTableWidget(this);
    keys_->setColumnCount(3);
    keys_->setHorizontalHeaderLabels({"Key", "Shape", "Value / raw bytes"});
    keys_->horizontalHeader()->setSectionResizeMode(0, QHeaderView::ResizeToContents);
    keys_->horizontalHeader()->setSectionResizeMode(1, QHeaderView::ResizeToContents);
    keys_->horizontalHeader()->setSectionResizeMode(2, QHeaderView::Stretch);
    registration_ = new QTreeWidget(this);
    registration_->setHeaderLabels({"Stage ID", "Registration"});
    registration_->header()->setSectionResizeMode(0, QHeaderView::ResizeToContents);
    registration_->header()->setSectionResizeMode(1, QHeaderView::Stretch);
    layout->addWidget(error_);
    layout->addWidget(new QLabel("Level data", this));
    layout->addWidget(keys_, 2);
    layout->addWidget(new QLabel("Derived registration (read-only)", this));
    layout->addWidget(registration_, 1);
    connect(keys_, &QTableWidget::cellChanged, this, &LevelDataWidget::editCell);
}

void LevelDataWidget::setLevel(nsmb::project::EditableLevel* level, const nsmb::project::ModuleDataCatalog* catalog) {
    level_ = level; catalog_ = catalog; refresh();
}

void LevelDataWidget::applyThemeColors() {
    if (!error_) return;
    auto label = error_->palette();
    label.setColor(QPalette::WindowText, bse::theme::danger(palette()));
    error_->setPalette(label);
}

void LevelDataWidget::changeEvent(QEvent* event) {
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

void LevelDataWidget::refresh() {
    refreshing_ = true;
    keys_->clearContents(); keys_->setRowCount(0); registration_->clear();
    error_->clear();
    if (!level_) { setEnabled(false); refreshing_ = false; return; }
    setEnabled(true);
    if (level_->glue_error()) error_->setText("Malformed glue data: " + QString::fromStdString(*level_->glue_error()) + "\nThis block is read-only and will be preserved byte-for-byte.");

    struct Row { std::uint32_t hash{}; bool flag{}, present{}; std::vector<std::uint8_t> payload; };
    std::vector<Row> rows;
    for (const auto hash : level_->glue().flags) rows.push_back({hash, true, true, {}});
    for (const auto& value : level_->glue().values) if (value.hash != registration_hash) rows.push_back({value.hash, false, true, value.payload});
    if (catalog_) for (const auto& declaration : catalog_->level_data) {
        if (declaration.hash == registration_hash) continue;
        const bool flag = declaration.kind == nsmb::project::LevelDataKind::Flag;
        if (!std::ranges::any_of(rows, [&](const Row& row) { return row.hash == declaration.hash && row.flag == flag; }))
            rows.push_back({declaration.hash, flag, false, {}});
    }
    for (const auto& row : rows) {
        const auto index = keys_->rowCount(); keys_->insertRow(index);
        auto* key = new QTableWidgetItem(keyName(row.hash, catalog_));
        key->setData(hash_role, static_cast<qulonglong>(row.hash)); key->setData(flag_role, row.flag); key->setData(present_role, row.present);
        key->setFlags(key->flags() & ~Qt::ItemIsEditable);
        auto* shape = new QTableWidgetItem(row.flag ? "flag" : "value"); shape->setFlags(shape->flags() & ~Qt::ItemIsEditable);
        auto* value = new QTableWidgetItem(row.flag ? QString{} : hex(row.payload));
        if (row.flag) { value->setFlags((value->flags() & ~Qt::ItemIsEditable) | Qt::ItemIsUserCheckable); value->setCheckState(row.present ? Qt::Checked : Qt::Unchecked); }
        else if (!row.present) value->setText("<absent — enter bytes to add>");
        if (level_->glue_error()) value->setFlags(value->flags() & ~(Qt::ItemIsEditable | Qt::ItemIsUserCheckable));
        keys_->setItem(index, 0, key); keys_->setItem(index, 1, shape); keys_->setItem(index, 2, value);
    }

    const auto hashes = level_->registration_hashes();
    for (std::size_t i = 0; i < hashes.size(); ++i) {
        const auto hash = hashes[i];
        QString name = QString("0x%1 (undeclared)").arg(hash, 8, 16, QLatin1Char('0'));
        if (catalog_) if (const auto* entry = catalog_->stage_object_by_hash(hash)) name = QString::fromStdString(entry->name);
        new QTreeWidgetItem(registration_, {QString::number(level_->stage_object_id_base() + i), name});
    }
    refreshing_ = false;
}

void LevelDataWidget::editCell(int row, int column) {
    if (refreshing_ || column != 2 || !level_ || level_->glue_error()) return;
    auto* key = keys_->item(row, 0); auto* value = keys_->item(row, 2); if (!key || !value) return;
    const auto hash = static_cast<std::uint32_t>(key->data(hash_role).toULongLong());
    const bool flag = key->data(flag_role).toBool();
    auto glue = level_->glue();
    if (flag) {
        const auto found = std::ranges::find(glue.flags, hash);
        if (value->checkState() == Qt::Checked && found == glue.flags.end()) glue.flags.push_back(hash);
        if (value->checkState() == Qt::Unchecked && found != glue.flags.end()) glue.flags.erase(found);
    } else {
        auto decoded = bytes(value->text());
        if (!decoded) { emit errorOccurred("Values must contain a non-empty, even number of hexadecimal digits."); refresh(); return; }
        auto found = std::ranges::find(glue.values, hash, &nsmb::formats::GlueValue::hash);
        if (found == glue.values.end()) {
            std::size_t order{}; for (const auto& existing : glue.values) order = std::max(order, existing.payload_order + 1);
            glue.values.push_back({hash, std::move(*decoded), order});
        } else found->payload = std::move(*decoded);
    }
    auto result = level_->set_glue(std::move(glue));
    if (!result) emit errorOccurred(QString::fromStdString(result.error().message)); else emit levelChanged();
    refresh();
}
