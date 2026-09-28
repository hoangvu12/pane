//! Running a helper's process: what a helper's file is (read from its
//! header), and a supervising thread per run that feeds its input, collects
//! its output and ends and reaps the process when asked, when its
//! generation ends or when it writes too much. Nothing here depends on the
//! extension runtime, so it is checked for every system on its own.

use std::fmt;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, mpsc};
use std::thread::{self, Thread};
use std::time::Duration;

use tokio::sync::oneshot;

use crate::generation::{End, Generation};
use crate::platform::{self, Platform};

/// The largest input a helper run takes, in bytes.
pub const MAX_HELPER_INPUT: usize = 1 << 20;

/// The largest output (standard output) a helper run passes back, in
/// bytes; a helper writing more is stopped.
pub const MAX_HELPER_OUTPUT: usize = 1 << 20;

/// The most arguments a helper run takes.
const MAX_HELPER_ARGS: usize = 64;

/// The most bytes of all arguments together.
const MAX_HELPER_ARGS_BYTES: usize = 64 << 10;

/// How much of the end of a failed helper's standard error its error shows.
const STDERR_KEPT: usize = 2048;

/// How often a supervising thread checks its process when nothing wakes it.
const TICK: Duration = Duration::from_millis(10);

/// How long a helper's output may stay open after its process exited (held
/// by a process it started) before the run gives up on it.
const OUTPUT_GRACE: Duration = Duration::from_secs(1);

/// How long ending a process waits for its supervising thread to reap it.
const REAP_WAIT: Duration = Duration::from_secs(5);

/// The processors a helper target may name.
const ARCHS: [&str; 2] = ["x86_64", "aarch64"];

/// The target of this Pane: its operating system and processor, such as
/// `linux-x86_64`, in the form a package's `helpers` entries use.
pub fn current_target() -> String {
    let os = Platform::current().map_or(std::env::consts::OS, Platform::id);
    format!("{os}-{}", std::env::consts::ARCH)
}

/// Whether `target` is `<os>-<arch>` with a system Pane runs on and a
/// processor it knows.
pub(crate) fn is_known_target(target: &str) -> bool {
    target
        .split_once('-')
        .is_some_and(|(os, arch)| Platform::from_id(os).is_some() && ARCHS.contains(&arch))
}

/// People's name for `target`: "Linux x86-64", "macOS arm64".
pub(crate) fn target_name(target: &str) -> String {
    let Some((os, arch)) = target.split_once('-') else {
        return target.to_owned();
    };
    let os = Platform::from_id(os).map_or_else(|| os.to_owned(), |os| os.to_string());
    format!("{os} {}", arch_name(arch))
}

fn arch_name(arch: &str) -> &str {
    match arch {
        "x86_64" => "x86-64",
        "aarch64" => "arm64",
        other => other,
    }
}

/// What a file is, as far as starting it goes, from its first bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Program {
    /// A native program for `os` (a [`Platform`] id) on each of `archs`
    /// (more than one for a macOS universal program).
    Native {
        os: &'static str,
        archs: Vec<&'static str>,
    },
    /// A script starting with `#!`, which macOS and Linux run with the
    /// interpreter it names.
    Script,
    /// Not a program Pane recognizes.
    Unknown,
}

/// Reads what kind of program `path` is from its header.
fn program(path: &Path) -> io::Result<Program> {
    let mut header = Vec::new();
    std::fs::File::open(path)?
        .take(4096)
        .read_to_end(&mut header)?;
    Ok(program_of(&header))
}

