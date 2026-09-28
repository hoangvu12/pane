//! Pane's native launcher window, rendered with GPUI CE.
//!
//! The window is a thin renderer over [`pane_core::Launcher`]: key and mouse
//! input call launcher actions, and each frame draws the launcher's snapshot.

use std::path::{Path, PathBuf};

use gpui::{
    App, Context, Div, FocusHandle, KeyBinding, PathPromptOptions, Role, SharedString, Stateful,
    Window, actions, div, prelude::*, rgb,
};
use pane_core::{CommandRegistration, INSTALL_FROM_FOLDER, Launcher, Row, Screen, Status};

actions!(launcher, [SelectNext, SelectPrevious, Confirm, Back]);

const KEY_CONTEXT: &str = "Launcher";

/// Registers the launcher's key bindings.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("down", SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("up", SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", Back, Some(KEY_CONTEXT)),
    ]);
}

/// The sample commands: (id, title, subtitle, component file name). Each
/// implements the same command in a different extension language.
const SAMPLES: [(&str, &str, &str, &str); 3] = [
    (
        "rust-sample",
        "Rust sample",
        "A sample command implemented by a Rust extension",
        "sample_rust.wasm",
    ),
    (
        "javascript-sample",
        "JavaScript sample",
        "The same command implemented by a JavaScript extension",
        "sample_js.wasm",
    ),
    (
        "typescript-sample",
        "TypeScript sample",
        "The same command implemented by a TypeScript extension",
        "sample_ts.wasm",
    ),
];

/// The commands this build offers: the Rust, JavaScript and TypeScript
/// sample commands.
///
/// Their components are read from `PANE_EXTENSIONS_DIR` when set, otherwise
/// from the development build output `target/guests`. A missing component
/// leaves its command listed; opening it explains what is missing.
pub fn sample_commands() -> Vec<CommandRegistration> {
    let dir = std::env::var_os("PANE_EXTENSIONS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/guests"));
    SAMPLES
        .iter()
        .map(|&(id, title, subtitle, file)| CommandRegistration {
            id: id.into(),
            title: title.into(),
            subtitle: Some(subtitle.into()),
            component: dir.join(file),
        })
        .collect()
}

/// Where Pane keeps disposable cached data, such as compiled extension code:
/// `%LOCALAPPDATA%\Pane\cache` on Windows, `~/Library/Caches/Pane` on
/// macOS and `$XDG_CACHE_HOME/pane` (default `~/.cache/pane`) elsewhere.
pub fn cache_dir() -> Option<PathBuf> {
    let env = |name| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(target_os = "windows") {
        env("LOCALAPPDATA").map(|dir| dir.join("Pane").join("cache"))
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|home| home.join("Library/Caches/Pane"))
    } else {
        env("XDG_CACHE_HOME")
            .or_else(|| env("HOME").map(|home| home.join(".cache")))
            .map(|dir| dir.join("pane"))
    }
}

/// Where Pane keeps installed extension packages: `PANE_DATA_DIR` when set,
/// otherwise `%LOCALAPPDATA%\Pane\data` on Windows,
/// `~/Library/Application Support/Pane` on macOS and `$XDG_DATA_HOME/pane`
/// (default `~/.local/share/pane`) elsewhere. Packages go in its
/// `extensions` folder.
pub fn data_dir() -> Option<PathBuf> {
    let env = |name| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    if let Some(dir) = env("PANE_DATA_DIR") {
        Some(dir)
    } else if cfg!(target_os = "windows") {
        env("LOCALAPPDATA").map(|dir| dir.join("Pane").join("data"))
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|home| home.join("Library/Application Support/Pane"))
    } else {
        env("XDG_DATA_HOME")
            .or_else(|| env("HOME").map(|home| home.join(".local/share")))
            .map(|dir| dir.join("pane"))
    }
}

/// The launcher window's root view.
pub struct LauncherWindow {
    launcher: Launcher,
    focus_handle: FocusHandle,
}

impl LauncherWindow {
    pub fn new(launcher: Launcher, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        LauncherWindow {
            launcher,
            focus_handle,
        }
    }

    pub fn launcher(&self) -> &Launcher {
        &self.launcher
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.launcher.move_selection(1);
        cx.notify();
    }

    fn select_previous(&mut self, _: &SelectPrevious, _: &mut Window, cx: &mut Context<Self>) {
        self.launcher.move_selection(-1);
        cx.notify();
    }

