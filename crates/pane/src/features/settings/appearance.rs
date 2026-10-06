//! The Appearance section of the General page: the theme and material
//! choices (#98), as two rows of segmented choices.
//!
//! Every value it shows and every choice it takes goes through the host
//! settings ([`crate::settings`]) — the one entity the launcher window and
//! the Settings window both observe — so a choice made here repaints both
//! windows at once, without a restart, and is written to the record off
//! the window's thread. What the section itself owns is presentation only:
//! the rows and the honesty notes (glass's platform fallback, an override
//! in force, a save that failed).
//!
//! It offers only the settings Pane has (#100): the theme (System, Light,
//! Dark) and the material (Glass and Solid). The board's accent swatches,
//! blur and tint sliders, density, toggles and preview are not Pane's; the
//! visual workbench's reference fixture draws them to measure the shared
//! controls against the board.
//!
//! The material's segments carry the *preference*; where the platform
//! cannot provide glass, the material row says so, and the windows render
//! the solid surface.
//!
//! The segments are the keyboard's too: each is a tab stop, and Enter or
//! Space chooses it as a click does, under Pane's focus ring (the board
//! draws none). While an override is in force they are drawn at the
//! board's disabled opacity, and neither take the keyboard nor answer a
//! click.

use std::collections::HashMap;

use gpui::{
    AnyElement, App, Context, Div, FocusHandle, Hsla, KeyBinding, Role, ScrollAnchor, SharedString,
    Stateful, Toggled, actions, div, prelude::*,
};
use pane_core::{Launcher, MaterialPreference, ThemePreference};

use super::{SettingsWindow, search};
use crate::ui::controls;
use crate::ui::theme::Theme;

actions!(appearance, [Choose]);

/// The key context a segment carries while it has the keyboard.
const CHOICE: &str = "AppearanceChoice";

/// Registers the segments' keys: a segment is a button of a radio group,
/// so Enter and Space choose it, as a click does.
pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", Choose, Some(CHOICE)),
        KeyBinding::new("space", Choose, Some(CHOICE)),
    ]);
}

/// The section's label on the General page.
pub(crate) const LABEL: &str = "Appearance";

/// The theme choices the section offers, in segment order: the
/// preference, the segment's name and its test selector.
pub(crate) const THEMES: [(ThemePreference, &str, &str); 3] = [
    (ThemePreference::System, "System", "appearance-theme-System"),
    (ThemePreference::Light, "Light", "appearance-theme-Light"),
    (ThemePreference::Dark, "Dark", "appearance-theme-Dark"),
];

/// The material choices the section offers, in segment order: the
/// preference, the segment's name, and its test selector.
pub(crate) const MATERIALS: [(MaterialPreference, &str, &str); 2] = [
    (
        MaterialPreference::Glass,
        "Glass",
        "appearance-material-Glass",
    ),
    (
        MaterialPreference::Solid,
        "Solid",
        "appearance-material-Solid",
    ),
];

/// The section's state, held by the window as a field: each segment's
/// focus, a tab stop, so the keyboard reaches every choice.
pub(crate) struct State {
    focuses: HashMap<&'static str, FocusHandle>,
}

impl State {
    /// The section's state, over the window's `cx` (its focus handles).
    pub(crate) fn new(cx: &mut Context<SettingsWindow>) -> State {
        let selectors = THEMES
            .iter()
            .map(|&(_, _, selector)| selector)
            .chain(MATERIALS.iter().map(|&(_, _, selector)| selector));
        let focuses = selectors
            .map(|selector| (selector, cx.focus_handle().tab_stop(true)))
            .collect();
        State { focuses }
    }

    /// The focus of the segment `selector`.
    fn focus(&self, selector: &str) -> FocusHandle {
        self.focuses
            .get(selector)
            .expect("every segment has a focus")
            .clone()
    }
}

/// The settings the section offers the sidebar's search: each choice of
/// the theme and material rows, named as the section names it, in the row
/// it sits in. An override in force leaves the choices listed (the page
/// still shows them) but says why none can be used here.
pub(crate) fn entries(_launcher: &Launcher, cx: &App) -> Vec<search::Entry> {
    let overrides = crate::settings::shared(cx).read(cx).override_descriptions();
    let unavailable = (!overrides.is_empty()).then(|| override_text(&overrides));
    THEMES
        .iter()
        .map(|&(_, name, selector)| (name, selector, "Theme"))
        .chain(
            MATERIALS
                .iter()
                .map(|&(_, name, selector)| (name, selector, "Material")),
        )
        .map(|(name, selector, group)| search::Entry {
            control: Some(selector.into()),
            title: name.into(),
            group: Some(group.into()),
            unavailable: unavailable.clone(),
        })
        .collect()
}

