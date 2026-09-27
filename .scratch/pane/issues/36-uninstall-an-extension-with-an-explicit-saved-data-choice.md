# 36 - Uninstall an extension with an explicit saved-data choice

**What to build:** A user removes an extension and chooses whether to keep its durable data.

**Blocked by:** [35 - Clear an extension's cache without deleting saved data](35-clear-an-extension-s-cache-without-deleting-saved-data.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US61, US63, US64; scenarios T20; gates G5. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Show the keep/delete durable-data choice; remove managed code, cache and locally managed credentials for either choice, using the established ownership contract.
- [ ] Stop managed work and remove contributions without executing broken guest code; explain partial filesystem failures rather than reporting a successful uninstall.
- [ ] Preserve developer source folders and external documents. Test two source identities with identical display titles.
- [ ] Reinstall the same source after keeping data and verify ownership remains with that identity; do not merge another source's data or claim remote credential revocation.

## Scope

One extension and its saved-data choice. Required dependent removal is a later slice.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 35 supplies the tested managed settings/content/cache/credential ownership distinctions.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
