#include "TilesetEditorDialog.hpp"
#include "GraphicsCanvas.hpp"
#include "ThemeColors.hpp"

#include "nsmb/core/file_io.hpp"

#include <QCheckBox>
#include <QComboBox>
#include <QDialogButtonBox>
#include <QFileDialog>
#include <QFormLayout>
#include <QGridLayout>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMessageBox>
#include <QMouseEvent>
#include <QPainter>
#include <QPushButton>
#include <QScrollArea>
#include <QSignalBlocker>
#include <QSpinBox>
#include <QTabWidget>
#include <QTableWidget>
#include <QVBoxLayout>

#include <algorithm>
#include <charconv>
#include <functional>

namespace {

using namespace nsmb::formats;

QImage image(const RgbaImage& source) {
    QImage output(static_cast<int>(source.width), static_cast<int>(source.height), QImage::Format_RGBA8888);
    for (int y = 0; y < output.height(); ++y)
        std::copy_n(source.pixels.begin() + static_cast<std::ptrdiff_t>(y * source.width * 4U), source.width * 4U, output.scanLine(y));
    return output;
}

RgbaImage image(const QImage& source) {
    const auto rgba = source.convertToFormat(QImage::Format_RGBA8888);
    RgbaImage output{static_cast<std::size_t>(rgba.width()), static_cast<std::size_t>(rgba.height()), {}};
    output.pixels.resize(output.width * output.height * 4U);
    for (int y = 0; y < rgba.height(); ++y)
        std::copy_n(rgba.constScanLine(y), output.width * 4U, output.pixels.begin() + static_cast<std::ptrdiff_t>(y * output.width * 4U));
    return output;
}

QWidget* scroller(GraphicsCanvas*& canvas, QString name, QWidget* parent) {
    auto* scroll = new QScrollArea(parent); scroll->setWidgetResizable(false); scroll->setAlignment(Qt::AlignCenter);
    canvas = new GraphicsCanvas(scroll); canvas->setObjectName(std::move(name)); scroll->setWidget(canvas); return scroll;
}

bool export_png(QWidget* parent, const RgbaImage& value, const QString& title) {
    const auto path = QFileDialog::getSaveFileName(parent, title, {}, "PNG image (*.png)");
    return path.isEmpty() || image(value).save(path, "PNG") || (QMessageBox::warning(parent, title, "Could not write the PNG."), false);
}

std::optional<RgbaImage> import_png(QWidget* parent, const QString& title) {
    const auto path = QFileDialog::getOpenFileName(parent, title, {}, "PNG image (*.png)"); if (path.isEmpty()) return std::nullopt;
    QImage loaded(path); if (loaded.isNull()) { QMessageBox::warning(parent, title, "Could not read the PNG."); return std::nullopt; }
    return image(loaded);
}

} // namespace

struct TilesetEditorDialog::Impl {
    struct MapUi { GraphicsCanvas* canvas{}; QSpinBox* entry{}; QSpinBox* tile{}; QSpinBox* palette{}; QCheckBox* hflip{}; QCheckBox* vflip{}; };

    Impl(TilesetEditorDialog* owner,
         std::array<EditableTileset,3> values,
         EditableTilemap background,
         EditableTilemap foreground,
         nsmb::project::LevelTilesetReferences references,
         std::shared_ptr<const nsmb::project::TileOverlayCatalog> overlays)
        : owner(owner), tilesets(std::move(values)), tilemaps{std::move(background), std::move(foreground)},
          initial_references(references), overlays(std::move(overlays)) {}

    EditableTileset& tileset() { return tilesets[static_cast<std::size_t>(slot->currentIndex())]; }
    std::shared_ptr<const nsmb::project::TileOverlayCatalog> overlays;

