//! Clipboard history: text the user copies, which Pane keeps for a package
//! that asked for it through `pane:extension/clipboard-history`, only after
//! the user turned it on, and never while it is paused or the package does
//! not run (disabled, paused after a failure, uninstalled).
//!
//! What is kept, and what is not, is decided here, the same on every system
//! ([`accept`]): only plain text ([`Content::Text`]) of at most
//! [`MAX_TEXT_BYTES`], not blank, not marked by the application that copied
//! it as something a clipboard monitor or clipboard history must not keep
//! ([`Markers`]), and not copied from a program the user excluded. The kept
//! items are the package's extension data of their own kind (see
//! `extension_data`), newest first, one item per text ([`add`]) and at most
//! [`MAX_ITEMS`] of them.
//!
//! The system is reached through one small trait, [`ClipboardSystem`], with
//! one adapter per system, chosen by [`native`]:
//!
//! - Windows: a clipboard format listener (`AddClipboardFormatListener`,
//!   `WM_CLIPBOARDUPDATE`) on a thread of Pane's own ([`windows`]);
//! - macOS and Linux: none yet (#37, #38), so clipboard history is
//!   unavailable there and says why.
//!
//! An adapter watches the clipboard only while Pane holds the [`Watch`] it
//! returned, which Pane does exactly while some package keeps clipboard
//! history ([`Capture`]).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::extension_data::ExtensionData;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::WindowsClipboard;
#[cfg(target_os = "windows")]
#[doc(hidden)]
pub use windows::testing;

/// The most items Pane keeps for a package: copying more drops the oldest.
pub const MAX_ITEMS: usize = 100;

/// The longest text Pane keeps, in bytes of UTF-8; longer text is not kept
/// at all (not cut short).
pub const MAX_TEXT_BYTES: usize = 32 * 1024;

/// The most programs a package can exclude.
pub const MAX_EXCLUDED: usize = 64;

/// The longest program name Pane accepts to exclude.
const MAX_PROGRAM_NAME: usize = 260;

/// What the application that copied something said about keeping it, as
/// the system's clipboard formats carry it. On Windows these are the
/// formats `ExcludeClipboardContentFromMonitorProcessing` (and the older
/// `Clipboard Viewer Ignore`), `CanIncludeInClipboardHistory` and
/// `CanUploadToCloudClipboard`, which password managers set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Markers {
    /// A clipboard monitor must not look at it.
    pub exclude_from_monitoring: bool,
    /// Whether it may be kept in clipboard history, if the application said.
    pub include_in_history: Option<bool>,
    /// Whether it may be synced to other devices, if the application said.
    /// Pane never syncs anything, but an application saying no is treated as
    /// saying the text is sensitive, so it is not kept either.
    pub upload_to_cloud: Option<bool>,
}

impl Markers {
    /// Whether these markers let Pane keep what was copied.
    pub fn allow(&self) -> bool {
        !self.exclude_from_monitoring
            && self.include_in_history != Some(false)
            && self.upload_to_cloud != Some(false)
    }
}

/// What was on the clipboard, as far as Pane read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Content {
    /// Plain text.
    Text(String),
    /// Nothing Pane keeps: no text (an image or files alone), or empty.
    Other,
    /// Not read, because its markers forbid keeping it.
    Withheld,
}

/// One change of the clipboard, as an adapter reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub content: Content,
    pub markers: Markers,
    /// The file name of the program that owns the clipboard, such as
    /// `notepad.exe`, if the system says which it is.
    pub source: Option<String>,
}

/// Why an observation is not kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Skip {
    /// The application marked it as not to be kept.
    Marked,
    /// It is not text.
    NotText,
    /// It is empty or only white space.
    Blank,
    /// It is longer than [`MAX_TEXT_BYTES`].
    TooLong,
    /// It was copied from this excluded program.
    Excluded(String),
}

/// The text of `observation` if Pane keeps it for a package that excluded
/// the programs `excluded` (as [`program_name`] writes them), or why not.
pub fn accept<'a>(observation: &'a Observation, excluded: &[String]) -> Result<&'a str, Skip> {
    if !observation.markers.allow() {
        return Err(Skip::Marked);
    }
    let text = match &observation.content {
        Content::Text(text) => text,
        Content::Withheld => return Err(Skip::Marked),
        Content::Other => return Err(Skip::NotText),
    };
    if let Some(source) = &observation.source
        && let Some(program) = excluded.iter().find(|program| matches(source, program))
    {
        return Err(Skip::Excluded(program.clone()));
    }
    if text.trim().is_empty() {
        return Err(Skip::Blank);
    }
    if text.len() > MAX_TEXT_BYTES {
        return Err(Skip::TooLong);
    }
    Ok(text)
}

