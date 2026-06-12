#include "ProjectTreeModel.hpp"

#include "nsmb/formats/identify.hpp"

#include <algorithm>
#include <filesystem>

namespace {

QString levelNameFromPath(const std::filesystem::path& path) {
    const auto stem = QString::fromStdString(path.stem().string());
    return stem.isEmpty() ? QString::fromStdString(path.filename().string()) : stem;
}

bool isLevelCandidate(const nsmb::project::GameFile& file) {
    const auto path = file.path.generic_string();
    return nsmb::formats::identify_format({}, path) == nsmb::formats::FormatKind::LevelCandidate;
}

} // namespace

ProjectTreeModel::ProjectTreeModel(QObject* parent) : QAbstractItemModel(parent) {}

void ProjectTreeModel::setProject(const nsmb::project::Project& project) {
    beginResetModel();
    project_ = project;
    rebuildLevels();
    endResetModel();
}

const nsmb::project::Project& ProjectTreeModel::project() const noexcept { return project_; }

int ProjectTreeModel::levelCount() const noexcept { return static_cast<int>(levels_.size()); }

QModelIndex ProjectTreeModel::index(int row, int column, const QModelIndex& parent) const {
    if (parent.isValid() || row < 0 || column < 0 || column >= columnCount(parent) || row >= rowCount(parent)) {
        return {};
    }
    return createIndex(row, column);
}

QModelIndex ProjectTreeModel::parent(const QModelIndex&) const { return {}; }

int ProjectTreeModel::rowCount(const QModelIndex& parent) const {
    if (parent.isValid()) {
        return 0;
    }
    return static_cast<int>(levels_.size());
}

int ProjectTreeModel::columnCount(const QModelIndex&) const { return 2; }

QVariant ProjectTreeModel::data(const QModelIndex& idx, int role) const {
    if (!idx.isValid() || idx.row() < 0 || idx.row() >= rowCount({})) {
        return {};
    }

    const auto& level = levels_.at(static_cast<std::size_t>(idx.row()));
    if (role == Qt::DisplayRole) {
        if (idx.column() == 0) {
            return level.name;
        }
        if (idx.column() == 1) {
            return level.path;
        }
    }
    if (role == PathRole) {
        return level.path;
    }
    if (role == SizeRole) {
        return QVariant::fromValue<qulonglong>(static_cast<qulonglong>(level.size));
    }
    if (role == Qt::ToolTipRole) {
        return QString("%1\n%2 bytes").arg(level.path).arg(static_cast<qulonglong>(level.size));
    }
    return {};
}

QVariant ProjectTreeModel::headerData(int section, Qt::Orientation orientation, int role) const {
    if (orientation != Qt::Horizontal || role != Qt::DisplayRole) {
        return {};
    }
    if (section == 0) {
        return "Level";
    }
    if (section == 1) {
        return "Path";
    }
    return {};
}

void ProjectTreeModel::rebuildLevels() {
    levels_.clear();
    for (const auto& file : project_.game_files()) {
        if (!isLevelCandidate(file)) {
            continue;
        }
        levels_.push_back(LevelEntry{levelNameFromPath(file.path), QString::fromStdString(file.path.generic_string()), file.size});
    }

    std::sort(levels_.begin(), levels_.end(), [](const auto& lhs, const auto& rhs) {
        return QString::localeAwareCompare(lhs.path, rhs.path) < 0;
    });
}