    void build() {
        owner->setWindowTitle("Tilesets and backgrounds"); owner->resize(1050, 760);
        auto* root = new QVBoxLayout(owner);
        auto* select = new QHBoxLayout; select->addWidget(new QLabel("Tileset slot", owner)); slot = new QComboBox(owner); slot->setObjectName("TilesetSlot"); slot->addItems({"0 — shared", "1 — level", "2 — sub-unit"}); slot->setCurrentIndex(1); select->addWidget(slot); select->addStretch();
        auto* import_bundle = new QPushButton("Import tileset…", owner); auto* export_bundle = new QPushButton("Export tileset…", owner); select->addWidget(import_bundle); select->addWidget(export_bundle); root->addLayout(select);
        tabs = new QTabWidget(owner); tabs->setObjectName("TilesetTabs"); root->addWidget(tabs, 1);
        buildSlots(); buildGraphics(); buildMap16(); buildObjects(); buildBehaviors(); buildTilemap(0, "Background"); buildTilemap(1, "Foreground");
        auto* buttons = new QDialogButtonBox(QDialogButtonBox::Save | QDialogButtonBox::Cancel, owner); root->addWidget(buttons);
        QObject::connect(buttons, &QDialogButtonBox::accepted, owner, &QDialog::accept); QObject::connect(buttons, &QDialogButtonBox::rejected, owner, &QDialog::reject);
        QObject::connect(slot, &QComboBox::currentIndexChanged, owner, [this] { refreshAll(); });
        QObject::connect(import_bundle, &QPushButton::clicked, owner, [this] { importBundle(); }); QObject::connect(export_bundle, &QPushButton::clicked, owner, [this] { exportBundle(); });
        refreshAll();
    }

    void buildSlots() {
        auto* page = new QWidget(owner); auto* form = new QFormLayout(page);
        for (auto*& value : reference_spins) { value = new QSpinBox(page); value->setRange(0,255); }
        reference_spins[0]->setObjectName("TilesetId"); reference_spins[0]->setValue(initial_references.tileset_id);
        reference_spins[1]->setValue(initial_references.background_graphics_id); reference_spins[2]->setValue(initial_references.background_screen_id); reference_spins[3]->setValue(initial_references.foreground_graphics_id); reference_spins[4]->setValue(initial_references.foreground_screen_id);
        form->addRow("Level tileset", reference_spins[0]); form->addRow("Background graphics", reference_spins[1]); form->addRow("Background screen", reference_spins[2]); form->addRow("Foreground graphics", reference_spins[3]); form->addRow("Foreground screen", reference_spins[4]);
        auto* note = new QLabel("These IDs select the level's graphics slots. Resource edits below apply to the resources currently loaded; changed IDs take effect when the level is saved and reloaded.", page); note->setWordWrap(true); form->addRow(note); tabs->addTab(page, "Slots");
    }

    void buildGraphics() {
        auto* page = new QWidget(owner); auto* layout = new QVBoxLayout(page); layout->addWidget(scroller(graphics_canvas, "TilesetGraphicsCanvas", page), 1);
        auto* buttons = new QHBoxLayout; auto* import_button = new QPushButton("Import PNG…", page); auto* export_button = new QPushButton("Export PNG…", page); buttons->addWidget(import_button); buttons->addWidget(export_button); buttons->addStretch(); layout->addLayout(buttons);
        QObject::connect(import_button, &QPushButton::clicked, owner, [this] { auto value=import_png(owner,"Import tileset graphics"); if(!value)return; auto result=import_tileset_graphics(tileset(),*value); if(!result)QMessageBox::warning(owner,"Import tileset graphics",QString::fromStdString(result.error().message)); else refreshAll(); });
        QObject::connect(export_button, &QPushButton::clicked, owner, [this] { auto value=render_tileset_graphics(tileset()); if(!value)QMessageBox::warning(owner,"Export tileset graphics",QString::fromStdString(value.error().message)); else export_png(owner,value.value(),"Export tileset graphics"); });
        tabs->addTab(page, "Graphics");
    }

