# 24 - Create and invoke a persistent URL quicklink

**What to build:** A user saves a URL quicklink, finds it in root search after restarting Pane, and opens it in their default browser.

**Blocked by:** [16 - Submit and validate a native form from an extension](16-submit-and-validate-a-native-form-from-an-extension.md); [19 - Find and invoke installed commands through root search](19-find-and-invoke-installed-commands-through-root-search.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US03, US07, US12, US59; scenarios T01, T03, T10, T22; gates G2, G5, G7. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Provide create/edit/remove for one documented URL-target format through the default extension's native form; validate invalid inputs without treating them as fatal extension errors.
- [ ] Persist entries under extension ownership, retain them on disable, hide disabled contributions and restore retained entries when re-enabled.
- [ ] Open the selected URL through the platform's normal handler and explain missing-handler or denied-action errors.
- [ ] Test form-to-storage-to-search-to-open behavior, restart and disable on all supported contributor baselines.

## Scope

A URL quicklink is the first bounded target format. Additional target types require their own slices; no separate per-OS quicklink feature bundle.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 16 supplies native input, submission, focus and guest event handling.
- 19 supplies root provider results, command selection and invocation.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
