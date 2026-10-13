//! Workspace tooling: `cargo xtask <command>`.

mod layers;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  layers   enforce the crate dependency layering (docs/rust-port-plan.md, rule 4)
  wasm     cargo check --target wasm32-unknown-unknown for the wasm-safe crates
  ci       fmt --check, clippy -D warnings, test, layers, wasm (stops at first failure)
  oracle <project>
           build nsmb-cli (C++, in build/) and bigstar-host (Rust), run `oracle` in both on the project and diff
";

fn main() -> ExitCode {
    let result = match std::env::args().nth(1).as_deref() {
        Some("layers") => cmd_layers(),
        Some("wasm") => cmd_wasm(),
        Some("ci") => cmd_ci(),
        Some("oracle") => match std::env::args().nth(2) {
            Some(project) => cmd_oracle(Path::new(&project)),
            None => Err(format!("oracle needs a project directory\n\n{USAGE}")),
        },
        Some("-h" | "--help" | "help") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_default()
}

fn cargo() -> Command {
    let mut c = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    c.current_dir(root());
    c
}

fn run(mut cmd: Command, what: &str) -> Result<(), String> {
    eprintln!("$ {what}");
    let status = cmd.status().map_err(|e| format!("{what}: failed to spawn: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{what}: exited with {status}")) }
}

fn metadata() -> Result<serde_json::Value, String> {
    let out = cargo().args(["metadata", "--format-version", "1", "--no-deps"]).output().map_err(|e| format!("cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!("cargo metadata failed:\n{}", String::from_utf8_lossy(&out.stderr)));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata: bad JSON: {e}"))
}

fn cmd_layers() -> Result<(), String> {
    let crates = layers::from_metadata(&metadata()?)?;
    let violations = layers::check(&crates);
    if violations.is_empty() {
        println!("OK: {} crates, no layering violations.", crates.len());
        Ok(())
    } else {
        for v in &violations {
            println!("  - {v}");
        }
        Err(format!("{} layering violation(s)", violations.len()))
    }
}

fn cmd_wasm() -> Result<(), String> {
    let set: Vec<String> = layers::from_metadata(&metadata()?)?
        .into_iter()
        .filter(|c| layers::WASM_SAFE.contains(&layers::short_name(&c.name)))
        .map(|c| c.name)
        .collect();
    let mut failed = Vec::new();
    for pkg in &set {
        let mut c = cargo();
        c.args(["check", "--target", "wasm32-unknown-unknown", "-p", pkg]);
        if run(c, &format!("cargo check --target wasm32-unknown-unknown -p {pkg}")).is_err() {
            failed.push(pkg.clone());
        }
    }
    if failed.is_empty() { Ok(()) } else { Err(format!("wasm check failed for: {}", failed.join(", "))) }
}

/// Runs `oracle` in the C++ and the Rust CLI on one project and compares the reports line by line. The C++ tree is
/// built in `build/`, configured there first if it has never been.
fn cmd_oracle(project: &Path) -> Result<(), String> {
    let root = root();
    let build = root.join("build");
    if !build.join("CMakeCache.txt").is_file() {
        let mut configure = Command::new("cmake");
        configure.current_dir(&root).args(["-S", ".", "-B", "build"]);
        run(configure, "cmake -S . -B build")?;
    }
    let mut cpp = Command::new("cmake");
    cpp.current_dir(&root).args(["--build", "build", "--target", "nsmb-cli"]);
    run(cpp, "cmake --build build --target nsmb-cli")?;
    let mut rust = cargo();
    rust.args(["build", "--release", "-p", "bigstar-host-app"]);
    run(rust, "cargo build --release -p bigstar-host-app")?;

    let data = root.join("data");
    let out_dir = root.join("target").join("oracle");
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let report = |program: PathBuf, name: &str| -> Result<Vec<String>, String> {
        eprintln!("$ {} oracle {} {}", program.display(), project.display(), data.display());
        let output = Command::new(&program).arg("oracle").arg(project).arg(&data).output().map_err(|e| format!("{}: {e}", program.display()))?;
        if !output.status.success() {
            return Err(format!("{name} oracle failed ({}):\n{}", output.status, String::from_utf8_lossy(&output.stderr)));
        }
        let path = out_dir.join(format!("{name}.txt"));
        std::fs::write(&path, &output.stdout).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(String::from_utf8_lossy(&output.stdout).lines().map(str::to_owned).collect())
    };
    let cpp = report(build.join("apps/nsmb-cli/nsmb-cli"), "cpp")?;
    let rust = report(root.join("target/release/bigstar-host"), "rust")?;

    let mut differences = 0usize;
    for index in 0..cpp.len().max(rust.len()) {
        let (left, right) = (cpp.get(index), rust.get(index));
        if left == right {
            continue;
        }
        differences += 1;
        if differences <= 40 {
            println!(
                "line {}:\n  c++:  {}\n  rust: {}",
                index + 1,
                left.map_or("<missing>", String::as_str),
                right.map_or("<missing>", String::as_str)
            );
        }
    }
    if differences == 0 {
        println!("OK: {} lines, the C++ and Rust reports agree ({}).", cpp.len(), out_dir.display());
        Ok(())
    } else {
        Err(format!("{differences} differing line(s); full reports in {}", out_dir.display()))
    }
}

fn cmd_ci() -> Result<(), String> {
    type Step = (&'static str, Box<dyn Fn() -> Result<(), String>>);
    let steps: Vec<Step> = vec![
        (
            "fmt",
            Box::new(|| {
                let mut c = cargo();
                c.args(["fmt", "--all", "--", "--check"]);
                run(c, "cargo fmt --all -- --check")
            }),
        ),
        (
            "clippy",
            Box::new(|| {
                let mut c = cargo();
                c.args(["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]);
                run(c, "cargo clippy --workspace --all-targets -- -D warnings")
            }),
        ),
        (
            "test",
            Box::new(|| {
                let mut c = cargo();
                c.args(["test", "--workspace"]);
                run(c, "cargo test --workspace")
            }),
        ),
        ("layers", Box::new(cmd_layers)),
        ("wasm", Box::new(cmd_wasm)),
    ];
    let mut done = Vec::new();
    for (name, f) in &steps {
        eprintln!("\n=== ci: {name} ===");
        if let Err(e) = f() {
            println!("\nCI summary: ok [{}]; FAIL {name}", done.join(", "));
            return Err(format!("ci failed at `{name}`: {e}"));
        }
        done.push(*name);
    }
    println!("\nCI summary: all {} steps passed ({})", done.len(), done.join(", "));
    Ok(())
}