    void buildMap16() {
        auto* page = new QWidget(owner); auto* layout = new QVBoxLayout(page); layout->addWidget(scroller(map16_canvas, "Map16Canvas", page), 1);
        auto* form = new QFormLayout; map16_entry = new QSpinBox(page); map16_entry->setObjectName("Map16Entry"); map16_quadrant = new QComboBox(page); map16_quadrant->addItems({"Top left","Top right","Bottom left","Bottom right"}); map16_tile = new QSpinBox(page); map16_tile->setObjectName("Map16TileIndex"); map16_tile->setRange(-1023,1023); map16_palette = new QSpinBox(page); map16_palette->setRange(-15,15); map16_hflip = new QCheckBox(page); map16_vflip = new QCheckBox(page);
        composed = new QCheckBox("Show the other tilesets’ graphics", page); composed->setObjectName("Map16Composed"); composed->setChecked(true);
        composed->setToolTip("A Map16 entry can name a tile another tileset supplies. Checked, the sheet draws those against all three tilesets, the way the game loads them; unchecked, it shows only what this tileset carries.");
        solid3d = new QCheckBox("Show 3D tiles", page); solid3d->setObjectName("Map16ThreeD"); solid3d->setChecked(true);
        solid3d->setToolTip("The game hands some blocks to the 3D engine and draws nothing from their Map16. Checked, the sheet shows the texture the game shows; unchecked, it shows the bytes the file holds, which is what you want when editing them.");
        form->addRow("Map16 tile",map16_entry); form->addRow("Quadrant",map16_quadrant); form->addRow("8×8 tile",map16_tile); form->addRow("Palette",map16_palette); form->addRow("Horizontal flip",map16_hflip); form->addRow("Vertical flip",map16_vflip); form->addRow(composed); form->addRow(solid3d); layout->addLayout(form);
        auto redraw=[this]{refreshMap16();refreshBehaviors();};
        QObject::connect(composed,&QCheckBox::toggled,owner,redraw);
        QObject::connect(solid3d,&QCheckBox::toggled,owner,redraw);
        map16_canvas->clicked=[this](int x,int y){const auto index=static_cast<int>(y/16)*16+x/16;if(index<static_cast<int>(tileset().map16.size()))map16_entry->setValue(index);};
        QObject::connect(map16_entry,&QSpinBox::valueChanged,owner,[this]{loadMap16Entry();}); QObject::connect(map16_quadrant,&QComboBox::currentIndexChanged,owner,[this]{loadMap16Entry();});
        auto changed=[this]{if(updating||tileset().map16.empty())return;auto& value=tileset().map16[static_cast<std::size_t>(map16_entry->value())].tiles[static_cast<std::size_t>(map16_quadrant->currentIndex())];value={map16_tile->value(),static_cast<std::int16_t>(map16_palette->value()),map16_hflip->isChecked(),map16_vflip->isChecked()};refreshMap16();};
        QObject::connect(map16_tile,&QSpinBox::valueChanged,owner,changed); QObject::connect(map16_palette,&QSpinBox::valueChanged,owner,changed); QObject::connect(map16_hflip,&QCheckBox::toggled,owner,changed); QObject::connect(map16_vflip,&QCheckBox::toggled,owner,changed);
        tabs->addTab(page,"Map16");
    }

