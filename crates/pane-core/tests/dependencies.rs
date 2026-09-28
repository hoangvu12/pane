//! Installing a local package with the other packages it declares under
//! `dependencies`, through the launcher's public interface: the preview
//! lists the required and optional ones before anything is installed,
//! installing adds the missing required ones with it and its code then calls
//! them by dependency id, while optional, disabled and already installed
//! (pinned) dependencies are left as they are, and anything that stops a
//! required one leaves nothing installed. The operations samples
//! (`guests/sample-operations*`) and the operations fixture
//! (`guests/fixtures/operations`) from `cargo xtask guests` serve as
//! packages.

use std::fs;
use std::path::PathBuf;

use futures::executor::block_on;
use pane_core::{Launcher, PackageIdentity, Runtime, SavedData, Status};
use tempfile::TempDir;

#[path = "support/platforms.rs"]
mod platforms;

fn guest(file: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/guests")
        .join(file);
    assert!(
        path.exists(),
        "{} is missing; run `cargo xtask guests`",
        path.display()
    );
    path
}

const RUST: &str = "sample-operations";
const TYPESCRIPT: &str = "sample-operations-ts";

struct Dirs {
    sources: TempDir,
    data: TempDir,
    runtime: Runtime,
}

impl Dirs {
    fn new() -> Dirs {
        Dirs {
            sources: tempfile::tempdir().unwrap(),
            data: tempfile::tempdir().unwrap(),
            runtime: Runtime::start().unwrap(),
        }
    }

    fn folder(&self, name: &str) -> PathBuf {
        self.sources.path().join(name)
    }

    fn launcher(&self) -> Launcher {
        Launcher::with_packages(
            Ok(self.runtime.clone()),
            vec![],
            self.data.path().join("extensions"),
        )
    }

    fn identity(&self, name: &str) -> PackageIdentity {
        PackageIdentity::local(&self.folder(name)).unwrap()
    }

    /// Copies the assembled sample package `package` into source folder
    /// `package`.
    fn sample(&self, package: &str) -> PathBuf {
        let assembled = guest("packages").join(package);
        let folder = self.folder(package);
        fs::create_dir_all(&folder).unwrap();
        for entry in fs::read_dir(assembled).unwrap() {
            let entry = entry.unwrap();
            fs::copy(entry.path(), folder.join(entry.file_name())).unwrap();
        }
        folder
    }

    /// Writes package "Caller" in source folder `caller`: the JavaScript
    /// operations sample's command and component, declaring
    /// `dependencies` (JSON array contents).
    fn caller(&self, dependencies: &str) -> PathBuf {
        let folder = self.folder("caller");
        fs::create_dir_all(&folder).unwrap();
        fs::copy(
            guest("sample_operations_js.wasm"),
            folder.join("caller.wasm"),
        )
        .unwrap();
        let manifest = format!(
            r#"{{
                "manifestVersion": 1,
                "title": "Caller",
                "apiVersion": "0.1",
                "commands": [
                    {{ "id": "call", "title": "Call from JavaScript", "component": "caller.wasm" }}
                ],
                "operations": [{{ "id": "greet", "version": 1, "component": "caller.wasm" }}],
                "dependencies": [{dependencies}]
            }}"#
        );
        fs::write(folder.join("pane.json"), manifest).unwrap();
        folder
    }

    /// Writes a fixture package in source folder `name`, titled
    /// "Package <name>", publishing `echo` at `version` and declaring
    /// `dependencies` (JSON array contents).
    fn fixture(&self, name: &str, version: u32, dependencies: &str) -> PathBuf {
        let folder = self.folder(name);
        fs::create_dir_all(&folder).unwrap();
        fs::copy(
            guest("operations_fixture.wasm"),
            folder.join("fixture.wasm"),
        )
        .unwrap();
        let manifest = format!(
            r#"{{
                "manifestVersion": 1,
                "title": "Package {name}",
                "apiVersion": "0.1",
                "operations": [
                    {{ "id": "echo", "version": {version}, "component": "fixture.wasm" }}
                ],
                "dependencies": [{dependencies}]
            }}"#
        );
        fs::write(folder.join("pane.json"), manifest).unwrap();
        folder
    }
}

