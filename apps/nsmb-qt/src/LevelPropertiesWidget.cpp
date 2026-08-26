#include "LevelPropertiesWidget.hpp"

#include <QComboBox>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QSpinBox>
#include <QVBoxLayout>

#include <algorithm>
#include <array>
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

} // namespace

LevelPropertiesWidget::LevelPropertiesWidget(QWidget* parent) : QWidget(parent) {
    auto* layout = new QVBoxLayout(this);
    auto* form = new QFormLayout;

    kindBox_ = new QComboBox(this);
    kindBox_->addItem("Level settings", static_cast<int>(Kind::Level));
    kindBox_->addItem("Objects", static_cast<int>(Kind::Object));
    kindBox_->addItem("Sprites", static_cast<int>(Kind::Sprite));
    kindBox_->addItem("Entrances", static_cast<int>(Kind::Entrance));
    kindBox_->addItem("Zones", static_cast<int>(Kind::Zone));
    kindBox_->addItem("Paths", static_cast<int>(Kind::Path));
    kindBox_->addItem("Progress paths", static_cast<int>(Kind::ProgressPath));
    form->addRow("Edit", kindBox_);

    itemBox_ = new QSpinBox(this);
    itemBox_->setMinimum(0);
    form->addRow("Item", itemBox_);

    nodeLabel_ = new QLabel("Node", this);
    nodeBox_ = new QSpinBox(this);
    nodeBox_->setMinimum(0);
    form->addRow(nodeLabel_, nodeBox_);

    for (auto& field : fields_) {
        field.label = new QLabel(this);
        field.value = new QSpinBox(this);
        field.value->setRange(-32768, 65535);
        form->addRow(field.label, field.value);
        connect(field.value, &QSpinBox::valueChanged, this, [this] { applyFields(); });
    }
    layout->addLayout(form);

    auto* entityButtons = new QHBoxLayout;
    addButton_ = new QPushButton("Add", this);
    deleteButton_ = new QPushButton("Delete", this);
    entityButtons->addWidget(addButton_);
    entityButtons->addWidget(deleteButton_);
    layout->addLayout(entityButtons);

    auto* nodeButtons = new QHBoxLayout;
    addNodeButton_ = new QPushButton("Add Node", this);
    deleteNodeButton_ = new QPushButton("Delete Node", this);
    nodeButtons->addWidget(addNodeButton_);
    nodeButtons->addWidget(deleteNodeButton_);
    layout->addLayout(nodeButtons);
    layout->addStretch();

    connect(kindBox_, &QComboBox::currentIndexChanged, this, [this] { refresh(); });
    connect(itemBox_, &QSpinBox::valueChanged, this, [this] { refresh(); });
    connect(nodeBox_, &QSpinBox::valueChanged, this, [this] { refresh(); });
    connect(addButton_, &QPushButton::clicked, this, [this] { addEntity(); });
    connect(deleteButton_, &QPushButton::clicked, this, [this] { deleteEntity(); });
    connect(addNodeButton_, &QPushButton::clicked, this, [this] { addNode(); });
    connect(deleteNodeButton_, &QPushButton::clicked, this, [this] { deleteNode(); });
    refresh();
}

void LevelPropertiesWidget::setLevel(nsmb::project::EditableLevel* level) {
    level_ = level;
    itemBox_->setValue(0);
    nodeBox_->setValue(0);
    refresh();
}

LevelPropertiesWidget::Kind LevelPropertiesWidget::kind() const {
    return static_cast<Kind>(kindBox_->currentData().toInt());
}

