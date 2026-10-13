//! Embeds the browser frontend's trunk bundle so `bigstar-host serve` hands it out at `/`: `dist/web` at the
//! repository root (what `trunk build --release` in `apps/bigstar-web` writes), or wherever `BIGSTAR_WEB_DIST` says.
//! Without a bundle the host is built all the same, and its page says how to make one.

use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=BIGSTAR_WEB_DIST");
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let dist = std::env::var_os("BIGSTAR_WEB_DIST").map_or_else(|| manifest.join("../../dist/web"), PathBuf::from);
    println!("cargo:rerun-if-changed={}", dist.display());
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dist)
        .map(|entries| entries.flatten().map(|entry| entry.path()).filter(|path| path.is_file()).collect())
        .unwrap_or_default();
    files.sort();
    let mut code = String::from(
        "/// The browser frontend's files, by name; empty when the host was built without a bundle.\npub static WEB_FILES: &[(&str, &[u8])] = &[\n",
    );
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        let (Some(name), Ok(absolute)) = (file.file_name().and_then(|name| name.to_str()), std::fs::canonicalize(file)) else { continue };
        let _ = writeln!(code, "    ({name:?}, include_bytes!({:?})),", absolute.display().to_string());
    }
    code.push_str("];\n");
    let Some(out) = std::env::var_os("OUT_DIR").map(PathBuf::from) else { return };
    if let Err(error) = std::fs::write(out.join("web_bundle.rs"), code) {
        println!("cargo:warning=could not write the web bundle table: {error}");
    }
}
