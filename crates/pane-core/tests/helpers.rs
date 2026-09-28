//! Native helpers through the launcher's public interface, with the helper
//! sample (`guests/sample-helper`) and its real helper program, `pane-echo`,
//! built for this system by `cargo xtask guests` and put in the package as
//! this target's file. Pane runs that file and compiles nothing; it ends
//! the helper's process when the guest cancels the run, and when the
//! package is disabled, reloaded, updated or uninstalled while it runs,
//! keeping the package's saved data. Each check that a process ended asks
//! the system too, not only Pane.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use futures::executor::block_on;
use pane_core::{Launcher, PackageIdentity, Runtime, SavedData, Screen, Status, current_target};
use tempfile::TempDir;

/// Under the ten seconds "Echo after waiting" has its helper wait: a run
/// that ends sooner was stopped.
const STOPPED_WITHIN: Duration = Duration::from_secs(8);

/// How long starting the command and its helper may take.
const PROMPTLY: Duration = Duration::from_secs(6);

/// The assembled helper sample package.
fn assembled() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/guests/packages/sample-helper");
    assert!(
        path.exists(),
        "{} is missing; run `cargo xtask guests`",
        path.display()
    );
    let manifest = fs::read_to_string(path.join("pane.json")).unwrap();
    assert!(
        manifest.contains(&format!("\"{}\"", current_target())),
        "the helper sample ships no helper for {}: it is built for the contributor \
         baselines (linux-x86_64, macos-aarch64, windows-x86_64) only",
        current_target()
    );
    path
}

/// This system's helper file in the package, as its `pane.json` names it.
fn helper_file() -> String {
    let file = format!("helpers/{}/pane-echo", current_target());
    if cfg!(windows) { file + ".exe" } else { file }
}

/// Copies the assembled helper sample into `folder`, with its `pane.json`
/// changed by `manifest`, and returns `folder`.
fn helper_package(folder: &Path, manifest: impl FnOnce(String) -> String) -> PathBuf {
    let from = assembled();
    fs::create_dir_all(folder.join(Path::new(&helper_file()).parent().unwrap())).unwrap();
    let text = fs::read_to_string(from.join("pane.json")).unwrap();
    fs::write(folder.join("pane.json"), manifest(text)).unwrap();
    for file in ["sample_helper.wasm".to_owned(), helper_file()] {
        fs::copy(from.join(&file), folder.join(&file))
            .unwrap_or_else(|error| panic!("{file}: {error}"));
    }
    folder.to_path_buf()
}

/// Whether the system still has a process with id `pid`, asked of the
/// system itself.
fn process_exists(pid: u32) -> bool {
    if cfg!(windows) {
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .expect("tasklist runs");
        String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
    } else {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .expect("kill runs")
            .success()
    }
}

struct Installed {
    _sources: TempDir,
    _cache: TempDir,
    data: TempDir,
    runtime: Runtime,
    launcher: Launcher,
    folder: PathBuf,
    identity: PackageIdentity,
}

impl Installed {
    fn new() -> Installed {
        Installed::with_manifest(|text| text)
    }

    fn with_manifest(manifest: impl FnOnce(String) -> String) -> Installed {
        let sources = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let runtime = Runtime::start_with_cache(cache.path().to_path_buf()).unwrap();
        let launcher =
            Launcher::with_packages(Ok(runtime.clone()), vec![], data.path().join("extensions"));
        let folder = helper_package(&sources.path().join("helper"), manifest);
        block_on(launcher.install_package(&folder));
        assert_eq!(
            launcher.view().status,
            Status::Result("Installed Helper sample".into())
        );
        let identity = PackageIdentity::local(&folder).unwrap();
        Installed {
            _sources: sources,
            _cache: cache,
            data,
            runtime,
            launcher,
            folder,
            identity,
        }
    }

    /// Opens the command and runs its item `item`, returning the status.
    fn run(&self, item: &str) -> Status {
        open_sample_at(&self.launcher, item);
        block_on(self.launcher.activate_selected());
        self.launcher.view().status
    }

    /// What "Echo after waiting" has noted: "started", "finished" or
    /// nothing.
    fn waiting(&self) -> Option<String> {
        let path = self.data.path().join("extensions/settings.json");
        let text = fs::read_to_string(path).unwrap_or_default();
        ["finished", "started"]
            .into_iter()
            .find(|progress| text.contains(&format!("\"helper-wait\": \"{progress}\"")))
            .map(str::to_owned)
    }

