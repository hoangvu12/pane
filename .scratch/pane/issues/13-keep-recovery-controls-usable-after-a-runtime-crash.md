# 13 - Keep recovery controls usable after a runtime crash

**What to build:** A user sees that the extension runtime stopped, can open management and explicitly retry without replaying an action.

**Blocked by:** [11 - Run and stop a packaged native helper](11-run-and-stop-a-packaged-native-helper.md); [12 - Pause an attributable broken extension and offer Retry](12-pause-an-attributable-broken-extension-and-offer-retry.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US77, US78, US80; scenarios T18, T19; gates G3. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Kill the selected runtime process while two fixture extensions are active; keep core navigation/settings and a recovery explanation usable.
- [ ] When attribution is unavailable, report runtime failure without naming the last-active extension as its cause. Suppress repeated automatic restarts and retain saved data.
- [ ] Simulate losing a response after a completed side effect; restarting execution must not automatically run the action again.
- [ ] Verify managed helper cleanup, explicit retry and diagnostics through the public host boundary and a native UI smoke.

## Scope

A terminated runtime and response-loss recovery. Non-cooperating guest hangs are the next separate slice.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 11 supplies real managed helper artifacts and cleanup behavior used by this flow.
- 12 supplies attributable failure status and Retry/repeated-failure policy.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
