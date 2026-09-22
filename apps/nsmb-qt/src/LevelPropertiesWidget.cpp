#include "LevelPropertiesWidget.hpp"

#include <QCheckBox>
#include <QComboBox>
#include <QCompleter>
#include <QDialog>
#include <QDialogButtonBox>
#include <QFontDatabase>
#include <QFontMetrics>
#include <QFrame>
#include <QGridLayout>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QPushButton>
#include <QRegularExpression>
#include <QScrollArea>
#include <QSignalBlocker>
#include <QSpinBox>
#include <QStyle>
#include <QTabBar>
#include <QToolButton>
#include <QToolTip>
#include <QTreeWidget>
#include <QVBoxLayout>

#include <algorithm>
#include <array>
#include <optional>
#include <limits>
#include <set>
#include <string>
#include <utility>

namespace {

// "0x" stays lower, the digits go upper, and either case is accepted back.
QString hexText(int value) {
    return value < 0 ? "-0x" + QString::number(-static_cast<qlonglong>(value), 16).toUpper()
                     : "0x" + QString::number(value, 16).toUpper();
}

// QSpinBox::setDisplayIntegerBase renders lowercase digits and cannot be told
// otherwise, so the box does its own reading and writing.
class FieldSpinBox final : public QSpinBox {
public:
    using QSpinBox::QSpinBox;

    void setHex(bool on) {
        if (hex_ == on) return;
        hex_ = on;
        // Re-renders the text through the new base without moving the value.
        const QSignalBlocker blocker(this);
        const auto held = value();
        setValue(minimum());
        setValue(held);
    }

protected:
    QString textFromValue(int value) const override {
        return hex_ ? hexText(value) : QSpinBox::textFromValue(value);
    }

    int valueFromText(const QString& text) const override {
        if (!hex_) return QSpinBox::valueFromText(text);
        auto digits = text.trimmed();
        const bool negative = digits.startsWith('-');
        if (negative) digits.remove(0, 1);
        if (digits.startsWith("0x", Qt::CaseInsensitive)) digits.remove(0, 2);
        const auto parsed = digits.toLongLong(nullptr, 16);
        return static_cast<int>(negative ? -parsed : parsed);
    }

    QValidator::State validate(QString& input, int& position) const override {
        if (!hex_) return QSpinBox::validate(input, position);
        if (input.isEmpty() || input == specialValueText()) return QValidator::Intermediate;
        static const QRegularExpression whole(R"(^-?(0[xX])?[0-9a-fA-F]+$)");
        if (whole.match(input).hasMatch()) {
            const auto held = valueFromText(input);
            return held >= minimum() && held <= maximum() ? QValidator::Acceptable
                                                          : QValidator::Intermediate;
        }
        static const QRegularExpression partial(R"(^-?(0([xX])?)?$)");
        return partial.match(input).hasMatch() ? QValidator::Intermediate : QValidator::Invalid;
    }

private:
    bool hex_{};
};

int firstUnusedNumber(const auto& values, int start, auto number) {
    std::set<int> used;
    for (const auto& value : values) {
        used.insert(number(value));
    }
    while (used.contains(start)) {
        ++start;
    }
    return start;
}

// Object ID, X, Y and the two event bytes come before an actor's own fields.
constexpr std::size_t sprite_field_row = 5;

std::uint64_t spriteData(const nsmb::formats::NsmbLevelSprite& sprite) {
    return sprite.settings | (static_cast<std::uint64_t>(sprite.events[1]) << 32U) |
           (static_cast<std::uint64_t>(sprite.events[0]) << 40U);
}

void setSpriteData(nsmb::formats::NsmbLevelSprite& sprite, std::uint64_t data) {
    sprite.settings = static_cast<std::uint32_t>(data);
    sprite.events[1] = static_cast<std::uint8_t>(data >> 32U);
    sprite.events[0] = static_cast<std::uint8_t>(data >> 40U);
}

// The settings word from its high byte down. The two event bytes are not
// settings: one names the event this actor raises and the other the event it
// waits for, and each has its own row above.
QString formatSettings(std::uint32_t settings) {
    auto text = QString("%1").arg(settings, 8, 16, QChar('0')).toUpper();
    for (int position = 6; position > 0; position -= 2) text.insert(position, ' ');
    return text;
}

std::optional<std::uint32_t> parseSettings(QString text) {
    text.remove(' ');
    if (text.size() != 8) return std::nullopt;
    bool parsed = false;
    const auto settings = text.toUInt(&parsed, 16);
    return parsed ? std::optional<std::uint32_t>(settings) : std::nullopt;
}

} // namespace

