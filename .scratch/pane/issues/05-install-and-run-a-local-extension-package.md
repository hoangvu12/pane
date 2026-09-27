# 05 - Install and run a local extension package

**What to build:** A user selects a supported local package in Pane, sees its identity and compatibility, installs it and invokes its command.

**Blocked by:** [03 - Build and run the sample command natively on macOS](03-build-and-run-the-sample-command-natively-on-macos.md); [04 - Build and run the sample command natively on Linux](04-build-and-run-the-sample-command-natively-on-linux.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US17, US18, US19, US20, US21, US22, US23, US24; scenarios T11; gates G4. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Define the minimum manifest/artifact and host/API compatibility fields needed for runnable JS/TS and Rust component packages; version the contract and explain invalid/incompatible packages in the UI.
- [ ] Resolve local identity from its absolute source location, separate it from display title, and reject duplicate explicit installs while supporting tracked replacement.
- [ ] Preserve user-owned source folders; manage local and later published copies independently without automatic switching or data merging.
- [ ] Keep package metadata available without executing the guest and prove install/run/duplicate/incompatible outcomes through the public host interface.
- [ ] Verify local identities and package paths on all supported contributor OS baselines, including spaces, Unicode and native path/case/symlink behavior as defined by the documented policy; do not apply Windows normalization blindly.

## Scope

Remote acquisition and cross-extension dependency solving are separate; source-only packages do not magically become toolchain-free releases.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 03 establishes the early macOS contributor build/run baseline required before shared feature expansion.
- 04 establishes the early Linux contributor build/run baseline required before shared feature expansion.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
