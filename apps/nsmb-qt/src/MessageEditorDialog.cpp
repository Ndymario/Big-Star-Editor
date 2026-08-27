#include "MessageEditorDialog.hpp"

#include "nsmb/formats/message.hpp"

#include <QDialogButtonBox>
#include <QFile>
#include <QFileDialog>
#include <QFontDatabase>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QInputDialog>
#include <QLabel>
#include <QLineEdit>
#include <QMessageBox>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QSpinBox>
#include <QSplitter>
#include <QTableWidget>
#include <QTextStream>
#include <QVBoxLayout>

#include <algorithm>
#include <cctype>
#include <limits>
#include <optional>

namespace {

using namespace nsmb::formats;

QString toQt(const std::string& value) { return QString::fromUtf8(value.c_str(), static_cast<qsizetype>(value.size())); }
std::string fromQt(const QString& value) { const auto utf8 = value.toUtf8(); return {utf8.constData(), static_cast<std::size_t>(utf8.size())}; }

QString hex(const std::vector<std::uint8_t>& bytes) {
    QString out;
    for (const auto byte : bytes) out += QString("%1").arg(byte, 2, 16, QChar('0')).toUpper();
    return out;
}

std::optional<std::vector<std::uint8_t>> unhex(QString text) {
    text.remove(' ');
    if (text.size() % 2 != 0) return std::nullopt;
    for (const auto c : text) if (!std::isxdigit(static_cast<unsigned char>(c.toLatin1()))) return std::nullopt;
    const auto data = QByteArray::fromHex(text.toLatin1());
    return std::vector<std::uint8_t>(data.begin(), data.end());
}

// The text export is line-based, so a message's own newlines and backslashes have to survive
// as escapes. Escape sequences keep the [type:HEXDATA] spelling the NDS BMG Editor writes.
QString escapeLine(QString text) { return text.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "\\r"); }

QString unescapeLine(const QString& text) {
    QString out;
    for (int i = 0; i < text.size(); ++i) {
        if (text[i] != '\\' || i + 1 >= text.size()) { out += text[i]; continue; }
        const auto next = text[++i];
        if (next == 'n') out += '\n';
        else if (next == 'r') out += '\r';
        else if (next == '\\') out += '\\';
        else { out += '\\'; out += next; }
    }
    return out;
}

} // namespace

struct MessageEditorDialog::Impl {
    Impl(MessageEditorDialog* owner, std::string_view path, std::span<const std::uint8_t> source)
        : owner(owner), path(path), body(source.begin(), source.end()) {
        auto parsed = parse_bmg(body);
        if (!parsed) { error = parsed.error(); return; }
        bmg = std::move(parsed.value());
    }

