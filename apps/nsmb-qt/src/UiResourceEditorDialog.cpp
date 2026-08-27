#include "UiResourceEditorDialog.hpp"

#include "nsmb/formats/graphics.hpp"
#include "nsmb/formats/ui_resources.hpp"

#include <QColorDialog>
#include <QComboBox>
#include <QDialogButtonBox>
#include <QFormLayout>
#include <QHeaderView>
#include <QHBoxLayout>
#include <QLabel>
#include <QListWidget>
#include <QMouseEvent>
#include <QPainter>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QScrollArea>
#include <QTabWidget>
#include <QTableWidget>
#include <QTreeWidget>
#include <QVBoxLayout>

#include <algorithm>
#include <cmath>
#include <functional>
#include <limits>
#include <optional>
#include <tuple>

namespace {

using namespace nsmb::formats;

QColor bgr(std::uint16_t value) {
    const auto expand=[](std::uint16_t channel){return static_cast<int>((channel<<3U)|(channel>>2U));};
    return {expand(value&31U),expand((value>>5U)&31U),expand((value>>10U)&31U)};
}

std::uint16_t bgr(const QColor& value) {
    const auto five=[](int channel){return static_cast<std::uint16_t>((channel*31+127)/255);};
    return static_cast<std::uint16_t>(five(value.red())|(five(value.green())<<5U)|(five(value.blue())<<10U));
}

class IndexedCanvas final : public QWidget {
public:
    explicit IndexedCanvas(QWidget* parent=nullptr):QWidget(parent){setObjectName("UiResourceCanvas");setCursor(Qt::CrossCursor);}
    void setImage(QImage image,bool editable=true){image_=std::move(image);editable_=editable;const auto longest=std::max(image_.width(),image_.height());zoom_=std::clamp(longest?512/longest:1,1,16);setFixedSize(std::max(1,image_.width()*zoom_),std::max(1,image_.height()*zoom_));update();}
    std::function<void(int,int)> clicked;
protected:
    void paintEvent(QPaintEvent*)override{QPainter painter(this);painter.fillRect(rect(),palette().brush(QPalette::Base));painter.setRenderHint(QPainter::SmoothPixmapTransform,false);if(!image_.isNull())painter.drawImage(rect(),image_);if(editable_&&zoom_>=8){painter.setPen(QColor(0,0,0,45));for(int x=0;x<=image_.width();++x)painter.drawLine(x*zoom_,0,x*zoom_,height());for(int y=0;y<=image_.height();++y)painter.drawLine(0,y*zoom_,width(),y*zoom_);}}
    void mousePressEvent(QMouseEvent* event)override{if(!editable_||!clicked||image_.isNull())return;const auto x=static_cast<int>(event->position().x())/zoom_,y=static_cast<int>(event->position().y())/zoom_;if(x>=0&&y>=0&&x<image_.width()&&y<image_.height())clicked(x,y);}
private:QImage image_;int zoom_{1};bool editable_{};
};

class LayoutPreview final : public QWidget {
public:
    explicit LayoutPreview(QWidget* parent=nullptr):QWidget(parent){setMinimumSize(512,384);}
    std::function<std::vector<std::tuple<QRectF,QString,QColor>>()> items;
protected:void paintEvent(QPaintEvent*)override{QPainter p(this);p.fillRect(rect(),palette().brush(QPalette::Base));p.scale(width()/256.0,height()/192.0);if(!items)return;for(const auto& [rect,label,color]:items()){p.setPen(QPen(color,1));p.setBrush(QColor(color.red(),color.green(),color.blue(),55));p.drawRect(rect);p.drawText(rect.adjusted(2,2,-2,-2),Qt::AlignCenter,label);}}
};

QTableWidget* table(QStringList headers,int rows,QWidget* parent){auto* result=new QTableWidget(rows,headers.size(),parent);result->setHorizontalHeaderLabels(headers);result->horizontalHeader()->setSectionResizeMode(QHeaderView::Stretch);result->setSelectionBehavior(QAbstractItemView::SelectRows);return result;}
void cell(QTableWidget* table,int row,int column,qulonglong value){table->setItem(row,column,new QTableWidgetItem(QString::number(value)));}
nsmb::core::Result<qulonglong> number(const QTableWidget* table,int row,int column,qulonglong maximum){bool ok=false;const auto value=table->item(row,column)?table->item(row,column)->text().toULongLong(&ok,0):0;if(!ok||value>maximum)return nsmb::core::make_error(nsmb::core::ErrorCode::InvalidArgument,"table contains an invalid numeric value");return value;}

std::size_t tile_sheet_width(std::size_t tiles){return std::max<std::size_t>(1,static_cast<std::size_t>(std::ceil(std::sqrt(static_cast<double>(tiles)))));}

} // namespace

