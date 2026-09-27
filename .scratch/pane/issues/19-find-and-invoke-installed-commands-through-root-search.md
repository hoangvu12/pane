# 19 - Find and invoke installed commands through root search

**What to build:** Root search finds installed command metadata and activates only the selected enabled extension.

**Blocked by:** [06 - Disable an extension and retain its settings after restart](06-disable-an-extension-and-retain-its-settings-after-restart.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US09, US14; scenarios T01, T02, T03; gates G2, G4. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Build command discovery, basic matching/ranking, selection and invocation through the real host/UI path using installed package metadata.
- [ ] Inspect many installed fixtures without starting all guests; explicitly enabled background behavior remains distinct.
- [ ] Removing or disabling a contribution updates search coherently; distinguish a missing result from a failed action.
- [ ] Keep provider-specific functionality outside the core and document the minimum search-provider contract needed for default features.

## Scope

No automatic global querying of online integrations or final relevance-tuning project.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 06 supplies persistent enablement, saved-state ownership and the stop/re-enable lifecycle.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
