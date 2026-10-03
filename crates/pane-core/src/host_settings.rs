//! Pane's own host settings: the preferences the host records for itself,
//! separate from every extension's data.
//!
//! The record is `settings.json` in Pane's data folder, beside the
//! `extensions` folder that holds packages and their own records — the
//! host's preferences are never written inside a package's folder, and a
//! package's data is never written inside the host's record. It follows
//! the house record-file rules, as `installed.json`, `aliases.json` and
//! `updates.json` do: versioned, so a record another Pane cannot read is
//! refused rather than misread; validated, so a field this Pane does not
//! understand fails the whole record instead of half-loading; written
//! atomically, so a crash or power loss leaves either the old record or
//! the new one, never a torn one; and defaulted, so a missing field (or a
//! missing record) means the default, not an error.
//!
//! Reading never repairs: a record that cannot be read or parsed is
//! reported as a problem and left exactly as it is on disk, so the source
//! data stays for diagnosis. Whether to refuse replacing it is the
//! caller's policy (the window keeps the house rule: it does not replace
//! an unreadable record); this module only reads and writes.
//!
//! Nothing here knows the renderer: the preferences are plain values, and
//! the window layer resolves them into a theme and a material, including
//! the platform's own normalization of glass to solid.

use std::path::Path;

use serde_json::{Map, Value};

use crate::atomic::{Readers, write_atomically};
use crate::hotkeys::Shortcut;

/// The file the settings are recorded in, in Pane's data folder.
const FILE: &str = "settings.json";

/// The record's version; a record of another version is not read.
const VERSION: u64 = 1;

/// The theme the user chose for Pane's windows: follow the operating
/// system's appearance, or force one of the two palettes.
///
/// The default is the reference's dark palette, as the launcher has
/// shipped since the visual rework; the appearance page records whichever
/// the user picks, and this value is renderer-independent — the window
/// layer resolves it against the system appearance to a palette.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    /// Pane's dark palette, whatever the system's appearance is.
    #[default]
    Dark,
    /// Pane's light palette, whatever the system's appearance is.
    Light,
    /// The palette that matches the system's appearance, following its
    /// changes while Pane runs.
    System,
}

/// The window surface the user chose: the translucent glass where the
/// platform can frost the window, or the deterministic solid surface.
///
/// A request for glass is a request, not a result: the platform may not
/// provide compositor frost, and the window layer normalizes such a
/// request to the solid surface and says so in the appearance page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaterialPreference {
    /// The translucent panel over a frosted window, where the platform
    /// provides frost; normalized to solid where it does not.
    #[default]
    Glass,
    /// The opaque window and solid panel, the same on every platform.
    Solid,
}

/// The display the launcher window opens on, as the user chose it. The
/// choice is a preference, not a placement: where the launcher actually
/// opens is the display layout the platform reports and the resolution
/// that turns this choice into a display (see `crate::placement`), which
/// falls back to an available display when the chosen one is gone.
///
/// The default is the primary display, matching where the launcher has
/// opened since it first shipped; it is the provisional default of the
/// settings specification, not a separately confirmed product decision,
/// and the Launcher page names it as such.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpeningMonitor {
    /// The system's primary display, whatever else is connected.
    #[default]
    Primary,
    /// The display the pointer is on when the launcher opens, where the
    /// system tells Pane where the pointer is.
    Pointer,
    /// The display of the operating system's active window — the one the
    /// user is working in — where the system tells Pane which window is
    /// active.
    #[serde(rename = "active-window")]
    ActiveWindow,
}

/// What reopening the launcher shows, as the user chose it. Dismissal —
/// hiding the launcher, by the Open Pane hotkey or by Escape at root
/// search with an empty query — never quits Pane, and what the next
/// opening starts from is this choice.
///
/// The default restores a still-valid view, as the settings
/// specification proposes provisionally; a view that is no longer valid —
/// its command removed or its extension disabled — returns safely to
/// root search either way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reopening {
    /// Show the view the launcher was left on, when it is still valid.
    #[serde(rename = "restore-view")]
    #[default]
    RestoreView,
    /// Start from root search, with an empty query, whatever was left.
    #[serde(rename = "root-search")]
    RootSearch,
}

