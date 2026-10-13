//! The application: which screen is showing and what its menus and buttons do. With a project open it is
//! `MainWindow`: the level list and the picker on the left, the documents in the middle, the properties on the right.

use std::sync::Arc;

use bigstar_client::{Connection, ConnectionState, Waker, WebSocket, websocket_url};
use bigstar_protocol::{DirectoryEntryKind, LevelId, LogLevel};
use egui::{Color32, Key, Modifiers, RichText, Ui, ViewportCommand};

use crate::canvas::{Art, TexturePool};
use crate::files::{FilesDock, OpenRequest};
use crate::picker::{self, PickerState};
use crate::services::Services;
use crate::session::Session;
use crate::tools::{self, Tools};
use crate::workspace::Workspace;
use crate::{level_data, properties_ui};

/// Storage keys, so a shell can seed them.
pub const RECENT_HOSTS_KEY: &str = "recent_hosts";
pub const EMBEDDED_ROOT_KEY: &str = "embedded_root";
const RECENT_LIMIT: usize = 8;
const ERROR_COLOR: Color32 = Color32::from_rgb(210, 70, 60);

#[derive(Default)]
struct ConnectForm {
    host: String,
    token: String,
    root: String,
    recent: Vec<String>,
    error: Option<String>,
    loaded: bool,
}

#[cfg(test)]
mod tests {
    use super::{host_base, join, parent};

    #[test]
    fn a_host_may_be_given_with_or_without_its_socket_path() {
        assert_eq!(host_base("ws://127.0.0.1:7878/ws"), "ws://127.0.0.1:7878");
        assert_eq!(host_base(" ws://example.org:7878/ "), "ws://example.org:7878");
        assert_eq!(host_base("wss://example.org/ws/"), "wss://example.org");
    }

    #[test]
    fn paths_join_and_split_at_the_root() {
        assert_eq!((join("", "a"), join("a/b", "c")), ("a".to_owned(), "a/b/c".to_owned()));
        assert_eq!((parent("a/b"), parent("a"), parent("")), ("a".to_owned(), String::new(), String::new()));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Leave {
    Project,
    Host,
}

/// A question waiting for the person, about edits that would be lost.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Confirm {
    Leave(Leave),
    Close(LevelId),
    Quit,
}

enum Action {
    Undo,
    Redo,
    History { undo: bool, steps: usize },
    Save,
    Leave(Leave),
    OpenProject(String),
    Refresh,
    BeginImport,
    Browse(String),
    ChooseRom { path: String, name: String },
    Import { rom: String, project: String, name: String },
    CancelImport,
}

/// The left panel's tabs.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Left {
    #[default]
    Levels,
    Palette,
    Filesystem,
}

/// The right-hand panel's two tabs.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Side {
    #[default]
    Properties,
    LevelData,
}

pub struct BigStarApp {
    services: Box<dyn Services>,
    form: ConnectForm,
    pub(crate) session: Option<Session>,
    /// What the workspace is remembered under: the host's address, or the embedded host's folder.
    host_key: String,
    confirm: Option<Confirm>,
    /// The person said the window may close with unsaved edits.
    allow_quit: bool,
    show_log: bool,
    actions: Vec<Action>,
    textures: TexturePool,
    pub(crate) picker: PickerState,
    pub(crate) workspace: Workspace,
    pub(crate) tools: Tools,
    pub(crate) files: FilesDock,
    left: Left,
    side: Side,
    /// Level-data value cells being typed in.
    data_drafts: level_data::Drafts,
}

fn waker(ctx: &egui::Context) -> Waker {
    let ctx = ctx.clone();
    Arc::new(move || ctx.request_repaint())
}

/// `ws://host:port/ws` and `ws://host:port` both name a host; the transport wants the second.
fn host_base(address: &str) -> String {
    let trimmed = address.trim().trim_end_matches('/');
    trimmed.strip_suffix("/ws").unwrap_or(trimmed).trim_end_matches('/').to_owned()
}

fn join(directory: &str, name: &str) -> String {
    if directory.is_empty() { name.to_owned() } else { format!("{directory}/{name}") }
}

fn parent(directory: &str) -> String {
    directory.rsplit_once('/').map(|(head, _)| head.to_owned()).unwrap_or_default()
}

