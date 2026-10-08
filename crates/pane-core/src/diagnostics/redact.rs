//! Redaction at the log's writer, before anything is written: the home
//! folder's path becomes `~`, the user's name `<user>` and the computer's
//! name `<computer>`. Each is matched without regard to case (and with `/`
//! and `\` taken as one), and only when it is at least [`MIN_CHARS`]
//! characters long, so a short name does not eat ordinary words. Other
//! paths are kept: they are what a diagnosis needs.

use std::path::{Path, PathBuf};

/// The fewest characters a name must have to be redacted.
pub(crate) const MIN_CHARS: usize = 3;

/// What the log replaces before it writes.
#[derive(Clone, Debug, Default)]
pub(crate) struct Redactor {
    /// Each text and what replaces it, longest first, so the home folder's
    /// path wins over the user's name inside it.
    needles: Vec<(String, &'static str)>,
}

impl Redactor {
    /// One that replaces nothing (tests of other things).
    #[cfg(test)]
    pub(crate) fn none() -> Redactor {
        Redactor::default()
    }

    /// One replacing `home`, the `users`' names and the `computers`'.
    pub(crate) fn new(home: Option<&Path>, users: &[String], computers: &[String]) -> Redactor {
        let mut needles: Vec<(String, &'static str)> = Vec::new();
        let mut add = |text: &str, replacement: &'static str| {
            let text = text.trim();
            let known = needles
                .iter()
                .any(|(needle, _)| needle.to_lowercase() == text.to_lowercase());
            if text.chars().count() >= MIN_CHARS && !known {
                needles.push((text.to_owned(), replacement));
            }
        };
        if let Some(home) = home {
            let home = home.to_string_lossy();
            add(home.trim_end_matches(['/', '\\']), "~");
        }
        for user in users {
            add(user, "<user>");
        }
        for computer in computers {
            add(computer, "<computer>");
        }
        needles.sort_by_key(|(needle, _)| std::cmp::Reverse(needle.chars().count()));
        Redactor { needles }
    }

    /// One for this computer: its user's home folder, the user's name and
    /// the computer's, as the system reports them.
    pub(crate) fn of_this_system() -> Redactor {
        let home = home_folder();
        let mut users = user_names();
        // The home folder's own name is usually the user's.
        if let Some(name) = home
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
        {
            users.push(name.to_owned());
        }
        Redactor::new(home.as_deref(), &users, &computer_names())
    }

    /// `text` with every needle replaced.
    pub(crate) fn redact(&self, text: &str) -> String {
        if self.needles.is_empty() {
            return text.to_owned();
        }
        let mut redacted = String::with_capacity(text.len());
        let mut rest = text;
        'text: while let Some(next) = rest.chars().next() {
            for (needle, replacement) in &self.needles {
                if let Some(length) = matches_at(rest, needle) {
                    redacted.push_str(replacement);
                    rest = &rest[length..];
                    continue 'text;
                }
            }
            redacted.push(next);
            rest = &rest[next.len_utf8()..];
        }
        redacted
    }
}

/// How many bytes of `text` its start matches `needle` with, if it does:
/// letters in any case, and either slash for either.
fn matches_at(text: &str, needle: &str) -> Option<usize> {
    let mut length = 0;
    let mut chars = text.chars();
    for wanted in needle.chars() {
        let found = chars.next()?;
        if !same(found, wanted) {
            return None;
        }
        length += found.len_utf8();
    }
    Some(length)
}

fn same(found: char, wanted: char) -> bool {
    let slash = |c: char| c == '/' || c == '\\';
    found == wanted
        || (slash(found) && slash(wanted))
        || found.to_lowercase().eq(wanted.to_lowercase())
}

