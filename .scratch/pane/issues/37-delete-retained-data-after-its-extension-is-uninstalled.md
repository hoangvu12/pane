# 37 - Delete retained data after its extension is uninstalled

**What to build:** A user finds retained extension data in management and deletes it while the extension code is absent.

**Blocked by:** [36 - Uninstall an extension with an explicit saved-data choice](36-uninstall-an-extension-with-an-explicit-saved-data-choice.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US62, US63, US64; scenarios T20; gates G5. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] List retained data by canonical source identity with a readable title and clear deletion scope.
- [ ] Delete only the selected managed durable data without downloading or executing the former extension.
- [ ] Verify absence after restart and reinstall; other identities, credentials owned elsewhere and external files remain intact.
- [ ] Surface missing/locked-data errors through the management UI and public behavior checks.

## Scope

One retained-data deletion action; no automatic cross-source merging.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 36 supplies guest-independent removal and the durable-data choice.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