impl BigStarApp {
    pub fn new(services: Box<dyn Services>) -> Self {
        Self {
            services,
            form: ConnectForm::default(),
            session: None,
            host_key: String::new(),
            confirm: None,
            allow_quit: false,
            show_log: false,
            actions: Vec::new(),
            textures: TexturePool::default(),
            picker: PickerState::default(),
            workspace: Workspace::default(),
            tools: Tools::default(),
            files: FilesDock::default(),
            left: Left::default(),
            side: Side::default(),
            data_drafts: level_data::Drafts::new(),
        }
    }

    /// Starts on a connection that already exists (screenshots, tests).
    pub fn with_connection(services: Box<dyn Services>, conn: Box<dyn Connection>) -> Self {
        let mut app = Self::new(services);
        app.host_key = "local".to_owned();
        app.session = Some(Session::new(conn));
        app
    }

    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    pub fn session_mut(&mut self) -> Option<&mut Session> {
        self.session.as_mut()
    }

    /// Whether a file is open as a document (screenshots).
    pub fn has_document(&self, path: &str) -> bool {
        self.workspace.files.values().any(|doc| doc.path == path)
    }

    /// Chooses what an open graphics document shows: its image/cell choice and sheet width (screenshots).
    pub fn choose_for_screenshot(&mut self, path: &str, primary: usize, width: usize) {
        if let Some(doc) = self.workspace.files.values_mut().find(|doc| doc.path == path)
            && let crate::workspace::FileView::Graphics(graphics, _) = &mut doc.view
        {
            graphics.set_primary(primary);
            graphics.set_width(width);
        }
    }

    /// Asks for the tileset workspace of an open level, by area (screenshots).
    pub fn open_tilesets_for_screenshot(&mut self, area: &str) {
        if let Some(session) = &mut self.session
            && let Some(level) = session.levels.values().find(|view| view.snapshot.area == area).map(|view| view.snapshot.level)
        {
            session.request_tileset_resources(level);
        }
    }

    /// Brings a tab of every open tileset workspace forward: 0 Slots … 6 Foreground (screenshots).
    pub fn tileset_tab_for_screenshot(&mut self, index: usize) {
        for (doc, _) in self.workspace.tilesets.values_mut() {
            if let Some((tab, _)) = crate::tileset_editor::TABS.get(index) {
                doc.tab = *tab;
            }
        }
    }

    /// Pairs an open graphics document with a palette from anywhere, as its combo box would from its folder
    /// (screenshots).
    pub fn pair_for_screenshot(&mut self, path: &str, palette: &str) {
        if let Some(doc) = self.workspace.files.values_mut().find(|doc| doc.path == path) {
            doc.waiting.push((palette.to_owned(), true));
        }
        if let Some(session) = &mut self.session {
            session.read_file(palette, crate::session::FileRead::Companion);
        }
    }

    fn load_form(&mut self) {
        if self.form.loaded {
            return;
        }
        self.form.loaded = true;
        self.form.recent =
            self.services.load(RECENT_HOSTS_KEY).map(|v| v.lines().filter(|l| !l.is_empty()).map(str::to_owned).collect()).unwrap_or_default();
        self.form.host = self.form.recent.first().cloned().unwrap_or_else(|| self.services.default_host());
        self.form.root = self.services.load(EMBEDDED_ROOT_KEY).unwrap_or_default();
    }

    fn remember_host(&mut self, host: &str) {
        self.form.recent.retain(|h| h != host);
        self.form.recent.insert(0, host.to_owned());
        self.form.recent.truncate(RECENT_LIMIT);
        self.services.store(RECENT_HOSTS_KEY, &self.form.recent.join("\n"));
    }

    /// Connects to a host as the connect screen's button would.
    pub fn connect_to_host(&mut self, ctx: &egui::Context, host: &str, token: &str) {
        self.form.host = host.to_owned();
        self.form.token = token.to_owned();
        self.connect_remote(ctx);
    }

    fn start(&mut self, conn: Box<dyn Connection>, host_key: String) {
        self.form.error = None;
        self.host_key = host_key;
        self.workspace = Workspace::default();
        self.session = Some(Session::new(conn));
    }

