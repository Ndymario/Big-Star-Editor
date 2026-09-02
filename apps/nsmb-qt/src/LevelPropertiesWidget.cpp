#include "LevelPropertiesWidget.hpp"

#include <QComboBox>
#include <QFontDatabase>
#include <QFrame>
#include <QGraphicsOpacityEffect>
#include <QGridLayout>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QPushButton>
#include <QScrollArea>
#include <QSignalBlocker>
#include <QSpinBox>
#include <QTabBar>
#include <QVariantAnimation>
#include <QVBoxLayout>

#include <algorithm>
#include <array>
#include <optional>
#include <limits>
#include <set>
#include <string>
#include <utility>

namespace {

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

// The six bytes NSMBe shows and its stageobjsettings nybbles address: Event 1,
// Event 2, then the settings word from its high byte down.
QString formatSpriteData(std::uint64_t data) {
    auto text = QString("%1").arg(data & 0xffffffffffffULL, 12, 16, QChar('0')).toUpper();
    for (int position = 10; position > 0; position -= 2) text.insert(position, ' ');
    return text;
}

std::optional<std::uint64_t> parseSpriteData(QString text) {
    text.remove(' ');
    if (text.size() != 12) return std::nullopt;
    bool parsed = false;
    const auto data = text.toULongLong(&parsed, 16);
    return parsed ? std::optional<std::uint64_t>(data) : std::nullopt;
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
    contentLayout->setSpacing(8);

    auto* selectionGroup = new QGroupBox("Selection", content);
    auto* selectionLayout = new QGridLayout(selectionGroup);

    itemBox_ = new QSpinBox(this);
    itemBox_->setMinimum(0);
    itemBox_->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    selectionLayout->addWidget(new QLabel("Item", selectionGroup), 0, 0);
    selectionLayout->addWidget(itemBox_, 1, 0);

    nodeLabel_ = new QLabel("Node", this);
    nodeBox_ = new QSpinBox(this);
    nodeBox_->setMinimum(0);
    nodeBox_->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    selectionLayout->addWidget(nodeLabel_, 0, 1);
    selectionLayout->addWidget(nodeBox_, 1, 1);

    auto* propertiesGroup = new QGroupBox("Properties", content);
    auto* fieldsLayout = new QGridLayout(propertiesGroup);
    fieldsLayout->setHorizontalSpacing(10);
    fieldsLayout->setVerticalSpacing(7);

    for (std::size_t index = 0; index < fields_.size(); ++index) {
        auto& field = fields_[index];
        field.container = new QWidget(propertiesGroup);
        auto* fieldLayout = new QVBoxLayout(field.container);
        fieldLayout->setContentsMargins(0, 0, 0, 0);
        fieldLayout->setSpacing(2);
        field.label = new QLabel(field.container);
        field.label->setWordWrap(true);
        field.value = new QSpinBox(field.container);
        field.value->setObjectName(QString("PropertyField%1").arg(&field - fields_.data()));
        field.value->setRange(-32768, 65535);
        field.value->setAccelerated(true);
        field.value->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
        field.note = new QLabel(field.container);
        field.note->setObjectName(QString("PropertyNote%1").arg(index));
        field.note->setWordWrap(true);
        field.note->setTextFormat(Qt::PlainText);
        field.note->setForegroundRole(QPalette::PlaceholderText);
        auto noteFont = field.note->font();
        noteFont.setPointSizeF(noteFont.pointSizeF() * 0.9);
        field.note->setFont(noteFont);
        // A wrapped note only knows its height once it knows its width, and the
        // grid only asks when both the note and the widget it sits in say they
        // have an answer. Without that the row is laid out one line tall and
        // the note spills over its neighbours.
        auto notePolicy = field.note->sizePolicy();
        notePolicy.setVerticalPolicy(QSizePolicy::Minimum);
        notePolicy.setHeightForWidth(true);
        field.note->setSizePolicy(notePolicy);
        auto containerPolicy = field.container->sizePolicy();
        containerPolicy.setHeightForWidth(true);
        field.container->setSizePolicy(containerPolicy);
        field.note->setMaximumHeight(0);
        field.note->hide();
        field.note_fade = new QGraphicsOpacityEffect(field.note);
        field.note_fade->setOpacity(0.0);
        field.note->setGraphicsEffect(field.note_fade);
        field.note_reveal = new QVariantAnimation(this);
        field.note_reveal->setDuration(180);
        field.note_reveal->setEasingCurve(QEasingCurve::OutCubic);
        field.note_reveal->setStartValue(0.0);
        field.note_reveal->setEndValue(1.0);
        connect(field.note_reveal, &QVariantAnimation::valueChanged, this, [row = &field](const QVariant& value) {
            const auto progress = value.toDouble();
            row->note->setMaximumHeight(qRound(progress * row->note_height));
            row->note_fade->setOpacity(progress);
        });
        // Released at the end so a later resize can rewrap the note: an
        // animated height that stays put clips whatever the new width wraps to.
        connect(field.note_reveal, &QVariantAnimation::finished, this,
                [row = &field] { row->note->setMaximumHeight(QWIDGETSIZE_MAX); });
        fieldLayout->addWidget(field.label);
        fieldLayout->addWidget(field.value);
        fieldLayout->addWidget(field.note);
        // Keeps a field at the top of its grid row when the field beside it
        // opened a note and made the row tall.
        fieldLayout->addStretch();
        fieldsLayout->addWidget(field.container, static_cast<int>(index / 2), static_cast<int>(index % 2));
        connect(field.value, &QSpinBox::valueChanged, this, [this, current = &field, index] {
            if (!refreshing_) current->mixed = false;
            applyFields(index);
        });
    }
    fieldsLayout->setColumnStretch(0, 1);
    fieldsLayout->setColumnStretch(1, 1);
    // Below the fields, because it is what they are a reading of: an actor that
    // declares nothing still has its whole data value here, and one that
    // declares everything still shows what its fields add up to.
    settingsGroup_ = new QGroupBox("Settings", content);
    auto* settingsLayout = new QVBoxLayout(settingsGroup_);
    settingsLayout->setSpacing(4);
    settingsBox_ = new QLineEdit(settingsGroup_);
    settingsBox_->setObjectName("SpriteDataField");
    settingsBox_->setInputMask(">HH HH HH HH HH HH");
    settingsBox_->setFont(QFontDatabase::systemFont(QFontDatabase::FixedFont));
    settingsBox_->setToolTip("Event 1, Event 2, then the settings word from its high byte down.");
    auto* settingsCaption = new QLabel("Event 1, Event 2, settings", settingsGroup_);
    settingsCaption->setForegroundRole(QPalette::PlaceholderText);
    auto captionFont = settingsCaption->font();
    captionFont.setPointSizeF(captionFont.pointSizeF() * 0.9);
    settingsCaption->setFont(captionFont);
    settingsLayout->addWidget(settingsBox_);
    settingsLayout->addWidget(settingsCaption);
    connect(settingsBox_, &QLineEdit::textEdited, this, [this] { applySpriteData(); });

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
    refresh();
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
    for (auto& field : fields_) {
        field.container->hide();
        field.label->setToolTip({});
        field.value->setToolTip({});
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
    // note that is already open stays open instead of replaying its reveal on
    // every refresh -- and one whose row went away closes.
    std::array<QString, std::tuple_size_v<decltype(fields_)>> notes;
    auto field = [&](const char* name, std::optional<int> value, int minimum = 0, int maximum = 65535, QString note = {}) {
        const auto index = row++;
        auto& current = fields_[index];
        current.label->setText(name);
        current.mixed = !value.has_value();
        current.unset_value = minimum - 1;
        current.value->setRange(current.unset_value, maximum);
        current.value->setSpecialValueText("—");
        current.value->setValue(value.value_or(current.unset_value));
        current.container->show();
        notes[index] = std::move(note);
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
            field("X (tiles)", common([&](auto i) { return level_->objects()[i].x; }), -32768, 32767);
            field("Y (tiles)", common([&](auto i) { return level_->objects()[i].y; }), -32768, 32767);
            field("Width", common([&](auto i) { return level_->objects()[i].width; }), 1);
            field("Height", common([&](auto i) { return level_->objects()[i].height; }), 1);
            break;
        }
        case Kind::Sprite: {
            // Resolved before the rows are laid out: what the actor says about
            // itself belongs under the object ID that selected it.
            const nsmb::project::SpriteDefinition* definition = nullptr;
            if (!multi) {
                if (const auto hash = level_->sprite_hash(index)) definition = sprite_definitions_.by_hash(*hash);
                else if (const auto object = actor_object_ids_.find(course.sprites[index].id); object != actor_object_ids_.end())
                    definition = sprite_definitions_.by_object_id(object->second);
            }
            field("Object ID", common([&](auto i) { return course.sprites[i].id; }), 0, 65535,
                  definition ? QString::fromStdString(definition->notes) : QString());
            field("X (tiles)", common([&](auto i) { return course.sprites[i].x; }));
            field("Y (tiles)", common([&](auto i) { return course.sprites[i].y; }));
            field("Event 1", common([&](auto i) { return course.sprites[i].events[0]; }), 0, 255);
            field("Event 2", common([&](auto i) { return course.sprites[i].events[1]; }), 0, 255);
            if (definition && !definition->fields.empty()) {
                for (const auto& named : definition->fields) {
                    if (row >= fields_.size()) break;
                    field(named.label.c_str(), nsmb::project::read_sprite_field(spriteData(course.sprites[index]), named),
                          nsmb::project::sprite_field_min(named), nsmb::project::sprite_field_max(named),
                          QString::fromStdString(named.notes));
                    visible_sprite_fields_.push_back(&named);
                    // The note itself now reads under the field. The tooltip is
                    // where the value table stays: it is a lookup, not prose.
                    if (named.choices.empty()) continue;
                    QStringList choices;
                    for (const auto& choice : named.choices)
                        choices.push_back(QString("%1 = %2").arg(choice.value).arg(QString::fromStdString(choice.label)));
                    fields_[row - 1].label->setToolTip(choices.join("\n"));
                    fields_[row - 1].value->setToolTip(choices.join("\n"));
                }
            }
            break;
        }
        case Kind::Entrance: {
            field("X", common([&](auto i) { return course.entrances[i].x; }));
            field("Y", common([&](auto i) { return course.entrances[i].y; }));
            field("Camera X", common([&](auto i) { return course.entrances[i].camera_x; }));
            field("Camera Y", common([&](auto i) { return course.entrances[i].camera_y; }));
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
            field("X",v.x);field("Y",v.y);field("Width",v.width,1);field("Height",v.height,1);
            field("ID",v.id,0,255);field("Camera settings",v.camera_settings_id,0,255);field("BGM",v.bgm_id,0,255);
            field("Background",v.background_id,0,255);field("Tileset",v.tileset_id,0,255);field("Foreground",v.foreground_id,0,255);
            field("Light",v.light_type,0,255);field("Progress path",v.progress_path_id,0,255);break;
        }
        case Kind::Zone: {
            field("X", common([&](auto i) { return course.zones[i].x; }));
            field("Y", common([&](auto i) { return course.zones[i].y; }));
            field("Width", common([&](auto i) { return course.zones[i].width; }), 1);
            field("Height", common([&](auto i) { return course.zones[i].height; }), 1);
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
                field("X", node.x);
                field("Y", node.y);
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

    // The raw value is the sprite; the declared fields are only a reading of
    // it. So it is shown whether or not an actor was resolved, and it is the
    // panel's own edit that moves it -- a refresh only reports it.
    settingsGroup_->setVisible(kind() == Kind::Sprite);
    if (kind() == Kind::Sprite) {
        settingsBox_->setEnabled(level_ != nullptr && count != 0);
        QString text;
        if (level_ && !indices.empty()) {
            const auto& sprites = level_->course().sprites;
            const auto data = spriteData(sprites[indices.front()]);
            if (std::ranges::all_of(indices, [&](auto i) { return spriteData(sprites[i]) == data; })) text = formatSpriteData(data);
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
    const auto data = parseSpriteData(settingsBox_->text());
    if (!data) return;
    const auto indices = selectedIndices();
    if (indices.empty()) return;
    if (indices.size() == 1U) {
        auto sprite = level_->course().sprites[indices.front()];
        setSpriteData(sprite, *data);
        emitChange(level_->update_sprite(indices.front(), sprite));
        return;
    }
    nsmb::project::LevelEntityUpdates edits;
    for (const auto selected : indices) {
        auto sprite = level_->course().sprites[selected];
        setSpriteData(sprite, *data);
        edits.sprites.push_back({selected, sprite});
    }
    emitChange(level_->update_entities(std::move(edits), "Edit selection"));
}

void LevelPropertiesWidget::setFieldNote(FieldRow& row, const QString& text) {
    if (row.note_text == text) return;
    row.note_text = text;
    row.note_reveal->stop();
    if (text.isEmpty()) {
        // Nothing to read on the way out, so the row closes at once rather than
        // holding the rest of the panel down for the length of an animation.
        row.note->clear();
        row.note->hide();
        row.note->setMaximumHeight(0);
        row.note_fade->setOpacity(0.0);
        return;
    }
    row.note->setText(text);
    row.note->setMaximumHeight(QWIDGETSIZE_MAX);
    row.note->show();
    // heightForWidth is what the wrapped note really needs; sizeHint is the
    // fallback for a panel that has not been laid out yet.
    const auto width = row.note->width() > 0 ? row.note->width() : row.container->width();
    row.note_height = width > 0 ? row.note->heightForWidth(width) : 0;
    if (row.note_height <= 0) row.note_height = row.note->sizeHint().height();
    row.note->setMaximumHeight(0);
    row.note_fade->setOpacity(0.0);
    row.note_reveal->start();
}

void LevelPropertiesWidget::previewInteraction(const QPointF& delta, int resize_handle) {
    if (refreshing_ || !level_ || itemCount() == 0) return;
    const auto indices = selectedIndices();
    if (indices.empty()) return;

    auto show = [&](std::size_t field, int value) {
        if (fields_[field].mixed) return;
        const QSignalBlocker blocker(fields_[field].value);
        fields_[field].value->setValue(value);
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
    auto value = [&](std::size_t field) { return fields_[field].value->value(); };
    nsmb::core::Result<void> result;

    if (indices.size() > 1U) {
        auto changed = [&](std::size_t field, int original) { return fields_[field].mixed ? original : value(field); };
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