    /// Runs `item` on another thread and returns once its helper process
    /// runs, with that process's id.
    fn start(&self, item: &str) -> Pending {
        open_sample_at(&self.launcher, item);
        let running = self.launcher.activate_selected();
        let started = Instant::now();
        let thread = thread::spawn(move || {
            block_on(running);
            Instant::now()
        });
        let pid = loop {
            if let [pid] = self.runtime.helper_processes().as_slice() {
                break *pid;
            }
            assert!(
                started.elapsed() < PROMPTLY,
                "the helper did not start: {:?}",
                self.launcher.view().status
            );
            thread::sleep(Duration::from_millis(5));
        };
        assert!(process_exists(pid), "the system does not list {pid}");
        Pending {
            thread,
            started,
            pid,
        }
    }
}

/// A command whose helper is running.
struct Pending {
    thread: thread::JoinHandle<Instant>,
    started: Instant,
    pid: u32,
}

impl Pending {
    /// Waits for the call to end, and checks it ended well before the
    /// helper would have finished, and that the helper's process is gone:
    /// Pane lists none, and the system has none with its id.
    fn assert_stopped(self, runtime: &Runtime) {
        let ended = self.thread.join().unwrap();
        let took = ended - self.started;
        assert!(took < STOPPED_WITHIN, "the call ran for {took:?}");
        assert_eq!(runtime.helper_processes(), Vec::<u32>::new());
        assert!(
            !process_exists(self.pid),
            "helper process {} is still running",
            self.pid
        );
    }
}

fn titles(launcher: &Launcher) -> Vec<String> {
    launcher
        .view()
        .rows
        .into_iter()
        .map(|row| row.title)
        .collect()
}

fn select_title(launcher: &Launcher, title: &str) {
    let index = titles(launcher)
        .iter()
        .position(|row| row == title)
        .unwrap_or_else(|| panic!("no row {title:?} in {:?}", titles(launcher)));
    launcher.select(index);
}

/// Opens the installed helper sample from root search and selects its item
/// titled `item`.
fn open_sample_at(launcher: &Launcher, item: &str) {
    launcher.back();
    launcher.back();
    select_title(launcher, "Helper sample");
    block_on(launcher.activate_selected());
    assert_eq!(
        launcher.view().screen,
        Screen::Command,
        "{:?}",
        launcher.view()
    );
    select_title(launcher, item);
}

/// "Linux x86-64", as the helper names the system it was built for.
fn this_system() -> String {
    let os = match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        _ => "Linux",
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        _ => "x86-64",
    };
    format!("{os} {arch}")
}

fn error(message: &str) -> Status {
    Status::Error(format!("The extension reported an error: {message}"))
}

#[test]
fn the_helper_for_this_system_answers_and_leaves_no_process() {
    let installed = Installed::new();

    let status = installed.run("Echo through the helper");

    assert_eq!(
        status,
        Status::Result(format!("Echoed \"hello from Pane\" on {}", this_system()))
    );
    assert_eq!(installed.runtime.helper_processes(), Vec::<u32>::new());
}

#[test]
fn installing_copies_only_this_system_s_helper_file_ready_to_run() {
    let installed = Installed::new();

    let location = installed.launcher.packages()[0].location.clone();
    let file = location.join(helper_file());
    assert!(file.is_file(), "{} was not copied", file.display());
    let helpers: Vec<PathBuf> = fs::read_dir(location.join("helpers"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into())
        .collect();
    assert_eq!(helpers, [PathBuf::from(current_target())]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "{mode:o}");
    }
}

#[test]
fn the_package_preview_lists_its_helpers_and_this_system() {
    let sources = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let launcher = Launcher::with_packages(
        Ok(Runtime::start().unwrap()),
        vec![],
        data.path().join("extensions"),
    );
    let targets = ["linux-x86_64", "macos-aarch64", "windows-x86_64"];
    let names = ["Linux x86-64", "macOS arm64", "Windows x86-64"];
    let folder = helper_package(&sources.path().join("helper"), |text| text);

    block_on(launcher.preview_package(&folder));

    let listed: Vec<String> = targets
        .iter()
        .zip(names)
        .map(|(target, name)| {
            if *target == current_target() {
                format!("{name} (this system)")
            } else {
                name.to_owned()
            }
        })
        .collect();
    let expected = format!(
        "Helpers: echo for {}, {} and {}{}",
        listed[0],
        listed[1],
        listed[2],
        if targets.contains(&current_target().as_str()) {
            ""
        } else {
            " (none for this system)"
        }
    );
    let view = launcher.view();
    assert!(view.details().contains(&expected), "{:?}", view.details());
}

#[test]
fn a_failing_helper_is_explained_with_its_exit_code_and_errors() {
    let installed = Installed::new();

    let status = installed.run("Make the helper fail");

    assert_eq!(
        status,
        error("failed: helper `echo` failed (exit code 3): pane-echo was asked to fail")
    );
    assert_eq!(installed.runtime.helper_processes(), Vec::<u32>::new());
}

#[test]
fn an_undeclared_helper_is_explained() {
    let installed = Installed::new();

    let status = installed.run("Run an undeclared helper");

    assert_eq!(
        status,
        error(
            "not-found: Helper sample declares no helper `absent` in its pane.json; \
             it declares `echo`"
        )
    );
}