std::size_t LevelPropertiesWidget::itemCount() const {
    if (!level_) {
        return 0;
    }
    const auto& course = level_->course();
    switch (kind()) {
    case Kind::Level:
        return 1;
    case Kind::Object:
        return level_->objects().size();
    case Kind::Sprite:
        return course.sprites.size();
    case Kind::Entrance:
        return course.entrances.size();
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
    for (auto& field : fields_) {
        field.label->hide();
        field.value->hide();
    }

    const auto count = itemCount();
    itemBox_->setEnabled(level_ && kind() != Kind::Level && count != 0);
    itemBox_->setMaximum(static_cast<int>(count == 0 ? 0 : count - 1U));
    if (static_cast<std::size_t>(itemBox_->value()) >= count) {
        itemBox_->setValue(static_cast<int>(count == 0 ? 0 : count - 1U));
    }

    const bool path_kind = kind() == Kind::Path || kind() == Kind::ProgressPath;
    nodeLabel_->setVisible(path_kind);
    nodeBox_->setVisible(path_kind);
    addNodeButton_->setVisible(path_kind);
    deleteNodeButton_->setVisible(path_kind);
    addButton_->setVisible(kind() != Kind::Level);
    deleteButton_->setVisible(kind() != Kind::Level);
    addButton_->setEnabled(level_);
    deleteButton_->setEnabled(level_ && count != 0);

    std::size_t row = 0;
    auto field = [&](const char* name, int value, int minimum = 0, int maximum = 65535) {
        auto& current = fields_[row++];
        current.label->setText(name);
        current.value->setRange(minimum, maximum);
        current.value->setValue(value);
        current.label->show();
        current.value->show();
    };

    if (level_ && count != 0) {
        const auto index = static_cast<std::size_t>(itemBox_->value());
        const auto& course = level_->course();
        switch (kind()) {
        case Kind::Level: {
            const auto refs = level_->tileset_references();
            field("Tileset", refs.tileset_id, 0, 255);
            field("BG graphics", refs.background_graphics_id, 0, 255);
            field("BG screen", refs.background_screen_id, 0, 255);
            field("FG graphics", refs.foreground_graphics_id, 0, 255);
            break;
        }
        case Kind::Object: {
            const auto& value = level_->objects()[index];
            field("Object ID", value.object_id, 0, 4095);
            field("Tileset slot", value.tileset_slot, 0, 15);
            field("X (tiles)", value.x, -32768, 32767);
            field("Y (tiles)", value.y, -32768, 32767);
            field("Width", value.width, 1);
            field("Height", value.height, 1);
            break;
        }
        case Kind::Sprite: {
            const auto& value = course.sprites[index];
            field("Type", value.type);
            field("X (tiles)", value.x);
            field("Y (tiles)", value.y);
            for (std::size_t i = 0; i < value.data.size(); ++i) {
                field(("Data " + std::to_string(i + 1U)).c_str(), value.data[i], 0, 255);
            }
            break;
        }
        case Kind::Entrance: {
            const auto& value = course.entrances[index];
            field("X", value.x);
            field("Y", value.y);
            field("Camera X", value.camera_x);
            field("Camera Y", value.camera_y);
            field("Number", value.number, 0, 255);
            field("Destination area", value.destination_area, 0, 255);
            field("Pipe ID", value.connected_pipe_id, 0, 255);
            field("Destination entrance", value.destination_entrance, 0, 255);
            field("Type", value.type, 0, 255);
            field("Settings", value.settings, 0, 255);
            field("Unknown 1", value.unknown1, 0, 255);
            field("Entry view", value.entry_view, 0, 255);
            field("Unknown 2", value.unknown2, 0, 255);
            break;
        }
        case Kind::Zone: {
            const auto& value = course.zones[index];
            field("X", value.x);
            field("Y", value.y);
            field("Width", value.width, 1);
            field("Height", value.height, 1);
            field("Number", value.number, 0, 255);
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
                for (std::size_t i = 0; i < node.values.size(); ++i) {
                    field(("Value " + std::to_string(i + 1U)).c_str(), node.values[i]);
                }
            }
            deleteNodeButton_->setEnabled(!path.nodes.empty());
            break;
        }
        }
    } else {
        nodeBox_->setEnabled(false);
        deleteNodeButton_->setEnabled(false);
    }

    refreshing_ = false;
    emit selectionChanged(static_cast<int>(kind()), itemBox_->value(), nodeBox_->value());
}