LevelPropertiesWidget::LevelPropertiesWidget(QWidget* parent) : QWidget(parent) {
    auto* layout = new QVBoxLayout(this);
    layout->setContentsMargins(8, 8, 8, 8);
    layout->setSpacing(8);

    kindTabs_ = new QTabBar(this); kindTabs_->setExpanding(false); kindTabs_->setUsesScrollButtons(true);
    for (const auto& [name, value] : std::array{
             std::pair{"Course", Kind::Level}, std::pair{"Banks", Kind::ObjectBanks}, std::pair{"Objects", Kind::Object},
             std::pair{"Sprites", Kind::Sprite}, std::pair{"Entrances", Kind::Entrance}, std::pair{"Views", Kind::View},
             std::pair{"Zones", Kind::Zone}, std::pair{"Paths", Kind::Path}, std::pair{"Progress", Kind::ProgressPath}}) {
        const auto tab = kindTabs_->addTab(name); kindTabs_->setTabData(tab, static_cast<int>(value));
    }
    layout->addWidget(kindTabs_);

    auto* scroll = new QScrollArea(this);
    scroll->setWidgetResizable(true);
    scroll->setFrameShape(QFrame::NoFrame);
    scroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    auto* content = new QWidget(scroll);
    auto* contentLayout = new QVBoxLayout(content);
    contentLayout->setContentsMargins(0, 0, 0, 0);
    contentLayout->setSpacing(10);

    auto* selectionGroup = new QGroupBox("Selection", content);
    auto* selectionLayout = new QGridLayout(selectionGroup);

    // The same label-left reading the fields below use, so the panel has one
    // grammar rather than two.
    selectionLayout->setHorizontalSpacing(12);
    selectionLayout->setVerticalSpacing(6);
    selectionLayout->setColumnStretch(1, 1);

    itemBox_ = new QSpinBox(this);
    itemBox_->setMinimum(0);
    itemBox_->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    selectionLayout->addWidget(new QLabel("Item", selectionGroup), 0, 0);
    selectionLayout->addWidget(itemBox_, 0, 1);

    nodeLabel_ = new QLabel("Node", this);
    nodeBox_ = new QSpinBox(this);
    nodeBox_->setMinimum(0);
    nodeBox_->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    selectionLayout->addWidget(nodeLabel_, 1, 0);
    selectionLayout->addWidget(nodeBox_, 1, 1);

    auto* propertiesGroup = new QGroupBox("Properties", content);
    auto* propertiesLayout = new QVBoxLayout(propertiesGroup);
    propertiesLayout->setSpacing(8);
    // The rows live in their own widget: `placeFields` empties and refills this
    // grid, and anything else in it would be swept out with them.
    auto* fieldsHost = new QWidget(propertiesGroup);
    fieldsLayout_ = new QGridLayout(fieldsHost);
    fieldsLayout_->setContentsMargins(0, 0, 0, 0);
    fieldsLayout_->setHorizontalSpacing(12);
    fieldsLayout_->setVerticalSpacing(6);

    for (std::size_t index = 0; index < fields_.size(); ++index) {
        auto& field = fields_[index];
        field.container = new QWidget(fieldsHost);
        // Label beside the editor, not above it. A label above makes the row's
        // height a reading of the label's length, and two rows side by side
        // then put their editors at two different heights.
        auto* fieldLayout = new QHBoxLayout(field.container);
        fieldLayout->setContentsMargins(0, 0, 0, 0);
        fieldLayout->setSpacing(8);

        field.label = new QLabel(field.container);
        // A wrapping label in a column that does not stretch is what made the
        // rows ragged. A long one is cut short here and read in full on hover.
        field.label->setWordWrap(false);
        field.label->setAlignment(Qt::AlignLeft | Qt::AlignVCenter);

        field.value = new FieldSpinBox(field.container);
        field.value->setObjectName(QString("PropertyField%1").arg(index));
        field.value->setRange(-32768, 65535);
        field.value->setAccelerated(true);
        field.value->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);

        field.choice = new QComboBox(field.container);
        field.choice->setObjectName(QString("PropertyChoice%1").arg(index));
        field.choice->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
        // A choice label can run to sixty characters. It elides rather than
        // forcing the dock wider, and the whole of it stays on the item.
        field.choice->setSizeAdjustPolicy(QComboBox::AdjustToMinimumContentsLengthWithIcon);
        field.choice->setMinimumContentsLength(12);
        field.choice->hide();

        field.toggle = new QCheckBox(field.container);
        field.toggle->setObjectName(QString("PropertyToggle%1").arg(index));
        field.toggle->hide();

        field.info = new QToolButton(field.container);
        field.info->setObjectName(QString("PropertyInfo%1").arg(index));
        field.info->setAutoRaise(true);
        field.info->setFocusPolicy(Qt::NoFocus);
        field.info->setIcon(style()->standardIcon(QStyle::SP_MessageBoxInformation));
        field.info->setIconSize({14, 14});
        field.info->hide();

        fieldLayout->addWidget(field.label);
        fieldLayout->addWidget(field.value, 1);
        fieldLayout->addWidget(field.choice, 1);
        fieldLayout->addWidget(field.toggle, 1);
        fieldLayout->addWidget(field.info);

        auto edited = [this, current = &field, index] {
            if (!refreshing_) current->mixed = false;
            applyFields(index);
        };
        connect(field.value, &QSpinBox::valueChanged, this, edited);
        // Both of these fire for the user and never for `setRowValue`, so a
        // refresh reporting a value cannot be mistaken for an edit of it.
        connect(field.choice, &QComboBox::activated, this, edited);
        connect(field.toggle, &QCheckBox::clicked, this, [this, current = &field, edited] {
            // Undecided is something the data says, not something the user can
            // assert, so the first click leaves that state for good.
            current->toggle->setTristate(false);
            current->representable = true;
            edited();
        });
        connect(field.info, &QToolButton::clicked, this, [this, current = &field] { showFieldNote(*current); });
    }
    fieldsLayout_->setColumnStretch(0, 1);
    fieldsLayout_->setColumnStretch(1, 1);
    // Sits with the values it governs rather than in the raw box, which is
    // hex either way -- and so it is reachable on every kind, not just sprites.
    hexBox_ = new QCheckBox("Display as hex", propertiesGroup);
    hexBox_->setObjectName("HexFields");
    connect(hexBox_, &QCheckBox::clicked, this, [this](bool on) {
        hex_fields_ = on;
        refresh();
    });
    propertiesLayout->addWidget(fieldsHost);
    propertiesLayout->addWidget(hexBox_);
    // Below the fields, because it is what they are a reading of: an actor that
    // declares nothing still has its whole data value here, and one that
    // declares everything still shows what its fields add up to.
    settingsGroup_ = new QGroupBox("Raw Fields", content);
    auto* settingsLayout = new QVBoxLayout(settingsGroup_);
    settingsLayout->setSpacing(4);
    settingsBox_ = new QLineEdit(settingsGroup_);
    settingsBox_->setObjectName("SpriteDataField");
    settingsBox_->setInputMask(">HH HH HH HH");
    settingsBox_->setFont(QFontDatabase::systemFont(QFontDatabase::FixedFont));
    settingsBox_->setToolTip("The settings word from its high byte down.");
    settingsLayout->addWidget(settingsBox_);
    connect(settingsBox_, &QLineEdit::textEdited, this, [this] { applySpriteData(); });

    // Above everything a sprite's fields mean anything in terms of: the panel
    // said "Object ID 42" and left the reader to know what 42 was.
    spriteHeader_ = new QWidget(content);
    auto* headerLayout = new QGridLayout(spriteHeader_);
    headerLayout->setContentsMargins(0, 0, 0, 0);
    headerLayout->setHorizontalSpacing(8);
    headerLayout->setVerticalSpacing(2);
    spriteName_ = new QLabel(spriteHeader_);
    spriteName_->setObjectName("SpriteName");
    auto nameFont = spriteName_->font();
    nameFont.setBold(true);
    spriteName_->setFont(nameFont);
    spriteCategory_ = new QLabel(spriteHeader_);
    spriteCategory_->setObjectName("SpriteCategory");
    spriteCategory_->setForegroundRole(QPalette::PlaceholderText);
    auto categoryFont = spriteCategory_->font();
    categoryFont.setPointSizeF(categoryFont.pointSizeF() * 0.9);
    spriteCategory_->setFont(categoryFont);
    spriteInfo_ = new QToolButton(spriteHeader_);
    spriteInfo_->setObjectName("SpriteInfo");
    spriteInfo_->setAutoRaise(true);
    spriteInfo_->setFocusPolicy(Qt::NoFocus);
    spriteInfo_->setIcon(style()->standardIcon(QStyle::SP_MessageBoxInformation));
    spriteInfo_->setToolTip("What this actor is, and what each of its fields does");
    spritePicker_ = new QComboBox(spriteHeader_);
    spritePicker_->setObjectName("SpritePicker");
    spritePicker_->setEditable(true);
    spritePicker_->setInsertPolicy(QComboBox::NoInsert);
    spritePicker_->setSizeAdjustPolicy(QComboBox::AdjustToMinimumContentsLengthWithIcon);
    spritePicker_->setMinimumContentsLength(12);
    headerLayout->addWidget(spriteName_, 0, 0);
    headerLayout->addWidget(spriteInfo_, 0, 1, 2, 1, Qt::AlignVCenter);
    headerLayout->addWidget(spriteCategory_, 1, 0);
    headerLayout->addWidget(spritePicker_, 2, 0, 1, 2);
    headerLayout->setColumnStretch(0, 1);
    connect(spriteInfo_, &QToolButton::clicked, this, [this] { showActorInfo(); });
    // The picker turns the Object ID row, which is what the level is actually
    // edited through -- so there is one write path, not two.
    connect(spritePicker_, &QComboBox::activated, this, [this](int item) {
        const auto type = spritePicker_->itemData(item);
        if (refreshing_ || !type.isValid()) return;
        fields_[0].mixed = false;
        fields_[0].value->setValue(type.toInt());
    });

    contentLayout->addWidget(spriteHeader_);
    contentLayout->addWidget(selectionGroup);
    contentLayout->addWidget(propertiesGroup);
    contentLayout->addWidget(settingsGroup_);

    auto* entityButtons = new QHBoxLayout;
    addButton_ = new QPushButton("Add", this);
    deleteButton_ = new QPushButton("Delete", this);
    entityButtons->addWidget(addButton_);
    entityButtons->addWidget(deleteButton_);
    contentLayout->addLayout(entityButtons);

    auto* nodeButtons = new QHBoxLayout;
    addNodeButton_ = new QPushButton("Add Node", this);
    deleteNodeButton_ = new QPushButton("Delete Node", this);
    nodeButtons->addWidget(addNodeButton_);
    nodeButtons->addWidget(deleteNodeButton_);
    contentLayout->addLayout(nodeButtons);
    contentLayout->addStretch();
    scroll->setWidget(content);
    layout->addWidget(scroll, 1);

    connect(kindTabs_, &QTabBar::currentChanged, this, [this] {
        selected_items_.clear();
        refresh();
        emit currentKindChanged(static_cast<int>(kind()));
    });
    connect(itemBox_, &QSpinBox::valueChanged, this, [this] {
        if (!refreshing_) selected_items_.clear();
        refresh();
    });
    connect(nodeBox_, &QSpinBox::valueChanged, this, [this] { refresh(); });
    connect(addButton_, &QPushButton::clicked, this, [this] { addEntity(); });
    connect(deleteButton_, &QPushButton::clicked, this, [this] { deleteEntity(); });
    connect(addNodeButton_, &QPushButton::clicked, this, [this] { addNode(); });
    connect(deleteNodeButton_, &QPushButton::clicked, this, [this] { deleteNode(); });
    refresh();
}

void LevelPropertiesWidget::setLevel(nsmb::project::EditableLevel* level) {
    level_ = level;
    selected_items_.clear();
    itemBox_->setValue(0);
    nodeBox_->setValue(0);
    refresh();
}

void LevelPropertiesWidget::setObjectBankOverlays(std::optional<nsmb::project::ObjectBankOverlayTable> overlays) { object_bank_overlays_=std::move(overlays);refresh(); }

void LevelPropertiesWidget::setSpriteDefinitions(const nsmb::project::SpriteDefinitionCatalog& catalog,
                                                 std::unordered_map<std::uint16_t, std::uint32_t> actor_object_ids) {
    sprite_definitions_ = catalog;
    actor_object_ids_ = std::move(actor_object_ids);
    current_definition_ = nullptr;
    rebuildSpritePicker();
    refresh();
}

