//! The document workspace: a tab well holding the reusable Preview tab and one document per open level, documents
//! moved out to windows of their own, and what is open remembered per host and project. A port of `MainWindow`'s tab
//! and document handling (`openLevelEditor`, `detachEditor`, `saveWorkspaceState`, `restoreWorkspaceState`).

use crate::controls::{self, Button, TextField};
use std::collections::{BTreeMap, BTreeSet, HashMap};

use bigstar_protocol::LevelId;
use egui::{Id, RichText, Ui, Vec2, ViewportBuilder, ViewportCommand, ViewportId, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{DockArea, DockState, TabViewer};
use serde::{Deserialize, Serialize};

use crate::canvas::{self, TexturePool};
use crate::editors::EditorKind;
use crate::graphics_editor::{GraphicsAction, GraphicsDoc, GraphicsView, Pairing, pairing_key};
use crate::level_view::{Item, Kind};
use crate::message_editor::{MessageAction, MessageDoc, MessageView};
use crate::resource_editor::{ResourceDoc, ResourceView};
use crate::services::Services;
use crate::session::FileRead;
use crate::session::Session;
use crate::tileset_editor::{TilesetAction, TilesetDoc, TilesetView};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocTab {
    /// Whatever level is picked in the level list, read-only.
    Preview,
    Level(LevelId),
    /// A file opened from the Filesystem dock, by its key in `Workspace::files`.
    File(u64),
    /// A level's tilesets and backgrounds.
    Tileset(LevelId),
}

/// A file open as a document, in the editor that understands it, or as hex.
pub struct FileDoc {
    pub path: String,
    /// What the editor that understands it is, when one does.
    pub kind: EditorKind,
    pub view: FileView,
    /// Companion files asked for and not arrived: (path, as the palette rather than the graphics).
    pub waiting: Vec<(String, bool)>,
    pub destination: Destination,
}

/// Where a document's edits are saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    /// The module that supplies the file, or the active one.
    Filesystem,
    /// The banner file NCPatcher's configuration names.
    Banner,
}

pub enum FileView {
    Hex {
        text: String,
        /// The text as last read or saved, so a change shows.
        saved: String,
    },
    Graphics(Box<GraphicsDoc>, GraphicsView),
    /// A BMG; `compression` is the LZ wrapper it came in, put back on save.
    Messages {
        doc: Box<MessageDoc>,
        view: MessageView,
        compression: u8,
    },
    /// A uiStudio resource, the banner or the ROM metadata: pages of the graphics editor.
    Resource {
        doc: Box<ResourceDoc>,
        view: ResourceView,
        compression: u8,
    },
}

impl FileView {
    /// What the tab calls the editor.
    pub fn editor(&self) -> &'static str {
        match self {
            FileView::Hex { .. } => "Hex",
            FileView::Graphics(..) => "Graphics",
            FileView::Messages { .. } => "Messages",
            FileView::Resource { doc, .. } => doc.title(),
        }
    }
}

impl FileDoc {
    pub fn dirty(&self) -> bool {
        match &self.view {
            FileView::Hex { text, saved } => text != saved,
            FileView::Graphics(doc, _) => doc.dirty,
            FileView::Messages { doc, .. } => doc.dirty,
            FileView::Resource { doc, .. } => doc.dirty,
        }
    }

    /// Takes a companion file the editor asked for.
    pub fn companion(&mut self, path: &str, bytes: &[u8]) {
        let FileView::Graphics(doc, _) = &mut self.view else { return };
        let (arrived, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.waiting).into_iter().partition(|(wanted, _)| wanted == path);
        self.waiting = waiting;
        for (_, palette) in arrived {
            if palette {
                doc.set_companion_palette(Some(path.to_owned()), Some(bytes));
            } else {
                doc.set_companion_characters(Some(path.to_owned()), Some(bytes));
            }
        }
    }
}

/// What a workspace remembers about a project between runs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    pub documents: Vec<DocMemory>,
    /// The area of the document that was in front.
    pub active: Option<String>,
    pub preview: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocMemory {
    pub area: String,
    pub zoom: f32,
    pub pan: [f32; 2],
    pub detached: bool,
}

