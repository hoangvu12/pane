//! Pane's Run, a default extension (ADR 0040): it runs what Windows' Run
//! dialog (Win+R) runs — a command line naming a program by its name or
//! path with arguments, a Control Panel applet, a management console, a
//! document, a folder, a network path, a `shell:` or `ms-settings:`
//! address or another registered scheme, with environment variables
//! expanded — through Pane's host (`pane_extension::run`), which runs it
//! elevated through Windows' own prompt when asked, and shares the Run
//! dialog's own history in both directions: what ran in either appears in
//! both, and deleting an entry here removes it there.
//!
//! "Run" is a no-view command that takes a query: it is offered as a
//! fallback for any text and usable through an alias, so a command line
//! typed in root search runs when Enter is pressed. "Run as Administrator"
//! runs the same command line through Windows' elevation prompt, which
//! the user may decline; a declined run, or a failed one, says why and
//! records nothing. "Run History" lists the shared history, newest
//! first: Enter runs an entry again, its action runs it as an
//! administrator, and Delete removes it from the history.
#![no_std]

use pane_extension::alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use pane_extension::feedback::{Toast, ToastStyle, show_hud, show_toast};
use pane_extension::run::{self, RunError};
use pane_extension::{
    Action, Command, CustomView, FieldValue, FormError, Item, LaunchRecord, LaunchType, List,
    NoCustomView,
};

/// The command that runs a command line.
const RUN: &str = "run";
/// The command that runs a command line as an administrator.
const RUN_AS_ADMINISTRATOR: &str = "run-as-administrator";
/// The command that lists the history.
const RUN_HISTORY: &str = "run-history";
/// The prefix of a history entry's item id, followed by its line.
const ENTRY: &str = "entry:";

/// The longest title of a history entry, in characters.
const TITLE_CHARS: usize = 80;

struct Run;
pane_extension::export!(Run);

/// The error's message, as the answer the launcher shows.
fn explain(error: RunError) -> String {
    error.message().to_string()
}

/// Runs the command line `text` was sent, elevated when `elevated`,
/// telling the user what ran (a launch in the background shows nothing).
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
        show_hud(&format!("Ran {line}"), ToastStyle::Success);
    }
    Ok(())
}

/// Removes `line` from the shared history, saying so.
async fn delete_line(line: String) -> Result<(), String> {
    run::delete_from_history(&line).map_err(explain)?;
    show_toast(Toast::success("Deleted from Run's history"));
    Ok(())
}

/// A history entry's item: Enter runs it again, its action runs it as an
/// administrator, and Delete removes it from the history.
fn entry_item(line: &str) -> Item {
    let (again, elevated, gone) = (line.to_string(), line.to_string(), line.to_string());
    Item::new(format!("{ENTRY}{line}"), title_of(line))
        .subtitle("Enter runs it again")
        .on_action(move || run_line(Some(again), false, LaunchType::UserInitiated))
        .action(Action::new("Run as Administrator", move || {
            run_line(Some(elevated), true, LaunchType::UserInitiated)
        }))
        .action(Action::new("Delete", move || delete_line(gone)).destructive())
}

/// `line`, trimmed and at most [`TITLE_CHARS`] characters long.
fn title_of(line: &str) -> String {
    let line = line.trim();
    if line.chars().count() <= TITLE_CHARS {
        return line.into();
    }
    let mut title: String = line.chars().take(TITLE_CHARS - 1).collect();
    title.push('…');
    title
}

impl Command for Run {
    type CustomView = NoCustomView;

    /// Runs a no-view command: "Run" runs the command line it was sent,
    /// "Run as Administrator" runs it elevated. "Run History" opens a
    /// screen, so launching it directly is an error.
    async fn run(command: String, launch: LaunchRecord) -> Result<(), String> {
        let elevated = match command.as_str() {
            RUN => false,
            RUN_AS_ADMINISTRATOR => true,
            RUN_HISTORY => {
                return Err(format!(
                    "`{RUN_HISTORY}` opens a screen; it has no run entry point"
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
        Ok(List::new("Run History").items(
            entries.iter().map(|line| entry_item(line)),
        ))
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
