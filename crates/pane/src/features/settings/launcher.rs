//! The Launcher page: the choices that govern the launcher window — which
//! display it opens on, and what reopening it starts from.
//!
//! Every value it shows and every choice it takes goes through the host
//! settings ([`crate::settings`]), so the record's own rules — atomic
//! writes, an unreadable record never replaced, a failed save reported
//! with the shown choice rolled back — are the ones these choices live
//! by. Neither choice changes what the windows render, so choosing here
//! repaints nothing: the opening display is resolved against the display
//! layout every time the launcher opens (through [`crate::placement`],
//! the platform seam this page also reads to explain the choices), and
//! what reopening shows is applied when the launcher is next opened.
//!
//! What the page explains, as the General page does for its hotkey: the
//! choices the platform cannot answer — the pointer's display where the
//! system does not tell Pane where the pointer is, the active window's
//! display where it does not tell Pane which window is active — are
//! shown with their reason and not offered, rather than pretending they
//! succeeded; a platform that cannot choose the launcher's display at
//! all (Wayland) explains that instead; and a choice whose display is
//! disconnected falls back to the primary display, which the page says.
//! The dismissal behavior is the specification's Escape contract, not a
//! choice: backing out of what is open — a composition, a menu, a screen
//! — comes first, and hiding the launcher never quits Pane.

use gpui::{
    AnyElement, App, Context, Div, Hsla, Role, ScrollAnchor, Stateful, Toggled, Window, div,
    prelude::*, px,
};
use pane_core::placement::{DisplayLayout, resolve};
use pane_core::{Launcher, OpeningMonitor, Reopening};

use super::{Page, SettingsWindow, search};
use crate::ui::icon::{Glyph, IconTone};
use crate::ui::theme::Theme;

/// The opening-monitor choices the page offers, in row order: the
/// preference, the row's name and subtitle, and its test selector. The
/// subtitle of the default names it as the provisional default, so the
/// page does not silently turn the specification's proposal into a
/// confirmed product decision.
const MONITORS: [(OpeningMonitor, &str, &str, &str); 3] = [
    (
        OpeningMonitor::Primary,
        "Primary display",
        "The system's main display, the provisional default",
        "launcher-monitor-Primary",
    ),
    (
        OpeningMonitor::Pointer,
        "Pointer's display",
        "The display the pointer is on when the launcher opens",
        "launcher-monitor-Pointer",
    ),
    (
        OpeningMonitor::ActiveWindow,
        "Active window's display",
        "The display of the window you are working in",
        "launcher-monitor-ActiveWindow",
    ),
];

/// The reopening choices the page offers, in row order: the preference,
/// the row's name and subtitle, and its test selector.
const REOPENINGS: [(Reopening, &str, &str, &str); 2] = [
    (
        Reopening::RestoreView,
        "Restore the current view",
        "Show what the launcher was left on, when it is still valid; the provisional default",
        "launcher-reopening-RestoreView",
    ),
    (
        Reopening::RootSearch,
        "Start at root search",
        "Begin from root search with an empty query, whatever was left",
        "launcher-reopening-RootSearch",
    ),
];

/// The Launcher page, registered after General in the window's page list:
/// the page of the launcher window itself.
pub(crate) fn page() -> Page {
    Page {
        title: "Launcher",
        about: "The display the launcher opens on, and what reopening shows",
        icon: (IconTone::Command, Glyph::Monitor),
        render,
        search: entries,
        focus,
    }
}

/// The settings the page offers the sidebar's search: each choice of both
/// groups, named as the page names it, in the group it sits in, saying
/// why it cannot be used where the system does not answer it — the result
/// stays listed with its reason, as the control does on the page. The
/// reopening choices are no platform integration: they are always usable.
fn entries(_launcher: &Launcher, cx: &App) -> Vec<search::Entry> {
    let placement = crate::placement::shared(cx);
    let layout = placement.layout();
    let unavailable = placement.unavailable();
    let monitors = MONITORS.iter().map(|&(monitor, name, _, selector)| {
        search::Entry {
            control: Some(selector.into()),
            title: name.into(),
            group: Some("Opening monitor".into()),
            unavailable: match &unavailable {
                // The platform cannot choose the launcher's display at all:
                // every choice says so, as the page does.
                Some(why) => Some(why.clone()),
                None => unsupported(&layout, monitor),
            },
        }
    });
    let reopenings = REOPENINGS
        .iter()
        .map(|&(_, name, _, selector)| search::Entry {
            control: Some(selector.into()),
            title: name.into(),
            group: Some("Reopening".into()),
            unavailable: None,
        });
    monitors.chain(reopenings).collect()
}