// A sprite's Object ID is the actor's *type*, and only the level's own actor
// table says which definition that type resolves to. So the list is built from
// that table rather than from the catalog, whose ids are something else.
void LevelPropertiesWidget::rebuildSpritePicker() {
    const QSignalBlocker blocker(spritePicker_);
    spritePicker_->clear();
    std::vector<std::pair<QString, int>> entries;
    entries.reserve(actor_object_ids_.size());
    for (const auto& [type, object_id] : actor_object_ids_) {
        const auto* definition = sprite_definitions_.by_object_id(object_id);
        if (!definition || definition->name.empty()) continue;
        entries.emplace_back(QString::fromStdString(definition->name), static_cast<int>(type));
    }
    std::sort(entries.begin(), entries.end());
    for (const auto& [name, type] : entries) {
        spritePicker_->addItem(QString("%1 — %2").arg(type).arg(name), type);
        spritePicker_->setItemData(spritePicker_->count() - 1, name, Qt::ToolTipRole);
    }
    auto* completer = new QCompleter(spritePicker_->model(), spritePicker_);
    completer->setFilterMode(Qt::MatchContains);
    completer->setCaseSensitivity(Qt::CaseInsensitive);
    completer->setCompletionMode(QCompleter::PopupCompletion);
    spritePicker_->setCompleter(completer);
}

// One reading of a field value, so a choice entry and the box beside it never
// disagree about which base they are in.
QString LevelPropertiesWidget::formatFieldValue(int value) const {
    return hex_fields_ ? hexText(value) : QString::number(value);
}

void LevelPropertiesWidget::setRowEditor(std::size_t row, FieldEditor editor, int minimum, int maximum,
                                         const nsmb::project::SpriteFieldDefinition* named) {
    auto& current = fields_[row];
    current.editor = editor;
    current.representable = true;
    current.held_value = 0;
    // A hidden container does not mark its children hidden, so a row coming
    // back has to be told again which editor it wears.
    current.value->setVisible(editor == FieldEditor::Number);
    current.choice->setVisible(editor == FieldEditor::Choice);
    current.toggle->setVisible(editor == FieldEditor::Boolean);
    // A checkbox is its own label; anything else needs one beside it.
    current.label->setVisible(editor != FieldEditor::Boolean);

    switch (editor) {
    case FieldEditor::Number:
        current.unset_value = minimum - 1;
        current.value->setRange(current.unset_value, maximum);
        current.value->setSpecialValueText("—");
        static_cast<FieldSpinBox*>(current.value)->setHex(hex_fields_);
        break;
    case FieldEditor::Choice: {
        const QSignalBlocker blocker(current.choice);
        current.choice->clear();
        if (named) {
            for (const auto& choice : named->choices) {
                // The number leads the label: it is what the Settings box holds,
                // and it is what makes the gaps in a sparse list legible.
                current.choice->addItem(QString("%1 — %2").arg(formatFieldValue(static_cast<int>(choice.value)),
                                                               QString::fromStdString(choice.label)),
                                        static_cast<int>(choice.value));
                current.choice->setItemData(current.choice->count() - 1,
                                            QString::fromStdString(choice.label), Qt::ToolTipRole);
            }
        }
        break;
    }
    case FieldEditor::Boolean: {
        const QSignalBlocker blocker(current.toggle);
        current.toggle->setTristate(false);
        current.toggle->setText(current.label->text());
        break;
    }
    }
}

namespace {
// Marks the two items a choice row grows for itself, so the next value it is
// given replaces them instead of stacking another one behind.
constexpr int synthetic_choice_role = Qt::UserRole + 1;
} // namespace

void LevelPropertiesWidget::setRowValue(std::size_t row, std::optional<int> value) {
    auto& current = fields_[row];
    switch (current.editor) {
    case FieldEditor::Number: {
        const QSignalBlocker blocker(current.value);
        current.value->setValue(value.value_or(current.unset_value));
        break;
    }
    case FieldEditor::Choice: {
        const QSignalBlocker blocker(current.choice);
        for (int item = current.choice->count() - 1; item >= 0; --item)
            if (current.choice->itemData(item, synthetic_choice_role).toBool()) current.choice->removeItem(item);
        if (!value) {
            current.choice->insertItem(0, "—");
            current.choice->setItemData(0, true, synthetic_choice_role);
            current.choice->setCurrentIndex(0);
            break;
        }
        const auto item = current.choice->findData(*value);
        if (item >= 0) {
            current.choice->setCurrentIndex(item);
            break;
        }
        // The bits hold something this actor does not name. Saying so is the
        // honest reading; quietly snapping to a neighbour would not be.
        current.representable = false;
        current.held_value = *value;
        current.choice->addItem(QString("%1 — not listed").arg(formatFieldValue(*value)), *value);
        current.choice->setItemData(current.choice->count() - 1, true, synthetic_choice_role);
        current.choice->setCurrentIndex(current.choice->count() - 1);
        break;
    }
    case FieldEditor::Boolean: {
        const QSignalBlocker blocker(current.toggle);
        if (!value || (*value != 0 && *value != 1)) {
            // Either the selection disagrees, or the field is wider than the
            // value that turns it on and holds something in between.
            if (value) { current.representable = false; current.held_value = *value; }
            current.toggle->setTristate(true);
            current.toggle->setCheckState(Qt::PartiallyChecked);
            break;
        }
        current.toggle->setTristate(false);
        current.toggle->setCheckState(*value == 1 ? Qt::Checked : Qt::Unchecked);
        break;
    }
    }
}

std::optional<int> LevelPropertiesWidget::rowValue(std::size_t row) const {
    const auto& current = fields_[row];
    switch (current.editor) {
    case FieldEditor::Number:
        return current.value->value() == current.unset_value ? std::nullopt
                                                             : std::optional<int>(current.value->value());
    case FieldEditor::Choice: {
        const auto data = current.choice->currentData();
        return data.isValid() ? std::optional<int>(data.toInt()) : std::nullopt;
    }
    case FieldEditor::Boolean:
        switch (current.toggle->checkState()) {
        case Qt::Checked: return 1;
        case Qt::Unchecked: return 0;
        // Undecided because the selection disagrees is nothing to write;
        // undecided because the bits say something odd is those bits.
        default: return current.mixed ? std::nullopt : std::optional<int>(current.held_value);
        }
    }
    return std::nullopt;
}

int LevelPropertiesWidget::rowValueOr(std::size_t row, int fallback) const {
    return rowValue(row).value_or(fallback);
}

std::vector<std::size_t> LevelPropertiesWidget::selectedIndices() const {
    if (!selected_items_.empty()) return selected_items_;
    return itemCount() == 0 ? std::vector<std::size_t>{} : std::vector<std::size_t>{static_cast<std::size_t>(itemBox_->value())};
}

LevelPropertiesWidget::Kind LevelPropertiesWidget::kind() const {
    return static_cast<Kind>(kindTabs_->tabData(kindTabs_->currentIndex()).toInt());
}

LevelPropertiesWidget::Kind LevelPropertiesWidget::currentKind() const { return kind(); }

std::size_t LevelPropertiesWidget::itemCount() const {
    if (!level_) {
        return 0;
    }
    const auto& course = level_->course();
    switch (kind()) {
    case Kind::Level:
        return 1;
    case Kind::ObjectBanks:
        return course.object_banks.size();
    case Kind::Object:
        return level_->objects().size();
    case Kind::Sprite:
        return course.sprites.size();
    case Kind::Entrance:
        return course.entrances.size();
    case Kind::View:
        return course.views.size();
    case Kind::Zone:
        return course.zones.size();
    case Kind::Path:
        return course.paths.size();
    case Kind::ProgressPath:
        return course.progress_paths.size();
    }
    return 0;
}