#[test]
fn a_helper_not_built_for_this_system_is_explained_and_the_rest_works() {
    // The package ships echo only for a target other than this one.
    let other = if current_target() == "windows-aarch64" {
        "linux-aarch64"
    } else {
        "windows-aarch64"
    };
    let other_name = if other == "linux-aarch64" {
        "Linux arm64"
    } else {
        "Windows arm64"
    };
    let file = helper_file();
    let installed = Installed::with_manifest(|text| {
        let echo = text
            .find("\"targets\"")
            .expect("the sample declares targets");
        let end = echo + text[echo..].find('}').unwrap() + 1;
        format!(
            "{}\"targets\": {{ \"{other}\": \"{file}\" }}{}",
            &text[..echo],
            &text[end..]
        )
    });

    let status = installed.run("Echo through the helper");

    assert_eq!(
        status,
        error(&format!(
            "unavailable: Not available on {}: helper `echo` is built only for {other_name}",
            this_system()
        ))
    );
    // The command's other items still work.
    assert_eq!(
        installed.run("Run an undeclared helper"),
        error(
            "not-found: Helper sample declares no helper `absent` in its pane.json; \
             it declares `echo`"
        )
    );
}

/// The first bytes of a program for another system than this one.
fn program_for_another_system() -> (Vec<u8>, &'static str) {
    if cfg!(windows) {
        let mut elf = b"\x7fELF\x02\x01\x01".to_vec();
        elf.resize(18, 0);
        elf.extend_from_slice(&0x3Eu16.to_le_bytes());
        elf.resize(64, 0);
        (elf, "Linux x86-64")
    } else {
        let mut pe = b"MZ".to_vec();
        pe.resize(0x3C, 0);
        pe.extend_from_slice(&0x80u32.to_le_bytes());
        pe.resize(0x80, 0);
        pe.extend_from_slice(b"PE\0\0");
        pe.extend_from_slice(&0x8664u16.to_le_bytes());
        (pe, "Windows x86-64")
    }
}

#[test]
fn a_helper_file_for_another_system_or_missing_is_refused_at_install() {
    let sources = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let launcher = Launcher::with_packages(
        Ok(Runtime::start().unwrap()),
        vec![],
        data.path().join("extensions"),
    );
    let folder = helper_package(&sources.path().join("helper"), |text| text);
    let (program, found) = program_for_another_system();
    fs::write(folder.join(helper_file()), program).unwrap();

    block_on(launcher.install_package(&folder));

    let shown = Path::new(&helper_file()).display().to_string();
    assert_eq!(
        launcher.view().status,
        Status::Error(format!(
            "Not ready to run: the package ships helper `echo` for {}, but its file {shown} \
             is a program for {found}, not {}",
            this_system(),
            this_system()
        ))
    );
    assert!(launcher.packages().is_empty());

    fs::remove_file(folder.join(helper_file())).unwrap();
    block_on(launcher.install_package(&folder));

    assert_eq!(
        launcher.view().status,
        Status::Error(format!(
            "Not ready to run: the package ships helper `echo` for {}, but its file {shown} \
             is missing",
            this_system()
        ))
    );
    assert!(launcher.packages().is_empty());
}

#[test]
fn a_helper_file_replaced_after_install_is_explained_when_run() {
    let installed = Installed::new();
    let location = installed.launcher.packages()[0].location.clone();
    fs::write(location.join(helper_file()), b"not a program").unwrap();

    let status = installed.run("Echo through the helper");

    let shown = Path::new(&helper_file()).display().to_string();
    assert_eq!(
        status,
        error(&format!(
            "unavailable: helper `echo` cannot run: its file {shown} is not a program Pane \
             recognizes for {}",
            this_system()
        ))
    );
}

#[test]
fn cancelling_a_run_ends_the_helper_s_process() {
    let installed = Installed::new();
    let pending = installed.start("Echo within a second");

    pending.assert_stopped(&installed.runtime);

    assert_eq!(
        installed.launcher.view().status,
        Status::Result("Stopped the helper after one second".into())
    );
    // The command runs its helper again at once.
    assert_eq!(
        installed.run("Echo through the helper"),
        Status::Result(format!("Echoed \"hello from Pane\" on {}", this_system()))
    );
}

