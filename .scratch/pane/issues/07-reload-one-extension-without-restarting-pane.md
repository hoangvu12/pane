# 07 - Reload one extension without restarting Pane

**What to build:** Manual reload replaces one extension while Pane and an unrelated extension stay open, preserving saved data and reporting failed replacement startup.

**Blocked by:** [06 - Disable an extension and retain its settings after restart](06-disable-an-extension-and-retain-its-settings-after-restart.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US52, US54, US55, US56; scenarios T07, T08; gates G3, G5. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Replace actual guest code and assign generation ownership; demonstrate a changed guest result through the native UI without restarting Pane.
- [ ] Clean up the old instance before starting the validated replacement and retain managed saved data; transient restoration occurs only when explicitly supported.
- [ ] If replacement startup fails, report failure with Retry/diagnostics and managed cleanup, without restoring the old version automatically.
- [ ] Keep build/artifact validation failure before replacement distinct from startup failure after replacement; verify an unrelated active extension remains usable in ordinary reload.

## Scope

No automatic rollback or reversal of migrations/side effects; filesystem watchers/build invocation are next.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 06 supplies persistent enablement, saved-state ownership and the stop/re-enable lifecycle.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