    fn confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        self.activate_selected(cx);
    }

    fn back(&mut self, _: &Back, _: &mut Window, cx: &mut Context<Self>) {
        self.launcher.back();
        cx.notify();
    }

    /// Shows the package in `folder` with its identity and compatibility,
    /// redrawing when the check finishes.
    pub fn preview_package(&mut self, folder: &Path, cx: &mut Context<Self>) {
        let pending = self.launcher.preview_package(folder);
        cx.notify();
        cx.spawn(async move |this, cx| {
            pending.await;
            this.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
    }

    /// Asks for a package folder with the platform's folder picker, then
    /// previews it. Cancelling leaves root search as it was.
    fn choose_package_folder(&mut self, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Install".into()),
        });
        cx.spawn(async move |this, cx| {
            let folder = match chosen.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) | Err(_) => None,
                Ok(Err(error)) => {
                    this.update(cx, |this, cx| {
                        this.launcher
                            .show_error(format!("Could not open a folder picker: {error:#}"));
                        cx.notify();
                    })
                    .ok();
                    None
                }
            };
            if let Some(folder) = folder {
                this.update(cx, |this, cx| this.preview_package(&folder, cx))
                    .ok();
            }
        })
        .detach();
    }

    /// Starts the selected row's action and redraws when the guest answers,
    /// without blocking the window meanwhile.
    fn activate_selected(&mut self, cx: &mut Context<Self>) {
        let view = self.launcher.view();
        let selected = view.selected.and_then(|index| view.rows.get(index));
        if selected.is_some_and(|row| row.id == INSTALL_FROM_FOLDER) {
            self.choose_package_folder(cx);
            return;
        }
        let pending = self.launcher.activate_selected();
        cx.notify();
        cx.spawn(async move |this, cx| {
            pending.await;
            this.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
    }

    fn render_row(
        &self,
        index: usize,
        row: Row,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(("row", index))
            .debug_selector(|| format!("row-{}", row.title))
            .flex()
            .flex_col()
            .px_3()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .role(Role::ListBoxOption)
            .aria_label(row.title.clone())
            .aria_selected(selected)
            .when(selected, |row| {
                row.aria_active_descendant().bg(rgb(0x364355))
            })
            .hover(|row| row.bg(rgb(0x2e3a48)))
            .child(div().child(row.title))
            .when_some(row.subtitle, |element, subtitle| {
                element
                    .aria_description(subtitle.clone())
                    .child(div().text_sm().text_color(rgb(0xaab4c0)).child(subtitle))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.launcher.select(index);
                this.activate_selected(cx);
            }))
    }
}

impl Render for LauncherWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = self.launcher.view();
        let (empty, hint) = match view.screen {
            Screen::Root => ("No commands are installed.", "↑↓ select · Enter open"),
            Screen::Command => (
                "This command has no items.",
                "↑↓ select · Enter run · Esc back",
            ),
            Screen::Package => ("Nothing to install.", "Enter confirm · Esc back"),
        };
        let details = view.details.into_iter().enumerate().map(|(index, line)| {
            div()
                .id(("detail", index))
                .debug_selector(|| format!("detail-{line}"))
                .text_sm()
                .text_color(rgb(0xaab4c0))
                .child(line)
        });
        let (status_selector, status_text, status_color): (&str, SharedString, u32) =
            match view.status {
                Status::Idle => ("status-idle", hint.into(), 0x8a96a3),
                Status::Running => ("status-running", "Running…".into(), 0xd6c27a),
                Status::Result(answer) => ("status-result", answer.into(), 0x9fd8a8),
                Status::Error(message) => ("status-error", message.into(), 0xf08c8c),
            };
        let rows: Vec<_> = view
            .rows
            .into_iter()
            .enumerate()
            .map(|(index, row)| {
                let selected = view.selected == Some(index);
                self.render_row(index, row, selected, cx)
            })
            .collect();

        div()
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::back))
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(rgb(0x20252d))
            .text_color(rgb(0xf1f3f5))
            .child(div().text_xl().child(view.title.clone()))
            .children(details)
            .child(
                div()
                    .id("rows")
                    // The list holds keyboard focus; the selected row is its
                    // active descendant, and key actions bubble to the root.
                    .track_focus(&self.focus_handle)
                    .role(Role::ListBox)
                    .aria_label(view.title.clone())
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .overflow_y_scroll()
                    .children(rows)
                    .when(view.selected.is_none(), |rows| {
                        rows.child(div().text_color(rgb(0x8a96a3)).child(empty))
                    }),
            )
            .child(
                div()
                    .id("status")
                    .role(Role::Status)
                    .aria_label(status_text.clone())
                    .debug_selector(|| status_selector.into())
                    .text_sm()
                    .text_color(rgb(status_color))
                    .child(status_text),
            )
    }
}
