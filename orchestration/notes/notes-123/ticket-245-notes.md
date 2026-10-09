# Ticket #245 — selection wash without an edge, fainter hover wash

Branch: `pi-subagent/123-245-selection-wash` (worktree spec-123-t245, base 26c8e224 = origin/main)
Spec #123 (ADR 0035). No Rust toolchain locally — CI only. rustfmt by hand (≤100 cols).

## Design decided

**Tokens (theme.rs):** collapse the separate fixed washes into ONE set, derived from the
text colour:
- `selection_wash` = text colour @ 10% (dark `0xEDEDEF1A`, light `0x2021261A`)
- `hover_wash` = text colour @ 5% (dark `0xEDEDEF0D`, light `0x2021260D`)

Deleted tokens: `row_selected_border` (the edge), `action_selected` (11%),
`slot_hover` (7%), `control_hover` (6%), `row_selected` (8.5%), `row_hover` (3.5%).
Every user migrates to the two new tokens, including borrowed uses:
- text-selection highlight (`selection_color`) → `selection_wash` (fields: root search,
  command search, the Actions panel's search, split view, Settings fields/sidebar search)
- Settings titlebar caption-button hover, ghost button, select's `menu_row` → the new
  washes (values shift ≤1.5% alpha — imperceptible; the Settings window's OWN tokens
  nav_*/segment_*/toggle/card/field are untouched)
- over_backdrop: the row_hover/row_selected/row_selected_border/slot_hover overrides are
  dropped (the washes inherit from text_title; frost tokens stay)

**Where the wash applies:**
- Selected rows (no edge, no ring, keep frost over backdrop): `result_row::row_surface`
  (ALL launcher list rows: root search, command list, command search, confirmation rows,
  Manage-extensions flow rows, details screens), `split_view::clip_row/file_row`,
  `extension_log` rows, `actions_panel::action_row`, `footer_menu` items,
  `controls::menu_row`, answer card (fill becomes selection_wash while selected, accent
  ring deleted).
- Hover wash (only where hovering does NOT move the selection): command-list rows,
  root-search rows under an open overlay (`pointer_selection_held`), split-view rows,
  log rows, pinned slots, compact pins, footer-family buttons (mark, primary action,
  toast buttons, confirmation buttons, split footer, log footer), Pane-menu items.
  The Actions panel's entry hover branch is REMOVED (hovering there selects, so only
  the selection wash shows — spec's rule).
- `pressed()` still doubles the wash's alpha; press of an unselected row =
  pressed(hover_wash).

**The 70 ms hover fade (new motion):** `ui::motion::HOVER_FADE` = 70 ms.
State lives in the window: `app/hover_wash.rs` — `HoverWashes { hovered: Option<Spot>,
exits: Vec<(Spot, Tween)> }` with `set(spot, over, cx)` (called from each surface's
`.on_hover` listener), `look(spot, now) -> f32` (1 under the pointer, tween value while
fading, 0 otherwise), `advance(reduced, now)` (called once per frame from
`FrameMotion::frame`; returns whether still animating → requests frames).
Hover ARRIVES at once; only the exit fades. Re-hover cancels the fade (wash back to
full). Reduced motion: exits cleared — the wash leaves at once.
`Spot` enum: Row(usize), Slot(usize) (strip + vertical row), Pin(usize) (compact),
MenuItem(usize), Clip(usize) (split view rows), Log(usize), Button(&'static str)
(footer-family buttons).

**Focus rings kept:** nothing removed — search-field focus, `focus_visible` wash on the
vertical slot row (quick_slots.rs:585 → hover_wash), mark button focus shadow, Settings
controls' focus rings.

## Key code facts learned
- `pointer_selection_held()` = frozen || menu open || actions open — root rows under an
  overlay still receive hover (the dimmer registers no hitbox) but don't select.
- Only `Screen::Root` rows select under the pointer (`render_row`'s `.when(root,
  on_mouse_move)`); every other screen's rows keep click-runs semantics → hover wash.
- Hover observation: gpui-ce `Stateful<Div>::on_hover(|&bool|)` (used by toast.rs).
- `settle_frames`/`frame` in tests/support/wait.rs advance the controlled clock +
  deliver requested animation frames; `App::set_reduce_motion` in tests
  (`cx.update(|_, cx| cx.set_reduce_motion(true))`).
- Shadows (the edge, rings, focus rings) are Shadow primitives — NOT visible to
  `window.painted_quads()`; `paints_fill_at` sees only quads. The edge/ring removal is
  therefore covered by the fill-change assertions (wash replaces old washes) + code;
  the backdrop blur likewise. No rasterizer in the test platform (render_to_image needs
  a headless renderer pane never configures).
- Answer card: `result_layouts.rs::answer_card` — `.when(selected, shadow(ring(accent)))`
  goes; selected bg becomes `selection_wash`.
- Background image in tests: write a JPEG into `data/backgrounds/<name>` +
  `settings.json { "version": 1, "background": "<name>" }` → `request_backdrop` bakes
  it (async; wait until observable). `image` crate is a pane dependency (usable in
  integration tests).
- Manage-extensions flow in the window: `settle::enter_flow` → `Screen::Extensions`;
  rows need an installed package (support/packages.rs `assembled_package`), else empty.
- The Pane menu's items don't select under the pointer (no on_mouse_move) → hover wash.
- Tests are ONE binary: declare `mod <name>;` in crates/pane/tests/main.rs.
- Tests that assert old hexes: launcher_settings.rs:2156/2176/2179 (menu_row 11%) —
  update; other negative assertions (0xFFFFFF09/0xFFFFFF16 "not a root wash") stay true.

## Wash hexes for tests (dark default theme)
- selection_wash 0xEDEDEF1A; hover_wash 0xEDEDEF0D; old root washes 0xFFFFFF16/09;
  old Actions/menu wash 0xFFFFFF1C; card_fill 0xFFFFFF0F.

## TODO / risks
- [ ] implement list above
- [ ] tests in tests/selection.rs (new) + main.rs declaration
- [ ] push, watch CI quick tier, iterate
