#include "ProjectTreeModel.hpp"

#include "nsmb/formats/identify.hpp"
#include "nsmb/project/level_catalog.hpp"

#include <algorithm>
#include <cctype>
#include <filesystem>
#include <map>
#include <set>
#include <string>
#include <string_view>
#include <unordered_map>
#include <utility>

namespace {

QString pathText(const nsmb::project::GameFile* file) {
    return file ? QString::fromStdString(file->path.generic_string()) : QString{};
}

std::string lower(std::string value) {
    std::transform(value.begin(), value.end(), value.begin(), [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    return value;
}

struct FileLookup {
    std::unordered_map<std::string, const nsmb::project::GameFile*> by_name;
    std::vector<const nsmb::project::GameFile*> sorted_files;
};

FileLookup buildLookup(const nsmb::project::Project& project) {
    FileLookup lookup;
    lookup.sorted_files.reserve(project.game_files().size());
    for (const auto& file : project.game_files()) {
        lookup.sorted_files.push_back(&file);
    }
    std::sort(lookup.sorted_files.begin(), lookup.sorted_files.end(), [](const auto* lhs, const auto* rhs) {
        return lhs->path.generic_string() < rhs->path.generic_string();
    });
    for (const auto* file : lookup.sorted_files) {
        lookup.by_name.emplace(file->path.filename().string(), file);
    }
    return lookup;
}

const nsmb::project::GameFile* findByName(const FileLookup& lookup, const std::string& name) {
    const auto found = lookup.by_name.find(name);
    return found == lookup.by_name.end() ? nullptr : found->second;
}

const nsmb::project::GameFile* findNarcMember(const FileLookup& lookup, const nsmb::project::LevelCatalogArea& area, std::string_view suffix) {
    if (!area.narc_name) {
        return nullptr;
    }

    const auto wanted_name = area.course_token + std::string(suffix);
    const auto wanted_narc = lower(area.narc_name.value());
    for (const auto* file : lookup.sorted_files) {
        const auto file_name = file->path.filename().string();
        if (file_name != wanted_name) {
            continue;
        }
        const auto path = lower(file->path.generic_string());
        if (path.find(wanted_narc) != std::string::npos) {
            return file;
        }
    }
    return nullptr;
}

const nsmb::project::GameFile* findCourseFile(const FileLookup& lookup, const nsmb::project::LevelCatalogArea& area) {
    if (const auto* narc_member = findNarcMember(lookup, area, ".bin")) {
        return narc_member;
    }
    if (area.narc_name) {
        return nullptr;
    }
    return findByName(lookup, area.course_token + ".bin");
}

const nsmb::project::GameFile* findBgdatFile(const FileLookup& lookup, const nsmb::project::LevelCatalogArea& area) {
    if (const auto* narc_member = findNarcMember(lookup, area, "_bgdat.bin")) {
        return narc_member;
    }
    if (area.narc_name) {
        return nullptr;
    }
    return findByName(lookup, area.course_token + "_bgdat.bin");
}

QString sourceTokenText(const nsmb::project::LevelCatalogArea& area) {
    return QString::fromStdString(area.source_token);
}

QString areaLabel(const nsmb::project::LevelCatalogEntry& entry, const nsmb::project::LevelCatalogArea& area) {
    return QString::fromStdString(entry.areas.size() == 1U ? entry.display_name : area.display_name);
}

} // namespace

ProjectTreeModel::ProjectTreeModel(QObject* parent) : QAbstractItemModel(parent), root_(makeNode(RootNode, {})) {}

void ProjectTreeModel::setProject(const nsmb::project::Project& project) {
    beginResetModel();
    project_ = project;
    rebuildLevels();
    endResetModel();
}

const nsmb::project::Project& ProjectTreeModel::project() const noexcept { return project_; }

int ProjectTreeModel::levelCount() const noexcept { return level_count_; }

int ProjectTreeModel::availableLevelCount() const noexcept { return available_level_count_; }

QModelIndex ProjectTreeModel::index(int row, int column, const QModelIndex& parent_index) const {
    if (row < 0 || column < 0 || column >= columnCount(parent_index)) {
        return {};
    }

    auto* parent_node = nodeFromIndex(parent_index);
    if (!parent_node || row >= static_cast<int>(parent_node->children.size())) {
        return {};
    }

    return createIndex(row, column, parent_node->children[static_cast<std::size_t>(row)].get());
}

QModelIndex ProjectTreeModel::parent(const QModelIndex& child) const {
    auto* node = nodeFromIndex(child);
    if (!node || !node->parent || node->parent == root_.get()) {
        return {};
    }
    return createIndex(node->parent->row, 0, node->parent);
}

int ProjectTreeModel::rowCount(const QModelIndex& parent_index) const {
    const auto* node = nodeFromIndex(parent_index);
    return node ? static_cast<int>(node->children.size()) : 0;
}

int ProjectTreeModel::columnCount(const QModelIndex&) const { return 3; }

QVariant ProjectTreeModel::data(const QModelIndex& idx, int role) const {
    const auto* node = nodeFromIndex(idx);
    if (!node || node == root_.get()) {
        return {};
    }

    if (role == Qt::DisplayRole) {
        if (idx.column() == 0) {
            return node->label;
        }
        if (node->kind == LevelNode && idx.column() == 1) {
            return QString("%1 areas").arg(node->children.size());
        }
        if (idx.column() == 1) {
            return romRelative(node->course_path);
        }
        if (idx.column() == 2) {
            return romRelative(node->bgdat_path);
        }
    }
    if (role == CoursePathRole || role == PathRole) {
        return node->course_path;
    }
    if (role == BgdatPathRole) {
        return node->bgdat_path;
    }
    if (role == CourseRomPathRole) {
        return romRelative(node->course_path);
    }
    if (role == BgdatRomPathRole) {
        return romRelative(node->bgdat_path);
    }
    if (role == SizeRole) {
        return QVariant::fromValue<qulonglong>(static_cast<qulonglong>(node->size));
    }
    if (role == SourceTokenRole) {
        return node->source_token;
    }
    if (role == WorldRole) {
        return node->world;
    }
    if (role == AreaNameRole) {
        return node->area_name;
    }
    if (role == AvailableRole) {
        return node->available;
    }
    if (role == NodeKindRole) {
        return static_cast<int>(node->kind);
    }
    if (role == Qt::ToolTipRole) {
        if (node->kind != AreaNode) {
            return node->label;
        }
        QString text = node->label;
        if (!node->source_token.isEmpty()) {
            text += "\nToken: " + node->source_token;
        }
        text += "\nCourse: " + (node->course_path.isEmpty() ? QString("missing") : romRelative(node->course_path));
        text += "\nBGDat: " + (node->bgdat_path.isEmpty() ? QString("missing") : romRelative(node->bgdat_path));
        return text;
    }
    return {};
}

QString ProjectTreeModel::romRelative(const QString& projectRelative) const {
    if (projectRelative.isEmpty() || !project_.rom()) return projectRelative;
    const auto root = QString::fromStdString(
        (project_.rom()->extracted_path / "root").lexically_normal().generic_string()) + "/";
    return projectRelative.startsWith(root) ? projectRelative.mid(root.size()) : projectRelative;
}

QVariant ProjectTreeModel::headerData(int section, Qt::Orientation orientation, int role) const {
    if (orientation != Qt::Horizontal || role != Qt::DisplayRole) {
        return {};
    }
    if (section == 0) {
        return "Level";
    }
    if (section == 1) {
        return "Course";
    }
    if (section == 2) {
        return "BGDat";
    }
    return {};
}

Qt::ItemFlags ProjectTreeModel::flags(const QModelIndex& index) const {
    const auto* node = nodeFromIndex(index);
    if (!node || node == root_.get()) {
        return Qt::NoItemFlags;
    }

    Qt::ItemFlags flags = Qt::ItemIsEnabled;
    if (node->kind == AreaNode) {
        flags |= Qt::ItemIsSelectable;
    }
    return flags;
}

ProjectTreeModel::Node* ProjectTreeModel::nodeFromIndex(const QModelIndex& index) const {
    if (!index.isValid()) {
        return root_.get();
    }
    return static_cast<Node*>(index.internalPointer());
}

std::unique_ptr<ProjectTreeModel::Node> ProjectTreeModel::makeNode(NodeKind kind, QString label) {
    auto node = std::make_unique<Node>();
    node->kind = kind;
    node->label = std::move(label);
    return node;
}

ProjectTreeModel::Node* ProjectTreeModel::addChild(Node& parent, std::unique_ptr<Node> child) {
    child->parent = &parent;
    child->row = static_cast<int>(parent.children.size());
    auto* raw = child.get();
    parent.children.push_back(std::move(child));
    return raw;
}

void ProjectTreeModel::rebuildLevels() {
    root_ = makeNode(RootNode, {});
    level_count_ = 0;
    available_level_count_ = 0;

    const auto lookup = buildLookup(project_);
    std::map<std::string, Node*> worlds;

    // A catalog world may name a path rather than a single group: "Mario vs.
    // Luigi > Multicard" is one Mario vs. Luigi node holding a Multicard node,
    // not a row with an arrow in its label.
    auto worldNode = [&](const std::string& world) -> Node* {
        if (const auto found = worlds.find(world); found != worlds.end()) {
            return found->second;
        }
        Node* parent = root_.get();
        QString prefix;
        for (const auto& part : QString::fromStdString(world).split('>', Qt::SkipEmptyParts)) {
            const auto name = part.trimmed();
            if (name.isEmpty()) continue;
            prefix = prefix.isEmpty() ? name : prefix + " > " + name;
            const auto key = prefix.toStdString();
            if (const auto found = worlds.find(key); found != worlds.end()) {
                parent = found->second;
                continue;
            }
            auto* node = addChild(*parent, makeNode(WorldNode, name));
            node->world = prefix;
            worlds.emplace(key, node);
            parent = node;
        }
        return parent;
    };

    auto populateAreaNode = [&](Node& node, const nsmb::project::LevelCatalogEntry& entry, const nsmb::project::LevelCatalogArea& area) {
        const auto* course = findCourseFile(lookup, area);
        const auto* bgdat = findBgdatFile(lookup, area);
        node.world = QString::fromStdString(entry.world);
        node.area_name = QString::fromStdString(area.display_name);
        node.source_token = sourceTokenText(area);
        node.course_path = pathText(course);
        node.bgdat_path = pathText(bgdat);
        node.available = course != nullptr && bgdat != nullptr;
        node.size = (course ? course->size : 0U) + (bgdat ? bgdat->size : 0U);
        ++level_count_;
        if (node.available) {
            ++available_level_count_;
        }
    };

    for (const auto& entry : nsmb::project::nsmb_level_catalog()) {
        auto* world = worldNode(entry.world);
        if (entry.areas.size() == 1U) {
            auto area_node = makeNode(AreaNode, areaLabel(entry, entry.areas.front()));
            populateAreaNode(*area_node, entry, entry.areas.front());
            addChild(*world, std::move(area_node));
            continue;
        }

        auto* level = addChild(*world, makeNode(LevelNode, QString::fromStdString(entry.display_name)));
        level->world = QString::fromStdString(entry.world);
        for (const auto& area : entry.areas) {
            auto area_node = makeNode(AreaNode, QString::fromStdString(area.display_name));
            populateAreaNode(*area_node, entry, area);
            addChild(*level, std::move(area_node));
        }
    }

}
