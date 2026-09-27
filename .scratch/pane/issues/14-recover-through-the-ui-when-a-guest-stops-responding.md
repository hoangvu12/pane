# 14 - Recover through the UI when a guest stops responding

**What to build:** A user can recover from a non-cooperating guest without closing Pane or losing saved data.

**Blocked by:** [13 - Keep recovery controls usable after a runtime crash](13-keep-recovery-controls-usable-after-a-runtime-crash.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US77, US78, US80; scenarios T18, T19; gates G3. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Run a bounded fixture that intentionally fails to cooperate with cancellation while another extension is active; observe UI responsiveness.
- [ ] Apply a documented timeout/interruption policy, release supported managed work, and present the existing recovery controls. State effects on other active guests honestly.
- [ ] Keep expected slow-operation errors separate from known fatal failure; do not invent culprit attribution when only a shared-runtime hang is known.
- [ ] Verify that recovery neither replays external side effects nor restores an obsolete generation; retain native diagnostics and resource-release evidence.

## Scope

One hang/interruption path using the existing supervision contract. Any runtime inability to support it is a concrete blocker, not an unbounded backend rewrite.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 13 supplies the recoverable execution boundary and no-replay behavior used for hangs.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