struct UiResourceEditorDialog::Impl {
    explicit Impl(UiResourceEditorDialog* owner,std::string_view path,std::span<const std::uint8_t> source,bool metadata):owner(owner),path(path),body(source.begin(),source.end()),metadata_mode(metadata){
        if(!metadata_mode&&!body.empty()&&(body[0]==0x10||body[0]==0x11)){compression=body[0];auto decoded=decompress_nitro_lz(body);if(!decoded){error=decoded.error();return;}body=std::move(decoded.value());}
        if(metadata_mode){auto parsed=parse_rom_user_metadata(body);if(!parsed)error=parsed.error();else user=std::move(parsed.value());return;}
        kind=identify_ui_resource(body,path);parse();
    }

    void parse(){
        if(kind==UiResourceKind::Bnbl){auto v=parse_bnbl(body);if(v)bnbl=std::move(v.value());else error=v.error();}
        else if(kind==UiResourceKind::Bncl){auto v=parse_bncl(body);if(v)bncl=std::move(v.value());else error=v.error();}
        else if(kind==UiResourceKind::Bncd){auto v=parse_bncd(body);if(v)bncd=std::move(v.value());else error=v.error();}
        else if(kind==UiResourceKind::Enpg){auto v=parse_enpg(body);if(v)enpg=std::move(v.value());else error=v.error();}
        else if(kind==UiResourceKind::Banner){auto v=parse_ds_banner(body);if(v)banner=std::move(v.value());else error=v.error();}
        else error=nsmb::core::make_error(nsmb::core::ErrorCode::Unsupported,"file is not a supported M13 UI resource");
    }

    void build(){
        owner->setWindowTitle(QString("UI Resource — %1").arg(QString::fromStdString(path)));owner->resize(960,700);auto* root=new QVBoxLayout(owner);
        if(error){root->addWidget(new QLabel(QString::fromStdString(error->message),owner));auto* buttons=new QDialogButtonBox(QDialogButtonBox::Cancel,owner);root->addWidget(buttons);QObject::connect(buttons,&QDialogButtonBox::rejected,owner,&QDialog::reject);return;}
        if(metadata_mode)buildMetadata(root);else if(bnbl)buildBnbl(root);else if(bncl)buildBncl(root);else if(bncd)buildBncd(root);else if(enpg)buildEnpg(root);else if(banner)buildBanner(root);
        if(layout_table)layout_table->setObjectName("BnxlTable");
        if(entry_table)entry_table->setObjectName("BncdEntryTable");
        if(subentry_table)subentry_table->setObjectName("BncdSubentryTable");
        if(metadata_tree)metadata_tree->setObjectName("MetadataTree");
        auto* buttons=new QDialogButtonBox(QDialogButtonBox::Save|QDialogButtonBox::Cancel,owner);root->addWidget(buttons);QObject::connect(buttons,&QDialogButtonBox::accepted,owner,&QDialog::accept);QObject::connect(buttons,&QDialogButtonBox::rejected,owner,&QDialog::reject);
    }

