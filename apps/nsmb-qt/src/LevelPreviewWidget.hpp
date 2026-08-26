#pragma once

#include <QString>
#include <QWidget>

#include <optional>

#include "nsmb/formats/level.hpp"
#include "nsmb/project/editable_level.hpp"
#include "nsmb/project/project.hpp"

class LevelPreviewWidget final : public QWidget {
    Q_OBJECT

public:
    explicit LevelPreviewWidget(QWidget* parent = nullptr);

    void showProjectSummary(const nsmb::project::Project& project, int level_count, int available_level_count);
    void showLevel(const QString& level_name,
                   const QString& world,
                   const QString& area_name,
                   const QString& course_path,
                   const QString& bgdat_path,
                   const QString& source_token,
                   nsmb::project::EditableLevel* level);
    void setSelection(int kind, int item_index, int node_index);

protected:
    void paintEvent(QPaintEvent* event) override;

public:
    struct Scene {
        QString title;
        QString world;
        QString area_name;
        QString course_path;
        QString bgdat_path;
        QString source_token;
        nsmb::project::EditableLevel* level{};
        int selected_kind{};
        int selected_item{};
        int selected_node{};
    };

private:
    std::optional<Scene> scene_;
    QString message_;
};