fn program_of(header: &[u8]) -> Program {
    let u16_le = |at: usize| {
        header
            .get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    let u32_le = |at: usize| {
        header
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let u32_be = |at: usize| {
        header
            .get(at..at + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };
    let native = |os, archs: Vec<Option<&'static str>>| {
        let archs: Vec<&'static str> = archs.into_iter().flatten().collect();
        if archs.is_empty() {
            Program::Unknown
        } else {
            Program::Native { os, archs }
        }
    };
    let mach_arch = |cpu: u32| match cpu {
        0x0100_0007 => Some("x86_64"),
        0x0100_000C => Some("aarch64"),
        _ => None,
    };
    if header.starts_with(b"#!") {
        return Program::Script;
    }
    if header.starts_with(b"\x7fELF") {
        let arch = match u16_le(18) {
            Some(0x3E) => Some("x86_64"),
            Some(0xB7) => Some("aarch64"),
            _ => None,
        };
        return native("linux", vec![arch]);
    }
    if header.starts_with(&[0xCF, 0xFA, 0xED, 0xFE]) {
        return native("macos", vec![u32_le(4).and_then(mach_arch)]);
    }
    if u32_be(0) == Some(0xCAFE_BABE) {
        let count = u32_be(4).unwrap_or(0).min(16) as usize;
        let archs = (0..count)
            .map(|i| u32_be(8 + i * 20).and_then(mach_arch))
            .collect();
        return native("macos", archs);
    }
    if header.starts_with(b"MZ")
        && let Some(pe) = u32_le(0x3C).map(|at| at as usize)
        && header.get(pe..pe + 4) == Some(b"PE\0\0")
    {
        let arch = match u16_le(pe + 4) {
            Some(0x8664) => Some("x86_64"),
            Some(0xAA64) => Some("aarch64"),
            _ => None,
        };
        return native("windows", vec![arch]);
    }
    Program::Unknown
}

/// Why the file at `path` (named `shown` in explanations) cannot run as the
/// helper for `target`, if it cannot: it is missing, unreadable, or a
/// program for another system.
pub(crate) fn unfit(path: &Path, shown: &Path, target: &str) -> Option<String> {
    let shown = shown.display();
    let kind = match program(path) {
        Ok(kind) => kind,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Some(format!("its file {shown} is missing"));
        }
        Err(error) => return Some(format!("its file {shown} cannot be read: {error}")),
    };
    let (os, arch) = target.split_once('-').unwrap_or((target, ""));
    match kind {
        Program::Native {
            os: found,
            ref archs,
        } if found == os && archs.contains(&arch) => None,
        Program::Native { os: found, archs } => {
            let archs: Vec<String> = archs.iter().map(|a| arch_name(a).to_owned()).collect();
            let found = Platform::from_id(found).map_or_else(String::new, |os| os.to_string());
            Some(format!(
                "its file {shown} is a program for {found} {}, not {}",
                platform::join(&archs),
                target_name(target)
            ))
        }
        Program::Script if os != Platform::Windows.id() => None,
        Program::Script => Some(format!(
            "its file {shown} is a script, which Windows does not run by itself"
        )),
        Program::Unknown => Some(format!(
            "its file {shown} is not a program Pane recognizes for {}",
            target_name(target)
        )),
    }
}

/// Why a helper run did not produce the helper's output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HelperError {
    pub kind: HelperErrorKind,
    pub message: String,
}

/// The WIT `helper-error-kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HelperErrorKind {
    NotFound,
    Unavailable,
    Failed,
    Refused,
}

