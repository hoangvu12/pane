//! Searching an online service inside its command, through the launcher's
//! public interface: the user opens Package search, the search sample, and
//! types into its own search field; the command asks a web service through
//! `wasi:http` and Pane lists what it found. The command is a real guest
//! (`cargo xtask guests`), in Rust, JavaScript and TypeScript alike, and the
//! service is the fixture service, a made-up package registry each test
//! serves on a free port of 127.0.0.1: nothing here reaches the network
//! beyond this computer.

#[path = "support/service.rs"]
mod service;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures::executor::block_on;
use pane_core::{Launcher, Runtime, Screen, Status};
use service::Service;
use tempfile::TempDir;

struct Fixture {
    /// The assembled package under `target/guests/packages`.
    package: &'static str,
}

const RUST: Fixture = Fixture {
    package: "sample-search",
};

const JAVASCRIPT: Fixture = Fixture {
    package: "sample-search-js",
};
const TYPESCRIPT: Fixture = Fixture {
    package: "sample-search-ts",
};

const ALL: [Fixture; 3] = [RUST, JAVASCRIPT, TYPESCRIPT];

const COMMAND: &str = "Package search";

/// Copies the assembled package `name` under `target/guests/packages` to
/// `folder`.
fn package(name: &str, folder: &Path) -> PathBuf {
    let assembled = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/guests/packages")
        .join(name);
    assert!(
        assembled.exists(),
        "{} is missing; run `cargo xtask guests`",
        assembled.display()
    );
    fs::create_dir_all(folder).unwrap();
    for entry in fs::read_dir(&assembled).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), folder.join(entry.file_name())).unwrap();
    }
    folder.to_path_buf()
}

/// A launcher with the search sample `fixture` installed, and the folders
/// it keeps its data and the package's source in.
struct Pane {
    launcher: Launcher,
    _sources: TempDir,
    _data: TempDir,
}

impl Pane {
    fn with(fixture: &Fixture) -> Pane {
        let sources = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let launcher =
            Launcher::with_packages(Runtime::start(), vec![], data.path().join("extensions"));
        let folder = package(fixture.package, &sources.path().join(fixture.package));
        block_on(launcher.install_package(&folder));
        assert!(
            matches!(launcher.view().status, Status::Result(_)),
            "{:?}",
            launcher.view().status
        );
        Pane {
            launcher,
            _sources: sources,
            _data: data,
        }
    }

    fn view(&self) -> pane_core::LauncherView {
        self.launcher.view()
    }

    fn titles(&self) -> Vec<String> {
        self.view().rows.into_iter().map(|row| row.title).collect()
    }

    fn activate(&self, title: &str) {
        let index = self
            .titles()
            .iter()
            .position(|row| row == title)
            .unwrap_or_else(|| panic!("no row {title:?} in {:?}", self.titles()));
        self.launcher.select(index);
        block_on(self.launcher.activate_selected());
    }

    fn to_root(&self) {
        while !matches!(self.view().screen, Screen::Root { .. }) {
            self.launcher.back();
        }
        block_on(self.launcher.set_query(""));
    }

    /// Opens Package search from root search.
    fn open(&self) {
        self.to_root();
        block_on(self.launcher.set_query("package search"));
        self.activate(COMMAND);
        assert_eq!(
            self.view().screen,
            Screen::CommandSearch {
                query: String::new()
            },
            "{:?}",
            self.view().status
        );
    }

    /// Points Package search at `address` through its form, leaving it
    /// open.
    fn use_service(&self, address: &str) {
        self.open();
        self.activate("Service address");
        self.launcher.set_field_value("address", address);
        block_on(self.launcher.submit_form());
        assert_eq!(
            self.view().status,
            Status::Result(format!("Searching {address} from now on"))
        );
        // Back to the command, its search field blank.
        self.launcher.back();
        assert_eq!(
            self.view().screen,
            Screen::CommandSearch {
                query: String::new()
            }
        );
    }

    fn search(&self, text: &str) {
        block_on(self.launcher.set_query(text));
    }

    fn error(&self) -> String {
        match self.view().status {
            Status::Error(message) => message,
            other => panic!("expected an error, found {other:?}"),
        }
    }
}

