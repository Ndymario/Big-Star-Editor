#pragma once

#include <QImage>
#include <QString>
#include <QWidget>

#include <optional>
#include <vector>

#include "nsmb/formats/level.hpp"
#include "nsmb/project/project.hpp"

class LevelPreviewWidget final : public QWidget {
public:
    explicit LevelPreviewWidget(QWidget* parent = nullptr);

    void showProjectSummary(const nsmb::project::Project& project, int level_count, int available_level_count);
    void showLevel(const nsmb::project::Project& project,
                   const QString& level_name,
                   const QString& world,
                   const QString& area_name,
                   const QString& course_path,
                   const QString& bgdat_path,
                   const QString& source_token);

protected:
    void paintEvent(QPaintEvent* event) override;

public:
    struct GraphicsSample {
        QString character_path;
        QString palette_path;
        QImage image;
    };

    struct Scene {
        QString title;
        QString world;
        QString area_name;
        QString course_path;
        QString bgdat_path;
        QString source_token;
        nsmb::formats::NsmbLevelCourse course;
        std::vector<nsmb::formats::NsmbLevelObject> objects;
        std::optional<GraphicsSample> graphics;
    };

private:
    std::optional<Scene> scene_;
    QString message_;
};
