//! Development mode's building blocks: how a local package's source folder
//! is built on save, and the processes such a build runs (ADR 0004; #12,
//! #13).
//!
//! The launcher (`launcher::develop`) watches the folder with the system's
//! file watcher, runs the package's [`Build`] after each save, and reloads
//! the package when the build succeeds. This module decides what a build
//! is: one adapter per language, each running the build command the guest
//! README documents, in the package folder itself:
//!
//! - **Rust**, a folder with `Cargo.toml`: `cargo build --release --target
//!   wasm32-wasip2`, whose component the manifest names under `target/`.
//! - **JavaScript or TypeScript**, a folder with `package.json`: Pane's JS
//!   build, `python3 tools/componentize-js/pane_js.py build <folder>
//!   <folder>/<component>`, once for each component the manifest names.
//!
//! It is not a general build system: there is no build command of the
//! package's own, and a folder with neither file cannot be developed.

use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::packages::Manifest;

/// Decides how a package is built on save. [`Toolchains`] is Pane's; tests
/// supply their own.
pub trait Builder: Send + Sync + 'static {
    /// The build Pane runs after each save in the package's source folder
    /// `folder`, or why Pane cannot build it there.
    fn build_for(&self, folder: &Path) -> Result<Arc<dyn Build>, String>;
}

/// One package's build.
pub trait Build: Send + Sync + 'static {
    /// The build command, as the author would type it in the package
    /// folder.
    fn command(&self) -> String;

    /// Whether a change at `path`, relative to the package folder, belongs
    /// to the build (its output, its lock file) rather than being a save.
    /// Hidden files and editor backups (`~`) are never saves either.
    fn ignores(&self, path: &Path) -> bool;

    /// Builds the package, returning what the build printed, or the
    /// diagnostics if it failed. Once `stop` is set, it stops what it
    /// started and returns early; its outcome is then not used.
    fn run(&self, stop: &BuildStop) -> Result<String, String>;
}

/// Tells a running build to stop. Cloning shares it.
#[derive(Clone, Debug, Default)]
pub struct BuildStop(Arc<AtomicBool>);

impl BuildStop {
    /// Whether the build should stop.
    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    pub(crate) fn stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Pane's builds, with the tools they run.
#[derive(Clone, Debug)]
pub struct Toolchains {
    /// Cargo, which builds Rust packages.
    pub cargo: OsString,
    /// The Python interpreter that runs Pane's JavaScript build.
    pub python: OsString,
    /// Pane's JavaScript build, `tools/componentize-js/pane_js.py` of a Pane
    /// source checkout; `None` if this Pane does not know one.
    pub componentize_js: Option<PathBuf>,
}

impl Toolchains {
    /// The tools as this computer names them: `cargo` from `PATH` (rustup's,
    /// so a package's `rust-toolchain.toml` applies), `PYTHON` or else
    /// `python3` (`python` on Windows), as `cargo xtask` does, and
    /// `PANE_COMPONENTIZE_JS`, else `default_js` if that file exists.
    pub fn from_env(default_js: Option<PathBuf>) -> Toolchains {
        let python = std::env::var_os("PYTHON")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| if cfg!(windows) { "python" } else { "python3" }.into());
        let componentize_js = std::env::var_os("PANE_COMPONENTIZE_JS")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| default_js.filter(|path| path.is_file()));
        Toolchains {
            cargo: "cargo".into(),
            python,
            componentize_js,
        }
    }
}

impl Builder for Toolchains {
    fn build_for(&self, folder: &Path) -> Result<Arc<dyn Build>, String> {
        let manifest = Manifest::read_parsed(folder)
            .map_err(|error| error.to_string())?
            .0;
        let components = components(&manifest);
        if folder.join("Cargo.toml").is_file() {
            return Ok(Arc::new(CargoBuild {
                cargo: self.cargo.clone(),
                folder: folder.to_path_buf(),
                components,
            }));
        }
        if folder.join("package.json").is_file() {
            let Some(tool) = self.componentize_js.clone() else {
                return Err("Pane does not know where its JavaScript build is: set \
                            PANE_COMPONENTIZE_JS to tools/componentize-js/pane_js.py in a Pane \
                            source checkout"
                    .into());
            };
            return Ok(Arc::new(JsBuild {
                python: self.python.clone(),
                tool,
                folder: folder.to_path_buf(),
                components,
            }));
        }
        Err(format!(
            "{} has neither Cargo.toml (Rust) nor package.json (JavaScript or TypeScript), so \
             Pane does not know how to build it; build it yourself and reload it",
            folder.display()
        ))
    }
}

