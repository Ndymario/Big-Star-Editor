//! Browser frontend (stub until Phase 6).

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("bigstar-web only runs in the browser: build it with `trunk build --release` in apps/bigstar-web");
}