pub struct Workspace {
    pub dock: DockState<DocTab>,
    /// The level the Preview tab shows.
    pub preview: Option<LevelId>,
    /// Documents in windows of their own.
    pub detached: BTreeSet<LevelId>,
    /// The detached document whose window has the focus, if one does.
    detached_focus: Option<LevelId>,
    /// The project the memory was last restored for, so it is restored once.
    restored_for: Option<String>,
    cameras: HashMap<String, (f32, [f32; 2])>,
    to_detach: BTreeSet<String>,
    to_activate: Option<String>,
    /// A finder's pick, selected once its level has arrived.
    pub to_select: Option<(String, Kind, usize)>,
    last_saved: Option<String>,
    /// A document whose unsaved edits the person is being asked about before it closes.
    pub asking_close: Option<LevelId>,
    pub files: BTreeMap<u64, FileDoc>,
    next_file: u64,
    /// Tileset workspaces, by the level they belong to.
    pub tilesets: BTreeMap<LevelId, (TilesetDoc, TilesetView)>,
    /// The interface language, for the tabs' own names.
    pub language: crate::i18n::Language,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            dock: DockState::new(vec![DocTab::Preview]),
            preview: None,
            detached: BTreeSet::new(),
            detached_focus: None,
            restored_for: None,
            cameras: HashMap::new(),
            to_detach: BTreeSet::new(),
            to_activate: None,
            to_select: None,
            last_saved: None,
            asking_close: None,
            files: BTreeMap::new(),
            next_file: 1,
            tilesets: BTreeMap::new(),
            language: crate::i18n::Language::default(),
        }
    }
}

/// Where a project's memory is kept: per host and project, since the same path on two hosts is two projects.
pub fn memory_key(host: &str, project: &str) -> String {
    format!("workspace/{host}/{project}")
}

impl Workspace {
    /// Every document, in the tab well or in a window.
    pub fn documents(&self) -> Vec<LevelId> {
        let mut ids: Vec<LevelId> = self
            .dock
            .iter_all_tabs()
            .filter_map(|(_, tab)| match tab {
                DocTab::Level(id) => Some(*id),
                DocTab::Preview | DocTab::File(_) | DocTab::Tileset(_) => None,
            })
            .collect();
        ids.extend(self.detached.iter().copied());
        ids
    }

    pub fn is_document(&self, level: LevelId) -> bool {
        self.detached.contains(&level) || self.dock.find_tab(&DocTab::Level(level)).is_some()
    }

    /// Brings a document forward, adding its tab if it has none.
    pub fn show_document(&mut self, level: LevelId) {
        if self.detached.contains(&level) {
            return;
        }
        let tab = DocTab::Level(level);
        if self.dock.find_tab(&tab).is_none() {
            self.dock.push_to_first_leaf(tab);
        }
        if let Some(path) = self.dock.find_tab(&tab) {
            let _ = self.dock.set_active_tab(path);
            self.dock.set_focused_node_and_surface(path.node_path());
        }
    }

    pub fn show_preview_tab(&mut self) {
        if self.dock.find_tab(&DocTab::Preview).is_none() {
            self.dock.push_to_first_leaf(DocTab::Preview);
        }
        if let Some(path) = self.dock.find_tab(&DocTab::Preview) {
            let _ = self.dock.set_active_tab(path);
        }
    }