/// What an override in force means here, as the section's notice and the
/// search's results say it: which environment variables set the choice,
/// and that a choice made here neither applies nor is saved.
pub(crate) fn override_text(overrides: &[String]) -> String {
    format!(
        "Set by {} for this session. Changes here won't apply or be saved.",
        overrides.join(" and "),
    )
}

/// What the section shows, as plain values: what [`section`] reads from
/// the host settings, and what the visual workbench's fixture supplies to
/// draw the same rows.
pub(crate) struct AppearanceView {
    /// The choices in effect.
    pub(crate) theme: ThemePreference,
    pub(crate) material: MaterialPreference,
    /// The environment variables overriding the choices, if any.
    pub(crate) overrides: Vec<String>,
    /// What a save reported, if it failed.
    pub(crate) status: Option<String>,
}

/// The Appearance section as the General page draws it: the theme and
/// material rows with their handlers, from the host settings as they
/// stand.
pub(crate) fn section(this: &mut SettingsWindow, cx: &mut Context<SettingsWindow>) -> Div {
    let settings = crate::settings::shared(cx);
    let view = {
        let state = settings.read(cx);
        AppearanceView {
            theme: state.theme_preference(),
            material: state.material_preference(),
            overrides: state.override_descriptions(),
            status: state.status(),
        }
    };
    let theme = crate::settings::visuals(cx).theme;
    // A choice is offered (focusable, clickable and saved) only where
    // nothing overrides it for this process.
    let offered = view.overrides.is_empty();
    let theme_segments = THEMES
        .iter()
        .map(|&(preference, name, selector)| {
            let anchor = this.search_anchor(selector);
            let focus = this.appearance.focus(selector);
            segment(
                Segment {
                    selector,
                    name,
                    chosen: preference == view.theme,
                    offered,
                },
                anchor,
                &focus,
                &theme,
                move |cx| {
                    crate::settings::shared(cx)
                        .update(cx, |settings, cx| settings.set_theme(preference, cx));
                },
            )
        })
        .collect();
    let material_segments = MATERIALS
        .iter()
        .map(|&(preference, name, selector)| {
            let anchor = this.search_anchor(selector);
            let focus = this.appearance.focus(selector);
            segment(
                Segment {
                    selector,
                    name,
                    chosen: preference == view.material,
                    offered,
                },
                anchor,
                &focus,
                &theme,
                move |cx| {
                    crate::settings::shared(cx)
                        .update(cx, |settings, cx| settings.set_material(preference, cx));
                },
            )
        })
        .collect();
    compose(&view, (theme_segments, material_segments), &theme)
}

/// The section's composition, which the visual workbench's fixture draws
/// too: its label over a card of two rows (Theme, then Material, each
/// with its segments at its end, the material's row saying when glass is
/// unavailable here), then the override notice and a save's failure, if
/// any. While an override is in force the rows' labels and choices are
/// drawn at the disabled opacity.
pub(crate) fn compose(
    view: &AppearanceView,
    segments: (Vec<Stateful<Div>>, Vec<Stateful<Div>>),
    theme: &Theme,
) -> Div {
    let offered = view.overrides.is_empty();
    let material_lines = material_note(view.material, theme)
        .map(|(text, color)| notice("appearance-material-note", text, color, theme))
        .into_iter()
        .map(IntoElement::into_any_element)
        .collect();
    let rows = [
        row("theme", "Theme", segments.0, Vec::new(), offered, theme),
        row(
            "material",
            "Material",
            segments.1,
            material_lines,
            offered,
            theme,
        ),
    ];
    let notes: Vec<AnyElement> = (!offered)
        .then(|| {
            notice(
                "appearance-override",
                override_text(&view.overrides),
                theme.warning,
                theme,
            )
        })
        .into_iter()
        .chain(
            view.status
                .as_ref()
                .map(|status| notice("appearance-status", status.clone(), theme.danger, theme)),
        )
        .map(|note| {
            note.px(theme.geometry.settings.section_label_inset)
                .into_any_element()
        })
        .collect();
    let body = div()
        .flex()
        .flex_col()
        .gap(theme.geometry.settings.section_label_gap)
        .child(controls::card(
            rows.into_iter().map(IntoElement::into_any_element),
            theme,
        ))
        .children(notes);
    controls::section(Some(LABEL.into()), body, theme).debug_selector(|| "appearance".into())
}

