# 12 - Pause an attributable broken extension and offer Retry

**What to build:** Pane skips an identified failing extension, shows a toast and persistent status, and allows Retry without requiring a CLI.

**Blocked by:** [10 - Discard late guest results after reload or disable](10-discard-late-guest-results-after-reload-or-disable.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US77, US78, US79, US80; scenarios T17; gates G3. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Distinguish expected operation errors from attributable fatal startup/repeated execution failures; healthy operations do not indiscriminately disable the extension.
- [ ] Define a small failure-paused state separate from user-disabled state, with recorded generation/version, bounded repeated activation and persistence across restart.
- [ ] Show a useful toast plus persistent Retry/Details controls; preserve saved data and keep core settings/management usable.
- [ ] Test attributable failure, normal operation error, restart and retry/reload recovery; document thresholds and reset behavior as explicit choices.

## Scope

This does not identify the culprit in an unattributed shared-process crash; no dependency-uninstall cascade on automatic failure.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 10 supplies pending-work cancellation and ownership of late generation results.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