    /// Opens a file's bytes as a document, in its editor when it has one and as hex otherwise, or brings forward the one
    /// already open on that path. `pairing` is what the person paired it with last time; `metadata` says the file is
    /// NitroFS file 131, the ROM user metadata. Returns the companion files to read.
    pub fn open_file(&mut self, path: &str, bytes: &[u8], kind: EditorKind, pairing: Option<Pairing>, metadata: bool) -> Vec<String> {
        let existing = self.files.iter().find(|(_, doc)| doc.path == path && doc.destination == Destination::Filesystem).map(|(key, _)| *key);
        let mut reads = Vec::new();
        let key = match existing {
            Some(key) => key,
            None => {
                let mut waiting = Vec::new();
                let (compression, plain) = crate::editors::unwrap_with(bytes);
                let view = match kind {
                    EditorKind::Messages if MessageDoc::supports(&plain) => {
                        FileView::Messages { doc: Box::new(MessageDoc::open(path, &plain)), view: MessageView::default(), compression }
                    }
                    // The uiStudio resources and the ROM metadata are pages of the graphics editor; only when none of
                    // them claims the file does it go down the NNS G2D path.
                    EditorKind::Graphics if ResourceDoc::supports(&plain, path, metadata) => {
                        FileView::Resource { doc: Box::new(ResourceDoc::open(path, &plain, metadata)), view: ResourceView::default(), compression }
                    }
                    EditorKind::Graphics if GraphicsDoc::supports(bytes, path) => {
                        let mut doc = GraphicsDoc::open(path, bytes);
                        if let Some(pairing) = pairing {
                            doc.set_width(pairing.width);
                            waiting.extend(pairing.palette.filter(|_| doc.pairs_palette()).map(|p| (p, true)));
                            waiting.extend(pairing.characters.filter(|_| doc.pairs_characters()).map(|p| (p, false)));
                            reads.extend(waiting.iter().map(|(p, _)| p.clone()));
                        }
                        FileView::Graphics(Box::new(doc), GraphicsView::default())
                    }
                    _ => {
                        let text = crate::editors::hex_text(bytes);
                        FileView::Hex { saved: text.clone(), text }
                    }
                };
                self.insert_file(FileDoc { path: path.to_owned(), kind, view, waiting, destination: Destination::Filesystem })
            }
        };
        self.raise_file(key);
        reads
    }

    /// Opens the ROM banner in the banner page, saved to `target`, or brings forward the one already open.
    pub fn open_banner(&mut self, bytes: &[u8], target: &str) {
        let existing = self.files.iter().find(|(_, doc)| doc.destination == Destination::Banner).map(|(key, _)| *key);
        let key = existing.unwrap_or_else(|| {
            // The name is what tells the banner apart from any other file.
            let mut doc = ResourceDoc::open("banner.bin", bytes, false);
            doc.path = target.to_owned();
            let view = FileView::Resource { doc: Box::new(doc), view: ResourceView::default(), compression: 0 };
            self.insert_file(FileDoc {
                path: target.to_owned(),
                kind: EditorKind::Graphics,
                view,
                waiting: Vec::new(),
                destination: Destination::Banner,
            })
        });
        self.raise_file(key);
    }

    fn insert_file(&mut self, doc: FileDoc) -> u64 {
        let key = self.next_file;
        self.next_file += 1;
        self.files.insert(key, doc);
        key
    }

    fn raise_file(&mut self, key: u64) {
        let tab = DocTab::File(key);
        if self.dock.find_tab(&tab).is_none() {
            self.dock.push_to_first_leaf(tab);
        }
        if let Some(path) = self.dock.find_tab(&tab) {
            let _ = self.dock.set_active_tab(path);
            self.dock.set_focused_node_and_surface(path.node_path());
        }
    }

    /// Opens a level's tileset workspace, or brings forward the one already open; a fresh one replaces nothing that
    /// is open, so unsaved edits there are kept.
    pub fn open_tileset(&mut self, doc: TilesetDoc) {
        let level = doc.level;
        self.tilesets.entry(level).or_insert_with(|| (doc, TilesetView::default()));
        let tab = DocTab::Tileset(level);
        if self.dock.find_tab(&tab).is_none() {
            self.dock.push_to_first_leaf(tab);
        }
        if let Some(path) = self.dock.find_tab(&tab) {
            let _ = self.dock.set_active_tab(path);
            self.dock.set_focused_node_and_surface(path.node_path());
        }
    }

    /// Hands companion files that arrived to the documents waiting for them.
    pub fn companions(&mut self, arrived: Vec<(String, Vec<u8>)>) {
        for (path, bytes) in arrived {
            for doc in self.files.values_mut() {
                doc.companion(&path, &bytes);
            }
        }
    }