/// Whether the program `source` (a file name) is the excluded `program`:
/// the same file name, or the same name without its extension, ignoring
/// case, so `KeePass` excludes `KeePass.exe`.
fn matches(source: &str, program: &str) -> bool {
    let source = source.to_lowercase();
    let stem = source
        .rsplit_once('.')
        .map_or(source.as_str(), |(stem, _)| stem);
    source == program || stem == program
}

/// `name` as Pane keeps an excluded program: trimmed and lowercased, such
/// as `keepass.exe`; or why it is not a program's file name.
pub fn program_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Name the program's file, such as KeePass.exe".into());
    }
    if name.contains(['/', '\\', ':']) {
        return Err(format!(
            "“{name}” is a path: name only the program's file, such as KeePass.exe"
        ));
    }
    if name.chars().count() > MAX_PROGRAM_NAME {
        return Err(format!(
            "A program's file name has at most {MAX_PROGRAM_NAME} characters"
        ));
    }
    Ok(name.to_lowercase())
}

/// Whether Pane keeps what is copied for a package.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaptureState {
    /// Never turned on (or turned off): nothing is kept. Every package
    /// starts so.
    #[default]
    Off,
    /// Text copied is kept while the package runs.
    On,
    /// Turned on, then paused: nothing is kept until it is resumed.
    Paused,
}

/// One kept text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Identifies it among the package's items; later items have greater
    /// ids.
    pub id: String,
    pub text: String,
    /// When it was copied, in milliseconds since the Unix epoch.
    pub copied_at: u64,
    /// The program it was copied from, if the system said.
    pub source: Option<String>,
}

