//! The application: which screen is showing and what its menus and buttons do. With a project open it is
//! `MainWindow`: the level list and the picker on the left, the documents in the middle, the properties on the right.

use std::sync::Arc;

use bigstar_client::{Connection, ConnectionState, Waker, WebSocket, websocket_url};
use bigstar_protocol::{DirectoryEntryKind, LevelId};
use egui::{Color32, Key, Modifiers, RichText, Ui, ViewportCommand};

use crate::canvas::{Art, TexturePool};
use crate::controls::{self, Button, Choice as _, Row, TextField, Width};
use crate::files::{FilesDock, OpenRequest};
use crate::i18n::Language;
use crate::level_view::CanvasOptions;
use crate::picker::{self, PickerState};
use crate::services::Services;
use crate::session::Session;
use crate::theme::{Theme, Tokens};
use crate::tools::{self, Tools};
use crate::workspace::Workspace;
use crate::zoom_canvas::Grids;
use crate::{dialogs, icons, level_data, level_list, properties_ui};

/// Storage keys, so a shell can seed them.
pub const RECENT_HOSTS_KEY: &str = "recent_hosts";
pub const EMBEDDED_ROOT_KEY: &str = "embedded_root";
const RECENT_LIMIT: usize = 8;
const ICON: &[u8] = include_bytes!("../../../packaging/bigstar.png");

#[derive(Default)]
struct ConnectForm {
    host: String,
    token: String,
    root: String,
    recent: Vec<String>,
    error: Option<String>,
    loaded: bool,
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
    History {
        undo: bool,
        steps: usize,
    },
    Save,
    Leave(Leave),
    OpenProject(String),
    Refresh,
    Browse(String),
    ChooseRom(String),
    UseCleanRom(String),
    /// Asks for the clean ROM with the system's file dialog.
    PickCleanRom,
    SaveRomAs,
    UploadRom,
    DownloadRom,
    CancelCleanRom,
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
    pub(crate) builds: crate::builds::Builds,
    /// The interface language, kept with the other per-machine choices.
    pub(crate) language: Language,
    /// The theme, kept with them.
    theme: Theme,
    /// The system's accent colour, as of this frame.
    accent: Option<Color32>,
    /// The palette the style was last set from; `None` before the first frame.
    styled: Option<Tokens>,
    /// The Preferences window's choices while it is open.
    pub(crate) preferences: Option<Preferences>,
    /// Whether the About window is open.
    about: bool,
    /// The editor's icon for the About window, uploaded the first time it opens.
    about_icon: Option<egui::TextureHandle>,
    actions: Vec<Action>,
    textures: TexturePool,
    pub(crate) picker: PickerState,
    pub(crate) workspace: Workspace,
    pub(crate) tools: Tools,
    pub(crate) files: FilesDock,
    left: Left,
    side: Side,
    /// The canvas toolbar's switches, the same on every canvas.
    canvas_options: CanvasOptions,
    /// The graphics canvases' grid switches, as last stored.
    grids: Grids,
    /// The connection is the desktop's own host for a ROM on this machine, which holds nothing but that ROM.
    local_rom: bool,
    /// A ROM to upload has been asked for from a picker that answers later.
    awaiting_upload: bool,
    /// Level-data value cells being typed in.
    data_drafts: level_data::Drafts,
}

/// Where the interface language is kept.
pub const LANGUAGE_KEY: &str = "language";
/// Where the choice of theme is kept, as [`Theme::stored`] writes it.
pub const THEME_KEY: &str = "theme";
/// Where the canvas toolbar's switches are kept.
pub const CANVAS_OPTIONS_KEY: &str = "canvas_options";
/// Where the graphics canvases' grid switches are kept.
pub const GRIDS_KEY: &str = "graphics_grids";

