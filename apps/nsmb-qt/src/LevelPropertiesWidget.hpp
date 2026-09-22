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

class QCheckBox;
class QComboBox;
class QGridLayout;
class QGroupBox;
class QLineEdit;
class QTabBar;
class QLabel;
class QPushButton;
class QSpinBox;
class QToolButton;
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
    // Which editor a row wears. A field that names its values is picked from a
    // list, one that is on or off is a checkbox, and everything else is typed.
    enum class FieldEditor { Number, Choice, Boolean };
    // Whether a row shares its line with the field beside it. Only true peers
    // pair up -- X with Y, Width with Height -- and only when their labels are
    // close enough in length that the two editors line up.
    enum class FieldSpan { Half, Full };

    struct FieldRow {
        QWidget* container{};
        QLabel* label{};
        QSpinBox* value{};
        QComboBox* choice{};
        QCheckBox* toggle{};
        // What the actor says about this field, kept on the row it belongs to
        // rather than in a list somewhere else that a renamed label can outrun.
        QToolButton* info{};
        QString note_text;
        FieldEditor editor{FieldEditor::Number};
        FieldSpan span{FieldSpan::Full};
        bool mixed{};
        int unset_value{};
        // What the row stands for when no editor state can express it: a raw
        // value the actor does not name, or checkbox bits that are neither
        // zero nor the value that turns the field on.
        int held_value{};
        bool representable{true};
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
    // Hands the row its note, or takes away the one it had.
    void setFieldNote(FieldRow& row, const QString& text);
    // Dresses a row in the editor its field asks for. `named` is null for a
    // plain number, which is every field outside a sprite's declared ones.
    void setRowEditor(std::size_t row, FieldEditor editor, int minimum, int maximum,
                      const nsmb::project::SpriteFieldDefinition* named);
    // Reports a value to a row without the row reporting it back: every caller
    // of this is saying what the level holds, not editing it.
    void setRowValue(std::size_t row, std::optional<int> value);
    // Nothing, when the row stands for more than one value across a selection.
    [[nodiscard]] std::optional<int> rowValue(std::size_t row) const;
    [[nodiscard]] int rowValueOr(std::size_t row, int fallback) const;
    [[nodiscard]] QString formatFieldValue(int value) const;
    // Puts the visible rows in the grid, and only when they have moved.
    void placeFields();
    void showFieldNote(const FieldRow& row);
    void showActorInfo();
    void rebuildSpritePicker();
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
    QCheckBox* hexBox_{};
    QWidget* spriteHeader_{};
    QLabel* spriteName_{};
    QLabel* spriteCategory_{};
    QComboBox* spritePicker_{};
    QToolButton* spriteInfo_{};
    QGridLayout* fieldsLayout_{};
    std::array<FieldRow, 37> fields_{};
    // What the grid was last given, so a refresh that changes nothing about the
    // shape of the panel leaves the panel's geometry -- and the focus in it --
    // exactly where the user put it.
    std::vector<std::pair<std::size_t, FieldSpan>> field_layout_;
    bool refreshing_{};
    bool hex_fields_{};
    std::vector<std::size_t> selected_items_;
    std::optional<nsmb::project::ObjectBankOverlayTable> object_bank_overlays_;
    nsmb::project::SpriteDefinitionCatalog sprite_definitions_;
    std::unordered_map<std::uint16_t, std::uint32_t> actor_object_ids_;
    std::vector<const nsmb::project::SpriteFieldDefinition*> visible_sprite_fields_;
    // The actor the selected sprite resolved to, kept so the info button has
    // something to describe. Points into `sprite_definitions_`.
    const nsmb::project::SpriteDefinition* current_definition_{};
};