    void buildUi() {
        owner->setWindowTitle(QString("Messages — %1").arg(toQt(path)));
        owner->resize(940, 660);
        auto* root = new QVBoxLayout(owner);

        info = new QLabel(owner);
        info->setObjectName("MessageInfo");
        info->setWordWrap(true);
        root->addWidget(info);

        filter = new QLineEdit(owner);
        filter->setObjectName("MessageFilter");
        filter->setPlaceholderText("Filter messages by text or ID");
        filter->setClearButtonEnabled(true);
        root->addWidget(filter);

        auto* split = new QSplitter(Qt::Vertical, owner);
        list = new QTableWidget(0, 3, split);
        list->setObjectName("MessageList");
        list->setHorizontalHeaderLabels({"#", "ID", "Message"});
        list->setSelectionBehavior(QAbstractItemView::SelectRows);
        list->setSelectionMode(QAbstractItemView::SingleSelection);
        list->setEditTriggers(QAbstractItemView::NoEditTriggers);
        list->verticalHeader()->setVisible(false);
        list->horizontalHeader()->setStretchLastSection(true);
        list->setColumnWidth(0, 60);
        list->setColumnWidth(1, 80);
        list->setColumnHidden(1, !bmg.has_mid1);
        split->addWidget(list);

        auto* lower = new QWidget(split);
        auto* lowerLayout = new QVBoxLayout(lower);
        lowerLayout->setContentsMargins(0, 0, 0, 0);
        editor = new QPlainTextEdit(lower);
        editor->setObjectName("MessageText");
        editor->setFont(QFontDatabase::systemFont(QFontDatabase::FixedFont));
        editor->setEnabled(false);
        lowerLayout->addWidget(editor, 1);

        auto* fields = new QFormLayout;
        identifier = new QSpinBox(lower);
        identifier->setObjectName("MessageId");
        identifier->setRange(0, std::numeric_limits<int>::max());
        attributes = new QLineEdit(lower);
        attributes->setObjectName("MessageAttributes");
        attributes->setPlaceholderText("INF1 attribute bytes, hexadecimal");
        if (bmg.has_mid1) fields->addRow("Message ID", identifier);
        if (bmg.attribute_size) fields->addRow("Attributes", attributes);
        lowerLayout->addLayout(fields);

        stats = new QLabel(lower);
        stats->setObjectName("MessageStats");
        stats->setWordWrap(true);
        lowerLayout->addWidget(stats);
        split->addWidget(lower);
        split->setStretchFactor(0, 3);
        split->setStretchFactor(1, 2);
        root->addWidget(split, 1);

        auto* actions = new QHBoxLayout;
        auto* add = new QPushButton("Add", owner);
        add->setObjectName("MessageAdd");
        auto* duplicate = new QPushButton("Duplicate", owner);
        duplicate->setObjectName("MessageDuplicate");
        remove = new QPushButton("Remove", owner);
        remove->setObjectName("MessageRemove");
        auto* insert = new QPushButton("Insert escape…", owner);
        insert->setObjectName("MessageInsertEscape");
        auto* exportText = new QPushButton("Export text…", owner);
        exportText->setObjectName("MessageExport");
        auto* importText = new QPushButton("Import text…", owner);
        importText->setObjectName("MessageImport");
        for (auto* button : {add, duplicate, remove, insert, exportText, importText}) actions->addWidget(button);
        actions->addStretch(1);
        root->addLayout(actions);

        buttons = new QDialogButtonBox(QDialogButtonBox::Save | QDialogButtonBox::Cancel, owner);
        root->addWidget(buttons);
        QObject::connect(buttons, &QDialogButtonBox::accepted, owner, &QDialog::accept);
        QObject::connect(buttons, &QDialogButtonBox::rejected, owner, &QDialog::reject);

        QObject::connect(list, &QTableWidget::currentCellChanged, owner,
                         [this](int row, int, int previous, int) { commit(previous); load(row); });
        QObject::connect(editor, &QPlainTextEdit::textChanged, owner, [this] { updateStats(); });
        QObject::connect(filter, &QLineEdit::textChanged, owner, [this] { applyFilter(); });
        QObject::connect(add, &QPushButton::clicked, owner, [this] { addMessage(false); });
        QObject::connect(duplicate, &QPushButton::clicked, owner, [this] { addMessage(true); });
        QObject::connect(remove, &QPushButton::clicked, owner, [this] { removeMessage(); });
        QObject::connect(insert, &QPushButton::clicked, owner, [this] { insertEscape(); });
        QObject::connect(exportText, &QPushButton::clicked, owner, [this] { exportToText(); });
        QObject::connect(importText, &QPushButton::clicked, owner, [this] { importFromText(); });

        rebuild();
        if (!bmg.messages.empty()) list->setCurrentCell(0, 2);
        updateInfo();
    }

    void updateInfo() {
        info->setText(QString("%1 message(s) · %2 · file ID %3 · %4 attribute byte(s)")
                          .arg(bmg.messages.size())
                          .arg(QString::fromUtf8(bmg_encoding_name(bmg.encoding).data(),
                                                 static_cast<qsizetype>(bmg_encoding_name(bmg.encoding).size())))
                          .arg(bmg.file_id)
                          .arg(bmg.attribute_size));
    }

    void rebuild() {
        const QSignalBlocker block(list);
        list->setRowCount(static_cast<int>(bmg.messages.size()));
        for (std::size_t i = 0; i < bmg.messages.size(); ++i) refreshRow(static_cast<int>(i));
        list->setColumnHidden(1, !bmg.has_mid1);
        applyFilter();
    }

    void refreshRow(int row) {
        const auto& message = bmg.messages[static_cast<std::size_t>(row)];
        const auto set = [this, row](int column, const QString& text) {
            if (auto* item = list->item(row, column)) { item->setText(text); return; }
            list->setItem(row, column, new QTableWidgetItem(text));
        };
        set(0, QString::number(row));
        set(1, message.id ? QString::number(*message.id) : QString());
        auto preview = toQt(bmg_message_plain_text(message));
        if (preview.isEmpty()) preview = "(empty)";
        set(2, preview);
    }

