#include "LevelPreviewWidget.hpp"
#include "FilesystemDock.hpp"
#include "FileEditors.hpp"
#include "GraphicsEditorDialog.hpp"

#include "nsmb/formats/g2d.hpp"
#include "MessageEditorDialog.hpp"
#include "UiResourceEditorDialog.hpp"
#include "TilesetEditorDialog.hpp"
#include "LevelPropertiesWidget.hpp"
#include "MainWindow.hpp"

#include "synthetic_rom.hpp"
#include "ProjectTreeModel.hpp"
#include "PatcherPanel.hpp"
#include "nsmb/core/file_io.hpp"
#include "nsmb/formats/graphics.hpp"
#include "nsmb/formats/message.hpp"
#include "nsmb/project/level_exchange.hpp"
#include "nsmb/project/manifest.hpp"

#include <QAction>
#include <QApplication>
#include <QCheckBox>
#include <QComboBox>
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
#include <QToolButton>
#include <QPlainTextEdit>
#include <QPushButton>
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
#include <sstream>

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

// A project the editor can actually open: one built ROM holding one level, read
// where it lies. There is no unpacked copy of it to write any more.
nsmb::project::Project make_project(const std::filesystem::path& root) {
    auto empty = make_level();
    synthetic::write_bytes(root / "big-star-build.nds",
                           synthetic::make_rom({{"course/A01_1.bin", empty.serialize_course().value()},
                                                {"course/A01_1_bgdat.bin", empty.serialize_bgdat().value()}}));
    auto project = nsmb::project::create_empty_project("Workspace Test", root).value();
    nsmb::nds::RomHeader header;
    header.game_code = "A2DE";
    project.attach_rom("big-star-build.nds", header);
    auto indexed = nsmb::project::index_rom_files(project);
    Q_ASSERT(indexed.has_value());
    project.mark_clean();
    return project;
}

// The dock reads its bytes through the window's reader now, so a dock test
// needs a ROM rather than a directory of loose files.
nsmb::project::EffectiveFilesystem romFixture(const QString& directory,
                                              nsmb::project::FilesystemView& view,
                                              const std::vector<std::pair<std::string, std::vector<std::uint8_t>>>& files) {
    std::vector<synthetic::RomFile> contents;
    for (const auto& [path, body] : files) {
        contents.push_back({path, body});
        nsmb::project::FilesystemEntry entry;
        entry.path = path;
        view.entries.push_back(std::move(entry));
    }
    const auto rom = std::filesystem::path(directory.toStdString()) / "fixture.nds";
    synthetic::write_bytes(rom, synthetic::make_rom(contents));
    nsmb::project::EffectiveFilesystem effective;
    auto opened = nsmb::project::RomReader::open(rom);
    if (opened) effective.set_rom(std::move(opened.value()));
    return effective;
}

