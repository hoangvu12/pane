//! The General page: the Open Pane hotkey — the application-owned global
//! binding that summons the launcher from any application, recorded here
//! and applied through the platform's global-shortcut registration — and
//! the tray or menu-bar visibility, the preference that shows and hides
//! Pane's native entry, applied through the platform's tray adapter.
//!
//! The binding is Pane's own, not any extension's: the record it is kept
//! in is the host settings' (`settings.json`, whose rules the entity in
//! `crate::settings` keeps), it is registered through the launcher this
//! window shares with the launcher window, and it stays registered while
//! every extension is disabled and while the extension runtime has
//! failed — nothing of its lifecycle belongs to a package. The tray
//! visibility is a host setting the same way: the entry is the platform's
//! own — the one place outside Pane's windows whose menu opens the
//! launcher, Settings and Quit — so hiding it leaves the launcher's
//! footer menu, its Settings root result and the local Settings shortcut
//! as the entry points they always were, and a launcher whose Open Pane
//! binding failed keeps its window: nothing here can hide the only
//! usable recovery window, because the entry is the only thing the
//! preference ever hides.
//!
//! The recorder captures keys without executing them. While it listens,
//! its row holds focus and its key context swallows the keys that would
//! otherwise act (the sidebar's navigation, Tab's traversal), so a
//! captured combination records instead of navigating; Escape cancels
//! recording, changing nothing. A captured combination is checked —
//! against the combinations the system keeps for itself (the Windows key,
//! Spotlight, the window menu — never taken over silently) and against
//! the command hotkeys — and registered *before* the binding it replaces
//! is released, so a refusal, a cancellation, a reset that cannot be
//! applied or a save that fails never discards the previous working
//! binding. Reset goes back to the provisional default through the same
//! checks as recording.
//!
//! What the page explains: the binding's state — why a chosen one is not
//! registered, including the Wayland limitation and the desktop-shortcut
//! guidance the adapter itself carries — the reason a recording was
//! refused, and what a save reported. A binding that could not be
//! registered at startup keeps the entry itself visible and honest
//! rather than hiding the only window that can fix it.

use gpui::{
    AnyElement, App, Context, Div, FocusHandle, Hsla, KeyBinding, KeyDownEvent, MouseDownEvent,
    Role, Stateful, Toggled, Window, actions, div, prelude::*, px,
};
use pane_core::hotkeys::Shortcut;

use super::{Page, SettingsWindow};
use crate::ui::icon::{Glyph, IconTone};
use crate::ui::theme::Theme;

/// The tray row's title, in the platform's own terms for the entry: the
/// menu bar's status item on macOS, the notification area's tray icon
/// elsewhere.
fn tray_row_title() -> &'static str {
    if cfg!(target_os = "macos") {
        "Show in Menu Bar"
    } else {
        "Show in tray"
    }
}

/// The tray row's subtitle, in the same terms: what the entry is and
/// what its menu holds.
fn tray_row_subtitle() -> &'static str {
    if cfg!(target_os = "macos") {
        "Pane's menu-bar item, with Open Pane, Settings and Quit Pane"
    } else {
        "Pane's item in the notification area, with Open Pane, Settings and Exit"
    }
}

/// The recorder row's key context: while the recorder holds focus, its
/// keys are the binding being recorded, not the window's navigation.
const RECORDER: &str = "OpenPaneRecorder";

actions!(general, [ActivateRecorder, CancelRecording]);

/// Registers the recorder's key bindings, in the recorder's own context —
/// deeper in the focus stack than the sidebar's and the window's keys, so
/// while the recorder holds focus they never fall through. The navigation
/// and traversal keys are bound to [`gpui::NoAction`] there: a key the
/// recorder cannot use does nothing instead of acting, and everything
/// else reaches the recorder's own key handler as the combination being
/// recorded.
pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        // The row is a button: Enter and Space activate it, as a click does.
        KeyBinding::new("enter", ActivateRecorder, Some(RECORDER)),
        KeyBinding::new("space", ActivateRecorder, Some(RECORDER)),
        // Escape cancels recording.
        KeyBinding::new("escape", CancelRecording, Some(RECORDER)),
        // Swallowed while the recorder holds focus (and harmless when it
        // does not): the sidebar's navigation and Tab's traversal stay
        // put, so captured keys never act.
        KeyBinding::new("down", gpui::NoAction, Some(RECORDER)),
        KeyBinding::new("up", gpui::NoAction, Some(RECORDER)),
        KeyBinding::new("tab", gpui::NoAction, Some(RECORDER)),
        KeyBinding::new("shift-tab", gpui::NoAction, Some(RECORDER)),
    ]);
}