/// A dependency on the package in sibling folder `folder` calling `echo`
/// at `version`.
fn needs_echo(id: &str, folder: &str, version: u32) -> String {
    format!(
        r#"{{ "id": "{id}", "source": "local:../{folder}",
             "operations": [{{ "id": "echo", "version": {version} }}] }}"#
    )
}

/// The Rust operations sample's `greet`, as `greeter`, required.
const GREETER: &str = r#"{ "id": "greeter", "source": "local:../sample-operations",
    "operations": [{ "id": "greet", "version": 1 }] }"#;

/// The TypeScript operations sample's `greet`, as `helper`, optional.
const HELPER: &str = r#"{ "id": "helper", "source": "local:../sample-operations-ts",
    "optional": true, "operations": [{ "id": "greet", "version": 1 }] }"#;

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

/// The titles of the installed packages, in the order they were installed.
fn installed(launcher: &Launcher) -> Vec<String> {
    launcher.packages().iter().map(|p| p.title()).collect()
}

/// Calls `greet` from Caller's command with `source` and the name "Ada".
fn greet(launcher: &Launcher, source: &str) -> Status {
    launcher.back();
    launcher.back();
    launcher.back();
    select_title(launcher, "Call from JavaScript");
    block_on(launcher.activate_selected());
    select_title(launcher, "Greet through another extension");
    block_on(launcher.activate_selected());
    assert!(launcher.view().form().is_some(), "no form");
    launcher.set_field_value("source", source);
    launcher.set_field_value("name", "Ada");
    launcher.set_field_value("times", "once");
    block_on(launcher.submit_form());
    launcher.view().status
}

fn result(text: &str) -> Status {
    Status::Result(text.into())
}

/// The error the form shows: the call's error kind and message.
fn error(text: &str) -> Status {
    Status::Error(text.into())
}

fn details(launcher: &Launcher) -> Vec<String> {
    launcher.view().details().to_vec()
}

#[test]
fn the_preview_lists_required_and_optional_dependencies_and_installs_nothing() {
    let dirs = Dirs::new();
    dirs.sample(RUST);
    dirs.sample(TYPESCRIPT);
    let caller = dirs.caller(&format!("{GREETER}, {HELPER}"));
    let launcher = dirs.launcher();

    block_on(launcher.preview_package(&caller));

    let view = launcher.view();
    assert_eq!(view.title, "Caller");
    let details = details(&launcher);
    assert!(
        details.contains(&format!(
            "Requires: Rust operations sample, installed with it from {}",
            dirs.identity(RUST)
        )),
        "{details:#?}"
    );
    assert!(
        details.contains(&format!(
            "Optional: `helper` from {}, not installed: Pane does not install it; install it \
             yourself to use it",
            dirs.identity(TYPESCRIPT)
        )),
        "{details:#?}"
    );
    assert_eq!(titles(&launcher), ["Install"]);
    assert_eq!(
        view.rows[0].subtitle.as_deref(),
        Some(
            "Copy the package into Pane and add its commands, and install Rust operations \
             sample, which it requires"
        )
    );
    // Nothing is installed by looking.
    assert!(launcher.packages().is_empty());
}