    fn connect_remote(&mut self, ctx: &egui::Context) {
        let base = host_base(&self.form.host);
        if base.is_empty() {
            self.form.error = Some("Enter the address of a host.".to_owned());
            return;
        }
        self.remember_host(&base);
        let conn = WebSocket::connect(&websocket_url(&base, &self.form.token), &self.form.token, Some(waker(ctx)));
        self.start(Box::new(conn), base);
    }

    fn connect_embedded(&mut self, ctx: &egui::Context) {
        let root = self.form.root.trim().to_owned();
        if root.is_empty() {
            self.form.error = Some("Choose the folder that holds your projects.".to_owned());
            return;
        }
        match self.services.connect_embedded(&root, waker(ctx)) {
            Ok(conn) => {
                self.services.store(EMBEDDED_ROOT_KEY, &root);
                self.start(conn, format!("embedded:{root}"));
            }
            Err(message) => self.form.error = Some(message),
        }
    }

    /// Per-frame work that is not drawing: take in what the host sent, and follow it.
    pub fn logic(&mut self, ctx: &egui::Context) {
        if let Some(session) = &mut self.session
            && session.poll()
        {
            ctx.request_repaint();
        }
        if let Some(reason) = self.session.as_ref().and_then(|s| s.ended.clone()) {
            self.form.error = Some(format!("Disconnected: {reason}"));
            self.session = None;
        }
        let Some(session) = &mut self.session else { return };
        self.workspace.sync(session);
        self.tools.take_bytes(session, &*self.services);
        self.tools.open_metadata(session);
        if let Some((bytes, target)) = session.banner.take() {
            self.workspace.open_banner(&bytes, &target);
        }
        if let Some(resources) = session.tileset_resources.take() {
            let title = session.levels.get(&resources.level).map_or_else(String::new, |view| view.snapshot.title.clone());
            match crate::tileset_editor::TilesetDoc::open(resources, &title) {
                Ok(doc) => self.workspace.open_tileset(doc),
                Err(error) => session.report(error),
            }
        }
        for request in self.files.follow(session, &*self.services) {
            let reads = match request {
                OpenRequest::Hex { path, bytes } => self.workspace.open_file(&path, &bytes, crate::editors::EditorKind::None, None, false),
                OpenRequest::Editor { path, bytes, kind, metadata } => {
                    // What the file was paired with last time is paired again.
                    let key = crate::graphics_editor::pairing_key(&path);
                    let pairing = self.services.load(&key).and_then(|text| serde_json::from_str(&text).ok());
                    self.workspace.open_file(&path, &bytes, kind, pairing, metadata)
                }
            };
            for companion in reads {
                session.read_file(&companion, crate::session::FileRead::Companion);
            }
        }
        self.workspace.companions(session.take_file_reads(crate::session::FileRead::Companion));
        self.workspace.remember(session, &*self.services, &self.host_key);
        if self.confirm.is_none()
            && let Some(level) = self.workspace.asking_close.take()
        {
            self.confirm = Some(Confirm::Close(level));
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.logic(&ctx);
        if self.session.is_none() {
            self.load_form();
            self.connect_screen(ui, &ctx);
            return;
        }
        self.guard_quit(&ctx);
        self.shortcuts(&ctx);
        self.top_bar(ui);
        self.bottom_bar(ui);
        // The canvas takes keys only when nothing else wants them: no text field, no question waiting.
        let keyboard = !ctx.egui_wants_keyboard_input() && self.confirm.is_none();
        let project_open = self.session.as_ref().is_some_and(|s| s.connected() && s.project.is_some());
        if project_open {
            self.left_panel(ui);
            self.right_panel(ui);
        }
        egui::CentralPanel::default().show(ui, |ui| {
            let Some(session) = &mut self.session else { return };
            if !session.connected() {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.spinner();
                    ui.label("Connecting…");
                });
            } else if session.project.is_some() {
                self.workspace.show(ui, session, &mut self.textures, keyboard, &*self.services);
            } else {
                self.workspace_screen(ui);
            }
        });
        if let Some(session) = &mut self.session {
            self.workspace.show_detached(&ctx, session, &mut self.textures, keyboard);
            self.tools.show(&ctx, session, &mut self.workspace);
            session.request_actor_draws();
        }
        self.import_window(&ctx);
        self.recovery_window(&ctx);
        self.confirm_window(&ctx);
        for action in std::mem::take(&mut self.actions) {
            self.apply(action);
        }
    }

    /// Closing the window with unsaved edits asks first.
    fn guard_quit(&mut self, ctx: &egui::Context) {
        let dirty = self.session.as_ref().is_some_and(Session::dirty);
        if dirty && !self.allow_quit && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.confirm = Some(Confirm::Quit);
        }
    }

    fn connect_screen(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);
                ui.heading("Big Star Editor");
                ui.add_space(20.0);
                let (mut embedded, mut remote) = (false, false);
                if self.services.can_embed() {
                    ui.group(|ui| {
                        ui.set_width(460.0);
                        ui.label(RichText::new("This computer").strong());
                        ui.label("Open a folder of projects with the editor's own host.");
                        ui.horizontal(|ui| {
                            let edit = ui.add(egui::TextEdit::singleline(&mut self.form.root).hint_text("Workspace folder").desired_width(290.0));
                            if ui.button("Browse…").clicked()
                                && let Some(folder) = self.services.pick_folder()
                            {
                                self.form.root = folder;
                            }
                            embedded = ui.button("Open").clicked() || (edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
                        });
                    });
                    ui.add_space(12.0);
                }
                ui.group(|ui| {
                    ui.set_width(460.0);
                    ui.label(RichText::new("A host").strong());
                    ui.label("Connect to a running bigstar-host, on this computer or another.");
                    egui::Grid::new("remote").num_columns(2).show(ui, |ui| {
                        ui.label("Address");
                        ui.add(egui::TextEdit::singleline(&mut self.form.host).desired_width(330.0));
                        ui.end_row();
                        ui.label("Token");
                        let token = ui.add(egui::TextEdit::singleline(&mut self.form.token).password(true).desired_width(330.0));
                        remote = token.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                        ui.end_row();
                    });
                    if !self.form.recent.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Recent:");
                            for host in self.form.recent.clone() {
                                if ui.link(&host).clicked() {
                                    self.form.host = host;
                                }
                            }
                        });
                    }
                    remote |= ui.button("Connect").clicked();
                });
                if let Some(error) = &self.form.error {
                    ui.add_space(10.0);
                    ui.colored_label(ERROR_COLOR, error);
                }
                if embedded {
                    self.connect_embedded(ctx);
                } else if remote {
                    self.connect_remote(ctx);
                }
            });
        });
    }

    fn top_bar(&mut self, ui: &mut Ui) {
        let services = &*self.services;
        let tools = &mut self.tools;
        let workspace = &mut self.workspace;
        let show_log = &mut self.show_log;
        egui::Panel::top("top").show(ui, |ui| {
            let Some(session) = &mut self.session else { return };
            let actions = &mut self.actions;
            ui.horizontal(|ui| {
                match &session.state {
                    ConnectionState::Connected(info) => {
                        ui.label(RichText::new(&info.workspace_name).strong());
                        ui.weak(format!("host {}", info.host_version));
                    }
                    _ => {
                        ui.label("Connecting…");
                    }
                }
                if let Some(project) = &session.project {
                    ui.separator();
                    ui.label(&project.name);
                    if let Some(title) = &project.game_title {
                        ui.weak(title);
                    }
                    ui.separator();
                    ui.menu_button("Tools", |ui| tools::menu(ui, tools, session, services));
                    ui.menu_button("View", |ui| {
                        if ui.button("Preview").clicked() {
                            workspace.show_preview_tab();
                            ui.close();
                        }
                        if ui.selectable_label(*show_log, "Log").clicked() {
                            *show_log = !*show_log;
                            ui.close();
                        }
                    });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Disconnect").clicked() {
                        actions.push(Action::Leave(Leave::Host));
                    }
                    if session.project.is_some() && ui.button("Close project").clicked() {
                        actions.push(Action::Leave(Leave::Project));
                    }
                });
            });
            let idle = !session.busy();
            if let Some(view) = session.level() {
                let snapshot = &view.snapshot;
                ui.horizontal(|ui| {
                    let undo = snapshot.undo_names.first().map_or("Undo".to_owned(), |n| format!("Undo {n}"));
                    let redo = snapshot.redo_names.first().map_or("Redo".to_owned(), |n| format!("Redo {n}"));
                    if ui.add_enabled(idle && snapshot.can_undo, egui::Button::new(undo)).clicked() {
                        actions.push(Action::Undo);
                    }
                    // Every step back, named; picking one undoes it and everything after it.
                    ui.add_enabled_ui(idle && snapshot.can_undo, |ui| {
                        ui.menu_button("⏷", |ui| {
                            for (index, name) in snapshot.undo_names.iter().enumerate() {
                                if ui.button(name).clicked() {
                                    actions.push(Action::History { undo: true, steps: index + 1 });
                                    ui.close();
                                }
                            }
                        })
                        .response
                        .on_hover_text("Undo several steps");
                    });
                    if ui.add_enabled(idle && snapshot.can_redo, egui::Button::new(redo)).clicked() {
                        actions.push(Action::Redo);
                    }
                    ui.add_enabled_ui(idle && snapshot.can_redo, |ui| {
                        ui.menu_button("⏷", |ui| {
                            for (index, name) in snapshot.redo_names.iter().enumerate() {
                                if ui.button(name).clicked() {
                                    actions.push(Action::History { undo: false, steps: index + 1 });
                                    ui.close();
                                }
                            }
                        })
                        .response
                        .on_hover_text("Redo several steps");
                    });
                    if ui.add_enabled(idle && snapshot.dirty, egui::Button::new("Save")).clicked() {
                        actions.push(Action::Save);
                    }
                    ui.separator();
                    ui.weak(format!(
                        "{} · {} objects · {} sprites · {} entrances · revision {}",
                        snapshot.title,
                        snapshot.objects.len(),
                        snapshot.course.sprites.len(),
                        snapshot.course.entrances.len(),
                        snapshot.revision
                    ));
                    if let Some(error) = &snapshot.glue_error {
                        ui.colored_label(Color32::from_rgb(210, 140, 40), format!("Custom actor data unreadable: {error}"));
                    }
                });
            }
        });
    }

    fn bottom_bar(&mut self, ui: &mut Ui) {
        let Some(session) = &mut self.session else { return };
        if self.show_log {
            egui::Panel::bottom("log").resizable(true).default_size(120.0).show(ui, |ui| {
                egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                    for line in &session.log {
                        let color = match line.level {
                            LogLevel::Info => ui.visuals().text_color(),
                            LogLevel::Warning => Color32::from_rgb(210, 140, 40),
                            LogLevel::Error => ERROR_COLOR,
                        };
                        ui.colored_label(color, &line.text);
                    }
                });
            });
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(self.show_log, "Log").clicked() {
                    self.show_log = !self.show_log;
                }
                let mut dismiss = false;
                if let Some(notice) = &session.notice {
                    let color = if notice.error { ERROR_COLOR } else { ui.visuals().text_color() };
                    ui.colored_label(color, &notice.text);
                    dismiss = ui.small_button("×").clicked();
                } else if let Some(last) = session.log.last() {
                    ui.weak(&last.text);
                }
                if dismiss {
                    session.dismiss_notice();
                }
            });
        });
    }

    /// The level list and the picker.
    fn left_panel(&mut self, ui: &mut Ui) {
        let Some(session) = &mut self.session else { return };
        let ctx = ui.ctx().clone();
        egui::Panel::left("left").resizable(true).default_size(280.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.left, Left::Levels, "Levels");
                ui.selectable_value(&mut self.left, Left::Palette, "Palette");
                ui.selectable_value(&mut self.left, Left::Filesystem, "Filesystem");
            });
            ui.separator();
            match self.left {
                Left::Filesystem => self.files.show(ui, session, &*self.services),
                Left::Levels => {
                    if let Some(path) = level_list(ui, session, self.workspace.preview) {
                        // A level row hands the person over to its file.
                        self.left = Left::Filesystem;
                        if !self.files.reveal(session.filesystem.as_ref(), &path) {
                            session.report(format!("{path} is not in this variant's files"));
                        }
                    }
                }
                Left::Palette => match session.active.and_then(|level| session.levels.get_mut(&level)) {
                    Some(view) => {
                        let mut art = Art { ctx: &ctx, store: &session.textures, pool: &mut self.textures };
                        let modules = session.project_data.as_ref().map(|d| &d.modules);
                        picker::show(ui, view, &mut self.picker, modules, &mut art);
                    }
                    None => {
                        ui.weak("Open a level to pick from its tileset objects and actors.");
                    }
                },
            }
        });
    }

    /// The properties and level data of the active document.
    fn right_panel(&mut self, ui: &mut Ui) {
        let Some(session) = &mut self.session else { return };
        let Some(level) = session.active else { return };
        let busy = session.busy();
        let mut edit = None;
        let mut error = None;
        egui::Panel::right("properties").resizable(true).default_size(330.0).show(ui, |ui| {
            let Some(view) = session.levels.get_mut(&level) else { return };
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.side, Side::Properties, "Properties");
                ui.selectable_value(&mut self.side, Side::LevelData, "Level Data");
            });
            ui.separator();
            match self.side {
                Side::Properties => {
                    let overlays = session.project_data.as_ref().and_then(|d| d.object_bank_overlays.as_ref());
                    edit = properties_ui::show(ui, view, overlays, busy);
                }
                Side::LevelData => {
                    let modules = session.project_data.as_ref().map(|d| &d.modules);
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        match level_data::show(ui, &view.snapshot, modules, &mut self.data_drafts, busy) {
                            Some(level_data::Outcome::Edit(made)) => edit = Some(made),
                            Some(level_data::Outcome::Error(message)) => error = Some(message),
                            None => {}
                        }
                    });
                }
            }
        });
        if let Some(edit) = edit.filter(|_| !busy) {
            session.edit_level(level, edit);
        }
        if let Some(error) = error {
            session.report(error);
        }
    }

    fn workspace_screen(&mut self, ui: &mut Ui) {
        let Some(session) = &self.session else { return };
        let actions = &mut self.actions;
        ui.horizontal(|ui| {
            ui.heading("Projects");
            if ui.button("Refresh").clicked() {
                actions.push(Action::Refresh);
            }
            if ui.button("Import ROM…").clicked() {
                actions.push(Action::BeginImport);
            }
        });
        ui.separator();
        if session.projects.is_empty() {
            ui.weak("No projects here yet. Import a ROM that is inside the workspace folder to make one.");
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for project in &session.projects {
                ui.horizontal(|ui| {
                    if ui.button(RichText::new(&project.name).strong()).clicked() {
                        actions.push(Action::OpenProject(project.path.clone()));
                    }
                    ui.weak(&project.path);
                });
            }
        });
    }

    fn import_window(&mut self, ctx: &egui::Context) {
        let Some(session) = &mut self.session else { return };
        let actions = &mut self.actions;
        let Some(browser) = &mut session.browser else { return };
        let mut open = true;
        egui::Window::new("Import ROM").open(&mut open).collapsible(false).default_width(460.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.add_enabled(!browser.path.is_empty(), egui::Button::new("Up")).clicked() {
                    actions.push(Action::Browse(parent(&browser.path)));
                }
                ui.label(if browser.path.is_empty() { "/".to_owned() } else { format!("/{}", browser.path) });
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, true]).show(ui, |ui| match &browser.listing {
                None => {
                    ui.spinner();
                }
                Some(listing) => {
                    for entry in &listing.entries {
                        let full = join(&listing.path, &entry.name);
                        match entry.kind {
                            DirectoryEntryKind::Directory => {
                                if ui.link(format!("{}/", entry.name)).clicked() {
                                    actions.push(Action::Browse(full));
                                }
                            }
                            DirectoryEntryKind::File if entry.name.to_ascii_lowercase().ends_with(".nds") => {
                                let chosen = browser.rom.as_deref() == Some(full.as_str());
                                if ui.selectable_label(chosen, &entry.name).clicked() {
                                    let stem = entry.name.rsplit_once('.').map_or(entry.name.as_str(), |(stem, _)| stem);
                                    actions.push(Action::ChooseRom { path: full, name: stem.to_owned() });
                                }
                            }
                            DirectoryEntryKind::File => {
                                ui.weak(&entry.name);
                            }
                        }
                    }
                }
            });
            if let Some(rom) = browser.rom.clone() {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Project name");
                    ui.text_edit_singleline(&mut browser.project_name);
                });
                let folder = join(&browser.path, browser.project_name.trim());
                ui.weak(format!("Made in /{folder}"));
                let ready = !browser.project_name.trim().is_empty();
                if ui.add_enabled(ready, egui::Button::new("Import")).clicked() {
                    actions.push(Action::Import { rom, project: folder, name: browser.project_name.trim().to_owned() });
                }
            }
        });
        if !open {
            actions.push(Action::CancelImport);
        }
    }

    /// The offer of what an interrupted session left autosaved.
    fn recovery_window(&mut self, ctx: &egui::Context) {
        let Some(session) = &mut self.session else { return };
        let Some(levels) = &session.recovery_offer else { return };
        let mut answer = None;
        egui::Window::new("Recover Autosaved Levels").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(
            ctx,
            |ui| {
                ui.label(format!("Big Star found {} autosaved level(s) from an interrupted or unsaved session. Recover them?", levels.len()));
                for level in levels {
                    ui.weak(format!("• {}", level.title));
                }
                ui.horizontal(|ui| {
                    if ui.button("Recover").clicked() {
                        answer = Some(true);
                    }
                    if ui.button("Discard").clicked() {
                        answer = Some(false);
                    }
                });
            },
        );
        if let Some(accept) = answer {
            session.answer_recovery(accept);
        }
    }

    fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(question) = self.confirm else { return };
        let (text, leave) = match question {
            Confirm::Close(_) => ("This level has changes that were not saved. Closing it drops them.", "Close without saving"),
            Confirm::Quit => ("Some levels have changes that were not saved. Quitting drops them.", "Quit without saving"),
            Confirm::Leave(_) => ("Some levels have changes that were not saved. Leaving drops them.", "Leave without saving"),
        };
        let mut decision = None;
        egui::Window::new("Unsaved changes").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label(text);
            ui.horizontal(|ui| {
                if ui.button(leave).clicked() {
                    decision = Some(true);
                }
                if ui.button("Stay").clicked() {
                    decision = Some(false);
                }
            });
        });
        match decision {
            Some(true) => {
                self.confirm = None;
                match question {
                    Confirm::Leave(target) => self.leave(target),
                    Confirm::Close(level) => {
                        if let Some(session) = &mut self.session {
                            self.workspace.discard_document(session, level);
                        }
                    }
                    Confirm::Quit => {
                        self.allow_quit = true;
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                }
            }
            Some(false) => self.confirm = None,
            None => {}
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.session.as_ref().is_none_or(|s| s.active.is_none()) || self.confirm.is_some() {
            return;
        }
        // Redo first: Cmd+Shift+Z would otherwise also match Cmd+Z.
        let (redo, undo, save) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) || i.consume_key(Modifiers::COMMAND, Key::Y),
                i.consume_key(Modifiers::COMMAND, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::S),
            )
        });
        if undo {
            self.actions.push(Action::Undo);
        }
        if redo {
            self.actions.push(Action::Redo);
        }
        if save {
            self.actions.push(Action::Save);
        }
    }

    fn apply(&mut self, action: Action) {
        let Some(session) = &mut self.session else { return };
        match action {
            Action::Undo => session.undo(),
            Action::Redo => session.redo(),
            Action::History { undo, steps } => session.step_history(undo, steps),
            Action::Save => session.save(),
            Action::Leave(target) => self.request_leave(target),
            Action::OpenProject(path) => session.open_project(&path),
            Action::Refresh => session.list_projects(),
            Action::BeginImport => session.browse(""),
            Action::Browse(path) => {
                if let Some(browser) = &mut session.browser {
                    browser.listing = None;
                }
                session.browse(&path);
            }
            Action::ChooseRom { path, name } => {
                if let Some(browser) = &mut session.browser {
                    browser.rom = Some(path);
                    browser.project_name = name;
                }
            }
            Action::Import { rom, project, name } => session.import_rom(&rom, &project, &name),
            Action::CancelImport => session.browser = None,
        }
    }

    fn request_leave(&mut self, target: Leave) {
        if self.session.as_ref().is_some_and(Session::dirty) {
            self.confirm = Some(Confirm::Leave(target));
        } else {
            self.leave(target);
        }
    }

    fn leave(&mut self, target: Leave) {
        match target {
            Leave::Project => {
                if let Some(session) = &mut self.session {
                    for level in session.levels.keys().copied().collect::<Vec<_>>() {
                        session.close_level(level);
                    }
                    session.close_project();
                }
                self.workspace = Workspace::default();
            }
            Leave::Host => {
                self.session = None;
                self.workspace = Workspace::default();
            }
        }
    }
}

