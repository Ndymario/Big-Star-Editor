//! The document workspace: a tab well holding the reusable Preview tab and one document per open level, documents
//! moved out to windows of their own, and what is open remembered per host and project. A port of `MainWindow`'s tab
//! and document handling (`openLevelEditor`, `detachEditor`, `saveWorkspaceState`, `restoreWorkspaceState`).

use std::collections::{BTreeSet, HashMap};

use bigstar_protocol::LevelId;
use egui::{Id, RichText, Ui, Vec2, ViewportBuilder, ViewportCommand, ViewportId, WidgetText};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{DockArea, DockState, TabViewer};
use serde::{Deserialize, Serialize};

use crate::canvas::{self, TexturePool};
use crate::level_view::{Item, Kind};
use crate::session::Session;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocTab {
    /// Whatever level is picked in the level list, read-only.
    Preview,
    Level(LevelId),
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
                DocTab::Preview => None,
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
        self.dock.retain_tabs(|tab| match tab {
            DocTab::Level(level) => levels.contains_key(level),
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
    pub fn show(&mut self, ui: &mut Ui, session: &mut Session, pool: &mut TexturePool, keyboard: bool) {
        let mut viewer = Viewer { session, preview: self.preview, pool, keyboard, close: Vec::new(), detach: Vec::new() };
        DockArea::new(&mut self.dock).show_close_buttons(true).show_leaf_collapse_buttons(false).show_inside(ui, &mut viewer);
        let Viewer { close, detach, session, .. } = viewer;
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
                        if ui.button("Move Back to Main Window").clicked() {
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

struct Viewer<'a> {
    session: &'a mut Session,
    preview: Option<LevelId>,
    pool: &'a mut TexturePool,
    keyboard: bool,
    close: Vec<LevelId>,
    detach: Vec<LevelId>,
}

impl TabViewer for Viewer<'_> {
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
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut DocTab) {
        match *tab {
            DocTab::Level(level) => {
                let mine = self.keyboard && self.session.active == Some(level);
                document(ui, self.session, self.pool, level, mine);
            }
            DocTab::Preview => preview(ui, self.session, self.pool, self.preview),
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
        }
    }

    fn context_menu(&mut self, ui: &mut Ui, tab: &mut DocTab, _path: egui_dock::NodePath) {
        if let DocTab::Level(level) = *tab {
            if ui.button("Move to New Window").clicked() {
                self.detach.push(level);
                ui.close();
            }
            if ui.button("Close").clicked() {
                self.close.push(level);
                ui.close();
            }
        }
    }

    fn allowed_in_windows(&self, _tab: &mut DocTab) -> bool {
        false
    }
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
