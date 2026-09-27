# 39 - Disable required dependents together or cancel

**What to build:** A user disabling a required dependency sees affected extensions and chooses Disable All or Cancel.

**Blocked by:** [38 - Install missing required dependencies with a local extension](38-install-missing-required-dependencies-with-a-local-extension.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US30, US31, US57, US59; scenarios T13; gates G3, G4. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Compute the required dependent closure under the documented dependency-cycle policy; exclude optional integrations.
- [ ] Show the affected identities and both choices before mutation. Cancel leaves enablement and running work unchanged.
- [ ] Confirming disables the displayed set, stops supported managed work and retains unexpired data.
- [ ] Re-enable the dependency alone and verify its dependents remain disabled; keep automatic failure-pausing distinct from this user action.

## Scope

Disablement only; uninstall and its data choices are separate.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 38 supplies required-versus-optional dependency relationships and conflict/cycle policy.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
