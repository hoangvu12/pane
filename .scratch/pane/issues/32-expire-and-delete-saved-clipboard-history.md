# 32 - Expire and delete saved clipboard history

**What to build:** Clipboard history has configurable finite retention and deletion controls whose effects remain correct while disabled and after downtime.

**Blocked by:** [31 - Capture opt-in clipboard history on Windows](31-capture-opt-in-clipboard-history-on-windows.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US67, US68, US69; scenarios T10, T21; gates G5. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Select and document a finite default retention recommendation, obtaining a user decision if it changes unresolved product expectations; implement the configurable policy.
- [ ] Expire records independently of executing the extension and enforce expiry before display after downtime; re-enable does not restart retention.
- [ ] Provide per-item/bulk deletion, Clear history and Disable and delete history with their distinct future-capture effects.
- [ ] Use an injectable clock for expiry scenarios plus a real capture/delete smoke; coordinate writes during deletion to avoid immediately reviving removed history.

## Scope

No forensic erasure promise or deletion of the OS's current clipboard as a side effect of managed-history removal.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 31 supplies the first usable capture/history view and shared storage contract; this prerequisite is about reusing that behavior.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