/// The General page, registered first in the window's page list: the one
/// the window opens on, holding the Open Pane hotkey.
pub(crate) fn page() -> Page {
    Page {
        title: "General",
        icon: (IconTone::Command, Glyph::Prompt),
        render,
    }
}

/// The General page's state, held by the window as a field: the recorder
/// and the tray toggle's last refusal.
pub(crate) struct State {
    /// Whether the recorder is listening for a new binding.
    recording: bool,
    /// Why the last attempt was refused, if it was: a validation, a
    /// collision or the system's refusal. Shown as the page's status; the
    /// recorder keeps listening for another try.
    rejection: Option<String>,
    /// Why the last tray or menu-bar toggle was refused, if it was: the
    /// system's refusal, or the platform's lack of an entry. Shown as the
    /// page's status; the entry's own state is explained beside the
    /// toggle.
    tray_refusal: Option<String>,
    /// The recorder row's focus, held while it listens (and a tab stop
    /// otherwise, so the keyboard reaches the row).
    focus: FocusHandle,
}

impl State {
    /// The recorder's state, over the window's `cx` (its focus handle).
    pub(crate) fn new(cx: &mut Context<SettingsWindow>) -> State {
        State {
            recording: false,
            rejection: None,
            tray_refusal: None,
            focus: cx.focus_handle().tab_stop(true),
        }
    }
}

/// Draws the General page: the Open Pane hotkey — its recorder and reset —
/// what the binding's state explains, and what the last attempt or save
/// reported.
fn render(
    this: &mut SettingsWindow,
    _window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    let theme = crate::settings::visuals(cx).theme;
    let typography = &theme.typography;
    // Everything the page shows about the binding comes from the host
    // settings (the choice, the save's word) and the launcher (what is
    // registered, and why not): what the record holds and what actually
    // works stay distinguishable.
    let (choice, status) = {
        let settings = crate::settings::shared(cx).read(cx);
        (settings.open_pane(), settings.status())
    };
    let problem = this.launcher.open_pane_problem();
    let rejection = this.general.rejection.clone();
    let resettable = choice != Shortcut::open_pane_default();
    let (tray_visible, tray_status) = {
        let settings = crate::settings::shared(cx).read(cx);
        (settings.tray_visible(), settings.tray_status())
    };
    let tray_refusal = this.general.tray_refusal.clone();

    let page = div()
        .id("general")
        .debug_selector(|| "general".into())
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .id("general-title")
                .debug_selector(|| "general-title".into())
                .pb(px(8.))
                .text_size(typography.search_size)
                .font_weight(typography.medium)
                .text_color(theme.text_title)
                .child("General"),
        )
        .child(group(
            "Open Pane",
            vec![recorder_row(this, cx), reset_row(resettable, cx)],
            &theme,
        ))
        .child(group(
            "Application",
            vec![tray_row(tray_visible, cx)],
            &theme,
        ))
        // The binding's state: why the chosen one is not registered — a
        // registration the system refused, or a system where global
        // hotkeys cannot be used at all, with the adapter's own
        // explanation (Wayland's limitation and the desktop-shortcut
        // guidance it carries).
        .when_some(problem, |page, problem| {
            page.child(note(
                "open-pane-note",
                &format!("Not active: {problem}"),
                theme.warning,
                &theme,
            ))
        })
        // The entry's state: why the native entry is not what the
        // preference names — a system with no tray or menu-bar entry at
        // all (Linux today, with the adapter's own guidance), or a show
        // or hide the system refused. An unavailable entry is explained
        // rather than represented as a successful toggle.
        .when_some(tray_status, |page, status| {
            page.child(note("tray-note", &status, theme.warning, &theme))
        })
        // What the last tray toggle was refused with, if anything.
        .when_some(tray_refusal, |page, refusal| {
            page.child(note("tray-refusal", &refusal, theme.danger, &theme))
        })
        // What the last attempt to record a binding was refused with, if
        // anything.
        .when_some(rejection, |page, rejection| {
            page.child(note("general-refusal", &rejection, theme.danger, &theme))
        })
        // What a save reported, if it failed — the same status the
        // Appearance page shows for its own choices.
        .when_some(status, |page, status| {
            page.child(note("general-status", &status, theme.danger, &theme))
        });
    page.into_any_element()
}