void LevelPropertiesWidget::refresh() {
    refreshing_ = true;
    visible_sprite_fields_.clear();
    current_definition_ = nullptr;
    for (auto& field : fields_) {
        field.container->hide();
        field.label->setToolTip({});
        field.value->setToolTip({});
        field.choice->setToolTip({});
        field.toggle->setToolTip({});
        field.span = FieldSpan::Full;
    }

    const auto count = itemCount();
    selected_items_.erase(std::remove_if(selected_items_.begin(), selected_items_.end(), [count](auto index) { return index >= count; }),
                          selected_items_.end());
    const bool multi = selected_items_.size() > 1U;
    itemBox_->setEnabled(level_ && kind() != Kind::Level && count != 0 && !multi);
    itemBox_->setMaximum(static_cast<int>(count == 0 ? 0 : count - 1U));
    if (static_cast<std::size_t>(itemBox_->value()) >= count) {
        itemBox_->setValue(static_cast<int>(count == 0 ? 0 : count - 1U));
    }
    // Read after the clamp: switching kinds leaves the box holding the previous
    // kind's index, which can be past the end of the new kind's collection.
    const auto indices = selectedIndices();

    const bool path_kind = (kind() == Kind::Path || kind() == Kind::ProgressPath) && !multi;
    nodeLabel_->setVisible(path_kind);
    nodeBox_->setVisible(path_kind);
    addNodeButton_->setVisible(path_kind);
    deleteNodeButton_->setVisible(path_kind);
    addButton_->setVisible(kind() != Kind::Level && kind() != Kind::ObjectBanks);
    deleteButton_->setVisible(kind() != Kind::Level && kind() != Kind::ObjectBanks);
    addButton_->setEnabled(level_);
    deleteButton_->setEnabled(level_ && count != 0);

    std::size_t row = 0;
    // Filled as the rows are laid out and applied in one pass at the end, so a
    // row that went away loses the note it was carrying.
    std::array<QString, std::tuple_size_v<decltype(fields_)>> notes;
    auto field = [&](const char* name, std::optional<int> value, int minimum = 0, int maximum = 65535,
                     FieldSpan span = FieldSpan::Full) {
        const auto index = row++;
        auto& current = fields_[index];
        current.label->setText(name);
        current.mixed = !value.has_value();
        current.span = span;
        setRowEditor(index, FieldEditor::Number, minimum, maximum, nullptr);
        setRowValue(index, value);
        current.container->show();
        return index;
    };
    // Two fields share a line only when they are peers and their labels are
    // close enough in length that the editors beside them still line up.
    auto pair = [&](const char* first_name, std::optional<int> first_value,
                    const char* second_name, std::optional<int> second_value,
                    int minimum = 0, int maximum = 65535) {
        field(first_name, first_value, minimum, maximum, FieldSpan::Half);
        field(second_name, second_value, minimum, maximum, FieldSpan::Half);
    };

    auto declaredField = [&](const nsmb::project::SpriteFieldDefinition& named, std::optional<int> value) {
        const auto index = row++;
        auto& current = fields_[index];
        current.label->setText(QString::fromStdString(named.label));
        current.mixed = !value.has_value();
        current.span = FieldSpan::Full;
        // A field that is on or off is a checkbox; one that names its values is
        // picked from them; the rest are typed. No actor declares both.
        if (named.checkbox_value != 0) {
            setRowEditor(index, FieldEditor::Boolean, 0, 1, &named);
        } else if (!named.choices.empty()) {
            setRowEditor(index, FieldEditor::Choice, 0, 0, &named);
        } else {
            setRowEditor(index, FieldEditor::Number, nsmb::project::sprite_field_min(named),
                         nsmb::project::sprite_field_max(named), nullptr);
        }
        setRowValue(index, value);
        current.container->show();
        auto note = QString::fromStdString(named.notes);
        if (!current.representable) {
            if (!note.isEmpty()) note += "\n\n";
            note += QString("This actor does not name the value %1, which is what these bits hold.")
                        .arg(current.held_value);
        }
        notes[index] = std::move(note);
        visible_sprite_fields_.push_back(&named);
    };

    auto common = [&](auto getter) -> std::optional<int> {
        if (indices.empty()) return std::nullopt;
        const auto first = static_cast<int>(getter(indices.front()));
        return std::ranges::all_of(indices, [&](auto index) { return static_cast<int>(getter(index)) == first; })
                   ? std::optional<int>(first)
                   : std::nullopt;
    };

    if (level_ && count != 0) {
        const auto index = indices.empty() ? static_cast<std::size_t>(itemBox_->value()) : indices.front();
        const auto& course = level_->course();
        switch (kind()) {
        case Kind::Level: {
            const auto refs = level_->tileset_references();
            field("Tileset", refs.tileset_id, 0, 255);
            field("BG graphics", refs.background_graphics_id, 0, 255);
            field("BG screen", refs.background_screen_id, 0, 255);
            field("FG graphics", refs.foreground_graphics_id, 0, 255);
            field("FG screen", refs.foreground_screen_id, 0, 255);
            break;
        }
        case Kind::ObjectBanks: {
            const auto bank=course.object_banks[index];
            const auto label=object_bank_overlays_&&bank>0&&bank<=10 ? "Bank ID (overlay "+std::to_string((*object_bank_overlays_)[index][bank-1])+")" : "Bank ID";
            field(label.c_str(), bank, 0, 10);
            break;
        }
        case Kind::Object: {
            field("Object ID", common([&](auto i) { return level_->objects()[i].object_id; }), 0, 4095);
            field("Tileset slot", common([&](auto i) { return level_->objects()[i].tileset_slot; }), 0, 15);
            pair("X (tiles)", common([&](auto i) { return level_->objects()[i].x; }),
                 "Y (tiles)", common([&](auto i) { return level_->objects()[i].y; }), -32768, 32767);
            pair("Width", common([&](auto i) { return level_->objects()[i].width; }),
                 "Height", common([&](auto i) { return level_->objects()[i].height; }), 1);
            break;
        }
        case Kind::Sprite: {
            // Resolved before the rows are laid out: which actor this is decides
            // what the rest of the panel has to say.
            if (!multi) {
                if (const auto hash = level_->sprite_hash(index)) current_definition_ = sprite_definitions_.by_hash(*hash);
                else if (const auto object = actor_object_ids_.find(course.sprites[index].id); object != actor_object_ids_.end())
                    current_definition_ = sprite_definitions_.by_object_id(object->second);
            }
            const auto* definition = current_definition_;
            field("Object ID", common([&](auto i) { return course.sprites[i].id; }), 0, 65535);
            notes[0] = definition ? QString::fromStdString(definition->notes) : QString();
            pair("X (tiles)", common([&](auto i) { return course.sprites[i].x; }),
                 "Y (tiles)", common([&](auto i) { return course.sprites[i].y; }));
            // Not "Event 1" and "Event 2": one names the event this actor
            // raises when it fires, the other the event that lets it act.
            pair("Target event", common([&](auto i) { return course.sprites[i].events[0]; }),
                 "Trigger event", common([&](auto i) { return course.sprites[i].events[1]; }), 0, 255);
            notes[3] = "The event this actor activates when it fires.";
            notes[4] = "The event that has to be active for this actor to act.";
            if (definition) {
                for (const auto& named : definition->fields) {
                    if (row >= fields_.size()) break;
                    declaredField(named, nsmb::project::read_sprite_field(spriteData(course.sprites[index]), named));
                }
            }
            break;
        }
        case Kind::Entrance: {
            pair("X", common([&](auto i) { return course.entrances[i].x; }),
                 "Y", common([&](auto i) { return course.entrances[i].y; }));
            pair("Camera X", common([&](auto i) { return course.entrances[i].camera_x; }),
                 "Camera Y", common([&](auto i) { return course.entrances[i].camera_y; }));
            field("ID", common([&](auto i) { return course.entrances[i].id; }), 0, 255);
            field("Target area", common([&](auto i) { return course.entrances[i].target_stage_area; }), 0, 255);
            field("Target stage / pipe path", common([&](auto i) { return course.entrances[i].target_stage_index; }), 0, 255);
            field("Target group", common([&](auto i) { return course.entrances[i].target_stage_group; }), 0, 255);
            field("Target entrance", common([&](auto i) { return course.entrances[i].target_entrance; }), 0, 255);
            field("Camera zoom", common([&](auto i) { return course.entrances[i].camera_zoom; }), 0, 255);
            field("Type", common([&](auto i) { return course.entrances[i].type; }), 0, 255);
            field("Flags", common([&](auto i) { return course.entrances[i].flags; }), 0, 255);
            field("Unknown 0x10", common([&](auto i) { return course.entrances[i].unknown_10; }), 0, 255);
            field("Unknown 0x11", common([&](auto i) { return course.entrances[i].unknown_11; }), 0, 255);
            field("View ID", common([&](auto i) { return course.entrances[i].view_id; }), 0, 255);
            field("World-map node", common([&](auto i) { return course.entrances[i].target_worldmap_node; }), 0, 255);
            break;
        }
        case Kind::View: {
            const auto& v=course.views[index];
            pair("X",v.x,"Y",v.y);pair("Width",v.width,"Height",v.height,1);
            field("ID",v.id,0,255);field("Camera settings",v.camera_settings_id,0,255);field("BGM",v.bgm_id,0,255);
            field("Background",v.background_id,0,255);field("Tileset",v.tileset_id,0,255);field("Foreground",v.foreground_id,0,255);
            field("Light",v.light_type,0,255);field("Progress path",v.progress_path_id,0,255);break;
        }
        case Kind::Zone: {
            pair("X", common([&](auto i) { return course.zones[i].x; }),
                 "Y", common([&](auto i) { return course.zones[i].y; }));
            pair("Width", common([&](auto i) { return course.zones[i].width; }),
                 "Height", common([&](auto i) { return course.zones[i].height; }), 1);
            field("ID", common([&](auto i) { return course.zones[i].id; }), 0, 255);
            break;
        }
        case Kind::Path:
        case Kind::ProgressPath: {
            const auto& paths = kind() == Kind::ProgressPath ? course.progress_paths : course.paths;
            const auto& path = paths[index];
            nodeBox_->setEnabled(!path.nodes.empty());
            nodeBox_->setMaximum(static_cast<int>(path.nodes.empty() ? 0 : path.nodes.size() - 1U));
            if (static_cast<std::size_t>(nodeBox_->value()) >= path.nodes.size()) {
                nodeBox_->setValue(static_cast<int>(path.nodes.empty() ? 0 : path.nodes.size() - 1U));
            }
            field("Path ID", path.id);
            if (!path.nodes.empty()) {
                const auto& node = path.nodes[static_cast<std::size_t>(nodeBox_->value())];
                pair("X", node.x, "Y", node.y);
                field("Speed (fx32 raw)",node.speed.raw(),-2147483647,2147483647);
                field("Acceleration (fx32 raw)",node.acceleration.raw(),-2147483647,2147483647);
                field("Delay",node.delay);field("Flags",node.flags);
            }
            deleteNodeButton_->setEnabled(!path.nodes.empty());
            break;
        }
        }
    } else {
        nodeBox_->setEnabled(false);
        deleteNodeButton_->setEnabled(false);
    }

    for (std::size_t index = 0; index < fields_.size(); ++index) setFieldNote(fields_[index], notes[index]);
    placeFields();

    spriteHeader_->setVisible(kind() == Kind::Sprite);
    if (kind() == Kind::Sprite) {
        const auto type = level_ && count != 0 && !indices.empty()
                              ? std::optional<int>(level_->course().sprites[indices.front()].id)
                              : std::nullopt;
        if (current_definition_) {
            spriteName_->setText(QString::fromStdString(current_definition_->name));
            spriteName_->setForegroundRole(QPalette::WindowText);
            spriteCategory_->setText(QString("%1 · actor type %2")
                                         .arg(QString::fromStdString(current_definition_->category))
                                         .arg(type ? QString::number(*type) : QString("—")));
        } else {
            spriteName_->setText(type ? QString("Actor %1").arg(*type) : QString("No sprite selected"));
            spriteName_->setForegroundRole(QPalette::PlaceholderText);
            spriteCategory_->setText(multi ? "Several sprites selected" : "Not in the actor catalog");
        }
        spriteName_->setToolTip(spriteName_->text());
        spriteInfo_->setEnabled(current_definition_ != nullptr);
        const QSignalBlocker picker_blocker(spritePicker_);
        // A sprite placed by hash names an actor but has no type to pick, and a
        // catalog that never arrived has nothing to offer either way.
        const auto item = type ? spritePicker_->findData(*type) : -1;
        spritePicker_->setEnabled(spritePicker_->count() != 0 && !multi && type.has_value());
        spritePicker_->setCurrentIndex(item);
        if (item < 0) spritePicker_->setEditText(type ? QString("Actor %1").arg(*type) : QString());
    }

    // The raw value is the sprite; the declared fields are only a reading of
    // it. So it is shown whether or not an actor was resolved, and it is the
    // panel's own edit that moves it -- a refresh only reports it.
    settingsGroup_->setVisible(kind() == Kind::Sprite);
    if (kind() == Kind::Sprite) {
        settingsBox_->setEnabled(level_ != nullptr && count != 0);
        QString text;
        if (level_ && !indices.empty()) {
            const auto& sprites = level_->course().sprites;
            const auto settings = sprites[indices.front()].settings;
            if (std::ranges::all_of(indices, [&](auto i) { return sprites[i].settings == settings; }))
                text = formatSettings(settings);
        }
        if (!text.isEmpty()) {
            if (settingsBox_->text() != text) {
                // Typing here is what changed the level in the first place, so
                // the cursor stays where the user left it.
                const auto cursor = settingsBox_->cursorPosition();
                settingsBox_->setText(text);
                settingsBox_->setCursorPosition(cursor);
            }
        } else if (settingsBox_->hasAcceptableInput()) {
            settingsBox_->clear();
        }
    }

    refreshing_ = false;
    if (!multi) emit selectionChanged(static_cast<int>(kind()), itemBox_->value(), nodeBox_->value());
}

