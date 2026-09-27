# 18 - Call an explicit operation in another extension

**What to build:** An installed JS/TS command calls a Rust operation and a Rust command calls JS/TS, displaying structured results and meaningful target errors.

**Blocked by:** [06 - Disable an extension and retain its settings after restart](06-disable-an-extension-and-retain-its-settings-after-restart.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US46, US47, US48, US49; scenarios T14; gates G2, G4. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Define explicit operation publication, source/resource addressing and the minimum input/result/error schema/version contract for the examples.
- [ ] Route calls through the host and lazily activate a valid enabled target when needed; a UI-only command is not implicitly a headless API.
- [ ] Exercise missing, disabled and incompatible targets without silent re-enablement; distinguish operation errors from lifecycle failure.
- [ ] Define bounded behavior for recursive/cyclic calls and cancellation ownership before claiming general composition.
- [ ] Include a pair of minimal cross-language author examples and assert results/errors through the public host interface.

## Scope

One named operation in two fixtures. Bounded recursion/cancellation behavior is part of this contract; no general workflow engine.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 06 supplies persistent enablement, saved-state ownership and the stop/re-enable lifecycle.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
