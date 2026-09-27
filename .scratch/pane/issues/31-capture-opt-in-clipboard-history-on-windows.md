# 31 - Capture opt-in clipboard history on Windows

**What to build:** A Windows clipboard extension starts off, captures supported clipboard content only when enabled, and supports visible pause/disable controls.

**Blocked by:** [06 - Disable an extension and retain its settings after restart](06-disable-an-extension-and-retain-its-settings-after-restart.md); [15 - Explain an unavailable extension action without hiding working actions](15-explain-an-unavailable-extension-action-without-hiding-working-actions.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US65, US66, US70, US71; scenarios T10, T21, T22; gates G5, G7. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Define and document a bounded initial clipboard-content format scope and use a real Windows observation path.
- [ ] Verify initial opt-in, active capture, pause/resume and disable stopping host-managed observation, including after restart.
- [ ] Respect available sensitive markers and configured app exclusions; disclose platform detection limits and keep capture local by default.
- [ ] Store captured items under managed ownership and provide a native history view without claiming this ticket establishes final retention/deletion behavior.

## Scope

No automatic AI/network transmission or guaranteed secret detection; retention is the next slice.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 06 supplies persistent enablement, saved-state ownership and the stop/re-enable lifecycle.
- 15 supplies supported-target declarations and user-visible unavailable-action reasons.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