#[test]
fn installing_adds_the_missing_required_dependency_whose_operation_the_code_calls_by_id() {
    let dirs = Dirs::new();
    dirs.sample(RUST);
    dirs.sample(TYPESCRIPT);
    let caller = dirs.caller(&format!("{GREETER}, {HELPER}"));
    let launcher = dirs.launcher();
    block_on(launcher.preview_package(&caller));

    block_on(launcher.activate_selected());

    assert_eq!(
        launcher.view().status,
        result("Installed Caller with Rust operations sample, which it requires")
    );
    // The dependency first; the optional one is not installed.
    assert_eq!(installed(&launcher), ["Rust operations sample", "Caller"]);
    // The JavaScript code calls the Rust package by the id its manifest
    // declares.
    assert_eq!(greet(&launcher, "greeter"), result("Hello, Ada, from Rust"));
    assert_eq!(
        greet(&launcher, "helper"),
        error(&format!(
            "not-found: Caller's optional dependency `helper` from {} is not installed; \
             install it to use it",
            dirs.identity(TYPESCRIPT)
        ))
    );
    assert_eq!(
        greet(&launcher, "nobody"),
        error(
            "not-found: Caller declares no dependency `nobody` in its pane.json, and `nobody` \
             is not a package identity; it declares `greeter` and `helper`"
        )
    );

    // The optional one, installed on its own, is used.
    block_on(launcher.install_package(&dirs.folder(TYPESCRIPT)));
    assert_eq!(
        launcher.view().status,
        result("Installed TypeScript operations sample")
    );
    assert_eq!(
        greet(&launcher, "helper"),
        result("Hello, Ada, from TypeScript")
    );

    // Where each dependency resolved is kept: after a restart, even with the
    // source folders gone, the calls still reach them.
    drop(launcher);
    fs::remove_dir_all(dirs.folder(RUST)).unwrap();
    fs::remove_dir_all(dirs.folder("caller")).unwrap();
    let launcher = dirs.launcher();
    assert_eq!(greet(&launcher, "greeter"), result("Hello, Ada, from Rust"));
}

#[test]
fn an_installed_required_dependency_is_used_as_it_is() {
    let dirs = Dirs::new();
    let rust = dirs.sample(RUST);
    let caller = dirs.caller(GREETER);
    let launcher = dirs.launcher();
    block_on(launcher.install_package(&rust));

    block_on(launcher.preview_package(&caller));
    assert!(
        details(&launcher).contains(&"Requires: Rust operations sample, already installed".into()),
        "{:#?}",
        details(&launcher)
    );
    assert_eq!(
        launcher.view().rows[0].subtitle.as_deref(),
        Some("Copy the package into Pane and add its commands")
    );
    block_on(launcher.activate_selected());

    assert_eq!(launcher.view().status, result("Installed Caller"));
    assert_eq!(installed(&launcher), ["Rust operations sample", "Caller"]);
    assert_eq!(greet(&launcher, "greeter"), result("Hello, Ada, from Rust"));
}

#[test]
fn a_disabled_required_dependency_is_not_enabled_again() {
    let dirs = Dirs::new();
    let rust = dirs.sample(RUST);
    let caller = dirs.caller(GREETER);
    let launcher = dirs.launcher();
    block_on(launcher.install_package(&rust));
    block_on(launcher.set_enabled(&dirs.identity(RUST), false));

    block_on(launcher.preview_package(&caller));
    assert!(
        details(&launcher).contains(
            &"Requires: Rust operations sample, which you disabled: it stays disabled, and \
              Caller cannot use it until you enable it in Manage extensions"
                .into()
        ),
        "{:#?}",
        details(&launcher)
    );
    block_on(launcher.activate_selected());

    assert_eq!(
        launcher.view().status,
        result(
            "Installed Caller; Rust operations sample stays disabled: enable it in Manage \
             extensions for Caller to use it"
        )
    );
    let rust = launcher
        .packages()
        .into_iter()
        .find(|p| p.identity == dirs.identity(RUST))
        .unwrap();
    assert!(!rust.enabled);
    assert_eq!(
        greet(&launcher, "greeter"),
        error(
            "disabled: Rust operations sample is disabled; Pane does not enable it for a call, \
             enable it in Manage extensions"
        )
    );
}