    void addRowButtons(QVBoxLayout* root,QTableWidget* target,std::function<void()> add){auto* row=new QHBoxLayout;auto* plus=new QPushButton("Add",owner);auto* minus=new QPushButton("Delete",owner);row->addWidget(plus);row->addWidget(minus);row->addStretch();root->addLayout(row);QObject::connect(plus,&QPushButton::clicked,owner,[target,add=std::move(add)]{add();target->selectRow(target->rowCount()-1);});QObject::connect(minus,&QPushButton::clicked,owner,[target]{if(target->currentRow()>=0)target->removeRow(target->currentRow());});}

    void buildBnbl(QVBoxLayout* root){root->addWidget(new QLabel("JNBL touch rectangles. Alignment: 0=start, 1=center, 2=end, 3=reserved.",owner));layout_table=table({"X","Y","X align","Y align","Width","Height"},static_cast<int>(bnbl->entries.size()),owner);for(int r=0;r<layout_table->rowCount();++r){const auto& e=bnbl->entries[static_cast<std::size_t>(r)];for(const auto& [c,v]:std::array<std::pair<int,qulonglong>,6>{{{0,e.x},{1,e.y},{2,e.x_alignment},{3,e.y_alignment},{4,e.width},{5,e.height}}})cell(layout_table,r,c,v);}auto* preview=new LayoutPreview(owner);preview->items=[this]{std::vector<std::tuple<QRectF,QString,QColor>> result;for(int r=0;r<layout_table->rowCount();++r){auto x=number(layout_table,r,0,4095),y=number(layout_table,r,1,4095),w=number(layout_table,r,4,255),h=number(layout_table,r,5,255);if(x&&y&&w&&h)result.emplace_back(QRectF(x.value(),y.value(),w.value(),h.value()),QString::number(r),QColor(35,125,220));}return result;};auto* split=new QHBoxLayout;split->addWidget(layout_table,1);split->addWidget(preview,1);root->addLayout(split);QObject::connect(layout_table,&QTableWidget::cellChanged,preview,qOverload<>(&QWidget::update));addRowButtons(root,layout_table,[this]{const auto r=layout_table->rowCount();layout_table->insertRow(r);for(int c=0;c<6;++c)cell(layout_table,r,c,c>=4?16:0);});}

    void buildBncl(QVBoxLayout* root){root->addWidget(new QLabel("JNCL graphic placements. Alignment: 0=start, 1=center, 2=end, 3=reserved.",owner));layout_table=table({"X","Y","X align","Y align","Graphic ID"},static_cast<int>(bncl->entries.size()),owner);for(int r=0;r<layout_table->rowCount();++r){const auto& e=bncl->entries[static_cast<std::size_t>(r)];for(const auto& [c,v]:std::array<std::pair<int,qulonglong>,5>{{{0,e.x},{1,e.y},{2,e.x_alignment},{3,e.y_alignment},{4,e.graphic_id}}})cell(layout_table,r,c,v);}auto* preview=new LayoutPreview(owner);preview->items=[this]{std::vector<std::tuple<QRectF,QString,QColor>> result;for(int r=0;r<layout_table->rowCount();++r){auto x=number(layout_table,r,0,4095),y=number(layout_table,r,1,4095),id=number(layout_table,r,4,255);if(x&&y&&id)result.emplace_back(QRectF(x.value(),y.value(),16,16),QString::number(id.value()),QColor(160,70,190));}return result;};auto* split=new QHBoxLayout;split->addWidget(layout_table,1);split->addWidget(preview,1);root->addLayout(split);QObject::connect(layout_table,&QTableWidget::cellChanged,preview,qOverload<>(&QWidget::update));addRowButtons(root,layout_table,[this]{const auto r=layout_table->rowCount();layout_table->insertRow(r);for(int c=0;c<5;++c)cell(layout_table,r,c,0);});}