    // Writes whatever the editor holds back into the message it was loaded from.
    void commit(int row) {
        if (row < 0 || row >= static_cast<int>(bmg.messages.size()) || row != active) return;
        auto& message = bmg.messages[static_cast<std::size_t>(row)];
        message.parts = parse_bmg_message_text(fromQt(editor->toPlainText()));
        if (bmg.has_mid1) message.id = static_cast<std::uint32_t>(identifier->value());
        if (bmg.attribute_size) {
            if (auto decoded = unhex(attributes->text())) {
                decoded->resize(bmg.attribute_size, 0);
                message.attributes = std::move(*decoded);
            }
        }
        refreshRow(row);
    }

    void load(int row) {
        active = row;
        const auto valid = row >= 0 && row < static_cast<int>(bmg.messages.size());
        editor->setEnabled(valid);
        identifier->setEnabled(valid);
        attributes->setEnabled(valid);
        remove->setEnabled(valid);
        const QSignalBlocker blockEditor(editor), blockId(identifier), blockAttributes(attributes);
        if (!valid) { editor->clear(); attributes->clear(); updateStats(); return; }
        const auto& message = bmg.messages[static_cast<std::size_t>(row)];
        editor->setPlainText(toQt(bmg_message_text(message)));
        identifier->setValue(static_cast<int>(message.id.value_or(static_cast<std::uint32_t>(row))));
        attributes->setText(hex(message.attributes));
        updateStats();
    }

    void updateStats() {
        if (active < 0) { stats->setText("No message selected."); return; }
        const auto parts = parse_bmg_message_text(fromQt(editor->toPlainText()));
        int escapes{};
        QString plain;
        for (const auto& part : parts) {
            if (part.escape) ++escapes;
            else plain += toQt(part.text);
        }
        stats->setText(QString("%1 character(s) · %2 line(s) · %3 escape sequence(s)")
                           .arg(plain.size())
                           .arg(plain.count('\n') + 1)
                           .arg(escapes));
    }

    void applyFilter() {
        const auto needle = filter->text().trimmed();
        for (int row = 0; row < list->rowCount(); ++row) {
            if (needle.isEmpty()) { list->setRowHidden(row, false); continue; }
            const auto* text = list->item(row, 2);
            const auto* id = list->item(row, 1);
            const auto matches = (text && text->text().contains(needle, Qt::CaseInsensitive)) ||
                                 (id && id->text() == needle) || QString::number(row) == needle;
            list->setRowHidden(row, !matches);
        }
    }

    void addMessage(bool duplicate) {
        commit(active);
        BmgMessage message;
        if (duplicate && active >= 0 && active < static_cast<int>(bmg.messages.size()))
            message = bmg.messages[static_cast<std::size_t>(active)];
        message.attributes.resize(bmg.attribute_size, 0);
        // A duplicate must not clone the ID: two entries answering to the same ID would make the
        // game's lookup order decide which text it shows.
        if (bmg.has_mid1) {
            std::uint32_t next{};
            for (const auto& existing : bmg.messages) next = std::max(next, existing.id.value_or(0) + 1);
            message.id = next;
        } else {
            message.id.reset();
        }
        const auto row = static_cast<int>(bmg.messages.size());
        bmg.messages.push_back(std::move(message));
        rebuild();
        updateInfo();
        list->setCurrentCell(row, 2);
    }

    void removeMessage() {
        if (active < 0 || active >= static_cast<int>(bmg.messages.size())) return;
        // INF1 entries are addressed by index, so dropping one renumbers everything after it.
        if (QMessageBox::question(owner, "Remove message",
                                  QString("Remove message %1? Every later message shifts down one index, which "
                                          "breaks any code that indexes this file directly.")
                                      .arg(active)) != QMessageBox::Yes)
            return;
        const auto row = active;
        active = -1;
        bmg.messages.erase(bmg.messages.begin() + row);
        rebuild();
        updateInfo();
        list->setCurrentCell(std::min(row, list->rowCount() - 1), 2);
    }

    void insertEscape() {
        if (active < 0) return;
        bool accepted{};
        const auto text = QInputDialog::getText(owner, "Insert escape sequence",
                                                "Escape as type:hexdata, for example 255:00000100",
                                                QLineEdit::Normal, "255:00000000", &accepted)
                              .trimmed();
        if (!accepted || text.isEmpty()) return;
        const auto candidate = "[" + text + "]";
        const auto parts = parse_bmg_message_text(fromQt(candidate));
        if (parts.size() != 1 || !parts.front().escape) {
            QMessageBox::warning(owner, "Insert escape sequence",
                                 "Escapes are a decimal type, a colon, and an even number of hexadecimal digits.");
            return;
        }
        editor->insertPlainText(candidate);
    }

