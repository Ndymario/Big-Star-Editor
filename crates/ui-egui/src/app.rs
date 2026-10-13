//! The application: which screen is showing and what its buttons do.

use std::sync::Arc;

use bigstar_client::{Connection, ConnectionState, Waker, WebSocket, websocket_url};
use bigstar_protocol::{DirectoryEntryKind, LogLevel};
use egui::{Color32, Key, Modifiers, RichText, Ui};

use crate::canvas;
use crate::services::Services;
use crate::session::Session;

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
    Level,
    Project,
    Host,
}

enum Action {
    Undo,
    Redo,
    Save,
    Leave(Leave),
    OpenProject(String),
    OpenArea(String),
    Refresh,
    BeginImport,
    Browse(String),
    ChooseRom { path: String, name: String },
    Import { rom: String, project: String, name: String },
    CancelImport,
}

pub struct BigStarApp {
    services: Box<dyn Services>,
    form: ConnectForm,
    session: Option<Session>,
    /// Waiting for the person to say whether unsaved edits may go.
    confirm_leave: Option<Leave>,
    show_log: bool,
    actions: Vec<Action>,
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
        Self { services, form: ConnectForm::default(), session: None, confirm_leave: None, show_log: false, actions: Vec::new() }
    }

    /// Starts on a connection that already exists (screenshots, tests).
    pub fn with_connection(services: Box<dyn Services>, conn: Box<dyn Connection>) -> Self {
        let mut app = Self::new(services);
        app.session = Some(Session::new(conn));
        app
    }

    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    pub fn session_mut(&mut self) -> Option<&mut Session> {
        self.session.as_mut()
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

    fn connect_remote(&mut self, ctx: &egui::Context) {
        let base = host_base(&self.form.host);
        if base.is_empty() {
            self.form.error = Some("Enter the address of a host.".to_owned());
            return;
        }
        self.remember_host(&base);
        let conn = WebSocket::connect(&websocket_url(&base, &self.form.token), &self.form.token, Some(waker(ctx)));
        self.form.error = None;
        self.session = Some(Session::new(Box::new(conn)));
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
                self.form.error = None;
                self.session = Some(Session::new(conn));
            }
            Err(message) => self.form.error = Some(message),
        }
    }

    /// Per-frame work that is not drawing: take in what the host sent.
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
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.logic(&ctx);
        if self.session.is_none() {
            self.load_form();
            self.connect_screen(ui, &ctx);
            return;
        }
        self.shortcuts(&ctx);
        self.top_bar(ui);
        self.bottom_bar(ui);
        egui::CentralPanel::default().show(ui, |ui| {
            let Some(session) = &mut self.session else { return };
            let busy = session.busy();
            if !session.connected() {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.spinner();
                    ui.label("Connecting…");
                });
            } else if let Some(view) = &mut session.level {
                if let Some(edit) = canvas::show(ui, view, busy) {
                    session.edit(edit);
                }
            } else if session.project.is_some() {
                self.levels_screen(ui);
            } else {
                self.workspace_screen(ui);
            }
        });
        self.import_window(&ctx);
        self.confirm_window(&ctx);
        for action in std::mem::take(&mut self.actions) {
            self.apply(action);
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
        egui::Panel::top("top").show(ui, |ui| {
            let Some(session) = &self.session else { return };
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
                }
                if let Some(view) = &session.level {
                    ui.separator();
                    let dirty = if view.snapshot.dirty { " •" } else { "" };
                    ui.label(RichText::new(format!("{}{dirty}", view.snapshot.title)).strong());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Disconnect").clicked() {
                        actions.push(Action::Leave(Leave::Host));
                    }
                    if session.project.is_some() && ui.button("Close project").clicked() {
                        actions.push(Action::Leave(Leave::Project));
                    }
                    if session.level.is_some() && ui.button("Levels").clicked() {
                        actions.push(Action::Leave(Leave::Level));
                    }
                });
            });
            if let Some(view) = &session.level {
                let snapshot = &view.snapshot;
                let idle = !session.busy();
                ui.horizontal(|ui| {
                    let undo = snapshot.undo_names.first().map_or("Undo".to_owned(), |n| format!("Undo {n}"));
                    let redo = snapshot.redo_names.first().map_or("Redo".to_owned(), |n| format!("Redo {n}"));
                    if ui.add_enabled(idle && snapshot.can_undo, egui::Button::new(undo)).clicked() {
                        actions.push(Action::Undo);
                    }
                    if ui.add_enabled(idle && snapshot.can_redo, egui::Button::new(redo)).clicked() {
                        actions.push(Action::Redo);
                    }
                    if ui.add_enabled(idle && snapshot.dirty, egui::Button::new("Save")).clicked() {
                        actions.push(Action::Save);
                    }
                    ui.separator();
                    ui.weak(format!(
                        "{} objects · {} sprites · {} entrances · revision {}",
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

    fn levels_screen(&mut self, ui: &mut Ui) {
        let Some(session) = &self.session else { return };
        let actions = &mut self.actions;
        if let Some(project) = &session.project {
            if project.rom_path.is_none() {
                ui.colored_label(ERROR_COLOR, "This project has no ROM, so no level can be opened.");
            }
            ui.horizontal(|ui| {
                ui.heading("Levels");
                if let (Some(title), Some(code)) = (&project.game_title, &project.game_code) {
                    ui.weak(format!("{title} ({code}) · {} files", project.file_count));
                }
            });
        }
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for world in &session.worlds {
                egui::CollapsingHeader::new(RichText::new(&world.name).strong()).id_salt(&world.name).default_open(false).show(ui, |ui| {
                    for level in &world.levels {
                        ui.horizontal(|ui| {
                            ui.label(&level.display_name);
                            for area in &level.areas {
                                let text = if level.areas.len() == 1 { "Open".to_owned() } else { area.display_name.clone() };
                                if ui.add_enabled(area.available, egui::Button::new(text)).on_hover_text(&area.token).clicked() {
                                    actions.push(Action::OpenArea(area.token.clone()));
                                }
                            }
                        });
                    }
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

    fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(target) = self.confirm_leave else { return };
        let mut decision = None;
        egui::Window::new("Unsaved changes").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label("This level has changes that were not saved. Leaving drops them.");
            ui.horizontal(|ui| {
                if ui.button("Leave without saving").clicked() {
                    decision = Some(true);
                }
                if ui.button("Stay").clicked() {
                    decision = Some(false);
                }
            });
        });
        match decision {
            Some(true) => {
                self.confirm_leave = None;
                self.leave(target);
            }
            Some(false) => self.confirm_leave = None,
            None => {}
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.session.as_ref().is_none_or(|s| s.level.is_none()) {
            return;
        }
        if self.confirm_leave.is_some() {
            return;
        }
        let (undo, redo, save, delete, escape) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::Z),
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) || i.consume_key(Modifiers::COMMAND, Key::Y),
                i.consume_key(Modifiers::COMMAND, Key::S),
                i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace),
                i.consume_key(Modifiers::NONE, Key::Escape),
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
        if let Some(session) = &mut self.session {
            if delete
                && !session.busy()
                && let Some(edit) = session.level.as_ref().and_then(|v| v.remove_edit())
            {
                if let Some(view) = &mut session.level {
                    view.selection.clear();
                }
                session.edit(edit);
            }
            if escape && let Some(view) = &mut session.level {
                view.selection.clear();
            }
        }
    }

    fn apply(&mut self, action: Action) {
        let Some(session) = &mut self.session else { return };
        match action {
            Action::Undo => session.undo(),
            Action::Redo => session.redo(),
            Action::Save => session.save(),
            Action::Leave(target) => self.request_leave(target),
            Action::OpenProject(path) => session.open_project(&path),
            Action::OpenArea(token) => session.open_level(&token),
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
        let dirty = self.session.as_ref().and_then(|s| s.level.as_ref()).is_some_and(|v| v.snapshot.dirty);
        if dirty {
            self.confirm_leave = Some(target);
        } else {
            self.leave(target);
        }
    }

    fn leave(&mut self, target: Leave) {
        match target {
            Leave::Level => {
                if let Some(session) = &mut self.session {
                    session.close_level();
                }
            }
            Leave::Project => {
                if let Some(session) = &mut self.session {
                    session.close_level();
                    session.close_project();
                }
            }
            Leave::Host => self.session = None,
        }
    }
}
