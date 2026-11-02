//! Finds the language files: every `lang/<code>.json` is a language, embedded so the frontend stays wasm-safe. Writes
//! the list `i18n` reads, English first and the rest by code.

use std::{env, error::Error, fs, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let lang = Path::new(&env::var("CARGO_MANIFEST_DIR")?).join("lang");
    println!("cargo:rerun-if-changed={}", lang.display());
    let mut codes = Vec::new();
    for entry in fs::read_dir(&lang)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "json")
            && let Some(code) = path.file_stem().and_then(|stem| stem.to_str())
        {
            codes.push(code.to_owned());
        }
    }
    if !codes.iter().any(|code| code == "en") {
        return Err("lang/en.json is the language everything falls back to".into());
    }
    for code in &codes {
        if !code.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') || code.starts_with('-') {
            return Err(format!("lang/{code}.json: a language's code is lowercase letters, digits and dashes, like `pt` or `pt-br`").into());
        }
    }
    codes.sort_by(|a, b| (a != "en", a).cmp(&(b != "en", b)));

    let sources: String =
        codes.iter().map(|code| format!("    ({code:?}, include_str!({:?})),\n", lang.join(format!("{code}.json")).display())).collect();
    let source = format!(
        "/// Every language file: its code, the file's name without `.json`, and its text. English is first.\nconst SOURCES: [(&str, &str); {}] = [\n{sources}];\n",
        codes.len()
    );
    fs::write(Path::new(&env::var("OUT_DIR")?).join("languages.rs"), source)?;
    Ok(())
}