#[test]
fn an_installed_copy_that_is_not_compatible_is_kept_and_nothing_is_installed() {
    let dirs = Dirs::new();
    let rust = dirs.sample(RUST);
    let caller = dirs.caller(
        r#"{ "id": "greeter", "source": "local:../sample-operations",
             "operations": [{ "id": "greet", "version": 2 }] }"#,
    );
    let launcher = dirs.launcher();
    block_on(launcher.install_package(&rust));

    let expected = "Nothing was installed: Caller requires `greet` version 2 from Rust \
                    operations sample, which publishes version 1; Pane does not replace the \
                    installed copy of Rust operations sample while installing another \
                    extension: update it from its folder first";
    block_on(launcher.preview_package(&caller));
    let view = launcher.view();
    assert_eq!(view.title, "Cannot install Caller");
    assert!(view.rows.is_empty(), "nothing to choose: {:?}", view.rows);
    assert_eq!(view.status, Status::Error(expected.into()));

    // Installing without the preview is refused the same way.
    block_on(launcher.install_package(&caller));
    assert_eq!(launcher.view().status, Status::Error(expected.into()));
    assert_eq!(installed(&launcher), ["Rust operations sample"]);
}

#[test]
fn conflicting_versions_of_one_source_are_explained_and_nothing_is_installed() {
    let dirs = Dirs::new();
    // a needs b's echo 1 and c; c needs b's echo 2. There is one b.
    dirs.fixture("b", 1, "");
    dirs.fixture("c", 1, &needs_echo("b", "b", 2));
    let a = dirs.fixture(
        "a",
        1,
        &format!("{}, {}", needs_echo("b", "b", 1), needs_echo("c", "c", 1)),
    );
    let launcher = dirs.launcher();

    block_on(launcher.install_package(&a));

    assert_eq!(
        launcher.view().status,
        Status::Error(
            "Nothing was installed: Package a and Package c need different versions of `echo` \
             from Package b (1 and 2); Pane installs one copy of each source, so they conflict"
                .into()
        )
    );
    assert!(launcher.packages().is_empty());
}

/// Writes what a case needs and returns the dependency to declare.
type Setup = Box<dyn Fn(&Dirs) -> String>;

#[test]
fn a_required_dependency_that_cannot_be_installed_leaves_nothing_installed() {
    let [other, _] = platforms::other_systems();
    let cases: [(&str, Setup); 6] = [
        (
            "a missing folder",
            Box::new(|_| needs_echo("b", "missing", 1)),
        ),
        (
            "a source-only package",
            Box::new(|dirs| {
                dirs.fixture("b", 1, "");
                fs::remove_file(dirs.folder("b").join("fixture.wasm")).unwrap();
                needs_echo("b", "b", 1)
            }),
        ),
        (
            "a package for another system",
            Box::new(move |dirs| {
                let folder = dirs.fixture("b", 1, "");
                let manifest = fs::read_to_string(folder.join("pane.json")).unwrap();
                let manifest = manifest.replacen(
                    r#""apiVersion": "0.1","#,
                    &format!(r#""apiVersion": "0.1", "platforms": ["{}"],"#, other.id()),
                    1,
                );
                fs::write(folder.join("pane.json"), manifest).unwrap();
                needs_echo("b", "b", 1)
            }),
        ),
        (
            "an operation it does not publish",
            Box::new(|dirs| {
                dirs.fixture("b", 1, "");
                r#"{ "id": "b", "source": "local:../b",
                     "operations": [{ "id": "secret", "version": 1 }] }"#
                    .into()
            }),
        ),
        (
            "an npm source",
            Box::new(|_| {
                r#"{ "id": "b", "source": "npm:left-pad",
                     "operations": [{ "id": "echo", "version": 1 }] }"#
                    .into()
            }),
        ),
        ("itself", Box::new(|_| needs_echo("b", "a", 1))),
    ];
    for (case, dependency) in cases {
        let dirs = Dirs::new();
        let dependency = dependency(&dirs);
        let a = dirs.fixture("a", 1, &dependency);
        let launcher = dirs.launcher();

        block_on(launcher.preview_package(&a));
        assert_eq!(launcher.view().title, "Cannot install Package a", "{case}");
        let expected = match case {
            "a missing folder" => format!(
                "Package a requires `b` from {}, which cannot be installed: Cannot open",
                dirs.folder("missing").display()
            ),
            "a source-only package" => format!(
                "Package a requires `b` from {}, which cannot be installed: Not ready to run",
                dirs.identity("b").local_folder().unwrap().display()
            ),
            "a package for another system" => format!(
                "Package a requires `b` from {}, which cannot be installed: {}",
                dirs.identity("b").local_folder().unwrap().display(),
                platforms::only("this package", platforms::name(other))
            ),
            "an operation it does not publish" => {
                "Package a requires Package b to publish `secret`, which it does not publish".into()
            }
            "an npm source" => "Package a requires `b` from npm:left-pad: npm sources are not \
                                supported yet; Pane installs only from local folders"
                .into(),
            _ => "Package a names itself as its dependency `b`".into(),
        };
        block_on(launcher.install_package(&a));
        let status = launcher.view().status;
        assert!(
            matches!(&status, Status::Error(text)
                if text.starts_with(&format!("Nothing was installed: {expected}"))),
            "{case}: {status:?}"
        );
        assert!(launcher.packages().is_empty(), "{case}");
    }
}