// A package's clipboard history is kept as string values by key, like every
// kind of extension data: `capture` ("on" or "paused"; missing is off),
// `excluded` (a JSON list of program names) and one `item:<id>` per item (a
// JSON object), whose ids are 16 digits so that they sort by age.
const CAPTURE: &str = "capture";
const EXCLUDED: &str = "excluded";
const ITEM: &str = "item:";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemJson {
    text: String,
    copied_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

/// The capture state kept in `values`.
pub(crate) fn capture_state(values: &BTreeMap<String, String>) -> CaptureState {
    match values.get(CAPTURE).map(String::as_str) {
        Some("on") => CaptureState::On,
        Some("paused") => CaptureState::Paused,
        _ => CaptureState::Off,
    }
}

/// Keeps `state` in `values`.
pub(crate) fn set_capture_state(values: &mut BTreeMap<String, String>, state: CaptureState) {
    match state {
        CaptureState::Off => values.remove(CAPTURE),
        CaptureState::On => values.insert(CAPTURE.into(), "on".into()),
        CaptureState::Paused => values.insert(CAPTURE.into(), "paused".into()),
    };
}

/// The programs excluded in `values`.
pub(crate) fn excluded(values: &BTreeMap<String, String>) -> Vec<String> {
    values
        .get(EXCLUDED)
        .and_then(|list| serde_json::from_str(list).ok())
        .unwrap_or_default()
}

/// Keeps `programs` as the excluded programs in `values`: each as
/// [`program_name`] writes it, once, in the order given.
pub(crate) fn set_excluded(
    values: &mut BTreeMap<String, String>,
    programs: &[String],
) -> Result<(), String> {
    let mut kept: Vec<String> = Vec::new();
    for program in programs {
        let program = program_name(program)?;
        if !kept.contains(&program) {
            kept.push(program);
        }
    }
    if kept.len() > MAX_EXCLUDED {
        return Err(format!("At most {MAX_EXCLUDED} programs can be excluded"));
    }
    if kept.is_empty() {
        values.remove(EXCLUDED);
    } else {
        let list = serde_json::to_string(&kept).map_err(|error| error.to_string())?;
        values.insert(EXCLUDED.into(), list);
    }
    Ok(())
}

/// The items kept in `values`, newest first. A value that cannot be read
/// is left out.
pub(crate) fn items(values: &BTreeMap<String, String>) -> Vec<Item> {
    values
        .iter()
        .rev()
        .filter_map(|(key, value)| {
            let id = key.strip_prefix(ITEM)?;
            let item: ItemJson = serde_json::from_str(value).ok()?;
            Some(Item {
                id: id.to_owned(),
                text: item.text,
                copied_at: item.copied_at,
                source: item.source,
            })
        })
        .collect()
}

/// How many items `values` keeps.
pub(crate) fn count(values: &BTreeMap<String, String>) -> usize {
    values.keys().filter(|key| key.starts_with(ITEM)).count()
}

/// Keeps `text`, copied from `source` at `now`, as the newest item in
/// `values`: an item with the same text moves to the front instead of being
/// kept twice, and beyond [`MAX_ITEMS`] the oldest go.
pub(crate) fn add(
    values: &mut BTreeMap<String, String>,
    text: &str,
    source: Option<&str>,
    now: u64,
) {
    let next = values
        .keys()
        .filter_map(|key| key.strip_prefix(ITEM)?.parse::<u64>().ok())
        .max()
        .map_or(1, |last| last + 1);
    let same: Vec<String> = items(values)
        .into_iter()
        .filter(|item| item.text == text)
        .map(|item| format!("{ITEM}{}", item.id))
        .collect();
    for key in same {
        values.remove(&key);
    }
    let item = ItemJson {
        text: text.to_owned(),
        copied_at: now,
        source: source.map(str::to_owned),
    };
    let value = serde_json::to_string(&item).expect("an item is always JSON");
    values.insert(format!("{ITEM}{next:016}"), value);
    let mut kept: Vec<String> = values
        .keys()
        .filter(|key| key.starts_with(ITEM))
        .cloned()
        .collect();
    while kept.len() > MAX_ITEMS {
        values.remove(&kept.remove(0));
    }
}

/// Removes every item from `values`, keeping the capture state and the
/// excluded programs; returns how many there were.
pub(crate) fn clear(values: &mut BTreeMap<String, String>) -> usize {
    let before = values.len();
    values.retain(|key, _| !key.starts_with(ITEM));
    before - values.len()
}

/// Receives each change of the clipboard an adapter observes, on the
/// adapter's thread.
pub type Sink = Box<dyn Fn(Observation) + Send + Sync + 'static>;

/// Watching the clipboard, which stops when it is dropped: the adapter no
/// longer listens and calls its sink no more once the drop returns.
pub type Watch = Box<dyn Send>;

/// The system's clipboard, as Pane watches and writes it.
pub trait ClipboardSystem: Send + Sync + 'static {
    /// Why Pane cannot watch the clipboard on this system, if it cannot.
    fn unavailable(&self) -> Option<String>;

    /// Starts watching: `sink` is told of each later change of the
    /// clipboard (not of what is on it now) until the returned watch is
    /// dropped.
    fn watch(&self, sink: Sink) -> Result<Watch, String>;

    /// Puts `text` on the clipboard, as copying it would.
    fn write_text(&self, text: &str) -> Result<(), String>;
}

/// This system's adapter: Windows' clipboard format listener, or one that
/// explains why clipboard history is unavailable here.
pub fn native() -> Arc<dyn ClipboardSystem> {
    #[cfg(target_os = "windows")]
    {
        Arc::new(WindowsClipboard)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let here = crate::platform::Platform::current()
            .map_or_else(|| std::env::consts::OS.to_string(), |p| p.to_string());
        Arc::new(Unavailable(format!(
            "Not available on {here}: Pane watches the clipboard only on Windows so far"
        )))
    }
}

/// A clipboard Pane does not watch, for a launcher given none.
pub fn none() -> Arc<dyn ClipboardSystem> {
    Arc::new(Unavailable(
        "Not available: this Pane does not watch the clipboard".into(),
    ))
}

/// A system where clipboard history is unavailable, saying why.
struct Unavailable(String);

impl ClipboardSystem for Unavailable {
    fn unavailable(&self) -> Option<String> {
        Some(self.0.clone())
    }

    fn watch(&self, _sink: Sink) -> Result<Watch, String> {
        Err(self.0.clone())
    }