// A BMG the message editor can be pointed at: two messages, MID1 IDs, INF1 attributes.
std::vector<std::uint8_t> sampleBmg() {
    nsmb::formats::Bmg bmg;
    bmg.attribute_size = 4;
    bmg.has_mid1 = true;
    bmg.messages.push_back({{{true, {}, 255, {0, 0, 1, 0}}, {false, " Luigi has appeared!", 0, {}}}, {1, 0, 0, 0}, 40});
    bmg.messages.push_back({{{false, "Welcome!", 0, {}}}, {2, 0, 0, 0}, 41});
    auto written = nsmb::formats::write_bmg(bmg);
    return written ? written.value() : std::vector<std::uint8_t>{};
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
        QVERIFY(image.findChild<QWidget*>("GraphicsCanvas"));
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

    void oneGraphicsEditorOpensBothFormatFamilies() {
        // There used to be two editors, and a file landed in whichever one
        // claimed it. There is one now, so the same dialog has to show a
        // uiStudio layout and an NNS G2D sheet alike.
        const std::vector<std::uint8_t> bnbl{'J','N','B','L',0,0,1,0, 1,0,2,0,16,12};
        QVERIFY(GraphicsEditorDialog::supports(bnbl, "uiStudio/touch.bnbl"));
        GraphicsEditorDialog layout("uiStudio/touch.bnbl", bnbl);
        QVERIFY(layout.findChild<QTableWidget*>("BnxlTable"));
        QVERIFY(layout.editedBytes());
        QCOMPARE(layout.editedBytes().value(), bnbl);

        const std::string text = "[Music]\n0=Overworld\n";
        const std::vector<std::uint8_t> metadata(text.begin(), text.end());
        GraphicsEditorDialog notes("NSMBeUD", metadata, nullptr, {}, true);
        QVERIFY(notes.findChild<QTreeWidget*>("MetadataTree"));

        GraphicsEditorDialog sheet("root/enemy/test_ncg.bin", std::vector<std::uint8_t>(32));
        QVERIFY(sheet.findChild<QWidget*>("GraphicsCanvas"));
        QVERIFY(!sheet.findChild<QTableWidget*>("BnxlTable"));
    }

    void messageEditorRoundTripsAndEditsBmgMessages() {
        const auto bytes = sampleBmg();
        QVERIFY(!bytes.empty());
        QVERIFY(MessageEditorDialog::supports(bytes, "script/course.bmg"));

        MessageEditorDialog dialog("script/course.bmg", bytes);
        auto* list = dialog.findChild<QTableWidget*>("MessageList");
        auto* editor = dialog.findChild<QPlainTextEdit*>("MessageText");
        auto* filter = dialog.findChild<QLineEdit*>("MessageFilter");
        QVERIFY(list && editor && filter);
        QCOMPARE(list->rowCount(), 2);
        // The list previews plain text; escapes belong in the editor, spelled the way the
        // NDS BMG Editor spells them.
        QCOMPARE(list->item(0, 2)->text(), QString(" Luigi has appeared!"));
        QCOMPARE(list->item(0, 1)->text(), QString("40"));
        QCOMPARE(editor->toPlainText(), QString("[255:00000100] Luigi has appeared!"));

        // Untouched in, untouched out.
        auto untouched = dialog.editedBytes();
        QVERIFY(untouched);
        QCOMPARE(untouched.value(), bytes);

        filter->setText("Welcome");
        QVERIFY(list->isRowHidden(0));
        QVERIFY(!list->isRowHidden(1));
        filter->clear();

        list->setCurrentCell(1, 2);
        editor->setPlainText("Bye![1:0A]");
        auto edited = dialog.editedBytes();
        QVERIFY(edited);
        auto parsed = nsmb::formats::parse_bmg(edited.value());
        QVERIFY(parsed);
        QCOMPARE(parsed.value().messages.size(), std::size_t{2});
        QCOMPARE(nsmb::formats::bmg_message_text(parsed.value().messages[1]), std::string("Bye![1:0A]"));
        // The edit must not disturb the other message, its attributes, or its MID1 ID.
        QCOMPARE(parsed.value().messages[0], nsmb::formats::parse_bmg(bytes).value().messages[0]);
        QCOMPARE(parsed.value().messages[1].attributes, std::vector<std::uint8_t>({2, 0, 0, 0}));
        QCOMPARE(parsed.value().messages[1].id.value_or(0), std::uint32_t{41});
    }

    // A BMG with neither MID1 IDs nor INF1 attribute bytes: the Message ID and Attributes
    // fields must not appear at all, rather than float unmanaged over the text editor.
    void messageEditorHidesFieldsTheFileDoesNotCarry() {
        nsmb::formats::Bmg bareBmg;
        bareBmg.messages.push_back({{{false, "Hello!", 0, {}}}, {}, std::nullopt});
        auto written = nsmb::formats::write_bmg(bareBmg);
        QVERIFY(written);

        MessageEditorDialog dialog("script/error.bmg", written.value());
        dialog.show();
        QVERIFY(QTest::qWaitForWindowExposed(&dialog));
        auto* identifier = dialog.findChild<QSpinBox*>("MessageId");
        auto* attributes = dialog.findChild<QLineEdit*>("MessageAttributes");
        QVERIFY(identifier && attributes);
        QVERIFY(!identifier->isVisible());
        QVERIFY(!attributes->isVisible());
    }

    void openingAFileChoosesOneEditorThroughAnyLzWrapper() {
        using bse::editors::Kind;
        const QByteArray pad(28, '\0');
        const auto bytes = [](const QByteArray& value) {
            return std::vector<std::uint8_t>(value.begin(), value.end());
        };
        QCOMPARE(bse::editors::classify(sampleBmg(), "script/course.bmg"), Kind::Messages);
        // A uiStudio layout is a page of the one graphics editor, not an editor of its own.
        QCOMPARE(bse::editors::classify(bytes(QByteArray("JNBL") + QByteArray("\0\0\1\0\1\0\2\0\x10\x0c", 10)),
                                        "uiStudio/a.bnbl"),
                 Kind::Graphics);
        QCOMPARE(bse::editors::classify(bytes(QByteArray(64, '\0')), "BG_ncg/a_ncg.bin"), Kind::Graphics);
        QCOMPARE(bse::editors::classify(bytes(QByteArray("nothing here at all")), "notes.txt"), Kind::None);

        // Compression is the dispatcher's business, not each editor's: an LZ-wrapped file must
        // reach the same editor as the plain one, whatever that editor happens to be.
        auto compressed = nsmb::formats::compress_nitro_lz10(sampleBmg());
        QVERIFY(compressed);
        QCOMPARE(bse::editors::classify(compressed.value(), "script/course.bmg"), Kind::Messages);
        QVERIFY(!MessageEditorDialog::supports(compressed.value(), "script/course.bmg"));
    }

    void filesystemDockOffersOneOpenActionForEveryEditableFile() {
        QTemporaryDir files;
        QVERIFY(files.isValid());
        nsmb::project::FilesystemView view;
        auto effective = romFixture(files.path(), view,
                                    {{"script/course.bmg", sampleBmg()},
                                     // nothing edits raw tileset data
                                     {"BG_unt/a.bin", std::vector<std::uint8_t>(16, 0)}});

        FilesystemDock dock;
        dock.setData({}, &effective, {}, std::move(view), {}, {}, {});
        // The two format-specific buttons are gone; one Open button covers every editor.
        QVERIFY(!dock.findChild<QPushButton*>("GraphicsEdit"));
        QVERIFY(!dock.findChild<QPushButton*>("UiResourceEdit"));
        auto* open = dock.findChild<QPushButton*>("OpenFile");
        auto* tree = dock.findChild<QTreeWidget*>("FilesystemTree");
        auto* details = dock.findChild<QLabel*>("FilesystemDetails");
        QVERIFY(open && tree && details);

        const auto select = [&](const QString& romPath) {
            for (QTreeWidgetItemIterator it(tree); *it; ++it)
                if ((*it)->data(0, Qt::UserRole).toString() == romPath) { tree->setCurrentItem(*it); return true; }
            return false;
        };
        QVERIFY(select("script/course.bmg"));
        QVERIFY(open->isEnabled());
        QVERIFY(details->text().contains("Opens in: Messages"));

        QVERIFY(select("BG_unt/a.bin"));
        QVERIFY(!open->isEnabled());
        QVERIFY(!details->text().contains("Opens in:"));
    }

    void anOpenedFileBecomesADocumentTabRatherThanAModalDialog() {
        // The whole shell used to freeze behind QDialog::exec() while a file was
        // being edited. It is a tab now, so this test can go on driving the
        // window after opening one.
        QTemporaryDir files;
        QVERIFY(files.isValid());
        const auto onDisk = files.filePath("course.bmg");
        QFile file(onDisk);
        QVERIFY(file.open(QIODevice::WriteOnly));
        const auto body = sampleBmg();
        file.write(reinterpret_cast<const char*>(body.data()), static_cast<qint64>(body.size()));
        file.close();

        MainWindow window;
        auto* tabs = window.findChild<QTabWidget*>("DocumentTabs");
        QVERIFY(tabs);
        const auto before = tabs->count();

        nsmb::project::FilesystemEntry entry;
        entry.path = "script/course.bmg";
        window.filesystemView_.entries.push_back(entry);
        window.openFileDocument("\nscript/course.bmg", "script/course.bmg", body, false, {},
                                [](const std::vector<std::uint8_t>&) { return nsmb::core::Result<void>{}; });

        QCOMPARE(tabs->count(), before + 1);
        auto* page = tabs->currentWidget();
        QVERIFY(page);
        QVERIFY(page->findChild<QPushButton*>("FileDocumentSave"));
        QVERIFY(page->findChild<QTableWidget*>("MessageList"));

        // Opening the same file again returns to its tab instead of stacking another.
        window.openFileDocument("\nscript/course.bmg", "script/course.bmg", body, false, {},
                                [](const std::vector<std::uint8_t>&) { return nsmb::core::Result<void>{}; });
        QCOMPARE(tabs->count(), before + 1);

        window.closeTab(tabs->indexOf(page));
        QCOMPARE(tabs->count(), before);
    }

    void previewAndDocksAreRecoverable() {
        MainWindow window;
        window.show();
        QTest::qWait(20);
        auto* tabs = window.findChild<QTabWidget*>("DocumentTabs");
        auto* preview = window.findChild<QAction*>("PreviewAction");
        QVERIFY(window.findChild<QAction*>("RomBannerAction"));
        QVERIFY(window.findChild<QAction*>("RomUserMetadataAction"));
        // The Fireflower and NSMBe-wrapper build actions are gone.
        QVERIFY(!window.findChild<QAction*>("FireflowerBuildAction"));
        QVERIFY(!window.findChild<QAction*>("NsmbeWrapperBuildAction"));
        auto* levels = window.findChild<QDockWidget*>("LevelsDock");
        QVERIFY(tabs);
        QVERIFY(preview);
        QVERIFY(levels);
        // The Project Hub is gone: its level tree was the Levels dock's tree with
        // a second selection model, and its pages restated the docks and menus.
        QCOMPARE(tabs->count(), 1);
        QVERIFY(!window.findChild<QTabWidget*>("ProjectHubTabs"));
        QVERIFY(!window.findChild<QTreeView*>("HubLevelTree"));
        QVERIFY(window.findChild<QTabWidget*>("ObjectPickerTabs"));
        QVERIFY(window.findChild<QToolBox*>("SpritePicker"));
        QVERIFY(window.findChild<QTreeWidget*>("CustomActorTree"));
        QVERIFY(window.findChild<QComboBox*>("VariantSelector"));
        tabs->tabCloseRequested(tabs->indexOf(window.preview_));
        QCOMPARE(tabs->count(), 0);
        QVERIFY(!preview->isChecked());
        preview->trigger();
        QCOMPARE(tabs->count(), 1);
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

    void oneVariantSelectorDrivesTheWholeWindow() {
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto root = std::filesystem::path(directory.path().toStdString());
        std::ofstream(root / "ncpatcher.yaml")
            << "version: 2\nvariants:\n  en:\n    defines: EN\n  fr:\n    defines: FR\n";
        // Which variants a project has is NCPatcher's answer, not something the
        // editor reads out of the YAML itself.
        std::filesystem::create_directories(root / "fake-ncpatcher");
        std::ofstream(root / "fake-ncpatcher/config-dump.json")
            << "{\"schema\":\"ncpatcher.config/1\",\"version\":2,\"variants\":{"
               "\"en\":{\"defines\":[\"EN\"]},\"fr\":{\"defines\":[\"FR\"]}}}";
        const auto project = make_project(root);

        MainWindow window;
        window.patcherPanel_->setExecutable(BSE_FAKE_NCPATCHER);
        window.setProject(project);

        // Exactly one selector, and it carries the bare variant name in the data
        // role -- reading the label instead is what made switching a no-op.
        auto* selector = window.findChild<QComboBox*>("VariantSelector");
        QVERIFY(selector);
        QVERIFY(!window.findChild<QComboBox*>("ActiveVariant"));
        QVERIFY(selector->findData(QString("en")) >= 0);
        QVERIFY(selector->findData(QString("fr")) >= 0);

        // Opening a level caches a session keyed to the variant it was loaded in.
        const QString course = "course/A01_1.bin";
        const QString bgdat = "course/A01_1_bgdat.bin";
        const auto index = window.findLevelIndex(course, bgdat);
        QVERIFY(index.isValid());
        window.openLevelEditor(index);
        QCOMPARE(window.documents_.size(), 1U);
        const auto before = window.documentKey(course, bgdat);
        QVERIFY(window.levelSessions_.contains(before));

        // Switching drops that session rather than handing it back for the other
        // variant's layers, and the build is told about the change too.
        selector->setCurrentIndex(selector->findData(QString("fr")));
        QCOMPARE(window.activeVariant_, QString("fr"));
        QVERIFY(!window.levelSessions_.contains(before));
        QCOMPARE(window.patcherPanel_->selectedVariant(), QString("fr"));

        // The level comes back, keyed to the variant now in force.
        const auto after = window.documentKey(course, bgdat);
        QVERIFY(after != before);
        QCOMPARE(window.documents_.size(), 1U);
        QVERIFY(window.documents_.contains(after));
    }

    void anUnlistedVariantIsKeptAndLabelled() {
        // A stored variant survives a project that cannot confirm it. Silently
        // resetting to "none" would switch the next build to a different
        // language without saying so.
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto project = make_project(std::filesystem::path(directory.path().toStdString()));

        MainWindow window;
        window.setProject(project);
        // As a restored workspace does: it names a variant, and the project it
        // is reopened against no longer lists it.
        window.activeVariant_ = "fr";
        window.reloadFilesystemView();

        auto* selector = window.findChild<QComboBox*>("VariantSelector");
        QVERIFY(selector);
        QCOMPARE(window.activeVariant_, QString("fr"));
        const auto index = selector->findData(QString("fr"));
        QVERIFY(index >= 0);
        QCOMPARE(selector->currentIndex(), index);
        QVERIFY(selector->itemText(index).contains("not configured"));
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

    // Helper for the pairing tests below: a one-tile-wide NCGR whose every
    // pixel is index 1, plus an NCLR that paints index 1 red.
    static std::vector<std::uint8_t> make_ncgr(std::vector<std::uint8_t> characters,
                                               std::uint16_t width_tiles,
                                               std::uint32_t character_format = 0) {
        nsmb::formats::Ncgr ncgr;
        ncgr.width = width_tiles;
        ncgr.height = static_cast<std::uint16_t>(characters.size() / 32U / std::max<std::uint16_t>(1, width_tiles));
        ncgr.pixel_format = 3;  // GX_TEXFMT_PLTT16 — 4bpp
        ncgr.character_format = character_format;
        ncgr.characters = std::move(characters);
        return nsmb::formats::write_ncgr(ncgr).value();
    }

    void aPairedPaletteColoursTheGraphicsInsteadOfTheGreyRamp() {
        // Index 1 across one whole tile.
        const auto ncgr = make_ncgr(std::vector<std::uint8_t>(32, 0x11), 1);
        nsmb::formats::Nclr nclr;
        nclr.palette.assign(32, 0);
        // BGR555 colour 1 is pure red.
        nclr.palette[2] = 0x1f;
        const auto encoded = nsmb::formats::write_nclr(nclr).value();

        bse::editors::Companions companions;
        companions.paths << "pair/red.nclr";
        companions.load = [&](const QString&) -> nsmb::core::Result<std::vector<std::uint8_t>> { return encoded; };

        GraphicsEditorDialog dialog("pair/sheet.ncgr", ncgr, nullptr, companions);
        dialog.show();
        QTest::qWait(10);
        auto* source = dialog.findChild<QComboBox*>("GraphicsPaletteSource");
        QVERIFY(source);
        const auto index = source->findData(QString("pair/red.nclr"));
        QVERIFY(index >= 0);
        source->setCurrentIndex(index);

        auto* canvas = dialog.findChild<QWidget*>("GraphicsCanvas");
        QVERIFY(canvas);
        const auto zoom = canvas->width() / 8;
        const auto painted = canvas->grab().toImage().pixelColor(zoom / 2, zoom / 2);
        // A grey ramp would give equal channels; a real palette does not.
        QVERIFY(painted.red() > painted.green() + 40);
        QVERIFY(painted.red() > painted.blue() + 40);
    }

    void aBitmapFormattedNcgrIsNotDeTiled() {
        // Four tiles, two across, with one bright pixel at flat offset 8. Read
        // linearly that lands at (8, 0); de-tiled it would land at (0, 1).
        std::vector<std::uint8_t> characters(128, 0);
        characters[4] = 0x0f;
        const auto ncgr = make_ncgr(characters, 2, 1);  // NNS_G2D_CHARACTER_FMT_BMP

        GraphicsEditorDialog dialog("bmp/sheet.ncgr", ncgr);
        dialog.show();
        QTest::qWait(10);
        auto* canvas = dialog.findChild<QWidget*>("GraphicsCanvas");
        QVERIFY(canvas);
        const auto zoom = canvas->width() / 16;
        QVERIFY(zoom > 0);
        const auto grabbed = canvas->grab().toImage();
        const auto at = [&](int x, int y) { return grabbed.pixelColor(x * zoom + zoom / 2, y * zoom + zoom / 2); };
        QVERIFY(at(8, 0).lightness() > 200);
        QVERIFY(at(0, 1).lightness() < 50);
    }

    void aHeaderlessSheetHonoursTheWidthItIsGiven() {
        // Eight tiles: the square-root guess would lay them out three across.
        GraphicsEditorDialog dialog("raw/width_ncg.bin", std::vector<std::uint8_t>(8 * 32, 0));
        dialog.show();
        QTest::qWait(10);
        auto* width = dialog.findChild<QSpinBox*>("GraphicsRawWidth");
        auto* canvas = dialog.findChild<QWidget*>("GraphicsCanvas");
        QVERIFY(width);
        QVERIFY(canvas);
        width->setValue(8);
        // One row of eight tiles is eight times as wide as it is tall.
        QCOMPARE(canvas->width(), canvas->height() * 8);
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
        // A catalog that claims block 0, so the 3D toggle has something to do and
        // the round trips below prove the substitution never reaches the file.
        auto overlays = std::make_shared<nsmb::project::TileOverlayCatalog>();
        nsmb::project::TileOverlayScript script;
        script.name = "test.block-zero";
        nsmb::formats::RgbaImage texture{16, 16, std::vector<std::uint8_t>(16 * 16 * 4, 0x7f)};
        script.blocks[0] = nsmb::project::TileOverlay{std::make_shared<const nsmb::formats::RgbaImage>(std::move(texture)), std::nullopt, true};
        overlays->scripts.push_back(std::move(script));
        TilesetEditorDialog dialog(std::move(tilesets), tilemap.value(), tilemap.value(), {1,2,3,4}, overlays);
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
        auto* threeD = dialog.findChild<QCheckBox*>("Map16ThreeD");
        QVERIFY(threeD);
        QVERIFY(threeD->isChecked());
        threeD->setChecked(false);
        threeD->setChecked(true);
        // Drawing a block as the game draws it must not edit the block.
        const auto overlaid = dialog.tilesetData(1);
        QVERIFY(overlaid);
        QCOMPARE(overlaid.value().map16, files.map16);
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

    void rightClickCreatesTheCurrentKindAndLeftClickNeverDoes() {
        auto level = make_level();
        LevelPreviewWidget canvas;
        canvas.resize(800, 600);
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);
        canvas.setCreateKind(1);
        canvas.show();
        QTest::qWait(20);
        QSignalSpy changed(&canvas, &LevelPreviewWidget::levelChanged);

        // Left-click selects and marquees. It never creates, whatever kind is set.
        QTest::mouseClick(&canvas, Qt::LeftButton, {}, QPoint(400, 350));
        QVERIFY(level.objects().empty());

        // Right-click creates without needing anything armed from the palette:
        // the default is object 0 of tileset slot 0.
        QTest::mouseClick(&canvas, Qt::RightButton, {}, QPoint(400, 350));
        QCOMPARE(level.objects().size(), 1U);
        QCOMPARE(level.objects()[0].tileset_slot, 0);
        QCOMPARE(level.objects()[0].object_id, 0);

        // A palette pick changes what is stamped, not whether stamping works.
        canvas.setPlacementObject(1, 12);
        QCOMPARE(canvas.createKind(), 1);
        QTest::mouseClick(&canvas, Qt::RightButton, {}, QPoint(420, 370));
        QCOMPARE(level.objects().size(), 2U);
        QCOMPARE(changed.count(), 2);
        QCOMPARE(level.objects()[1].tileset_slot, 1);
        QCOMPARE(level.objects()[1].object_id, 12);

        // The kind alone decides what is created; views used to be reachable
        // only from the properties panel's Add button.
        canvas.setCreateKind(7);
        const auto views = level.course().views.size();
        QTest::mouseClick(&canvas, Qt::RightButton, {}, QPoint(440, 390));
        QCOMPARE(level.course().views.size(), views + 1U);

        QVERIFY(level.can_undo());
        QVERIFY(level.undo());
        canvas.refreshLevelState();
        QCOMPARE(level.course().views.size(), views);
    }

    void theLevelListHoldsOnlyLevelsAndNestsTheVersusGroups() {
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto project = make_project(std::filesystem::path(directory.path().toStdString()));

        ProjectTreeModel model;
        model.setProject(project);

        const auto topLevel = [&](const QString& name) {
            for (int row = 0; row < model.rowCount({}); ++row)
                if (model.data(model.index(row, 0), Qt::DisplayRole).toString() == name)
                    return model.index(row, 0);
            return QModelIndex{};
        };

        // Loose course-shaped files are not levels, so they get no bucket here.
        QVERIFY(!topLevel("Other Files").isValid());

        // "Mario vs. Luigi > Multicard" is a path, not a label: one group node
        // holding the two modes, each holding the levels.
        const auto versus = topLevel("Mario vs. Luigi");
        QVERIFY(versus.isValid());
        QCOMPARE(model.rowCount(versus), 2);
        QCOMPARE(model.data(model.index(0, 0, versus), Qt::DisplayRole).toString(), QString("Multicard"));
        QCOMPARE(model.data(model.index(1, 0, versus), Qt::DisplayRole).toString(), QString("Download Play"));
        QVERIFY(model.rowCount(model.index(0, 0, versus)) > 0);
        QVERIFY(model.rowCount(model.index(1, 0, versus)) > 0);

        // The mode names carry no parenthetical essay any more.
        for (int row = 0; row < model.rowCount(versus); ++row)
            QVERIFY(!model.data(model.index(row, 0, versus), Qt::DisplayRole).toString().contains('('));
    }

    void levelRowsNamePathsTheFilesystemWayAndCanJumpToThem() {
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto project = make_project(std::filesystem::path(directory.path().toStdString()));

        MainWindow window;
        window.setProject(project);
        const QString course = "course/A01_1.bin";
        const QString bgdat = "course/A01_1_bgdat.bin";
        const auto index = window.findLevelIndex(course, bgdat);
        QVERIFY(index.isValid());

        // The columns read the way the Filesystem tab reads: from the NitroFS
        // root, not from the project directory.
        const auto row = index.sibling(index.row(), 1);
        QCOMPARE(window.projectModel_->data(row, Qt::DisplayRole).toString(), QString("course/A01_1.bin"));
        QCOMPARE(window.projectModel_->data(index.sibling(index.row(), 2), Qt::DisplayRole).toString(),
                 QString("course/A01_1_bgdat.bin"));
        QCOMPARE(window.projectModel_->data(index, ProjectTreeModel::CoursePathRole).toString(),
                 QString("course/A01_1.bin"));
        // The path roles the rest of the window opens levels with are unchanged.
        QCOMPARE(window.projectModel_->data(index, ProjectTreeModel::CoursePathRole).toString(), course);

        // And the row can hand the user over to that file.
        QVERIFY(window.filesystemPanel_->revealPath("course/A01_1.bin"));
        auto* tree = window.filesystemPanel_->findChild<QTreeWidget*>("FilesystemTree");
        QVERIFY(tree && tree->currentItem());
        QCOMPARE(tree->currentItem()->data(0, Qt::UserRole).toString(), QString("course/A01_1.bin"));
        QVERIFY(!window.filesystemPanel_->revealPath("course/nothing_here.bin"));
    }

    void objectPreviewsShowTheWholeObjectPerTilesetSlot() {
        // A pipe previewed at one tile is a corner, which says nothing about
        // what is being placed. Previews render at the object's own extent, and
        // the picker splits by tileset slot the way ObjectPickerControlNew does.
        auto tileset = std::make_shared<nsmb::project::DecodedTileset>();
        nsmb::formats::RgbaImage tile{16, 16, {}};
        tile.pixels.assign(16 * 16 * 4, 0xff);
        tileset->map16_tiles.assign(4, tile);
        nsmb::formats::ObjectLayout wide;
        wide.rows = {{{0, 0}, {1, 0}}, {{2, 0}, {3, 0}}};  // two map16 tiles across, two down
        tileset->objects.push_back(wide);

        LevelPreviewWidget scratch;
        scratch.tilesets_[0] = tileset;
        const auto entries = scratch.paletteEntries();
        QCOMPARE(entries.size(), 1U);
        QCOMPARE(entries.front().slot, 0);
        QVERIFY(entries.front().image.width() > 16);
        QVERIFY(entries.front().image.height() > 16);
        QVERIFY(entries.front().tooltip.contains("2x2"));

        // One Objects sub-tab per loaded slot, and none for an empty one.
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto project = make_project(std::filesystem::path(directory.path().toStdString()));
        MainWindow window;
        window.setProject(project);
        const auto index = window.findLevelIndex("course/A01_1.bin",
                                                 "course/A01_1_bgdat.bin");
        QVERIFY(index.isValid());
        window.openLevelEditor(index);
        QCOMPARE(window.documents_.size(), 1U);
        auto* view = window.documents_.begin()->second.view;
        QVERIFY(view);
        view->tilesets_[0] = tileset;
        view->tilesets_[2] = tileset;
        window.refreshPalette();

        auto* tabs = window.findChild<QTabWidget*>("ObjectPickerTabs");
        QVERIFY(tabs);
        QCOMPARE(tabs->count(), 2);
        QCOMPARE(tabs->tabText(0), QString("Tileset 0"));
        QCOMPARE(tabs->tabText(1), QString("Tileset 2"));
        QVERIFY(window.findChild<QWidget*>("ObjectPicker0"));
        QVERIFY(window.findChild<QWidget*>("ObjectPicker2"));
        QVERIFY(!window.findChild<QWidget*>("ObjectPicker1"));
    }

    void objectsThatDrawNothingSayWhichNothingTheyAre() {
        // Every tileset opens with objects that put no art on screen, and they
        // all previewed as the same empty button. They are not the same thing:
        // object 0 writes the empty block and clears the cells it covers, while
        // the ones naming blocks 1 to 6 are never written at all and leave what
        // is under them standing.
        auto tileset = std::make_shared<nsmb::project::DecodedTileset>();
        nsmb::formats::RgbaImage tile{16, 16, {}};
        tile.pixels.assign(16 * 16 * 4, 0xff);
        tileset->map16_tiles.assign(8, tile);
        nsmb::formats::ObjectLayout eraser;
        eraser.rows = {{{nsmb::formats::object_layout_empty_tile, 0}}};
        nsmb::formats::ObjectLayout inert;
        inert.rows = {{{1, 0}}};
        nsmb::formats::ObjectLayout ordinary;
        ordinary.rows = {{{7, 0}}};
        tileset->objects = {eraser, inert, ordinary};

        LevelPreviewWidget scratch;
        scratch.tilesets_[0] = tileset;
        const auto entries = scratch.paletteEntries();
        QCOMPARE(entries.size(), 3U);
        QVERIFY(entries[0].label.contains("eraser"));
        QVERIFY(entries[0].tooltip.contains("clears"));
        QVERIFY(entries[1].label.contains("draws nothing"));
        QVERIFY(entries[1].tooltip.contains("leaves the cells alone"));
        // Two glyphs, not one, and neither is the empty preview they had.
        QVERIFY(!entries[0].image.isNull() && !entries[1].image.isNull());
        QVERIFY(entries[0].image != entries[1].image);
        // An object that does draw keeps its own preview.
        QVERIFY(!entries[2].label.contains("eraser") && !entries[2].label.contains("nothing"));
    }

    void thePickerNamesTheObjectsThatDrawNothing() {
        // Jyotyu objects 0 to 6 all preview as an empty square: 0 writes the
        // empty block and 1 to 6 name blocks the game never writes. Left as
        // previews they are seven identical blanks, one of which is the eraser
        // every level is built with.
        auto tileset = std::make_shared<nsmb::project::DecodedTileset>();
        nsmb::formats::RgbaImage tile{16, 16, {}};
        tile.pixels.assign(16 * 16 * 4, 0);
        tileset->map16_tiles.assign(8, tile);
        nsmb::formats::ObjectLayout eraser;
        eraser.rows.push_back({{nsmb::formats::object_layout_empty_tile, 0}});
        nsmb::formats::ObjectLayout inert;
        inert.rows.push_back({{1, 0}});
        nsmb::formats::ObjectLayout ordinary;
        ordinary.rows.push_back({{7, 0}});
        tileset->objects = {eraser, inert, ordinary};

        LevelPreviewWidget scratch;
        scratch.tilesets_[0] = tileset;
        const auto entries = scratch.paletteEntries();
        QCOMPARE(entries.size(), 3U);
        QVERIFY(entries[0].label.contains("eraser"));
        QVERIFY(entries[0].tooltip.contains("clears the cells it covers"));
        QVERIFY(entries[1].label.contains("draws nothing"));
        QVERIFY(entries[1].tooltip.contains("leaves the cells alone"));
        QCOMPARE(entries[2].label, QString("Object 0:2"));
        // The two that do nothing are told apart by their glyph, so neither is
        // the blank the block itself would have drawn.
        QVERIFY(!entries[0].image.isNull());
        QVERIFY(!entries[1].image.isNull());
        QVERIFY(entries[0].image != entries[1].image);
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
        canvas.setCreateKind(1);
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

    void everyResizableKindHasEightKnobs() {
        auto level = make_level();
        QVERIFY(level.add_object({1, 0, 10, 10, 4, 4}));
        QVERIFY(level.add_zone({320, 320, 256, 192, 0}));
        nsmb::formats::NsmbLevelView view{};
        view.x = 640; view.y = 640; view.width = 256; view.height = 192;
        QVERIFY(level.add_view(view));

        LevelPreviewWidget canvas;
        canvas.resize(900, 700);
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);
        canvas.show();
        QTest::qWait(20);

        // Objects, zones and views all resize; sprites and entrances do not.
        for (const int kind : {1, 4, 7}) {
            canvas.setSelection(kind, 0, -1);
            const auto rect = canvas.selectedResizableRect();
            QVERIFY2(rect.has_value(), qPrintable(QString("kind %1 has no resizable rect").arg(kind)));
            const auto screen = canvas.sceneTransform().mapRect(*rect);

            // All eight knobs answer, and each reports its own corner or edge.
            const std::array centres{screen.topLeft(),
                                     QPointF(screen.center().x(), screen.top()),
                                     screen.topRight(),
                                     QPointF(screen.right(), screen.center().y()),
                                     screen.bottomRight(),
                                     QPointF(screen.center().x(), screen.bottom()),
                                     screen.bottomLeft(),
                                     QPointF(screen.left(), screen.center().y())};
            for (std::size_t i = 0; i < centres.size(); ++i) {
                QCOMPARE(canvas.handleAt(centres[i]), static_cast<LevelPreviewWidget::Handle>(i + 1));
            }
            QCOMPARE(canvas.handleAt(screen.center()), LevelPreviewWidget::Handle::None);
        }

        canvas.setSelection(2, 0, -1);
        QVERIFY(!canvas.selectedResizableRect().has_value());
    }

    void nodelessKindsStayResizableWhenTheNodeIndexArrivesAsZero() {
        // The properties panel's node spin box floors at 0, so selecting a view
        // on the canvas comes back through it as node 0 rather than -1. That
        // must still be the same selection, or the knobs draw but cannot be
        // grabbed.
        auto level = make_level();
        QVERIFY(level.add_object({1, 0, 10, 10, 4, 4}));
        QVERIFY(level.add_zone({320, 320, 256, 192, 0}));
        nsmb::formats::NsmbLevelView view{};
        view.x = 640; view.y = 640; view.width = 256; view.height = 192;
        QVERIFY(level.add_view(view));

        LevelPreviewWidget canvas;
        canvas.resize(900, 700);
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);
        canvas.show();
        QTest::qWait(20);

        for (const int kind : {1, 4, 7}) {
            canvas.setSelection(kind, 0, 0);
            const auto rect = canvas.selectedResizableRect();
            QVERIFY2(rect.has_value(), qPrintable(QString("kind %1 lost its knobs to node 0").arg(kind)));
            const auto screen = canvas.sceneTransform().mapRect(*rect);
            QCOMPARE(canvas.handleAt(screen.bottomRight()), LevelPreviewWidget::Handle::BottomRight);
        }
    }

    void topLeftKnobMovesTheViewOriginRatherThanItsExtent() {
        auto level = make_level();
        nsmb::formats::NsmbLevelView view{};
        view.x = 320; view.y = 320; view.width = 256; view.height = 192;
        QVERIFY(level.add_view(view));

        LevelPreviewWidget canvas;
        canvas.resize(900, 700);
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);
        canvas.setSelection(7, 0, -1);
        canvas.show();
        QTest::qWait(20);

        // Dragging the bottom-right knob grows the view away from its origin.
        auto screen = canvas.sceneTransform().mapRect(*canvas.selectedResizableRect());
        auto knob = screen.bottomRight().toPoint();
        QTest::mousePress(&canvas, Qt::LeftButton, {}, knob);
        QTest::mouseMove(&canvas, knob + QPoint(48, 48));
        QTest::mouseRelease(&canvas, Qt::LeftButton, {}, knob + QPoint(48, 48));
        QCOMPARE(level.course().views[0].x, 320);
        QCOMPARE(level.course().views[0].y, 320);
        QVERIFY(level.course().views[0].width > 256);
        QVERIFY(level.course().views[0].height > 192);

        // Dragging the top-left knob moves the origin instead, which the old
        // single bottom-right handle could not express at all.
        const auto width = level.course().views[0].width;
        const auto height = level.course().views[0].height;
        canvas.refreshLevelState();
        canvas.setSelection(7, 0, -1);
        screen = canvas.sceneTransform().mapRect(*canvas.selectedResizableRect());
        knob = screen.topLeft().toPoint();
        QTest::mousePress(&canvas, Qt::LeftButton, {}, knob);
        QTest::mouseMove(&canvas, knob + QPoint(32, 32));
        QTest::mouseRelease(&canvas, Qt::LeftButton, {}, knob + QPoint(32, 32));
        QVERIFY(level.course().views[0].x > 320);
        QVERIFY(level.course().views[0].y > 320);
        QVERIFY(level.course().views[0].width < width);
        QVERIFY(level.course().views[0].height < height);
    }

    void propertiesPreviewAResizeBeforeItIsCommitted() {
        auto level = make_level();
        nsmb::formats::NsmbLevelView view{};
        view.x = 320; view.y = 320; view.width = 256; view.height = 192;
        QVERIFY(level.add_view(view));

        LevelPreviewWidget canvas;
        LevelPropertiesWidget properties;
        canvas.resize(900, 700);
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);
        canvas.setSelection(7, 0, -1);
        properties.setLevel(&level);
        properties.select(LevelPropertiesWidget::Kind::View, 0);
        connect(&canvas, &LevelPreviewWidget::interactionPreviewRequested,
                &properties, &LevelPropertiesWidget::previewInteraction);
        canvas.show();
        properties.resize(360, 600);
        properties.show();
        QTest::qWait(20);

        const auto knob = canvas.sceneTransform().mapRect(*canvas.selectedResizableRect()).bottomRight().toPoint();
        QTest::mousePress(&canvas, Qt::LeftButton, {}, knob);
        QTest::mouseMove(&canvas, knob + QPoint(48, 48));

        // The inspector tracks the gesture while the undoable model edit still
        // waits for mouse release.
        QCOMPARE(level.course().views[0].width, 256);
        QVERIFY(properties.fields_[2].value->value() > 256);
        QCOMPARE(properties.fields_[0].container->y(), properties.fields_[1].container->y());
        QVERIFY(properties.fields_[0].container->x() < properties.fields_[1].container->x());

        QTest::mouseRelease(&canvas, Qt::LeftButton, {}, knob + QPoint(48, 48));
        QCOMPARE(properties.fields_[2].value->value(), static_cast<int>(level.course().views[0].width));
    }

    void selectionIgnoresTheCreateKind() {
        auto level = make_level();
        QVERIFY(level.add_object({1, 0, 1, 1, 1, 1}));
        QVERIFY(level.add_sprite({7, 10, 10, {}}));
        LevelPreviewWidget canvas;
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);

        // A mixed selection survives a change of create kind. Deciding what to
        // place next is not a reason to drop what is already held.
        canvas.selections_ = {{1, 0, -1}, {2, 0, -1}};
        canvas.setCreateKind(1);
        QCOMPARE(canvas.selections_.size(), 2U);

        // Both entities stay hittable whatever the kind is, so an object never
        // has to be "switched to" before it can be grabbed.
        for (const int kind : {1, 2, 4, 7}) {
            canvas.setCreateKind(kind);
            QCOMPARE(canvas.createKind(), kind);
            const auto object = canvas.hitTest({20, 20});
            const auto sprite = canvas.hitTest({160, 160});
            QVERIFY(object);
            QCOMPARE(object->kind, 1);
            QVERIFY(sprite);
            QCOMPARE(sprite->kind, 2);
        }
    }

    void containersYieldTheClickToWhatTheyEnclose() {
        // A view or a zone covers most of the level, so picking it first would
        // make everything inside it unreachable. Overlapping entities pick in
        // this order: paths, entrances, sprites, objects, zones, views.
        auto level = make_level();
        QVERIFY(level.add_object({1, 0, 16, 16, 4, 4}));
        QVERIFY(level.add_zone({0, 0, 1024, 768, 0}));
        nsmb::formats::NsmbLevelView view{};
        view.x = 0; view.y = 0; view.width = 1024; view.height = 768;
        QVERIFY(level.add_view(view));

        LevelPreviewWidget canvas;
        canvas.resize(900, 700);
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);

        // Over the object, all three overlap and the object wins.
        const auto over_object = canvas.hitTest({260, 260});
        QVERIFY(over_object);
        QCOMPARE(over_object->kind, 1);

        // Clear of the object, the zone still beats the larger view.
        const auto over_zone = canvas.hitTest({700, 600});
        QVERIFY(over_zone);
        QCOMPARE(over_zone->kind, 4);

        // A view is reachable wherever nothing else covers it.
        QVERIFY(level.update_zone(0, {0, 0, 64, 64, 0}));
        const auto over_view = canvas.hitTest({700, 600});
        QVERIFY(over_view);
        QCOMPARE(over_view->kind, 7);
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
        canvas.setCreateKind(2);
        canvas.actor_object_ids_.emplace(7, 42);
        canvas.setSpriteDefinitions(catalog);
        const auto hit = canvas.hitTest({130, 150});
        QVERIFY(hit);
        QCOMPARE(hit->kind, 2);
    }

    void spriteSettingsSurviveTheFieldsThatReadThem() {
        // The declared fields are a reading of the sprite's data value, not a
        // replacement for it: the raw box stays, it reads back what a field
        // wrote, and nothing but a field the user turned rewrites it.
        auto level = make_level();
        nsmb::formats::NsmbLevelSprite sprite;
        sprite.id = 7; sprite.x = 10; sprite.y = 10; sprite.settings = 0x12345678U; sprite.events = {0xab, 0xcd};
        QVERIFY(level.add_sprite(sprite));
        nsmb::project::SpriteDefinition definition;
        definition.object_id = 42;
        definition.name = "Synthetic actor";
        definition.fields.push_back({"speed", "Speed", 4, 3, {}});
        nsmb::project::SpriteDefinitionCatalog catalog{{definition}};

        LevelPropertiesWidget properties;
        properties.setSpriteDefinitions(catalog, {{7, 42}});
        properties.setLevel(&level);
        properties.select(LevelPropertiesWidget::Kind::Sprite, 0);
        auto* raw = properties.findChild<QLineEdit*>("SpriteDataField");
        QVERIFY(raw);
        // The settings word from its high byte down. The two event bytes are
        // not settings -- they name events, and each has its own row.
        QCOMPARE(raw->text(), QString("12 34 56 78"));

        // A field edit lands in the raw value, and only in the bits it owns.
        properties.fields_[5].value->setValue(2);
        QCOMPARE(level.course().sprites[0].settings, 0x12345628U);
        QCOMPARE(raw->text(), QString("12 34 56 28"));

        // Changing the object ID keeps the settings: the sprite is now read by
        // an actor that declares nothing, and the value is still there to read.
        properties.fields_[0].value->setValue(9);
        QCOMPARE(level.course().sprites[0].id, 9);
        QCOMPARE(level.course().sprites[0].settings, 0x12345628U);
        QCOMPARE(raw->text(), QString("12 34 56 28"));
        QVERIFY(properties.fields_[5].container->isHidden());

        // Typing a value writes the settings word, and leaves the event bytes
        // to the rows that name them.
        properties.fields_[0].value->setValue(7);
        raw->setText("22 33 44 55");
        emit raw->textEdited(raw->text());
        QCOMPARE(level.course().sprites[0].events, (std::array<std::uint8_t, 2>{0xab, 0xcd}));
        QCOMPARE(level.course().sprites[0].settings, 0x22334455U);
        QCOMPARE(properties.rowValue(3), std::optional<int>(0xab));
        QCOMPARE(properties.rowValue(4), std::optional<int>(0xcd));
        QCOMPARE(properties.rowValue(5), std::optional<int>(5));

        // The event rows are named for what the game does with them.
        QCOMPARE(properties.fields_[3].label->text(), QString("Target event"));
        QCOMPARE(properties.fields_[4].label->text(), QString("Trigger event"));

        // Hex is a way of reading the same values, not different ones.
        auto* hex = properties.findChild<QCheckBox*>("HexFields");
        QVERIFY(hex);
        hex->click();
        QCOMPARE(properties.rowValue(4), std::optional<int>(0xcd));
        QCOMPARE(properties.fields_[4].value->text(), QString("0xCD"));
        QCOMPARE(level.course().sprites[0].settings, 0x22334455U);
    }

    void actorNotesStayOnTheFieldTheyBelongTo() {
        // What the actor says about itself is carried by the object ID that
        // picked it, and what it says about a field is carried by that field --
        // so a renamed label takes its documentation with it.
        auto level = make_level();
        nsmb::formats::NsmbLevelSprite sprite;
        sprite.id = 7; sprite.x = 10; sprite.y = 10;
        QVERIFY(level.add_sprite(sprite));
        nsmb::project::SpriteDefinition definition;
        definition.object_id = 42;
        definition.name = "Synthetic actor";
        definition.category = "Enemies";
        definition.notes = "Spawned by Chain Chomp/Log.";
        definition.fields.push_back({"speed", "Speed", 4, 3, {{1, "Slow"}, {2, "Fast"}},
                                     "8 is used by ones thrown out of the Monty Tank."});
        nsmb::project::SpriteDefinitionCatalog catalog{{definition}};

        LevelPropertiesWidget properties;
        properties.setSpriteDefinitions(catalog, {{7, 42}});
        properties.setLevel(&level);
        properties.select(LevelPropertiesWidget::Kind::Sprite, 0);
        QCOMPARE(properties.fields_[0].label->text(), QString("Object ID"));
        QCOMPARE(properties.fields_[0].note_text, QString("Spawned by Chain Chomp/Log."));
        QVERIFY(!properties.fields_[0].info->isHidden());
        QVERIFY(properties.fields_[5].note_text.startsWith("8 is used by ones thrown out of the Monty Tank."));
        QVERIFY(!properties.fields_[5].info->isHidden());

        // The panel names the actor it resolved, rather than leaving the reader
        // to know what object 7 was.
        QCOMPARE(properties.findChild<QLabel*>("SpriteName")->text(), QString("Synthetic actor"));
        QVERIFY(properties.findChild<QLabel*>("SpriteCategory")->text().startsWith("Enemies"));

        // A field that names its values is picked from them.
        QCOMPARE(properties.fields_[5].editor, LevelPropertiesWidget::FieldEditor::Choice);
        QVERIFY(properties.fields_[5].choice->findText("1 — Slow") >= 0);
        QVERIFY(properties.fields_[5].choice->findText("2 — Fast") >= 0);
        // The settings are zero and this actor does not name zero, so the row
        // says so rather than snapping to a value the sprite does not hold.
        QCOMPARE(properties.rowValue(5), std::optional<int>(0));
        QVERIFY(!properties.fields_[5].representable);
        QVERIFY(properties.fields_[5].note_text.contains("does not name the value 0"));

        // A refresh reports; it does not edit.
        properties.refresh();
        QCOMPARE(level.course().sprites[0].settings, 0U);

        // And a row that stops carrying a note puts its button away.
        properties.select(LevelPropertiesWidget::Kind::Entrance, 0);
        QVERIFY(properties.fields_[0].note_text.isEmpty());
        QVERIFY(properties.fields_[0].info->isHidden());
    }

    // A sprite carrying `settings`, read by an actor declaring `fields`.
    static nsmb::project::SpriteDefinitionCatalog actorFor(
        nsmb::project::EditableLevel& level, std::uint32_t settings,
        std::vector<nsmb::project::SpriteFieldDefinition> fields) {
        nsmb::formats::NsmbLevelSprite sprite;
        sprite.id = 7; sprite.x = 10; sprite.y = 10; sprite.settings = settings;
        [&] { QVERIFY(level.add_sprite(sprite)); }();
        nsmb::project::SpriteDefinition definition;
        definition.object_id = 42;
        definition.name = "Synthetic actor";
        definition.fields = std::move(fields);
        return {{definition}};
    }

    void aChoiceFieldOffersItsLabelsAndWritesTheSparseValue() {
        // The values an actor names are not consecutive, so the list cannot be
        // read by position: each entry carries the number it stands for.
        auto level = make_level();
        const auto catalog = actorFor(level, 0x000000ffU,
                                      {{"item", "Item", 8, 5, {{0, "A"}, {5, "B"}, {25, "C"}}}});
        LevelPropertiesWidget properties;
        properties.setSpriteDefinitions(catalog, {{7, 42}});
        properties.setLevel(&level);
        properties.select(LevelPropertiesWidget::Kind::Sprite, 0);

        QCOMPARE(properties.fields_[5].editor, LevelPropertiesWidget::FieldEditor::Choice);
        QCOMPARE(properties.fields_[5].choice->count(), 3);
        const auto item = properties.fields_[5].choice->findData(25);
        QVERIFY(item >= 0);
        properties.fields_[5].choice->setCurrentIndex(item);
        emit properties.fields_[5].choice->activated(item);

        // The value it stands for, in the bits it owns, and nowhere else.
        QCOMPARE(level.course().sprites[0].settings, (25U << 8U) | 0xffU);
        QCOMPARE(properties.rowValue(5), std::optional<int>(25));
    }

    void aCheckboxFieldTogglesOnlyTheBitsItOwns() {
        // A field that is on or off is a checkbox, whatever value turning it on
        // happens to write -- and it is not always 1.
        auto level = make_level();
        const auto catalog = actorFor(level, 0x00000f0fU, {{"on", "On", 4, 4, {}, "", 0, 8}});
        LevelPropertiesWidget properties;
        properties.setSpriteDefinitions(catalog, {{7, 42}});
        properties.setLevel(&level);
        properties.select(LevelPropertiesWidget::Kind::Sprite, 0);

        QCOMPARE(properties.fields_[5].editor, LevelPropertiesWidget::FieldEditor::Boolean);
        QCOMPARE(properties.fields_[5].toggle->text(), QString("On"));
        QCOMPARE(properties.fields_[5].toggle->checkState(), Qt::Unchecked);
        // A checkbox is its own label, so the row does not carry a second one.
        QVERIFY(properties.fields_[5].label->isHidden());

        properties.fields_[5].toggle->click();
        QCOMPARE(level.course().sprites[0].settings, 0x00000f8fU);
        QCOMPARE(properties.rowValue(5), std::optional<int>(1));

        properties.fields_[5].toggle->click();
        QCOMPARE(level.course().sprites[0].settings, 0x00000f0fU);
    }

    void aValueOutsideTheChoiceListIsShownRatherThanRewritten() {
        // The raw settings box can put anything in the bits. A list that does
        // not name what is there says so, and leaves it alone.
        auto level = make_level();
        const auto catalog = actorFor(level, 4U, {{"pick", "Pick", 0, 3, {{0, "A"}, {1, "B"}, {2, "C"}}},
                                                  {"speed", "Speed", 8, 4, {}}});
        LevelPropertiesWidget properties;
        properties.setSpriteDefinitions(catalog, {{7, 42}});
        properties.setLevel(&level);
        properties.select(LevelPropertiesWidget::Kind::Sprite, 0);

        QVERIFY(!properties.fields_[5].representable);
        QCOMPARE(properties.fields_[5].choice->currentText(), QString("4 — not listed"));
        QCOMPARE(properties.rowValue(5), std::optional<int>(4));
        QCOMPARE(level.course().sprites[0].settings, 4U);

        // Turning the field beside it does not carry the unnamed value away.
        properties.fields_[6].value->setValue(3);
        QCOMPARE(level.course().sprites[0].settings, (3U << 8U) | 4U);
        QCOMPARE(properties.rowValue(5), std::optional<int>(4));

        // And the row only grows one entry for it, however often it is read.
        properties.refresh();
        properties.refresh();
        QCOMPARE(properties.fields_[5].choice->count(), 4);
    }

    void anUnrepresentableCheckboxReadsAsIndeterminate() {
        // A four-bit field turned on by 1 can hold 5. A box ranged 0..1 showed
        // that as "on" and would have written the 5 away.
        auto level = make_level();
        const auto catalog = actorFor(level, 5U, {{"flag", "Flag", 0, 4, {}, "", 0, 1}});
        LevelPropertiesWidget properties;
        properties.setSpriteDefinitions(catalog, {{7, 42}});
        properties.setLevel(&level);
        properties.select(LevelPropertiesWidget::Kind::Sprite, 0);

        QCOMPARE(properties.fields_[5].toggle->checkState(), Qt::PartiallyChecked);
        QVERIFY(!properties.fields_[5].representable);
        QCOMPARE(properties.rowValue(5), std::optional<int>(5));
        properties.refresh();
        QCOMPARE(level.course().sprites[0].settings, 5U);

        // Once the user decides, the field means what a checkbox means, and
        // undecided is not somewhere they can click back to.
        properties.fields_[5].toggle->click();
        QVERIFY(!properties.fields_[5].toggle->isTristate());
        QCOMPARE(properties.fields_[5].toggle->checkState(), Qt::Checked);
        QCOMPARE(level.course().sprites[0].settings, 1U);
        properties.fields_[5].toggle->click();
        QCOMPARE(properties.fields_[5].toggle->checkState(), Qt::Unchecked);
        QCOMPARE(level.course().sprites[0].settings, 0U);
    }

    void aMixedSelectionLeavesEveryEditorUndecided() {
        auto level = make_level();
        QVERIFY(level.add_object({1, 0, 1, 1, 1, 1}));
        QVERIFY(level.add_object({2, 0, 2, 1, 1, 1}));
        LevelPropertiesWidget properties;
        properties.setLevel(&level);
        properties.selectMany(LevelPropertiesWidget::Kind::Object, {0, 1});
        QCOMPARE(properties.rowValue(0), std::nullopt);
        QCOMPARE(properties.fields_[0].value->text(), QString("—"));
        // Width agrees across both, so it is not undecided.
        QCOMPARE(properties.rowValue(4), std::optional<int>(1));

        // Only numeric rows reach this through the panel today, since an actor's
        // own fields are not shown for a selection. The contract holds for all
        // three regardless, so a kind that grows a list later inherits it.
        nsmb::project::SpriteFieldDefinition choice{"pick", "Pick", 0, 2, {{0, "A"}, {1, "B"}}};
        nsmb::project::SpriteFieldDefinition flag{"flag", "Flag", 4, 1, {}, "", 0, 1};
        properties.setRowEditor(6, LevelPropertiesWidget::FieldEditor::Choice, 0, 0, &choice);
        properties.fields_[6].mixed = true;
        properties.setRowValue(6, std::nullopt);
        QCOMPARE(properties.rowValue(6), std::nullopt);
        QCOMPARE(properties.fields_[6].choice->currentText(), QString("—"));

        properties.setRowEditor(7, LevelPropertiesWidget::FieldEditor::Boolean, 0, 1, &flag);
        properties.fields_[7].mixed = true;
        properties.setRowValue(7, std::nullopt);
        QCOMPARE(properties.fields_[7].toggle->checkState(), Qt::PartiallyChecked);
        QCOMPARE(properties.rowValue(7), std::nullopt);
    }

    void theSpritePickerRetypesTheSpriteByName() {
        // 306 actors is too many to address by number, and the panel already
        // knows every one of their names.
        auto level = make_level();
        const auto catalog = actorFor(level, 0U, {});
        nsmb::project::SpriteDefinitionCatalog both = catalog;
        nsmb::project::SpriteDefinition other;
        other.object_id = 43;
        other.name = "Another actor";
        both.definitions.push_back(other);

        LevelPropertiesWidget properties;
        properties.setSpriteDefinitions(both, {{7, 42}, {9, 43}});
        properties.setLevel(&level);
        properties.select(LevelPropertiesWidget::Kind::Sprite, 0);

        auto* picker = properties.findChild<QComboBox*>("SpritePicker");
        QVERIFY(picker);
        QCOMPARE(picker->count(), 2);
        // The list holds the actor's *type*, which is what a sprite carries --
        // not the object id the catalog is keyed by.
        QCOMPARE(picker->currentData().toInt(), 7);

        const auto item = picker->findData(9);
        QVERIFY(item >= 0);
        picker->setCurrentIndex(item);
        emit picker->activated(item);
        QCOMPARE(level.course().sprites[0].id, 9);
        QCOMPARE(properties.findChild<QLabel*>("SpriteName")->text(), QString("Another actor"));

        // An id no actor claims still reads and edits as a number.
        properties.fields_[0].value->setValue(512);
        QCOMPARE(level.course().sprites[0].id, 512);
        QCOMPARE(picker->currentIndex(), -1);
        QCOMPARE(properties.findChild<QLabel*>("SpriteName")->text(), QString("Actor 512"));
    }

    void spriteFieldsStayAtTheirIndexWhateverEditorTheyWear() {
        // The canvas and the write-back both address rows by number, so which
        // editor a row wears must not move it.
        auto level = make_level();
        const auto catalog = actorFor(level, 0U, {{"flag", "Flag", 0, 1, {}, "", 0, 1},
                                                  {"pick", "Pick", 4, 2, {{0, "A"}, {1, "B"}}},
                                                  {"speed", "Speed", 8, 4, {}}});
        LevelPropertiesWidget properties;
        properties.setSpriteDefinitions(catalog, {{7, 42}});
        properties.setLevel(&level);
        properties.select(LevelPropertiesWidget::Kind::Sprite, 0);

        QCOMPARE(properties.fields_[5].editor, LevelPropertiesWidget::FieldEditor::Boolean);
        QCOMPARE(properties.fields_[6].editor, LevelPropertiesWidget::FieldEditor::Choice);
        QCOMPARE(properties.fields_[7].editor, LevelPropertiesWidget::FieldEditor::Number);

        // X and Y are still rows 1 and 2, which is what a canvas drag moves.
        properties.previewInteraction({16, 32}, 0);
        QCOMPARE(properties.rowValue(1), std::optional<int>(11));
        QCOMPARE(properties.rowValue(2), std::optional<int>(12));

        // And each declared row still writes the field it stands for.
        properties.fields_[7].value->setValue(3);
        QCOMPARE(level.course().sprites[0].settings, 3U << 8U);
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
        // Inside the map the 16 px grid is finer than one device pixel at the
        // fitted zoom, so the surface reads as a wash a few units off the flat
        // colour, and it moves with any sub-pixel shift of the grid. The
        // threshold has room for that: what is under test is the theme, not
        // where the lines land.
        QVERIFY(QColor(light.pixel(40, 140)).lightness() > 180);
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

    void unsignedEntitiesSnapBackWhenDraggedPastTheOrigin() {
        auto level = make_level();
        QVERIFY(level.add_object({1, 0, 1, 1, 1, 1}));
        QVERIFY(level.add_sprite({326, 2, 3, {}}));
        LevelPreviewWidget canvas;
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);

        // Within bounds the drag lands where it was dropped.
        canvas.selections_ = {{2, 0, -1}};
        canvas.commitMove({-16, -16});
        QCOMPARE(level.course().sprites[0].x, 1);
        QCOMPARE(level.course().sprites[0].y, 2);

        // Past the origin nothing moves at all -- not even the object sharing
        // the selection, because the drag is a single gesture.
        canvas.selections_ = {{1, 0, -1}, {2, 0, -1}};
        canvas.commitMove({-64, -64});
        QCOMPARE(level.course().sprites[0].x, 1);
        QCOMPARE(level.course().sprites[0].y, 2);
        QCOMPARE(level.objects()[0].x, 1);
        QCOMPARE(level.objects()[0].y, 1);

        // An object on its own is still free to go out there.
        canvas.selections_ = {{1, 0, -1}};
        canvas.commitMove({-64, -64});
        QCOMPARE(level.objects()[0].x, -3);
        QCOMPARE(level.objects()[0].y, -3);

        // Creating an unsigned entity out of bounds is refused for the same
        // reason; an object placed there is clamped to the origin as before.
        for (const int kind : {2, 3, 4, 7}) {
            canvas.setCreateKind(kind);
            canvas.placeEntity({-80, 32});
        }
        QCOMPARE(level.course().sprites.size(), 1U);
        QCOMPARE(level.course().entrances.size(), 0U);
        QCOMPARE(level.course().zones.size(), 0U);
        QCOMPARE(level.course().views.size(), 0U);
        canvas.setCreateKind(1);
        canvas.placeEntity({-80, 32});
        QCOMPARE(level.objects().size(), 2U);
    }

    void entrancesZonesAndViewsHoldTheSameBound() {
        auto level = make_level();
        nsmb::formats::NsmbLevelEntrance entrance;
        entrance.x = 32;
        entrance.y = 48;
        QVERIFY(level.add_entrance(entrance));
        QVERIFY(level.add_zone({64, 80, 256, 192, 0}));
        nsmb::formats::NsmbLevelView view{};
        view.x = 96;
        view.y = 112;
        view.width = 256;
        view.height = 192;
        QVERIFY(level.add_view(view));
        LevelPreviewWidget canvas;
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);

        for (const int kind : {3, 4, 7}) {
            canvas.selections_ = {{kind, 0, -1}};
            canvas.commitMove({-1024, -1024});
        }
        QCOMPARE(level.course().entrances[0].x, 32);
        QCOMPARE(level.course().entrances[0].y, 48);
        QCOMPARE(level.course().zones[0].x, 64);
        QCOMPARE(level.course().zones[0].y, 80);
        QCOMPARE(level.course().views[0].x, 96);
        QCOMPARE(level.course().views[0].y, 112);

        // A near edge dragged past the origin used to pin the origin at zero
        // while the far edge kept going, inflating the rectangle. Now nothing
        // moves and the size is untouched.
        canvas.selections_ = {{4, 0, -1}};
        canvas.commitResize({-1024, -1024, 0, 0});
        QCOMPARE(level.course().zones[0].x, 64);
        QCOMPARE(level.course().zones[0].width, 256);
        QCOMPARE(level.course().zones[0].height, 192);

        // A resize that stays in bounds still lands.
        canvas.commitResize({-64, -80, 0, 0});
        QCOMPARE(level.course().zones[0].x, 0);
        QCOMPARE(level.course().zones[0].y, 0);
        QCOMPARE(level.course().zones[0].width, 320);
        QCOMPARE(level.course().zones[0].height, 272);
    }

    void outOfBoundsQuadrantIsTinted() {
        auto level = make_level();
        QVERIFY(level.add_object({1, 0, 4, 4, 4, 4}));
        LevelPreviewWidget canvas;
        canvas.resize(480, 360);
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);

        QImage image(canvas.size(), QImage::Format_ARGB32);
        image.fill(Qt::transparent);
        canvas.render(&image);

        // Sampled either side of the origin, away from the grid lines and the
        // object itself, so only the tint separates the two.
        const auto transform = canvas.sceneTransform();
        const auto outside = QColor(image.pixel(transform.map(QPointF(-40, -40)).toPoint()));
        const auto inside = QColor(image.pixel(transform.map(QPointF(40, 150)).toPoint()));
        // The tint is translucent red over the map surface, so it shows as a
        // red cast -- the channels either side of it cannot be compared raw.
        QVERIFY(outside.green() < inside.green());
        QVERIFY(outside.red() - outside.green() > inside.red() - inside.green());
    }

    void anObjectInFrontCullsTheOneBehind() {
        // A level is one array of blocks, not a stack of objects: `InitBGDat`
        // rasterises every object into it and a cell keeps whichever object
        // wrote it last. Drawing an image per object let the block behind show
        // through the transparent pixels of the one in front, and left the
        // empty object -- the eraser every tileset has -- drawing nothing.
        auto tileset = std::make_shared<nsmb::project::DecodedTileset>();
        nsmb::formats::RgbaImage blank{16, 16, {}};
        blank.pixels.assign(16 * 16 * 4, 0);
        nsmb::formats::RgbaImage red{16, 16, {}};
        red.pixels.assign(16 * 16 * 4, 0);
        for (std::size_t i = 0; i < 16 * 16; ++i) { red.pixels[i * 4] = 230; red.pixels[i * 4 + 3] = 255; }
        // Block 7, because the game never writes blocks 1 through 6.
        tileset->map16_tiles.assign(7, blank);
        tileset->map16_tiles.push_back(red);
        nsmb::formats::ObjectLayout solid;
        solid.rows.push_back({{7, 0}});
        nsmb::formats::ObjectLayout eraser;
        eraser.rows.push_back({{nsmb::formats::object_layout_empty_tile, 0}});
        tileset->objects = {solid, eraser};

        auto level = make_level();
        QVERIFY(level.add_object({0, 0, 4, 4, 4, 4}));
        QVERIFY(level.add_object({1, 0, 5, 5, 2, 2}));
        LevelPreviewWidget canvas;
        canvas.resize(480, 360);
        canvas.showLevel("Test", "World 1", "Area 1", "course.bin", "course_bgdat.bin", "T", &level);
        canvas.setEditable(true);
        canvas.setZoom(1.0);
        canvas.tilesets_[0] = tileset;

        QImage image(canvas.size(), QImage::Format_ARGB32);
        image.fill(Qt::transparent);
        canvas.render(&image);
        const auto at = [&](double x, double y) {
            return QColor(image.pixel(canvas.sceneTransform().map(QPointF(x, y)).toPoint()));
        };
        // A cell of the object behind that nothing covers, and one the eraser
        // was laid over.
        const auto behind = at(4 * 16 + 8, 4 * 16 + 8);
        const auto erased = at(5 * 16 + 8, 5 * 16 + 8);
        QVERIFY(behind.red() > behind.green() + 60);
        QVERIFY(erased.red() < erased.green() + 30);
    }

    void autosaveSnapshotIsWrittenAndClearedWhenClean() {
        QTemporaryDir directory; QVERIFY(directory.isValid()); const auto project=make_project(directory.path().toStdString());
        MainWindow window; window.setProject(project);
        auto session=window.levelSession("course/A01_1.bin","course/A01_1_bgdat.bin");QVERIFY(session);QVERIFY(session.value()->add_object({4,0,1,1,1,1}));
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
        const QString course = "course/A01_1.bin";
        const QString bgdat = "course/A01_1_bgdat.bin";

        MainWindow first;
        first.setProject(project);
        // Documents are keyed by variant as well as by path, so ask the window
        // rather than rebuilding the key here.
        const auto key = first.documentKey(course, bgdat);
        const auto index = first.findLevelIndex(course, bgdat);
        QVERIFY(index.isValid());
        first.updateSelectedLevel(index);
        QCOMPARE(first.tabs_->count(), 1);
        first.openLevelEditor(index);
        first.openLevelEditor(index);
        QCOMPARE(first.documents_.size(), 1U);
        QCOMPARE(first.tabs_->count(), 2);
        QVERIFY(first.objectTabs_->count() > 0);
        QVERIFY(first.spriteToolbox_->count() > 0);
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

        // The module already owns this level, and its copy is what the last
        // build put in the ROM. Editing it is what has to show up as Pending.
        auto rom = nsmb::project::RomReader::open(root / "big-star-build.nds");
        QVERIFY(rom);
        const auto moduleCourse = nitrofs / "en/course/A01_1.bin";
        const auto moduleBgdat = nitrofs / "en/course/A01_1_bgdat.bin";
        std::filesystem::create_directories(moduleCourse.parent_path());
        const auto romCourse = rom.value().read("course/A01_1.bin").value();
        QVERIFY(nsmb::core::write_file_atomically(moduleCourse, romCourse));
        QVERIFY(nsmb::core::write_file_atomically(moduleBgdat, rom.value().read("course/A01_1_bgdat.bin").value()));

        const auto entries = [&] {
            std::ostringstream out;
            const auto entry = [&](int id, const char* path, const std::filesystem::path& source) {
                out << (id == 200 ? "" : ",") << "{\"id\":" << id << ",\"path\":\"" << path
                    << "\",\"size\":" << std::filesystem::file_size(source)
                    << ",\"action\":\"modified\",\"source\":\""
                    << std::filesystem::relative(source, root).generic_string()
                    << "\",\"module\":\"levels\",\"from-variant\":\"en\"}";
            };
            entry(200, "course/A01_1.bin", moduleCourse);
            entry(201, "course/A01_1_bgdat.bin", moduleBgdat);
            return out.str();
        }();
        std::ofstream(root / "build/generated/files.json")
            << "{\"schema\":\"ncpatcher.files/1\",\"variant\":\"en\",\"count\":2,\"files\":["
            << entries << "]}";
        std::filesystem::create_directories(root / "fake-ncpatcher");
        std::ofstream(root / "fake-ncpatcher/files-plan.json")
            << "{\"schema\":\"ncpatcher.files/1\",\"variant\":\"en\",\"planned\":true,\"count\":2,\"files\":["
            << entries << "]}";
        std::ofstream(root / "fake-ncpatcher/config-dump.json")
            << "{\"schema\":\"ncpatcher.config/1\",\"version\":2,\"variants\":{\"en\":{\"defines\":[]}}}";

        MainWindow window;
        window.patcherPanel_->setExecutable(BSE_FAKE_NCPATCHER);
        window.setProject(project);

        // Nothing edited yet: the plan and the last build agree, and the bytes
        // behind them are the bytes the ROM holds.
        QVERIFY(window.filesystemView_.find("course/A01_1_bgdat.bin"));
        QCOMPARE(window.filesystemView_.find("course/A01_1_bgdat.bin")->status,
                 nsmb::project::FilesystemStatus::Replaced);

        auto session = window.levelSession("course/A01_1.bin", "course/A01_1_bgdat.bin");
        QVERIFY(session);
        QVERIFY(session.value()->add_object({7, 0, 1, 2, 1, 1}));
        QVERIFY(window.saveEditedLevels());
        QVERIFY(std::filesystem::is_regular_file(moduleCourse));
        QVERIFY(std::filesystem::is_regular_file(moduleBgdat));
        // Nothing wrote into the ROM: a build does that, and only a build.
        QCOMPARE(nsmb::project::RomReader::open(root / "big-star-build.nds").value().read("course/A01_1.bin").value(),
                 romCourse);

        // An object lives in the bgdat, so that is the file whose bytes moved.
        // The edit went into the module tree, so what a build would place no
        // longer matches what the ROM holds -- and only for that file. Marking
        // the whole level pending would be telling the user less than is known.
        QCOMPARE(window.filesystemView_.find("course/A01_1_bgdat.bin")->status,
                 nsmb::project::FilesystemStatus::Pending);
        QCOMPARE(window.filesystemView_.find("course/A01_1.bin")->status,
                 nsmb::project::FilesystemStatus::Replaced);
    }

    void withoutAPlanTheDockReportsTheLastBuild() {
        // NCPatcher resolves the module trees; the editor does not. When it
        // cannot be asked -- not installed, or a project it refuses -- the
        // honest answer is the last build's own record, not a guess made from
        // the module directories.
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto root = std::filesystem::path(directory.path().toStdString());
        auto project = make_project(root);
        std::filesystem::create_directories(root / "build/generated");
        std::ofstream(root / "ncpatcher.yaml") << "version: 2\n";
        std::ofstream(root / "build/generated/files.json")
            << "{\"schema\":\"ncpatcher.files/1\",\"variant\":\"en\",\"count\":1,\"files\":["
               "{\"id\":200,\"path\":\"course/A01_1.bin\",\"size\":4,\"action\":\"modified\","
               "\"source\":\"modules/levels/nitrofs/en/course/A01_1.bin\",\"module\":\"levels\"}]}";

        MainWindow window;
        // Staged nothing, so every query exits 3 the way NCPatcher does when it
        // cannot read the project.
        window.patcherPanel_->setExecutable(BSE_FAKE_NCPATCHER);
        window.setProject(project);

        const auto* entry = window.filesystemView_.find("course/A01_1.bin");
        QVERIFY(entry);
        QCOMPARE(entry->status, nsmb::project::FilesystemStatus::Orphaned);
        QCOMPARE(entry->id, std::optional<std::int64_t>(200));
        QVERIFY(!entry->id_is_planned);
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
        dock.setData({}, nullptr, {}, std::move(view), {}, {}, {});
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
        dock.setData({}, nullptr, {}, std::move(view), {}, {}, {});
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

    void archivesExpandLikeFoldersAndListTheirMembersOnDemand() {
        QTemporaryDir files;
        QVERIFY(files.isValid());
        // A container with a member at its root and two in a folder, so the
        // member address nests exactly the way a ROM path does.
        const auto narc = synthetic::make_narc({{"vs.bmg", sampleBmg()},
                                                {"menu/title.bin", std::vector<std::uint8_t>(8, 1)},
                                                {"menu/hd.bin", std::vector<std::uint8_t>(4, 2)}});
        nsmb::project::FilesystemView view;
        auto effective = romFixture(files.path(), view, {{"ARCHIVE/menu.narc", narc}});
        // The one member a build would write. Its provenance is the thing only
        // the plan knows, so it is on the row before anything is expanded.
        nsmb::project::FilesystemEntry planned;
        planned.path = "ARCHIVE/menu.narc!menu/hd.bin";
        planned.member_id = 2;
        planned.status = nsmb::project::FilesystemStatus::Pending;
        planned.module = "message";
        planned.source = std::filesystem::path(files.path().toStdString()) / "modules/message/hd.bin";
        view.entries.push_back(std::move(planned));

        FilesystemDock dock;
        dock.setData({}, &effective, {}, std::move(view), {}, {}, {});
        auto* tree = dock.findChild<QTreeWidget*>("FilesystemTree");
        QVERIFY(tree);
        const auto find = [&](const QString& romPath) -> QTreeWidgetItem* {
            for (QTreeWidgetItemIterator it(tree); *it; ++it)
                if ((*it)->data(0, Qt::UserRole).toString() == romPath) return *it;
            return nullptr;
        };
        const auto childNames = [](const QTreeWidgetItem* parent) {
            QStringList names;
            for (int i = 0; i < parent->childCount(); ++i) names << parent->child(i)->text(0);
            return names;
        };

        // The container is a row of its own, under ARCHIVE, and it holds only
        // what the plan named: nothing has read the archive yet.
        auto* container = find("ARCHIVE/menu.narc");
        QVERIFY(container);
        QCOMPARE(childNames(container), QStringList({"menu"}));
        QCOMPARE(childNames(container->child(0)), QStringList({"hd.bin"}));

        // Opening it fills in the rest, ordered by the ID the container gives
        // each member the way the tree above is ordered by NitroFS file ID --
        // so vs.bmg leads and hd.bin, which the plan put there first, falls in
        // behind its sibling.
        container->setExpanded(true);
        QCOMPARE(childNames(container), QStringList({"vs.bmg", "menu"}));
        QCOMPARE(childNames(container->child(1)), QStringList({"title.bin", "hd.bin"}));
        QVERIFY(find("ARCHIVE/menu.narc!vs.bmg"));
        // Expanding again must not list them twice.
        container->setExpanded(false);
        container->setExpanded(true);
        QCOMPARE(childNames(container), QStringList({"vs.bmg", "menu"}));

        // A member has a file ID like any other file -- assigned by the
        // container's own name table, which is what game code loads it by --
        // so the column shows it rather than a dash. The one the plan named
        // carries the same ID the container gives it, and the tooltip says
        // which table to read it in.
        QCOMPARE(find("ARCHIVE/menu.narc!vs.bmg")->text(4), QString("0"));
        QCOMPARE(find("ARCHIVE/menu.narc!menu/title.bin")->text(4), QString("1"));
        QCOMPARE(find("ARCHIVE/menu.narc!menu/hd.bin")->text(4), QString("2"));
        QVERIFY(find("ARCHIVE/menu.narc!vs.bmg")->toolTip(4).contains("ARCHIVE/menu.narc"));
        // The container keeps the ROM's numbering, which is a different table.
        QVERIFY(container->text(4) != QString("0"));

        auto* open = dock.findChild<QPushButton*>("OpenFile");
        auto* details = dock.findChild<QLabel*>("FilesystemDetails");
        QPushButton* remove = nullptr;
        for (auto* button : dock.findChildren<QPushButton*>()) if (button->text() == "Remove") remove = button;
        QVERIFY(open && details && remove);

        // A member reaches the editors on its own bytes, read through the
        // container -- which is what the dock could never do before.
        tree->setCurrentItem(find("ARCHIVE/menu.narc!vs.bmg"));
        QVERIFY(open->isEnabled());
        QVERIFY(details->text().contains("Opens in: Messages"));
        // The footer names the table the ID belongs to, since the same number
        // outside the archive is a different file.
        QVERIFY2(details->text().contains("File ID: 0 (inside ARCHIVE/menu.narc)"),
                 qPrintable(details->text()));

        // Replace and Extract work on a member; Remove does not, because the
        // archive codec replaces in place and dropping a member would renumber
        // every member after it.
        tree->setCurrentItem(find("ARCHIVE/menu.narc!menu/hd.bin"));
        QVERIFY(!remove->isEnabled());
        for (auto* button : dock.findChildren<QPushButton*>())
            if (button->text() == "Replace" || button->text() == "Extract") QVERIFY(button->isEnabled());
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
            dock.setData({}, nullptr, {}, view, {}, {}, {});
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
        restored.setData({}, nullptr, {}, view, {}, {}, {});
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
        std::vector<std::pair<std::string, std::vector<std::uint8_t>>> contents;
        const auto add = [&](const char* romPath, const QByteArray& header) {
            contents.emplace_back(romPath, std::vector<std::uint8_t>(header.begin(), header.end()));
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

        nsmb::project::FilesystemView view;
        auto effective = romFixture(files.path(), view, contents);

        FilesystemDock dock;
        dock.setData({}, &effective, {}, std::move(view), {}, {}, {});
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