#[test]
fn packages_requiring_each_other_are_installed_together_once() {
    let dirs = Dirs::new();
    dirs.fixture("b", 1, &needs_echo("a", "a", 1));
    let a = dirs.fixture("a", 1, &needs_echo("b", "b", 1));
    let launcher = dirs.launcher();

    block_on(launcher.install_package(&a));

    assert_eq!(
        launcher.view().status,
        result("Installed Package a with Package b, which it requires")
    );
    assert_eq!(installed(&launcher), ["Package b", "Package a"]);
}

#[test]
fn required_dependencies_of_dependencies_are_installed_first() {
    let dirs = Dirs::new();
    dirs.fixture("c", 1, "");
    dirs.fixture("b", 1, &needs_echo("c", "c", 1));
    let a = dirs.fixture("a", 1, &needs_echo("b", "b", 1));
    let launcher = dirs.launcher();

    block_on(launcher.preview_package(&a));
    assert!(
        details(&launcher).contains(&format!(
            "Requires (for Package b): Package c, installed with it from {}",
            dirs.identity("c")
        )),
        "{:#?}",
        details(&launcher)
    );
    assert_eq!(
        launcher.view().rows[0].subtitle.as_deref(),
        Some(
            "Copy the package into Pane and add its commands, and install the 2 extensions it \
             requires"
        )
    );
    block_on(launcher.activate_selected());

    assert_eq!(
        launcher.view().status,
        result("Installed Package a with Package c and Package b, which it requires")
    );
    assert_eq!(
        installed(&launcher),
        ["Package c", "Package b", "Package a"]
    );
}

#[test]
fn a_dependency_needed_only_on_other_systems_is_not_installed() {
    let dirs = Dirs::new();
    let [other, _] = platforms::other_systems();
    dirs.fixture("b", 1, "");
    let a = dirs.fixture(
        "a",
        1,
        &format!(
            r#"{{ "id": "b", "source": "local:../b", "platforms": ["{}"],
                 "operations": [{{ "id": "echo", "version": 1 }}] }}"#,
            other.id()
        ),
    );
    let launcher = dirs.launcher();

    block_on(launcher.preview_package(&a));
    assert!(
        details(&launcher).contains(&format!(
            "Not needed on this system: `b` from local:../b (only on {})",
            platforms::name(other)
        )),
        "{:#?}",
        details(&launcher)
    );
    block_on(launcher.activate_selected());

    assert_eq!(launcher.view().status, result("Installed Package a"));
    assert_eq!(installed(&launcher), ["Package a"]);
}