/// Waits up to five seconds for `service` to have received `path`.
fn wait_for_request(service: &Service, path: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !service.requests().iter().any(|seen| seen == path) {
        assert!(
            Instant::now() < deadline,
            "the service never received {path}: {:?}",
            service.requests()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn typing_in_root_search_sends_nothing_to_the_service() {
    for fixture in &ALL {
        let service = Service::start();
        let pane = Pane::with(fixture);
        pane.use_service(&service.url());
        pane.to_root();
        // Its title, a result's name, the fixture's special texts: root
        // search lists the command, and asks neither it nor its service.
        for text in ["package", "Package search", "aurora", "slow", "down", "a"] {
            pane.search(text);
            assert!(matches!(pane.view().screen, Screen::Root { .. }));
            assert!(
                !pane
                    .titles()
                    .iter()
                    .any(|title| title.starts_with("aurora-"))
            );
        }
        assert_eq!(
            service.requests(),
            Vec::<String>::new(),
            "{}",
            fixture.package
        );

        // Inside the command, the same text reaches the service.
        pane.open();
        pane.search("aurora");
        assert_eq!(
            service.requests(),
            ["/search?q=aurora"],
            "{}",
            fixture.package
        );
    }
}

#[test]
fn the_service_results_are_listed_and_open_their_details() {
    for fixture in &ALL {
        let service = Service::start();
        let pane = Pane::with(fixture);
        pane.use_service(&service.url());

        pane.search("aurora");
        let view = pane.view();
        assert_eq!(
            view.screen,
            Screen::CommandSearch {
                query: "aurora".into()
            }
        );
        assert_eq!(view.status, Status::Idle, "{}", fixture.package);
        let found: Vec<(String, Option<String>)> = view
            .rows
            .iter()
            .map(|row| (row.title.clone(), row.subtitle.clone()))
            .collect();
        assert_eq!(
            found,
            [
                (
                    "aurora-charts".into(),
                    Some("Charts that draw in the terminal".into())
                ),
                (
                    "aurora-cli".into(),
                    Some("Command-line parsing with subcommands".into())
                ),
            ]
        );
        assert_eq!(view.selected, Some(0));

        // Enter asks the service for the package's details.
        pane.activate("aurora-cli");
        assert_eq!(
            pane.view().status,
            Status::Result(
                "aurora-cli 0.9.3 (Apache-2.0): Command-line parsing with subcommands".into()
            ),
            "{}",
            fixture.package
        );
        assert_eq!(
            service.requests(),
            ["/search?q=aurora", "/packages/aurora-cli"]
        );

        // The text is sent encoded; nothing found lists nothing.
        pane.search("no such thing");
        assert_eq!(pane.titles(), Vec::<String>::new());
        assert_eq!(pane.view().status, Status::Idle);
        assert_eq!(
            service.requests().last().map(String::as_str),
            Some("/search?q=no%20such%20thing")
        );

        // A blank search shows the command's own list again, asking
        // nothing.
        let asked = service.requests().len();
        pane.search("  ");
        assert_eq!(
            pane.titles(),
            ["Type to search the package registry", "Service address"]
        );
        assert_eq!(service.requests().len(), asked);
    }
}

#[test]
fn a_newer_search_stops_the_one_the_service_is_still_answering() {
    for fixture in &ALL {
        let service = Service::start();
        let pane = Pane::with(fixture);
        pane.use_service(&service.url());

        // The service holds this one for ten seconds.
        let slow = pane.launcher.set_query("slow");
        wait_for_request(&service, "/search?q=slow");
        assert_eq!(pane.view().status, Status::Running);

        let started = Instant::now();
        pane.search("basalt");
        assert_eq!(pane.titles(), ["basalt"], "{}", fixture.package);
        assert_eq!(pane.view().status, Status::Idle);
        // Not after the slow one's ten seconds: it was stopped, and the
        // connection it waited on closed.
        assert!(started.elapsed() < service::SLOW / 2);
        assert!(service.wait_for_abandoned(Duration::from_secs(5)));
        assert_eq!(service.abandoned(), ["/search?q=slow"]);

        // Its answer, stopped, never replaces the newer one's.
        block_on(slow);
        assert_eq!(pane.titles(), ["basalt"]);
        assert_eq!(pane.view().status, Status::Idle);

        // And the extension carries on: stopping it was not its failure.
        pane.search("cobalt");
        assert_eq!(pane.titles(), ["cobalt-http"]);
    }
}

#[test]
fn an_answer_to_an_older_search_is_not_shown() {
    for fixture in &ALL {
        let service = Service::start();
        let pane = Pane::with(fixture);
        pane.use_service(&service.url());

        let older = pane.launcher.set_query("aurora");
        let newer = pane.launcher.set_query("ember");
        // The newer answer is shown first, then the older one arrives.
        block_on(newer);
        block_on(older);
        assert_eq!(pane.titles(), ["ember-tz"], "{}", fixture.package);
        assert_eq!(
            pane.view().screen,
            Screen::CommandSearch {
                query: "ember".into()
            }
        );
    }
}

#[test]
fn leaving_the_command_stops_its_search() {
    for fixture in &ALL {
        let service = Service::start();
        let pane = Pane::with(fixture);
        pane.use_service(&service.url());

        let slow = pane.launcher.set_query("slow");
        wait_for_request(&service, "/search?q=slow");
        // Escape clears the search, which stops it...
        pane.launcher.back();
        assert_eq!(
            pane.view().screen,
            Screen::CommandSearch {
                query: String::new()
            }
        );
        assert_eq!(pane.view().status, Status::Idle);
        assert!(service.wait_for_abandoned(Duration::from_secs(5)));
        block_on(slow);
        assert_eq!(
            pane.titles(),
            ["Type to search the package registry", "Service address"]
        );

        // ...and leaving the command stops one too.
        let slow = pane.launcher.set_query("slower");
        wait_for_request(&service, "/search?q=slower");
        pane.to_root();
        let deadline = Instant::now() + Duration::from_secs(5);
        while service.abandoned().len() < 2 {
            assert!(Instant::now() < deadline, "{:?}", service.abandoned());
            std::thread::sleep(Duration::from_millis(10));
        }
        block_on(slow);
        assert!(matches!(pane.view().screen, Screen::Root { .. }));
        assert_eq!(pane.view().status, Status::Idle, "{}", fixture.package);
    }
}

#[test]
fn an_offline_or_failing_service_is_an_error_that_does_not_pause_the_extension() {
    for fixture in &ALL {
        let service = Service::start();
        let pane = Pane::with(fixture);

        // Nothing listens there: more errors than would pause a crashing
        // extension (three within five minutes).
        let offline = format!("http://127.0.0.1:{}", service::closed_port());
        pane.use_service(&offline);
        for text in ["aurora", "basalt", "cobalt", "driftwood"] {
            pane.search(text);
            assert_eq!(
                pane.error(),
                format!(
                    "The extension reported an error: Could not reach the service at \
                     {offline}: connection refused"
                ),
                "{}",
                fixture.package
            );
            assert_eq!(pane.titles(), Vec::<String>::new());
        }

        // The service answers with an error of its own.
        pane.use_service(&service.url());
        pane.search("down");
        assert_eq!(
            pane.error(),
            "The extension reported an error: The service answered 503: \
             the registry is down for maintenance"
        );

        // Still running: none of that paused it.
        pane.search("granite");
        assert_eq!(pane.titles(), ["granite-uuid"], "{}", fixture.package);
        assert_eq!(pane.view().status, Status::Idle);
    }
}

#[test]
fn a_manifest_saying_a_command_searches_needs_the_search_export() {
    let sources = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    // The query sample's component does not export `command-search`.
    let folder = package("sample-query", &sources.path().join("claims-search"));
    let manifest = fs::read_to_string(folder.join("pane.json")).unwrap();
    let manifest = manifest.replace("\"takesQuery\": true", "\"search\": true");
    fs::write(folder.join("pane.json"), manifest).unwrap();
    let launcher = Launcher::with_packages(Runtime::start(), vec![], data.path().join("x"));
    block_on(launcher.install_package(&folder));
    let Status::Error(message) = launcher.view().status else {
        panic!("installed: {:?}", launcher.view().status);
    };
    assert!(
        message.contains(
            "its manifest says it searches as the user types, but it does not export \
             pane:extension/command-search@0.1.0"
        ),
        "{message}"
    );
}
