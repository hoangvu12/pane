# 34 - Capture opt-in clipboard history on Linux

**What to build:** A Linux user enables clipboard history, sees a captured item, and can pause, delete or expire it through Pane.

**Blocked by:** [32 - Expire and delete saved clipboard history](32-expire-and-delete-saved-clipboard-history.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US03, US44, US65, US66, US67, US68, US69, US70, US71; scenarios T21, T22; gates G5, G7. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Implement Linux observation for the existing history format and show real capture in the native history view; history is off initially.
- [ ] Reuse the retention/storage contract and verify pause/disable, restart, downtime expiry and the distinction between Clear and Disable and delete.
- [ ] Apply supported sensitive markers/app exclusions and explain platform limitations. Capture remains local by default.
- [ ] Record results separately for each claimed display/desktop combination; unsupported capture must not hide other working extension actions.

## Scope

Clipboard capture using the existing policy and history view. Global shortcuts are a separate slice.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 32 supplies the shared clipboard history, expiry and deletion semantics reused by the native capture adapter.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
