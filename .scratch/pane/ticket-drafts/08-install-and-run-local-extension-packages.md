# 08 - Install and run local extension packages

**What to build:** A user selects a supported local package in Pane, sees its identity and compatibility, installs it and invokes its command.

**Blocked by:** [07 - Keep a working contributor workflow on all three operating systems](07-keep-a-working-contributor-workflow-on-all-three-operating-systems.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US17, US18, US19, US20, US21, US22, US23, US24. Planned scenarios T11. Gates G4.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Define the minimum manifest/artifact and host/API compatibility fields needed for runnable JS/TS and Rust component packages; version the contract and explain invalid/incompatible packages in the UI.
- [ ] Resolve local identity from its absolute source location, separate it from display title, and reject duplicate explicit installs while supporting tracked replacement.
- [ ] Preserve user-owned source folders; manage local and later published copies independently without automatic switching or data merging.
- [ ] Keep package metadata available without executing the guest and prove install/run/duplicate/incompatible outcomes through the public host interface.
- [ ] Verify local identities and package paths on all supported contributor OS baselines, including spaces, Unicode and native path/case/symlink behavior as defined by the documented policy; do not apply Windows normalization blindly.

## Scope and prerequisites

Remote acquisition and cross-extension dependency solving are separate; source-only packages do not magically become toolchain-free releases.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
