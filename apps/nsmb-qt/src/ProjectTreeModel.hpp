#pragma once

#include <QAbstractItemModel>
#include <QString>

#include <cstdint>
#include <memory>
#include <vector>

#include "nsmb/project/project.hpp"

class ProjectTreeModel final : public QAbstractItemModel {
public:
    enum Role {
        PathRole = Qt::UserRole + 1,
        CoursePathRole = PathRole,
        BgdatPathRole,
        SizeRole,
        SourceTokenRole,
        WorldRole,
        AreaNameRole,
        AvailableRole,
        NodeKindRole,
    };

    enum NodeKind {
        RootNode,
        WorldNode,
        LevelNode,
        AreaNode,
    };

    explicit ProjectTreeModel(QObject* parent = nullptr);

    void setProject(const nsmb::project::Project& project);
    [[nodiscard]] const nsmb::project::Project& project() const noexcept;
    [[nodiscard]] int levelCount() const noexcept;
    [[nodiscard]] int availableLevelCount() const noexcept;
    QModelIndex index(int row, int column, const QModelIndex& parent = {}) const override;
    QModelIndex parent(const QModelIndex& child) const override;
    int rowCount(const QModelIndex& parent = {}) const override;
    int columnCount(const QModelIndex& parent = {}) const override;
    QVariant data(const QModelIndex& index, int role) const override;
    QVariant headerData(int section, Qt::Orientation orientation, int role) const override;
    Qt::ItemFlags flags(const QModelIndex& index) const override;

private:
    struct Node {
        NodeKind kind{RootNode};
        QString label;
        QString world;
        QString area_name;
        QString source_token;
        QString course_path;
        QString bgdat_path;
        std::uintmax_t size{};
        bool available{};
        int row{};
        Node* parent{};
        std::vector<std::unique_ptr<Node>> children;
    };

    [[nodiscard]] Node* nodeFromIndex(const QModelIndex& index) const;
    [[nodiscard]] static std::unique_ptr<Node> makeNode(NodeKind kind, QString label);
    Node* addChild(Node& parent, std::unique_ptr<Node> child);
    void rebuildLevels();

    nsmb::project::Project project_;
    std::unique_ptr<Node> root_;
    int level_count_{};
    int available_level_count_{};
};
