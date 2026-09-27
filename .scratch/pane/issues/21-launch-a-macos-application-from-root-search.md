# 21 - Launch a macOS application from root search

**What to build:** A user finds an installed macOS application in Pane, launches it, and can disable that default extension.

**Blocked by:** [15 - Explain an unavailable extension action without hiding working actions](15-explain-an-unavailable-extension-action-without-hiding-working-actions.md); [19 - Find and invoke installed commands through root search](19-find-and-invoke-installed-commands-through-root-search.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US03, US05, US12, US44; scenarios T01, T03, T22; gates G2, G7. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Implement the minimum macOS discovery/launch adapter and default extension using the root provider contract; state the supported application locations and desktop baseline.
- [ ] Select a real discovered application in the native UI and observe its launch; expected unavailable/permission errors remain visible.
- [ ] Disable the extension and confirm its results and managed observation disappear without affecting other providers.
- [ ] Use deterministic discovery fixtures plus a real native launch. Share provider contracts with the other OS adapters; this slice does not require another OS's app launcher to finish.

## Scope

One platform's application discovery and launch. File search and quicklinks are separate features.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 15 supplies supported-target declarations and user-visible unavailable-action reasons.
- 19 supplies root provider results, command selection and invocation.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