    void buildBncd(QVBoxLayout* root){auto* tabs=new QTabWidget(owner);entry_table=table({"Width","Height","First subentry","Subentry count"},static_cast<int>(bncd->entries.size()),tabs);for(int r=0;r<entry_table->rowCount();++r){const auto& e=bncd->entries[static_cast<std::size_t>(r)];cell(entry_table,r,0,e.width);cell(entry_table,r,1,e.height);cell(entry_table,r,2,e.subentry_start);cell(entry_table,r,3,e.subentry_count);}subentry_table=table({"OAM attr 0","OAM attr 1","Unknown","Tile","Tile count"},static_cast<int>(bncd->subentries.size()),tabs);for(int r=0;r<subentry_table->rowCount();++r){const auto& e=bncd->subentries[static_cast<std::size_t>(r)];cell(subentry_table,r,0,e.oam_attr0);cell(subentry_table,r,1,e.oam_attr1);cell(subentry_table,r,2,e.unknown);cell(subentry_table,r,3,e.tile_number);cell(subentry_table,r,4,e.tile_count);}tabs->addTab(entry_table,"Cells");tabs->addTab(subentry_table,"OAM");auto* texture_page=new QWidget(tabs);auto* layout=new QVBoxLayout(texture_page);bpp=new QComboBox(texture_page);bpp->addItems({"4bpp","8bpp"});layout->addWidget(bpp);addCanvas(layout,texture_page);tabs->addTab(texture_page,"Texture");root->addWidget(tabs,1);QObject::connect(bpp,&QComboBox::currentIndexChanged,owner,[this]{refreshIndexed();});canvas->clicked=[this](int x,int y){editTiledPixel(bncd->texture,x,y,bpp->currentIndex()==1);refreshIndexed();};refreshIndexed();}

    void addCanvas(QVBoxLayout* root,QWidget* parent){auto* scroll=new QScrollArea(parent);scroll->setAlignment(Qt::AlignCenter);canvas=new IndexedCanvas(scroll);scroll->setWidget(canvas);root->addWidget(scroll,1);palette_list=new QListWidget(parent);palette_list->setViewMode(QListView::IconMode);palette_list->setFlow(QListView::LeftToRight);palette_list->setWrapping(true);palette_list->setMaximumHeight(120);palette_list->setIconSize({22,22});palette_list->setGridSize({48,42});root->addWidget(palette_list);QObject::connect(palette_list,&QListWidget::currentRowChanged,owner,[this](int row){if(row>=0)selected_colour=static_cast<std::size_t>(row);});QObject::connect(palette_list,&QListWidget::itemDoubleClicked,owner,[this](QListWidgetItem* item){editPalette(item);});}

    void palette(std::span<const std::uint16_t> colours,bool editable){palette_editable=editable;palette_list->clear();for(std::size_t i=0;i<colours.size();++i){auto* item=new QListWidgetItem(QString::number(i),palette_list);QPixmap icon(22,22);icon.fill(bgr(colours[i]));item->setIcon(icon);}if(palette_list->count())palette_list->setCurrentRow(static_cast<int>(std::min(selected_colour,colours.size()-1)));}

    QImage tiled(std::span<const std::uint8_t> data,bool eight,std::span<const std::uint16_t> colours){const std::size_t tile_size=eight?64U:32U;const auto tiles=data.size()/tile_size;const auto width=tile_sheet_width(tiles);const auto height=(tiles+width-1)/width;QImage image(static_cast<int>(width*8),static_cast<int>(height*8),QImage::Format_RGBA8888);image.fill(Qt::transparent);for(std::size_t tile=0;tile<tiles;++tile)for(std::size_t pixel=0;pixel<64;++pixel){const auto index=eight?data[tile*64+pixel]:static_cast<std::uint8_t>((data[tile*32+pixel/2]>>((pixel&1)*4))&15);const auto colour=colours.empty()?QColor(index,index,index):bgr(colours[std::min<std::size_t>(index,colours.size()-1)]);image.setPixelColor(static_cast<int>((tile%width)*8+pixel%8),static_cast<int>((tile/width)*8+pixel/8),colour);}return image;}

