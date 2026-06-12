#pragma once
#include <QAbstractItemModel>
#include "nsmb/project/project.hpp"
class ProjectTreeModel final : public QAbstractItemModel {
public:
    explicit ProjectTreeModel(QObject* parent = nullptr);
    void setProject(const nsmb::project::Project& project);
    QModelIndex index(int row, int column, const QModelIndex& parent = {}) const override;
    QModelIndex parent(const QModelIndex& child) const override;
    int rowCount(const QModelIndex& parent = {}) const override;
    int columnCount(const QModelIndex& parent = {}) const override;
    QVariant data(const QModelIndex& index, int role) const override;
private:
    nsmb::project::Project project_;
};