    /// Takes a document out of the tab well into a window of its own.
    pub fn detach(&mut self, level: LevelId) {
        if let Some(path) = self.dock.find_tab(&DocTab::Level(level)) {
            self.dock.remove_tab(path);
        }
        self.detached.insert(level);
    }

    /// Puts a detached document back in the tab well.
    pub fn reattach(&mut self, level: LevelId) {
        if self.detached.remove(&level) {
            self.show_document(level);
        }
    }

    fn remove_document(&mut self, level: LevelId) {
        if let Some(path) = self.dock.find_tab(&DocTab::Level(level)) {
            self.dock.remove_tab(path);
        }
        self.detached.remove(&level);
        if self.detached_focus == Some(level) {
            self.detached_focus = None;
        }
    }

    /// Closes a document, asking first when it has unsaved edits.
    pub fn close_document(&mut self, session: &mut Session, level: LevelId) {
        if session.levels.get(&level).is_some_and(|view| view.snapshot.dirty) {
            self.asking_close = Some(level);
        } else {
            self.discard_document(session, level);
        }
    }

    /// Closes a document whatever it holds; the host drops its unsaved edits.
    pub fn discard_document(&mut self, session: &mut Session, level: LevelId) {
        self.remove_document(level);
        // The Preview tab may still be showing the same level.
        if self.preview != Some(level) {
            session.close_level(level);
        }
    }

    /// Follows what arrived from the host: levels opened as documents or into the preview, views the host gave new
    /// ids, autosaves recovered, and remembered cameras for documents that have now arrived. Then decides which
    /// document is active.
    pub fn sync(&mut self, session: &mut Session) {
        for (old, new) in session.take_renamed() {
            for (_, tab) in self.dock.iter_all_tabs_mut() {
                if *tab == DocTab::Level(old) {
                    *tab = DocTab::Level(new);
                }
                if *tab == DocTab::Tileset(old) {
                    *tab = DocTab::Tileset(new);
                }
            }
            if let Some((mut doc, view)) = self.tilesets.remove(&old) {
                doc.level = new;
                self.tilesets.insert(new, (doc, view));
            }
            if self.preview == Some(old) {
                self.preview = Some(new);
            }
            if self.detached.remove(&old) {
                self.detached.insert(new);
            }
        }
        for (level, opening) in session.take_opened() {
            match opening {
                crate::session::Opening::Document => self.show_document(level),
                crate::session::Opening::Preview => {
                    if let Some(old) = self.preview.replace(level)
                        && old != level
                        && !self.is_document(old)
                    {
                        session.close_level(old);
                    }
                    self.show_preview_tab();
                }
                _ => {}
            }
        }
        for area in session.take_recovered() {
            session.open_level(&area);
        }
        // Documents whose levels are gone: the project closed, or the host refused them.
        let levels = &session.levels;
        self.tilesets.retain(|level, _| levels.contains_key(level));
        let files = &self.files;
        let tilesets = &self.tilesets;
        self.dock.retain_tabs(|tab| match tab {
            DocTab::Level(level) => levels.contains_key(level),
            DocTab::File(key) => files.contains_key(key),
            DocTab::Tileset(level) => tilesets.contains_key(level),
            DocTab::Preview => true,
        });
        self.detached.retain(|level| levels.contains_key(level));
        if self.preview.is_some_and(|level| !levels.contains_key(&level)) {
            self.preview = None;
        }
        // What was remembered or picked for levels that have now arrived.
        let documents = self.documents();
        for level in documents {
            let Some(view) = session.levels.get_mut(&level) else { continue };
            let area = view.snapshot.area.clone();
            if let Some((zoom, [x, y])) = self.cameras.remove(&area) {
                view.zoom = zoom;
                view.pan = Vec2::new(x, y);
                view.zoom_pinned = true;
                view.needs_fit = false;
            }
            if let Some((_, kind, index)) = self.to_select.as_ref().filter(|(wanted, _, _)| *wanted == area).cloned() {
                view.selection = vec![Item::new(kind, index)];
                self.to_select = None;
            }
            if self.to_detach.remove(&area) {
                self.detach(level);
            }
            if self.to_activate.as_ref() == Some(&area) {
                self.to_activate = None;
                self.show_document(level);
            }
        }
        session.active = self.active();
    }