/// The Preferences window: the language is this machine's; the autosave interval and the backups kept are the open
/// project's, kept by its host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Preferences {
    pub(crate) language: Language,
    pub(crate) theme: Theme,
    pub(crate) autosave_minutes: u32,
    pub(crate) backups_kept: u32,
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
        let language = services.load(LANGUAGE_KEY).map_or_else(Language::default, |code| Language::from_code(&code));
        let theme = services.load(THEME_KEY).map_or_else(Theme::default, |stored| Theme::parse(&stored));
        let canvas_options = services.load(CANVAS_OPTIONS_KEY).and_then(|stored| serde_json::from_str(&stored).ok()).unwrap_or_default();
        let grids = services.load(GRIDS_KEY).and_then(|stored| serde_json::from_str(&stored).ok()).unwrap_or_default();
        Self {
            services,
            language,
            theme,
            accent: None,
            styled: None,
            preferences: None,
            about: false,
            about_icon: None,
            form: ConnectForm::default(),
            session: None,
            host_key: String::new(),
            confirm: None,
            allow_quit: false,
            builds: crate::builds::Builds::default(),
            actions: Vec::new(),
            textures: TexturePool::default(),
            picker: PickerState::default(),
            workspace: Workspace::default(),
            tools: Tools::default(),
            files: FilesDock::default(),
            left: Left::default(),
            side: Side::default(),
            canvas_options,
            grids,
            data_drafts: level_data::Drafts::new(),
            local_rom: false,
            awaiting_upload: false,
        }
    }

    /// Starts on a connection that already exists (screenshots, tests).
    pub fn with_connection(services: Box<dyn Services>, conn: Box<dyn Connection>) -> Self {
        let mut app = Self::new(services);
        app.host_key = "local".to_owned();
        app.session = Some(Session::new(conn));
        app
    }

    /// A switch flipped on one canvas' toolbar is flipped on them all, and remembered; a level that arrives takes
    /// them as they stand.
    fn share_canvas_options(&mut self) {
        let Some(session) = &mut self.session else { return };
        if let Some(changed) = session.levels.values().map(|view| view.options).find(|options| *options != self.canvas_options) {
            self.canvas_options = changed;
            if let Ok(stored) = serde_json::to_string(&changed) {
                self.services.store(CANVAS_OPTIONS_KEY, &stored);
            }
        }
        session.canvas_options = self.canvas_options;
        for view in session.levels.values_mut() {
            view.options = self.canvas_options;
        }
    }

    /// The graphics canvases' grid switches: handed to them on the first frame, and remembered when one is flipped.
    fn share_grids(&mut self, ctx: &egui::Context) {
        match Grids::stored(ctx) {
            None => self.grids.set(ctx),
            Some(grids) if grids != self.grids => {
                self.grids = grids;
                if let Ok(stored) = serde_json::to_string(&grids) {
                    self.services.store(GRIDS_KEY, &stored);
                }
            }
            Some(_) => {}
        }
    }

    /// Chooses the dark or the light mode of the theme, as Preferences would.
    pub fn set_dark(&mut self, dark: bool) {
        self.set_theme(Theme { dark, ..self.theme });
    }

    /// Chooses the theme, as Preferences would.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.services.store(THEME_KEY, &theme.stored());
    }

    /// The fonts once, and the style whenever the palette changes: a theme chosen in Preferences shows while the window
    /// is open, before it is saved.
    fn style(&mut self, ctx: &egui::Context) {
        if self.styled.is_none() {
            ctx.set_fonts(crate::theme::fonts());
        }
        self.accent = self.services.accent_color().map(|[r, g, b]| Color32::from_rgb(r, g, b));
        let theme = self.preferences.as_ref().map_or(self.theme, |preferences| preferences.theme);
        let tokens = theme.tokens(self.accent);
        if self.styled.as_ref() != Some(&tokens) {
            crate::theme::apply(ctx, &tokens);
            self.styled = Some(tokens);
        }
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

    /// Pairs an open graphics document with a palette from anywhere, as dropping it on its palette row would
    /// (screenshots).
    pub fn pair_for_screenshot(&mut self, path: &str, palette: &str) {
        if let Some(doc) = self.workspace.files.values_mut().find(|doc| doc.path == path) {
            doc.waiting.push((palette.to_owned(), Some(true)));
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
        // A page the host handed out with its token is for that host.
        if let Some(token) = self.services.default_token() {
            self.form.token = token;
            self.form.host = self.services.default_host();
        }
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
        self.local_rom = false;
        self.workspace = Workspace::default();
        self.session = Some(Session::new(conn));
    }

    /// Asks for a `.nds` on this machine and opens it to edit directly, as NSMB Editor's Open ROM does: with the
    /// desktop's own host for it, which has no workspace.
    pub fn open_local_rom(&mut self, ctx: &egui::Context) {
        let Some(path) = self.services.pick_rom("Open ROM") else { return };
        match self.services.connect_local_roms(waker(ctx)) {
            Ok(conn) => {
                self.start(conn, format!("rom:{path}"));
                self.local_rom = true;
                if let Some(session) = &mut self.session {
                    session.open_rom(&path);
                }
            }
            Err(message) => self.form.error = Some(message),
        }
    }

    /// Asks for a ROM to send to the host and edit there.
    fn upload_rom(&mut self) {
        let Some(session) = &mut self.session else { return };
        match self.services.open_file("Upload ROM", Some("nds")) {
            Some(Ok((name, bytes))) => session.upload_rom(&name, &bytes),
            Some(Err(error)) => session.report(error),
            None => self.awaiting_upload = self.services.pick_file_later(Some("nds")),
        }
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
                if let Some(session) = &mut self.session {
                    session.local_files = true;
                }
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
        // A ROM picked on this machine that would not open leaves nothing to show but the start screen and why.
        if self.local_rom
            && let Some(session) = &self.session
            && session.project.is_none()
            && let Some(notice) = session.notice.as_ref().filter(|notice| notice.error)
        {
            self.form.error = Some(notice.text.clone());
            self.session = None;
        }
        let Some(session) = &mut self.session else { return };
        if self.awaiting_upload
            && let Some(picked) = self.services.take_picked_file()
        {
            self.awaiting_upload = false;
            match picked {
                Ok((name, bytes)) => session.upload_rom(&name, &bytes),
                Err(error) => session.report(error),
            }
        }
        if let Some((name, bytes)) = session.downloaded.take()
            && let Err(error) = self.services.save_file("Download ROM", &name, "nds", &bytes)
        {
            session.report(error);
        }
        self.workspace.sync(session);
        self.tools.take_bytes(session, &*self.services);
        self.tools.open_metadata(session);
        self.builds.follow(session);
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
                // A palette or graphics opened while an editor could take it may be meant for that editor.
                OpenRequest::Editor { path, bytes, kind, metadata } if self.workspace.offer_attach(&path, &bytes, kind, metadata) => Vec::new(),
                OpenRequest::Editor { path, bytes, kind, metadata } => {
                    open_editor(&*self.services, &mut self.workspace, session.filesystem.as_ref(), &path, &bytes, kind, metadata)
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
        self.style(&ctx);
        self.share_canvas_options();
        self.share_grids(&ctx);
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
        let keyboard = !ctx.egui_wants_keyboard_input() && self.confirm.is_none() && self.workspace.asking_attach.is_none();
        let project_open = self.session.as_ref().is_some_and(|s| s.connected() && s.project.is_some());
        if project_open {
            self.left_panel(ui);
            self.right_panel(ui);
        }
        egui::CentralPanel::default().show(ui, |ui| {
            let Some(session) = &mut self.session else { return };
            if !session.connected() {
                controls::loading(ui, "Connecting…");
            } else if session.project.is_some() {
                self.workspace.language = self.language;
                self.workspace.show(ui, session, &mut self.textures, keyboard, &*self.services);
            } else if session.has_workspace() {
                self.workspace_screen(ui);
            } else {
                self.rom_screen(ui);
            }
        });
        if let Some(session) = &mut self.session {
            self.workspace.show_detached(&ctx, session, &mut self.textures, keyboard);
            self.tools.show(&ctx, session, &mut self.workspace);
            self.builds.windows(&ctx, session);
        }
        self.preferences_window(&ctx);
        self.about_window(&ctx);
        if let Some(session) = &mut self.session {
            session.request_actor_draws();
        }
        self.clean_rom_window(&ctx);
        self.recovery_window(&ctx);
        self.confirm_window(&ctx);
        self.attach_window(&ctx);
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
        /// How wide the two cards are.
        const WIDTH: f32 = 480.0;
        egui::CentralPanel::default().show(ui, |ui| {
            let side = ((ui.available_width() - WIDTH) / 2.0).max(0.0);
            ui.horizontal_top(|ui| {
                ui.add_space(side);
                ui.vertical(|ui| {
                    ui.set_width(WIDTH.min(ui.available_width()));
                    ui.add_space(56.0);
                    ui.heading("Big Star Editor");
                    if self.services.can_embed() {
                        ui.weak("Open a ROM or a folder of projects, or connect to a host that has them.");
                    } else {
                        ui.weak("Connect to a host to edit its projects, or a ROM you upload to it.");
                    }
                    ui.add_space(16.0);
                    let (mut embedded, mut remote, mut rom) = (false, false, false);
                    if self.services.can_embed() {
                        controls::section(ui, "A ROM", |ui| {
                            ui.weak("Edit a .nds file directly. Every save writes to the file, and no project is needed.");
                            ui.add_space(4.0);
                            rom = Button::new("Open ROM…").with_icon(icons::FILE).primary().show(ui).clicked();
                        });
                        ui.add_space(12.0);
                        controls::section(ui, "NCPatcher projects", |ui| {
                            ui.weak("Open a folder of NCPatcher projects on this computer.");
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                let edit = TextField::new(&mut self.form.root).hint("Workspace folder").width(Width::Rest(170.0)).show(ui);
                                if Button::new("Browse…").with_icon(icons::FOLDER).show(ui).clicked()
                                    && let Some(folder) = self.services.pick_folder()
                                {
                                    self.form.root = folder;
                                }
                                embedded = Button::new("Open").primary().show(ui).clicked()
                                    || (edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
                            });
                        });
                        ui.add_space(12.0);
                    }
                    controls::section(ui, "A host", |ui| {
                        ui.weak("Connect to a bigstar-host running on this computer or another one.");
                        ui.add_space(4.0);
                        egui::Grid::new("remote").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                            ui.label("Address");
                            TextField::new(&mut self.form.host).name("Address").show(ui);
                            ui.end_row();
                            ui.label("Token");
                            let token = TextField::new(&mut self.form.token).name("Token").password().show(ui);
                            remote = token.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                            ui.end_row();
                        });
                        if !self.form.recent.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                ui.weak("Recent:");
                                for host in self.form.recent.clone() {
                                    if controls::link(ui, &host).clicked() {
                                        self.form.host = host;
                                    }
                                }
                            });
                        }
                        ui.add_space(4.0);
                        remote |= Button::new("Connect").primary().show(ui).clicked();
                    });
                    if let Some(error) = &self.form.error {
                        ui.add_space(10.0);
                        controls::error_text(ui, error);
                    }
                    if rom {
                        self.open_local_rom(ctx);
                    } else if embedded {
                        self.connect_embedded(ctx);
                    } else if remote {
                        self.connect_remote(ctx);
                    }
                });
            });
        });
    }

    fn top_bar(&mut self, ui: &mut Ui) {
        let services = &*self.services;
        let tools = &mut self.tools;
        let workspace = &mut self.workspace;
        let builds = &mut self.builds;
        let language = self.language;
        let theme = self.theme;
        let preferences = &mut self.preferences;
        let about = &mut self.about;
        egui::Panel::top("top").show(ui, |ui| {
            let Some(session) = &mut self.session else { return };
            let actions = &mut self.actions;
            let ctx = ui.ctx().clone();
            let idle = !session.busy();
            let rom_mode = session.rom_mode();
            let has_project = session.project.is_some();
            // What undo and redo would take back, for the Edit menu and the toolbar both.
            let (mut undo_names, mut redo_names, mut can_undo, mut can_redo, can_save) =
                session.level().map_or((Vec::new(), Vec::new(), false, false, false), |view| {
                    let s = &view.snapshot;
                    (s.undo_names.clone(), s.redo_names.clone(), idle && s.can_undo, idle && s.can_redo, idle && s.dirty)
                });
            // A graphics editor in front keeps its own history.
            if let Some(history) = workspace.editor_history() {
                (undo_names, redo_names, can_undo, can_redo) = history;
            }
            // A ROM opened without a workspace is the whole connection, so closing it ends the connection.
            let close_host = if session.has_workspace() {
                "Close Workspace"
            } else if has_project {
                "Close ROM"
            } else {
                "Disconnect"
            };
            // Within a workspace, closing what is open goes back to its projects.
            let close_project = (session.has_workspace() && has_project).then_some(if rom_mode { "Close ROM" } else { "Close project" });
            let shortcut = |modifiers, key| egui::KeyboardShortcut::new(modifiers, key);

            ui.add_space(2.0);
            egui::MenuBar::new().style(controls::bar_style).ui(ui, |ui| {
                controls::bar_menu(ui, language.tr("File"), |ui| {
                    if has_project {
                        if Button::menu("Save").shortcut(&ctx, shortcut(Modifiers::COMMAND, Key::S)).enabled(can_save).show(ui).clicked() {
                            actions.push(Action::Save);
                            ui.close();
                        }
                        if session.uploaded() {
                            let download =
                                Button::menu(if session.undownloaded { "Download ROM •" } else { "Download ROM" }).enabled(!session.downloading());
                            if download.show(ui).clicked() {
                                actions.push(Action::DownloadRom);
                                ui.close();
                            }
                        } else if rom_mode && Button::menu("Save ROM As…").show(ui).clicked() {
                            actions.push(Action::SaveRomAs);
                            ui.close();
                        }
                        ui.separator();
                        tools::level_file_items(ui, tools, session, services);
                        ui.separator();
                    }
                    if Button::menu("Preferences…").show(ui).clicked() {
                        let settings = session.settings.as_ref();
                        *preferences = Some(Preferences {
                            language,
                            theme,
                            autosave_minutes: settings.map_or(1, |s| s.autosave_minutes.max(1)),
                            backups_kept: settings.map_or(10, |s| s.backups_kept.max(1)),
                        });
                        ui.close();
                    }
                    ui.separator();
                    if let Some(close) = close_project
                        && Button::menu(close).show(ui).clicked()
                    {
                        actions.push(Action::Leave(Leave::Project));
                        ui.close();
                    }
                    if Button::menu(close_host).show(ui).clicked() {
                        actions.push(Action::Leave(Leave::Host));
                        ui.close();
                    }
                });
                if has_project {
                    controls::bar_menu(ui, language.tr("Edit"), |ui| {
                        if Button::menu("Undo").shortcut(&ctx, shortcut(Modifiers::COMMAND, Key::Z)).enabled(can_undo).show(ui).clicked() {
                            actions.push(Action::Undo);
                            ui.close();
                        }
                        let redo = shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
                        if Button::menu("Redo").shortcut(&ctx, redo).enabled(can_redo).show(ui).clicked() {
                            actions.push(Action::Redo);
                            ui.close();
                        }
                        ui.separator();
                        tools::find_items(ui, tools, session);
                    });
                    controls::bar_menu(ui, language.tr("View"), |ui| {
                        if Button::menu("Preview").show(ui).clicked() {
                            workspace.show_preview_tab();
                            ui.close();
                        }
                        if Button::menu("Output").selected(builds.show_output).show(ui).clicked() {
                            builds.show_output = !builds.show_output;
                            ui.close();
                        }
                    });
                    if !rom_mode {
                        controls::bar_menu(ui, "Build", |ui| builds.build_menu(ui, session));
                    }
                    controls::bar_menu(ui, language.tr("Tools"), |ui| tools::menu(ui, tools, session, language));
                }
                controls::bar_menu(ui, "Help", |ui| {
                    if Button::menu("About").show(ui).clicked() {
                        *about = true;
                        ui.close();
                    }
                });

                // What is open, at the far end of the bar; laid out from the right, so last first.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !has_project {
                        // Nothing open yet, so the way out stays in sight rather than only in the File menu.
                        if Button::new(close_host).quiet().show(ui).clicked() {
                            actions.push(Action::Leave(Leave::Host));
                        }
                    }
                    if let Some(project) = &session.project {
                        if session.uploaded() {
                            ui.weak("Uploaded ROM")
                                .on_hover_text("The host keeps this ROM only while you are connected. Saves stay there until you download it");
                        } else if rom_mode {
                            ui.weak("ROM").on_hover_text("Every save writes straight into this ROM file");
                        }
                        if let Some(title) = &project.game_title {
                            ui.weak(title);
                        }
                        ui.label(RichText::new(&project.name).strong()).on_hover_text(&project.path);
                    }
                    if !matches!(session.state, ConnectionState::Connected(_)) {
                        ui.label("Connecting…");
                    }
                });
            });

            // The row stays when no document is active (the Preview tab, say), so switching tabs does not move everything
            // below it; its buttons just have nothing to act on.
            if has_project {
                let mut build = false;
                ui.horizontal(|ui| {
                    // Right-click either for every step, named; picking one takes it back and everything after it.
                    let history = |undo: bool, names: &[String], actions: &mut Vec<Action>, ui: &mut Ui| {
                        for (index, name) in names.iter().enumerate() {
                            if Button::menu(name).show(ui).clicked() {
                                actions.push(Action::History { undo, steps: index + 1 });
                                ui.close();
                            }
                        }
                    };
                    let undo = Button::new("Undo").with_icon(icons::UNDO).enabled(can_undo).show(ui);
                    if undo.clicked() {
                        actions.push(Action::Undo);
                    }
                    controls::context_menu(&undo, |ui| history(true, &undo_names, actions, ui));
                    let redo = Button::new("Redo").with_icon(icons::REDO).enabled(can_redo).show(ui);
                    if redo.clicked() {
                        actions.push(Action::Redo);
                    }
                    controls::context_menu(&redo, |ui| history(false, &redo_names, actions, ui));
                    if Button::new("Save").with_icon(icons::SAVE).enabled(can_save).show(ui).clicked() {
                        actions.push(Action::Save);
                    }
                    ui.separator();
                    // What this kind of project ends in, one press away: the ROM to take home, or the build to test.
                    if session.uploaded() {
                        let download = Button::new(if session.undownloaded { "Download ROM •" } else { "Download ROM" })
                            .with_icon(icons::SAVE)
                            .primary()
                            .enabled(!session.downloading())
                            .show(ui)
                            .on_hover_text("Saves a copy of the ROM, with your saved edits, to this computer");
                        if download.clicked() {
                            actions.push(Action::DownloadRom);
                        }
                    } else if rom_mode {
                        if Button::new("Save ROM As…").with_icon(icons::FILE).show(ui).clicked() {
                            actions.push(Action::SaveRomAs);
                        }
                    } else {
                        build = Button::new("Run Patcher")
                            .with_icon(icons::PLAY)
                            .primary()
                            .enabled(!session.build.running)
                            .show(ui)
                            .on_hover_text("Saves your levels, builds the selected variant with NCPatcher, and loads the new ROM")
                            .clicked();
                        crate::builds::Builds::variant_selector(ui, session);
                    }
                    ui.separator();
                    let Some(snapshot) = session.level().map(|view| &view.snapshot) else {
                        ui.weak("No level open for editing");
                        return;
                    };
                    let summary = format!(
                        "{} · {} objects · {} sprites · {} entrances · revision {}",
                        snapshot.title,
                        snapshot.objects.len(),
                        snapshot.course.sprites.len(),
                        snapshot.course.entrances.len(),
                        snapshot.revision
                    );
                    // Cut short rather than run off the window, sharing the rest of the row with a warning when there is one.
                    let reserve = if snapshot.glue_error.is_some() { ui.available_width() / 2.0 } else { 0.0 };
                    controls::one_line(ui, RichText::new(summary).weak(), reserve);
                    if let Some(error) = &snapshot.glue_error {
                        let warning = RichText::new(format!("Custom actor data unreadable: {error}")).color(Tokens::get(ui.ctx()).warning);
                        controls::one_line(ui, warning, 0.0);
                    }
                });
                if build {
                    builds.run(session);
                }
            }
            ui.add_space(2.0);
        });
    }

    fn bottom_bar(&mut self, ui: &mut Ui) {
        let Some(session) = &mut self.session else { return };
        if self.builds.show_output {
            let builds = &mut self.builds;
            egui::Panel::bottom("output").resizable(true).default_size(180.0).show(ui, |ui| builds.output(ui, session));
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                Button::new("Output").small().quiet().selected(self.builds.show_output).show(ui).clicked().then(|| {
                    self.builds.show_output = !self.builds.show_output;
                });
                if session.build.running {
                    ui.add(controls::Spinner::new());
                    ui.weak(&session.build.status);
                }
                // One line whatever it says, so the bar keeps its height and Dismiss stays in reach.
                let mut dismiss = false;
                if let Some(notice) = &session.notice {
                    let text = RichText::new(&notice.text);
                    let text = if notice.error { text.color(Tokens::get(ui.ctx()).danger) } else { text };
                    controls::one_line(ui, text, controls::HEIGHT_SM + ui.spacing().item_spacing.x);
                    dismiss = Button::icon(icons::CLOSE).ghost().small().name("Dismiss").show(ui).on_hover_text("Dismiss").clicked();
                } else if let Some(last) = session.log.last() {
                    controls::one_line(ui, RichText::new(&last.text).weak(), 0.0);
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
            let filesystem = self.language.tr("Filesystem");
            controls::tabs(ui, "left", &mut self.left, &[(Left::Levels, "Levels"), (Left::Palette, "Palette"), (Left::Filesystem, filesystem)]);
            ui.add_space(2.0);
            match self.left {
                Left::Filesystem => self.files.show(ui, session, &*self.services),
                Left::Levels => {
                    if let Some(path) = level_list::show(ui, session, self.workspace.previewed()) {
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

    /// The properties and level data of the active document. The panel stays while a project is open, whatever is in
    /// front, so moving between a level and another document does not resize everything in the middle.
    fn right_panel(&mut self, ui: &mut Ui) {
        let Some(session) = &mut self.session else { return };
        let level = session.active;
        let busy = session.busy();
        let rom_mode = session.rom_mode();
        let mut edit = None;
        let mut error = None;
        egui::Panel::right("properties").resizable(true).default_size(330.0).show(ui, |ui| {
            // The Level Data tab edits glue, which only the module's code reads; a ROM edited directly keeps it as it is.
            if rom_mode {
                self.side = Side::Properties;
                controls::tabs(ui, "side", &mut self.side, &[(Side::Properties, "Properties")]);
            } else {
                controls::tabs(ui, "side", &mut self.side, &[(Side::Properties, "Properties"), (Side::LevelData, "Level Data")]);
            }
            ui.add_space(2.0);
            let Some(view) = level.and_then(|level| session.levels.get_mut(&level)) else {
                ui.weak("Bring a level to the front to see and change its properties.");
                return;
            };
            match self.side {
                Side::Properties => {
                    let overlays = session.project_data.as_ref().and_then(|d| d.object_bank_overlays.as_ref());
                    let modules = session.project_data.as_ref().filter(|_| !rom_mode).map(|d| &d.modules);
                    edit = properties_ui::show(ui, view, overlays, modules, busy);
                }
                Side::LevelData => {
                    let modules = session.project_data.as_ref().map(|d| &d.modules);
                    // Scrolls sideways too: its value fields cannot shrink with the panel.
                    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                        match level_data::show(ui, &view.snapshot, modules, &mut self.data_drafts, busy) {
                            Some(level_data::Outcome::Edit(made)) => edit = Some(made),
                            Some(level_data::Outcome::Error(message)) => error = Some(message),
                            None => {}
                        }
                    });
                }
            }
        });
        if let Some(level) = level
            && let Some(edit) = edit.filter(|_| !busy)
        {
            session.edit_level(level, edit);
        }
        if let Some(error) = error {
            session.report(error);
        }
    }

    fn workspace_screen(&mut self, ui: &mut Ui) {
        let Some(session) = &self.session else { return };
        let actions = &mut self.actions;
        // Only a host elsewhere needs a ROM sent to it; this computer's own opens one from the connect screen.
        let remote = !self.host_key.starts_with("embedded:");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.heading("Projects");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if remote
                    && Button::new("Upload ROM…")
                        .with_icon(icons::FILE)
                        .show(ui)
                        .on_hover_text("Edit a ROM from this computer without making a project. Download it when you are done")
                        .clicked()
                {
                    actions.push(Action::UploadRom);
                }
                if Button::new("Refresh").with_icon(icons::REFRESH).show(ui).clicked() {
                    actions.push(Action::Refresh);
                }
            });
        });
        ui.add_space(4.0);
        if session.projects.is_empty() {
            controls::notice(
                ui,
                icons::FOLDER,
                "No projects here yet",
                "A project is a folder with an ncpatcher.yaml. Make one with ncpatcher init in the workspace folder, then refresh.",
            );
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for project in &session.projects {
                controls::section(ui, "", |ui| {
                    ui.horizontal(|ui| {
                        if Button::new(RichText::new(&project.name).strong()).with_icon(icons::FOLDER).quiet().show(ui).clicked() {
                            actions.push(Action::OpenProject(project.path.clone()));
                        }
                        ui.weak(&project.path);
                    });
                });
            }
        });
    }

    /// What a connection with no workspace shows with no ROM open: a way to upload one.
    fn rom_screen(&mut self, ui: &mut Ui) {
        let Some(session) = &self.session else { return };
        ui.add_space(8.0);
        if let Some((done, total)) = session.upload {
            ui.heading("Uploading ROM");
            ui.add_space(8.0);
            ui.add(egui::ProgressBar::new(done as f32 / total.max(1) as f32).show_percentage());
            return;
        }
        // A ROM picked on this machine is opening, and one that fails goes back to the start screen: there is no page
        // for a desktop ROM connection without its ROM.
        if !self.local_rom {
            controls::notice(
                ui,
                icons::FILE,
                "Edit a ROM",
                "Upload a New Super Mario Bros. ROM to edit it here. The host keeps it only while you are connected, so download it when you are done.",
            );
            if Button::new("Upload ROM…").with_icon(icons::FILE).primary().show(ui).clicked() {
                self.actions.push(Action::UploadRom);
            }
        }
    }

    /// Choosing the clean ROM the project builds from, which `ncpatcher.yaml` does not name: asked for when a project
    /// opens without a usable one, and from the Build menu.
    fn clean_rom_window(&mut self, ctx: &egui::Context) {
        let Some(session) = &mut self.session else { return };
        let actions = &mut self.actions;
        let problem = session.settings.as_ref().and_then(|settings| settings.clean_rom_problem.clone());
        let clean_rom = session.settings.as_ref().map(|settings| settings.clean_rom.clone());
        let local_files = session.local_files;
        let Some(browser) = &mut session.browser else { return };
        let mut open = true;
        dialogs::window("Clean ROM").open(&mut open).default_width(460.0).show(ctx, |ui| {
            ui.label(problem.as_deref().unwrap_or("Choose the clean ROM this project is built from."));
            ui.weak("An unmodified New Super Mario Bros. ROM. Every build starts from it, and it is never changed.");
            ui.add_space(4.0);
            // The desktop's own host takes a ROM anywhere on this machine, so the system's dialog picks it.
            if local_files {
                if let Some(current) = clean_rom.as_deref().filter(|rom| !rom.is_empty()) {
                    ui.label(format!("Now: {current}"));
                }
                dialogs::actions(ui, |ui| {
                    if Button::new("Choose ROM…").primary().show(ui).clicked() {
                        actions.push(Action::PickCleanRom);
                    }
                });
                return;
            }
            ui.horizontal(|ui| {
                if Button::new("Up").with_icon(icons::ARROW_UP).small().enabled(!browser.path.is_empty()).show(ui).clicked() {
                    actions.push(Action::Browse(parent(&browser.path)));
                }
                ui.label(if browser.path.is_empty() { "/".to_owned() } else { format!("/{}", browser.path) });
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, true]).show(ui, |ui| match &browser.listing {
                None => {
                    ui.add(controls::Spinner::new());
                }
                Some(listing) => {
                    for entry in &listing.entries {
                        let full = join(&listing.path, &entry.name);
                        match entry.kind {
                            DirectoryEntryKind::Directory => {
                                if Row::new(&entry.name).icon(icons::FOLDER).show(ui).clicked() {
                                    actions.push(Action::Browse(full));
                                }
                            }
                            DirectoryEntryKind::File if entry.name.to_ascii_lowercase().ends_with(".nds") => {
                                let chosen = browser.rom.as_deref() == Some(full.as_str());
                                if Row::new(&entry.name).icon(icons::FILE).selected(chosen).show(ui).clicked() {
                                    actions.push(Action::ChooseRom(full));
                                }
                            }
                            DirectoryEntryKind::File => {
                                ui.add_enabled_ui(false, |ui| Row::new(&entry.name).icon(icons::FILE).show(ui));
                            }
                        }
                    }
                }
            });
            dialogs::actions(ui, |ui| {
                if Button::new("Use This ROM").primary().enabled(browser.rom.is_some()).show(ui).clicked()
                    && let Some(rom) = browser.rom.clone()
                {
                    actions.push(Action::UseCleanRom(rom));
                }
            });
        });
        if !open {
            actions.push(Action::CancelCleanRom);
        }
    }

    /// The offer of what an interrupted session left autosaved.
    fn recovery_window(&mut self, ctx: &egui::Context) {
        let Some(session) = &mut self.session else { return };
        let Some(levels) = &session.recovery_offer else { return };
        let mut answer = None;
        dialogs::modal(ctx, "recovery", "Recover Autosaved Levels", 440.0, |ui| {
            ui.label(format!("Big Star found {} autosaved level(s) from a session that ended without saving. Recover them?", levels.len()));
            for level in levels {
                ui.weak(format!("• {}", level.title));
            }
            dialogs::actions(ui, |ui| {
                if Button::new("Recover").primary().show(ui).clicked() {
                    answer = Some(true);
                }
                if Button::new("Discard").show(ui).clicked() {
                    answer = Some(false);
                }
            });
        });
        if let Some(accept) = answer {
            session.answer_recovery(accept);
        }
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        if !self.about {
            return;
        }
        let mut open = true;
        let mut close = false;
        let icon = about_icon(ctx, &mut self.about_icon);
        dialogs::window("About Big Star Editor").open(&mut open).resizable(false).default_width(480.0).show(ctx, |ui| {
            dialogs::fit_width(ui, 480.0);
            ui.horizontal_top(|ui| {
                if let Some(icon) = icon {
                    ui.add(egui::Image::new((icon, egui::vec2(72.0, 72.0))));
                    ui.add_space(12.0);
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new(format!("Big Star Editor {}", env!("CARGO_PKG_VERSION"))).strong().size(16.0));
                    ui.add_space(6.0);
                    ui.label(
                        "A level and ROM editor for New Super Mario Bros. DS: edit levels, their objects and \
                         actors, browse and change the ROM's files, and build NCPatcher code projects into it.",
                    );
                    ui.add_space(6.0);
                    ui.label(
                        "Thank you to the whole New Super Mario Bros. DS community for the years of effort, \
                         curiosity and generosity that made this editor possible.",
                    );
                    ui.add_space(6.0);
                    ui.label(
                        "Thank you to every original contributor to NSMB Editor, whose work this editor builds on: \
                         its actors, its artwork and the formats it first understood.",
                    );
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.weak("Built with ");
                        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
                        ui.weak(".");
                    });
                });
            });
            dialogs::actions(ui, |ui| {
                if Button::new("Close").primary().show(ui).clicked() {
                    close = true;
                }
            });
        });
        if close || !open {
            self.about = false;
        }
    }

    fn preferences_window(&mut self, ctx: &egui::Context) {
        let Some(preferences) = &mut self.preferences else { return };
        let project = self.session.as_ref().is_some_and(|session| session.project.is_some());
        let mut open = true;
        let mut answer = None;
        let accent = self.accent;
        dialogs::window("Preferences").open(&mut open).resizable(false).show(ctx, |ui| {
            egui::Grid::new("preferences").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label(preferences.language.tr("Language"));
                controls::drop_down("preference-language").selected_text(preferences.language.name()).show_ui(ui, |ui| {
                    for language in Language::ALL {
                        ui.choice_value(&mut preferences.language, language, language.name());
                    }
                });
                ui.end_row();
                ui.label("Recovery snapshots");
                ui.add_enabled(project, controls::number(&mut preferences.autosave_minutes).range(1..=60).suffix(" min"))
                    .on_hover_text("How often unsaved level edits are backed up, in case the editor closes unexpectedly.");
                ui.end_row();
                ui.label("Backups kept");
                ui.add_enabled(project, controls::number(&mut preferences.backups_kept).range(1..=100)).on_hover_text(
                    "How many backups of level saves are kept. Older ones are deleted when a project opens and when a new backup is made.",
                );
                ui.end_row();
            });
            if !project {
                ui.weak("These settings are kept per project. Open a project to change them.");
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Theme").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.choice_value(&mut preferences.theme.dark, false, "Light");
                    ui.choice_value(&mut preferences.theme.dark, true, "Dark");
                });
            });
            egui::Grid::new("preference-themes").num_columns(3).spacing([8.0, 8.0]).show(ui, |ui| {
                for (index, family) in crate::theme::FAMILIES.iter().enumerate() {
                    let palette = Theme { family: family.id, ..preferences.theme }.tokens(accent);
                    if controls::theme_card(ui, family.name, &palette, preferences.theme.family == family.id).clicked() {
                        preferences.theme.family = family.id;
                    }
                    if index % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
            ui.add_enabled_ui(accent.is_some(), |ui| {
                controls::checkbox(ui, &mut preferences.theme.system_accent, "Use the system's accent colour")
                    .on_disabled_hover_text("This system does not say what its accent colour is.");
            });
            dialogs::actions(ui, |ui| {
                if Button::new("Save").primary().show(ui).clicked() {
                    answer = Some(true);
                }
                if Button::new("Cancel").show(ui).clicked() {
                    answer = Some(false);
                }
            });
        });
        match answer {
            Some(true) => {
                let chosen = preferences.clone();
                self.preferences = None;
                self.language = chosen.language;
                self.services.store(LANGUAGE_KEY, chosen.language.code());
                self.set_theme(chosen.theme);
                if let Some(session) = self.session.as_mut().filter(|session| session.project.is_some()) {
                    session.set_preferences(chosen.autosave_minutes, chosen.backups_kept);
                }
            }
            Some(false) => self.preferences = None,
            None if !open => self.preferences = None,
            None => {}
        }
    }

    /// Asks which open graphics editor a palette or graphics file being opened goes to, as NSMB Editor's palettes are
    /// chosen for any graphics; or opens it on its own.
    fn attach_window(&mut self, ctx: &egui::Context) {
        let Some(prompt) = &self.workspace.asking_attach else { return };
        let name = prompt.path.rsplit(['/', '!']).next().unwrap_or(&prompt.path).to_owned();
        let (mut decision, mut cancelled) = (None, false);
        let (_, dismissed) = dialogs::modal(ctx, "attach", &format!("Open {name}"), 460.0, |ui| {
            ui.label("Attach it to an open graphics editor, to draw that file with it, or open it in its own editor.");
            ui.add_space(6.0);
            for (key, path) in &prompt.targets {
                if Row::new(format!("Attach to {path}")).icon(crate::icons::IMAGE).show(ui).clicked() {
                    decision = Some(Some(*key));
                }
            }
            dialogs::actions(ui, |ui| {
                if Button::new("Open on its own").show(ui).clicked() {
                    decision = Some(None);
                }
                if Button::new("Cancel").show(ui).clicked() {
                    cancelled = true;
                }
            });
        });
        if cancelled || (dismissed && decision.is_none()) {
            self.workspace.asking_attach = None;
        }
        let Some(decision) = decision else { return };
        let Some(prompt) = self.workspace.asking_attach.take() else { return };
        match decision {
            Some(key) => self.workspace.attach(key, &prompt.path, &prompt.bytes),
            None => {
                let listing = self.session.as_ref().and_then(|session| session.filesystem.as_ref());
                let reads = open_editor(&*self.services, &mut self.workspace, listing, &prompt.path, &prompt.bytes, prompt.kind, prompt.metadata);
                if let Some(session) = &mut self.session {
                    for companion in reads {
                        session.read_file(&companion, crate::session::FileRead::Companion);
                    }
                }
            }
        }
    }

    fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(question) = self.confirm else { return };
        let undownloaded = self.session.as_ref().is_some_and(|session| session.undownloaded);
        let (text, leave) = match question {
            Confirm::Close(_) => ("This level has changes that were not saved. Closing it loses them.", "Close without saving"),
            Confirm::Quit if undownloaded => {
                ("The uploaded ROM has edits that were not downloaded. Quitting loses them.", "Quit without downloading")
            }
            Confirm::Leave(_) if undownloaded => {
                ("The uploaded ROM has edits that were not downloaded. Leaving loses them.", "Leave without downloading")
            }
            Confirm::Quit => ("Some levels have changes that were not saved. Quitting loses them.", "Quit without saving"),
            Confirm::Leave(_) => ("Some levels have changes that were not saved. Leaving loses them.", "Leave without saving"),
        };
        let mut decision = None;
        let (_, dismissed) = dialogs::modal(ctx, "unsaved", "Unsaved changes", 420.0, |ui| {
            ui.label(text);
            dialogs::actions(ui, |ui| {
                if Button::new(leave).danger().show(ui).clicked() {
                    decision = Some(true);
                }
                if Button::new("Stay").show(ui).clicked() {
                    decision = Some(false);
                }
            });
        });
        if dismissed && decision.is_none() {
            decision = Some(false);
        }
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
        // A hex editor with the keyboard undoes its own typing.
        let level = self.session.as_ref().is_some_and(|s| s.active.is_some());
        let editor = self.workspace.editor_history().is_some();
        if !(level || editor) || self.confirm.is_some() || self.workspace.asking_attach.is_some() || crate::hex_editor::has_keyboard(ctx) {
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
        match action {
            Action::UploadRom => return self.upload_rom(),
            Action::SaveRomAs => {
                let suggested = self.session.as_ref().and_then(|s| s.project.as_ref()).map_or_else(|| "rom.nds".to_owned(), |p| p.name.clone());
                if let Some(path) = self.services.pick_save_path("Save ROM As", &suggested, "nds")
                    && let Some(session) = &mut self.session
                {
                    session.save_rom_as(&path);
                }
                return;
            }
            _ => {}
        }
        let step = match action {
            Action::Undo => Some((true, 1)),
            Action::Redo => Some((false, 1)),
            Action::History { undo, steps } => Some((undo, steps)),
            _ => None,
        };
        if let Some((undo, steps)) = step
            && self.workspace.editor_step(undo, steps)
        {
            return;
        }
        let Some(session) = &mut self.session else { return };
        match action {
            Action::Undo => session.undo(),
            Action::Redo => session.redo(),
            Action::History { undo, steps } => session.step_history(undo, steps),
            Action::Save => session.save(),
            Action::Leave(target) => self.request_leave(target),
            Action::OpenProject(path) => session.open_project(&path),
            Action::Refresh => session.list_projects(),
            Action::Browse(path) => {
                if let Some(browser) = &mut session.browser {
                    browser.listing = None;
                }
                session.browse(&path);
            }
            Action::ChooseRom(path) => {
                if let Some(browser) = &mut session.browser {
                    browser.rom = Some(path);
                }
            }
            Action::UseCleanRom(rom) => session.set_clean_rom(&rom),
            Action::PickCleanRom => {
                if let Some(rom) = self.services.pick_rom("Clean ROM") {
                    session.set_clean_rom(&rom);
                }
            }
            Action::DownloadRom => session.download_rom(),
            Action::CancelCleanRom => session.browser = None,
            Action::UploadRom | Action::SaveRomAs => {}
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

/// The editor's icon, scaled down once and smoothed: unlike the level art, it is not pixel art.
fn about_icon(ctx: &egui::Context, slot: &mut Option<egui::TextureHandle>) -> Option<egui::TextureId> {
    if slot.is_none() {
        let icon = image::load_from_memory_with_format(ICON, image::ImageFormat::Png).ok()?;
        let icon = icon.resize(144, 144, image::imageops::FilterType::Lanczos3).into_rgba8();
        let image = egui::ColorImage::from_rgba_unmultiplied([icon.width() as usize, icon.height() as usize], icon.as_raw());
        *slot = Some(ctx.load_texture("bigstar:icon", image, egui::TextureOptions::LINEAR));
    }
    slot.as_ref().map(egui::TextureHandle::id)
}

/// Opens a file in its editor, paired with what it was paired with last time, or at first with the palette and
/// graphics of its name in `listing`. Returns the companion files to read.
fn open_editor(
    services: &dyn Services,
    workspace: &mut crate::workspace::Workspace,
    listing: Option<&bigstar_protocol::FilesystemListing>,
    path: &str,
    bytes: &[u8],
    kind: crate::editors::EditorKind,
    metadata: bool,
) -> Vec<String> {
    let key = crate::graphics_editor::pairing_key(path);
    let pairing = services.load(&key).and_then(|text| serde_json::from_str(&text).ok()).or_else(|| {
        // Never paired before: the palette and graphics of the same name, which are usually the ones, until the
        // person says else.
        let files = || listing.map(|l| l.entries.iter().map(|e| e.path.as_str())).into_iter().flatten();
        let palettes: Vec<String> = crate::graphics_editor::namesake_palette(path, files()).into_iter().collect();
        let characters = crate::graphics_editor::namesake_graphics(path, files());
        (!palettes.is_empty() || characters.is_some()).then(|| crate::graphics_editor::Pairing { palettes, characters, ..Default::default() })
    });
    workspace.open_file(path, bytes, kind, pairing, metadata)
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
