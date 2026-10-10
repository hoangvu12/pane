//! Searching an online service inside its command, through the launcher's
//! public interface: the user opens Package search, the search sample,
//! whose screen is a designed List that handles its search itself (#240)
//! — the search field Pane draws in the view's header, its text told to
//! the view through the List's search-text event, and the service's
//! answers listed as the items. The command is a real guest (`cargo xtask
//! guests`), in Rust, JavaScript and TypeScript alike, and the service is
//! the fixture service, a made-up package registry each test serves on a
//! free port of 127.0.0.1: nothing here reaches the network beyond this
//! computer.
//!
//! While a command's call waits on the service, Pane serves other calls
//! (#136): the calculator, another package, answers root search meanwhile.

#[path = "support/feedback.rs"]
mod feedback;
#[path = "support/service.rs"]
mod service;
#[path = "support/unreachable.rs"]
mod unreachable;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use feedback::shown;
use futures::executor::block_on;
use pane_core::{HttpLimits, Launcher, PackageIdentity, Runtime, Screen, Status};
use service::Service;
use tempfile::TempDir;

struct Fixture {
    /// The assembled package under `target/guests/packages`.
    package: &'static str,
    /// Its title.
    title: &'static str,
}

const RUST: Fixture = Fixture {
    package: "sample-search",
    title: "Search sample",
};

const JAVASCRIPT: Fixture = Fixture {
    package: "sample-search-js",
    title: "JavaScript search sample",
};
const TYPESCRIPT: Fixture = Fixture {
    package: "sample-search-ts",
    title: "TypeScript search sample",
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
    runtime: Runtime,
    _sources: TempDir,
    _data: TempDir,
}

impl Pane {
    fn with(fixture: &Fixture) -> Pane {
        Pane::with_runtime(fixture, Runtime::start().unwrap())
    }

    /// Like [`Pane::with`], its web requests bounded by `limits`.
    fn with_limits(fixture: &Fixture, limits: HttpLimits) -> Pane {
        let runtime = Runtime::start().unwrap();
        runtime.set_http_limits(limits);
        Pane::with_runtime(fixture, runtime)
    }

    fn with_runtime(fixture: &Fixture, runtime: Runtime) -> Pane {
        let sources = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let launcher =
            Launcher::with_packages(Ok(runtime.clone()), vec![], data.path().join("extensions"));
        let folder = package(fixture.package, &sources.path().join(fixture.package));
        block_on(launcher.install_package(&folder));
        assert!(
            matches!(launcher.view().status, Status::Result(_)),
            "{:?}",
            launcher.view().status
        );
        Pane {
            launcher,
            runtime,
            _sources: sources,
            _data: data,
        }
    }

    /// The identity of the search sample, installed.
    fn identity(&self) -> PackageIdentity {
        self.launcher
            .packages()
            .into_iter()
            .next()
            .expect("the search sample is installed")
            .identity
    }

    fn view(&self) -> pane_core::LauncherView {
        self.launcher.view()
    }

    fn titles(&self) -> Vec<String> {
        self.view().rows.into_iter().map(|row| row.title).collect()
    }

    fn activate(&self, title: &str) {
        self.select(title);
        block_on(self.launcher.activate_selected());
    }

    fn select(&self, title: &str) {
        let index = self
            .titles()
            .iter()
            .position(|row| row == title)
            .unwrap_or_else(|| panic!("no row {title:?} in {:?}", self.titles()));
        self.launcher.select(index);
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
        assert!(
            matches!(self.view().screen, Screen::DesignedView { .. }),
            "{:?}",
            self.view().status
        );
    }

    /// Points Package search at `address`, through the setting its
    /// searches read, leaving it at root search.
    fn use_service(&self, address: &str) {
        let identity = self.identity();
        block_on(
            self.launcher
                .set_preference(&identity, "service", Some(address)),
        )
        .expect("the address is saved");
    }

    fn search(&self, text: &str) {
        block_on(self.launcher.set_query(text));
    }

