//! Big Star Editor in the browser.
//!
//! Runs the same `bigstar_ui_egui::BigStarApp` as the desktop app through eframe's web runner (wgpu: WebGPU where
//! available, WebGL2 otherwise). It never opens a ROM: it connects to a `bigstar-host` and shows what that host holds.
//! Build with `trunk build --release` from this directory (output in `dist/web`).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("bigstar-web only runs in the browser: build it with `trunk build --release` in apps/bigstar-web");
}
