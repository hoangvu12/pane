//! Pane's file search, a default extension: the user chooses one folder
//! through its command's form, and typing into root search lists the files
//! in it whose names (or folders) match; invoking one opens it with the
//! system's handler for its type. Pane's host lists the folder under its
//! bounded scan policy (`pane_guest::files`); the extension keeps the folder
//! in its settings, matches the files and answers `open-file` results,
//! which Pane opens. Disabling the package removes its results and stops
//! any listing it has running.
#![no_std]

mod matching;

use pane_guest::alloc::{format, string::String, vec, vec::Vec};
use pane_guest::files::{self, FolderListing};
use pane_guest::root::{RootAction, RootResult};
use pane_guest::{
    CustomView, Field, FieldKind, FieldValue, Form, FormError, Guest, Item, NoCustomView,
    TextField, View, settings,
};

struct Files;
pane_guest::export!(Files);
pane_guest::root::export!(Files);

/// The settings key holding the chosen folder.
const FOLDER: &str = "folder";
/// The item whose form chooses the folder.
const CHOOSE: &str = "choose";
/// The item that forgets the chosen folder.
const FORGET: &str = "forget";

/// The chosen folder, if there is one.
fn chosen() -> Result<Option<String>, String> {
    Ok(settings::get(FOLDER)?.filter(|folder| !folder.is_empty()))
}

fn choose_form(current: Option<&str>) -> Form {
    let placeholder = match current {
        Some(folder) => format!("{folder} (unchanged)"),
        None => String::from("The folder's full path"),
    };
    Form {
        title: String::from("Choose the folder to search"),
        fields: vec![Field {
            id: FOLDER.into(),
            label: "Folder".into(),
            kind: FieldKind::Text(TextField {
                placeholder: Some(placeholder),
            }),
        }],
        submit_label: "Search this folder".into(),
    }
}

/// "5 files", with a note when Pane stopped at its limits.
fn described(listing: &FolderListing) -> String {
    let count = listing.files.len();
    let files = if count == 1 { "file" } else { "files" };
    let limits = if listing.truncated {
        "; Pane stopped at its limits (8 folders deep, 5,000 files, 20,000 entries), so some files are not found"
    } else {
        ""
    };
    format!("{count} {files}{limits}")
}

impl Guest for Files {
    type CustomView = NoCustomView;

    async fn get_view() -> Result<View, String> {
        let folder = chosen()?;
        let subtitle = match &folder {
            Some(folder) => format!("Searching {folder} · Enter chooses another"),
            None => String::from("No folder chosen yet: root search finds no files"),
        };
        let mut items = vec![Item {
            id: CHOOSE.into(),
            title: "Choose folder".into(),
            subtitle: Some(subtitle),
            form: Some(choose_form(folder.as_deref())),
            platforms: None,
            custom_view: None,
        }];
        if folder.is_some() {
            items.push(Item {
                id: FORGET.into(),
                title: "Stop searching the folder".into(),
                subtitle: Some("Root search then finds no files; the folder is not changed".into()),
                form: None,
                platforms: None,
                custom_view: None,
            });
        }
        items.push(Item {
            id: "policy".into(),
            title: "What is searched".into(),
            subtitle: Some(
                "Files of the folder and its subfolders, 8 deep, at most 5,000; not hidden files or links"
                    .into(),
            ),
            form: None,
            platforms: None,
            custom_view: None,
        });
        Ok(View {
            title: "Files".into(),
            items,
        })
    }

    async fn run_action(item_id: String) -> Result<String, String> {
        match item_id.as_str() {
            FORGET => {
                settings::set(FOLDER, "")?;
                Ok("Root search no longer finds files".into())
            }
            "policy" => Ok(
                "Pane lists regular files breadth first, each folder in name order, at most 8 \
                 folders deep, 5,000 files and 20,000 entries; it skips hidden entries, links \
                 and folders it cannot read"
                    .into(),
            ),
            _ => Err(format!("unknown item: {item_id}")),
        }
    }

    async fn submit_form(item_id: String, values: Vec<FieldValue>) -> Result<String, FormError> {
        let error = |field: Option<&str>, message: String| FormError {
            field: field.map(Into::into),
            message,
        };
        if item_id != CHOOSE {
            return Err(error(None, format!("unknown form: {item_id}")));
        }
        let typed = values
            .iter()
            .find(|value| value.id == FOLDER)
            .map_or("", |value| value.value.trim());
        let folder = match (typed, chosen().map_err(|problem| error(None, problem))?) {
            ("", Some(current)) => current,
            ("", None) => return Err(error(Some(FOLDER), "Enter the folder's full path".into())),
            (typed, _) => typed.into(),
        };
        // The folder must be one Pane can list now; its error names why not.
        let listing = files::list_folder(folder.clone())
            .await
            .map_err(|problem| error(Some(FOLDER), problem))?;
        settings::set(FOLDER, &folder).map_err(|problem| error(None, problem))?;
        Ok(format!(
            "Searching “{}”: {}",
            matching::last_name(&folder),
            described(&listing)
        ))
    }

    async fn open_view(item_id: String) -> Result<CustomView, String> {
        Err(format!("unknown view: {item_id}"))
    }
}

impl pane_guest::root::Guest for Files {
    /// The files of the chosen folder the query finds, each opening the
    /// file; none while no folder is chosen.
    async fn results_for(query: String) -> Result<Vec<RootResult>, String> {
        let Some(folder) = chosen()? else {
            return Ok(Vec::new());
        };
        let listing = files::list_folder(folder.clone())
            .await
            .map_err(|problem| format!("cannot search the chosen folder: {problem}"))?;
        let name = matching::last_name(&folder);
        Ok(matching::matching(&listing.files, &query)
            .into_iter()
            .map(|file| {
                let parent = matching::parent(&file.relative);
                let within = if parent.is_empty() {
                    String::from(name)
                } else {
                    format!("{name}/{parent}")
                };
                RootResult {
                    id: file.relative.clone(),
                    title: matching::last_name(&file.relative).into(),
                    subtitle: Some(format!("File in {within}")),
                    action: RootAction::OpenFile(file.path.clone()),
                }
            })
            .collect())
    }
}