/// The distinct components `manifest` names, relative to its folder.
fn components(manifest: &Manifest) -> Vec<PathBuf> {
    let mut components: Vec<PathBuf> = Vec::new();
    for (_, component) in manifest.components() {
        if !components.iter().any(|known| known == component) {
            components.push(component.to_path_buf());
        }
    }
    components
}

/// Whether `path`, relative to a package folder, is or is inside one of
/// `outputs` (components the build writes), or is inside the top-level
/// folder of one that is in a folder (`dist/a.wasm` makes `dist` output).
fn is_output(path: &Path, outputs: &[PathBuf]) -> bool {
    outputs.iter().any(|output| {
        let mut parts = output.components();
        let top = parts.next();
        let nested = parts.next().is_some();
        path.starts_with(output) || (nested && top.is_some_and(|top| path.starts_with(top)))
    })
}

/// Whether `path`, relative to a package folder, starts with the top-level
/// entry `name`.
fn under(path: &Path, name: &str) -> bool {
    path.components().next() == Some(Component::Normal(OsStr::new(name)))
}

/// A Rust package: `cargo build --release --target wasm32-wasip2`.
struct CargoBuild {
    cargo: OsString,
    folder: PathBuf,
    components: Vec<PathBuf>,
}

impl CargoBuild {
    const ARGS: [&'static str; 4] = ["build", "--release", "--target", "wasm32-wasip2"];
}

impl Build for CargoBuild {
    fn command(&self) -> String {
        format!("cargo {}", CargoBuild::ARGS.join(" "))
    }

    fn ignores(&self, path: &Path) -> bool {
        under(path, "target")
            || path == Path::new("Cargo.lock")
            || is_output(path, &self.components)
    }

    fn run(&self, stop: &BuildStop) -> Result<String, String> {
        let mut command = Command::new(&self.cargo);
        command.current_dir(&self.folder).args(CargoBuild::ARGS);
        without_inherited_cargo(&mut command);
        run_command(command, &self.command(), stop)
    }
}

/// A JavaScript or TypeScript package: Pane's JS build, once per component.
struct JsBuild {
    python: OsString,
    tool: PathBuf,
    folder: PathBuf,
    components: Vec<PathBuf>,
}

impl JsBuild {
    fn command_for(&self, component: &Path) -> String {
        format!(
            "{} {} build {} {}",
            self.python.to_string_lossy(),
            self.tool.display(),
            self.folder.display(),
            self.folder.join(component).display()
        )
    }
}

impl Build for JsBuild {
    fn command(&self) -> String {
        let commands: Vec<String> = self
            .components
            .iter()
            .map(|component| self.command_for(component))
            .collect();
        commands.join(" && ")
    }

    fn ignores(&self, path: &Path) -> bool {
        under(path, "node_modules") || is_output(path, &self.components)
    }

    fn run(&self, stop: &BuildStop) -> Result<String, String> {
        // The manifest as saved now: it may name other components.
        let manifest = Manifest::read_parsed(&self.folder)
            .map_err(|error| error.to_string())?
            .0;
        let mut printed = String::new();
        for component in components(&manifest) {
            let mut command = Command::new(&self.python);
            command
                .current_dir(&self.folder)
                .arg(&self.tool)
                .arg("build")
                .arg(&self.folder)
                .arg(self.folder.join(&component));
            match run_command(command, &self.command_for(&component), stop) {
                Ok(output) => printed.push_str(&output),
                Err(diagnostics) => return Err(format!("{printed}{diagnostics}")),
            }
        }
        Ok(printed)
    }
}

/// Removes what `cargo run` or `cargo xtask` set for Pane itself, so the
/// package's own toolchain file and target folder apply, as `pane_js.py`
/// does for its builds.
fn without_inherited_cargo(command: &mut Command) {
    for (key, _) in std::env::vars_os() {
        let Some(key) = key.to_str() else { continue };
        let cargo = key.starts_with("CARGO_") && key != "CARGO_HOME";
        let named = ["CARGO", "RUSTUP_TOOLCHAIN", "RUSTC", "RUSTDOC", "RUSTFLAGS"].contains(&key);
        if cargo || named {
            command.env_remove(key);
        }
    }
}

/// How often a running build is checked for having finished or being
/// stopped.
const POLL: Duration = Duration::from_millis(50);

/// Runs `command` (shown as `shown`) to its end, returning what it printed
/// on standard output and error, or that with why it failed. When `stop` is
/// set, the command and every process it started are killed, and it
/// returns at once.
pub(crate) fn run_command(
    mut command: Command,
    shown: &str,
    stop: &BuildStop,
) -> Result<String, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    own_process_group(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| format!("Pane could not run `{shown}`: {error}"))?;
    let printed = Arc::new(Mutex::new(String::new()));
    let readers = [
        read_into(child.stdout.take(), printed.clone()),
        read_into(child.stderr.take(), printed.clone()),
    ];
    let status = loop {
        if stop.is_stopped() {
            kill_tree(&mut child);
            // The readers end once the killed processes' pipes close.
            return Err(format!("`{shown}` was stopped"));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(POLL),
            Err(error) => {
                kill_tree(&mut child);
                return Err(format!("Pane lost track of `{shown}`: {error}"));
            }
        }
    };
    for reader in readers.into_iter().flatten() {
        let _ = reader.join();
    }
    let printed = std::mem::take(&mut *printed.lock().unwrap_or_else(|p| p.into_inner()));
    if status.success() {
        Ok(printed)
    } else {
        Err(format!("{printed}`{shown}` failed ({})", describe(status)))
    }
}

