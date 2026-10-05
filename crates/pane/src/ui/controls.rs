//! The Settings controls, as the reference's Settings board draws them
//! (#98): a field group — its label (`.flabel`), its control and its
//! description (`.fdesc`) — and the control families the board's
//! Appearance page shows: the segmented choice (`.segwrap`/`.seg`), the
//! accent swatches (`.sw`), the range input and the toggle.
//!
//! Presentation only, like [`crate::ui::settings_shell`]: each piece
//! returns a plain [`Div`], and identity, accessibility, focus, keys and
//! clicks stay with the caller. Nothing here imports launcher state.
//!
//! ## What production uses
//!
//! The Appearance page offers the settings Pane has (#100): the theme and
//! the material, each a field group around a segmented choice. The
//! swatches, the range input and the toggle are the board's advanced
//! Appearance controls — accent, blur, tint, pinned visibility, footer
//! tips — which #100 defers: only the visual workbench's reference fixture
//! draws them, so they are measured against the board without being
//! promised in the product. The toggle is the board's switch, a family
//! the Settings pages' own switches can share (#99 decides).
//!
//! ## Pointer and keyboard
//!
//! The board's controls answer the pointer at once: `.seg:hover` changes a
//! label's color and nothing fades. Every segment carries a hover style in
//! every state, the chosen one's being its own chosen look, rather than
//! attaching one only while unchosen: GPUI updates an element's remembered
//! hover state only while a hover style is attached, so a style that comes
//! and goes with the choice leaves the state stale (see `settings_shell`'s
//! module docs). The board draws no keyboard focus for these controls;
//! Pane's ring ([`segment_focus_shadows`]) is an adaptation, and so is a
//! disabled control's opacity outside the board's one case (Solid's
//! sliders, at 40%).

use gpui::prelude::*;
use gpui::{BoxShadow, Div, Hsla, Pixels, SharedString, div, px, relative};

use crate::ui::icon::{Glyph, glyph};
use crate::ui::theme::Theme;

/// A 1px ring inset along a box's edge (`box-shadow: inset 0 0 0 1px`):
/// it takes no layout space.
fn inset_ring(color: Hsla, width: Pixels) -> BoxShadow {
    BoxShadow::new(px(0.), px(0.), color)
        .spread_radius(width)
        .inset()
}

/// A page's column of field groups, 18px apart (the heading block above
/// them keeps its own 4px below it).
pub(crate) fn column(theme: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(theme.geometry.controls.group_gap)
}

/// One field group: its label, its control and its description, 8px
/// apart. The caller adds them in that order.
pub(crate) fn field(theme: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(theme.geometry.controls.field_gap)
}

/// A field's label (`.flabel`): 13.5/500 in the title ink, in the
/// reference's 18px line.
pub(crate) fn field_label(label: impl Into<SharedString>, theme: &Theme) -> Div {
    let line = theme.typography.settings.field_label;
    div()
        .text_size(line.size)
        .line_height(line.line_height)
        .font_weight(theme.typography.medium)
        .text_color(theme.text_title)
        .child(label.into())
}

/// A field's description (`.fdesc`): 12.5 at line height 1.45, in `color`
/// — the muted ink, or the warning's where it reports a fallback.
pub(crate) fn field_description(text: impl Into<SharedString>, color: Hsla, theme: &Theme) -> Div {
    let line = theme.typography.settings.field_description;
    div()
        .text_size(line.size)
        .line_height(line.line_height)
        .font_weight(theme.typography.regular)
        .text_color(color)
        .child(text.into())
}

/// A segmented choice's track (`.segwrap`): its segments side by side in
/// equal shares, 2px apart inside its 3px padding — 36 high around 30px
/// segments — on black 24% under a white 6% inset ring, radius 10.
pub(crate) fn segment_track(theme: &Theme) -> Div {
    let controls = &theme.geometry.controls;
    div()
        .w_full()
        .flex()
        .gap(controls.segment_gap)
        .p(controls.track_padding)
        .rounded(controls.track_radius)
        .bg(theme.controls.segment_track)
        .shadow(vec![inset_ring(theme.controls.segment_edge, px(1.))])
}

