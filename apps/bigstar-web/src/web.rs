//! The browser shell: storage in `localStorage`, the host that served the page as the default, and the eframe web runner.

use bigstar_ui_egui::{BigStarApp, Services};
use wasm_bindgen::JsCast as _;

const CANVAS_ID: &str = "bigstar_canvas";
const LOADING_ID: &str = "bigstar_loading";

struct WebServices {
    /// The system's accent colour, as the browser gave it when the page loaded.
    accent: Option<[u8; 3]>,
}

/// CSS's `AccentColor`, where the browser knows it: the colour an element given it comes out in.
fn accent_color() -> Option<[u8; 3]> {
    if !web_sys::css::supports_with_value("color", "AccentColor").unwrap_or(false) {
        return None;
    }
    let window = web_sys::window()?;
    let document = window.document()?;
    let probe = document.create_element("span").ok()?;
    probe.set_attribute("style", "color: AccentColor; display: none").ok()?;
    let body = document.body()?;
    body.append_child(&probe).ok()?;
    let color = window.get_computed_style(&probe).ok().flatten().and_then(|style| style.get_property_value("color").ok());
    body.remove_child(&probe).ok()?;
    parse_rgb(&color?)
}

/// `rgb(1, 2, 3)` or `rgba(1, 2, 3, 1)`, which is how a computed colour is written.
fn parse_rgb(text: &str) -> Option<[u8; 3]> {
    let inside = text.trim().strip_prefix("rgb")?.trim_start_matches('a').strip_prefix('(')?.strip_suffix(')')?;
    let mut channels = inside.split([',', ' ', '/']).filter(|part| !part.is_empty()).map(|part| part.parse::<f32>().ok());
    let mut next = || channels.next().flatten().map(|channel| channel.round().clamp(0.0, 255.0) as u8);
    Some([next()?, next()?, next()?])
}

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

    fn accent_color(&self) -> Option<[u8; 3]> {
        self.accent
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
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|_cc| Ok(Box::new(Shell(BigStarApp::new(Box::new(WebServices { accent: accent_color() })))))),
            )
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
