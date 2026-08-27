#include "LevelPreviewWidget.hpp"
#include "FilesystemDock.hpp"
#include "GraphicsEditorDialog.hpp"
#include "UiResourceEditorDialog.hpp"
#include "TilesetEditorDialog.hpp"
#include "LevelPropertiesWidget.hpp"
#include "MainWindow.hpp"
#include "PatcherPanel.hpp"
#include "nsmb/core/file_io.hpp"
#include "nsmb/project/level_exchange.hpp"
#include "nsmb/project/manifest.hpp"

#include <QAction>
#include <QApplication>
#include <QClipboard>
#include <QDockWidget>
#include <QFile>
#include <QFileSystemWatcher>
#include <QHeaderView>
#include <QLabel>
#include <QLocale>
#include <QMenu>
#include <QSaveFile>
#include <QScrollArea>
#include <QSettings>
#include <QSignalSpy>
#include <QSpinBox>
#include <QPlainTextEdit>
#include <QListWidget>
#include <QLineEdit>
#include <QTabWidget>
#include <QTableWidget>
#include <QDir>
#include <QFileIconProvider>
#include <QImage>
#include <QTreeWidget>
#include <QTreeWidgetItemIterator>
#include <QToolBox>
#include <QScopeGuard>
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
    put_u32(course,4,32);course.resize(course.size()+32);
    put_u32(course,2*8,static_cast<std::uint32_t>(course.size()));put_u32(course,2*8+4,20);course.resize(course.size()+20);
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
        QCoreApplication::setApplicationName("BigStarEditorTests");
        QSettings().clear();
    }

    void uiResourceEditorsPreserveUntouchedFiles() {
        const std::vector<std::uint8_t> bnbl{
            'J','N','B','L',0,0,1,0, 1,0,2,0,16,12
        };
        QVERIFY(UiResourceEditorDialog::supports(bnbl, "touch.bnbl"));
        UiResourceEditorDialog layout("touch.bnbl", bnbl, false);
        auto* table = layout.findChild<QTableWidget*>("BnxlTable");
        QVERIFY(table);
        QCOMPARE(table->rowCount(), 1);
        auto layoutOutput = layout.editedBytes();
        QVERIFY(layoutOutput);
        QCOMPARE(layoutOutput.value(), bnbl);

        std::vector<std::uint8_t> enpg(0x10200U);
        QVERIFY(UiResourceEditorDialog::supports(enpg, "loading.enpg"));
        UiResourceEditorDialog image("loading.enpg", enpg, false);
        QVERIFY(image.findChild<QWidget*>("UiResourceCanvas"));
        auto imageOutput = image.editedBytes();
        QVERIFY(imageOutput);
        QCOMPARE(imageOutput.value(), enpg);

        const std::string text = "[Music]\n0=Overworld\n";
        const std::vector<std::uint8_t> metadata(text.begin(), text.end());
        UiResourceEditorDialog notes("NSMBeUD", metadata, true);
        QVERIFY(notes.findChild<QTreeWidget*>("MetadataTree"));
        auto metadataOutput = notes.editedBytes();
        QVERIFY(metadataOutput);
        QCOMPARE(metadataOutput.value(), metadata);
    }

    void previewAndDocksAreRecoverable() {
        MainWindow window;
        window.show();
        QTest::qWait(20);
        auto* tabs = window.findChild<QTabWidget*>("DocumentTabs");
        auto* preview = window.findChild<QAction*>("PreviewAction");
        QVERIFY(window.findChild<QAction*>("RomBannerAction"));
        QVERIFY(window.findChild<QAction*>("RomUserMetadataAction"));
        QVERIFY(window.findChild<QAction*>("FireflowerBuildAction"));
        QVERIFY(window.findChild<QAction*>("NsmbeWrapperBuildAction"));
        auto* levels = window.findChild<QDockWidget*>("LevelsDock");
        QVERIFY(tabs);
        QVERIFY(preview);
        QVERIFY(levels);
        QCOMPARE(tabs->count(), 2);
        auto* hub = window.findChild<QTabWidget*>("ProjectHubTabs");
        QVERIFY(hub);
        QCOMPARE(hub->count(), 5);
        auto* palette = window.findChild<QToolBox*>("VanillaPaletteToolbox");
        QVERIFY(palette);
        tabs->tabCloseRequested(tabs->indexOf(window.hub_));
        QCOMPARE(tabs->count(), 2);
        tabs->tabCloseRequested(tabs->indexOf(window.preview_));
        QCOMPARE(tabs->count(), 1);
        QVERIFY(!preview->isChecked());
        preview->trigger();
        QCOMPARE(tabs->count(), 2);
        levels->close();
        QVERIFY(levels->isHidden());
        levels->toggleViewAction()->trigger();
        QVERIFY(levels->isVisible());

        // The Build Output dock is recoverable the same way. It is tabbed with
        // the Log rather than given its own strip, so closing it must not be a
        // one-way door.
        auto* buildOutput = window.findChild<QDockWidget*>("BuildOutputDock");
        QVERIFY(buildOutput);
        buildOutput->close();
        QVERIFY(buildOutput->isHidden());
        buildOutput->toggleViewAction()->trigger();
        QVERIFY(buildOutput->isVisible());
        auto* filesystem = window.findChild<QDockWidget*>("FilesystemDock");
        QVERIFY(filesystem);
        filesystem->close();
        QVERIFY(filesystem->isHidden());
        filesystem->toggleViewAction()->trigger();
        QVERIFY(filesystem->isVisible());
    }

    void patcherPanelWithoutAProjectRefusesToBuild() {
        // With no project there is nothing to query and nothing to build, so
        // this must not launch anything.
        PatcherPanel panel;
        QVERIFY(panel.selectedVariant().isEmpty());
        QVERIFY(!panel.running());
        QVERIFY(!panel.startBuild(false));
    }

    void patcherPanelReportsAFailedRunInsteadOfHanging() {
        QTemporaryDir project;
        QVERIFY(project.isValid());

        PatcherPanel panel;
        panel.setProjectDirectory(project.path().toStdString());
        // Deliberately not a real tool: the point is that a run which cannot
        // even start still reaches the UI as a reported failure rather than
        // leaving the panel spinning.
        panel.setExecutable("big-star-no-such-tool-anywhere");

        QSignalSpy finished(&panel, &PatcherPanel::buildFinished);
        QVERIFY(panel.startBuild(false));
        QVERIFY(finished.wait(10000));
        QCOMPARE(finished.count(), 1);
        QCOMPARE(finished.at(0).at(0).toBool(), false);

        // And the panel is idle again afterwards, so a second attempt is
        // possible rather than blocked by a run that never ended.
        QTRY_VERIFY(!panel.running());
    }

    void patcherPanelKeepsAVariantTheProjectNoLongerLists() {
        // A stored variant survives a project that cannot confirm it. Silently
        // resetting to "none" would switch the next build to a different
        // language without saying so.
        QTemporaryDir project;
        QVERIFY(project.isValid());

        PatcherPanel panel;
        panel.setSelectedVariant("fr");
        panel.setProjectDirectory(project.path().toStdString());
        panel.setSelectedVariant("fr");
        QCOMPARE(panel.selectedVariant(), QString("fr"));
    }

    void graphicsEditorPaintsRawCharacterPixels() {
        std::vector<std::uint8_t> tile(32);
        QVERIFY(GraphicsEditorDialog::supports(tile, "root/enemy/test_ncg.bin"));
        GraphicsEditorDialog dialog("root/enemy/test_ncg.bin", tile);
        dialog.show();
        QTest::qWait(10);
        auto* palette = dialog.findChild<QListWidget*>("GraphicsPalette");
        auto* canvas = dialog.findChild<QWidget*>("GraphicsCanvas");
        QVERIFY(palette);
        QVERIFY(canvas);
        QCOMPARE(palette->count(), 16);
        palette->setCurrentRow(3);
        QTest::mouseClick(canvas, Qt::LeftButton, {}, QPoint(8, 8));
        const auto edited = dialog.editedBytes();
        QVERIFY(edited);
        QCOMPARE(edited.value().size(), tile.size());
        QCOMPARE(edited.value().front() & 0x0fU, 3U);
    }

    void tilesetEditorExposesAllM10SurfacesAndPreservesResources() {
        nsmb::formats::IndexedTile8Bpp tile;
        const auto graphics = nsmb::formats::encode_8bpp_tiles(std::span{&tile, 1}).value();
        const std::array colors{nsmb::formats::Bgr555Color{0,0,0,true}, nsmb::formats::Bgr555Color{255,255,255,false}};
        const auto palette = nsmb::formats::encode_bgr555_palette(colors).value();
        nsmb::formats::Map16Tile map16_tile;
        const auto map16 = nsmb::formats::encode_map16(std::span{&map16_tile, 1}).value();
        nsmb::formats::TilesetResourceData files{graphics,palette,map16,{0,0,0,0xff},{0,0,0,0},{0,0,0,0}};
        auto parsed = nsmb::formats::parse_tileset(files);
        QVERIFY(parsed);
        std::array tilesets{parsed.value(), parsed.value(), parsed.value()};
        nsmb::formats::TilemapResourceData map_files{graphics,palette,{0,0}};
        auto tilemap = nsmb::formats::parse_tilemap(map_files, 1);
        QVERIFY(tilemap);
        TilesetEditorDialog dialog(std::move(tilesets), tilemap.value(), tilemap.value(), {1,2,3,4});
        dialog.show();
        QTest::qWait(10);
        auto* tabs = dialog.findChild<QTabWidget*>("TilesetTabs");
        auto* id = dialog.findChild<QSpinBox*>("TilesetId");
        QVERIFY(tabs);
        QVERIFY(id);
        QCOMPARE(tabs->count(), 7);
        id->setValue(42);
        QCOMPARE(dialog.references().tileset_id, 42);
        const auto unchanged = dialog.tilesetData(1);
        QVERIFY(unchanged);
        QCOMPARE(unchanged.value().graphics, files.graphics);
        QCOMPARE(unchanged.value().objects, files.objects);
        auto* map16Index = dialog.findChild<QSpinBox*>("Map16TileIndex");
        auto* objectTable = dialog.findChild<QTableWidget*>("TilesetObjectTable");
        auto* behaviorRaw = dialog.findChild<QLineEdit*>("BehaviorRaw");
        QVERIFY(map16Index);
        QVERIFY(objectTable);
        QVERIFY(behaviorRaw);
        map16Index->setValue(1);
        objectTable->item(0, 0)->setText("1");
        behaviorRaw->setText("89abcdef");
        QMetaObject::invokeMethod(behaviorRaw, "editingFinished", Qt::DirectConnection);
        const auto changed = dialog.tilesetData(1);
        QVERIFY(changed);
        const auto reparsed = nsmb::formats::parse_tileset(changed.value());
        QVERIFY(reparsed);
        QCOMPARE(reparsed.value().map16[0].tiles[0].tile, 1);
        QCOMPARE(reparsed.value().objects[0].rows[0][0].tile, 1);
        QCOMPARE(reparsed.value().behaviors[0], 0x89abcdefU);

        MainWindow window;
        QVERIFY(window.findChild<QAction*>("TilesetEditorAction"));
    }

    void patcherPanelRendersDiagnosticsAndLog() {
        PatcherPanel panel;
        panel.show();
        QTest::qWait(10);

        auto* diagnostics = panel.findChild<QTreeWidget*>();
        auto* log = panel.findChild<QPlainTextEdit*>();
        QVERIFY(diagnostics);
        QVERIFY(log);
        QCOMPARE(diagnostics->topLevelItemCount(), 0);

        QString openedFile;
        int openedLine = -1;
        connect(&panel, &PatcherPanel::diagnosticActivated, &panel,
                [&](const QString& file, int line, int) {
                    openedFile = file;
                    openedLine = line;
                });

        // NCPatcher's message carries the location and a source excerpt under
        // the headline. The list shows the claim; the detail belongs in the
        // tooltip rather than in a one-line column.
        const QString message =
            "\"rom.backup\" was not found.\n        at \"ncpatcher.yaml:3:3\", in \"rom\"";
        QMetaObject::invokeMethod(&panel, "onDiagnostic", Qt::DirectConnection,
                                  Q_ARG(int, 2), Q_ARG(QString, "NCP0001"),
                                  Q_ARG(QString, message), Q_ARG(QString, "/p/ncpatcher.yaml"),
                                  Q_ARG(int, 3), Q_ARG(int, 3));

        QCOMPARE(diagnostics->topLevelItemCount(), 1);
        auto* item = diagnostics->topLevelItem(0);
        QCOMPARE(item->text(0), QString("NCP0001"));
        QCOMPARE(item->text(1), QString("\"rom.backup\" was not found."));
        QCOMPARE(item->text(2), QString("/p/ncpatcher.yaml:3:3"));
        QCOMPARE(item->toolTip(1), message);
        QVERIFY(log->toPlainText().contains("NCP0001"));

        diagnostics->itemDoubleClicked(item, 0);
        QCOMPARE(openedFile, QString("/p/ncpatcher.yaml"));
        QCOMPARE(openedLine, 3);

        panel.clearOutput();
        QCOMPARE(diagnostics->topLevelItemCount(), 0);
        QVERIFY(log->toPlainText().isEmpty());
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

    void customPalettePlacementCarriesHash() {
        auto level = make_level();
        LevelPreviewWidget canvas;
        canvas.resize(800, 600);
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);
        canvas.setPlacementCustomSprite(0xf46a44b9U);
        canvas.show();
        QTest::qWait(20);
        QTest::mouseClick(&canvas, Qt::RightButton, {}, QPoint(400, 350));
        QCOMPARE(level.course().sprites.size(), 1U);
        QCOMPARE(level.sprite_hash(0), std::optional<std::uint32_t>(0xf46a44b9U));
        QCOMPARE(level.course().sprites[0].id, 326);
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

    void spriteDefinitionsDriveFieldsAndGrabBounds() {
        auto level = make_level();
        nsmb::formats::NsmbLevelSprite sprite;
        sprite.id = 7; sprite.x = 10; sprite.y = 10; sprite.settings = 0xfffffff0U;
        QVERIFY(level.add_sprite(sprite));
        nsmb::project::SpriteDefinition definition;
        definition.object_id = 42;
        definition.name = "Synthetic actor";
        definition.grab_bounds = {-32, -16, 64, 32};
        definition.render_bounds = definition.grab_bounds;
        definition.fields.push_back({"speed", "Speed", 4, 3, {}});
        nsmb::project::SpriteDefinitionCatalog catalog{{definition}};

        LevelPropertiesWidget properties;
        properties.setSpriteDefinitions(catalog, {{7, 42}});
        properties.setLevel(&level);
        properties.select(LevelPropertiesWidget::Kind::Sprite, 0);
        QCOMPARE(properties.fields_[5].label->text(), QString("Speed"));
        properties.fields_[5].value->setValue(2);
        QCOMPARE(level.course().sprites[0].settings, 0xffffffA0U);

        LevelPreviewWidget canvas;
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setTool(LevelPreviewWidget::Tool::Sprite);
        canvas.actor_object_ids_.emplace(7, 42);
        canvas.setSpriteDefinitions(catalog);
        const auto hit = canvas.hitTest({130, 150});
        QVERIFY(hit);
        QCOMPARE(hit->kind, 2);
    }

    void canvasChromeFollowsThePalette() {
        auto level = make_level();
        LevelPreviewWidget canvas;
        canvas.resize(480, 360);
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);

        auto scheme = [](const QColor& window, const QColor& window_text, const QColor& base, const QColor& text) {
            QPalette palette;
            palette.setColor(QPalette::Window, window);
            palette.setColor(QPalette::WindowText, window_text);
            palette.setColor(QPalette::Base, base);
            palette.setColor(QPalette::Text, text);
            return palette;
        };
        auto render = [&canvas](const QPalette& palette) {
            canvas.setPalette(palette);
            QImage image(canvas.size(), QImage::Format_ARGB32);
            image.fill(Qt::transparent);
            canvas.render(&image);
            return image;
        };

        const auto light = render(scheme(QColor(242, 245, 248), QColor(32, 38, 49), Qt::white, Qt::black));
        const auto dark = render(scheme(QColor(30, 31, 34), QColor(234, 236, 239), QColor(22, 23, 26), QColor(234, 236, 239)));

        // Page margin follows Window, the map surface follows Base: neither may stay light on a dark theme.
        QVERIFY(QColor(light.pixel(4, 4)).lightness() > 200);
        QVERIFY(QColor(dark.pixel(4, 4)).lightness() < 80);
        QVERIFY(QColor(light.pixel(40, 140)).lightness() > 200);
        QVERIFY(QColor(dark.pixel(40, 140)).lightness() < 80);
    }

    void clipboardCloneAndZOrderAreCompoundEdits() {
        auto level = make_level();
        QVERIFY(level.add_object({1, 0, 2, 3, 1, 1}));
        QVERIFY(level.add_object({2, 0, 5, 6, 1, 1}));
        QVERIFY(level.add_custom_sprite(0x12345678U, {326, 7, 8, {}, 9}));
        level.mark_clean();
        LevelPreviewWidget canvas;
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);
        canvas.selections_ = {{1, 0, -1}, {2, 0, -1}};
        canvas.copySelection();
        QVERIFY(QApplication::clipboard()->text().startsWith("BigStar-Entities/1\n"));
        canvas.pasteSelection();
        QCOMPARE(level.objects().size(), 3U);
        QCOMPARE(level.course().sprites.size(), 2U);
        QCOMPARE(level.objects().back().x, 3);
        QCOMPARE(level.sprite_hash(1), std::optional<std::uint32_t>(0x12345678U));
        QCOMPARE(level.undo_name(), std::string_view("Paste selection"));
        QVERIFY(level.undo());
        QCOMPARE(level.objects().size(), 2U);
        QCOMPARE(level.course().sprites.size(), 1U);
        auto moved = level.reorder_entity(1, 0, true);
        QVERIFY(moved);
        QCOMPARE(moved.value(), 1U);
        QCOMPARE(level.objects()[1].object_id, 1);
        QVERIFY(level.undo());
        QCOMPARE(level.objects()[0].object_id, 1);
        QCOMPARE(level.undo_names().front(), std::string("Add custom sprite"));
        QCOMPARE(level.redo_names().front(), std::string("Bring forward"));

        auto replacement = make_level();
        QVERIFY(replacement.add_object({77,0,1,1,1,1}));
        const auto replacement_course = replacement.serialize_course().value();
        const auto replacement_bgdat = replacement.serialize_bgdat().value();
        QVERIFY(level.replace_serialized(replacement_course, replacement_bgdat, nullptr, "Import level"));
        QCOMPARE(level.objects()[0].object_id, 77);
        QVERIFY(level.undo());
        QCOMPARE(level.objects()[0].object_id, 1);

        nsmb::formats::NsmbLevelPath signed_path;
        signed_path.nodes.push_back({-20, -30, {}, {}, 0, 0});
        QVERIFY(level.add_path(false, signed_path));
        canvas.selections_ = {{5, 0, 0}};
        canvas.commitMove({-8, -8});
        QCOMPARE(level.course().paths[0].nodes[0].x, -28);
        QCOMPARE(level.course().paths[0].nodes[0].y, -38);
    }

    void autosaveSnapshotIsWrittenAndClearedWhenClean() {
        QTemporaryDir directory; QVERIFY(directory.isValid()); const auto project=make_project(directory.path().toStdString());
        MainWindow window; window.setProject(project);
        auto session=window.levelSession("extracted-rom/root/course/A01_1.bin","extracted-rom/root/course/A01_1_bgdat.bin");QVERIFY(session);QVERIFY(session.value()->add_object({4,0,1,1,1,1}));
        window.writeRecoverySnapshots(); const auto recovery=std::filesystem::path(directory.path().toStdString())/".big-star-autosave";
        std::size_t count=0;for(const auto& entry:std::filesystem::directory_iterator(recovery))if(entry.path().extension()==".bsrecover"){auto bytes=nsmb::core::read_file(entry.path());QVERIFY(bytes);auto decoded=nsmb::project::decode_level_recovery(bytes.value());QVERIFY(decoded);++count;}QCOMPARE(count,1U);
        session.value()->mark_clean();window.writeRecoverySnapshots();count=0;for(const auto& entry:std::filesystem::directory_iterator(recovery))if(entry.path().extension()==".bsrecover")++count;QCOMPARE(count,0U);
    }

    void recentProjectsListMostRecentFirstAndDropWhatCannotOpen() {
        QSettings().remove("RecentProjects");
        const auto guard = qScopeGuard([] { QSettings().remove("RecentProjects"); });
        QTemporaryDir first_directory; QVERIFY(first_directory.isValid());
        QTemporaryDir second_directory; QVERIFY(second_directory.isValid());
        const auto first_root = std::filesystem::path(first_directory.path().toStdString());
        const auto second_root = std::filesystem::path(second_directory.path().toStdString());
        const auto first_project = make_project(first_root);
        const auto second_project = make_project(second_root);
        for (const auto& root : {first_root, second_root}) {
            QVERIFY(nsmb::project::save_manifest(root == first_root ? first_project : second_project,
                                                 root / nsmb::project::manifest_file_name));
        }

        MainWindow window;
        window.setProject(first_project);
        window.setProject(second_project);
        QCOMPARE(QSettings().value("RecentProjects").toStringList(),
                 QStringList({second_directory.path(), first_directory.path()}));

        // Reopening an older entry moves it back to the top rather than duplicating it.
        window.openRecentProject(first_directory.path());
        QCOMPARE(QString::fromStdString(window.currentProject_.directory().string()), first_directory.path());
        QCOMPARE(QSettings().value("RecentProjects").toStringList(),
                 QStringList({first_directory.path(), second_directory.path()}));

        QCOMPARE(window.recentMenu_->actions().size(), qsizetype{4});
        QVERIFY(window.recentMenu_->actions().at(0)->text().endsWith(QDir::toNativeSeparators(first_directory.path())));

        // A folder that no longer opens is dropped. openRecentProject reports the failure through a
        // modal warning first, so the removal it performs is exercised directly here.
        std::filesystem::remove(first_root / nsmb::project::manifest_file_name);
        QVERIFY(!nsmb::project::load_manifest(first_root / nsmb::project::manifest_file_name));
        window.forgetRecentProject(first_directory.path());
        QCOMPARE(QSettings().value("RecentProjects").toStringList(), QStringList({second_directory.path()}));
        QCOMPARE(window.recentMenu_->actions().size(), qsizetype{3});
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
        QCOMPARE(first.tabs_->count(), 2);
        first.openLevelEditor(index);
        first.openLevelEditor(index);
        QCOMPARE(first.documents_.size(), 1U);
        QCOMPARE(first.tabs_->count(), 3);
        QVERIFY(first.paletteToolbox_->count() > 0);
        QVERIFY(qobject_cast<QScrollArea*>(first.paletteToolbox_->widget(0)));
        auto& document = first.documents_.at(key);
        document.view->setZoom(2.0);
        document.view->setPan({12, 34});
        first.paletteDock_->hide();
        first.detachEditor(key);
        QVERIFY(document.window);
        QVERIFY(document.view->isVisible());
        QCOMPARE(first.tabs_->count(), 2);
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
        QCOMPARE(restored.tabs_->count(), 3);
    }

    void levelSavesGoToTheActiveModuleTree() {
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto root = std::filesystem::path(directory.path().toStdString());
        auto project = make_project(root);
        const auto nitrofs = root / "modules/levels/nitrofs";
        std::filesystem::create_directories(nitrofs / "en");
        std::filesystem::create_directories(root / "build/generated");
        std::ofstream(root / "ncpatcher.yaml") << "version: 2\nvariants:\n  en:\n    defines: TEST\n";
        std::ofstream(root / "build/generated/modules.json")
            << "{\"schema\":\"ncpatcher.modules/1\",\"dir\":\"" << (root / "modules").generic_string()
            << "\",\"modules\":[{\"key\":\"levels\",\"id\":\"Levels\",\"name\":\"Levels\",\"enabled\":true,"
               "\"dir\":\"" << (root / "modules/levels").generic_string()
            << "\",\"file\":\"" << (root / "modules/levels/module.yaml").generic_string()
            << "\",\"nitrofs\":{\"dir\":\"" << nitrofs.generic_string()
            << "\",\"layered\":true,\"base-variant\":\"en\"},\"components\":[]}]}";

        const auto extractedCourse = root / "extracted-rom/root/course/A01_1.bin";
        const auto before = nsmb::core::read_file(extractedCourse).value();
        MainWindow window;
        window.setProject(project);
        auto session = window.levelSession("extracted-rom/root/course/A01_1.bin", "extracted-rom/root/course/A01_1_bgdat.bin");
        QVERIFY(session);
        QVERIFY(session.value()->add_object({7, 0, 1, 2, 1, 1}));
        QVERIFY(window.saveEditedLevels());
        const auto moduleCourse = nitrofs / "en/course/A01_1.bin";
        const auto moduleBgdat = nitrofs / "en/course/A01_1_bgdat.bin";
        QVERIFY(std::filesystem::is_regular_file(moduleCourse));
        QVERIFY(std::filesystem::is_regular_file(moduleBgdat));
        QCOMPARE(nsmb::core::read_file(extractedCourse).value(), before);
        QVERIFY(window.filesystemView_.find("course/A01_1.bin"));
        QCOMPARE(window.filesystemView_.find("course/A01_1.bin")->status, nsmb::project::FilesystemStatus::Pending);
    }

    void filesystemTreeFollowsFileIdOrder() {
        // Real NSMB IDs. NitroFS lists in file-ID order, so 00DUMMY (131) sits above
        // ARCHIVE (lowest member 135) -- the same listing NSMBe's browser produces.
        nsmb::project::FilesystemView view;
        const auto add = [&view](const char* path, std::optional<std::int64_t> id) {
            nsmb::project::FilesystemEntry entry; entry.path = path; entry.id = id; view.entries.push_back(std::move(entry));
        };
        add("ARCHIVE/Dat_2D.narc", 139);
        add("ARCHIVE/ARC0.narc", 135);
        add("ARCHIVE/bomthrow.narc", 136);
        add("map/w2_castle.nsbmd", 1727);
        add("map/w1_castle.nsbmd", 1713);
        add("mgvs_sound_data.sdat", 133);
        add("00DUMMY", 131);
        add("BUILDTIME", 132);
        add("course/A01_1_bgdat.bin", 823);
        add("course/A01_1.bin", 822);
        add("z_new/pending.bin", std::nullopt);  // not built yet, so no ID

        FilesystemDock dock;
        dock.setData({}, {}, std::move(view), {}, {}, {});
        auto* tree = dock.findChild<QTreeWidget*>("FilesystemTree");
        QVERIFY(tree);
        QCOMPARE(tree->topLevelItemCount(), 1);

        const auto* root = tree->topLevelItem(0);
        const auto childNames = [](const QTreeWidgetItem* parent) {
            QStringList names;
            for (int i = 0; i < parent->childCount(); ++i) names << parent->child(i)->text(0);
            return names;
        };
        // Root files come first because they hold the lowest IDs; z_new has none, so it trails.
        QCOMPARE(childNames(root), QStringList({"00DUMMY", "BUILDTIME", "mgvs_sound_data.sdat",
                                                "ARCHIVE", "course", "map", "z_new"}));
        QCOMPARE(childNames(root->child(3)), QStringList({"ARC0.narc", "bomthrow.narc", "Dat_2D.narc"}));
        QCOMPARE(childNames(root->child(4)), QStringList({"A01_1.bin", "A01_1_bgdat.bin"}));
        QCOMPARE(childNames(root->child(5)), QStringList({"w1_castle.nsbmd", "w2_castle.nsbmd"}));
    }

    void filesystemTreeFallsBackToNaturalOrderWithoutIds() {
        // No NCPatcher manifest means no IDs at all; names then order the tree, and a
        // natural compare keeps w10 after w2 and folds case so Dat_ does not lead.
        nsmb::project::FilesystemView view;
        for (const auto* path : {"map/w10_castle.nsbmd", "map/w2_castle.nsbmd", "map/w1_castle.nsbmd",
                                 "ARCHIVE/bomthrow.narc", "ARCHIVE/Dat_2D.narc", "ARCHIVE/ARC0.narc",
                                 "BUILDTIME", "00DUMMY"})
            view.entries.push_back({.path = path});

        FilesystemDock dock;
        dock.setData({}, {}, std::move(view), {}, {}, {});
        auto* tree = dock.findChild<QTreeWidget*>("FilesystemTree");
        QVERIFY(tree);

        const auto* root = tree->topLevelItem(0);
        const auto childNames = [](const QTreeWidgetItem* parent) {
            QStringList names;
            for (int i = 0; i < parent->childCount(); ++i) names << parent->child(i)->text(0);
            return names;
        };
        QCOMPARE(childNames(root), QStringList({"00DUMMY", "ARCHIVE", "BUILDTIME", "map"}));
        QCOMPARE(childNames(root->child(1)), QStringList({"ARC0.narc", "bomthrow.narc", "Dat_2D.narc"}));
        QCOMPARE(childNames(root->child(3)), QStringList({"w1_castle.nsbmd", "w2_castle.nsbmd", "w10_castle.nsbmd"}));
    }

    void filesystemListShowsPathAndStatusWithOptionalColumns() {
        // The list itself is Path + Status; the rest of what is known about a file belongs in
        // the footer, and only becomes a column when the header menu is asked for it.
        QSettings().remove("FilesystemDock/columns");
        nsmb::project::FilesystemView view;
        nsmb::project::FilesystemEntry entry;
        entry.path = "ARCHIVE/Dat_2D.narc";
        entry.status = nsmb::project::FilesystemStatus::Replaced;
        entry.module = "levels"; entry.component = "nitrofs"; entry.variant = "en";
        entry.id = 139; entry.size = 4096; entry.source = "modules/levels/en/ARCHIVE/Dat_2D.narc";
        view.entries.push_back(std::move(entry));

        {
            FilesystemDock dock;
            dock.setData({}, {}, view, {}, {}, {});
            auto* tree = dock.findChild<QTreeWidget*>("FilesystemTree");
            QVERIFY(tree);
            QCOMPARE(tree->columnCount(), 6);
            QVERIFY(!tree->isColumnHidden(0));
            QVERIFY(!tree->isColumnHidden(1));
            for (int column = 2; column < 6; ++column) QVERIFY(tree->isColumnHidden(column));
            // Nothing may resize a column behind the user's back.
            for (int column = 0; column < 6; ++column)
                QCOMPARE(tree->header()->sectionResizeMode(column), QHeaderView::Interactive);
            QVERIFY(!tree->header()->stretchLastSection());

            auto* details = dock.findChild<QLabel*>("FilesystemDetails");
            QVERIFY(details);
            QCOMPARE(details->text(), QString("No file selected."));

            auto* menu = dock.findChild<QMenu*>("FilesystemColumns");
            QVERIFY(menu);
            QCOMPARE(menu->actions().size(), 6);
            QVERIFY(!menu->actions().at(0)->isEnabled());  // Path always stays
            QVERIFY(menu->actions().at(4)->isEnabled());
            QCOMPARE(menu->actions().at(4)->text(), QString("File ID"));
            menu->actions().at(4)->setChecked(true);
            QVERIFY(!tree->isColumnHidden(4));
            QVERIFY(tree->columnWidth(4) > 0);

            // The footer reports everything the hidden columns would have.
            auto* item = tree->topLevelItem(0)->child(0)->child(0);
            QVERIFY(item);
            QCOMPARE(item->data(0, Qt::UserRole).toString(), QString("ARCHIVE/Dat_2D.narc"));
            tree->setCurrentItem(item);
            const auto text = details->text();
            QVERIFY2(text.contains("ARCHIVE/Dat_2D.narc"), qPrintable(text));
            QVERIFY2(text.contains("File ID: 139"), qPrintable(text));
            QVERIFY2(text.contains("Size: " + QLocale().toString(4096) + " bytes"), qPrintable(text));
            QVERIFY2(text.contains("From: levels / nitrofs"), qPrintable(text));
            QVERIFY2(text.contains("Variant: en"), qPrintable(text));

            tree->setCurrentItem(tree->topLevelItem(0));  // the root directory is not a file
            QCOMPARE(details->text(), QString("No file selected."));
        }

        // Column choices and widths survive into the next dock.
        FilesystemDock restored;
        restored.setData({}, {}, view, {}, {}, {});
        auto* tree = restored.findChild<QTreeWidget*>("FilesystemTree");
        QVERIFY(tree);
        QVERIFY(!tree->isColumnHidden(4));
        QVERIFY(tree->isColumnHidden(5));
        QSettings().remove("FilesystemDock/columns");
    }

    void filesystemTreeIconsFollowTheDetectedFormat() {
        // The offscreen platform has no icon theme, so install a throwaway one holding just
        // the names the dock asks for. That keeps the assertions about the format -> icon
        // mapping rather than about whatever theme happens to be on the build machine.
        QTemporaryDir themeRoot;
        QVERIFY(themeRoot.isValid());
        const QStringList iconNames{"package-x-generic", "image-x-generic", "color-picker",
                                    "x-office-drawing", "applications-graphics", "video-x-generic",
                                    "audio-x-generic", "applications-games", "text-x-generic",
                                    "preferences-desktop-effects", "input-touchpad", "snap-bounding-box"};
        const auto sizeDir = themeRoot.filePath("bse-test/16x16");
        QVERIFY(QDir().mkpath(sizeDir));
        {
            QFile index(themeRoot.filePath("bse-test/index.theme"));
            QVERIFY(index.open(QIODevice::WriteOnly));
            index.write("[Icon Theme]\nName=bse-test\nDirectories=16x16\n\n[16x16]\nSize=16\nType=Fixed\n");
        }
        QImage swatch(16, 16, QImage::Format_ARGB32);
        swatch.fill(Qt::red);
        for (const auto& name : iconNames) QVERIFY(swatch.save(sizeDir + "/" + name + ".png"));

        const auto previousPaths = QIcon::themeSearchPaths();
        const auto previousTheme = QIcon::themeName();
        QIcon::setThemeSearchPaths({themeRoot.path()});
        QIcon::setThemeName("bse-test");
        const auto restore = qScopeGuard([&] { QIcon::setThemeName(previousTheme); QIcon::setThemeSearchPaths(previousPaths); });
        QVERIFY2(!QIcon::fromTheme("image-x-generic").isNull(), "test theme did not load");

        QTemporaryDir files;
        QVERIFY(files.isValid());
        nsmb::project::FilesystemView view;
        const auto add = [&](const char* romPath, const QByteArray& header) {
            const auto onDisk = files.filePath(QString::fromLatin1(romPath).replace('/', '_'));
            QFile file(onDisk);
            QVERIFY(file.open(QIODevice::WriteOnly));
            QCOMPARE(file.write(header), header.size());
            file.close();
            nsmb::project::FilesystemEntry entry;
            entry.path = romPath;
            entry.extracted = onDisk.toStdString();
            view.entries.push_back(std::move(entry));
        };
        const QByteArray pad(24, '\0');
        add("ARCHIVE/a.narc", QByteArray("NARC") + pad);
        add("BG_ncg/a_ncg.bin", QByteArray("RGCN") + pad);
        add("BG_ncl/a_ncl.bin", QByteArray("RLCN") + pad);
        add("BG_nsc/a_nsc.bin", QByteArray("RCSN") + pad);
        add("map/a.nsbmd", QByteArray("BMD0") + pad);
        add("demo/a.nsbca", QByteArray("BCA0") + pad);
        add("a.sdat", QByteArray("SDAT") + pad);
        add("script/course.bmg", QByteArray("MESG") + pad);          // magic must beat the "course" name hint
        add("particle/a.spa", QByteArray(" APS22_1") + pad);
        add("uiStudio/a.bnbl", QByteArray("JNBL") + pad);            // touch hit boxes
        add("uiStudio/a.bncl", QByteArray("JNCL") + pad);            // OAM positions, not a cell layout
        add("uiStudio/a.bncd", QByteArray("JNCD") + pad);            // OAM plus graphics
        add("uiStudio/course_select.bnll", QByteArray("JNLL") + pad);// undocumented, must not read as a level
        add("demo/a.enpg", QByteArray("\x10\x00\x02\x01\x3f", 5) + pad);  // LZ, identified by name
        add("enemy/a.nsbca", QByteArray("LZ77\x10\xd4\x09\x00\x00" "BCA0", 13) + pad);  // magic behind an LZ77 header
        add("player/cap_LZ.bin", QByteArray("\x10\xf0\x25\x00\x00" "BCA0", 9) + pad);   // magic behind a bare LZ header
        add("course/A01_1.bin", QByteArray(16, '\x00'));            // no magic, so the name decides
        add("BG_unt/a.bin", QByteArray(16, '\x00'));                // tileset data: nothing identifies it

        FilesystemDock dock;
        dock.setData({}, {}, std::move(view), {}, {}, {});
        auto* tree = dock.findChild<QTreeWidget*>("FilesystemTree");
        QVERIFY(tree);

        QMap<QString, QString> iconByPath;
        for (QTreeWidgetItemIterator it(tree); *it; ++it) {
            const auto path = (*it)->data(0, Qt::UserRole).toString();
            if (!path.isEmpty()) iconByPath[path] = (*it)->icon(0).name();
        }
        QCOMPARE(iconByPath["ARCHIVE/a.narc"], QString("package-x-generic"));
        QCOMPARE(iconByPath["BG_ncg/a_ncg.bin"], QString("image-x-generic"));
        QCOMPARE(iconByPath["BG_ncl/a_ncl.bin"], QString("color-picker"));
        QCOMPARE(iconByPath["BG_nsc/a_nsc.bin"], QString("x-office-drawing"));
        QCOMPARE(iconByPath["map/a.nsbmd"], QString("applications-graphics"));
        QCOMPARE(iconByPath["demo/a.nsbca"], QString("video-x-generic"));
        QCOMPARE(iconByPath["a.sdat"], QString("audio-x-generic"));
        QCOMPARE(iconByPath["script/course.bmg"], QString("text-x-generic"));
        QCOMPARE(iconByPath["particle/a.spa"], QString("preferences-desktop-effects"));
        QCOMPARE(iconByPath["uiStudio/a.bnbl"], QString("input-touchpad"));
        QCOMPARE(iconByPath["uiStudio/a.bncl"], QString("snap-bounding-box"));
        QCOMPARE(iconByPath["uiStudio/a.bncd"], QString("image-x-generic"));
        QCOMPARE(iconByPath["demo/a.enpg"], QString("image-x-generic"));
        QCOMPARE(iconByPath["enemy/a.nsbca"], QString("video-x-generic"));
        QCOMPARE(iconByPath["player/cap_LZ.bin"], QString("video-x-generic"));
        QCOMPARE(iconByPath["course/A01_1.bin"], QString("applications-games"));
        // Unidentified entries fall back to the system's own generic file icon rather than
        // going bare -- BNLL included, so its name can never be read as a level.
        const auto generic = QFileIconProvider().icon(QFileIconProvider::File).name();
        QCOMPARE(iconByPath["uiStudio/course_select.bnll"], generic);
        QCOMPARE(iconByPath["BG_unt/a.bin"], generic);

        // Directories, including the root node, carry the system folder icon.
        for (int i = 0; i < tree->topLevelItem(0)->childCount(); ++i) {
            const auto* child = tree->topLevelItem(0)->child(i);
            if (child->childCount() > 0) QVERIFY(!child->icon(0).isNull());
        }
        QVERIFY(!tree->topLevelItem(0)->icon(0).isNull());
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
