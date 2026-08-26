#include "LevelPreviewWidget.hpp"
#include "LevelPropertiesWidget.hpp"
#include "MainWindow.hpp"

#include <QAction>
#include <QDockWidget>
#include <QFile>
#include <QFileSystemWatcher>
#include <QSaveFile>
#include <QSettings>
#include <QSignalSpy>
#include <QSpinBox>
#include <QTabWidget>
#include <QTemporaryDir>
#include <QtTest>

#include <fstream>

namespace {

void put_u32(std::vector<std::uint8_t>& data, std::size_t offset, std::uint32_t value) {
    data[offset] = static_cast<std::uint8_t>(value & 0xFFU);
    data[offset + 1U] = static_cast<std::uint8_t>((value >> 8U) & 0xFFU);
    data[offset + 2U] = static_cast<std::uint8_t>((value >> 16U) & 0xFFU);
    data[offset + 3U] = static_cast<std::uint8_t>((value >> 24U) & 0xFFU);
}

nsmb::project::EditableLevel make_level() {
    std::vector<std::uint8_t> course(nsmb::formats::nsmb_level_block_count * 8U);
    for (std::size_t i = 0; i < nsmb::formats::nsmb_level_block_count; ++i) {
        put_u32(course, i * 8U, static_cast<std::uint32_t>(course.size()));
    }
    return std::move(nsmb::project::EditableLevel::load(course, {}).value());
}

void write_file(const std::filesystem::path& path, const std::vector<std::uint8_t>& bytes) {
    std::filesystem::create_directories(path.parent_path());
    std::ofstream output(path, std::ios::binary);
    output.write(reinterpret_cast<const char*>(bytes.data()), static_cast<std::streamsize>(bytes.size()));
}

nsmb::project::Project make_project(const std::filesystem::path& root) {
    const auto course = root / "extracted-rom/root/course/A01_1.bin";
    const auto bgdat = root / "extracted-rom/root/course/A01_1_bgdat.bin";
    auto empty = make_level();
    write_file(course, empty.serialize_course().value());
    write_file(bgdat, empty.serialize_bgdat().value());
    auto project = nsmb::project::create_empty_project("Workspace Test", root).value();
    nsmb::nds::RomHeader header;
    header.game_code = "A2DE";
    project.attach_extracted_rom("extracted-rom", header);
    project.add_game_file({"extracted-rom/root/course/A01_1.bin", std::filesystem::file_size(course)});
    project.add_game_file({"extracted-rom/root/course/A01_1_bgdat.bin", std::filesystem::file_size(bgdat)});
    project.mark_clean();
    return project;
}

} // namespace

class QtEditorTests final : public QObject {
    Q_OBJECT

private slots:
    void initTestCase() {
        QCoreApplication::setOrganizationName("BigStarEditorTests");
        QCoreApplication::setApplicationName("M4WorkspaceTests");
        QSettings().clear();
    }

    void previewAndDocksAreRecoverable() {
        MainWindow window;
        window.show();
        QTest::qWait(20);
        auto* tabs = window.findChild<QTabWidget*>("DocumentTabs");
        auto* preview = window.findChild<QAction*>("PreviewAction");
        auto* levels = window.findChild<QDockWidget*>("LevelsDock");
        QVERIFY(tabs);
        QVERIFY(preview);
        QVERIFY(levels);
        QCOMPARE(tabs->count(), 1);
        tabs->tabCloseRequested(0);
        QCOMPARE(tabs->count(), 0);
        QVERIFY(!preview->isChecked());
        preview->trigger();
        QCOMPARE(tabs->count(), 1);
        levels->close();
        QVERIFY(levels->isHidden());
        levels->toggleViewAction()->trigger();
        QVERIFY(levels->isVisible());
    }