/// One labelled group of rows, as the Appearance page's groups.
fn group(label: &'static str, rows: Vec<Stateful<Div>>, theme: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .pb(px(4.))
                .text_size(theme.typography.row_kind_size)
                .font_weight(theme.typography.medium)
                .text_color(theme.text_muted)
                .child(label),
        )
        .children(rows)
}

/// The Open Pane hotkey's recorder row: a button that shows the binding
/// in effect and, clicked or pressed with Enter, listens for the keys of
/// the next one. While it listens it holds focus, shows what it is doing,
/// and takes the keys pressed as the binding being recorded (Escape
/// cancels); a combination that is refused keeps it listening for another
/// try.
fn recorder_row(this: &mut SettingsWindow, cx: &mut Context<SettingsWindow>) -> Stateful<Div> {
    let theme = crate::settings::visuals(cx).theme;
    let typography = &theme.typography;
    let geometry = &theme.geometry;
    let recording = this.general.recording;
    let choice = crate::settings::shared(cx).read(cx).open_pane();
    let binding = format!("{choice}");
    let subtitle = if recording {
        "The keys are captured here: they do not act"
    } else {
        "Shows the launcher from any application, and hides it when it has focus"
    };
    let label = format!(
        "{}Open Pane with {binding}",
        if recording { "Recording; " } else { "" }
    );
    let focus = this.general.focus.clone();
    div()
        .id("open-pane-recorder")
        .debug_selector(|| "open-pane-recorder".into())
        .flex()
        .items_center()
        .gap(geometry.row_gap)
        .min_h(geometry.row_min_height)
        .px(geometry.row_padding_x)
        .rounded(geometry.row_radius)
        .cursor_pointer()
        .hover(|row| row.bg(theme.row_hover))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .truncate()
                        .text_size(typography.row_title_size)
                        .font_weight(typography.medium)
                        .text_color(theme.text_title)
                        .child(if recording {
                            "Press the keys…"
                        } else {
                            "Record shortcut"
                        }),
                )
                .child(
                    div()
                        .text_size(typography.row_subtitle_size)
                        .text_color(theme.text_muted)
                        .child(subtitle),
                ),
        )
        .child(binding_chip(&binding, recording, &theme))
        .key_context(RECORDER)
        .track_focus(&focus)
        .role(Role::Button)
        .aria_label(label)
        .aria_description(subtitle)
        .on_action(cx.listener(SettingsWindow::activate_recorder))
        .on_action(cx.listener(SettingsWindow::cancel_recording))
        .on_key_down(cx.listener(SettingsWindow::recorder_key_down))
        // A mouse-down anywhere outside the row while it listens cancels
        // the recording and is consumed, as the footer menu's popup does:
        // the click underneath does not act, and the recorder gives up
        // the keys.
        .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, window, cx| {
            if this.general.recording {
                this.stop_recording(window, cx);
                cx.stop_propagation();
            }
        }))
        .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
            if !this.general.recording {
                this.general.recording = true;
                this.general.rejection = None;
                window.focus(&this.general.focus, cx);
                cx.notify();
            }
        }))
}