void LevelPropertiesWidget::applySpriteData() {
    if (refreshing_ || !level_ || kind() != Kind::Sprite || itemCount() == 0) return;
    // A half-typed value is not a value: the box keeps it until it is whole.
    const auto settings = parseSettings(settingsBox_->text());
    if (!settings) return;
    const auto indices = selectedIndices();
    if (indices.empty()) return;
    if (indices.size() == 1U) {
        auto sprite = level_->course().sprites[indices.front()];
        sprite.settings = *settings;
        emitChange(level_->update_sprite(indices.front(), sprite));
        return;
    }
    nsmb::project::LevelEntityUpdates edits;
    for (const auto selected : indices) {
        auto sprite = level_->course().sprites[selected];
        sprite.settings = *settings;
        edits.sprites.push_back({selected, sprite});
    }
    emitChange(level_->update_entities(std::move(edits), "Edit selection"));
}

// The note belongs to the row, and it says so by sitting on it: a button that
// is there when there is something to read and gone when there is not. Nothing
// about it changes the panel's height, so nothing shifts under the reader.
void LevelPropertiesWidget::setFieldNote(FieldRow& row, const QString& text) {
    row.note_text = text;
    row.info->setVisible(!text.isEmpty());
    row.info->setToolTip(text);
    row.info->setAccessibleDescription(text);
}

void LevelPropertiesWidget::showFieldNote(const FieldRow& row) {
    if (row.note_text.isEmpty()) return;
    QToolTip::showText(row.info->mapToGlobal(QPoint{0, row.info->height()}), row.note_text, row.info);
}

// What the actor is, and what all of it does, read off the same declarations the
// rows above are built from -- so a renamed field is renamed in both at once.
void LevelPropertiesWidget::showActorInfo() {
    if (!current_definition_) return;
    const auto& definition = *current_definition_;

    auto* dialog = new QDialog(this);
    dialog->setAttribute(Qt::WA_DeleteOnClose);
    dialog->setWindowTitle(QString::fromStdString(definition.name));
    dialog->resize(520, 420);
    auto* layout = new QVBoxLayout(dialog);

    auto* heading = new QLabel(QString("%1 · object %2")
                                   .arg(QString::fromStdString(definition.category))
                                   .arg(definition.object_id),
                               dialog);
    heading->setForegroundRole(QPalette::PlaceholderText);
    layout->addWidget(heading);

    auto* about = new QLabel(definition.notes.empty() ? QString("This actor documents nothing about itself.")
                                                      : QString::fromStdString(definition.notes),
                             dialog);
    about->setWordWrap(true);
    about->setTextInteractionFlags(Qt::TextSelectableByMouse);
    layout->addWidget(about);

    auto* fields = new QTreeWidget(dialog);
    fields->setObjectName("ActorInfoFields");
    fields->setHeaderLabels({"Field", "Bits", "What it does"});
    fields->setRootIsDecorated(true);
    for (const auto& named : definition.fields) {
        const auto bits = named.bit_width == 1
                              ? QString::number(named.bit_offset)
                              : QString("%1–%2").arg(named.bit_offset).arg(named.bit_offset + named.bit_width - 1U);
        auto* item = new QTreeWidgetItem(fields, {QString::fromStdString(named.label), bits,
                                                  QString::fromStdString(named.notes)});
        for (const auto& choice : named.choices)
            new QTreeWidgetItem(item, {QString::number(choice.value), {}, QString::fromStdString(choice.label)});
        if (named.checkbox_value != 0)
            new QTreeWidgetItem(item, {QString::number(named.checkbox_value), {}, "On; anything else is off"});
    }
    fields->expandAll();
    for (int column = 0; column < 2; ++column) fields->resizeColumnToContents(column);
    layout->addWidget(fields, 1);

    auto* buttons = new QDialogButtonBox(QDialogButtonBox::Close, dialog);
    connect(buttons, &QDialogButtonBox::rejected, dialog, &QDialog::reject);
    layout->addWidget(buttons);
    dialog->show();
}