fn describe(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("exit code {code}"),
        None => status.to_string(),
    }
}

/// Appends what `pipe` carries to `printed` until it closes.
fn read_into(
    pipe: Option<impl Read + Send + 'static>,
    printed: Arc<Mutex<String>>,
) -> Option<JoinHandle<()>> {
    let mut pipe = pipe?;
    Some(std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) | Err(_) => return,
                Ok(read) => {
                    let text = String::from_utf8_lossy(&buffer[..read]);
                    printed
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .push_str(&text);
                }
            }
        }
    }))
}

/// Starts the command in a process group of its own, so that stopping it
/// reaches the processes it starts (rustc, npm, node, the componentizer);
/// on Windows, without a console window.
fn own_process_group(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
}

/// Kills `child` and the processes it started, and reaps it.
fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        // The group `own_process_group` made has the child's id.
        if let Ok(group) = i32::try_from(child.id()) {
            // SAFETY: kill(2) with a negative id signals that process group;
            // it has no memory effects.
            unsafe {
                libc::kill(-group, libc::SIGKILL);
            }
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// The line of a build's diagnostics to show first: the first that reports
/// an error, else the last line.
pub(crate) fn first_error(diagnostics: &str) -> &str {
    let lines = || {
        diagnostics
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
    };
    lines()
        .find(|line| line.to_ascii_lowercase().contains("error"))
        .or_else(|| lines().next_back())
        .unwrap_or("the build failed")
}

/// Whether a change at `path`, relative to a package folder, is a save for
/// `build`: not a hidden file or folder, an editor backup (`name~`), or the
/// build's own output.
pub(crate) fn is_save(path: &Path, build: &dyn Build) -> bool {
    let hidden = path.components().any(|part| match part {
        Component::Normal(name) => name.to_string_lossy().starts_with('.'),
        _ => false,
    });
    let backup = path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().ends_with('~'));
    !path.as_os_str().is_empty() && !hidden && !backup && !build.ignores(path)
}

/// A channel from the launcher's background work, such as development
/// mode, to the window, which redraws when told.
pub fn changes() -> (ChangeSender, Changes) {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    (ChangeSender(sender), Changes(receiver))
}

/// Tells the window that the launcher changed in the background.
#[derive(Clone, Debug)]
pub struct ChangeSender(tokio::sync::mpsc::UnboundedSender<()>);

impl ChangeSender {
    pub(crate) fn changed(&self) {
        let _ = self.0.send(());
    }
}

/// The window's end of [`changes`].
#[derive(Debug)]
pub struct Changes(tokio::sync::mpsc::UnboundedReceiver<()>);

