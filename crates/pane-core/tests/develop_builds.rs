//! Development mode with Pane's real builds, on copies of the development
//! samples (`guests/hello-rust`, `guests/hello-js`, `guests/hello-ts`):
//! saving an edit builds the package with its documented command and
//! reloads it, a save that does not build keeps the working code and shows
//! the compiler's diagnostics, and fixing it reloads it again.
//!
//! The Rust test runs `cargo build --release --target wasm32-wasip2` (the
//! pinned toolchain and its `wasm32-wasip2` target, as `cargo xtask guests`
//! needs). The JavaScript and TypeScript tests run
//! `tools/componentize-js/pane_js.py`, which needs the JS toolchain
//! (guests/README.md), so they run only with `PANE_TEST_JS_BUILDS=1`.
//!
//! Each copy is kept in Cargo's test folder between runs, so later runs
//! build incrementally; only its sources are replaced.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::executor::block_on;
use pane_core::develop::{BuildStop, Builder, Toolchains};
use pane_core::{Launcher, PackageIdentity, Runtime, Status};

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn toolchains() -> Toolchains {
    Toolchains::from_env(Some(repository().join("tools/componentize-js/pane_js.py")))
}

/// A development sample: its folder in `guests`, its title, the file
/// holding its greeting and that greeting's declaration.
struct Sample {
    name: &'static str,
    title: &'static str,
    files: &'static [&'static str],
    source: &'static str,
    greeting: &'static str,
}

impl Sample {
    /// A fresh copy of the sample's sources in Cargo's test folder, keeping
    /// what earlier runs built there; for Rust, with the path to
    /// `pane-guest` and the repository's toolchain file.
    fn copy(&self) -> PathBuf {
        let from = repository().join("guests").join(self.name);
        let to = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("develop-{}", self.name));
        let _ = fs::remove_dir_all(to.join("src"));
        fs::create_dir_all(to.join("src")).unwrap();
        for file in self.files {
            fs::copy(from.join(file), to.join(file)).unwrap();
        }
        if to.join("Cargo.toml").exists() {
            let guest = repository().join("guests/pane-guest");
            let manifest = fs::read_to_string(to.join("Cargo.toml")).unwrap().replace(
                r#"path = "../pane-guest""#,
                &format!("path = {:?}", guest.to_str().unwrap()),
            );
            fs::write(to.join("Cargo.toml"), manifest).unwrap();
            fs::copy(
                repository().join("rust-toolchain.toml"),
                to.join("rust-toolchain.toml"),
            )
            .unwrap();
        }
        to.canonicalize().unwrap()
    }

    /// Saves the sample's source with its greeting replaced by `line`.
    fn save(&self, folder: &Path, line: &str) {
        let original = fs::read_to_string(
            repository()
                .join("guests")
                .join(self.name)
                .join(self.source),
        )
        .unwrap();
        assert!(original.contains(self.greeting), "{}", self.greeting);
        fs::write(
            folder.join(self.source),
            original.replace(self.greeting, line),
        )
        .unwrap();
    }
}

const RUST: Sample = Sample {
    name: "hello-rust",
    title: "Hello Rust",
    files: &["Cargo.toml", "Cargo.lock", "pane.json", "src/lib.rs"],
    source: "src/lib.rs",
    greeting: r#"const GREETING: &str = "Hello from Rust";"#,
};

const JAVASCRIPT: Sample = Sample {
    name: "hello-js",
    title: "Hello JavaScript",
    files: &[
        "package.json",
        "package-lock.json",
        "tsconfig.json",
        "pane.json",
        "src/index.js",
    ],
    source: "src/index.js",
    greeting: r#"const GREETING = "Hello from JavaScript";"#,
};

const TYPESCRIPT: Sample = Sample {
    name: "hello-ts",
    title: "Hello TypeScript",
    files: &[
        "package.json",
        "package-lock.json",
        "tsconfig.json",
        "pane.json",
        "src/index.ts",
    ],
    source: "src/index.ts",
    greeting: r#"const GREETING: string = "Hello from TypeScript";"#,
};

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(600);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Opens the sample's command from root search and runs "Say hello".
fn say_hello(launcher: &Launcher, title: &str) -> Status {
    for _ in 0..3 {
        launcher.back();
    }
    let index = launcher
        .view()
        .rows
        .iter()
        .position(|row| row.title == title)
        .unwrap();
    launcher.select(index);
    block_on(launcher.activate_selected());
    let index = launcher
        .view()
        .rows
        .iter()
        .position(|row| row.title == "Say hello")
        .unwrap();
    launcher.select(index);
    block_on(launcher.activate_selected());
    launcher.view().status
}