/// A group of the level list: a world, or a step of a world path such as "Mario vs. Luigi > Multicard".
#[derive(Default)]
struct Group<'a> {
    name: String,
    groups: Vec<Group<'a>>,
    levels: Vec<&'a bigstar_protocol::LevelSummary>,
}

/// The catalog's worlds as a tree: a world named as a path is a group holding a group, not a row with an arrow in its
/// label.
fn level_groups(worlds: &[bigstar_protocol::LevelWorld]) -> Group<'_> {
    let mut root = Group::default();
    for world in worlds {
        let mut group = &mut root;
        for part in world.name.split('>').map(str::trim).filter(|part| !part.is_empty()) {
            let position = match group.groups.iter().position(|g| g.name == part) {
                Some(position) => position,
                None => {
                    group.groups.push(Group { name: part.to_owned(), ..Group::default() });
                    group.groups.len() - 1
                }
            };
            let Some(next) = group.groups.get_mut(position) else { break };
            group = next;
        }
        group.levels.extend(&world.levels);
    }
    root
}

/// The project's levels by world. Picking an area shows it in the Preview tab; Edit (or a double-click) opens it as a
/// document. A level of one area is one row. Returns a file to show in the Filesystem dock, when asked for.
fn level_list(ui: &mut Ui, session: &mut Session, previewed: Option<LevelId>) -> Option<String> {
    if let Some(project) = &session.project
        && project.rom_path.is_none()
    {
        ui.colored_label(ERROR_COLOR, "This project has no ROM, so no level can be opened.");
    }
    let shown = previewed.and_then(|level| session.levels.get(&level)).map(|view| view.snapshot.area.clone());
    let root = level_groups(&session.worlds);
    fn area_row(
        ui: &mut Ui,
        label: &str,
        area: &bigstar_protocol::AreaSummary,
        shown: Option<&str>,
        picked: &mut (Option<String>, Option<String>, Option<String>),
    ) {
        ui.horizontal(|ui| {
            ui.add_enabled_ui(area.available, |ui| {
                let tip = format!(
                    "{label}\nToken: {}\nCourse: {}\nBGDat: {}",
                    area.token,
                    if area.course_path.is_empty() { "missing" } else { &area.course_path },
                    if area.bgdat_path.is_empty() { "missing" } else { &area.bgdat_path }
                );
                let response = ui.selectable_label(shown == Some(area.token.as_str()), label).on_hover_text(tip);
                if response.double_clicked() {
                    picked.1 = Some(area.token.clone());
                } else if response.clicked() {
                    picked.0 = Some(area.token.clone());
                }
                response.context_menu(|ui| {
                    if ui.button("Open").clicked() {
                        picked.1 = Some(area.token.clone());
                        ui.close();
                    }
                    for (name, path) in [("Show Course in Filesystem", &area.course_path), ("Show BGDat in Filesystem", &area.bgdat_path)] {
                        if ui.add_enabled(!path.is_empty(), egui::Button::new(name)).clicked() {
                            picked.2 = Some(path.clone());
                            ui.close();
                        }
                    }
                });
                if ui.small_button("Edit").clicked() {
                    picked.1 = Some(area.token.clone());
                }
            });
        });
    }
    fn group_ui(ui: &mut Ui, group: &Group<'_>, path: &str, shown: Option<&str>, picked: &mut (Option<String>, Option<String>, Option<String>)) {
        for child in &group.groups {
            let id = format!("{path}>{}", child.name);
            egui::CollapsingHeader::new(RichText::new(&child.name).strong()).id_salt(&id).default_open(false).show(ui, |ui| {
                group_ui(ui, child, &id, shown, picked);
            });
        }
        for level in &group.levels {
            match level.areas.as_slice() {
                [only] => area_row(ui, &level.display_name, only, shown, picked),
                areas => {
                    egui::CollapsingHeader::new(&level.display_name).id_salt(format!("{path}/{}", level.display_name)).default_open(true).show(
                        ui,
                        |ui| {
                            for area in areas {
                                area_row(ui, &area.display_name, area, shown, picked);
                            }
                        },
                    );
                }
            }
        }
    }
    let mut picked = (None, None, None);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| group_ui(ui, &root, "", shown.as_deref(), &mut picked));
    let (preview, open, reveal) = picked;
    if let Some(area) = open {
        session.open_level(&area);
    } else if let Some(area) = preview {
        session.preview_level(&area);
    }
    reveal
}