impl HelperError {
    pub(crate) fn new(kind: HelperErrorKind, message: impl Into<String>) -> HelperError {
        HelperError {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for HelperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// Why stopped code may not start a helper.
pub(crate) fn stopped_code(end: End) -> HelperError {
    let why = match end {
        End::Disabled => "the extension is disabled",
        End::Replaced => "this code of the extension was replaced by a reload or an update",
        End::Uninstalled => "the extension was uninstalled",
        End::Paused => "the extension is paused after an error",
    };
    HelperError::new(
        HelperErrorKind::Refused,
        format!("{why}; its helpers do not run"),
    )
}

/// Refuses input or arguments over Pane's limits.
pub(crate) fn check_limits(args: &[String], input: &str) -> Result<(), HelperError> {
    let refused = |message: String| HelperError::new(HelperErrorKind::Refused, message);
    if input.len() > MAX_HELPER_INPUT {
        return Err(refused(format!(
            "the input is {} bytes; at most {MAX_HELPER_INPUT} are passed to a helper",
            input.len()
        )));
    }
    if args.len() > MAX_HELPER_ARGS {
        return Err(refused(format!(
            "{} arguments were given; a helper takes at most {MAX_HELPER_ARGS}",
            args.len()
        )));
    }
    let bytes: usize = args.iter().map(String::len).sum();
    if bytes > MAX_HELPER_ARGS_BYTES {
        return Err(refused(format!(
            "the arguments are {bytes} bytes; at most {MAX_HELPER_ARGS_BYTES} are passed"
        )));
    }
    if args.iter().any(|arg| arg.contains('\0')) {
        return Err(refused("an argument contains a NUL character".into()));
    }
    Ok(())
}

/// What to run: a helper's file, its arguments and input, and whose it is.
pub(crate) struct Spec {
    /// The helper's name, for explanations.
    pub name: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub input: String,
    /// The generation of the code running it: when it ends, so does the
    /// process.
    pub generation: Option<Generation>,
    /// The guest instance that started it (see [`Helpers::stop_owned_by`]).
    pub owner: u64,
}

/// The helper processes of one runtime, for stopping and diagnostics.
/// Cloning shares them.
#[derive(Clone, Default)]
pub(crate) struct Helpers {
    runs: Arc<Mutex<Vec<Arc<Run>>>>,
    next_owner: Arc<AtomicU64>,
}

/// One helper process and the thread supervising it.
struct Run {
    owner: u64,
    pid: u32,
    /// Set to end the process.
    stop: AtomicBool,
    /// The thread supervising the process, once it runs.
    supervisor: OnceLock<Thread>,
    /// Set once the process has been reaped.
    done: Mutex<bool>,
    reaped: Condvar,
}

impl Run {
    /// Asks the supervisor to end the process, without waiting.
    fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(supervisor) = self.supervisor.get() {
            supervisor.unpark();
        }
    }

    /// Ends the process and waits (briefly) until it has been reaped.
    fn stop_and_wait(&self) {
        self.request_stop();
        let done = self.done.lock().unwrap_or_else(|p| p.into_inner());
        let _ = self
            .reaped
            .wait_timeout_while(done, REAP_WAIT, |done| !*done);
    }
}

impl Helpers {
    /// A fresh owner id for a guest instance; never reused.
    pub fn new_owner(&self) -> u64 {
        self.next_owner.fetch_add(1, Ordering::Relaxed)
    }

    /// The process ids of the helpers running now, not yet reaped. A
    /// diagnostic for tests and logs.
    pub fn running(&self) -> Vec<u32> {
        self.lock().iter().map(|run| run.pid).collect()
    }

    /// Ends every helper process `owner` started, waiting until each is
    /// reaped.
    pub fn stop_owned_by(&self, owner: u64) {
        let owned: Vec<Arc<Run>> = self
            .lock()
            .iter()
            .filter(|run| run.owner == owner)
            .cloned()
            .collect();
        for run in owned {
            run.stop_and_wait();
        }
    }

    /// Ends every helper process, waiting until each is reaped.
    pub fn stop_all(&self) {
        let all: Vec<Arc<Run>> = self.lock().clone();
        for run in all {
            run.stop_and_wait();
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Arc<Run>>> {
        self.runs.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Starts the helper of `spec`. The returned run ends the process when
    /// it is dropped before it finishes.
    pub fn start(&self, spec: Spec) -> Result<Running, HelperError> {
        let name = spec.name.clone();
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = spec.program.parent() {
            command.current_dir(dir);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // No console window flashes up for a helper of a GUI launcher.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let child = Owned(command.spawn().map_err(|error| {
            HelperError::new(
                HelperErrorKind::Unavailable,
                format!("the system did not start helper `{name}`: {error}"),
            )
        })?);
        let run = Arc::new(Run {
            owner: spec.owner,
            pid: child.0.id(),
            stop: AtomicBool::new(false),
            supervisor: OnceLock::new(),
            done: Mutex::new(false),
            reaped: Condvar::new(),
        });
        self.lock().push(run.clone());
        let (reply, result) = oneshot::channel();
        let supervised = {
            let (helpers, run) = (self.clone(), run.clone());
            move || {
                let outcome = supervise(child, &spec, &run);
                helpers.lock().retain(|other| !Arc::ptr_eq(other, &run));
                *run.done.lock().unwrap_or_else(|p| p.into_inner()) = true;
                run.reaped.notify_all();
                let _ = reply.send(outcome);
            }
        };
        match thread::Builder::new()
            .name(format!("pane-helper-{name}"))
            .spawn(supervised)
        {
            Ok(supervisor) => {
                let _ = run.supervisor.set(supervisor.thread().clone());
            }
            Err(error) => {
                // The process went with the closure, ended by `Owned`.
                self.lock().retain(|other| !Arc::ptr_eq(other, &run));
                return Err(HelperError::new(
                    HelperErrorKind::Unavailable,
                    format!("Pane could not supervise helper `{name}`: {error}"),
                ));
            }
        }
        Ok(Running {
            run: Some(run),
            result,
            name,
        })
    }
}

/// A child process that is ended and reaped when dropped, unless it was
/// reaped already.
struct Owned(Child);

impl Drop for Owned {
    fn drop(&mut self) {
        if let Ok(None) = self.0.try_wait() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

/// A helper run in progress. Dropping it before it finishes ends the
/// process: the guest cancelled the call, or its instance went.
pub(crate) struct Running {
    run: Option<Arc<Run>>,
    result: oneshot::Receiver<Result<String, HelperError>>,
    name: String,
}

impl Running {
    /// The helper's output once it has exited, or why there is none.
    pub async fn finish(mut self) -> Result<String, HelperError> {
        let result = (&mut self.result).await;
        // Finished: nothing to end on drop.
        self.run = None;
        result.unwrap_or_else(|_| {
            Err(HelperError::new(
                HelperErrorKind::Failed,
                format!("helper `{}` ended without an answer", self.name),
            ))
        })
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(run) = self.run.take() {
            run.stop_and_wait();
        }
    }
}

/// Why a supervised process was ended before it exited by itself.
enum Ended {
    Generation(End),
    Stopped,
    TooMuchOutput,
}

/// Feeds the process its input, collects its output, and ends it when asked
/// to, when its generation ends or when it writes too much; then reaps it.
fn supervise(child: Owned, spec: &Spec, run: &Run) -> Result<String, HelperError> {
    let mut child = child;
    let child = &mut child.0;
    let name = &spec.name;
    let supervisor = thread::current();
    if let Some(mut stdin) = child.stdin.take() {
        let input = spec.input.clone();
        // A helper that exits without reading its input is not an error.
        thread::spawn(move || {
            let _ = stdin.write_all(input.as_bytes());
        });
    }
    let too_much = Arc::new(AtomicBool::new(false));
    let (out_tx, out_rx) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        let (too_much, supervisor) = (too_much.clone(), supervisor.clone());
        thread::spawn(move || {
            let mut output = Vec::new();
            let read = stdout
                .take(MAX_HELPER_OUTPUT as u64 + 1)
                .read_to_end(&mut output);
            if output.len() > MAX_HELPER_OUTPUT {
                too_much.store(true, Ordering::SeqCst);
                supervisor.unpark();
            }
            let _ = out_tx.send(read.map(|_| output));
        });
    }
    let (err_tx, err_rx) = mpsc::channel();
    if let Some(mut stderr) = child.stderr.take() {
        thread::spawn(move || {
            let mut kept = Vec::new();
            let mut buffer = [0u8; 4096];
            while let Ok(read) = stderr.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                kept.extend_from_slice(&buffer[..read]);
                if kept.len() > 2 * STDERR_KEPT {
                    kept.drain(..kept.len() - STDERR_KEPT);
                }
            }
            let _ = err_tx.send(kept);
        });
    }
    let status: Result<ExitStatus, Ended> = loop {
        let ended = if run.stop.load(Ordering::SeqCst) {
            Some(Ended::Stopped)
        } else if let Some(end) = spec.generation.as_ref().and_then(Generation::ended) {
            Some(Ended::Generation(end))
        } else if too_much.load(Ordering::SeqCst) {
            Some(Ended::TooMuchOutput)
        } else {
            None
        };
        if let Some(ended) = ended {
            let _ = child.kill();
            let _ = child.wait();
            break Err(ended);
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => thread::park_timeout(TICK),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(Ended::Stopped);
            }
        }
    };
    let status = match status {
        Ok(status) => status,
        Err(Ended::Generation(end)) => return Err(stopped_code(end)),
        Err(Ended::Stopped) => {
            return Err(HelperError::new(
                HelperErrorKind::Refused,
                format!("helper `{name}` was stopped before it finished"),
            ));
        }
        Err(Ended::TooMuchOutput) => {
            return Err(HelperError::new(
                HelperErrorKind::Refused,
                format!(
                    "helper `{name}` wrote more than {MAX_HELPER_OUTPUT} bytes of output; \
                     Pane stopped it"
                ),
            ));
        }
    };
    let held = || {
        HelperError::new(
            HelperErrorKind::Failed,
            format!("helper `{name}` exited, but a process it started still holds its output open"),
        )
    };
    let output = match out_rx.recv_timeout(OUTPUT_GRACE) {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            return Err(HelperError::new(
                HelperErrorKind::Failed,
                format!("reading the output of helper `{name}` failed: {error}"),
            ));
        }
        Err(mpsc::RecvTimeoutError::Timeout) => return Err(held()),
        Err(mpsc::RecvTimeoutError::Disconnected) => Vec::new(),
    };
    if too_much.load(Ordering::SeqCst) {
        return Err(HelperError::new(
            HelperErrorKind::Refused,
            format!("helper `{name}` wrote more than {MAX_HELPER_OUTPUT} bytes of output"),
        ));
    }
    if !status.success() {
        let errors = err_rx.recv_timeout(OUTPUT_GRACE).unwrap_or_default();
        let errors = String::from_utf8_lossy(&errors);
        let errors = errors.trim();
        let tail = match errors.char_indices().rev().nth(STDERR_KEPT) {
            Some((at, _)) => &errors[at..],
            None => errors,
        };
        let status = describe_status(status);
        return Err(HelperError::new(
            HelperErrorKind::Failed,
            if tail.is_empty() {
                format!("helper `{name}` failed ({status})")
            } else {
                format!("helper `{name}` failed ({status}): {tail}")
            },
        ));
    }
    String::from_utf8(output).map_err(|_| {
        HelperError::new(
            HelperErrorKind::Failed,
            format!("helper `{name}` wrote output that is not UTF-8 text"),
        )
    })
}

/// "exit code 3", or how the system ended the process.
fn describe_status(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("exit code {code}"),
        None => status.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_name_a_known_system_and_processor() {
        assert!(is_known_target("linux-x86_64"));
        assert!(is_known_target("macos-aarch64"));
        assert!(is_known_target("windows-x86_64"));
        assert!(!is_known_target("linux-arm64"));
        assert!(!is_known_target("freebsd-x86_64"));
        assert!(!is_known_target("linux"));
        assert!(is_known_target(&current_target()) || Platform::current().is_none());
        assert_eq!(target_name("macos-aarch64"), "macOS arm64");
        assert_eq!(target_name("windows-x86_64"), "Windows x86-64");
    }

    fn elf(machine: u16) -> Vec<u8> {
        let mut header = b"\x7fELF\x02\x01\x01".to_vec();
        header.resize(18, 0);
        header.extend_from_slice(&machine.to_le_bytes());
        header.resize(64, 0);
        header
    }

    fn mach_o(cpu: u32) -> Vec<u8> {
        let mut header = vec![0xCF, 0xFA, 0xED, 0xFE];
        header.extend_from_slice(&cpu.to_le_bytes());
        header.resize(32, 0);
        header
    }

    fn universal(cpus: &[u32]) -> Vec<u8> {
        let mut header = 0xCAFE_BABEu32.to_be_bytes().to_vec();
        header.extend_from_slice(&(cpus.len() as u32).to_be_bytes());
        for cpu in cpus {
            let mut arch = cpu.to_be_bytes().to_vec();
            arch.resize(20, 0);
            header.extend_from_slice(&arch);
        }
        header
    }

    fn pe(machine: u16) -> Vec<u8> {
        let mut header = b"MZ".to_vec();
        header.resize(0x3C, 0);
        header.extend_from_slice(&0x80u32.to_le_bytes());
        header.resize(0x80, 0);
        header.extend_from_slice(b"PE\0\0");
        header.extend_from_slice(&machine.to_le_bytes());
        header
    }

    #[test]
    fn a_program_s_system_and_processor_are_read_from_its_header() {
        let native = |os, archs: &[&'static str]| Program::Native {
            os,
            archs: archs.to_vec(),
        };
        assert_eq!(program_of(&elf(0x3E)), native("linux", &["x86_64"]));
        assert_eq!(program_of(&elf(0xB7)), native("linux", &["aarch64"]));
        assert_eq!(
            program_of(&mach_o(0x0100_000C)),
            native("macos", &["aarch64"])
        );
        assert_eq!(
            program_of(&universal(&[0x0100_0007, 0x0100_000C])),
            native("macos", &["x86_64", "aarch64"])
        );
        assert_eq!(program_of(&pe(0x8664)), native("windows", &["x86_64"]));
        assert_eq!(program_of(&pe(0xAA64)), native("windows", &["aarch64"]));
        assert_eq!(program_of(b"#!/bin/sh\necho hi\n"), Program::Script);
        assert_eq!(program_of(b"hello"), Program::Unknown);
        assert_eq!(program_of(&elf(0x28)), Program::Unknown);
        assert_eq!(program_of(b"MZ"), Program::Unknown);
    }

    #[test]
    fn a_file_for_another_target_is_explained() {
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str, bytes: &[u8]| {
            let path = dir.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        };
        let shown = Path::new("helpers/linux-x86_64/echo");
        let linux = write("linux", &elf(0x3E));
        assert_eq!(unfit(&linux, shown, "linux-x86_64"), None);
        assert_eq!(
            unfit(&linux, shown, "linux-aarch64").as_deref(),
            Some(
                "its file helpers/linux-x86_64/echo is a program for Linux x86-64, \
                 not Linux arm64"
            )
        );
        let windows = write("windows", &pe(0x8664));
        assert_eq!(
            unfit(&windows, shown, "linux-x86_64").as_deref(),
            Some(
                "its file helpers/linux-x86_64/echo is a program for Windows x86-64, \
                 not Linux x86-64"
            )
        );
        let fat = write("fat", &universal(&[0x0100_0007, 0x0100_000C]));
        assert_eq!(unfit(&fat, shown, "macos-aarch64"), None);
        assert_eq!(unfit(&fat, shown, "macos-x86_64"), None);
        let script = write("script", b"#!/bin/sh\n");
        assert_eq!(unfit(&script, shown, "macos-aarch64"), None);
        assert!(
            unfit(&script, shown, "windows-x86_64")
                .unwrap()
                .contains("is a script")
        );
        let text = write("text", b"not a program");
        assert_eq!(
            unfit(&text, shown, "linux-x86_64").as_deref(),
            Some(
                "its file helpers/linux-x86_64/echo is not a program Pane recognizes \
                 for Linux x86-64"
            )
        );
        assert_eq!(
            unfit(&dir.path().join("absent"), shown, "linux-x86_64").as_deref(),
            Some("its file helpers/linux-x86_64/echo is missing")
        );
    }

    /// The helper sample's `pane-echo`, built for this system by `cargo
    /// xtask guests`.
    fn echo() -> PathBuf {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/guests/packages/sample-helper/helpers")
            .join(current_target())
            .join(format!("pane-echo{}", std::env::consts::EXE_SUFFIX));
        assert!(
            path.exists(),
            "{} is missing; run `cargo xtask guests`",
            path.display()
        );
        path
    }

    fn spec(args: &[&str], generation: Option<Generation>, owner: u64) -> Spec {
        Spec {
            name: "echo".into(),
            program: echo(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            input: "hi".into(),
            generation,
            owner,
        }
    }

    /// Waits until `helpers` runs no process, or panics.
    fn until_none_run(helpers: &Helpers) {
        let started = std::time::Instant::now();
        while !helpers.running().is_empty() {
            assert!(started.elapsed() < Duration::from_secs(5), "still running");
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn a_run_answers_with_the_helper_s_output() {
        let helpers = Helpers::default();

        let running = helpers.start(spec(&[], None, 0)).unwrap();

        let answer = futures::executor::block_on(running.finish()).unwrap();
        assert!(answer.starts_with("Echoed \"hi\" on "), "{answer}");
        assert_eq!(helpers.running(), Vec::<u32>::new());
    }

    #[test]
    fn dropping_a_run_ends_its_process() {
        let helpers = Helpers::default();
        let running = helpers.start(spec(&["--wait", "10"], None, 0)).unwrap();
        assert_eq!(helpers.running().len(), 1);

        drop(running);

        assert_eq!(helpers.running(), Vec::<u32>::new());
    }

    /// No one polls the run: its supervising thread sees the generation end
    /// by itself, as it must while the runtime thread is busy in a guest.
    #[test]
    fn a_generation_ending_ends_its_helpers_without_waiting_for_the_runtime() {
        let helpers = Helpers::default();
        let generation = Generation::new();
        let running = helpers
            .start(spec(&["--wait", "10"], Some(generation.clone()), 0))
            .unwrap();

        generation.end(End::Disabled);
        until_none_run(&helpers);

        assert_eq!(
            futures::executor::block_on(running.finish()),
            Err(stopped_code(End::Disabled))
        );
    }

    #[test]
    fn stopping_an_owner_s_helpers_leaves_the_others_running() {
        let helpers = Helpers::default();
        let (first, second) = (helpers.new_owner(), helpers.new_owner());
        let _mine = helpers.start(spec(&["--wait", "10"], None, first)).unwrap();
        let theirs = helpers
            .start(spec(&["--wait", "10"], None, second))
            .unwrap();

        helpers.stop_owned_by(first);

        assert_eq!(helpers.running().len(), 1);
        drop(theirs);
        assert_eq!(helpers.running(), Vec::<u32>::new());
    }

    #[test]
    fn stopping_all_ends_every_helper() {
        let helpers = Helpers::default();
        let (first, second) = (helpers.new_owner(), helpers.new_owner());
        let mine = helpers.start(spec(&["--wait", "10"], None, first)).unwrap();
        let theirs = helpers
            .start(spec(&["--wait", "10"], None, second))
            .unwrap();

        helpers.stop_all();

        assert_eq!(helpers.running(), Vec::<u32>::new());
        let stopped = futures::executor::block_on(mine.finish()).unwrap_err();
        assert_eq!(stopped.kind, HelperErrorKind::Refused);
        assert_eq!(
            stopped.message,
            "helper `echo` was stopped before it finished"
        );
        drop(theirs);
    }

    #[test]
    fn a_helper_writing_too_much_is_stopped() {
        let helpers = Helpers::default();

        let running = helpers.start(spec(&["--flood"], None, 0)).unwrap();

        let error = futures::executor::block_on(running.finish()).unwrap_err();
        assert_eq!(error.kind, HelperErrorKind::Refused);
        assert!(error.message.contains("more than"), "{}", error.message);
    }

    #[test]
    fn a_program_that_cannot_start_is_unavailable() {
        let helpers = Helpers::default();
        let mut missing = spec(&[], None, 0);
        missing.program = PathBuf::from("/nonexistent/pane-echo");

        let error = helpers.start(missing).err().unwrap();

        assert_eq!(error.kind, HelperErrorKind::Unavailable);
        assert!(
            error
                .message
                .starts_with("the system did not start helper `echo`: "),
            "{}",
            error.message
        );
    }

    #[test]
    fn inputs_over_the_limits_are_refused() {
        assert!(check_limits(&[], "hello").is_ok());
        let big = "x".repeat(MAX_HELPER_INPUT + 1);
        assert_eq!(
            check_limits(&[], &big).unwrap_err().kind,
            HelperErrorKind::Refused
        );
        let many = vec![String::new(); MAX_HELPER_ARGS + 1];
        assert!(check_limits(&many, "").is_err());
        assert!(check_limits(&["a\0b".into()], "").is_err());
    }
}