// The grid is only rebuilt when the shape of the panel changed. A refresh runs
// on every edit, and re-laying it out under a user who is mid-keystroke would
// take the focus out from under them.
void LevelPropertiesWidget::placeFields() {
    std::vector<std::pair<std::size_t, FieldSpan>> wanted;
    for (std::size_t index = 0; index < fields_.size(); ++index) {
        auto& current = fields_[index];
        if (current.container->isHidden()) continue;
        // A list of values or a note to the side needs the whole width; so does
        // a label too long to leave room for the editor beside it.
        auto span = current.span;
        if (current.editor != FieldEditor::Number || !current.note_text.isEmpty()) span = FieldSpan::Full;
        wanted.emplace_back(index, span);
    }
    // A half that ended up alone beside a full one takes the width it was going
    // to share, rather than leaving a hole where its partner would have been.
    for (std::size_t item = 0; item < wanted.size(); ++item) {
        if (wanted[item].second != FieldSpan::Half) continue;
        const bool paired = item + 1 < wanted.size() && wanted[item + 1].second == FieldSpan::Half;
        if (paired) { ++item; continue; }
        wanted[item].second = FieldSpan::Full;
    }
    if (wanted == field_layout_) return;
    field_layout_ = wanted;

    while (auto* item = fieldsLayout_->takeAt(0)) delete item;

    // Every full-width row's editor starts at one edge. Without this the label
    // is as wide as its own text, so a long one pushes its editor further right
    // than its neighbour's and the column reads ragged -- which is most of what
    // was wrong with the old panel. A label past the cap wraps instead of
    // dragging the whole column with it.
    const auto cap = fields_[0].label->fontMetrics().horizontalAdvance(QString(18, 'n'));
    int label_column = 0;
    for (const auto& [index, span] : wanted) {
        if (span != FieldSpan::Full || fields_[index].editor == FieldEditor::Boolean) continue;
        label_column = std::max(label_column, std::min(fields_[index].label->sizeHint().width(), cap));
    }

    int grid_row = 0;
    for (std::size_t item = 0; item < wanted.size(); ++item) {
        const auto [index, span] = wanted[item];
        if (span == FieldSpan::Full) {
            // A row with the panel's whole width has no neighbour to fall out
            // of step with, so a long label may wrap rather than be cut off.
            fields_[index].label->setMinimumWidth(label_column);
            fields_[index].label->setMaximumWidth(std::max(label_column, cap));
            fields_[index].label->setWordWrap(true);
            fieldsLayout_->addWidget(fields_[index].container, grid_row++, 0, 1, 2);
            continue;
        }
        const auto [neighbour, _] = wanted[item + 1];
        // Peers line up when their labels do, so the shorter one is widened to
        // meet the longer instead of the two editors starting at two places.
        for (const auto side : {index, neighbour}) fields_[side].label->setWordWrap(false);
        const auto width = std::max(fields_[index].label->sizeHint().width(),
                                    fields_[neighbour].label->sizeHint().width());
        for (const auto side : {index, neighbour}) {
            fields_[side].label->setMinimumWidth(width);
            fields_[side].label->setMaximumWidth(QWIDGETSIZE_MAX);
        }
        fieldsLayout_->addWidget(fields_[index].container, grid_row, 0);
        fieldsLayout_->addWidget(fields_[neighbour].container, grid_row++, 1);
        ++item;
    }
}

void LevelPropertiesWidget::previewInteraction(const QPointF& delta, int resize_handle) {
    if (refreshing_ || !level_ || itemCount() == 0) return;
    const auto indices = selectedIndices();
    if (indices.empty()) return;

    auto show = [&](std::size_t field, int value) {
        if (fields_[field].mixed) return;
        setRowValue(field, value);
    };
    auto clamp16 = [](int value) { return std::clamp(value, 0, 65535); };
    auto clampSigned16 = [](int value) { return std::clamp(value, -32768, 32767); };
    auto common = [&](auto getter) -> std::optional<int> {
        const auto first = static_cast<int>(getter(indices.front()));
        return std::ranges::all_of(indices, [&](auto index) { return static_cast<int>(getter(index)) == first; })
                   ? std::optional<int>(first)
                   : std::nullopt;
    };

    if (resize_handle == 0) {
        const auto shifted = [&](std::size_t field, std::optional<int> original, double amount, double scale, bool signed_value) {
            if (!original) return;
            const auto value = *original + qRound(amount / scale);
            show(field, signed_value ? clampSigned16(value) : clamp16(value));
        };
        switch (kind()) {
        case Kind::Object:
            shifted(2, common([&](auto i) { return level_->objects()[i].x; }), delta.x(), 16.0, true);
            shifted(3, common([&](auto i) { return level_->objects()[i].y; }), delta.y(), 16.0, true);
            break;
        case Kind::Sprite:
            shifted(1, common([&](auto i) { return level_->course().sprites[i].x; }), delta.x(), 16.0, false);
            shifted(2, common([&](auto i) { return level_->course().sprites[i].y; }), delta.y(), 16.0, false);
            break;
        case Kind::Entrance:
            shifted(0, common([&](auto i) { return level_->course().entrances[i].x; }), delta.x(), 1.0, false);
            shifted(1, common([&](auto i) { return level_->course().entrances[i].y; }), delta.y(), 1.0, false);
            break;
        case Kind::Zone:
            shifted(0, common([&](auto i) { return level_->course().zones[i].x; }), delta.x(), 1.0, false);
            shifted(1, common([&](auto i) { return level_->course().zones[i].y; }), delta.y(), 1.0, false);
            break;
        case Kind::View:
            shifted(0, common([&](auto i) { return level_->course().views[i].x; }), delta.x(), 1.0, false);
            shifted(1, common([&](auto i) { return level_->course().views[i].y; }), delta.y(), 1.0, false);
            break;
        case Kind::Path:
        case Kind::ProgressPath: {
            if (indices.size() != 1U) break;
            const auto& paths = kind() == Kind::ProgressPath ? level_->course().progress_paths : level_->course().paths;
            const auto& nodes = paths[indices.front()].nodes;
            if (nodes.empty()) break;
            const auto& node = nodes[static_cast<std::size_t>(nodeBox_->value())];
            shifted(1, node.x, delta.x(), 1.0, true);
            shifted(2, node.y, delta.y(), 1.0, true);
            break;
        }
        default: break;
        }
        return;
    }

    if (indices.size() != 1U || (kind() != Kind::Object && kind() != Kind::Zone && kind() != Kind::View)) return;
    const auto left = resize_handle == 1 || resize_handle == 7 || resize_handle == 8;
    const auto right = resize_handle == 3 || resize_handle == 4 || resize_handle == 5;
    const auto top = resize_handle == 1 || resize_handle == 2 || resize_handle == 3;
    const auto bottom = resize_handle == 5 || resize_handle == 6 || resize_handle == 7;
    const auto resized = [](int origin, int extent, double near_edge, double far_edge, double scale) {
        const auto near_delta = qRound(near_edge / scale);
        const auto far_delta = qRound(far_edge / scale);
        auto result = std::pair{origin + near_delta, extent - near_delta + far_delta};
        if (result.second < 1) { result.first -= 1 - result.second; result.second = 1; }
        return result;
    };
    const auto index = indices.front();
    if (kind() == Kind::Object) {
        const auto& object = level_->objects()[index];
        const auto [x, width] = resized(object.x, object.width, left ? delta.x() : 0.0, right ? delta.x() : 0.0, 16.0);
        const auto [y, height] = resized(object.y, object.height, top ? delta.y() : 0.0, bottom ? delta.y() : 0.0, 16.0);
        show(2, clampSigned16(x)); show(3, clampSigned16(y)); show(4, clamp16(width)); show(5, clamp16(height));
    } else if (kind() == Kind::Zone) {
        const auto& value = level_->course().zones[index];
        const auto [x, width] = resized(value.x, value.width, left ? delta.x() : 0.0, right ? delta.x() : 0.0, 1.0);
        const auto [y, height] = resized(value.y, value.height, top ? delta.y() : 0.0, bottom ? delta.y() : 0.0, 1.0);
        show(0, clamp16(x)); show(1, clamp16(y)); show(2, clamp16(width)); show(3, clamp16(height));
    } else {
        const auto& value = level_->course().views[index];
        const auto [x, width] = resized(value.x, value.width, left ? delta.x() : 0.0, right ? delta.x() : 0.0, 1.0);
        const auto [y, height] = resized(value.y, value.height, top ? delta.y() : 0.0, bottom ? delta.y() : 0.0, 1.0);
        show(0, clamp16(x)); show(1, clamp16(y)); show(2, clamp16(width)); show(3, clamp16(height));
    }
}

