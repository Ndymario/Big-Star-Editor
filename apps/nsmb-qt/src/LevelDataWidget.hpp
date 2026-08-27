#pragma once

#include <QWidget>

#include "nsmb/project/editable_level.hpp"

class QLabel;
class QTableWidget;
class QTreeWidget;

class LevelDataWidget final : public QWidget {
    Q_OBJECT

public:
    explicit LevelDataWidget(QWidget* parent = nullptr);
    void setLevel(nsmb::project::EditableLevel* level, const nsmb::project::ModuleDataCatalog* catalog);
    void refresh();

signals:
    void levelChanged();
    void errorOccurred(const QString& message);

private:
    void editCell(int row, int column);

    nsmb::project::EditableLevel* level_{};
    const nsmb::project::ModuleDataCatalog* catalog_{};
    QLabel* error_{};
    QTableWidget* keys_{};
    QTreeWidget* registration_{};
    bool refreshing_{};
};