/// One segment's identity and state, as the section resolved it.
struct Segment {
    selector: &'static str,
    name: &'static str,
    /// Whether its choice is the one in effect.
    chosen: bool,
    /// Whether choosing it does anything: nothing is offered while an
    /// override is in force.
    offered: bool,
}

/// One segment of a row's choice ([`controls::segment`]) with its
/// identity and a radio button's semantics. Offered, it is a tab stop
/// that Enter, Space and a click choose — `choose` reports the choice to
/// the host settings, which repaint both windows and save — under Pane's
/// focus ring while the keyboard is on it; not offered, it is neither.
/// `anchor` is the scroll anchor the search's reveal scrolls to.
fn segment(
    segment: Segment,
    anchor: ScrollAnchor,
    focus: &FocusHandle,
    theme: &Theme,
    choose: impl Fn(&mut App) + Clone + 'static,
) -> Stateful<Div> {
    let Segment {
        selector,
        name,
        chosen,
        offered,
    } = segment;
    let drawn = controls::segment(name, chosen, offered, theme)
        .id(selector)
        .debug_selector(move || selector.into())
        .anchor_scroll(Some(anchor))
        .role(Role::RadioButton)
        .aria_label(name)
        .aria_toggled(if chosen {
            Toggled::True
        } else {
            Toggled::False
        });
    if !offered {
        return drawn.aria_disabled(true).cursor_default();
    }
    let ring = controls::segment_focus_shadows(chosen, theme);
    let on_click = choose.clone();
    drawn
        .key_context(CHOICE)
        .track_focus(focus)
        .focus_visible(move |style| style.shadow(ring))
        .on_action(move |_: &Choose, _, cx| choose(cx))
        .on_click(move |_: &gpui::ClickEvent, _, cx| on_click(cx))
}

/// One row: its label and `lines` at the left, its segmented choice
/// (`segments` on a row's track, a radio group named for the label) at
/// its end — the label and the track at the board's disabled opacity
/// while not `offered`, the lines, which say why or what holds, at full
/// strength. `id` names the row's debug selectors
/// (`appearance-<id>-field`, `appearance-<id>-track`).
fn row(
    id: &'static str,
    label: &'static str,
    segments: Vec<Stateful<Div>>,
    lines: Vec<AnyElement>,
    offered: bool,
    theme: &Theme,
) -> Div {
    let track = controls::row_segment_track(theme)
        .id(SharedString::from(format!("appearance-{id}-choices")))
        .debug_selector(move || format!("appearance-{id}-track"))
        .role(Role::RadioGroup)
        .aria_label(label)
        .children(segments);
    let opacity = if offered {
        1.
    } else {
        theme.geometry.controls.disabled_opacity
    };
    let label = controls::field_label(label, theme).opacity(opacity);
    controls::setting_row_with(label, lines, theme)
        .debug_selector(move || format!("appearance-{id}-field"))
        .child(track.opacity(opacity))
}

/// A line the section reports (the material's note, the override notice,
/// a save's failure) in `color`, announced as a status.
fn notice(
    id: &'static str,
    text: impl Into<SharedString>,
    color: Hsla,
    theme: &Theme,
) -> Stateful<Div> {
    let text = text.into();
    controls::field_description(text.clone(), color, theme)
        .id(id)
        .debug_selector(move || id.into())
        .role(Role::Status)
        .aria_label(text)
}

/// The material row's note, if it needs one, with its color: where glass
/// is chosen but the platform cannot provide it, why, in the warning's
/// color. Glass that stands and Solid need none.
pub(crate) fn material_note(
    preference: MaterialPreference,
    theme: &Theme,
) -> Option<(String, Hsla)> {
    if preference == MaterialPreference::Solid {
        return None;
    }
    crate::ui::material::glass_fallback_reason().map(|reason| {
        (
            format!("Glass isn't available here ({reason}), so Pane uses Solid."),
            theme.warning,
        )
    })
}
