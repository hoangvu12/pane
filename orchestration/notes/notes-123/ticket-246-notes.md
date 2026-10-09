# Ticket #246 — text levels by alpha of the text colour, with contrast floors

Branch: `pi-subagent/123-246-text-alpha` (worktree spec-123-t246), base 26c8e224 = origin/main.

## Ticket (contract)

Launcher text levels = primary text colour at strengths: primary (titles, the query) 100%,
secondary (subtitles, kind labels, footer labels, keycap labels) 60%, tertiary (section
labels, the placeholder, disabled text) 40%, faint marks 20%, separators 10% — wherever
legible. Per theme/material/background-image case, a level whose alpha form would fall
below its floor KEEPS its present opaque value: 4.5:1 primary+secondary, 3:1 tertiary +
placeholders. Check against each surface's own tint; glass over unknown desktop judged on
its tint alone (like the existing glass-tint floor, which is the tint's ≥55% alpha rule).
Settings window's text roles unchanged.

ACs:
1. Secondary, tertiary, faint and separator text follow the strengths wherever they pass.
2. Theme tests: each text role's chosen form meets its floor for every theme, material,
   background-image case.
3. A role that would fail its floor keeps its accepted opaque colour.
4. Settings window's text roles unchanged.

## Code map (verified)

- `crates/pane/src/ui/theme.rs` — every token. dark()/light() base palettes;
  `over_backdrop(canvas)` = the background-image case (called from
  `settings.rs::launcher_visuals` only; Settings reads `visuals()`, never the backdrop
  theme). Glass tint floor: doc comment on `panel_tint` ("reference keeps the tint at or
  above 55% alpha") — judgement-on-tint = the tint's own colour, no desktop assumption.
- `crates/pane/src/ui/contrast.rs` — WCAG contrast: `TEXT=4.5`, `GRAPHIC=3.0`,
  `luminance`, `ratio`, `corrected`. Tests live in `mod tests` in-module (plain #[test]).
  Theme tests today: none dedicated; settings.rs's `mod tests` mirrors palette values.
- Surfaces the launcher's level text sits on: panel (solid / glass tint), footer strip
  (footer_tint over panel), popover (popover_tint/solid — the Actions panel, not
  overridden by over_backdrop), over a backdrop: canvas for panel/footer/frost pill.
  Canvas is clamped by background.rs to lightness 6–16% (dark) / 86–96% (light).
- Token sharing with the Settings window (CRITICAL for AC4):
  - `text_placeholder` shared (settings_shell search, controls, select) → value UNCHANGED;
    new launcher-only `query_placeholder` for the launcher's search fields.
  - `hairline_soft` shared (settings_shell titlebar/sidebar rules) → value UNCHANGED; new
    launcher-only `separator` token; rewire the launcher's rules to it.
  - `keycap_text`, `footer_button_text`, `footer_divider`, `action_rule` launcher-only
    (Settings draws bindings as text via controls::binding_text, not caps).
  - `text_body`, `text_muted` shared → values UNCHANGED; launcher consumers that ARE
    levels move to new tokens.

## Design decisions

- Primary = `text_title` (the palette's Ink). Titles AND the query keep their present
  values (primary is 100%, AC1 lists only secondary/tertiary/faint/separator as following
  strengths; the reference's query colour is its own accepted value). Floors still tested.
- New fields: `text_secondary` (row subtitles + kind labels), `text_tertiary` (section
  labels [shell::section_label — root search, clipboard day groups, Files] + Actions
  group labels + the footer's hint), `query_placeholder` (launcher search fields: root
  search, Actions panel, split_view's command searches), `text_faint` (20%, no consumer
  yet — AC1 requires the level; documented), `separator` (10%: rule under the search
  field, footer top rule, footer button rule [was footer_divider], Actions rules [was
  action_rule], split view's rules).
- `footer_button_text`, `keycap_text` become the secondary level in place (launcher-only).
- Disabled text: the two unavailable sites (app.rs action_button, actions_panel action_row)
  dim at the tertiary strength 0.4 (was 0.5 element opacity) — matches the controls'
  disabled_opacity 0.4; the whole element dims (glyph+keys), not just text.
- Floors: secondary uses contrast::TEXT (4.5); tertiary/placeholder TERTIARY_FLOOR=3.0
  (theme.rs). Faint/separators: no floor, always at strength.
- Decision helper `level(primary, present, strength, floor, surfaces)` in theme.rs; each
  role checked against ITS surfaces (flattened tints): subtitle [panel], footer label
  [footer], keycap [cap fill over panel/footer/popover], tertiary [panel, footer,
  popover], placeholder [panel-or-pill, popover]. Glass: tint flattened over own base ≡
  panel_solid (the "tint alone" judgement). Backdrop: canvas + popover.
- over_backdrop re-resolves levels against [canvas, popover]; fallbacks = the backdrop's
  own accepted colours (stepped muted 0xA9AAAF/0x4A4D55, the old frost.label
  0xC9CACE/0x3B3D44 for section labels). frost.label field REMOVED (only shell.rs read
  it). hairline_soft's backdrop override removed (no launcher consumer left).
- Expected outcomes: dark takes alpha forms everywhere (secondary 6.4–7.4:1, tertiary
  3.6–3.8:1); light keeps every present value (alpha forms 2.4–4.3:1, below floors);
  backdrop follows the canvas. Settings: unchanged values (test asserts).

## Work log

- [x] Read ticket, spec #123, ADR 0035/0029, research deep dive, AGENTS, ci.md, CONTEXT.
- [x] Explored theme.rs, contrast.rs, material.rs, consumers, token usage audit.
- [ ] Implement contrast::over + test.
- [ ] Implement theme levels + tests.
- [ ] Rewire consumers.
- [ ] Commit/push/poll CI.

## Notes for the report

- faint level has no Pane consumer today (token + tests only) — flagged.
- Interpretations: "footer labels" = the footer buttons' labels (footer_button_text);
  the footer's hint is Ink 3 → tertiary. "separators" = the launcher's divider rules.
  Settings' own section labels/hairlines/controls placeholders stay on the shared tokens.

## Implementation state (2026-… before push)

Commits (branch pi-subagent/123-246-text-alpha):
- 4cefa8a7 contrast::over + test
- 24c30e93 theme levels + consumers + tests

Files: ui/theme.rs (level system, tokens, tests), ui/contrast.rs (over),
ui/result_row.rs (subtitle+kind → text_secondary), ui/shell.rs (section_label
→ text_tertiary), ui/footer.rs (hint → text_tertiary, divider → separator),
features/actions_panel.rs (group label → text_tertiary, search placeholder →
query_placeholder, rules → separator, opacity 0.4), app.rs (action button
opacity 0.4), features/root_search/mod.rs (placeholder + rule), ui/split_view.rs
(placeholder + 4 rules), ui/material.rs (footer rule).

Formatting hand-checked against rustfmt defaults (max_width 100, fn_call_width
60, chain_width 60): one-line fn signatures that fit, tall calls >60,
chain breaks >60, struct literals with >18-char bodies multi-line, macro args
one per line when vertical. Line lengths verified ≤100 (only pre-existing
URL line 9 exceeds).

Floors verified numerically by hand (see analysis): dark passes everywhere
(secondary ≥5.4, tertiary ≥3.6); light keeps every present (alpha forms
2.3–4.3 < floors); backdrop dark passes (secondary ≥5.4 over cap-on-canvas,
tertiary ≥3.6), light keeps. Primaries ≥11.9 everywhere.

## CI

- Run 37945814397 (24c30e93): FAILED — rustfmt only (8 hunks; rustfmt's actual
  rules learned: struct-field values stay one line up to max_width 100;
  array/vec/macro-arg literals over ~60 go vertical; assert_eq! vertical form
  packs short args; calls >60 in expression position go tall). No compile
  errors — the fmt step ran before cargo check.
- Run 37947642030 (1e02e7f8): GREEN — Check (ubuntu + windows) +
  CI passed. `cargo check --workspace --all-targets` compiles the new tests.

Final commits: 4cefa8a7, 24c30e93, 1e02e7f8.

## AC mapping (for the report)

- AC1 (levels follow strengths where floors pass): theme.rs `level()` +
  tokens (text_secondary/tertiary, footer_button_text, keycap_text,
  query_placeholder, text_faint, separator; TERTIARY_STRENGTH for disabled);
  test `the_levels_take_their_strengths_wherever_they_read`.
- AC2 (theme tests, every case): theme.rs `mod tests` — `cases()` = both
  palettes × glass/opaque × none/canvas-edges; test
  `every_text_role_meets_its_floor_in_every_case` (primaries + 5 roles ×
  their surfaces).
- AC3 (failing role keeps accepted colour): the same strength test's
  else-branch asserts the accepted constants (light + light-backdrop
  exercise it).
- AC4 (Settings unchanged): shared tokens keep values; test
  `the_settings_windows_text_roles_keep_their_values`.