/// The chosen segment's inset shadows: its 1px top inset (white 8%).
fn chosen_shadows(theme: &Theme) -> Vec<BoxShadow> {
    let highlight = BoxShadow::new(px(0.), px(1.), theme.controls.segment_on_highlight).inset();
    vec![highlight]
}

/// A segment's shadows while the keyboard focuses it (`focus_visible`):
/// the chosen one's top inset, if `chosen`, under Pane's 2px focus ring
/// (the theme's focus color) — an adaptation: the board draws none.
pub(crate) fn segment_focus_shadows(chosen: bool, theme: &Theme) -> Vec<BoxShadow> {
    let mut shadows = if chosen {
        chosen_shadows(theme)
    } else {
        Vec::new()
    };
    let ring = inset_ring(theme.focus_ring, theme.geometry.controls.focus_width);
    shadows.push(ring);
    shadows
}

/// One segment (`.seg`): an equal share of its track, 30 high, radius 7,
/// its label centered in 12.5/500 — #9A9BA0 at rest and #EDEDEF under the
/// pointer, or, `chosen`, white on the white 12% wash under its white 8%
/// top inset, which the pointer leaves as it is. Nothing fades. A segment
/// that is not `enabled` keeps its look under the pointer (its hover style
/// is its resting one, still attached). The caller attaches the segment's
/// identity, focus, keys and click.
pub(crate) fn segment(
    label: impl Into<SharedString>,
    chosen: bool,
    enabled: bool,
    theme: &Theme,
) -> Div {
    let controls = &theme.geometry.controls;
    let colors = &theme.controls;
    let line = theme.typography.settings.segment;
    let (text, hover_text) = match (chosen, enabled) {
        (true, _) => (colors.segment_on_text, colors.segment_on_text),
        (false, true) => (colors.segment_text, colors.segment_hover_text),
        (false, false) => (colors.segment_text, colors.segment_text),
    };
    div()
        .flex_1()
        .min_w(px(0.))
        .h(controls.segment_height)
        .flex()
        .items_center()
        .justify_center()
        .rounded(controls.segment_radius)
        .cursor_pointer()
        .text_size(line.size)
        .line_height(line.line_height)
        .font_weight(theme.typography.medium)
        .text_color(text)
        .when(chosen, |segment| {
            segment.bg(colors.segment_on).shadow(chosen_shadows(theme))
        })
        .hover(move |segment| segment.text_color(hover_text))
        .child(div().min_w(px(0.)).truncate().child(label.into()))
}

/// The accent swatches' row (`role=group`): 10px apart.
pub(crate) fn swatch_row(theme: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(theme.geometry.controls.swatch_gap)
}

/// One accent swatch (`.sw`): a 30px disc of `fill` under a black 20%
/// inset edge or, `chosen`, ringed by a 2px gap of the page's color and a
/// 2px ring of its own color outside that.
pub(crate) fn swatch(fill: Hsla, chosen: bool, theme: &Theme) -> Div {
    let controls = &theme.geometry.controls;
    let size = controls.swatch_size;
    let disc = div().flex_none().size(size).rounded(size / 2.).bg(fill);
    if chosen {
        // `0 0 0 2px gap, 0 0 0 4px fill`: CSS paints its first shadow on
        // top and GPUI its last, so the list is the reference's reversed.
        disc.shadow(vec![
            BoxShadow::new(px(0.), px(0.), fill)
                .spread_radius(controls.swatch_ring_gap + controls.swatch_ring),
            BoxShadow::new(px(0.), px(0.), theme.controls.swatch_gap)
                .spread_radius(controls.swatch_ring_gap),
        ])
    } else {
        disc.shadow(vec![inset_ring(theme.controls.swatch_edge, px(1.))])
    }
}

/// The custom-color swatch: a 30px disc under a white 22% ring, holding
/// a 14px plus in the body ink.
pub(crate) fn swatch_add(theme: &Theme) -> Div {
    let controls = &theme.geometry.controls;
    let size = controls.swatch_size;
    div()
        .flex_none()
        .size(size)
        .rounded(size / 2.)
        .flex()
        .items_center()
        .justify_center()
        .shadow(vec![inset_ring(theme.controls.swatch_add_edge, px(1.))])
        .child(glyph(Glyph::Plus, controls.swatch_glyph, theme.text_body))
}