    /// The document in front: a focused detached window, else the focused tab when it is a document.
    fn active(&mut self) -> Option<LevelId> {
        if let Some(level) = self.detached_focus.filter(|level| self.detached.contains(level)) {
            return Some(level);
        }
        match self.dock.find_active_focused() {
            Some((_, DocTab::Level(level))) => Some(*level),
            _ => None,
        }
    }

    /// What to remember about the open project.
    pub fn memory(&mut self, session: &Session) -> Memory {
        let active = self.active().and_then(|level| session.levels.get(&level)).map(|view| view.snapshot.area.clone());
        let documents = self
            .documents()
            .into_iter()
            .filter_map(|level| session.levels.get(&level).map(|view| (level, view)))
            .map(|(level, view)| DocMemory {
                area: view.snapshot.area.clone(),
                zoom: view.zoom,
                pan: [view.pan.x, view.pan.y],
                detached: self.detached.contains(&level),
            })
            .collect();
        Memory { documents, active, preview: self.dock.find_tab(&DocTab::Preview).is_some() }
    }

    /// Writes the memory when it changed; restores it once per project, after its levels are listed.
    pub fn remember(&mut self, session: &mut Session, services: &dyn crate::Services, host: &str) {
        let Some(project) = session.project.as_ref().map(|p| p.path.clone()) else {
            self.restored_for = None;
            return;
        };
        let key = memory_key(host, &project);
        if self.restored_for.as_deref() != Some(project.as_str()) {
            if session.worlds.is_empty() {
                return;
            }
            self.restored_for = Some(project);
            self.last_saved = None;
            let memory: Memory =
                services.load(&key).and_then(|text| serde_json::from_str(&text).ok()).unwrap_or(Memory { preview: true, ..Memory::default() });
            self.restore(session, memory);
            return;
        }
        // Remembered levels still on their way are not forgotten in the meantime.
        if !self.cameras.is_empty() {
            return;
        }
        let memory = self.memory(session);
        let Ok(text) = serde_json::to_string(&memory) else { return };
        if self.last_saved.as_ref() != Some(&text) {
            services.store(&key, &text);
            self.last_saved = Some(text);
        }
    }

    fn restore(&mut self, session: &mut Session, memory: Memory) {
        if !memory.preview
            && let Some(path) = self.dock.find_tab(&DocTab::Preview)
        {
            self.dock.remove_tab(path);
        }
        for document in memory.documents {
            self.cameras.insert(document.area.clone(), (document.zoom, document.pan));
            if document.detached {
                self.to_detach.insert(document.area.clone());
            }
            session.open_level(&document.area);
        }
        self.to_activate = memory.active;
    }

    /// The tab well.
    pub fn show(&mut self, ui: &mut Ui, session: &mut Session, pool: &mut TexturePool, keyboard: bool, services: &dyn Services) {
        let mut files = std::mem::take(&mut self.files);
        let mut tilesets = std::mem::take(&mut self.tilesets);
        let mut viewer = Viewer {
            session,
            services,
            preview: self.preview,
            pool,
            keyboard,
            close: Vec::new(),
            detach: Vec::new(),
            files: &mut files,
            closed_files: Vec::new(),
            tilesets: &mut tilesets,
            closed_tilesets: Vec::new(),
            language: self.language,
        };
        DockArea::new(&mut self.dock)
            .style(crate::theme::dock_style(ui))
            .show_close_buttons(true)
            .show_leaf_collapse_buttons(false)
            .show_inside(ui, &mut viewer);
        let Viewer { close, detach, session, closed_files, closed_tilesets, .. } = viewer;
        for key in closed_files {
            files.remove(&key);
        }
        self.files = files;
        for level in closed_tilesets {
            tilesets.remove(&level);
            if let Some(path) = self.dock.find_tab(&DocTab::Tileset(level)) {
                let _ = self.dock.remove_tab(path);
            }
        }
        self.tilesets = tilesets;
        for level in close {
            self.close_document(session, level);
        }
        for level in detach {
            self.detach(level);
        }
    }