/// The host settings as the user chose them: one theme preference, one
/// material preference, the Open Pane hotkey, the launch-at-login choice,
/// the launcher's opening display and what reopening shows, the whole of
/// what the Settings pages built so far offer. Later pages add fields
/// beside these, with the same rules: missing fields default, and
/// unknown values fail the record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostSettings {
    /// The theme the user chose for Pane's windows.
    pub theme: ThemePreference,
    /// The surface the user chose for Pane's windows.
    pub material: MaterialPreference,
    /// The global shortcut that opens Pane itself from any application —
    /// the application-owned binding the General page records. It is a
    /// host setting, not a command's hotkey: it belongs to no package,
    /// stays while extensions are disabled, and is applied through the
    /// same platform registration path the command hotkeys use.
    pub open_pane: Shortcut,
    /// Whether the user chose Pane to start at login. A preference, not a
    /// registration: whether Pane actually starts is the platform's own
    /// login integration, which the window layer reconciles with this
    /// choice (see `crate::autostart`) rather than trusting either side
    /// alone.
    pub launch_at_login: bool,
    /// The display the launcher window opens on, as the Launcher page
    /// records it. A preference: the placement is resolved against the
    /// display layout when the launcher opens (see `crate::placement`),
    /// with a fallback when the chosen display is gone.
    pub opening_monitor: OpeningMonitor,
    /// What reopening the launcher shows: the view it was left on, when
    /// still valid, or root search. A preference the Launcher page
    /// records; dismissal behavior itself follows the specification's
    /// Escape contract and is not a choice here.
    pub reopening: Reopening,
}

impl Default for HostSettings {
    fn default() -> HostSettings {
        HostSettings {
            theme: ThemePreference::default(),
            material: MaterialPreference::default(),
            open_pane: Shortcut::open_pane_default(),
            launch_at_login: false,
            opening_monitor: OpeningMonitor::default(),
            reopening: Reopening::default(),
        }
    }
}

impl HostSettings {
    /// Reads the settings recorded in `dir`. No record at all means the
    /// defaults, as a record with missing fields does; a record that
    /// cannot be read, parsed or validated is `Err` with the problem,
    /// phrased with the file's path, and is left as it is.
    pub fn open(dir: &Path) -> Result<HostSettings, String> {
        let file = dir.join(FILE);
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(HostSettings::default());
            }
            Err(error) => return Err(format!("{} cannot be read: {error}", file.display())),
        };
        let fields: Map<String, Value> = serde_json::from_str(&text)
            .map_err(|error| format!("{} is invalid: {error}", file.display()))?;
        match fields.get("version").and_then(Value::as_u64) {
            Some(VERSION) => {}
            Some(version) => {
                return Err(format!(
                    "{} has version {version}, which this Pane does not read",
                    file.display()
                ));
            }
            None => {
                return Err(format!(
                    "{} is invalid: it has no version number",
                    file.display()
                ));
            }
        }
        let recorded: Recorded = serde_json::from_value(Value::Object(fields))
            .map_err(|error| format!("{} is invalid: {error}", file.display()))?;
        let open_pane = match recorded.open_pane {
            None => Shortcut::open_pane_default(),
            Some(text) => Shortcut::parse(&text).map_err(|problem| {
                format!(
                    "{} is invalid: its open pane hotkey is not one: {problem}",
                    file.display()
                )
            })?,
        };
        Ok(HostSettings {
            theme: recorded.theme,
            material: recorded.material,
            open_pane,
            launch_at_login: recorded.launch_at_login,
            opening_monitor: recorded.opening_monitor,
            reopening: recorded.reopening,
        })
    }

    /// Writes these settings to the record in `dir`, atomically: the
    /// record is replaced whole or not at all, and a crash leaves the
    /// previous one. `Err` with the problem, phrased with the file's path,
    /// when the record cannot be written. An unreadable record is not
    /// treated specially here — the caller decides what it replaces.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let recorded = Recorded {
            version: VERSION,
            theme: self.theme,
            material: self.material,
            open_pane: Some(self.open_pane.id()),
            launch_at_login: self.launch_at_login,
            opening_monitor: self.opening_monitor,
            reopening: self.reopening,
        };
        let text = serde_json::to_string_pretty(&recorded).map_err(|error| error.to_string())?;
        let file = dir.join(FILE);
        write_atomically(&file, text.as_bytes(), Readers::Default)
            .map_err(|error| format!("{} cannot be written: {error}", file.display()))
    }
}

