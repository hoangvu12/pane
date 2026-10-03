//! The General page: the choices that govern Pane as a whole — today,
//! whether Pane starts at login.
//!
//! The choice, the registration and the platform's ability are three
//! different truths, and the page shows all three rather than one
//! pretense. The switch carries the user's *saved preference*; the note
//! under it carries what the platform actually holds or why the last
//! change failed (a registration that still awaits macOS's approval, a
//! system that refused, the freedesktop convention's limit on Linux);
//! and where the integration cannot manage a registration here at all —
//! an unsupported platform, a development build — the switch is not
//! offered, and the reason is shown instead. A failed registration,
//! removal or save is the page's status, never a switch that pretends
//! it succeeded. Every value shown and every choice taken goes through
//! the host settings ([`crate::settings`]), so the record's own rules
//! (atomic writes, an unreadable record never replaced) are the ones
//! this choice lives by.

use gpui::{AnyElement, App, Context, Div, Role, Stateful, Toggled, Window, div, prelude::*, px};
use pane_core::autostart::Registration;

use super::{Page, SettingsWindow};
use crate::ui::icon::{Glyph, IconTone};
use crate::ui::theme::Theme;

/// The General page, registered first in the window's page list: the
/// page of Pane as a whole, the one the window opens on. The Open Pane
/// hotkey and the tray entry that belong here come with their tickets.
pub(crate) fn page() -> Page {
    Page {
        title: "General",
        icon: (IconTone::Command, Glyph::Sliders),
        render,
    }
}

/// Draws the General page: the startup group with the launch-at-login
/// switch, its honesty note, and whatever the host settings report — a
/// save that failed, or a record that could not be read.
fn render(
    _this: &mut SettingsWindow,
    _window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    // Everything the page shows comes from the host settings: the
    // preference the switch carries, the registration the note explains,
    // and the status the record reports.
    let settings = crate::settings::shared(cx);
    let (preference, unavailable, registration, status) = {
        let state = settings.read(cx);
        (
            state.launch_at_login(),
            state.login_unavailable(),
            state.login_registration().clone(),
            state.status(),
        )
    };
    let visuals = crate::settings::visuals(cx);
    let theme = &visuals.theme;
    let typography = &theme.typography;

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
            "Startup",
            vec![switch(
                preference,
                unavailable.is_none(),
                theme,
                // The click reports the choice to the host settings: the
                // registration is changed, the record written, and the
                // switch redrawn with what was actually kept.
                cx.listener(move |_, _, _, cx| {
                    crate::settings::shared(cx).update(cx, |settings, cx| {
                        settings.set_launch_at_login(!preference, cx);
                    });
                }),
            )],
            theme,
        ))
        .when_some(
            login_note(preference, unavailable, registration, theme),
            |page, note| page.child(note),
        )
        .when_some(status, |page, status| {
            page.child(
                div()
                    .id("general-status")
                    .debug_selector(|| "general-status".into())
                    .pt(px(10.))
                    .role(Role::Status)
                    .aria_label(status.clone())
                    .text_size(typography.row_subtitle_size)
                    .text_color(theme.danger)
                    .child(status),
            )
        });
    page.into_any_element()
}

/// One choice group: its label (the section label style) and its rows.
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

/// The launch-at-login switch row: the reference's row chrome carrying a
/// switch's marks and semantics, the switch itself at the right. The
/// switch carries the *saved preference*; `offered` is whether choosing
/// it does anything (nothing is offered where the platform cannot manage
/// the registration, and the row says so by its state).
fn switch(
    preference: bool,
    offered: bool,
    theme: &Theme,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let typography = &theme.typography;
    let geometry = &theme.geometry;
    div()
        .flex()
        .items_center()
        .gap(geometry.row_gap)
        .min_h(geometry.row_min_height)
        .px(geometry.row_padding_x)
        .rounded(geometry.row_radius)
        .when(offered, |row| {
            row.cursor_pointer().hover(|row| row.bg(theme.row_hover))
        })
        .when(!offered, |row| row.opacity(0.5).cursor_default())
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
                        .child("Launch Pane at login"),
                )
                .child(
                    div()
                        .text_size(typography.row_subtitle_size)
                        .text_color(theme.text_muted)
                        .child("Pane is ready when you log in"),
                ),
        )
        .child(track(preference, theme))
        .id("launch-at-login")
        .debug_selector(|| "general-launch-at-login".into())
        .role(Role::Switch)
        .aria_label("Launch Pane at login")
        .aria_toggled(if preference {
            Toggled::True
        } else {
            Toggled::False
        })
        .when(!offered, |row| row.aria_disabled(true))
        .when(offered, |row| row.on_click(on_click))
}

/// The switch itself: the track with its knob, slid to the side the
/// preference names. Presentation only — the row owns the interaction.
fn track(preference: bool, theme: &Theme) -> Div {
    div()
        .flex_none()
        .flex()
        .w(px(36.))
        .h(px(20.))
        .px(px(2.))
        .items_center()
        .rounded(px(10.))
        // The track says the choice itself: the success tone when Pane
        // starts at login, the quiet hairline when it does not.
        .when(preference, |track| track.bg(theme.success))
        .when(!preference, |track| track.bg(theme.hairline))
        .when(preference, |track| track.justify_end())
        .child(
            div()
                .flex_none()
                .size(px(16.))
                .rounded(px(8.))
                .bg(theme.panel_solid)
                // The knob's lift off the track: a faint shadow.
                .shadow(vec![gpui::BoxShadow::new(
                    px(0.),
                    px(0.),
                    gpui::rgb_to_hsla(gpui::rgba(0x00000026)),
                )]),
        )
}

/// The note under the startup group, if the launch-at-login choice needs
/// one: why the integration is unavailable here, what the platform
/// actually holds when that differs from a working registration, or the
/// limit of the convention the platform uses. `None` when the preference
/// and the registration agree and the platform needs no explanation.
fn login_note(
    preference: bool,
    unavailable: Option<String>,
    registration: Result<Registration, String>,
    theme: &Theme,
) -> Option<Stateful<Div>> {
    let (text, color) = if let Some(reason) = unavailable {
        // The platform (or this build) cannot manage the registration
        // here at all: the reason, not a toggle that pretends.
        (reason, theme.warning)
    } else if let Err(problem) = registration {
        // The last query or change failed, and the preference is what it
        // was: the problem is the truth to show.
        (problem, theme.warning)
    } else if registration == Ok(Registration::NeedsApproval) {
        (
            "Pane is registered, but macOS asks for your approval: open System Settings, \
             under General → Login Items, and allow Pane."
                .into(),
            theme.text_muted,
        )
    } else if cfg!(target_os = "linux") && preference {
        (
            "The registration is an autostart entry in the freedesktop convention: the major \
             desktop environments start these, but not every desktop does, and Pane cannot see \
             whether it was started."
                .into(),
            theme.text_muted,
        )
    } else {
        return None;
    };
    Some(
        div()
            .id("general-login-note")
            .debug_selector(|| "general-login-note".into())
            .pt(px(6.))
            .role(Role::Status)
            .aria_label(text.clone())
            .text_size(theme.typography.row_kind_size)
            .text_color(color)
            .child(text),
    )
}
