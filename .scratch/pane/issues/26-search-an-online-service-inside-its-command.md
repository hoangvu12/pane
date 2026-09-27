# 26 - Search an online service inside its command

**What to build:** Opening an online extension command and entering a query displays service results without querying it during ordinary root search.

**Blocked by:** [10 - Discard late guest results after reload or disable](10-discard-late-guest-results-after-reload-or-disable.md); [19 - Find and invoke installed commands through root search](19-find-and-invoke-installed-commands-through-root-search.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US11, US39, US40; scenarios T03, T09; gates G2, G3. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Run a real guest against a controlled HTTP fixture and show its returned results in the native view; document the supported networking/library path.
- [ ] Assert no service-content request is made merely by typing in root search.
- [ ] Cancel an obsolete query, reject its late response and display expected offline/service errors without pausing the entire extension.
- [ ] Include the sample source and public behavior checks for networking and view updates.

## Scope

One fixture service and its command. Aliases, fallback selection and global hotkeys follow separately.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 10 supplies pending-work cancellation and ownership of late generation results.
- 19 supplies root provider results, command selection and invocation.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