    fn write_text(&self, _text: &str) -> Result<(), String> {
        Err(self.0.clone())
    }
}

/// Milliseconds since the Unix epoch, now.
pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Watches the clipboard exactly while some package keeps clipboard
/// history: its capture is on and it runs. Every change to either calls
/// [`Capture::reconcile`] (see [`ExtensionData::set_changed`]), so turning
/// history off, pausing it or disabling, pausing or uninstalling the
/// package stops the watch at once, and turning it on or enabling the
/// package starts it again.
pub(crate) struct Capture {
    system: Arc<dyn ClipboardSystem>,
    data: ExtensionData,
    watching: Mutex<Watching>,
}

#[derive(Default)]
struct Watching {
    watch: Option<Watch>,
    /// Why the adapter could not start watching, until it next can.
    problem: Option<String>,
}

impl Capture {
    /// Keeps clipboard history for the packages of `data` through
    /// `system`, starting to watch now if one keeps it.
    pub fn start(system: Arc<dyn ClipboardSystem>, data: ExtensionData) -> Arc<Capture> {
        let capture = Arc::new(Capture {
            system,
            data: data.clone(),
            watching: Mutex::new(Watching::default()),
        });
        let weak: Weak<Capture> = Arc::downgrade(&capture);
        data.set_changed(Arc::new(move || {
            if let Some(capture) = weak.upgrade() {
                capture.reconcile();
            }
        }));
        capture.reconcile();
        capture
    }

    /// The system's clipboard.
    pub fn system(&self) -> &Arc<dyn ClipboardSystem> {
        &self.system
    }

    /// Why Pane does not watch the clipboard now although it should, or
    /// cannot on this system.
    pub fn problem(&self) -> Option<String> {
        self.system
            .unavailable()
            .or_else(|| self.lock().problem.clone())
    }

