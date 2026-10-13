//! Dependency layering rules. Package names are `bigstar-<short>`.

use serde_json::Value;

/// Crates that must build for `wasm32-unknown-unknown`.
pub const WASM_SAFE: &[&str] = &["core", "nds", "formats", "level", "protocol", "client", "ui-egui", "web"];

/// Crates that may not be depended on by `ui-egui` (nor by anything wasm-safe).
const HOST_ONLY: &[&str] = &["project", "patching", "host"];

/// External UI crates; only `ui-egui` and the apps may use them.
const UI_CRATES: &[&str] = &["egui", "eframe", "egui_extras", "egui_dock", "egui_kittest", "rfd", "winit"];

/// Workspace crates allowed to use UI crates.
const UI_ALLOWED: &[&str] = &["ui-egui", "bigstar", "web", "xtask"];

#[derive(Debug)]
pub struct Krate {
    pub name: String,
    pub deps: Vec<Dep>,
}

#[derive(Debug)]
pub struct Dep {
    pub name: String,
    pub workspace: bool,
}

pub fn short_name(name: &str) -> &str {
    name.strip_prefix("bigstar-").unwrap_or(name)
}

pub fn from_metadata(meta: &Value) -> Result<Vec<Krate>, String> {
    let pkgs = meta.get("packages").and_then(Value::as_array).ok_or("metadata: no packages")?;
    let names: Vec<&str> = pkgs.iter().filter_map(|p| p.get("name").and_then(Value::as_str)).collect();
    let mut out = Vec::new();
    for p in pkgs {
        let name = p.get("name").and_then(Value::as_str).ok_or("metadata: unnamed package")?.to_string();
        let deps = p
            .get("dependencies")
            .and_then(Value::as_array)
            .map(|ds| {
                ds.iter()
                    .filter(|d| d.get("kind").is_none_or(Value::is_null))
                    .filter_map(|d| d.get("name").and_then(Value::as_str))
                    .map(|n| Dep { name: n.to_string(), workspace: names.contains(&n) })
                    .collect()
            })
            .unwrap_or_default();
        out.push(Krate { name, deps });
    }
    Ok(out)
}

pub fn check(crates: &[Krate]) -> Vec<String> {
    let mut v = Vec::new();
    for c in crates {
        let me = short_name(&c.name);
        let wasm_safe = WASM_SAFE.contains(&me);
        for d in &c.deps {
            let dep = short_name(&d.name);
            if d.workspace && wasm_safe && me != "web" && HOST_ONLY.contains(&dep) {
                v.push(format!("{me} depends on host-only crate {dep}"));
            }
            if d.workspace && wasm_safe && (dep == "bigstar" || dep == "host-app") {
                v.push(format!("{me} depends on app {dep}"));
            }
            if !d.workspace && UI_CRATES.contains(&d.name.as_str()) && !UI_ALLOWED.contains(&me) {
                v.push(format!("{me} depends on UI crate {}", d.name));
            }
            if d.workspace && dep == "ui-egui" && !matches!(me, "bigstar" | "web") {
                v.push(format!("{me} depends on ui-egui"));
            }
        }
    }
    v
}
