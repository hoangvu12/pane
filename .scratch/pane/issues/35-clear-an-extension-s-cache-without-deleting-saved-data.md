# 35 - Clear an extension's cache without deleting saved data

**What to build:** A user clears one extension's disposable cache while its settings, content and local credentials remain intact.

**Blocked by:** [06 - Disable an extension and retain its settings after restart](06-disable-an-extension-and-retain-its-settings-after-restart.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US60, US64; scenarios T20; gates G5. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Add only the managed cache/credential storage distinctions needed by a fixture containing settings, content, cache and a local credential; document ownership and migration responsibilities.
- [ ] Expose Clear cache in management and complete it even if the guest is broken, without running that guest's cleanup code.
- [ ] Check each data category before and after the action; other source identities and external files remain intact.
- [ ] Use the same public storage and management contracts as extension execution; surface storage errors with an actionable outcome.

## Scope

One cache-clearing action. Uninstall and deletion of retained data are separate slices.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 06 supplies persistent enablement, saved-state ownership and the stop/re-enable lifecycle.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