    void canvasRequiresPaletteAndRightClickToPlace() {
        auto level = make_level();
        LevelPreviewWidget canvas;
        canvas.resize(800, 600);
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);
        canvas.setTool(LevelPreviewWidget::Tool::Object);
        canvas.show();
        QTest::qWait(20);
        QSignalSpy changed(&canvas, &LevelPreviewWidget::levelChanged);
        QTest::mouseClick(&canvas, Qt::LeftButton, {}, QPoint(400, 350));
        QVERIFY(level.objects().empty());
        QTest::mouseClick(&canvas, Qt::RightButton, {}, QPoint(400, 350));
        QVERIFY(level.objects().empty());
        canvas.setPlacementObject(1, 12);
        QTest::mouseClick(&canvas, Qt::RightButton, {}, QPoint(400, 350));
        QCOMPARE(level.objects().size(), 1U);
        QCOMPARE(changed.count(), 1);
        QCOMPARE(level.objects()[0].tileset_slot, 1);
        QCOMPARE(level.objects()[0].object_id, 12);
        QVERIFY(level.can_undo());
        QVERIFY(level.undo());
        canvas.refreshLevelState();
        QVERIFY(level.objects().empty());
        QVERIFY(canvas.selections_.empty());
    }

    void objectResizeHandleCommitsAndEscapeCancels() {
        auto level = make_level();
        QVERIFY(level.add_object({1, 0, 10, 10, 2, 2}));
        LevelPreviewWidget canvas;
        canvas.resize(800, 600);
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);
        canvas.setTool(LevelPreviewWidget::Tool::Object);
        canvas.setSelection(1, 0, -1);
        canvas.show();
        QTest::qWait(20);
        QCOMPARE(canvas.mapRect().top(), 52.0);
        auto handle = canvas.sceneTransform().map(QPointF(12 * 16, 12 * 16)).toPoint();
        QTest::mousePress(&canvas, Qt::LeftButton, {}, handle);
        QTest::mouseMove(&canvas, handle + QPoint(40, 40));
        QTest::mouseRelease(&canvas, Qt::LeftButton, {}, handle + QPoint(40, 40));
        QVERIFY(level.objects()[0].width > 2);
        QVERIFY(level.objects()[0].height > 2);
        QVERIFY(level.undo());
        QCOMPARE(level.objects()[0].width, 2);
        QCOMPARE(level.objects()[0].height, 2);

        canvas.refreshLevelState();
        canvas.setSelection(1, 0, -1);
        handle = canvas.sceneTransform().map(QPointF(12 * 16, 12 * 16)).toPoint();
        QTest::mousePress(&canvas, Qt::LeftButton, {}, handle);
        QTest::mouseMove(&canvas, handle + QPoint(40, 40));
        QTest::keyClick(&canvas, Qt::Key_Escape);
        QTest::mouseRelease(&canvas, Qt::LeftButton, {}, handle + QPoint(40, 40));
        QCOMPARE(level.objects()[0].width, 2);
        QCOMPARE(level.objects()[0].height, 2);
    }

    void toolbarFiltersSelectionKind() {
        auto level = make_level();
        QVERIFY(level.add_object({1, 0, 1, 1, 1, 1}));
        QVERIFY(level.add_sprite({7, 10, 10, {}}));
        LevelPreviewWidget canvas;
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);
        canvas.selections_ = {{1, 0, -1}, {2, 0, -1}};
        canvas.setTool(LevelPreviewWidget::Tool::Object);
        QCOMPARE(canvas.selections_.size(), 1U);
        QCOMPARE(canvas.selections_.front().kind, 1);
        QVERIFY(canvas.hitTest({20, 20}));
        QVERIFY(!canvas.hitTest({160, 160}));
        canvas.setTool(LevelPreviewWidget::Tool::Sprite);
        QVERIFY(!canvas.hitTest({20, 20}));
        QVERIFY(canvas.hitTest({160, 160}));
    }

    void multiPropertiesEditIsOneUndoStep() {
        auto level = make_level();
        QVERIFY(level.add_object({1, 0, 1, 1, 1, 1}));
        QVERIFY(level.add_object({2, 0, 2, 1, 1, 1}));
        LevelPropertiesWidget properties;
        properties.setLevel(&level);
        properties.selectMany(LevelPropertiesWidget::Kind::Object, {0, 1});
        auto* object_id = properties.findChild<QSpinBox*>("PropertyField0");
        QVERIFY(object_id);
        object_id->setValue(99);
        QCOMPARE(level.objects()[0].object_id, 99);
        QCOMPARE(level.objects()[1].object_id, 99);
        QCOMPARE(level.undo_name(), std::string_view("Edit selection"));
        QVERIFY(level.undo());
        QCOMPARE(level.objects()[0].object_id, 1);
        QCOMPARE(level.objects()[1].object_id, 2);
    }

    void uniqueEditorsDetachAndRestore() {
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto project = make_project(directory.path().toStdString());
        const QString course = "extracted-rom/root/course/A01_1.bin";
        const QString bgdat = "extracted-rom/root/course/A01_1_bgdat.bin";
        const auto key = (course + "\n" + bgdat).toStdString();

        MainWindow first;
        first.setProject(project);
        const auto index = first.findLevelIndex(course, bgdat);
        QVERIFY(index.isValid());
        first.updateSelectedLevel(index);
        QCOMPARE(first.tabs_->count(), 1);
        first.openLevelEditor(index);
        first.openLevelEditor(index);
        QCOMPARE(first.documents_.size(), 1U);
        QCOMPARE(first.tabs_->count(), 2);
        auto& document = first.documents_.at(key);
        document.view->setZoom(2.0);
        document.view->setPan({12, 34});
        first.paletteDock_->hide();
        first.detachEditor(key);
        QVERIFY(document.window);
        QVERIFY(document.view->isVisible());
        QCOMPARE(first.tabs_->count(), 1);
        first.saveWorkspaceState();

        MainWindow restored;
        restored.setProject(project);
        QCOMPARE(restored.documents_.size(), 1U);
        const auto& restored_document = restored.documents_.at(key);
        QVERIFY(restored_document.window);
        QCOMPARE(restored_document.view->zoom(), 2.0);
        QCOMPARE(restored_document.view->pan(), QPointF(12, 34));
        QVERIFY(!restored.paletteDock_->toggleViewAction()->isChecked());

        restored.reattachEditor(key);
        QVERIFY(!restored.documents_.at(key).window);
        QCOMPARE(restored.tabs_->count(), 2);
    }

    void fileWatcherSeesDirectAndAtomicReplacement() {
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto path = directory.filePath("graphics.bin");
        QFile file(path);
        QVERIFY(file.open(QIODevice::WriteOnly));
        QCOMPARE(file.write("one"), 3);
        file.close();

        QFileSystemWatcher watcher;
        if (!watcher.addPath(path)) QSKIP("QFileSystemWatcher is unavailable in this test environment");
        QSignalSpy changed(&watcher, &QFileSystemWatcher::fileChanged);
        QVERIFY(file.open(QIODevice::WriteOnly | QIODevice::Truncate));
        QCOMPARE(file.write("two"), 3);
        file.close();
        QTRY_VERIFY_WITH_TIMEOUT(changed.count() > 0, 2000);

        changed.clear();
        if (!watcher.files().contains(path)) QVERIFY(watcher.addPath(path));
        QSaveFile replacement(path);
        QVERIFY(replacement.open(QIODevice::WriteOnly));
        QCOMPARE(replacement.write("three"), 5);
        QVERIFY(replacement.commit());
        QTRY_VERIFY_WITH_TIMEOUT(changed.count() > 0, 2000);
    }
};

QTEST_MAIN(QtEditorTests)
#include "qt_tests.moc"
