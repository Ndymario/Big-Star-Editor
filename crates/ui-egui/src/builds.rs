//! Builds in the frontend: the Output panel (status, progress, Cancel, the run's diagnostics and the log), the Build
//! menu, the variant selector, and the questions they ask. A port of `PatcherPanel` and of `MainWindow`'s `runPatcher`,
//! `offerNcpatcherMigration`, `openDiagnosticLocation` and `extractRom`; the work itself is the host's.

use bigstar_protocol::{BuildConfigKind, DiagnosticLevel, LogLevel};
use egui::Ui;

use crate::controls::{self, Button, Choice as _, TextField, Width};
use crate::dialogs;
use crate::session::Session;
use crate::theme::Tokens;

#[derive(Default)]
pub struct Builds {
    /// A build was asked for and waits on the project's configuration.
    waiting_config: bool,
    /// A legacy configuration, offered for migration.
    pub migrate: Option<String>,
    /// A migration is running for a build that should start after it.
    migrating: bool,
    /// The folder Extract ROM writes into, while its window is open.
    pub extract: Option<String>,
    /// A diagnostic's location, while it is being shown.
    location: Option<String>,
    pub show_output: bool,
}

impl Builds {
    /// Run Patcher: saves the edited levels and builds the active variant, after offering to migrate a legacy
    /// configuration. The host checks the rest.
    pub fn run(&mut self, session: &mut Session) {
        if session.build.running {
            session.report("NCPatcher is already running.".to_owned());
            return;
        }
        session.request_build_config();
        self.waiting_config = true;
        self.show_output = true;
    }

    /// Follows what the host answered.
    pub fn follow(&mut self, session: &mut Session) {
        if self.waiting_config
            && let Some(kind) = session.build_config.take()
        {
            self.waiting_config = false;
            match kind {
                BuildConfigKind::None => session.report("This project has no ncpatcher.yaml, so there is nothing for NCPatcher to build.".to_owned()),
                BuildConfigKind::Legacy(file) => self.migrate = Some(file),
                BuildConfigKind::Yaml => session.start_build(true),
            }
        }
        if self.migrating && session.migrated {
            self.migrating = false;
            session.migrated = false;
            session.start_build(true);
        }
    }

    pub fn build_menu(&mut self, ui: &mut Ui, session: &mut Session) {
        if Button::menu("Run Patcher…").enabled(!session.build.running).show(ui).clicked() {
            self.run(session);
            ui.close();
        }
        if Button::menu("Cancel Build").enabled(session.build.running && !session.build.cancelling).show(ui).clicked() {
            session.cancel_build();
            ui.close();
        }
        ui.separator();
        if Button::menu("Clean ROM…").show(ui).clicked() {
            session.choose_clean_rom();
            ui.close();
        }
        // NCPatcher's own extractor writes a directory it can read back, so that is the one that runs.
        if Button::menu("Extract ROM…").show(ui).clicked() {
            let project = session.project.as_ref().map(|p| p.path.clone()).unwrap_or_default();
            self.extract = Some(if project.is_empty() { "extracted".to_owned() } else { format!("{project}/extracted") });
            ui.close();
        }
    }

    /// One variant selector for the whole window: switching reloads levels and graphics from that variant's layers.
    pub fn variant_selector(ui: &mut Ui, session: &mut Session) {
        let Some(settings) = &session.settings else { return };
        let current = settings.active_variant.clone();
        let mut chosen = current.clone();
        ui.label("Variant:");
        controls::drop_down("variant-selector")
            .selected_text(if current.is_empty() { "(none)".to_owned() } else { current.clone() })
            .show_ui(ui, |ui| {
                ui.choice_value(&mut chosen, String::new(), "(none)");
                for variant in &settings.variants {
                    ui.choice_value(&mut chosen, variant.clone(), variant);
                }
            })
            .response
            .on_hover_text("The build variant you are editing.\nSwitching reloads the levels and graphics for that variant.");
        if chosen != current {
            session.set_active_variant(&chosen);
        }
    }