#[test]
fn an_update_adding_a_required_dependency_installs_it() {
    let dirs = Dirs::new();
    dirs.sample(RUST);
    let caller = dirs.caller("");
    let launcher = dirs.launcher();
    block_on(launcher.install_package(&caller));
    assert_eq!(installed(&launcher), ["Caller"]);
    assert_eq!(
        greet(&launcher, "greeter"),
        error(
            "not-found: Caller declares no dependency `greeter` in its pane.json, and \
             `greeter` is not a package identity; it declares none"
        )
    );

    dirs.caller(GREETER);
    block_on(launcher.preview_package(&caller));
    assert_eq!(titles(&launcher), ["Update"]);
    block_on(launcher.activate_selected());

    assert_eq!(
        launcher.view().status,
        result("Updated Caller with Rust operations sample, which it requires")
    );
    assert_eq!(installed(&launcher), ["Caller", "Rust operations sample"]);
    assert_eq!(greet(&launcher, "greeter"), result("Hello, Ada, from Rust"));
}

#[test]
fn a_required_dependency_uninstalled_later_is_explained_to_the_caller() {
    let dirs = Dirs::new();
    dirs.sample(RUST);
    let caller = dirs.caller(GREETER);
    let launcher = dirs.launcher();
    block_on(launcher.install_package(&caller));

    block_on(launcher.uninstall(&dirs.identity(RUST), SavedData::Delete));

    assert_eq!(
        greet(&launcher, "greeter"),
        error(&format!(
            "not-found: Caller requires `greeter` from {}, which is not installed; install \
             Caller again to install it",
            dirs.identity(RUST)
        ))
    );
}

#[test]
fn an_invalid_dependency_declaration_is_explained() {
    for (declaration, reason) in [
        (
            r#"{ "id": "Greeter", "source": "local:../b", "operations": [{ "id": "echo", "version": 1 }] }"#,
            "dependency id `Greeter` must be lowercase letters, digits and `-`",
        ),
        (
            r#"{ "id": "b", "source": "../b", "operations": [{ "id": "echo", "version": 1 }] }"#,
            "the source `../b` of dependency `b` must be `local:` followed by a folder path",
        ),
        (
            r#"{ "id": "b", "source": "local:../b", "operations": [] }"#,
            "dependency `b` lists no `operations`; name those the package calls",
        ),
        (
            r#"{ "id": "b", "source": "local:../b", "operations": [{ "id": "echo", "version": 0 }] }"#,
            "every operation of dependency `b` needs an `id` and a `version` from 1",
        ),
    ] {
        let dirs = Dirs::new();
        let a = dirs.fixture("a", 1, declaration);
        let launcher = dirs.launcher();
        block_on(launcher.preview_package(&a));
        assert_eq!(
            launcher.view().status,
            Status::Error(format!("Invalid pane.json: {reason}"))
        );
    }
}

/// From root search, opens the command titled `command` and runs its item
/// titled `item`.
fn run(launcher: &Launcher, command: &str, item: &str) -> Status {
    launcher.back();
    launcher.back();
    select_title(launcher, command);
    block_on(launcher.activate_selected());
    select_title(launcher, item);
    block_on(launcher.activate_selected());
    launcher.view().status
}

#[test]
fn the_dependencies_sample_installs_its_required_greeter_and_uses_the_optional_one_once_installed()
{
    let dirs = Dirs::new();
    let sample = dirs.sample("sample-dependencies");
    dirs.sample("sample-operations-js");
    let rust = dirs.sample(RUST);
    let launcher = dirs.launcher();

    block_on(launcher.install_package(&sample));

    assert_eq!(
        launcher.view().status,
        result(
            "Installed Dependencies sample with JavaScript operations sample, which it requires"
        )
    );
    let command = "Greet through dependencies";
    assert_eq!(
        run(&launcher, command, "Greet through the required greeter"),
        result("Hello, Pane, from JavaScript")
    );
    assert_eq!(
        run(&launcher, command, "Greet through the optional greeter"),
        result(
            "The optional Rust greeter is not installed; install the Rust operations sample to \
             use it"
        )
    );

    block_on(launcher.install_package(&rust));
    assert_eq!(
        run(&launcher, command, "Greet through the optional greeter"),
        result("Hello, Pane, from Rust")
    );
}
