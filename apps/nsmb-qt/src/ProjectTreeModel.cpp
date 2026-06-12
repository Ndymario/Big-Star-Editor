#include "ProjectTreeModel.hpp"
ProjectTreeModel::ProjectTreeModel(QObject* parent) : QAbstractItemModel(parent) {}
void ProjectTreeModel::setProject(const nsmb::project::Project& project) { beginResetModel(); project_ = project; endResetModel(); }
QModelIndex ProjectTreeModel::index(int row, int column, const QModelIndex& parent) const { if (parent.isValid() || column != 0 || row < 0 || row >= rowCount(parent)) return {}; return createIndex(row, column); }
QModelIndex ProjectTreeModel::parent(const QModelIndex&) const { return {}; }
int ProjectTreeModel::rowCount(const QModelIndex& parent) const { if (parent.isValid()) return 0; return 1 + static_cast<int>(project_.game_files().size()); }
int ProjectTreeModel::columnCount(const QModelIndex&) const { return 1; }
QVariant ProjectTreeModel::data(const QModelIndex& idx, int role) const { if (!idx.isValid() || role != Qt::DisplayRole) return {}; if (idx.row() == 0) return QString::fromStdString(project_.name()); const auto& f = project_.game_files().at(static_cast<std::size_t>(idx.row() - 1)); return QString::fromStdString(f.path.generic_string()); }