    /// Each detached document in a window of its own: a native one on the desktop, a window inside the page in a
    /// browser.
    pub fn show_detached(&mut self, ctx: &egui::Context, session: &mut Session, pool: &mut TexturePool, keyboard: bool) {
        let mut reattach = Vec::new();
        let mut close = Vec::new();
        let mut focused = None;
        for level in self.detached.clone() {
            let Some(title) = session.levels.get(&level).map(|v| v.snapshot.title.clone()) else { continue };
            let builder = ViewportBuilder::default().with_title(format!("{title} - Big Star Editor")).with_inner_size([1000.0, 700.0]);
            ctx.show_viewport_immediate(ViewportId::from_hash_of(("bigstar-level", level)), builder, |ui, _| {
                let ctx = ui.ctx().clone();
                egui::Panel::top(Id::new(("detached-menu", level))).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if Button::new("Move Back to Main Window").show(ui).clicked() {
                            reattach.push(level);
                        }
                        let dirty = session.levels.get(&level).is_some_and(|v| v.snapshot.dirty);
                        ui.label(RichText::new(if dirty { format!("{title} •") } else { title.clone() }).strong());
                    });
                });
                egui::CentralPanel::default().show(ui, |ui| {
                    let mine = keyboard && session.active == Some(level);
                    document(ui, session, pool, level, mine);
                });
                if ctx.input(|i| i.viewport().focused == Some(true)) {
                    focused = Some(level);
                }
                if ctx.input(|i| i.viewport().close_requested()) {
                    ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                    close.push(level);
                }
            });
        }
        if let Some(level) = focused {
            self.detached_focus = Some(level);
        } else if ctx.input(|i| i.viewport().focused == Some(true)) {
            // The main window has the focus back.
            self.detached_focus = None;
        }
        for level in reattach {
            self.reattach(level);
        }
        for level in close {
            self.close_document(session, level);
        }
    }
}

/// One document's canvas, sending what it does to the host.
fn document(ui: &mut Ui, session: &mut Session, pool: &mut TexturePool, level: LevelId, keyboard: bool) {
    let busy = session.busy();
    let Some(view) = session.levels.get_mut(&level) else {
        ui.weak("This level is no longer open.");
        return;
    };
    let out = canvas::show(ui, view, &session.textures, pool, busy, keyboard);
    for edit in out.edits {
        session.edit_level(level, edit);
    }
    if let Some(error) = out.error {
        session.report(error);
    }
}

struct Viewer<'a, 'f> {
    session: &'a mut Session,
    services: &'a dyn Services,
    preview: Option<LevelId>,
    files: &'f mut BTreeMap<u64, FileDoc>,
    closed_files: Vec<u64>,
    tilesets: &'f mut BTreeMap<LevelId, (TilesetDoc, TilesetView)>,
    closed_tilesets: Vec<LevelId>,
    language: crate::i18n::Language,
    pool: &'a mut TexturePool,
    keyboard: bool,
    close: Vec<LevelId>,
    detach: Vec<LevelId>,
}

impl TabViewer for Viewer<'_, '_> {
    type Tab = DocTab;

    fn id(&mut self, tab: &mut DocTab) -> Id {
        Id::new(("bigstar-doc", *tab))
    }

