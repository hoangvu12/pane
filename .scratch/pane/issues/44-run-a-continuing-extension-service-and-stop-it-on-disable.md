# 44 - Run a continuing extension service and stop it on disable

**What to build:** A user starts an enabled background service, sees its status, and stops it through extension management.

**Blocked by:** [12 - Pause an attributable broken extension and offer Retry](12-pause-an-attributable-broken-extension-and-offer-retry.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US14, US45, US57, US79; scenarios T02, T09, T17; gates G3. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Define the minimal continuing-service declaration, activation and restart behavior, keeping unused extensions inactive.
- [ ] Use a fixture service to emit visible status and manage one subscription/task; ordinary errors and repeated fatal failures follow the existing recovery policy.
- [ ] Disable/reload and verify the service's supported tasks/subscriptions stop and old results cannot update the current UI.
- [ ] Retain restart/resource-release evidence and include a runnable author sample with documented lifetime behavior.

## Scope

One continuing service. It does not depend on the separate scheduled-task implementation.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 12 supplies attributable failure status and Retry/repeated-failure policy.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
