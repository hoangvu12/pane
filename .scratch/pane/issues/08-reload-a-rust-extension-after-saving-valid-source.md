# 08 - Reload a Rust extension after saving valid source

**What to build:** A Rust author edits the sample, saves it, and sees the new behavior while Pane stays open.

**Blocked by:** [07 - Reload one extension without restarting Pane](07-reload-one-extension-without-restarting-pane.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US19, US51, US53; scenarios T07, T08; gates G1, G3. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Connect the Rust sample's documented build command to a native file watcher and the existing replacement path; include source and reproduction instructions in the ticket's deliverable.
- [ ] A build error shows diagnostics and keeps the working guest. A successful build replaces only that extension and changes the native result; a replacement startup error follows the existing Retry/no-rollback policy.
- [ ] Save again during an in-progress build and verify an obsolete artifact cannot replace a newer successful build. Stop owned watcher/build work when development mode ends.
- [ ] Demonstrate the same save/error/recovery flow on the three supported contributor baselines; local and published copies retain separate identities.

## Scope

One build-on-save adapter for Rust, reusing the existing replacement behavior. No general build system or automatic development-copy swapping.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 07 supplies the actual guest replacement and build-failure/startup-failure distinction.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