    void editTiledPixel(std::vector<std::uint8_t>& data,int x,int y,bool eight){const std::size_t tile_size=eight?64U:32U;const auto tiles=data.size()/tile_size;const auto width=tile_sheet_width(tiles);const auto tile=static_cast<std::size_t>(y/8)*width+static_cast<std::size_t>(x/8);const auto pixel=static_cast<std::size_t>(y%8)*8+static_cast<std::size_t>(x%8);if(tile>=tiles)return;if(eight)data[tile*64+pixel]=static_cast<std::uint8_t>(selected_colour);else{auto& byte=data[tile*32+pixel/2];const auto shift=static_cast<unsigned>((pixel&1)*4);const auto mask=static_cast<std::uint8_t>(15U<<shift);byte=static_cast<std::uint8_t>((byte&~mask)|((selected_colour&15U)<<shift));}}

    void refreshIndexed(){if(bncd){const auto colours=bpp->currentIndex()==1?256U:16U;std::vector<std::uint16_t> gray(colours);for(std::size_t i=0;i<colours;++i){const auto channel=static_cast<std::uint16_t>(i*31/(colours-1));gray[i]=static_cast<std::uint16_t>(channel|(channel<<5)|(channel<<10));}palette(gray,false);canvas->setImage(tiled(bncd->texture,bpp->currentIndex()==1,gray));}else if(enpg){palette(enpg->palette,true);QImage image(256,256,QImage::Format_RGBA8888);for(int y=0;y<256;++y)for(int x=0;x<256;++x)image.setPixelColor(x,y,bgr(enpg->palette[enpg->pixels[static_cast<std::size_t>(y)*256+x]]));canvas->setImage(image);}else if(banner){palette(banner->palette,true);canvas->setImage(tiled(bannerIcon(),false,banner->palette));}}

    std::vector<std::uint8_t> bannerIcon()const{return banner?std::vector<std::uint8_t>(banner->icon.begin(),banner->icon.end()):std::vector<std::uint8_t>{};}

    void buildEnpg(QVBoxLayout* root){root->addWidget(new QLabel("ENPG 256x256 indexed image. Select a palette entry, click pixels to paint, or double-click a colour to edit it.",owner));addCanvas(root,owner);canvas->clicked=[this](int x,int y){enpg->pixels[static_cast<std::size_t>(y)*256+x]=static_cast<std::uint8_t>(selected_colour);refreshIndexed();};refreshIndexed();}

    void buildBanner(QVBoxLayout* root){auto* tabs=new QTabWidget(owner);auto* image_page=new QWidget(tabs);auto* image_layout=new QVBoxLayout(image_page);addCanvas(image_layout,image_page);canvas->clicked=[this](int x,int y){auto data=bannerIcon();editTiledPixel(data,x,y,false);std::copy(data.begin(),data.end(),banner->icon.begin());refreshIndexed();};tabs->addTab(image_page,"Icon");auto* titles_page=new QWidget(tabs);auto* form=new QFormLayout(titles_page);static const std::array names{"Japanese","English","French","German","Italian","Spanish","Chinese","Korean"};for(std::size_t i=0;i<banner->title_count;++i){auto* editor=new QPlainTextEdit(QString::fromStdU16String(banner_title(*banner,i)),titles_page);editor->setObjectName(QString("BannerTitle%1").arg(i));editor->setMaximumHeight(64);title_editors.push_back(editor);form->addRow(names[i],editor);}tabs->addTab(titles_page,"Titles");root->addWidget(tabs,1);refreshIndexed();}

    void editPalette(QListWidgetItem* item){if(!palette_editable||!item)return;const auto index=static_cast<std::size_t>(palette_list->row(item));std::uint16_t before{};if(enpg)before=enpg->palette[index];else if(banner)before=banner->palette[index];else return;const auto chosen=QColorDialog::getColor(bgr(before),owner,"Choose BGR555 colour");if(!chosen.isValid())return;if(enpg)enpg->palette[index]=bgr(chosen);else banner->palette[index]=bgr(chosen);refreshIndexed();}