/// The binding the row shows: the shortcut as the user names it, in the
/// keycap chrome — or, while recording, the listening mark.
fn binding_chip(binding: &str, recording: bool, theme: &Theme) -> Stateful<Div> {
    let typography = &theme.typography;
    let geometry = &theme.geometry;
    div()
        .id("open-pane-binding")
        .debug_selector(|| "open-pane-binding".into())
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .h(geometry.keycap_height)
        .px(geometry.keycap_padding_x)
        .rounded(geometry.keycap_radius)
        .bg(theme.tile_background)
        .text_size(typography.row_title_size)
        .font_weight(typography.medium)
        .text_color(if recording {
            theme.warning
        } else {
            theme.text_title
        })
        .child(if recording {
            "…".to_owned()
        } else {
            binding.to_owned()
        })
}

/// The tray or menu-bar visibility row: a checkbox that shows the
/// preference in effect and, clicked, applies its other value through
/// the host settings — which apply it to the native entry first and save
/// only what took. The mark carries the row's state for the keyboard
/// and assistive technology, as the appearance choices' radios do.
fn tray_row(visible: bool, cx: &mut Context<SettingsWindow>) -> Stateful<Div> {
    let theme = crate::settings::visuals(cx).theme;
    let typography = &theme.typography;
    let geometry = &theme.geometry;
    let title = tray_row_title();
    let subtitle = tray_row_subtitle();
    div()
        .flex()
        .items_center()
        .gap(geometry.row_gap)
        .min_h(geometry.row_min_height)
        .px(geometry.row_padding_x)
        .rounded(geometry.row_radius)
        .cursor_pointer()
        .hover(|row| row.bg(theme.row_hover))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .truncate()
                        .text_size(typography.row_title_size)
                        .font_weight(typography.medium)
                        .text_color(theme.text_title)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(typography.row_subtitle_size)
                        .text_color(theme.text_muted)
                        .child(subtitle),
                ),
        )
        .child(
            // The checkbox's mark, as the appearance rows' radios are
            // drawn: the state itself, not an interactive control beside
            // the row — the row is the control.
            div()
                .flex_none()
                .w(px(18.))
                .text_size(typography.row_title_size)
                .text_color(theme.text_title)
                .child(if visible { "✓" } else { " " }),
        )
        .id("tray-visibility")
        .debug_selector(|| "tray-visibility".into())
        .role(Role::CheckBox)
        .aria_label(title)
        .aria_description(subtitle)
        .aria_toggled(if visible {
            Toggled::True
        } else {
            Toggled::False
        })
        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
            let visible = crate::settings::shared(cx).read(cx).tray_visible();
            this.apply_tray_visible(!visible, cx);
        }))
}

/// The reset row: back to the provisional default, through the same
/// checks as recording (a reset that cannot be applied is refused and
/// leaves the binding working). Inert while the default is the choice.
fn reset_row(resettable: bool, cx: &mut Context<SettingsWindow>) -> Stateful<Div> {
    let theme = crate::settings::visuals(cx).theme;
    let typography = &theme.typography;
    let geometry = &theme.geometry;
    let default = Shortcut::open_pane_default();
    let row = div()
        .flex()
        .items_center()
        .gap(geometry.row_gap)
        .min_h(geometry.row_min_height)
        .px(geometry.row_padding_x)
        .rounded(geometry.row_radius)
        .when(resettable, |row| {
            row.cursor_pointer().hover(|row| row.bg(theme.row_hover))
        })
        .when(!resettable, |row| row.opacity(0.5).cursor_default())
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .truncate()
                        .text_size(typography.row_title_size)
                        .font_weight(typography.medium)
                        .text_color(theme.text_title)
                        .child("Reset"),
                )
                .child(
                    div()
                        .text_size(typography.row_subtitle_size)
                        .text_color(theme.text_muted)
                        .child(format!("Back to {default}, the provisional default")),
                ),
        );
    row.id("open-pane-reset")
        .debug_selector(|| "open-pane-reset".into())
        .role(Role::Button)
        .aria_label(format!("Reset the Open Pane hotkey to {default}"))
        .when(!resettable, |row| row.aria_disabled(true))
        .when(resettable, |row| {
            row.on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                this.apply_open_pane(Shortcut::open_pane_default(), window, cx);
            }))
        })
}