    void buildObjects() {
        auto* page=new QWidget(owner);auto* layout=new QVBoxLayout(page);auto* select=new QHBoxLayout;object_index=new QComboBox(page);select->addWidget(new QLabel("Object",page));select->addWidget(object_index,1);auto* add_object=new QPushButton("Add",page);auto* delete_object=new QPushButton("Delete",page);select->addWidget(add_object);select->addWidget(delete_object);layout->addLayout(select);
        object_table=new QTableWidget(page);object_table->setObjectName("TilesetObjectTable");layout->addWidget(object_table,1);auto* actions=new QHBoxLayout;auto* add_row=new QPushButton("Add row",page);auto* add_tile=new QPushButton("Add tile",page);auto* remove_tile=new QPushButton("Remove tile",page);object_control=new QSpinBox(page);object_control->setRange(0,127);actions->addWidget(add_row);actions->addWidget(add_tile);actions->addWidget(remove_tile);actions->addStretch();actions->addWidget(new QLabel("Selected control",page));actions->addWidget(object_control);layout->addLayout(actions);
        QObject::connect(object_index,&QComboBox::currentIndexChanged,owner,[this]{loadObject();}); QObject::connect(object_table,&QTableWidget::itemChanged,owner,[this](QTableWidgetItem* item){if(updating||!item)return;bool ok=false;const auto value=item->text().toInt(&ok);auto& rows=tileset().objects[static_cast<std::size_t>(object_index->currentIndex())].rows;if(item->row()<static_cast<int>(rows.size())&&item->column()<static_cast<int>(rows[item->row()].size()))rows[item->row()][item->column()].tile=ok?value:-1;});
        QObject::connect(object_table,&QTableWidget::currentCellChanged,owner,[this](int row,int column,int,int){updating=true;const auto& rows=tileset().objects[static_cast<std::size_t>(std::max(0,object_index->currentIndex()))].rows;object_control->setValue(row>=0&&row<static_cast<int>(rows.size())&&column>=0&&column<static_cast<int>(rows[row].size())?rows[row][column].control:0);updating=false;});
        QObject::connect(object_control,&QSpinBox::valueChanged,owner,[this](int value){if(updating||object_index->currentIndex()<0)return;const auto row=object_table->currentRow(),column=object_table->currentColumn();auto& rows=tileset().objects[static_cast<std::size_t>(object_index->currentIndex())].rows;if(row>=0&&row<static_cast<int>(rows.size())&&column>=0&&column<static_cast<int>(rows[row].size()))rows[row][column].control=static_cast<std::uint8_t>(value);});
        QObject::connect(add_object,&QPushButton::clicked,owner,[this]{tileset().objects.push_back({{{{-1,0}}}});refreshObjects();object_index->setCurrentIndex(object_index->count()-1);}); QObject::connect(delete_object,&QPushButton::clicked,owner,[this]{if(tileset().objects.size()<=1)return;tileset().objects.erase(tileset().objects.begin()+object_index->currentIndex());refreshObjects();});
        QObject::connect(add_row,&QPushButton::clicked,owner,[this]{if(object_index->currentIndex()<0)return;tileset().objects[static_cast<std::size_t>(object_index->currentIndex())].rows.push_back({{-1,0}});loadObject();});
        QObject::connect(add_tile,&QPushButton::clicked,owner,[this]{if(object_index->currentIndex()<0)return;auto& rows=tileset().objects[static_cast<std::size_t>(object_index->currentIndex())].rows;const auto row=std::clamp(object_table->currentRow(),0,static_cast<int>(rows.size()-1));rows[static_cast<std::size_t>(row)].push_back({-1,0});loadObject();});
        QObject::connect(remove_tile,&QPushButton::clicked,owner,[this]{if(object_index->currentIndex()<0)return;auto& rows=tileset().objects[static_cast<std::size_t>(object_index->currentIndex())].rows;const auto row=object_table->currentRow(),column=object_table->currentColumn();if(row<0||row>=static_cast<int>(rows.size())||column<0||column>=static_cast<int>(rows[row].size()))return;rows[row].erase(rows[row].begin()+column);if(rows[row].empty())rows.erase(rows.begin()+row);if(rows.empty())rows.push_back({{-1,0}});loadObject();});
        tabs->addTab(page,"Objects");
    }