    fn title(&mut self, tab: &mut DocTab) -> WidgetText {
        match tab {
            DocTab::Preview => "Preview".into(),
            DocTab::Level(level) => match self.session.levels.get(level) {
                Some(view) if view.snapshot.dirty => format!("{} •", view.snapshot.title).into(),
                Some(view) => view.snapshot.title.clone().into(),
                None => "…".into(),
            },
            DocTab::File(key) => match self.files.get(key) {
                Some(doc) => {
                    let name = doc.path.rsplit(['/', '!']).next().unwrap_or(&doc.path);
                    let dirty = if doc.dirty() { " •" } else { "" };
                    let editor = if doc.destination == Destination::Banner { "Banner" } else { doc.view.editor() };
                    format!("{editor} — {name}{dirty}").into()
                }
                None => "…".into(),
            },
            DocTab::Tileset(level) => match self.tilesets.get(level) {
                Some((doc, _)) => format!("{} — {}", self.language.tr("Tilesets"), doc.title).into(),
                None => "…".into(),
            },
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut DocTab) {
        match *tab {
            DocTab::Level(level) => {
                let mine = self.keyboard && self.session.active == Some(level);
                document(ui, self.session, self.pool, level, mine);
            }
            DocTab::Preview => preview(ui, self.session, self.pool, self.preview),
            DocTab::File(key) => {
                if let Some(doc) = self.files.get_mut(&key) {
                    file_document(ui, self.session, self.services, doc);
                }
            }
            DocTab::Tileset(level) => {
                let Some((doc, view)) = self.tilesets.get_mut(&level) else { return };
                match crate::tileset_editor::show(ui, doc, view, self.services) {
                    Some(TilesetAction::Save) => {
                        if save_tilesets(self.session, doc) {
                            self.closed_tilesets.push(level);
                        }
                    }
                    Some(TilesetAction::Close) => self.closed_tilesets.push(level),
                    None => {}
                }
            }
        }
    }

    fn on_tab_button(&mut self, _tab: &mut DocTab, response: &egui::Response) {
        // The dock shows a grabbing hand over a tab; a tab is clicked far more often than it is dragged.
        if response.hovered() && response.ctx.dragged_id().is_none() {
            response.ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }

    fn on_close(&mut self, tab: &mut DocTab) -> OnCloseResponse {
        match *tab {
            // The workspace decides, after asking about unsaved edits.
            DocTab::Level(level) => {
                self.close.push(level);
                OnCloseResponse::Ignore
            }
            DocTab::Preview => OnCloseResponse::Close,
            DocTab::File(key) => {
                self.closed_files.push(key);
                OnCloseResponse::Close
            }
            DocTab::Tileset(level) => {
                self.closed_tilesets.push(level);
                OnCloseResponse::Close
            }
        }
    }

    fn context_menu(&mut self, ui: &mut Ui, tab: &mut DocTab, _path: egui_dock::NodePath) {
        if let DocTab::Level(level) = *tab {
            if Button::menu("Move to New Window").show(ui).clicked() {
                self.detach.push(level);
                ui.close();
            }
            if Button::menu("Close").show(ui).clicked() {
                self.close.push(level);
                ui.close();
            }
        }
    }

    fn allowed_in_windows(&self, _tab: &mut DocTab) -> bool {
        false
    }
}

/// Writes the files the tileset workspace changed into their modules, and the level's new references as an edit.
/// False, with the reason reported, when there is nothing it can save.
fn save_tilesets(session: &mut Session, doc: &TilesetDoc) -> bool {
    let writes = match doc.writes() {
        Ok(writes) => writes,
        Err(error) => {
            session.report(error);
            return false;
        }
    };
    let count = writes.len();
    if !writes.is_empty() {
        session.write_files(writes);
    }
    if doc.references_changed() {
        session.edit_level(doc.level, bigstar_protocol::LevelEdit::SetTilesetReferences { references: doc.references });
    }
    session.log.push(crate::session::LogLine {
        level: bigstar_protocol::LogLevel::Info,
        text: format!("Saved {count} edited graphics resource(s) to module targets."),
    });
    true
}

/// The files beside `path`: in its folder, or inside an archive the other members of its folder there.
fn siblings(session: &Session, path: &str) -> Vec<String> {
    let folder = |address: &str| {
        let segments = crate::files::split_address(address);
        segments.len().checked_sub(2).and_then(|i| segments.get(i)).map(|(_, key)| key.clone()).unwrap_or_default()
    };
    let own = folder(path);
    let mut found: Vec<String> = session
        .filesystem
        .as_ref()
        .map(|listing| listing.entries.iter().filter(|e| e.path != path && folder(&e.path) == own).map(|e| e.path.clone()).collect())
        .unwrap_or_default();
    found.sort();
    found
}

/// A file in its editor. Saving writes it into the module that supplies the file, or the active one.
fn file_document(ui: &mut Ui, session: &mut Session, services: &dyn Services, doc: &mut FileDoc) {
    let FileDoc { path, kind, view, waiting, destination } = doc;
    let save = |session: &mut Session, compression: u8, edited: Result<Vec<u8>, String>| {
        match edited.and_then(|bytes| crate::editors::wrap(compression, bytes)) {
            Ok(bytes) if *destination == Destination::Banner => session.write_banner(bytes),
            Ok(bytes) => session.write_file(path, bytes),
            Err(error) => {
                session.report(error);
                return false;
            }
        }
        true
    };
    let (text, saved) = match view {
        FileView::Hex { text, saved } => (text, saved),
        FileView::Messages { doc, view, compression } => {
            for action in crate::message_editor::show(ui, doc, view, services) {
                match action {
                    MessageAction::Save => {
                        if save(session, *compression, doc.output()) {
                            doc.dirty = false;
                        }
                    }
                }
            }
            return;
        }
        FileView::Resource { doc, view, compression } => {
            if crate::resource_editor::show(ui, doc, view) && save(session, *compression, doc.output()) {
                doc.dirty = false;
            }
            return;
        }
        FileView::Graphics(graphics, state) => {
            let siblings = siblings(session, path);
            for action in crate::graphics_editor::show(ui, graphics, state, &siblings) {
                match action {
                    GraphicsAction::Load { path: companion, palette } => {
                        waiting.retain(|(_, p)| *p != palette);
                        waiting.push((companion.clone(), palette));
                        session.read_file(&companion, FileRead::Companion);
                    }
                    GraphicsAction::Remember => {}
                    GraphicsAction::Save => match graphics.output() {
                        Ok(bytes) => {
                            session.write_file(path, bytes);
                            graphics.dirty = false;
                        }
                        Err(error) => session.report(error),
                    },
                }
            }
            // Remembered as soon as it changes, so a sheet is paired and dialled in once.
            if let Ok(text) = serde_json::to_string(&graphics.pairing) {
                let key = pairing_key(path);
                if services.load(&key).as_deref() != Some(text.as_str()) {
                    services.store(&key, &text);
                }
            }
            return;
        }
    };
    let (path, kind) = (&*path, *kind);
    let remark = if kind == EditorKind::None {
        String::new()
    } else {
        format!("· a {} file, shown as its bytes", crate::editors::editor_name(kind).to_lowercase())
    };
    if controls::document_header(ui, path, &remark, text != saved) {
        match crate::editors::hex_bytes(text) {
            Ok(bytes) => {
                session.write_file(path, bytes);
                saved.clone_from(text);
            }
            Err(error) => session.report(error),
        }
    }
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        ui.add(TextField::multiline(text).code().lock_focus());
    });
}

fn preview(ui: &mut Ui, session: &mut Session, pool: &mut TexturePool, shown: Option<LevelId>) {
    let summary = |session: &Session| {
        let areas: usize = session.worlds.iter().flat_map(|w| &w.levels).map(|l| l.areas.len()).sum();
        let available: usize = session.worlds.iter().flat_map(|w| &w.levels).flat_map(|l| &l.areas).filter(|a| a.available).count();
        if session.project.is_none() {
            "Open a project to list and preview levels.".to_owned()
        } else if areas == 0 {
            "No level entries are available for this project.".to_owned()
        } else {
            format!("Select a level. {areas} catalog areas, {available} available in project files.")
        }
    };
    match shown.and_then(|level| session.levels.get_mut(&level)) {
        Some(view) => canvas::show_preview(ui, view, &session.textures, pool),
        None => {
            ui.centered_and_justified(|ui| ui.weak(summary(session)));
        }
    }
}