void LevelPropertiesWidget::applyFields(std::size_t changed_row) {
    if (refreshing_ || !level_ || itemCount() == 0) {
        return;
    }
    const auto indices = selectedIndices();
    const auto index = indices.empty() ? static_cast<std::size_t>(itemBox_->value()) : indices.front();
    auto value = [&](std::size_t field) { return rowValueOr(field, 0); };
    nsmb::core::Result<void> result;

    if (indices.size() > 1U) {
        // A row that stands for more than one value writes none of them.
        auto changed = [&](std::size_t field, int original) { return rowValueOr(field, original); };
        nsmb::project::LevelEntityUpdates edits;
        switch (kind()) {
        case Kind::Object:
            for (const auto selected : indices) {
                auto object = level_->objects()[selected];
                object.object_id = static_cast<std::uint16_t>(changed(0, object.object_id));
                object.tileset_slot = static_cast<std::uint8_t>(changed(1, object.tileset_slot));
                object.x = static_cast<std::int16_t>(changed(2, object.x));
                object.y = static_cast<std::int16_t>(changed(3, object.y));
                object.width = static_cast<std::uint16_t>(changed(4, object.width));
                object.height = static_cast<std::uint16_t>(changed(5, object.height));
                edits.objects.push_back({selected, object});
            }
            break;
        case Kind::Sprite:
            for (const auto selected : indices) {
                auto sprite = level_->course().sprites[selected];
                sprite.id = static_cast<std::uint16_t>(changed(0, sprite.id));
                sprite.x = static_cast<std::uint16_t>(changed(1, sprite.x));
                sprite.y = static_cast<std::uint16_t>(changed(2, sprite.y));
                sprite.events[0]=static_cast<std::uint8_t>(changed(3,sprite.events[0]));
                sprite.events[1]=static_cast<std::uint8_t>(changed(4,sprite.events[1]));
                // No declared fields are shown for a multi-selection, and the
                // settings word is edited whole in its own box.
                edits.sprites.push_back({selected, sprite});
            }
            break;
        case Kind::Entrance:
            for (const auto selected : indices) {
                auto entrance = level_->course().entrances[selected];
                entrance.x = static_cast<std::uint16_t>(changed(0, entrance.x));
                entrance.y = static_cast<std::uint16_t>(changed(1, entrance.y));
                entrance.camera_x = static_cast<std::uint16_t>(changed(2, entrance.camera_x));
                entrance.camera_y = static_cast<std::uint16_t>(changed(3, entrance.camera_y));
                entrance.id=static_cast<std::uint8_t>(changed(4,entrance.id));entrance.target_stage_area=static_cast<std::uint8_t>(changed(5,entrance.target_stage_area));
                entrance.target_stage_index=static_cast<std::uint8_t>(changed(6,entrance.target_stage_index));entrance.target_stage_group=static_cast<std::uint8_t>(changed(7,entrance.target_stage_group));
                entrance.target_entrance=static_cast<std::uint8_t>(changed(8,entrance.target_entrance));entrance.camera_zoom=static_cast<std::uint8_t>(changed(9,entrance.camera_zoom));
                entrance.type=static_cast<std::uint8_t>(changed(10,entrance.type));entrance.flags=static_cast<std::uint8_t>(changed(11,entrance.flags));
                entrance.unknown_10=static_cast<std::uint8_t>(changed(12,entrance.unknown_10));entrance.unknown_11=static_cast<std::uint8_t>(changed(13,entrance.unknown_11));
                entrance.view_id=static_cast<std::uint8_t>(changed(14,entrance.view_id));entrance.target_worldmap_node=static_cast<std::uint8_t>(changed(15,entrance.target_worldmap_node));
                edits.entrances.push_back({selected, entrance});
            }
            break;
        case Kind::Zone:
            for (const auto selected : indices) {
                auto zone = level_->course().zones[selected];
                zone.x = static_cast<std::uint16_t>(changed(0, zone.x));
                zone.y = static_cast<std::uint16_t>(changed(1, zone.y));
                zone.width = static_cast<std::uint16_t>(changed(2, zone.width));
                zone.height = static_cast<std::uint16_t>(changed(3, zone.height));
                zone.id = static_cast<std::uint8_t>(changed(4, zone.id));
                edits.zones.push_back({selected, zone});
            }
            break;
        default:
            return;
        }
        emitChange(level_->update_entities(std::move(edits), "Edit selection"));
        return;
    }

    switch (kind()) {
    case Kind::Level:
        result = level_->set_tileset_references({static_cast<std::uint16_t>(value(0)),static_cast<std::uint16_t>(value(1)),static_cast<std::uint16_t>(value(2)),static_cast<std::uint16_t>(value(3)),static_cast<std::uint16_t>(value(4))});
        break;
    case Kind::ObjectBanks:
        result=level_->set_object_bank(index,static_cast<std::uint8_t>(value(0)));
        break;
    case Kind::Object: {
        auto object = level_->objects()[index];
        object.object_id = static_cast<std::uint16_t>(value(0));
        object.tileset_slot = static_cast<std::uint8_t>(value(1));
        object.x = static_cast<std::int16_t>(value(2));
        object.y = static_cast<std::int16_t>(value(3));
        object.width = static_cast<std::uint16_t>(value(4));
        object.height = static_cast<std::uint16_t>(value(5));
        result = level_->update_object(index, object);
        break;
    }
    case Kind::Sprite: {
        auto sprite = level_->course().sprites[index];
        sprite.id = static_cast<std::uint16_t>(value(0));
        sprite.x = static_cast<std::uint16_t>(value(1));
        sprite.y = static_cast<std::uint16_t>(value(2));
        sprite.events={static_cast<std::uint8_t>(value(3)),static_cast<std::uint8_t>(value(4))};
        // Only the field that was turned. Writing every visible one back would
        // commit whatever a spin box had to clamp to show, and would carry the
        // previous actor's fields into the settings when the object ID changes.
        if (changed_row >= sprite_field_row && changed_row - sprite_field_row < visible_sprite_fields_.size()) {
            const auto& named = *visible_sprite_fields_[changed_row - sprite_field_row];
            setSpriteData(sprite, nsmb::project::write_sprite_field(spriteData(sprite), named, value(changed_row)));
        }
        result = level_->update_sprite(index, sprite);
        break;
    }
    case Kind::Entrance: {
        auto entrance = level_->course().entrances[index];
        entrance.x = static_cast<std::uint16_t>(value(0));
        entrance.y = static_cast<std::uint16_t>(value(1));
        entrance.camera_x = static_cast<std::uint16_t>(value(2));
        entrance.camera_y = static_cast<std::uint16_t>(value(3));
        entrance.id=static_cast<std::uint8_t>(value(4));entrance.target_stage_area=static_cast<std::uint8_t>(value(5));entrance.target_stage_index=static_cast<std::uint8_t>(value(6));entrance.target_stage_group=static_cast<std::uint8_t>(value(7));
        entrance.target_entrance=static_cast<std::uint8_t>(value(8));entrance.camera_zoom=static_cast<std::uint8_t>(value(9));entrance.type=static_cast<std::uint8_t>(value(10));entrance.flags=static_cast<std::uint8_t>(value(11));
        entrance.unknown_10=static_cast<std::uint8_t>(value(12));entrance.unknown_11=static_cast<std::uint8_t>(value(13));entrance.view_id=static_cast<std::uint8_t>(value(14));entrance.target_worldmap_node=static_cast<std::uint8_t>(value(15));
        result = level_->update_entrance(index, entrance);
        break;
    }
    case Kind::Zone: {
        auto zone = level_->course().zones[index];
        zone.x = static_cast<std::uint16_t>(value(0));
        zone.y = static_cast<std::uint16_t>(value(1));
        zone.width = static_cast<std::uint16_t>(value(2));
        zone.height = static_cast<std::uint16_t>(value(3));
        zone.id = static_cast<std::uint8_t>(value(4));
        result = level_->update_zone(index, zone);
        break;
    }
    case Kind::Path:
    case Kind::ProgressPath: {
        const bool progress = kind() == Kind::ProgressPath;
        const auto& paths = progress ? level_->course().progress_paths : level_->course().paths;
        auto path = paths[index];
        path.id = static_cast<std::uint16_t>(value(0));
        if (!path.nodes.empty()) {
            auto& node = path.nodes[static_cast<std::size_t>(nodeBox_->value())];
            node.x = static_cast<std::int16_t>(value(1));node.y = static_cast<std::int16_t>(value(2));
            node.speed=nsmb::core::Fx32::from_raw(value(3));node.acceleration=nsmb::core::Fx32::from_raw(value(4));node.delay=static_cast<std::uint16_t>(value(5));node.flags=static_cast<std::uint16_t>(value(6));
        }
        result = level_->update_path(progress, index, std::move(path));
        break;
    }
    case Kind::View: { auto v=level_->course().views[index];v.x=value(0);v.y=value(1);v.width=value(2);v.height=value(3);v.id=value(4);v.camera_settings_id=value(5);v.bgm_id=value(6);v.background_id=value(7);v.tileset_id=value(8);v.foreground_id=value(9);v.light_type=value(10);v.progress_path_id=value(11);result=level_->update_view(index,v);break; }
    }
    emitChange(result);
}

