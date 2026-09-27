//! Portable developer commands, run as `cargo xtask <command>`.
//!
//! - `guests`: build the extension guests into `target/guests/`.
//! - `ci`: build guests, then check formatting, lints and tests.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const GUEST_TARGET: &str = "wasm32-wasip2";

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    let result = match task.as_deref() {
        Some("guests") => guests(),
        Some("ci") => ci(),
        _ => Err("usage: cargo xtask <guests|ci>".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

fn run(command: &mut Command) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("failed to start {command:?}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{command:?} failed with {status}"))
    }
}

/// Builds each guest workspace and copies its components to `target/guests`.
fn guests() -> Result<(), String> {
    let root = root();
    let out = root.join("target/guests");
    std::fs::create_dir_all(&out).map_err(|error| error.to_string())?;
    // (workspace directory, component file names)
    let workspaces: [(&str, &[&str]); 2] = [
        ("guests", &["sample_rust", "faulty"]),
        ("guests/fixtures/mixed-p2", &["mixed_p2"]),
    ];
    for (dir, components) in workspaces {
        let dir = root.join(dir);
        run(cargo().current_dir(&dir).args([
            "build",
            "--locked",
            "--release",
            "--target",
            GUEST_TARGET,
        ]))?;
        for name in components {
            let built = dir.join(format!("target/{GUEST_TARGET}/release/{name}.wasm"));
            let dest = out.join(format!("{name}.wasm"));
            std::fs::copy(&built, &dest)
                .map_err(|error| format!("copy {} failed: {error}", built.display()))?;
        }
    }
    println!("guests built into {}", out.display());
    Ok(())
}

fn ci() -> Result<(), String> {
    guests()?;
    let root = root();
    run(cargo().current_dir(&root).args(["fmt", "--all", "--check"]))?;
    for dir in ["guests", "guests/fixtures/mixed-p2"] {
        run(cargo()
            .current_dir(root.join(dir))
            .args(["fmt", "--all", "--check"]))?;
    }
    run(cargo().current_dir(&root).args([
        "clippy",
        "--locked",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ]))?;
    run(cargo()
        .current_dir(&root)
        .args(["test", "--locked", "--workspace"]))
}
