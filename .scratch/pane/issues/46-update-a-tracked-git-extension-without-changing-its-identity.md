# 46 - Update a tracked Git extension without changing its identity

**What to build:** A user updates a tracked Git package through the existing controls while pinned revisions remain unchanged.

**Blocked by:** [42 - Install and run a Git-distributed component package](42-install-and-run-a-git-distributed-component-package.md); [45 - Update an eligible npm extension at a safe activation boundary](45-update-an-eligible-npm-extension-at-a-safe-activation-boundary.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US72, US73, US74, US75; scenarios T15, T08, T11; gates G3, G4. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Use a controlled repository to distinguish tracked-ref updates from explicit pinned revisions according to the acquisition contract.
- [ ] Resolve a compatible runnable artifact, retain canonical repository identity, and stage it through the shared update lifecycle.
- [ ] Verify pinned/local copies and deliberately disabled dependencies remain under user control, including a moved tracked ref and a missing artifact.
- [ ] Demonstrate manual/automatic controls, deferred activation and startup failure through the existing management UI.

## Scope

One Git update-source adapter, reusing the safe replacement/control behavior.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 42 supplies Git identity, runnable artifact acquisition and tracked-ref/pin semantics.
- 45 supplies source-independent update controls and safe activation behavior.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