    void buildBehaviors() {
        auto* page=new QWidget(owner);auto* layout=new QHBoxLayout(page);behavior_preview=new GraphicsCanvas(page);behavior_preview->setObjectName("BehaviorMap16Canvas");auto* scroll=new QScrollArea(page);scroll->setWidget(behavior_preview);scroll->setAlignment(Qt::AlignCenter);layout->addWidget(scroll,1);auto* form=new QFormLayout;behavior_entry=new QSpinBox(page);behavior_entry->setObjectName("BehaviorEntry");behavior_flags=new QListWidget(page);behavior_flags->setObjectName("BehaviorFlags");for(const auto name:tile_behavior_flag_names){auto* item=new QListWidgetItem(QString::fromUtf8(name.data(),static_cast<qsizetype>(name.size())),behavior_flags);item->setFlags(item->flags()|Qt::ItemIsUserCheckable);item->setCheckState(Qt::Unchecked);}behavior_subtype=new QComboBox(page);for(const auto name:tile_behavior_subtype_names)behavior_subtype->addItem(QString::fromUtf8(name.data(),static_cast<qsizetype>(name.size())));behavior_parameter=new QSpinBox(page);behavior_parameter->setRange(0,255);behavior_parameter_name=new QLabel(page);behavior_raw=new QLineEdit(page);behavior_raw->setObjectName("BehaviorRaw");form->addRow("Map16 tile",behavior_entry);form->addRow("Flags",behavior_flags);form->addRow("Subtype",behavior_subtype);form->addRow("Parameter",behavior_parameter);form->addRow("Meaning",behavior_parameter_name);form->addRow("Raw hex",behavior_raw);layout->addLayout(form);
        behavior_preview->clicked=[this](int x,int y){const auto index=y/16*16+x/16;if(index<static_cast<int>(tileset().map16.size()))behavior_entry->setValue(index);};QObject::connect(behavior_entry,&QSpinBox::valueChanged,owner,[this]{loadBehavior();});
        auto changed=[this]{saveBehavior();};QObject::connect(behavior_flags,&QListWidget::itemChanged,owner,changed);QObject::connect(behavior_subtype,&QComboBox::currentIndexChanged,owner,changed);QObject::connect(behavior_parameter,&QSpinBox::valueChanged,owner,changed);QObject::connect(behavior_raw,&QLineEdit::editingFinished,owner,[this]{bool ok=false;const auto value=behavior_raw->text().toUInt(&ok,16);if(ok){ensureBehaviors();tileset().behaviors[static_cast<std::size_t>(behavior_entry->value())]=value;loadBehavior();}});
        tabs->addTab(page,"Behaviors");
    }

    void buildTilemap(int which,const QString& title) {
        auto* page=new QWidget(owner);auto* layout=new QVBoxLayout(page);layout->addWidget(scroller(map_ui[which].canvas,title+"Canvas",page),1);auto* form=new QFormLayout;auto& ui=map_ui[which];ui.entry=new QSpinBox(page);ui.entry->setObjectName(title+"Entry");ui.tile=new QSpinBox(page);ui.tile->setObjectName(title+"TileIndex");ui.tile->setRange(0,1023);ui.palette=new QSpinBox(page);ui.palette->setRange(0,15);ui.hflip=new QCheckBox(page);ui.vflip=new QCheckBox(page);form->addRow("Screen entry",ui.entry);form->addRow("Tile",ui.tile);form->addRow("Palette",ui.palette);form->addRow("Horizontal flip",ui.hflip);form->addRow("Vertical flip",ui.vflip);layout->addLayout(form);auto* buttons=new QHBoxLayout;auto* import_button=new QPushButton("Import PNG…",page);auto* export_button=new QPushButton("Export PNG…",page);buttons->addWidget(import_button);buttons->addWidget(export_button);buttons->addStretch();layout->addLayout(buttons);
        ui.canvas->clicked=[this,which](int x,int y){auto& map=tilemaps[which];const auto entry=y/8*static_cast<int>(map.width_tiles)+x/8;if(entry<static_cast<int>(map.screen.size()))map_ui[which].entry->setValue(entry);};QObject::connect(ui.entry,&QSpinBox::valueChanged,owner,[this,which]{loadTilemapEntry(which);});auto changed=[this,which]{saveTilemapEntry(which);};QObject::connect(ui.tile,&QSpinBox::valueChanged,owner,changed);QObject::connect(ui.palette,&QSpinBox::valueChanged,owner,changed);QObject::connect(ui.hflip,&QCheckBox::toggled,owner,changed);QObject::connect(ui.vflip,&QCheckBox::toggled,owner,changed);
        QObject::connect(import_button,&QPushButton::clicked,owner,[this,which,title]{auto value=import_png(owner,"Import "+title);if(!value)return;auto result=import_tilemap_image(tilemaps[which],*value);if(!result)QMessageBox::warning(owner,"Import "+title,QString::fromStdString(result.error().message));else refreshTilemap(which);});QObject::connect(export_button,&QPushButton::clicked,owner,[this,which,title]{auto value=render_tilemap(tilemaps[which]);if(!value)QMessageBox::warning(owner,"Export "+title,QString::fromStdString(value.error().message));else export_png(owner,value.value(),"Export "+title);});
        tabs->addTab(page,title);
    }