/// Builds the sample once, installs it, develops it, and saves a change,
/// an edit that does not build, and a fix.
fn develop(sample: &Sample, greeting: &str, broken: &str, again: &str, fixed: &str) {
    let folder = sample.copy();
    sample.save(&folder, sample.greeting);
    let build = toolchains().build_for(&folder).unwrap();
    build.run(&BuildStop::default()).unwrap();

    let data = tempfile::tempdir().unwrap();
    let (changes, _) = pane_core::develop::changes();
    let launcher = Launcher::with_packages(
        Ok(Runtime::start().unwrap()),
        vec![],
        data.path().join("extensions"),
    )
    .with_development(Arc::new(toolchains()), changes);
    block_on(launcher.install_package(&folder));
    let identity = PackageIdentity::local(&folder).unwrap();
    block_on(launcher.start_developing(&identity));
    assert_eq!(
        say_hello(&launcher, sample.title),
        Status::Result(greeting.into())
    );
    let handled = |count: u64| {
        wait_until(&format!("build {count}"), || {
            launcher
                .development(&identity)
                .is_some_and(|development| development.handled >= count)
        })
    };

    sample.save(&folder, &again.replace("{}", "Hello again"));
    handled(1);
    assert_eq!(
        launcher.view().status,
        Status::Result(format!("Reloaded {}", sample.title))
    );
    assert_eq!(
        say_hello(&launcher, sample.title),
        Status::Result("Hello again".into())
    );

    sample.save(&folder, broken);
    handled(2);
    let Status::Error(message) = launcher.view().status else {
        panic!("{:?}", launcher.view().status);
    };
    assert!(
        message.starts_with(&format!("{} did not build: ", sample.title)),
        "{message}"
    );
    assert!(message.contains("error"), "{message}");
    let failure = launcher.development(&identity).unwrap().failure.unwrap();
    // The compiler's diagnostics, then which command failed.
    assert!(failure.contains("error"), "{failure}");
    assert!(failure.contains("` failed (exit code "), "{failure}");
    assert_eq!(
        say_hello(&launcher, sample.title),
        Status::Result("Hello again".into())
    );

    sample.save(&folder, fixed);
    handled(3);
    assert_eq!(
        launcher.view().status,
        Status::Result(format!("Reloaded {}", sample.title))
    );
    assert_eq!(launcher.development(&identity).unwrap().failure, None);
    assert_eq!(
        say_hello(&launcher, sample.title),
        Status::Result("Hello once more".into())
    );
}

#[test]
fn a_rust_package_is_built_with_cargo_and_reloaded_on_save() {
    develop(
        &RUST,
        "Hello from Rust",
        r#"const GREETING: &str = 42;"#,
        r#"const GREETING: &str = "{}";"#,
        r#"const GREETING: &str = "Hello once more";"#,
    );
}

fn js_builds() -> bool {
    let wanted = std::env::var_os("PANE_TEST_JS_BUILDS").is_some_and(|value| value == "1");
    if !wanted {
        eprintln!("skipped: set PANE_TEST_JS_BUILDS=1 to run the JavaScript build");
    }
    wanted
}

#[test]
fn a_javascript_package_is_built_and_reloaded_on_save() {
    if !js_builds() {
        return;
    }
    develop(
        &JAVASCRIPT,
        "Hello from JavaScript",
        r#"const GREETING = 42;"#,
        r#"const GREETING = "{}";"#,
        r#"const GREETING = "Hello once more";"#,
    );
}

#[test]
fn a_typescript_package_is_built_and_reloaded_on_save() {
    if !js_builds() {
        return;
    }
    develop(
        &TYPESCRIPT,
        "Hello from TypeScript",
        r#"const GREETING: string = 42;"#,
        r#"const GREETING: string = "{}";"#,
        r#"const GREETING: string = "Hello once more";"#,
    );
}