    /// The Output panel's body: what the build is doing, its diagnostics, and the log.
    pub fn output(&mut self, ui: &mut Ui, session: &mut Session) {
        ui.horizontal(|ui| {
            let status = if session.build.status.is_empty() { "Idle" } else { session.build.status.as_str() };
            ui.label(status);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if session.build.running
                    && ui
                        .add(Button::new("Cancel").small().enabled(!session.build.cancelling))
                        .on_hover_text("Stops NCPatcher. The project is left as it was before this build.")
                        .clicked()
                {
                    session.cancel_build();
                }
                let bar = match session.build.progress {
                    Some((current, total)) => egui::ProgressBar::new((current as f32 / total.max(1) as f32).clamp(0.0, 1.0)).show_percentage(),
                    None if session.build.running => egui::ProgressBar::new(0.0).animate(true),
                    None => egui::ProgressBar::new(0.0),
                };
                ui.add(bar.desired_width(220.0));
            });
        });
        if !session.build.diagnostics.is_empty() {
            egui::ScrollArea::vertical().id_salt("diagnostics").max_height(140.0).show(ui, |ui| {
                egui::Grid::new("diagnostics-grid").num_columns(3).striped(true).show(ui, |ui| {
                    ui.strong("Code");
                    ui.strong("Message");
                    ui.strong("Location");
                    ui.end_row();
                    let t = Tokens::get(ui.ctx());
                    for diagnostic in &session.build.diagnostics {
                        let color = match diagnostic.level {
                            DiagnosticLevel::Error => t.danger,
                            DiagnosticLevel::Warning => t.warning,
                            DiagnosticLevel::Info => t.foreground,
                        };
                        ui.colored_label(color, &diagnostic.code);
                        // The first line is the claim; the source excerpt under it belongs in the tooltip.
                        let headline = diagnostic.message.lines().next().unwrap_or_default();
                        ui.label(headline).on_hover_text(&diagnostic.message);
                        let location = if diagnostic.file.is_empty() {
                            String::new()
                        } else {
                            format!("{}:{}:{}", diagnostic.file, diagnostic.line, diagnostic.column)
                        };
                        if !location.is_empty() && controls::link(ui, &location).on_hover_text("Shows where it was reported").clicked() {
                            self.location = Some(format!("{}\n\nLine {}, column {}", diagnostic.file, diagnostic.line, diagnostic.column));
                        }
                        ui.end_row();
                    }
                });
            });
            ui.separator();
        }
        egui::ScrollArea::vertical().id_salt("log").stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
            let t = Tokens::get(ui.ctx());
            ui.spacing_mut().item_spacing.y = 1.0;
            for line in &session.log {
                let color = match line.level {
                    LogLevel::Info => t.muted,
                    LogLevel::Warning => t.warning,
                    LogLevel::Error => t.danger,
                };
                ui.label(egui::RichText::new(&line.text).monospace().size(12.0).color(color));
            }
        });
    }

    /// The questions and small windows the menus open.
    pub fn windows(&mut self, ctx: &egui::Context, session: &mut Session) {
        if let Some(file) = self.migrate.clone() {
            let mut answer = None;
            dialogs::modal(ctx, "migrate-config", "Legacy NCPatcher Project", 480.0, |ui| {
                ui.label(format!(
                    "This project still uses the old ncpatcher.json:\n\n{file}\n\nNCPatcher 2 needs ncpatcher.yaml. Convert it now?\nThe original ncpatcher.json is kept."
                ));
                dialogs::actions(ui, |ui| {
                    if Button::new("Convert").primary().show(ui).clicked() {
                        answer = Some(true);
                    }
                    if Button::new("Cancel").show(ui).clicked() {
                        answer = Some(false);
                    }
                });
            });
            match answer {
                Some(true) => {
                    self.migrate = None;
                    self.migrating = true;
                    session.migrate_config();
                }
                Some(false) => {
                    self.migrate = None;
                    session.report("Run Patcher cancelled: the project still uses ncpatcher.json.".to_owned());
                }
                None => {}
            }
        }
        if let Some(text) = self.location.clone() {
            // Big Star has no YAML editor of its own, so the honest thing is to say exactly where the problem is.
            let (close, dismissed) = dialogs::modal(ctx, "diagnostic-location", "Diagnostic Location", 440.0, |ui| {
                ui.label(text);
                dialogs::actions(ui, |ui| Button::new("OK").primary().show(ui).clicked())
            });
            if close || dismissed {
                self.location = None;
            }
        }
        if let Some(folder) = &mut self.extract {
            let mut open = true;
            let mut run = false;
            dialogs::window("Extract ROM").open(&mut open).show(ctx, |ui| {
                ui.label("The folder in the workspace that the built ROM's files are extracted to.");
                ui.add(TextField::new(folder).name("Folder").width(Width::Px(360.0)));
                run = dialogs::actions(ui, |ui| Button::new("Extract").primary().show(ui).clicked());
            });
            if run {
                session.extract_rom(folder);
            }
            if run || !open {
                self.extract = None;
            }
        }
    }
}