    /// The error shown: a failure toast of an action's.
    fn error(&self) -> String {
        match shown(&self.launcher) {
            Status::Error(message) => message,
            other => panic!("expected an error, found {other:?}"),
        }
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

        // Inside the command, the same text reaches the service through
        // the List's search-text event.
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

        pane.open();
        pane.search("aurora");
        let view = pane.view();
        assert!(
            matches!(view.screen, Screen::DesignedView { .. }),
            "{}",
            fixture.package
        );
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
            shown(&pane.launcher),
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
        assert_eq!(pane.titles(), Vec::<String>(), "{}", fixture.package);
        assert_eq!(
            service.requests().last().map(String::as_str),
            Some("/search?q=no%20such%20thing")
        );
    }
}

#[test]
fn an_offline_or_failing_service_is_an_error_that_does_not_pause_the_extension() {
    for fixture in &ALL {
        let service = Service::start();
        let pane = Pane::with(fixture);

        // Nothing listens there: more errors than would pause a crashing
        // extension (three within five minutes).
        let closed = unreachable::ClosedPort::new();
        let offline = closed.url();
        pane.use_service(&offline);
        for text in ["aurora", "basalt", "cobalt", "driftwood"] {
            pane.open();
            pane.search(text);
            // The view answers its own error, in its empty view: the rows
            // list nothing, and the launcher is none the worse.
            assert_eq!(pane.titles(), Vec::<String>(), "{}", fixture.package);
            assert!(matches!(pane.view().status, Status::Idle));
            pane.to_root();
        }

        // The service answers with an error of its own: same shape.
        pane.use_service(&service.url());
        pane.open();
        pane.search("down");
        assert_eq!(pane.titles(), Vec::<String>(), "{}", fixture.package);
        assert!(matches!(pane.view().status, Status::Idle));

        // Still running: none of that paused it.
        pane.search("granite");
        assert_eq!(pane.titles(), ["granite-uuid"], "{}", fixture.package);
    }
}

/// A short wait between two pieces of an answer, and Pane's own deadline,
/// so a stalled answer can end only by that wait, and quickly.
fn stall_limits() -> HttpLimits {
    HttpLimits {
        between_bytes: Duration::from_millis(300),
        ..HttpLimits::default()
    }
}

/// A short deadline, and Pane's own wait between two pieces of an answer,
/// so a dripping answer (a byte every 50 ms, however late a slow machine
/// reads one) can end only by the deadline, and quickly.
fn drip_limits() -> HttpLimits {
    HttpLimits {
        deadline: Duration::from_millis(1500),
        ..HttpLimits::default()
    }
}

