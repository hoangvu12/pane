# 23 - Show a calculator result in root search

**What to build:** Typing a supported expression into root search returns a calculator result from a disableable default extension.

**Blocked by:** [19 - Find and invoke installed commands through root search](19-find-and-invoke-installed-commands-through-root-search.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US06, US12; scenarios T01, T03; gates . These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Define a small documented expression scope and calculate through the guest/provider path rather than embedding the feature in the permanent core.
- [ ] Show valid, invalid and incomplete expression behavior without turning ordinary input errors into extension failure.
- [ ] Support the normal selection/action flow for results and preserve responsiveness while typing.
- [ ] Verify that disabling the feature stops its contributions and leaves other root-search providers working.
- [ ] Check the real guest/provider path and native result action on all three supported contributor baselines.

## Scope

No symbolic algebra suite or arbitrary code evaluation feature.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 19 supplies root provider results, command selection and invocation.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
