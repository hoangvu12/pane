# 06 - Disable an extension and retain its settings after restart

**What to build:** A user disables a local extension in the UI, restarts Pane, and can re-enable it with its saved settings intact.

**Blocked by:** [05 - Install and run a local extension package](05-install-and-run-a-local-extension-package.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US57, US58, US59; scenarios T01, T10; gates G3, G5. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Introduce the smallest managed settings/durable-state contract needed by the fixture and persist user-selected enablement independently of whether an instance is running.
- [ ] Disable removes the fixture's contributions and stops its currently supported host-managed work; restarting Pane does not reactivate it.
- [ ] Re-enable restores settings and normal contributions without changing the source identity or silently enabling other installations.
- [ ] Exercise restart and two distinct-source copies with the same title; ownership follows identity, not display name.
- [ ] Verify behavior through the public management interface and a native restart smoke with a real guest.

## Scope

Full async cancellation, background services, credentials, cache deletion and clipboard expiry have their own slices.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 05 supplies installed package identity, compatible artifacts and a real invocable guest.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
