#pragma once

#include <QAbstractItemModel>
#include <QString>

#include <cstdint>
#include <vector>

#include "nsmb/project/project.hpp"

class ProjectTreeModel final : public QAbstractItemModel {
public:
    enum Role {
        PathRole = Qt::UserRole + 1,
        SizeRole,
    };

    explicit ProjectTreeModel(QObject* parent = nullptr);

    void setProject(const nsmb::project::Project& project);
    [[nodiscard]] const nsmb::project::Project& project() const noexcept;
    [[nodiscard]] int levelCount() const noexcept;

    QModelIndex index(int row, int column, const QModelIndex& parent = {}) const override;
    QModelIndex parent(const QModelIndex& child) const override;
    int rowCount(const QModelIndex& parent = {}) const override;
    int columnCount(const QModelIndex& parent = {}) const override;
    QVariant data(const QModelIndex& index, int role) const override;
    QVariant headerData(int section, Qt::Orientation orientation, int role) const override;

private:
    struct LevelEntry {
        QString name;
        QString path;
        std::uintmax_t size{};
    };

    void rebuildLevels();

    nsmb::project::Project project_;
    std::vector<LevelEntry> levels_;
};