void LevelPropertiesWidget::addEntity() {
    if (!level_) {
        return;
    }
    nsmb::core::Result<std::size_t> result = nsmb::core::make_error(nsmb::core::ErrorCode::Unsupported, "cannot add this item");
    switch (kind()) {
    case Kind::Level:
    case Kind::ObjectBanks:
        return;
    case Kind::Object:
        result = level_->add_object({0, 0, 0, 0, 1, 1});
        break;
    case Kind::Sprite:
        result = level_->add_sprite({});
        break;
    case Kind::Entrance: {
        nsmb::formats::NsmbLevelEntrance entrance;
        entrance.id = static_cast<std::uint8_t>(firstUnusedNumber(level_->course().entrances, 0, [](const auto& e) { return e.id; }));
        result = level_->add_entrance(entrance);
        break;
    }
    case Kind::View: { nsmb::formats::NsmbLevelView view{0,0,256,192}; view.id=static_cast<std::uint8_t>(firstUnusedNumber(level_->course().views,0,[](const auto& v){return v.id;})); result=level_->add_view(view); break; }
    case Kind::Zone: {
        nsmb::formats::NsmbLevelZone zone{0, 0, 256, 192, 0};
        zone.id = static_cast<std::uint8_t>(firstUnusedNumber(level_->course().zones, 0, [](const auto& z) { return z.id; }));
        result = level_->add_zone(zone);
        break;
    }
    case Kind::Path:
    case Kind::ProgressPath: {
        const bool progress = kind() == Kind::ProgressPath;
        const auto& paths = progress ? level_->course().progress_paths : level_->course().paths;
        nsmb::formats::NsmbLevelPath path;
        path.id = static_cast<std::uint8_t>(firstUnusedNumber(paths, progress ? 90 : 0, [](const auto& p) { return p.id; }));
        path.nodes.push_back({});
        result = level_->add_path(progress, std::move(path));
        break;
    }
    }
    if (!result) {
        emit errorOccurred(QString::fromStdString(result.error().message));
        return;
    }
    selected_items_.clear();
    itemBox_->setMaximum(static_cast<int>(itemCount() - 1U));
    itemBox_->setValue(static_cast<int>(result.value()));
    emit levelChanged();
    refresh();
}

void LevelPropertiesWidget::deleteEntity() {
    if (!level_ || itemCount() == 0) {
        return;
    }
    const auto selected = selectedIndices();
    if (selected.size() > 1U) {
        nsmb::project::LevelEntitySelection selection;
        auto* target = kind() == Kind::Object     ? &selection.objects
                       : kind() == Kind::Sprite   ? &selection.sprites
                       : kind() == Kind::Entrance ? &selection.entrances
                       : kind() == Kind::View    ? &selection.views
                       : kind() == Kind::Zone     ? &selection.zones
                       : kind() == Kind::Path     ? &selection.paths
                                                  : &selection.progress_paths;
        *target = selected;
        auto result = level_->remove_entities(std::move(selection));
        if (result) selected_items_.clear();
        emitChange(result);
        return;
    }
    const auto index = static_cast<std::size_t>(itemBox_->value());
    nsmb::core::Result<void> result;
    switch (kind()) {
    case Kind::Level:
    case Kind::ObjectBanks:
        return;
    case Kind::Object:
        result = level_->remove_object(index);
        break;
    case Kind::Sprite:
        result = level_->remove_sprite(index);
        break;
    case Kind::Entrance:
        result = level_->remove_entrance(index);
        break;
    case Kind::View:
        result = level_->remove_view(index);
        break;
    case Kind::Zone:
        result = level_->remove_zone(index);
        break;
    case Kind::Path:
        result = level_->remove_path(false, index);
        break;
    case Kind::ProgressPath:
        result = level_->remove_path(true, index);
        break;
    }
    emitChange(result);
}

void LevelPropertiesWidget::addNode() {
    if (!level_ || itemCount() == 0 || (kind() != Kind::Path && kind() != Kind::ProgressPath)) {
        return;
    }
    const bool progress = kind() == Kind::ProgressPath;
    const auto index = static_cast<std::size_t>(itemBox_->value());
    const auto& paths = progress ? level_->course().progress_paths : level_->course().paths;
    auto path = paths[index];
    auto node = path.nodes.empty() ? nsmb::formats::NsmbLevelPathNode{} : path.nodes.back();
    node.x = static_cast<std::int16_t>(node.x + 16);
    path.nodes.push_back(node);
    auto result = level_->update_path(progress, index, std::move(path));
    if (result) {
        nodeBox_->setMaximum(static_cast<int>(paths[index].nodes.size() - 1U));
        nodeBox_->setValue(static_cast<int>(paths[index].nodes.size() - 1U));
    }
    emitChange(result);
}

void LevelPropertiesWidget::deleteNode() {
    if (!level_ || itemCount() == 0 || (kind() != Kind::Path && kind() != Kind::ProgressPath)) {
        return;
    }
    const bool progress = kind() == Kind::ProgressPath;
    const auto index = static_cast<std::size_t>(itemBox_->value());
    const auto& paths = progress ? level_->course().progress_paths : level_->course().paths;
    auto path = paths[index];
    if (path.nodes.empty()) {
        return;
    }
    path.nodes.erase(path.nodes.begin() + nodeBox_->value());
    emitChange(level_->update_path(progress, index, std::move(path)));
}

void LevelPropertiesWidget::emitChange(const nsmb::core::Result<void>& result) {
    if (!result) {
        emit errorOccurred(QString::fromStdString(result.error().message));
        return;
    }
    emit levelChanged();
    refresh();
}

void LevelPropertiesWidget::select(Kind selected_kind, int item_index, int node_index) {
    selected_items_.clear();
    int box_index = -1; for (int i = 0; i < kindTabs_->count(); ++i) if (kindTabs_->tabData(i).toInt() == static_cast<int>(selected_kind)) box_index = i;
    if (box_index >= 0) {
        kindTabs_->setCurrentIndex(box_index);
    }
    itemBox_->setValue(std::max(0, item_index));
    nodeBox_->setValue(std::max(0, node_index));
    refresh();
}

void LevelPropertiesWidget::selectMany(Kind selected_kind, const QList<int>& item_indices) {
    selected_items_.clear();
    for (const auto item : item_indices) {
        if (item >= 0) selected_items_.push_back(static_cast<std::size_t>(item));
    }
    std::sort(selected_items_.begin(), selected_items_.end());
    selected_items_.erase(std::unique(selected_items_.begin(), selected_items_.end()), selected_items_.end());
    int box_index = -1; for (int i = 0; i < kindTabs_->count(); ++i) if (kindTabs_->tabData(i).toInt() == static_cast<int>(selected_kind)) box_index = i;
    const QSignalBlocker kind_blocker(kindTabs_);
    const QSignalBlocker item_blocker(itemBox_);
    if (box_index >= 0) {
        kindTabs_->setCurrentIndex(box_index);
    }
    if (!selected_items_.empty()) itemBox_->setValue(static_cast<int>(selected_items_.front()));
    refresh();
    // The tab bar's own signal is blocked above, so the change is announced here.
    emit currentKindChanged(static_cast<int>(kind()));
}
