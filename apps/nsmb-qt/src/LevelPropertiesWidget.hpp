#pragma once

#include <QList>
#include <QPointF>
#include <QString>
#include <QWidget>

#include <array>
#include <cstddef>
#include <optional>
#include <unordered_map>
#include <vector>

#include "nsmb/project/editable_level.hpp"
#include "nsmb/project/graphics_resources.hpp"
#include "nsmb/project/sprite_definitions.hpp"

class QComboBox;
class QGraphicsOpacityEffect;
class QGroupBox;
class QLineEdit;
class QTabBar;
class QLabel;
class QPushButton;
class QSpinBox;
class QVariantAnimation;
class QtEditorTests;

class LevelPropertiesWidget final : public QWidget {
    Q_OBJECT

public:
    enum class Kind { Level = 0, Object = 1, Sprite = 2, Entrance = 3, Zone = 4,
                      Path = 5, ProgressPath = 6, View = 7, ObjectBanks = 8 };

    explicit LevelPropertiesWidget(QWidget* parent = nullptr);
    friend class QtEditorTests;
    void setLevel(nsmb::project::EditableLevel* level);
    void setObjectBankOverlays(std::optional<nsmb::project::ObjectBankOverlayTable> overlays);
    void setSpriteDefinitions(const nsmb::project::SpriteDefinitionCatalog& catalog,
                              std::unordered_map<std::uint16_t, std::uint32_t> actor_object_ids);
    void refresh();
    // Shows the in-progress canvas transform without committing it to undo history.
    void previewInteraction(const QPointF& delta, int resize_handle);
    void select(Kind kind, int item_index, int node_index = 0);
    void selectMany(Kind kind, const QList<int>& item_indices);
    // Which entity kind the panel is showing. With no tool modes on the canvas,
    // this is also what a right-click there creates.
    [[nodiscard]] Kind currentKind() const;

signals:
    void levelChanged();
    // The kind tab changed, whether by the user or by a canvas selection.
    void currentKindChanged(int kind);
    void selectionChanged(int kind, int item_index, int node_index);
    void errorOccurred(const QString& message);

private:
    struct FieldRow {
        QWidget* container{};
        QLabel* label{};
        QSpinBox* value{};
        // What the actor says about this field, revealed under the value it
        // belongs to instead of hiding in a tooltip nobody hovers.
        QLabel* note{};
        QGraphicsOpacityEffect* note_fade{};
        QVariantAnimation* note_reveal{};
        QString note_text;
        int note_height{};
        bool mixed{};
        int unset_value{};
    };

    [[nodiscard]] Kind kind() const;
    [[nodiscard]] std::size_t itemCount() const;
    [[nodiscard]] std::vector<std::size_t> selectedIndices() const;
    // `changed_row` is the row the user turned: for a sprite it is the only
    // declared field written back, so the rest of the settings word survives
    // untouched.
    void applyFields(std::size_t changed_row);
    // Writes the raw NSMBe data value the settings box holds to the selection.
    void applySpriteData();
    // Slides a note open under its field, or closes the one that is there.
    void setFieldNote(FieldRow& row, const QString& text);
    void addEntity();
    void deleteEntity();
    void addNode();
    void deleteNode();
    void emitChange(const nsmb::core::Result<void>& result);

    nsmb::project::EditableLevel* level_{};
    QTabBar* kindTabs_{};
    QSpinBox* itemBox_{};
    QLabel* nodeLabel_{};
    QSpinBox* nodeBox_{};
    QPushButton* addButton_{};
    QPushButton* deleteButton_{};
    QPushButton* addNodeButton_{};
    QPushButton* deleteNodeButton_{};
    QGroupBox* settingsGroup_{};
    QLineEdit* settingsBox_{};
    std::array<FieldRow, 37> fields_{};
    bool refreshing_{};
    std::vector<std::size_t> selected_items_;
    std::optional<nsmb::project::ObjectBankOverlayTable> object_bank_overlays_;
    nsmb::project::SpriteDefinitionCatalog sprite_definitions_;
    std::unordered_map<std::uint16_t, std::uint32_t> actor_object_ids_;
    std::vector<const nsmb::project::SpriteFieldDefinition*> visible_sprite_fields_;
};