/// One explanatory line of the page: `text` in `color`, named for
/// assistive technology and drawn as a status.
fn note(selector: &'static str, text: &str, color: Hsla, theme: &Theme) -> Stateful<Div> {
    div()
        .id(selector)
        .debug_selector(move || selector.into())
        .pt(px(6.))
        .role(Role::Status)
        .aria_label(text.to_owned())
        .text_size(theme.typography.row_subtitle_size)
        .text_color(color)
        .child(text.to_owned())
}

impl SettingsWindow {
    /// The recorder's activation: Enter, Space or a click on its row. With
    /// no recording in progress it starts listening; while it listens the
    /// keys are captured, so this does nothing — Enter and Space cannot be
    /// part of a binding, and the recorder keeps listening.
    fn activate_recorder(
        &mut self,
        _: &ActivateRecorder,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        if !self.general.recording {
            self.start_recorder(window, cx);
        }
    }

    /// Escape on the recorder's row: cancels recording, changing nothing.
    fn cancel_recording(
        &mut self,
        _: &CancelRecording,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        if self.general.recording {
            self.stop_recording(window, cx);
        }
    }

    /// Starts listening: the recorder row takes focus, so the keys
    /// pressed next are the binding being recorded.
    fn start_recorder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.general.recording = true;
        self.general.rejection = None;
        let focus = self.general.focus.clone();
        window.focus(&focus, cx);
        cx.notify();
    }

    /// Stops listening, without changing anything: focus returns to the
    /// sidebar, the window's own keyboard focus.
    fn stop_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.general.recording = false;
        self.general.rejection = None;
        let sidebar = self.focus.clone();
        window.focus(&sidebar, cx);
        cx.notify();
    }

    /// A key pressed while the recorder listens: the combination it names
    /// is the binding to record. Keys the window binds (Enter, Space,
    /// Escape, the navigation and traversal keys) never reach here — they
    /// are bound in the recorder's context and handled above.
    fn recorder_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.general.recording {
            return;
        }
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        cx.stop_propagation();
        let shortcut = Shortcut::new(
            modifiers.control,
            modifiers.alt,
            modifiers.shift,
            modifiers.platform,
            &keystroke.key,
        );
        match shortcut {
            Ok(shortcut) => self.apply_open_pane(shortcut, window, cx),
            Err(problem) => {
                self.general.rejection = Some(format!("{problem}."));
                cx.notify();
            }
        }
    }

    /// Applies `shortcut` as the Open Pane hotkey: through the host
    /// settings, which register it with the system first and only then
    /// keep and save the choice. A refusal leaves the previous binding
    /// working and nothing saved; the reason is the page's status, and the
    /// recorder — if one is listening — keeps listening for another try.
    fn apply_open_pane(&mut self, shortcut: Shortcut, window: &mut Window, cx: &mut Context<Self>) {
        let applied = crate::settings::shared(cx)
            .update(cx, |settings, cx| settings.set_open_pane(shortcut, cx));
        match applied {
            Ok(()) => {
                // The change landed: the recorder is done, and focus
                // returns to the sidebar.
                self.general.recording = false;
                self.general.rejection = None;
                let sidebar = self.focus.clone();
                window.focus(&sidebar, cx);
                cx.notify();
            }
            Err(reason) => {
                self.general.rejection = Some(reason);
                cx.notify();
            }
        }
    }

    /// Applies `visible` as the tray or menu-bar visibility, as the
    /// toggle's click does: through the host settings, which apply it to
    /// the native entry first and only then keep and save the choice. A
    /// refusal leaves the entry and the record as they were; the reason
    /// is the page's status, and the entry's own state is explained
    /// beside the toggle whether or not a change was attempted.
    fn apply_tray_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        let applied = crate::settings::shared(cx)
            .update(cx, |settings, cx| settings.set_tray_visible(visible, cx));
        match applied {
            Ok(()) => self.general.tray_refusal = None,
            Err(reason) => self.general.tray_refusal = Some(reason),
        }
        cx.notify();
    }
}
