# 43 - Run a scheduled extension task and stop it on disable

**What to build:** A user enables a scheduled task, sees its result/status in Pane, and disables it to stop future runs.

**Blocked by:** [12 - Pause an attributable broken extension and offer Retry](12-pause-an-attributable-broken-extension-and-offer-retry.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US14, US45, US57, US79; scenarios T02, T09, T17; gates G3. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Define one minimal schedule declaration and documented restart/due-work behavior; installation alone must not schedule a disabled extension.
- [ ] Run a real guest task and expose its result/status without blocking the native UI or activating unrelated packages.
- [ ] Disable or reload while a task is pending; cancel supported work and prevent duplicate activation or late-generation results.
- [ ] Check clock-driven scheduling through public behavior, expected errors and repeated fatal failure handling; ship the small author sample.

## Scope

One schedule type with a bounded policy, not a general scheduler.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 12 supplies attributable failure status and Retry/repeated-failure policy.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