impl Changes {
    /// Resolves when the launcher changed since the last call, or with
    /// `None` once it has stopped. Several changes meanwhile are one.
    pub async fn next(&mut self) -> Option<()> {
        let change = self.0.recv().await;
        while self.0.try_recv().is_ok() {}
        change
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(folder: &Path, component: &str) {
        std::fs::write(
            folder.join("pane.json"),
            format!(
                r#"{{ "manifestVersion": 1, "title": "Hello", "apiVersion": "0.1",
                     "commands": [{{ "id": "hello", "title": "Hello", "component": "{component}" }}] }}"#
            ),
        )
        .unwrap();
    }

    fn toolchains() -> Toolchains {
        Toolchains {
            cargo: "cargo".into(),
            python: "python3".into(),
            componentize_js: Some(PathBuf::from("/pane/tools/componentize-js/pane_js.py")),
        }
    }

    #[test]
    fn a_folder_with_cargo_toml_builds_with_cargo_and_ignores_its_target() {
        let folder = tempfile::tempdir().unwrap();
        manifest(folder.path(), "target/wasm32-wasip2/release/hello.wasm");
        std::fs::write(folder.path().join("Cargo.toml"), "").unwrap();
        let build = toolchains().build_for(folder.path()).unwrap();
        assert_eq!(
            build.command(),
            "cargo build --release --target wasm32-wasip2"
        );
        for output in [
            "target",
            "target/wasm32-wasip2/release/hello.wasm",
            "Cargo.lock",
        ] {
            assert!(!is_save(Path::new(output), &*build), "{output}");
        }
        for source in ["src/lib.rs", "Cargo.toml", "pane.json", "wit/world.wit"] {
            assert!(is_save(Path::new(source), &*build), "{source}");
        }
    }

    #[test]
    fn a_folder_with_package_json_builds_each_component_with_pane_js() {
        let folder = tempfile::tempdir().unwrap();
        manifest(folder.path(), "dist/hello.wasm");
        std::fs::write(folder.path().join("package.json"), "{}").unwrap();
        let build = toolchains().build_for(folder.path()).unwrap();
        let dir = folder.path().display();
        let out = folder.path().join("dist/hello.wasm");
        assert_eq!(
            build.command(),
            format!(
                "python3 /pane/tools/componentize-js/pane_js.py build {dir} {}",
                out.display()
            )
        );
        for output in ["dist", "dist/hello.wasm", "node_modules/zod/index.js"] {
            assert!(!is_save(Path::new(output), &*build), "{output}");
        }
        for source in ["src/index.ts", "package.json", "tsconfig.json", "pane.json"] {
            assert!(is_save(Path::new(source), &*build), "{source}");
        }
    }

    #[test]
    fn hidden_files_and_backups_are_not_saves() {
        let folder = tempfile::tempdir().unwrap();
        manifest(folder.path(), "hello.wasm");
        std::fs::write(folder.path().join("Cargo.toml"), "").unwrap();
        let build = toolchains().build_for(folder.path()).unwrap();
        for path in [
            ".git/index",
            "src/.lib.rs.swp",
            "src/lib.rs~",
            "hello.wasm",
            "",
        ] {
            assert!(!is_save(Path::new(path), &*build), "{path}");
        }
    }

    #[test]
    fn a_folder_without_a_known_build_is_explained() {
        let folder = tempfile::tempdir().unwrap();
        manifest(folder.path(), "hello.wasm");
        let error = toolchains().build_for(folder.path()).err().unwrap();
        assert!(error.contains("neither Cargo.toml"), "{error}");
        let js = Toolchains {
            componentize_js: None,
            ..toolchains()
        };
        std::fs::write(folder.path().join("package.json"), "{}").unwrap();
        let error = js.build_for(folder.path()).err().unwrap();
        assert!(error.contains("PANE_COMPONENTIZE_JS"), "{error}");
    }

    #[test]
    fn the_first_error_line_is_shown_first() {
        let cargo =
            "   Compiling hello v0.1.0\nerror[E0308]: mismatched types\n --> src/lib.rs:3:5\n";
        assert_eq!(first_error(cargo), "error[E0308]: mismatched types");
        assert_eq!(first_error("one\nlast line\n\n"), "last line");
        assert_eq!(first_error(""), "the build failed");
    }

    /// A command that prints, then fails or succeeds.
    fn shell(script: &str) -> Command {
        if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.args(["/C", script]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", script]);
            command
        }
    }

    #[test]
    fn a_command_reports_what_it_printed() {
        let stop = BuildStop::default();
        let printed = run_command(shell("echo built"), "echo built", &stop).unwrap();
        assert_eq!(printed.trim(), "built");
        let failed = run_command(shell("echo broken && exit 3"), "fail", &stop).unwrap_err();
        assert!(failed.contains("broken"), "{failed}");
        assert!(failed.contains("`fail` failed (exit code 3)"), "{failed}");
        let missing = Command::new("pane-no-such-program");
        let error = run_command(missing, "pane-no-such-program", &stop).unwrap_err();
        assert!(error.starts_with("Pane could not run"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn stopping_a_command_kills_the_processes_it_started() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("pid");
        let script = format!("sleep 30 & echo $! > {}; wait", pid_file.display());
        let stop = BuildStop::default();
        let running = {
            let stop = stop.clone();
            std::thread::spawn(move || run_command(shell(&script), "sleep", &stop))
        };
        while std::fs::read_to_string(&pid_file).map_or(true, |pid| pid.trim().is_empty()) {
            std::thread::sleep(Duration::from_millis(20));
        }
        let grandchild: i32 = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let started = std::time::Instant::now();
        stop.stop();
        let outcome = running.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(outcome.unwrap_err(), "`sleep` was stopped");
        // The shell's own child is gone too, not left sleeping: once killed
        // and reaped by init, it no longer exists.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        // SAFETY: kill(2) with signal 0 only checks that the process exists.
        while unsafe { libc::kill(grandchild, 0) } == 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "{grandchild} still runs"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