/// The user's home folder.
fn home_folder() -> Option<PathBuf> {
    let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(variable)
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

/// The user's name, as the system and the environment say it.
fn user_names() -> Vec<String> {
    let mut names = Vec::new();
    for variable in ["USERNAME", "USER", "LOGNAME"] {
        if let Some(name) = std::env::var(variable).ok().filter(|name| !name.is_empty()) {
            names.push(name);
        }
    }
    #[cfg(windows)]
    names.extend(windows_names::user());
    names
}

/// The computer's name, as the system and the environment say it.
fn computer_names() -> Vec<String> {
    let mut names = Vec::new();
    if let Some(name) = std::env::var("COMPUTERNAME")
        .ok()
        .filter(|name| !name.is_empty())
    {
        names.push(name);
    }
    #[cfg(windows)]
    names.extend(windows_names::computer());
    #[cfg(unix)]
    if let Some(host) = unix_names::host() {
        // `Alices-MacBook.local` is also written `Alices-MacBook`.
        if let Some((first, _)) = host.split_once('.') {
            names.push(first.to_owned());
        }
        names.push(host);
    }
    names
}

/// The names Windows reports.
#[cfg(windows)]
mod windows_names {
    use ::windows::Win32::System::WindowsProgramming::{GetComputerNameW, GetUserNameW};
    use ::windows::core::PWSTR;

    /// The text in `buffer` up to its first NUL.
    fn text(buffer: &[u16]) -> Option<String> {
        let length = buffer
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(buffer.len());
        let name = String::from_utf16_lossy(&buffer[..length]);
        (!name.trim().is_empty()).then_some(name)
    }

    /// The name of the user this process runs as.
    pub(super) fn user() -> Option<String> {
        // UNLEN (256) and its NUL.
        let mut buffer = [0u16; 257];
        let mut size = buffer.len() as u32;
        // SAFETY: `buffer` is writable for `size` units.
        unsafe { GetUserNameW(Some(PWSTR(buffer.as_mut_ptr())), &mut size) }.ok()?;
        text(&buffer)
    }

    /// The computer's NetBIOS name.
    pub(super) fn computer() -> Option<String> {
        // MAX_COMPUTERNAME_LENGTH (15) and its NUL, with room to spare.
        let mut buffer = [0u16; 64];
        let mut size = buffer.len() as u32;
        // SAFETY: `buffer` is writable for `size` units.
        unsafe { GetComputerNameW(Some(PWSTR(buffer.as_mut_ptr())), &mut size) }.ok()?;
        text(&buffer)
    }
}

/// The names a Unix system reports.
#[cfg(unix)]
mod unix_names {
    /// The computer's host name.
    pub(super) fn host() -> Option<String> {
        let mut buffer = [0u8; 256];
        // SAFETY: `buffer` is writable for its length; the last byte stays
        // NUL whatever the call writes.
        let answered = unsafe { libc::gethostname(buffer.as_mut_ptr().cast(), buffer.len() - 1) };
        if answered != 0 {
            return None;
        }
        let name = std::ffi::CStr::from_bytes_until_nul(&buffer).ok()?;
        let name = name.to_string_lossy().trim().to_owned();
        (!name.is_empty()).then_some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn redactor() -> Redactor {
        Redactor::new(
            Some(Path::new(r"C:\Users\Alice")),
            &["alice".to_owned()],
            &["DESK-42".to_owned()],
        )
    }

    #[test]
    fn the_home_folder_becomes_a_tilde_in_either_slash_and_any_case() {
        let redactor = redactor();
        assert_eq!(
            redactor.redact(r"could not read C:\Users\Alice\AppData\Local\Pane\x.json"),
            r"could not read ~\AppData\Local\Pane\x.json"
        );
        assert_eq!(
            redactor.redact("could not read c:/users/ALICE/notes"),
            "could not read ~/notes"
        );
        // Another path is kept: it is what a diagnosis needs.
        assert_eq!(
            redactor.redact(r"D:\Projects\pane\pane.json"),
            r"D:\Projects\pane\pane.json"
        );
    }

    #[test]
    fn the_users_and_the_computers_names_go_in_any_case() {
        let redactor = redactor();
        assert_eq!(
            redactor.redact("ALICE asked desk-42 and Desk-42 for Alice's file"),
            "<user> asked <computer> and <computer> for <user>'s file"
        );
        assert_eq!(
            redactor.redact(r"\\DESK-42\share\alice"),
            r"\\<computer>\share\<user>"
        );
    }

    #[test]
    fn a_name_shorter_than_three_characters_is_left_alone() {
        let redactor = Redactor::new(
            Some(Path::new("/")),
            &["al".to_owned()],
            &["pc".to_owned()],
        );
        assert_eq!(
            redactor.redact("al on PC wrote / and /al"),
            "al on PC wrote / and /al"
        );
        // Three characters are enough.
        let redactor = Redactor::new(None, &["ali".to_owned()], &["box".to_owned()]);
        assert_eq!(redactor.redact("Ali's BOX"), "<user>'s <computer>");
    }

    #[test]
    fn names_with_letters_beyond_ascii_match_in_any_case() {
        let redactor = Redactor::new(None, &["Jürgen".to_owned()], &[]);
        assert_eq!(redactor.redact("JÜRGEN and jürgen"), "<user> and <user>");
    }

    #[test]
    fn this_systems_names_are_found() {
        // Whatever this computer is called, its home folder is redacted.
        let redactor = Redactor::of_this_system();
        let Some(home) = home_folder() else {
            return;
        };
        if home.to_string_lossy().len() >= MIN_CHARS {
            let path = home.join("notes.txt");
            let redacted = redactor.redact(&path.to_string_lossy());
            assert!(redacted.starts_with('~'), "{redacted}");
        }
    }
}