    void buildMetadata(QVBoxLayout* root){root->addWidget(new QLabel("Per-ROM names and object notes stored in NitroFS file ID 131. Numeric section names are tileset object-note lists.",owner));metadata_tree=new QTreeWidget(owner);metadata_tree->setHeaderLabels({"Section","Key","Value"});metadata_tree->setEditTriggers(QAbstractItemView::DoubleClicked|QAbstractItemView::EditKeyPressed);for(const auto& section:user->sections){auto* top=new QTreeWidgetItem(metadata_tree,{QString::fromStdString(section.name)});top->setFlags(top->flags()|Qt::ItemIsEditable);for(const auto& entry:section.entries){auto* child=new QTreeWidgetItem(top,{{},QString::number(entry.key),QString::fromStdString(entry.value)});child->setFlags(child->flags()|Qt::ItemIsEditable);}}metadata_tree->expandAll();root->addWidget(metadata_tree,1);auto* buttons=new QHBoxLayout;auto* section=new QPushButton("Add section",owner);auto* entry=new QPushButton("Add entry",owner);auto* remove=new QPushButton("Delete",owner);buttons->addWidget(section);buttons->addWidget(entry);buttons->addWidget(remove);buttons->addStretch();root->addLayout(buttons);QObject::connect(section,&QPushButton::clicked,owner,[this]{auto* item=new QTreeWidgetItem(metadata_tree,{"Names"});item->setFlags(item->flags()|Qt::ItemIsEditable);metadata_tree->setCurrentItem(item);});QObject::connect(entry,&QPushButton::clicked,owner,[this]{auto* current=metadata_tree->currentItem();auto* parent=current?(current->parent()?current->parent():current):nullptr;if(!parent)return;auto* item=new QTreeWidgetItem(parent,{{},"0","Name"});item->setFlags(item->flags()|Qt::ItemIsEditable);parent->setExpanded(true);metadata_tree->setCurrentItem(item);});QObject::connect(remove,&QPushButton::clicked,owner,[this]{auto* item=metadata_tree->currentItem();if(!item)return;if(item->parent())delete item->parent()->takeChild(item->parent()->indexOfChild(item));else delete metadata_tree->takeTopLevelItem(metadata_tree->indexOfTopLevelItem(item));});}

    nsmb::core::Result<void> collect(){
        if(bnbl){std::vector<BnblEntry> entries;for(int r=0;r<layout_table->rowCount();++r){std::array<qulonglong,6> v{};for(int c=0;c<6;++c){auto n=number(layout_table,r,c,c<2?4095:c<4?3:255);if(!n)return n.error();v[c]=n.value();}const auto old=static_cast<std::size_t>(r)<bnbl->entries.size()?bnbl->entries[static_cast<std::size_t>(r)]:BnblEntry{};entries.push_back({static_cast<std::uint16_t>(v[0]),static_cast<std::uint16_t>(v[1]),static_cast<std::uint8_t>(v[2]),static_cast<std::uint8_t>(v[3]),old.x_reserved,old.y_reserved,static_cast<std::uint8_t>(v[4]),static_cast<std::uint8_t>(v[5])});}bnbl->entries=std::move(entries);}
        if(bncl){std::vector<BnclEntry> entries;for(int r=0;r<layout_table->rowCount();++r){std::array<qulonglong,5> v{};for(int c=0;c<5;++c){auto n=number(layout_table,r,c,c<2?4095:c<4?3:255);if(!n)return n.error();v[c]=n.value();}const auto old=static_cast<std::size_t>(r)<bncl->entries.size()?bncl->entries[static_cast<std::size_t>(r)]:BnclEntry{};entries.push_back({static_cast<std::uint16_t>(v[0]),static_cast<std::uint16_t>(v[1]),static_cast<std::uint8_t>(v[2]),static_cast<std::uint8_t>(v[3]),old.x_reserved,old.y_reserved,static_cast<std::uint8_t>(v[4]),old.reserved});}bncl->entries=std::move(entries);}
        if(bncd){for(int r=0;r<entry_table->rowCount();++r){std::array<qulonglong,4> v{};for(int c=0;c<4;++c){auto n=number(entry_table,r,c,c<2?255:65535);if(!n)return n.error();v[c]=n.value();}bncd->entries[static_cast<std::size_t>(r)]={static_cast<std::uint8_t>(v[0]),static_cast<std::uint8_t>(v[1]),static_cast<std::uint16_t>(v[2]),static_cast<std::uint16_t>(v[3])};}for(int r=0;r<subentry_table->rowCount();++r){std::array<qulonglong,5> v{};for(int c=0;c<5;++c){auto n=number(subentry_table,r,c,c==2?std::numeric_limits<std::uint32_t>::max():65535);if(!n)return n.error();v[c]=n.value();}bncd->subentries[static_cast<std::size_t>(r)]={static_cast<std::uint16_t>(v[0]),static_cast<std::uint16_t>(v[1]),static_cast<std::uint32_t>(v[2]),static_cast<std::uint16_t>(v[3]),static_cast<std::uint16_t>(v[4])};}}
        if(banner)for(std::size_t i=0;i<title_editors.size();++i){auto result=set_banner_title(*banner,i,title_editors[i]->toPlainText().toStdU16String());if(!result)return result.error();}
        if(user){std::vector<RomUserMetadataSection> sections;for(int s=0;s<metadata_tree->topLevelItemCount();++s){const auto* top=metadata_tree->topLevelItem(s);RomUserMetadataSection section{top->text(0).toStdString(),{}};for(int e=0;e<top->childCount();++e){const auto* child=top->child(e);bool ok=false;const auto key=child->text(1).toInt(&ok);if(!ok||key<0)return nsmb::core::make_error(nsmb::core::ErrorCode::InvalidArgument,"metadata key must be a non-negative integer");section.entries.push_back({key,child->text(2).toStdString()});}sections.push_back(std::move(section));}user->sections=std::move(sections);}
        return {};
    }

