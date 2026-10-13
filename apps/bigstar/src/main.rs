//! Big Star Editor for the desktop. It can run the host in its own process or connect to a remote one; the window is
//! the same `bigstar-ui-egui` frontend the browser build shows.

mod services;

use bigstar_ui_egui::{BigStarApp, EMBEDDED_ROOT_KEY, Services};
use services::NativeServices;

struct Shell(BigStarApp);

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
}

fn main() -> eframe::Result {
    env_logger_init();
    let services = NativeServices::load();
    // `bigstar <workspace>` opens the folder named on the command line the next time the connect screen shows.
    if let Some(root) = std::env::args().nth(1) {
        services.store(EMBEDDED_ROOT_KEY, &root);
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("Big Star Editor").with_inner_size([1280.0, 800.0]).with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native("Big Star Editor", options, Box::new(move |_cc| Ok(Box::new(Shell(BigStarApp::new(Box::new(services)))))))
}

/// Logging goes to the terminal at `RUST_LOG`'s level; there is no logger crate in the tree, so a minimal one.
fn env_logger_init() {
    struct Stderr;
    impl log::Log for Stderr {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            metadata.level() <= log::Level::Warn
        }
        fn log(&self, record: &log::Record) {
            if self.enabled(record.metadata()) {
                eprintln!("[{}] {}", record.level(), record.args());
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: Stderr = Stderr;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    }
}
