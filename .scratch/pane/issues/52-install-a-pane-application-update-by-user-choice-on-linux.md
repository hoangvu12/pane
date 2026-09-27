# 52 - Install a Pane application update by user choice on Linux

**What to build:** A Linux user sees an update notification and chooses whether to install it.

**Blocked by:** [49 - Install Pane and acquire its calculator on Linux](49-install-pane-and-acquire-its-calculator-on-linux.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US76; scenarios T16; gates G6, G7. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Use controlled metadata and two runnable builds to show notification, explicit installation and a successful version transition.
- [ ] Taking no action must cause no automatic application download, install or restart. Keep extension auto-update controls separate.
- [ ] Maintain compatible runtime artifacts, extension pins, saved data and enablement; document active-work handling and restart timing.
- [ ] On the native Linux baseline, exercise acquisition/replacement failure and show a recoverable installation with useful UI diagnostics. Include native packaging/signing prerequisites in the result.

## Scope

One OS application updater. Shared protocol code is reusable; completion does not depend on another OS's updater.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 49 supplies the installed Linux package and runtime acquisition flow to update.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