    nsmb::core::Result<std::vector<std::uint8_t>> output(){if(error)return *error;if(auto result=collect();!result)return result.error();nsmb::core::Result<std::vector<std::uint8_t>> result=body;if(bnbl)result=write_bnbl(*bnbl);else if(bncl)result=write_bncl(*bncl);else if(bncd)result=write_bncd(*bncd);else if(enpg)result=write_enpg(*enpg);else if(banner)result=write_ds_banner(*banner);else if(user)result=write_rom_user_metadata(*user);if(!result||!compression)return result;return compression==0x10?compress_nitro_lz10(result.value()):compress_nitro_lz11(result.value());}

    UiResourceEditorDialog* owner;std::string path;std::vector<std::uint8_t> body;bool metadata_mode{};std::uint8_t compression{};UiResourceKind kind{UiResourceKind::Unknown};std::optional<nsmb::core::Error> error;std::optional<Bnbl> bnbl;std::optional<Bncl> bncl;std::optional<Bncd> bncd;std::optional<Enpg> enpg;std::optional<DsBanner> banner;std::optional<RomUserMetadata> user;
    QTableWidget* layout_table{};QTableWidget* entry_table{};QTableWidget* subentry_table{};QTreeWidget* metadata_tree{};QComboBox* bpp{};IndexedCanvas* canvas{};QListWidget* palette_list{};std::vector<QPlainTextEdit*> title_editors;std::size_t selected_colour{};bool palette_editable{};
};

UiResourceEditorDialog::UiResourceEditorDialog(std::string_view path,std::span<const std::uint8_t> bytes,bool metadata,QWidget* parent):QDialog(parent),impl_(std::make_unique<Impl>(this,path,bytes,metadata)){impl_->build();}
UiResourceEditorDialog::~UiResourceEditorDialog()=default;

bool UiResourceEditorDialog::supports(std::span<const std::uint8_t> bytes,std::string_view path,bool metadata){Impl probe(nullptr,path,bytes,metadata);return !probe.error;}
nsmb::core::Result<std::vector<std::uint8_t>> UiResourceEditorDialog::editedBytes()const{return impl_->output();}
