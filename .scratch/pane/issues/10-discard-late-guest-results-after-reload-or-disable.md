# 10 - Discard late guest results after reload or disable

**What to build:** Reloading or disabling an extension with pending async work stops managed resources and prevents old replies from changing the current UI.

**Blocked by:** [07 - Reload one extension without restarting Pane](07-reload-one-extension-without-restarting-pane.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US35, US54, US57, US64; scenarios T06, T09; gates G2, G3. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Define and exercise cancellation and completion ownership for pending guest calls, streams and futures across guest generations.
- [ ] Delay a result until after replacement/disable and prove it cannot mutate the current view or start new host operations under the stopped generation.
- [ ] Exercise overlapping calls, cancellation/error completion and repeated cycles from both language families, checking observable resource release.
- [ ] Document bounded cleanup behavior and unresolved runtime limits rather than treating the prior 20 sequential calls as concurrency proof.
- [ ] Run shared lifecycle regression checks on the native contributor OS matrix; platform-specific cleanup adapters must not leak Windows assumptions into the common lifecycle contract.

## Scope

Native process helpers and fatal shared-runtime termination are separate follow-ups.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 07 supplies the actual guest replacement and build-failure/startup-failure distinction.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