void LevelPropertiesWidget::applyFields() {
    if (refreshing_ || !level_ || itemCount() == 0) {
        return;
    }
    const auto index = static_cast<std::size_t>(itemBox_->value());
    auto value = [&](std::size_t field) { return fields_[field].value->value(); };
    nsmb::core::Result<void> result;

    switch (kind()) {
    case Kind::Level:
        result = level_->set_tileset_references({static_cast<std::uint8_t>(value(0)),
                                                 static_cast<std::uint8_t>(value(1)),
                                                 static_cast<std::uint8_t>(value(2)),
                                                 static_cast<std::uint8_t>(value(3))});
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
        sprite.type = static_cast<std::uint16_t>(value(0));
        sprite.x = static_cast<std::uint16_t>(value(1));
        sprite.y = static_cast<std::uint16_t>(value(2));
        for (std::size_t i = 0; i < sprite.data.size(); ++i) {
            sprite.data[i] = static_cast<std::uint8_t>(value(i + 3U));
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
        entrance.number = static_cast<std::uint8_t>(value(4));
        entrance.destination_area = static_cast<std::uint8_t>(value(5));
        entrance.connected_pipe_id = static_cast<std::uint8_t>(value(6));
        entrance.destination_entrance = static_cast<std::uint8_t>(value(7));
        entrance.type = static_cast<std::uint8_t>(value(8));
        entrance.settings = static_cast<std::uint8_t>(value(9));
        entrance.unknown1 = static_cast<std::uint8_t>(value(10));
        entrance.entry_view = static_cast<std::uint8_t>(value(11));
        entrance.unknown2 = static_cast<std::uint8_t>(value(12));
        result = level_->update_entrance(index, entrance);
        break;
    }
    case Kind::Zone: {
        auto zone = level_->course().zones[index];
        zone.x = static_cast<std::uint16_t>(value(0));
        zone.y = static_cast<std::uint16_t>(value(1));
        zone.width = static_cast<std::uint16_t>(value(2));
        zone.height = static_cast<std::uint16_t>(value(3));
        zone.number = static_cast<std::uint8_t>(value(4));
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
            node.x = static_cast<std::uint16_t>(value(1));
            node.y = static_cast<std::uint16_t>(value(2));
            for (std::size_t i = 0; i < node.values.size(); ++i) {
                node.values[i] = static_cast<std::uint16_t>(value(i + 3U));
            }
        }
        result = level_->update_path(progress, index, std::move(path));
        break;
    }
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
        return;
    case Kind::Object:
        result = level_->add_object({0, 0, 0, 0, 1, 1});
        break;
    case Kind::Sprite:
        result = level_->add_sprite({});
        break;
    case Kind::Entrance: {
        nsmb::formats::NsmbLevelEntrance entrance;
        entrance.number = static_cast<std::uint8_t>(firstUnusedNumber(level_->course().entrances, 0, [](const auto& e) { return e.number; }));
        result = level_->add_entrance(entrance);
        break;
    }
    case Kind::Zone: {
        nsmb::formats::NsmbLevelZone zone{0, 0, 256, 192, 0};
        zone.number = static_cast<std::uint8_t>(firstUnusedNumber(level_->course().zones, 0, [](const auto& z) { return z.number; }));
        result = level_->add_zone(zone);
        break;
    }
    case Kind::Path:
    case Kind::ProgressPath: {
        const bool progress = kind() == Kind::ProgressPath;
        const auto& paths = progress ? level_->course().progress_paths : level_->course().paths;
        nsmb::formats::NsmbLevelPath path;
        path.id = static_cast<std::uint16_t>(firstUnusedNumber(paths, progress ? 90 : 0, [](const auto& p) { return p.id; }));
        path.nodes.push_back({});
        result = level_->add_path(progress, std::move(path));
        break;
    }
    }
    if (!result) {
        emit errorOccurred(QString::fromStdString(result.error().message));
        return;
    }
    itemBox_->setMaximum(static_cast<int>(itemCount() - 1U));
    itemBox_->setValue(static_cast<int>(result.value()));
    emit levelChanged();
    refresh();
}

void LevelPropertiesWidget::deleteEntity() {
    if (!level_ || itemCount() == 0) {
        return;
    }
    const auto index = static_cast<std::size_t>(itemBox_->value());
    nsmb::core::Result<void> result;
    switch (kind()) {
    case Kind::Level:
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
    node.x = static_cast<std::uint16_t>(node.x + 16U);
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
    const auto box_index = kindBox_->findData(static_cast<int>(selected_kind));
    if (box_index >= 0) {
        kindBox_->setCurrentIndex(box_index);
    }
    itemBox_->setValue(std::max(0, item_index));
    nodeBox_->setValue(std::max(0, node_index));
    refresh();
}