/// The settings as the record holds them. Every field is written every
/// time; missing fields read as the defaults. The record's fields are
/// named as the house records name theirs, in camelCase.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Recorded {
    version: u64,
    #[serde(default)]
    theme: ThemePreference,
    #[serde(default)]
    material: MaterialPreference,
    /// The Open Pane hotkey as its id, such as `ctrl+alt+space`; missing
    /// means this system's provisional default. A value that is not a
    /// shortcut fails the whole record. The field keeps the name it was
    /// first recorded with, so records an earlier Pane wrote still read,
    /// while the record's other fields follow the house camelCase names.
    #[serde(default, rename = "open_pane")]
    open_pane: Option<String>,
    #[serde(default)]
    launch_at_login: bool,
    /// The display the launcher opens on, as one of the three words the
    /// preference names; missing means the primary display, the
    /// provisional default.
    #[serde(default)]
    opening_monitor: OpeningMonitor,
    /// What reopening the launcher shows; missing means restoring a
    /// still-valid view, the provisional default.
    #[serde(default)]
    reopening: Reopening,
}

#[cfg(test)]
mod tests {
    use super::{FILE, HostSettings, MaterialPreference, Shortcut, ThemePreference};

    /// Reads what `text` records in a fresh folder.
    fn reading(text: &str) -> Result<HostSettings, String> {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), text).unwrap();
        HostSettings::open(dir.path())
    }

    #[test]
    fn no_record_means_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            HostSettings::open(dir.path()).unwrap(),
            HostSettings {
                theme: ThemePreference::Dark,
                material: MaterialPreference::Glass,
                open_pane: Shortcut::open_pane_default(),
                ..HostSettings::default()
            }
        );
    }

    #[test]
    fn missing_fields_mean_the_defaults_and_unknown_fields_are_ignored() {
        assert_eq!(
            reading(r#"{ "version": 1 }"#).unwrap(),
            HostSettings::default()
        );
        assert_eq!(
            reading(r#"{ "version": 1, "later": "a field a newer Pane writes" }"#).unwrap(),
            HostSettings::default()
        );
    }

    #[test]
    fn a_record_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let settings = HostSettings {
            theme: ThemePreference::System,
            material: MaterialPreference::Solid,
            open_pane: Shortcut::parse("ctrl+alt+b").unwrap(),
            launch_at_login: true,
            opening_monitor: super::OpeningMonitor::Pointer,
            reopening: super::Reopening::RootSearch,
        };
        settings.save(dir.path()).unwrap();
        assert_eq!(HostSettings::open(dir.path()).unwrap(), settings);
    }

    #[test]
    fn the_launcher_choices_are_written_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let settings = HostSettings {
            opening_monitor: super::OpeningMonitor::ActiveWindow,
            reopening: super::Reopening::RootSearch,
            ..HostSettings::default()
        };
        settings.save(dir.path()).unwrap();
        assert_eq!(HostSettings::open(dir.path()).unwrap(), settings);
        // The fields are named as the house records name theirs, and the
        // values as the preferences name theirs, so the choices survive a
        // Pane that knows them by name alone.
        let text = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(
            text.contains("\"openingMonitor\": \"active-window\""),
            "the record is {text}"
        );
        assert!(
            text.contains("\"reopening\": \"root-search\""),
            "the record is {text}"
        );
        // A record without the fields is one an older Pane wrote: the
        // provisional defaults, not an error.
        assert_eq!(
            reading(r#"{ "version": 1 }"#).unwrap().opening_monitor,
            super::OpeningMonitor::Primary
        );
        assert_eq!(
            reading(r#"{ "version": 1 }"#).unwrap().reopening,
            super::Reopening::RestoreView
        );
    }

    #[test]
    fn the_open_pane_field_defaults_and_a_value_that_is_not_a_shortcut_fails_the_record() {
        // Missing: this system's provisional default.
        assert_eq!(
            reading(r#"{ "version": 1, "open pane": "missing" }"#).unwrap(),
            HostSettings::default()
        );
        // Recorded as the shortcut's id, on any system.
        assert_eq!(
            reading(r#"{ "version": 1, "open_pane": "alt+space" }"#).unwrap(),
            HostSettings {
                open_pane: Shortcut::parse("alt+space").unwrap(),
                ..HostSettings::default()
            }
        );
        // A value that is not a shortcut fails the whole record.
        let problem = reading(r#"{ "version": 1, "open_pane": "not a shortcut" }"#);
        assert!(problem.is_err(), "{problem:?}");
        assert!(
            problem
                .unwrap_err()
                .contains("its open pane hotkey is not one"),
            "the field is named"
        );
    }

    #[test]
    fn the_launch_at_login_choice_is_written_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let settings = HostSettings {
            launch_at_login: true,
            ..HostSettings::default()
        };
        settings.save(dir.path()).unwrap();
        assert_eq!(HostSettings::open(dir.path()).unwrap(), settings);
        // The field is named as the house records name their fields, so
        // the choice survives a Pane that knows it by name alone.
        let text = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(
            text.contains("\"launchAtLogin\": true"),
            "the record is {text}"
        );
        // A record without the field is one an older Pane wrote: the
        // choice defaults to off, not an error.
        assert!(!reading(r#"{ "version": 1 }"#).unwrap().launch_at_login);
    }

    #[test]
    fn an_unparseable_record_is_a_problem() {
        let problem = reading("{ not a record");
        assert!(problem.is_err(), "{problem:?}");
        assert!(problem.unwrap_err().contains("is invalid"));
    }

    #[test]
    fn another_versions_record_is_not_read() {
        let problem = reading(r#"{ "version": 2, "theme": "light" }"#);
        assert!(problem.is_err(), "{problem:?}");
        assert!(
            problem
                .unwrap_err()
                .contains("which this Pane does not read"),
            "the version is named"
        );
    }

    #[test]
    fn a_record_without_a_version_is_not_read() {
        let problem = reading(r#"{ "theme": "light" }"#);
        assert!(problem.is_err(), "{problem:?}");
        assert!(problem.unwrap_err().contains("no version number"));
    }

    #[test]
    fn unknown_values_fail_the_whole_record() {
        for text in [
            r#"{ "version": 1, "theme": "sepia" }"#,
            r#"{ "version": 1, "material": "frost" }"#,
            r#"{ "version": 1, "theme": 3 }"#,
            r#"{ "version": 1, "launchAtLogin": "yes" }"#,
            r#"{ "version": 1, "openingMonitor": "nearest" }"#,
            r#"{ "version": 1, "reopening": "blank" }"#,
        ] {
            assert!(reading(text).is_err(), "{text} half-loads");
        }
    }

    #[test]
    fn a_failed_write_leaves_the_previous_record_whole() {
        let dir = tempfile::tempdir().unwrap();
        HostSettings::default().save(dir.path()).unwrap();
        // A folder where the record belongs: the atomic replacement fails,
        // and the record it would have replaced is still the old one.
        std::fs::remove_file(dir.path().join(FILE)).unwrap();
        std::fs::create_dir(dir.path().join(FILE)).unwrap();
        let failed = HostSettings {
            theme: ThemePreference::Light,
            material: MaterialPreference::Solid,
            open_pane: Shortcut::parse("ctrl+alt+b").unwrap(),
            launch_at_login: true,
            opening_monitor: super::OpeningMonitor::Pointer,
            reopening: super::Reopening::RootSearch,
        }
        .save(dir.path());
        assert!(failed.is_err(), "{failed:?}");
        assert!(dir.path().join(FILE).is_dir());
        // The temporary files of the failed write are cleaned up.
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [FILE]);
    }
}