/// A slider's header line: its label (a field label) at the left and its
/// value (Geist Mono 12.5 in the body ink) at the right, their lines'
/// bottoms aligned, as the board's baseline puts them.
pub(crate) fn slider_header(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    theme: &Theme,
) -> Div {
    let line = theme.typography.settings.value;
    div()
        .flex()
        .items_end()
        .justify_between()
        .child(field_label(label, theme))
        .child(
            div()
                .flex_none()
                .font_family(theme.typography.mono_family.clone())
                .text_size(line.size)
                .line_height(line.line_height)
                .text_color(theme.text_body)
                .child(value.into()),
        )
}

/// A slider: Pane's drawing of the board's range input, whose look the
/// reference leaves to the browser — 20 high, a 4px track (the accent up
/// to `fraction` of its width, white 16% beyond) and a 16px accent thumb
/// on the value.
pub(crate) fn slider(fraction: f32, theme: &Theme) -> Div {
    let controls = &theme.geometry.controls;
    let fraction = fraction.clamp(0., 1.);
    let height = controls.slider_height;
    let track = controls.slider_track;
    let thumb = controls.slider_thumb;
    div()
        .relative()
        .w_full()
        .h(height)
        .child(
            div()
                .absolute()
                .left(px(0.))
                .right(px(0.))
                .top((height - track) / 2.)
                .h(track)
                .rounded(track / 2.)
                .bg(theme.controls.slider_track)
                .child(
                    div()
                        .h_full()
                        .w(relative(fraction))
                        .rounded(track / 2.)
                        .bg(theme.accent),
                ),
        )
        .child(
            // The thumb's travel: the track's width less the thumb's.
            div()
                .absolute()
                .top(px(0.))
                .bottom(px(0.))
                .left(px(0.))
                .right(thumb)
                .child(
                    div()
                        .absolute()
                        .top((height - thumb) / 2.)
                        .left(relative(fraction))
                        .size(thumb)
                        .rounded(thumb / 2.)
                        .bg(theme.accent),
                ),
        )
}

/// The toggles' list: rows of [`toggle_row`].
pub(crate) fn toggle_list() -> Div {
    div().flex().flex_col()
}

/// A toggle's row: its label (a field label at 400) at the left and
/// `toggle` at the right, 44 high with a 1px rule (white 6%) along its
/// top.
pub(crate) fn toggle_row(
    label: impl Into<SharedString>,
    toggle: impl IntoElement,
    theme: &Theme,
) -> Div {
    let label = field_label(label, theme).font_weight(theme.typography.regular);
    div()
        .flex()
        .items_center()
        .justify_between()
        .h(theme.geometry.controls.toggle_row_height)
        .border_t_1()
        .border_color(theme.hairline_soft)
        .child(label)
        .child(toggle)
}

/// A toggle (the board's switch): 40×24, radius 12, the accent while `on`
/// and white 16% while off, its 18px white knob 3px in from its left
/// (off) or its right (on) under a short shadow. The board slides the
/// knob over .2s; that motion is the caller's to add, and reduced
/// motion's to drop.
pub(crate) fn toggle(on: bool, theme: &Theme) -> Div {
    let controls = &theme.geometry.controls;
    let knob_shadow =
        BoxShadow::new(px(0.), px(1.), theme.controls.toggle_knob_shadow).blur_radius(px(3.));
    let knob_left = if on {
        controls.toggle_width - controls.toggle_inset - controls.toggle_knob
    } else {
        controls.toggle_inset
    };
    div()
        .relative()
        .flex_none()
        .w(controls.toggle_width)
        .h(controls.toggle_height)
        .rounded(controls.toggle_height / 2.)
        .bg(if on {
            theme.accent
        } else {
            theme.controls.toggle_off
        })
        .child(
            div()
                .absolute()
                .top(controls.toggle_inset)
                .left(knob_left)
                .size(controls.toggle_knob)
                .rounded(controls.toggle_knob / 2.)
                .bg(theme.controls.toggle_knob)
                .shadow(vec![knob_shadow]),
        )
}