/// The page's controls take no keyboard focus (they are chosen with the
/// pointer, as the reference's settings rows are), so a jump to one
/// reveals it where it drew and the sidebar keeps the focus: `false`.
fn focus(_: &mut SettingsWindow, _: &str, _: &mut Window, _: &mut Context<SettingsWindow>) -> bool {
    false
}

/// Draws the Launcher page: the opening-monitor group, the reopening
/// group, the dismissal note, and whatever the host settings and the
/// platform report — an unsupported choice, a fallback, a save that
/// failed.
fn render(
    this: &mut SettingsWindow,
    _window: &mut Window,
    cx: &mut Context<SettingsWindow>,
) -> AnyElement {
    let settings = crate::settings::shared(cx);
    let (chosen, reopening, status) = {
        let state = settings.read(cx);
        (state.opening_monitor(), state.reopening(), state.status())
    };
    let placement = crate::placement::shared(cx);
    let unavailable = placement.unavailable();
    // Whether the opening-monitor choices are offered at all: a platform
    // that cannot choose the launcher's display explains that instead.
    let offered = unavailable.is_none();
    let layout = placement.layout();
    let visuals = crate::settings::visuals(cx);
    let theme = &visuals.theme;
    let typography = &theme.typography;

    let page = div()
        .id("launcher")
        .debug_selector(|| "launcher".into())
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .id("launcher-title")
                .debug_selector(|| "launcher-title".into())
                .pb(px(8.))
                .text_size(typography.search_size)
                .font_weight(typography.medium)
                .text_color(theme.text_title)
                .child("Launcher"),
        )
        // A platform that cannot choose the launcher's display at all:
        // the reason, and no choices offered below.
        .when_some(
            unavailable.map(|why| {
                note(
                    "launcher-unavailable",
                    &format!("Not available: {why}"),
                    theme.warning,
                    theme,
                )
            }),
            |page, note| page.child(note),
        )
        .when(offered, |page| {
            page.child(group(
                "Opening monitor",
                MONITORS
                    .iter()
                    .map(|&(monitor, name, subtitle, selector)| {
                        // A choice whose answer the system does not give is
                        // shown with its reason, not offered: choosing it
                        // would pretend a placement that cannot be made.
                        let reason = unsupported(&layout, monitor);
                        // The choice's scroll anchor, which the search's
                        // reveal scrolls to (see the window's render).
                        let anchor = this.search_anchor(selector);
                        choice(
                            selector,
                            name,
                            subtitle,
                            monitor == chosen,
                            reason.is_none(),
                            reason,
                            anchor,
                            theme,
                            cx.listener(move |_, _, _, cx| {
                                crate::settings::shared(cx).update(cx, |settings, cx| {
                                    settings.set_opening_monitor(monitor, cx);
                                });
                            }),
                        )
                    })
                    .collect(),
                theme,
            ))
            // The choice's own honesty: what the launcher would open on
            // now, when that is not the display the choice names.
            .when_some(fallback_note(&layout, chosen, theme), |page, note| {
                page.child(note)
            })
        })
        .child(group(
            "Reopening",
            REOPENINGS
                .iter()
                .map(|&(preference, name, subtitle, selector)| {
                    let anchor = this.search_anchor(selector);
                    choice(
                        selector,
                        name,
                        subtitle,
                        preference == reopening,
                        true,
                        None,
                        anchor,
                        theme,
                        cx.listener(move |_, _, _, cx| {
                            crate::settings::shared(cx).update(cx, |settings, cx| {
                                settings.set_reopening(preference, cx);
                            });
                        }),
                    )
                })
                .collect(),
            theme,
        ))
        // Dismissal is not a choice: the specification's Escape contract
        // stands as it is, and this says what it is.
        .child(note(
            "launcher-dismissal",
            "Escape still backs out of what is open — a composition, a menu, an open screen — \
             before it clears the query, and only then hides the launcher. Hiding never quits \
             Pane: the Settings window stays open, and the next opening of the launcher \
             reuses the same live window.",
            theme.text_muted,
            theme,
        ))
        .when_some(status, |page, status| {
            page.child(note("launcher-status", &status, theme.danger, theme))
        });
    page.into_any_element()
}

