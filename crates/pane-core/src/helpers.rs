//! Native helpers: prebuilt programs a package ships for each operating
//! system and processor it supports, which Pane runs for its commands
//! through `pane:extension/helpers` (a WASI 0.3 guest cannot start a
//! program; [ADR 0014](../../../docs/adr/0014-optional-native-extension-helpers.md)).
//!
//! A package declares each helper under `helpers` in its `pane.json`, with
//! one file per target (`linux-x86_64`, `macos-aarch64`, `windows-x86_64`,
//! ...). Pane runs the file for the system it runs on, never compiles
//! anything, and explains a helper with no file for this target, a missing
//! file, or a file that is a program for another system (read from its
//! header: ELF, Mach-O or PE).
//!
//! Pane owns the helper's process: it writes the input to its standard
//! input, reads its standard output and error, and ends the process (and
//! reaps it) when the guest drops the call, when the Pane call that started
//! it returns, when the instance goes, or when the package's generation
//! ends, whichever comes first. A supervising thread per run does this, so
//! a generation's end stops its helpers even while the runtime thread is
//! busy. Processes the helper starts itself are not tracked.

use wasmtime::component::{Accessor, HasData};

use crate::packages::ManifestHelper;
use crate::platform;
use crate::runtime::{GuestState, bindings};

use bindings::pane::extension::helpers as wit;

pub(crate) mod runner;

pub(crate) use runner::*;

/// The host side of `pane:extension/helpers`: starts the helper for the
/// calling guest and waits for it, ending its process if the guest drops
/// the call.
pub(crate) struct Runs;

impl HasData for Runs {
    type Data<'a> = &'a mut GuestState;
}

impl wit::Host for GuestState {}

impl<T> wit::HostWithStore<T> for Runs {
    async fn run(
        accessor: &Accessor<T, Self>,
        helper: String,
        args: Vec<String>,
        input: String,
    ) -> Result<String, wit::HelperError> {
        let started = accessor.with(|mut view| view.get().start_helper(helper, args, input));
        // Dropped here if the guest cancels the call: the process ends.
        let result = match started {
            Ok(running) => running.finish().await,
            Err(error) => Err(error),
        };
        result.map_err(wit::HelperError::from)
    }
}

impl From<HelperError> for wit::HelperError {
    fn from(error: HelperError) -> wit::HelperError {
        use wit::HelperErrorKind as Wit;
        let kind = match error.kind {
            HelperErrorKind::NotFound => Wit::NotFound,
            HelperErrorKind::Unavailable => Wit::Unavailable,
            HelperErrorKind::Failed => Wit::Failed,
            HelperErrorKind::Refused => Wit::Refused,
        };
        wit::HelperError {
            kind,
            message: error.message,
        }
    }
}

/// "Helpers: echo for Linux x86-64 (this system) and Windows x86-64", or
/// "... (none for this system)", for a package preview; `None` when the
/// package ships none.
pub(crate) fn describe(helpers: &[ManifestHelper]) -> Option<String> {
    if helpers.is_empty() {
        return None;
    }
    let here = current_target();
    let described: Vec<String> = helpers
        .iter()
        .map(|helper| {
            let targets: Vec<String> = helper
                .targets
                .keys()
                .map(|target| {
                    if *target == here {
                        format!("{} (this system)", target_name(target))
                    } else {
                        target_name(target)
                    }
                })
                .collect();
            let none_here = if helper.targets.contains_key(&here) {
                ""
            } else {
                " (none for this system)"
            };
            format!("{} for {}{none_here}", helper.id, platform::join(&targets))
        })
        .collect();
    Some(format!("Helpers: {}", described.join("; ")))
}