    void exportToText() {
        commit(active);
        const auto destination = QFileDialog::getSaveFileName(owner, "Export messages", {}, "Text files (*.txt)");
        if (destination.isEmpty()) return;
        QFile file(destination);
        if (!file.open(QIODevice::WriteOnly | QIODevice::Text)) {
            QMessageBox::warning(owner, "Export messages", "Could not write " + destination + ".");
            return;
        }
        QTextStream out(&file);
        out.setEncoding(QStringConverter::Utf8);
        out << "# BMG Text Export\n";
        out << "# Total messages: " << bmg.messages.size() << "\n";
        out << "# Encoding: " << QString::fromUtf8(bmg_encoding_name(bmg.encoding).data(),
                                                   static_cast<qsizetype>(bmg_encoding_name(bmg.encoding).size()))
            << "\n";
        out << "# Format: [index] message text with [TYPE:HEXDATA] for escape sequences\n";
        out << "# Line breaks inside a message are written as \\n, a literal backslash as \\\\\n";
        out << "#" << QString(70, '=') << "\n\n";
        for (std::size_t i = 0; i < bmg.messages.size(); ++i) {
            const auto text = escapeLine(toQt(bmg_message_text(bmg.messages[i])));
            out << QString("[%1] ").arg(i, 3, 10, QChar('0')) << (text.isEmpty() ? QString("[EMPTY]") : text) << "\n";
        }
    }

    void importFromText() {
        const auto source = QFileDialog::getOpenFileName(owner, "Import messages", {}, "Text files (*.txt)");
        if (source.isEmpty()) return;
        QFile file(source);
        if (!file.open(QIODevice::ReadOnly | QIODevice::Text)) {
            QMessageBox::warning(owner, "Import messages", "Could not read " + source + ".");
            return;
        }
        QTextStream in(&file);
        in.setEncoding(QStringConverter::Utf8);
        // Existing entries are updated in place so their INF1 attributes and MID1 IDs survive;
        // only indices past the end of the file are appended.
        int updated{}, appended{}, skipped{};
        while (!in.atEnd()) {
            const auto line = in.readLine();
            if (line.startsWith('#') || line.trimmed().isEmpty()) continue;
            const auto close = line.indexOf(']');
            if (!line.startsWith('[') || close < 0) { ++skipped; continue; }
            bool numeric{};
            const auto index = line.mid(1, close - 1).toInt(&numeric);
            if (!numeric || index < 0 || index > 0xffff) { ++skipped; continue; }
            auto text = line.mid(close + 1).trimmed();
            if (text == "[EMPTY]" || text == "[NULL]") text.clear();
            while (static_cast<int>(bmg.messages.size()) <= index) {
                BmgMessage blank;
                blank.attributes.resize(bmg.attribute_size, 0);
                if (bmg.has_mid1) blank.id = static_cast<std::uint32_t>(bmg.messages.size());
                bmg.messages.push_back(std::move(blank));
                ++appended;
            }
            bmg.messages[static_cast<std::size_t>(index)].parts = parse_bmg_message_text(fromQt(unescapeLine(text)));
            ++updated;
        }
        active = -1;
        rebuild();
        updateInfo();
        list->setCurrentCell(0, 2);
        QMessageBox::information(owner, "Import messages",
                                 QString("Updated %1 message(s), appended %2, skipped %3 unreadable line(s).")
                                     .arg(updated)
                                     .arg(appended)
                                     .arg(skipped));
    }

    nsmb::core::Result<std::vector<std::uint8_t>> output() {
        if (error) return *error;
        commit(active);
        return write_bmg(bmg);
    }

    MessageEditorDialog* owner;
    std::string path;
    std::vector<std::uint8_t> body;
    std::optional<nsmb::core::Error> error;
    Bmg bmg;
    int active{-1};
    QLabel* info{};
    QLineEdit* filter{};
    QTableWidget* list{};
    QPlainTextEdit* editor{};
    QSpinBox* identifier{};
    QLineEdit* attributes{};
    QLabel* stats{};
    QPushButton* remove{};
    QDialogButtonBox* buttons{};
};

MessageEditorDialog::MessageEditorDialog(std::string_view path, std::span<const std::uint8_t> bytes, QWidget* parent)
    : QDialog(parent), impl_(std::make_unique<Impl>(this, path, bytes)) { impl_->buildUi(); }

MessageEditorDialog::~MessageEditorDialog() = default;

bool MessageEditorDialog::supports(std::span<const std::uint8_t> bytes, std::string_view path) {
    Impl probe(nullptr, path, bytes);
    return !probe.error;
}

nsmb::core::Result<std::vector<std::uint8_t>> MessageEditorDialog::editedBytes() const { return impl_->output(); }