/// Why `monitor`'s choice cannot be answered on this platform, if it
/// cannot: the layout's own `None`. The primary display is always
/// offered.
fn unsupported(layout: &DisplayLayout, monitor: OpeningMonitor) -> Option<String> {
    match monitor {
        OpeningMonitor::Primary => None,
        OpeningMonitor::Pointer if layout.pointer.is_none() => Some(
            "This system does not tell Pane where the pointer is outside its own windows, so \
             the launcher opens on the primary display instead"
                .into(),
        ),
        OpeningMonitor::ActiveWindow if layout.active.is_none() => Some(
            "This system does not tell Pane which window is active, so the launcher opens on \
             the primary display instead"
                .into(),
        ),
        _ => None,
    }
}

/// What the page says when the choice and the layout resolve to a
/// fallback: the resolution's own reason, as the opening that fell back
/// is placed on an available display and says so.
fn fallback_note(
    layout: &DisplayLayout,
    choice: OpeningMonitor,
    theme: &Theme,
) -> Option<Stateful<Div>> {
    resolve(layout, choice)
        .and_then(|resolved| resolved.fallback)
        .map(|reason| note("launcher-fallback", &reason, theme.warning, theme))
}

/// One choice group: its label (the section label style) and its rows,
/// with the radio group's semantics.
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

/// One choice row: the reference's row chrome carrying a radio's marks
/// and semantics. `chosen` is whether the row's choice is the one in
/// effect; `offered` is whether choosing it does anything (a choice whose
/// answer this system does not give is shown with its `reason`, not
/// offered); `anchor` is the scroll anchor the search's reveal scrolls
/// to; `on_click` reports the choice to the host settings, which records
/// and saves it.
#[allow(clippy::too_many_arguments)]
fn choice(
    selector: &'static str,
    name: &'static str,
    subtitle: &'static str,
    chosen: bool,
    offered: bool,
    reason: Option<String>,
    anchor: ScrollAnchor,
    theme: &Theme,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let typography = &theme.typography;
    let geometry = &theme.geometry;
    // What the row says under its name: the reason a choice cannot be
    // answered here, where it cannot, else the choice's own subtitle.
    let description = reason.unwrap_or_else(|| subtitle.to_owned());
    let row = div()
        .flex()
        .items_center()
        .gap(geometry.row_gap)
        .min_h(geometry.row_min_height)
        .px(geometry.row_padding_x)
        .rounded(geometry.row_radius)
        .when(offered, |row| {
            row.cursor_pointer()
                .when(!chosen, |row| row.hover(|row| row.bg(theme.row_hover)))
        })
        .when(!offered, |row| row.opacity(0.5).cursor_default())
        .when(chosen, |row| {
            row.bg(theme.row_selected).shadow(vec![
                gpui::BoxShadow::new(px(0.), px(0.), theme.row_selected_border)
                    .spread_radius(px(1.))
                    .inset(),
            ])
        })
        .child(
            // The radio's mark, as the extension form's choices render it.
            div()
                .flex_none()
                .w(px(18.))
                .text_size(typography.row_title_size)
                .text_color(theme.text_title)
                .child(if chosen { "◉" } else { "○" }),
        )
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
                        .child(name),
                )
                .child(
                    div()
                        .text_size(typography.row_subtitle_size)
                        .text_color(theme.text_muted)
                        .child(description.clone()),
                ),
        );
    row.id(name)
        .debug_selector(move || selector.into())
        .anchor_scroll(Some(anchor))
        .role(Role::RadioButton)
        .aria_label(name)
        // The reason a choice cannot be used here is read as the row's
        // description, as root search's rows read their subtitles.
        .aria_description(description)
        .aria_toggled(if chosen {
            Toggled::True
        } else {
            Toggled::False
        })
        .when(!offered, |row| row.aria_disabled(true))
        .when(offered, |row| row.on_click(on_click))
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
