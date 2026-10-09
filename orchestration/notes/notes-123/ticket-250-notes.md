# Ticket #250 notes — Shape the HUD: icon and message, exact placement, fade and announcements

Branch: `pi-subagent/123-250-hud-shape` (worktree spec-123-t250). Base: 26c8e224 = origin/main.
NO local cargo/rustc/rustfmt. CI = quick tier on push (fmt --check + check --all-targets, Linux+Windows).
Tests run at the verify tier (parent's job); my tests must compile and be right by reading.

## What the ticket asks

- Placement: centred horizontally on the monitor the launcher LAST showed on, bottom edge exactly
  150 LOGICAL px above that monitor's bottom. Today: 0.12 share of display height.
- Size: content-sized, 46 logical tall (56 with a second line), at most 500 wide. Today: 44, (160, 560).
- Content: optional icon + one-line title + optional one-line message, popover material and type.
  Core model gains `message` (+ `icon`): "add what the model needs, keeping the core's seam as small".
- Timing: 1.2 s default/success, 3 s failure, then fade out ~1 s; PENDING (Animated) HUD stays until
  updated or launcher active again; one at a time, newer replaces. Reduced motion: no fade.
- Never takes focus, click-through, on the launcher's monitor. Toast→HUD route while hidden/compact works.
- Wayland limitation stays documented (code comment + docs).
- Announced via the platform's mechanism for an unfocused window (UIA live region on Windows — how the
  window layer already announces) and via the harness's announcement capture.
- smoke-windows-hud.ps1: extend for exact placement (150 logical above bottom) and click-through
  (already checks WS_EX_TRANSPARENT) — note what it now checks.

## Key code facts found

- `crates/pane/src/features/hud.rs`: whole module. `placed(display, size)` uses `window.display(cx)`
  bounds (gpui logical px on Windows! WindowsDisplay::bounds is logical; WindowParams.bounds also
  logical for the target display — see gpui_windows/window.rs `retrieve_window_placement` comment).
  So all math is in logical px; no scale juggling.
- Core `Hud { title, style }` (feedback.rs:171) + `duration()`. ToastStyle::hides_by_itself() exists
  (Animated → false). HUD_DURATION/FAILURE_HUD_DURATION consts.
- Toast→HUD route: `present()` in pane-core/src/launcher/feedback.rs builds Hud from the toast
  (today `title: toast.text()`); with message field → title + message separately.
- Hud constructed at: own_actions.rs (×3), clipboard_view.rs:888, launcher/feedback.rs:688,
  runtime/host_functions.rs:174, app.rs show_smoke_hud, tests (pane-core feedback.rs, pane tests
  feedback.rs request_hud).
- Window harness: `window.hud()` (title, test support) + `request_hud()` test seam in app.rs.
- `cx.open_window` draws once synchronously → debug_bounds/a11y reachable. VisualTestContext::
  from_window for the HUD window. App::active_window() for focus test. set_a11y_forced → refresh →
  set_dirty wakes the platform → frame drawn on run_until_parked (same as launcher a11y tests).
- element opacity multiplies painted quad background alpha (window.rs paint_quad); tests can read
  `window.painted_quads()` → `quad.background.as_solid().a`. Default test theme = DARK
  (ThemePreference::default = Dark). popover_solid alpha 1.0.
- AccessKit windows adapter raises UIA_LiveRegionChanged for an ADDED node with a name and live!=Off
  → a HUD window whose live region is created WITH text announces at once (unfocused window fine).
- Test platform: ONE display 1920×1080 at (0,0), scale 1, ignores focus on open (never activates).
- Compact window: settings.json `{"version":1,"windowMode":"compact"}` + init_with_overrides;
  `collapses()` = home shown (root, empty query) + Status::Idle + no panel/menu/confirmation.
  Escape at root clears the query first (BackOrHide default). Install outcome leaves Status::Result
  until the next action — run the command once (outcome → toast, status Idle) before collapsing.
- no_view.rs already covers toast→HUD while HIDDEN (command's hotkey, window hidden).
- fade timing on the test clock: timers via cx.background_executor().timer → advance_clock; the
  notify-driven redraw works (icons.rs 2h→3h test). Mid-fade frames need simulate_next_frame (tests
  have no platform frame loop) — window.simulate_next_frame(cx) exists (test-support).

## Design

- pane-core feedback.rs: `Hud { title, message: Option<String>, icon: Option<Icon>, style }`,
  `Hud::new(style, title)`, `hides_by_itself()` (= style.hides_by_itself()); duration unchanged.
  `present()` passes toast title + message. Update core tests.
- pane/features/hud.rs: ABOVE_BOTTOM=150, HEIGHT=(46,56), WIDTH=(160,500), FADE=1s, size_for(hud)
  (widest line + chrome + icon), placed() bottom-150. HudView: fading: Option<Instant>, render
  opacity + request_animation_frame while fading; icon via icons::drawn + extension_icon::draw
  (IconSize::small(18)); title + message column; announcer node (Label, polite live, name=value=
  "Title"/"Title: message"). HudWindow { window, shown, title, pending }.
  show_hud: pending → no timer; else timer(duration) → reduced? close : fade + timer(FADE) → close.
- app.rs: close_pending_hud on unhide() + observe_window_activation(active). show_smoke_hud → Hud::new.
  Test support: hud_window() -> Option<AnyWindowHandle>.
- Tests: feedback.rs — timing (1.2+fade, 3+fade), reduced motion closes at duration, replacement,
  focus never taken (active_window + is_window_active), placement (bounds vs display: bottom 150,
  centred, 46/56 tall, ≤500 wide, 500 cap), mid-fade quad alpha, announcement via support a11y.
  no_view.rs — compact route (settings windowMode compact, run once, escape, hotkey → HUD, not hidden)
  + pending HUD stays until updated/launcher active (open-pane hotkey).
- smoke-windows-hud.ps1: exact gap 150 logical (× dpi/96 via GetDpiForWindow), height 46/56 cap,
  width ≤ 500; click-through already checked (keep).
- docs/platforms/linux.md: Wayland HUD fallback line under Remaining limits; hud.rs module docs.

## Status

- [x] Read ticket/spec/ADRs/research
- [ ] Implement core
- [ ] Implement window
- [ ] Tests
- [ ] Smoke + docs
- [ ] Push, CI quick tier green
