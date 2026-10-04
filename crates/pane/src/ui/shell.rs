//! The launcher shell's layout: the reference root's panel size and the
//! result list that fills the space between the search header and the
//! footer.
//!
//! The reference fixes the panel at 760×518 — a 64px search header, a
//! 404px list and a 50px footer — and paints the panel's inner edge as an
//! inset ring, so none of the three loses a pixel to a border (see
//! [`super::material::Material::panel`]). The list here is the one the
//! launcher's search and list screens and the visual workbench's root
//! fixture (#91) both lay their rows out in, so a change to its paddings
//! reaches both.

use gpui::prelude::*;
use gpui::{Div, Role, Stateful, div};

use crate::ui::theme::Theme;

/// The launcher window's client size, in logical pixels: the reference
/// root panel's 760×518 (64 search header + 404 list + 50 footer). The
/// window opens at this size; the user may resize it.
pub(crate) const LAUNCHER_CLIENT: (f32, f32) = (760., 518.);

/// The result list: the rows' column, inset by the reference root body's
/// paddings (4 above, 10 at the sides and below) with its 2px gap between
/// rows, filling the height the header and footer leave and scrolling
/// past it. The caller names it for assistive technology, tracks its
/// scroll and adds the rows.
pub(crate) fn result_list(theme: &Theme) -> Stateful<Div> {
    let geometry = &theme.geometry;
    div()
        .id("rows")
        .debug_selector(|| "rows".into())
        .role(Role::ListBox)
        .flex_1()
        .flex()
        .flex_col()
        .gap(geometry.row_list_gap)
        .px(geometry.list_padding_x)
        .pt(geometry.list_padding_top)
        .pb(geometry.list_padding_bottom)
        .overflow_y_scroll()
}