    void refreshAll() { refreshGraphics(); refreshMap16(); refreshObjects(); refreshBehaviors(); refreshTilemap(0); refreshTilemap(1); }
    void refreshGraphics(){auto value=render_tileset_graphics(tileset());graphics_canvas->setImage(value?image(value.value()):QImage{}, {8, 16}, 640, 8);}
    // The sheet the Map16 and Behaviour tabs both draw. Composed, an entry naming
    // another tileset's graphics resolves instead of coming out clear.
    nsmb::core::Result<RgbaImage> map16Sheet(){
        auto sheet=(!composed||!composed->isChecked())
                       ?render_map16_sheet(tileset())
                       :render_composed_map16_sheet(tileset(),compose_tileset_memory(tilesets));
        if(!sheet||!overlays||!solid3d||!solid3d->isChecked())return sheet;
        nsmb::project::overlay_map16_sheet(sheet.value(),16,tileset().map16.size(),
                                           initial_references.tileset_id,
                                           static_cast<std::uint16_t>(tileset().tile_page_offset*256U),*overlays);
        return sheet;
    }
    void refreshMap16(){auto value=map16Sheet();map16_canvas->setImage(value?image(value.value()):QImage{}, {16, 16}, 640, 8);map16_entry->setRange(0,std::max(0,static_cast<int>(tileset().map16.size())-1));loadMap16Entry();}
    void loadMap16Entry(){if(tileset().map16.empty())return;updating=true;const auto& value=tileset().map16[static_cast<std::size_t>(map16_entry->value())].tiles[static_cast<std::size_t>(map16_quadrant->currentIndex())];map16_tile->setValue(value.tile);map16_palette->setValue(value.palette);map16_hflip->setChecked(value.horizontal_flip);map16_vflip->setChecked(value.vertical_flip);updating=false;}
    void refreshObjects(){const QSignalBlocker block(object_index);const auto selected=object_index->currentIndex();object_index->clear();for(std::size_t i=0;i<tileset().objects.size();++i)object_index->addItem(QString("Object %1").arg(i));object_index->setCurrentIndex(std::clamp(selected,0,std::max(0,object_index->count()-1)));loadObject();}
    void loadObject(){if(object_index->currentIndex()<0||tileset().objects.empty())return;updating=true;const auto& rows=tileset().objects[static_cast<std::size_t>(object_index->currentIndex())].rows;std::size_t columns=0;for(const auto& row:rows)columns=std::max(columns,row.size());object_table->clear();object_table->setRowCount(static_cast<int>(rows.size()));object_table->setColumnCount(static_cast<int>(columns));for(std::size_t y=0;y<rows.size();++y)for(std::size_t x=0;x<rows[y].size();++x){auto* item=new QTableWidgetItem(rows[y][x].tile<0?QString{}:QString::number(rows[y][x].tile));item->setToolTip(QString("Control 0x%1").arg(rows[y][x].control,2,16,QChar('0')));object_table->setItem(static_cast<int>(y),static_cast<int>(x),item);}updating=false;}
    void ensureBehaviors(){if(tileset().behaviors.empty())tileset().behaviors.resize(tileset().map16.size());}
    void refreshBehaviors(){auto value=map16Sheet();behavior_preview->setImage(value?image(value.value()):QImage{}, {16, 16}, 640, 8);behavior_entry->setRange(0,std::max(0,static_cast<int>(tileset().map16.size())-1));loadBehavior();}
    void loadBehavior(){updating=true;const auto raw=tileset().behaviors.empty()?0U:tileset().behaviors[static_cast<std::size_t>(behavior_entry->value())];const auto parts=decode_tile_behavior(raw);for(int i=0;i<behavior_flags->count();++i)behavior_flags->item(i)->setCheckState(parts.flags&(1U<<i)?Qt::Checked:Qt::Unchecked);behavior_subtype->setCurrentIndex(parts.subtype);behavior_parameter->setValue(parts.parameter);behavior_raw->setText(QString("%1").arg(raw,8,16,QChar('0')));const auto kind=tile_behavior_parameter_kind(parts);const auto name=tile_behavior_parameter_name(kind,parts.parameter);auto meaning=name.empty()?QString("Unknown %1").arg(parts.parameter):QString::fromUtf8(name.data(),static_cast<qsizetype>(name.size()));if(kind==TileBehaviorParameterKind::PipeDoor){const auto main=tile_behavior_pipe_main_names[parts.parameter>>5U];meaning=QString::fromUtf8(main.data(),static_cast<qsizetype>(main.size()))+" — "+meaning;}behavior_parameter_name->setText(meaning);updating=false;}
    void saveBehavior(){if(updating||tileset().map16.empty())return;ensureBehaviors();TileBehaviorParts parts{};for(int i=0;i<behavior_flags->count();++i)if(behavior_flags->item(i)->checkState()==Qt::Checked)parts.flags|=1U<<i;parts.subtype=static_cast<std::uint8_t>(behavior_subtype->currentIndex());parts.parameter=static_cast<std::uint8_t>(behavior_parameter->value());tileset().behaviors[static_cast<std::size_t>(behavior_entry->value())]=encode_tile_behavior(parts);loadBehavior();}
    void refreshTilemap(int which){auto value=render_tilemap(tilemaps[which]);map_ui[which].canvas->setImage(value?image(value.value()):QImage{}, {8, 16}, 640, 8);map_ui[which].entry->setRange(0,std::max(0,static_cast<int>(tilemaps[which].screen.size())-1));loadTilemapEntry(which);}
    void loadTilemapEntry(int which){auto& map=tilemaps[which];if(map.screen.empty())return;updating=true;const auto& value=map.screen[static_cast<std::size_t>(map_ui[which].entry->value())];map_ui[which].tile->setValue(value.tile);map_ui[which].palette->setValue(value.palette);map_ui[which].hflip->setChecked(value.horizontal_flip);map_ui[which].vflip->setChecked(value.vertical_flip);updating=false;}
    void saveTilemapEntry(int which){auto& map=tilemaps[which];if(updating||map.screen.empty())return;auto& value=map.screen[static_cast<std::size_t>(map_ui[which].entry->value())];value={map_ui[which].tile->value(),static_cast<std::int16_t>(map_ui[which].palette->value()),map_ui[which].hflip->isChecked(),map_ui[which].vflip->isChecked()};refreshTilemap(which);}
    void importBundle(){const auto path=QFileDialog::getOpenFileName(owner,"Import tileset",{},"Big Star tileset (*.bstileset)");if(path.isEmpty())return;auto bytes=nsmb::core::read_file(path.toStdString());if(!bytes){QMessageBox::warning(owner,"Import tileset",QString::fromStdString(bytes.error().message));return;}auto data=parse_tileset_bundle(bytes.value());if(!data){QMessageBox::warning(owner,"Import tileset",QString::fromStdString(data.error().message));return;}auto value=parse_tileset(data.value(),tileset().tile_offset,tileset().palette_offset,tileset().tile_page_offset);if(!value){QMessageBox::warning(owner,"Import tileset",QString::fromStdString(value.error().message));return;}tileset()=std::move(value.value());refreshAll();}
    void exportBundle(){auto data=write_tileset(tileset());if(!data){QMessageBox::warning(owner,"Export tileset",QString::fromStdString(data.error().message));return;}auto bytes=write_tileset_bundle(data.value());if(!bytes){QMessageBox::warning(owner,"Export tileset",QString::fromStdString(bytes.error().message));return;}const auto path=QFileDialog::getSaveFileName(owner,"Export tileset",{},"Big Star tileset (*.bstileset)");if(path.isEmpty())return;auto result=nsmb::core::write_file_atomically(path.toStdString(),bytes.value());if(!result)QMessageBox::warning(owner,"Export tileset",QString::fromStdString(result.error().message));}

