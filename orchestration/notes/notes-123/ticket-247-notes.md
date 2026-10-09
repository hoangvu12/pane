# Ticket #247 notes — named built-in glyphs on Pane's neutral tile + icon-rule guard tests

Branch: `pi-subagent/123-247-glyph-tile` (worktree spec-123-t247). Base: 26c8e224 = origin/main.
NO local cargo/rustc/rustfmt. CI = quick tier on push (fmt --check + check --all-targets, Linux+Windows).

## What the ticket asks

- A built-in glyph an extension names (icon source `IconSource::Builtin{..}`) draws on Pane's
  NEUTRAL COMMAND TILE at the row's (28), a slot's (30) and the Actions header's (18) sizes.
  Image icons stay bare (with their mask); a package without an icon keeps the first-letter tile.
- Guard tests in crates/pane/tests/{icons,default_icons,application_icons,web_icons,compact_pins}.rs:
  app row with icon → no tile; without → same-size placeholder; command row keeps tile;
  pins, compact pins, Actions header follow the same rules.

## Model found

- Core icon model: `pane_core::Icon { source: IconSource, tint, mask, fallback, tooltip }` in
  crates/pane-core/src/icons.rs. `IconSource::Builtin{name, filled}` is a named reicon glyph.
  `Launcher::icon_of(id)` (launcher/looks.rs:274,378) resolves a command's own icon → else its
  package's → else the package's first-letter `IconSource::Letter`.
- Window resolution: `features/icons.rs::drawn(icon, theme) -> DrawnIcon`; `row_icon_of(launcher,
  id, theme) -> RowIcon` (Drawn when the launcher has an icon, else Pane tile). Rows:
  `app.rs:1388` builds `RowIcon::Drawn(drawn(shown.icon))` from the core presentation.
- Drawing: `ui/extension_icon.rs` — `RowIcon::Pane(tone,glyph) => tile_at(..)` (Pane's tile);
  `RowIcon::Drawn(d) => draw(d, IconSize::of(size,theme), ..)` bare. All tile sizes (Row/Slot/Mini)
  go through `row_icon_at`: result rows, pinned slots (scope `slot-N`), compact pins (scope
  `compact-pin-N`), Actions header (scope `actions-header`), footer command lead, Settings pages,
  forms. Small/bare sizes go through `draw` + `IconSize::small`: accessories, Actions *entries*
  (scope `action-<label>`), clipboard/search file rows+previews (File sources).
- `tile_at`'s `IconTone::Command` branch = the neutral command tile: `theme.tile_background`
  (white 8% dark / black 7% light), `theme.tile_border`+`theme.tile_highlight` inset shadows,
  glyph in `theme.tile_foreground`. Metrics from `TileSize::metrics(theme)`: Row 28/r7/g16,
  Slot 30/r8/g17, Mini 18/r5/g11.
- Web/system/application icons while loading: `drawn()` LIFTS the nested fallback to the top level
  (Url-none branch and File/Application branch). A fallback glyph must stay BARE (it stands in
  where a bare image will draw) — e.g. tests assert `icon-Slow web image-glyph-clock` then the
  image replaces it in the same box.

## Design (final)

Scope decision (after finding stand-ins are indistinguishable at the window): the tile is for
**a command's or package's own icon naming a built-in glyph** — the manifest icon model — which is
what the ticket's "a command naming one of Pane's glyphs reads as a command" names. Stand-ins
(an application's placeholder, web/system icon loading fallbacks) and command-list item icons
stay bare, because the CORE lifts fallbacks/stand-ins to top-level Builtin icons the window
cannot tell from named ones; tiling those would break the placeholder-without-a-tile and
fallback-shown-alike rules the same spec states.

- `DrawnIcon.tile_color: Option<Hsla>`: Some only when the icon's own source is `Builtin`.
  Value = `tint.unwrap_or(theme.tile_foreground)` (tint already contrast-corrected vs the panel
  at 3:1). Nested fallbacks get it cleared in `drawn()` (a stand-in draws where its icon does).
- `row_icon_at` (the only tile-size entry point) draws the neutral tile: a wrapper div with
  chrome `ui::icon::neutral_chrome` (factored out of `tile_at`'s Command branch), the
  `icon-<scope>-tile` debug selector, and the bare `draw` output (color overridden to the tile
  colour) inside. File rows draw bare at row size through direct `draw` calls — that is why the
  chrome lives in `row_icon_at` and not in `draw`/`IconSize`.
- `app.rs` root rows keep the flag only for a command's row (`RowKind::Command`/`Fallback`):
  item rows (kind None), application/file/folder/link rows draw bare whatever the icon is.
- pane-core `application_icons::of_row` (the `Launcher::icon_of` path for slots/headers/pins):
  the not-yet-there case returns `IconSource::Application(reference)` with the placeholder as
  its fallback (the shape the window's `drawn()` Application branch was built to lift, bare),
  instead of the bare placeholder Builtin. `shown()` (root rows) is unchanged, so pane-core's
  own tests (which read the presentation) stay green.
- Accessories, Actions entries, previews: unchanged (small sizes, direct `draw`).

## Status

- [x] Read ticket/spec/ADRs/AGENTS/ci
- [x] Explore icon model + surfaces
- [x] Implement (ui/icon.rs, ui/extension_icon.rs, features/icons.rs, app.rs,
      pane-core application_icons.rs)
- [x] Tests (support/packages.rs glyph_package; icons.rs fixture+guards+3-sizes test;
      compact_pins.rs new test; default_icons/application_icons/web_icons guards)
- [x] Commits: 2d5d6b6d (impl), 3da8e0de (tests), 631d42e5 (rustfmt fixes — first run
      failed `cargo fmt --check`; rustfmt splits assert!/assert_eq! calls whose
      width passes a threshold; applied its exact output)
- [x] CI green on the quick tier: run 37951633511 — Check (ubuntu-24.04) and
      Check (windows-2025) both pass: `cargo fmt --all --check` +
      `cargo check --locked --workspace --all-targets` (compiles pane, pane-core and
      all test binaries). Run 37949145885 was the fmt-only failure.

## Report-back skeleton

- AC1 (named glyph on the neutral tile at row/slot/header sizes): ui/extension_icon.rs
  `row_icon_at` + `ui/icon.rs::neutral_chrome` + features/icons.rs `drawn()` tile_color;
  tests: icons.rs `a_named_glyph_draws_on_panes_neutral_tile_at_every_tile_size` (row 28,
  header 18, slot 30), `root_search_draws_…` (row), compact_pins.rs
  `a_compact_pin_of_a_named_glyph_…` (compact pin, row size).
- AC2 (image icons bare with mask; first-letter tile): unchanged paths; guards in
  icons.rs root-search + dark-theme tests, default_icons.rs.
- AC3 (guards: app row no tile/placeholder same-size, command row keeps tile, pins/compact
  pins/header follow): application_icons.rs (placeholder + image + slot + header no-tile,
  same-size bounds unchanged), icons.rs (Pane rows keep tiles + star row tiles),
  web_icons.rs (fallback + image bare, action icon bare).
- Deviations to note: the tile is scoped to the command's/package's OWN (manifest) icon —
  command-list item icons and stand-ins stay bare (see notes above); the core's
  application_icons::of_row now wraps the not-yet-there icon (placeholder as the fallback
  of the Application reference) so the placeholder stays bare through slots/headers/pins.
  The launcher's extension-management FLOW rows (kind None) draw bare like command-list
  rows; the Settings Extensions PAGE tiles (row_icon_of path) like root search.
