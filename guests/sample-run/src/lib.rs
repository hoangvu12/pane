//! Pane's Run sample: a command that runs what Windows' Run dialog (Win+R)
//! runs through `pane_extension::run`, sharing the Run dialog's own
//! history. It answers the same in JavaScript and TypeScript
//! (guests/sample-run-js, guests/sample-run-ts), so the tests drive all
//! three alike.
//!
//! - "Run sample" is a no-view command that takes a query: the command
//!   line it is sent from root search, through its alias or as a
//!   fallback, runs; a toast says what ran.
//! - "Run elevated" runs the command line it is sent through Windows'
//!   own elevation prompt, which the user may decline.
//! - "Run history" lists the shared history, newest first: Enter runs an
//!   entry again, and Delete removes it from the history.
#![no_std]

use pane_extension::alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use pane_extension::feedback::{Toast, show_toast};
use pane_extension::run::{self, RunError};
use pane_extension::{
    Action, Command, CustomView, FieldValue, FormError, Item, LaunchRecord, LaunchType, List,
    NoCustomView,
};

/// The command that runs a command line.
const RUN: &str = "run";
/// The command that runs one as an administrator.
const ELEVATED: &str = "elevated";
/// The command that lists the history.
const HISTORY: &str = "history";
/// The prefix of a history entry's item id, followed by its line.
const ENTRY: &str = "entry:";

struct RunSample;
pane_extension::export!(RunSample);

/// The error's message, as the answer the launcher shows.
fn explain(error: RunError) -> String {
    error.message().to_string()
}

/// Runs the command line `text` was sent, elevated when `elevated`,
/// showing a toast that says what ran (a launch in the background shows
/// nothing).
async fn run_line(
    text: Option<String>,
    elevated: bool,
    launch_type: LaunchType,
) -> Result<(), String> {
    let line = text
        .map(|text| text.trim().to_string())
        .filter(|line| !line.is_empty());
    let Some(line) = line else {
        return Err(format!(
            "Run was sent no command line: give it an alias or make it a fallback in Settings, \
             then type a command line in root search"
        ));
    };
    run::run(&line, elevated).map_err(explain)?;
    if launch_type != LaunchType::Background {
        let title = if elevated {
            format!("Ran {line} as an administrator")
        } else {
            format!("Ran {line}")
        };
        show_toast(Toast::success(title));
    }
    Ok(())
}

/// Removes `line` from the shared history, saying so.
async fn delete_line(line: String) -> Result<(), String> {
    run::delete_from_history(&line).map_err(explain)?;
    show_toast(Toast::success("Deleted from Run's history"));
    Ok(())
}

impl Command for RunSample {
    type CustomView = NoCustomView;

    /// Runs a no-view command: "Run sample" runs the command line it was
    /// sent, "Run elevated" runs it elevated. "Run history" opens a
    /// screen, so launching it directly is an error.
    async fn run(command: String, launch: LaunchRecord) -> Result<(), String> {
        let elevated = match command.as_str() {
            RUN => false,
            ELEVATED => true,
            HISTORY => {
                return Err(format!(
                    "`{HISTORY}` opens a screen; it has no run entry point"
                ));
            }
            other => return Err(format!("unknown command: {other}")),
        };
        run_line(launch.fallback_text, elevated, launch.launch_type).await
    }

    /// The shared history, newest first; nothing run yet says so.
    async fn render() -> Result<List, String> {
        let entries = run::history().map_err(explain)?;
        if entries.is_empty() {
            return Ok(List::new("Run History").item(
                Item::new("empty", "Nothing has been run yet").subtitle(
                    "What you run in Pane or the Run dialog appears here",
                ),
            ));
        }
        let items = entries.iter().map(|line| {
            let (again, gone) = (line.to_string(), line.to_string());
            Item::new(format!("{ENTRY}{line}"), line.clone())
                .subtitle("Enter runs it again")
                .on_action(move || run_line(Some(again), false, LaunchType::UserInitiated))
                .action(
                    Action::new("Delete", move || delete_line(gone)).destructive(),
                )
        });
        Ok(List::new("Run History").items(items))
    }

    /// Runs the history entry whose item was drawn before, which is gone
    /// now; an id no item names is an error.
    async fn run_search_result(id: String) -> Result<(), String> {
        match id.strip_prefix(ENTRY) {
            Some(line) => run_line(Some(line.into()), false, LaunchType::UserInitiated).await,
            None => Err(format!("unknown item: {id}")),
        }
    }

    async fn submit_form(item_id: String, _values: Vec<FieldValue>) -> Result<String, FormError> {
        Err(FormError {
            field: None,
            message: format!("unknown form: {item_id}"),
        })
    }

    async fn open_view(item_id: String) -> Result<CustomView, String> {
        Err(format!("unknown view: {item_id}"))
    }
}