#[test]
fn disabling_while_the_helper_runs_ends_its_process_and_keeps_saved_data() {
    let installed = Installed::new();
    let pending = installed.start("Echo after waiting");
    assert_eq!(installed.waiting().as_deref(), Some("started"));

    block_on(installed.launcher.set_enabled(&installed.identity, false));
    pending.assert_stopped(&installed.runtime);

    assert_eq!(
        installed.launcher.view().status,
        Status::Result("Disabled Helper sample".into())
    );
    assert_eq!(installed.waiting().as_deref(), Some("started"));
    assert_eq!(block_on(installed.runtime.running()), Vec::<PathBuf>::new());

    block_on(installed.launcher.set_enabled(&installed.identity, true));
    assert_eq!(
        installed.run("Echo through the helper"),
        Status::Result(format!("Echoed \"hello from Pane\" on {}", this_system()))
    );
    assert_eq!(installed.waiting().as_deref(), Some("started"));
}

#[test]
fn reloading_while_the_helper_runs_ends_its_process_and_the_new_code_runs_it() {
    let installed = Installed::new();
    let pending = installed.start("Echo after waiting");

    block_on(installed.launcher.reload(&installed.identity));
    pending.assert_stopped(&installed.runtime);

    assert_eq!(
        installed.launcher.view().status,
        Status::Result("Reloaded Helper sample".into())
    );
    assert_eq!(installed.waiting().as_deref(), Some("started"));
    assert_eq!(
        installed.run("Echo through the helper"),
        Status::Result(format!("Echoed \"hello from Pane\" on {}", this_system()))
    );
}

#[test]
fn updating_while_the_helper_runs_ends_its_process() {
    let installed = Installed::new();
    let pending = installed.start("Echo after waiting");

    block_on(installed.launcher.preview_package(&installed.folder));
    select_title(&installed.launcher, "Update");
    block_on(installed.launcher.activate_selected());
    pending.assert_stopped(&installed.runtime);

    assert_eq!(
        installed.launcher.view().status,
        Status::Result("Updated Helper sample to 0.1.0".into())
    );
    assert_eq!(installed.waiting().as_deref(), Some("started"));
}

#[test]
fn uninstalling_while_the_helper_runs_ends_its_process_and_keeps_saved_data() {
    let installed = Installed::new();
    let pending = installed.start("Echo after waiting");

    block_on(
        installed
            .launcher
            .uninstall(&installed.identity, SavedData::Keep),
    );
    pending.assert_stopped(&installed.runtime);

    assert_eq!(installed.waiting().as_deref(), Some("started"));
    assert_eq!(block_on(installed.runtime.running()), Vec::<PathBuf>::new());
}

#[test]
fn a_helper_declaration_pane_cannot_use_is_an_invalid_manifest() {
    let declared = |helpers: &'static str| {
        move |text: String| {
            let at = text
                .find("\"helpers\"")
                .expect("the sample declares helpers");
            let end = text.rfind(']').unwrap() + 1;
            format!("{}\"helpers\": {helpers}{}", &text[..at], &text[end..])
        }
    };
    for (helpers, explanation) in [
        (
            r#"[{ "id": "echo", "targets": { "linux-arm64": "echo" } }]"#,
            "unknown target `linux-arm64` of helper `echo`; use windows, macos or linux, \
             a dash, and x86_64 or aarch64, such as \"linux-x86_64\"",
        ),
        (
            r#"[{ "id": "echo", "targets": { "linux-x86_64": "../echo" } }]"#,
            "helper file `../echo` must be a relative path inside the package folder",
        ),
        (
            r#"[{ "id": "echo", "targets": {} }]"#,
            "helper `echo` has no `targets`",
        ),
        (
            r#"[{ "id": "echo", "targets": { "linux-x86_64": "a" } },
                { "id": "echo", "targets": { "linux-x86_64": "b" } }]"#,
            "helper id `echo` is repeated",
        ),
        (
            r#"[{ "id": "echo", "file": "echo" }]"#,
            "unknown field `file`",
        ),
    ] {
        let sources = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let launcher = Launcher::with_packages(
            Ok(Runtime::start().unwrap()),
            vec![],
            data.path().join("extensions"),
        );
        let folder = helper_package(&sources.path().join("helper"), declared(helpers));

        block_on(launcher.preview_package(&folder));

        let Status::Error(message) = launcher.view().status else {
            panic!("{helpers}: {:?}", launcher.view().status);
        };
        assert!(message.starts_with("Invalid pane.json: "), "{message}");
        assert!(message.contains(explanation), "{helpers}: {message}");
    }
}

#[test]
fn repeated_stops_leave_no_helper_running() {
    let installed = Installed::new();
    for _ in 0..3 {
        let pending = installed.start("Echo after waiting");
        block_on(installed.launcher.set_enabled(&installed.identity, false));
        pending.assert_stopped(&installed.runtime);
        block_on(installed.launcher.set_enabled(&installed.identity, true));

        let pending = installed.start("Echo after waiting");
        block_on(installed.launcher.reload(&installed.identity));
        pending.assert_stopped(&installed.runtime);

        let pending = installed.start("Echo within a second");
        pending.assert_stopped(&installed.runtime);
    }
    assert_eq!(installed.waiting().as_deref(), Some("started"));
}