    /// Starts or stops watching, as the packages' capture states and
    /// generations now require.
    pub fn reconcile(&self) {
        let wanted = self.system.unavailable().is_none() && self.data.capturing();
        let mut watching = self.lock();
        if !wanted {
            // Dropping the watch waits for the adapter to stop.
            watching.watch = None;
            watching.problem = None;
            return;
        }
        if watching.watch.is_some() {
            return;
        }
        let data = self.data.clone();
        match self.system.watch(Box::new(move |observation| {
            data.capture(&observation, now())
        })) {
            Ok(watch) => {
                watching.watch = Some(watch);
                watching.problem = None;
            }
            Err(problem) => watching.problem = Some(problem),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Watching> {
        self.watching
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(text: &str) -> Observation {
        Observation {
            content: Content::Text(text.into()),
            markers: Markers::default(),
            source: None,
        }
    }

    fn from(text: &str, source: &str) -> Observation {
        Observation {
            source: Some(source.into()),
            ..self::text(text)
        }
    }

    #[test]
    fn plain_text_is_kept() {
        assert_eq!(accept(&text("hello"), &[]), Ok("hello"));
        assert_eq!(accept(&text("  two words \n"), &[]), Ok("  two words \n"));
    }

    #[test]
    fn marked_text_is_not_kept() {
        let marked = |markers: Markers| Observation {
            markers,
            ..text("hunter2")
        };
        let excluded = Markers {
            exclude_from_monitoring: true,
            ..Markers::default()
        };
        let no_history = Markers {
            include_in_history: Some(false),
            ..Markers::default()
        };
        let no_cloud = Markers {
            upload_to_cloud: Some(false),
            ..Markers::default()
        };
        for markers in [excluded, no_history, no_cloud] {
            assert_eq!(accept(&marked(markers), &[]), Err(Skip::Marked));
        }
        // Saying yes changes nothing.
        let allowed = Markers {
            include_in_history: Some(true),
            upload_to_cloud: Some(true),
            ..Markers::default()
        };
        assert_eq!(accept(&marked(allowed), &[]), Ok("hunter2"));
        let withheld = Observation {
            content: Content::Withheld,
            ..text("")
        };
        assert_eq!(accept(&withheld, &[]), Err(Skip::Marked));
    }

    #[test]
    fn other_blank_and_long_content_is_not_kept() {
        let other = Observation {
            content: Content::Other,
            ..text("")
        };
        assert_eq!(accept(&other, &[]), Err(Skip::NotText));
        assert_eq!(accept(&text(""), &[]), Err(Skip::Blank));
        assert_eq!(accept(&text(" \t\r\n"), &[]), Err(Skip::Blank));
        let longest = "a".repeat(MAX_TEXT_BYTES);
        assert_eq!(accept(&text(&longest), &[]), Ok(longest.as_str()));
        let longer = format!("{longest}é");
        assert_eq!(accept(&text(&longer), &[]), Err(Skip::TooLong));
    }

    #[test]
    fn text_from_an_excluded_program_is_not_kept() {
        let excluded = vec![program_name(" KeePass.exe ").unwrap(), "1password".into()];
        assert_eq!(
            accept(&from("secret", "KEEPASS.EXE"), &excluded),
            Err(Skip::Excluded("keepass.exe".into()))
        );
        assert_eq!(
            accept(&from("secret", "1Password.exe"), &excluded),
            Err(Skip::Excluded("1password".into()))
        );
        assert_eq!(accept(&from("note", "notepad.exe"), &excluded), Ok("note"));
        // A program the system does not name is not excluded.
        assert_eq!(accept(&text("note"), &excluded), Ok("note"));
        // Only the whole name counts.
        assert_eq!(
            accept(&from("note", "keepassxc.exe"), &excluded),
            Ok("note")
        );
    }

    #[test]
    fn a_program_is_named_by_its_file() {
        assert_eq!(program_name("KeePass.exe"), Ok("keepass.exe".into()));
        assert!(program_name("  ").is_err());
        assert!(program_name(r"C:\Program Files\KeePass.exe").is_err());
        assert!(program_name(&"a".repeat(261)).is_err());
    }

    #[test]
    fn items_are_newest_first_once_per_text_and_bounded() {
        let mut values = BTreeMap::new();
        add(&mut values, "one", Some("notepad.exe"), 10);
        add(&mut values, "two", None, 20);
        let texts = |values: &BTreeMap<String, String>| -> Vec<String> {
            items(values).into_iter().map(|item| item.text).collect()
        };
        assert_eq!(texts(&values), ["two", "one"]);
        assert_eq!(items(&values)[1].source.as_deref(), Some("notepad.exe"));
        assert_eq!(items(&values)[1].copied_at, 10);
        // Copying "one" again moves it to the front, with its new time.
        add(&mut values, "one", None, 30);
        assert_eq!(texts(&values), ["one", "two"]);
        assert_eq!(items(&values)[0].copied_at, 30);
        assert!(items(&values)[0].id > items(&values)[1].id);
        for number in 0..MAX_ITEMS {
            add(&mut values, &format!("item {number}"), None, 40);
        }
        assert_eq!(count(&values), MAX_ITEMS);
        assert_eq!(texts(&values)[0], format!("item {}", MAX_ITEMS - 1));
        assert!(!texts(&values).contains(&"two".to_string()));
        assert!(!texts(&values).contains(&"one".to_string()));
    }

    #[test]
    fn capture_state_and_exclusions_are_kept_apart_from_items() {
        let mut values = BTreeMap::new();
        assert_eq!(capture_state(&values), CaptureState::Off);
        set_capture_state(&mut values, CaptureState::On);
        assert_eq!(capture_state(&values), CaptureState::On);
        set_capture_state(&mut values, CaptureState::Paused);
        assert_eq!(capture_state(&values), CaptureState::Paused);
        let programs = [
            "KeePass.exe".to_string(),
            "keepass.exe".into(),
            "Bitwarden.exe".into(),
        ];
        set_excluded(&mut values, &programs).unwrap();
        assert_eq!(excluded(&values), ["keepass.exe", "bitwarden.exe"]);
        assert!(set_excluded(&mut values, &["a/b".into()]).is_err());
        assert_eq!(excluded(&values), ["keepass.exe", "bitwarden.exe"]);
        add(&mut values, "kept", None, 1);
        assert_eq!(count(&values), 1);
        assert_eq!(clear(&mut values), 1);
        assert_eq!(count(&values), 0);
        assert_eq!(capture_state(&values), CaptureState::Paused);
        assert_eq!(excluded(&values), ["keepass.exe", "bitwarden.exe"]);
        set_excluded(&mut values, &[]).unwrap();
        set_capture_state(&mut values, CaptureState::Off);
        assert!(values.is_empty());
    }
}
