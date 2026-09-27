# 28 - Open an extension command with a global hotkey on Windows

**What to build:** A user assigns a supported Windows shortcut and opens the selected command while another application has focus.

**Blocked by:** [19 - Find and invoke installed commands through root search](19-find-and-invoke-installed-commands-through-root-search.md); [15 - Explain an unavailable extension action without hiding working actions](15-explain-an-unavailable-extension-action-without-hiding-working-actions.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US03, US10, US44, US57; scenarios T03, T09, T22; gates G2, G7. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Register and dispatch one configurable shortcut through the Windows integration on the stated baseline, then open the real guest command in Pane.
- [ ] Explain reserved, conflicting, denied or unavailable shortcuts in the UI; preserve other extension actions.
- [ ] Disable the extension or change its shortcut and confirm the old registration is released; retain the chosen setting across restart.
- [ ] Verify the focus transition and registration cleanup natively, including the platform's permission behavior.

## Scope

One global command shortcut. No clipboard implementation or general keyboard remapper.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 19 supplies root provider results, command selection and invocation.
- 15 supplies supported-target declarations and user-visible unavailable-action reasons.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