#[test]
fn a_service_that_stalls_is_given_up_on_within_the_limits() {
    for fixture in &ALL {
        let service = Service::start();
        let pane = Pane::with_limits(fixture, stall_limits());
        pane.use_service(&service.url());

        // Each ends by the test's short limit, not by Pane's own, which is
        // far longer: the bounds leave a slow runner room, not the limits.
        let defaults = HttpLimits::default();

        // A head, then nothing: the wait between two pieces of the body.
        // The event the text raises waits for the whole search (the
        // JavaScript and TypeScript samples) or its push does (the Rust
        // one, `Pending` in the render): either way the rows appear once
        // the answer lands, waited for with a deadline a loaded runner
        // can afford.
        let started = Instant::now();
        pane.open();
        pane.search("stall");
        let deadline = Instant::now() + Duration::from_secs(30);
        while pane.titles().is_empty() {
            assert!(
                Instant::now() < deadline,
                "{}: the search never answered",
                fixture.package
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            started.elapsed() < defaults.between_bytes * 8,
            "{:?}",
            started.elapsed()
        );

        // A byte now and then: the whole request's deadline.
        pane.runtime.set_http_limits(drip_limits());
        let started = Instant::now();
        pane.search("drip");
        let deadline = Instant::now() + Duration::from_secs(30);
        while pane.titles().is_empty() {
            assert!(
                Instant::now() < deadline,
                "{}: the search never answered",
                fixture.package
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            started.elapsed() < defaults.deadline * 4,
            "{:?}",
            started.elapsed()
        );

        // An action's request is bounded the same way.
        pane.search("misbehaving");
        assert_eq!(
            pane.titles(),
            ["huge-details", "stall-details", "drip-details"],
            "{}",
            fixture.package
        );
        pane.runtime.set_http_limits(stall_limits());
        let started = Instant::now();
        pane.activate("stall-details");
        assert_eq!(
            pane.error(),
            format!(
                "The extension reported an error: Could not reach the service at {}: \
                 the service did not answer in time",
                service.url()
            ),
            "{}",
            fixture.package
        );
        assert!(
            started.elapsed() < defaults.between_bytes * 8,
            "{:?}",
            started.elapsed()
        );
        pane.runtime.set_http_limits(drip_limits());
        pane.activate("drip-details");
        assert_eq!(
            pane.error(),
            format!(
                "The extension reported an error: Could not reach the service at {}: \
                 the service took too long to answer",
                service.url()
            )
        );
        pane.activate("huge-details");
        assert_eq!(
            pane.error(),
            "The extension reported an error: The answer is larger than the \
             4194304 bytes Pane accepts"
        );

        // Pane hung up on each, and the extension carries on.
        let deadline = Instant::now() + Duration::from_secs(30);
        while service.abandoned().len() < 5 {
            assert!(Instant::now() < deadline, "{:?}", service.abandoned());
            std::thread::sleep(Duration::from_millis(10));
        }
        pane.search("cobalt");
        assert_eq!(pane.titles(), ["cobalt-http"], "{}", fixture.package);
    }
}

#[test]
fn a_service_whose_certificate_is_not_trusted_is_an_error() {
    for fixture in &ALL {
        let service = unreachable::UntrustedService::start();
        let pane = Pane::with(fixture);
        pane.use_service(&service.url());
        // Twice: a failed handshake leaves nothing behind for the next.
        for text in ["aurora", "basalt"] {
            pane.open();
            pane.search(text);
            assert_eq!(pane.titles(), Vec::<String>(), "{}", fixture.package);
            pane.to_root();
        }
    }
}

#[test]
fn the_extension_list_says_which_packages_use_the_network_and_what_they_reached() {
    for fixture in &ALL {
        let service = Service::start();
        let pane = Pane::with(fixture);
        pane.use_service(&service.url());
        pane.open();
        pane.search("aurora");

        pane.to_root();
        pane.search("manage extensions");
        pane.activate("Manage Extensions");
        let view = pane.view();
        assert!(matches!(view.screen, Screen::Extensions { .. }));
        // Its row says so.
        let using: Vec<&str> = view
            .rows
            .iter()
            .filter(|row| {
                row.subtitle
                    .as_deref()
                    .is_some_and(|subtitle| subtitle.contains("Uses the network"))
            })
            .map(|row| row.title.as_str())
            .collect();
        assert_eq!(using, [fixture.title], "{}", fixture.package);
        let network: Vec<&str> = view
            .rows
            .iter()
            .filter(|row| row.title.starts_with("Network use of"))
            .map(|row| row.title.as_str())
            .collect();
        let details = format!("Network use of {}", fixture.title);
        assert_eq!(network, [details.as_str()]);

        // Its details list the address it reached this session.
        pane.activate(&details);
        let view = pane.view();
        assert!(
            matches!(view.screen, Screen::NetworkDetails { .. }),
            "{:?}",
            view.screen
        );
        assert_eq!(view.title, details);
        let reached = format!("127.0.0.1:{}", service.port());
        let lines = view.details().to_vec();
        assert_eq!(
            lines.iter().skip(2).map(String::as_str).collect::<Vec<_>>(),
            [
                "Addresses it tried to reach this session:",
                reached.as_str()
            ],
            "{lines:?}"
        );
        pane.launcher.back();
        assert!(matches!(pane.view().screen, Screen::Extensions { .. }));
    }
}
