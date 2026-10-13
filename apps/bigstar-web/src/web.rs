//! The browser shell: storage in `localStorage`, the host that served the page as the default, and the eframe web runner.

use bigstar_ui_egui::{BigStarApp, Services};
use wasm_bindgen::JsCast as _;

const CANVAS_ID: &str = "bigstar_canvas";
const LOADING_ID: &str = "bigstar_loading";

struct WebServices;

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

impl Services for WebServices {
    fn load(&self, key: &str) -> Option<String> {
        storage()?.get_item(&format!("bigstar.{key}")).ok().flatten()
    }

    fn store(&self, key: &str, value: &str) {
        if let Some(storage) = storage()
            && storage.set_item(&format!("bigstar.{key}"), value).is_err()
        {
            log::warn!("could not save {key}");
        }
    }

    /// `?token=…` in the page's address, which is how `bigstar-host serve` prints its link.
    fn default_token(&self) -> Option<String> {
        let search = web_sys::window()?.location().search().ok()?;
        search.trim_start_matches('?').split('&').find_map(|pair| pair.strip_prefix("token=")).filter(|token| !token.is_empty()).map(str::to_owned)
    }

    /// The host that served this page, over the matching socket scheme.
    fn default_host(&self) -> String {
        let location = web_sys::window().map(|w| w.location());
        let secure = location.as_ref().and_then(|l| l.protocol().ok()).is_some_and(|p| p == "https:");
        match location.and_then(|l| l.host().ok()).filter(|h| !h.is_empty()) {
            Some(host) => format!("{}://{host}", if secure { "wss" } else { "ws" }),
            None => "ws://127.0.0.1:7878".to_owned(),
        }
    }
}

struct Shell(BigStarApp);

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
}

pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            log::error!("no document");
            return;
        };
        let Some(canvas) = document.get_element_by_id(CANVAS_ID).and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else {
            log::error!("missing <canvas id=\"{CANVAS_ID}\">");
            return;
        };
        let result = eframe::WebRunner::new()
            .start(canvas, eframe::WebOptions::default(), Box::new(|_cc| Ok(Box::new(Shell(BigStarApp::new(Box::new(WebServices)))))))
            .await;
        if let Some(el) = document.get_element_by_id(LOADING_ID) {
            match result {
                Ok(()) => el.remove(),
                Err(e) => {
                    el.set_inner_html(&format!("<p>Big Star Editor failed to start: {e:?}</p><p>A browser with WebGPU or WebGL2 is required.</p>"))
                }
            }
        }
    });
}