    TilesetEditorDialog* owner; std::array<EditableTileset,3> tilesets; std::array<EditableTilemap,2> tilemaps; nsmb::project::LevelTilesetReferences initial_references;
    QTabWidget* tabs{};QComboBox* slot{};std::array<QSpinBox*,5> reference_spins{};GraphicsCanvas* graphics_canvas{};GraphicsCanvas* map16_canvas{};QSpinBox* map16_entry{};QComboBox* map16_quadrant{};QSpinBox* map16_tile{};QSpinBox* map16_palette{};QCheckBox* map16_hflip{};QCheckBox* map16_vflip{};QCheckBox* composed{}; QCheckBox* solid3d{};
    QComboBox* object_index{};QTableWidget* object_table{};QSpinBox* object_control{};GraphicsCanvas* behavior_preview{};QSpinBox* behavior_entry{};QListWidget* behavior_flags{};QComboBox* behavior_subtype{};QSpinBox* behavior_parameter{};QLabel* behavior_parameter_name{};QLineEdit* behavior_raw{};std::array<MapUi,2> map_ui{};bool updating{};
};

TilesetEditorDialog::TilesetEditorDialog(std::array<EditableTileset,3> tilesets,EditableTilemap background,EditableTilemap foreground,nsmb::project::LevelTilesetReferences references,std::shared_ptr<const nsmb::project::TileOverlayCatalog> overlays,QWidget* parent)
    :QDialog(parent),impl_(std::make_unique<Impl>(this,std::move(tilesets),std::move(background),std::move(foreground),references,std::move(overlays))){impl_->build();}
TilesetEditorDialog::~TilesetEditorDialog()=default;
nsmb::core::Result<TilesetResourceData> TilesetEditorDialog::tilesetData(std::size_t slot)const{if(slot>=impl_->tilesets.size())return nsmb::core::make_error(nsmb::core::ErrorCode::OutOfBounds,"tileset slot is outside the editor");return write_tileset(impl_->tilesets[slot]);}
nsmb::core::Result<TilemapResourceData> TilesetEditorDialog::backgroundData()const{return write_tilemap(impl_->tilemaps[0]);}
nsmb::core::Result<TilemapResourceData> TilesetEditorDialog::foregroundData()const{return write_tilemap(impl_->tilemaps[1]);}
nsmb::project::LevelTilesetReferences TilesetEditorDialog::references()const{return{static_cast<std::uint16_t>(impl_->reference_spins[0]->value()),static_cast<std::uint16_t>(impl_->reference_spins[1]->value()),static_cast<std::uint16_t>(impl_->reference_spins[2]->value()),static_cast<std::uint16_t>(impl_->reference_spins[3]->value()),static_cast<std::uint16_t>(impl_->reference_spins[4]->value())};}
