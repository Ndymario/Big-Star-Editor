#pragma once

#include <QWidget>

#include <array>
#include <cstddef>

#include "nsmb/project/editable_level.hpp"

class QComboBox;
class QLabel;
class QPushButton;
class QSpinBox;

class LevelPropertiesWidget final : public QWidget {
    Q_OBJECT

public:
    enum class Kind { Level, Object, Sprite, Entrance, Zone, Path, ProgressPath };

    explicit LevelPropertiesWidget(QWidget* parent = nullptr);
    void setLevel(nsmb::project::EditableLevel* level);
    void refresh();
    void select(Kind kind, int item_index, int node_index = 0);

signals:
    void levelChanged();
    void selectionChanged(int kind, int item_index, int node_index);
    void errorOccurred(const QString& message);

private:
    struct FieldRow {
        QLabel* label{};
        QSpinBox* value{};
    };

    [[nodiscard]] Kind kind() const;
    [[nodiscard]] std::size_t itemCount() const;
    void applyFields();
    void addEntity();
    void deleteEntity();
    void addNode();
    void deleteNode();
    void emitChange(const nsmb::core::Result<void>& result);

    nsmb::project::EditableLevel* level_{};
    QComboBox* kindBox_{};
    QSpinBox* itemBox_{};
    QLabel* nodeLabel_{};
    QSpinBox* nodeBox_{};
    QPushButton* addButton_{};
    QPushButton* deleteButton_{};
    QPushButton* addNodeButton_{};
    QPushButton* deleteNodeButton_{};
    std::array<FieldRow, 13> fields_{};
    bool refreshing_{};
};
