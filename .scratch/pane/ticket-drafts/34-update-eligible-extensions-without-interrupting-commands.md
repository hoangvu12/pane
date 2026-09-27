# 34 - Update eligible extensions without interrupting commands

**What to build:** Compatible published extensions update automatically under user controls, respecting pins/local copies and staging replacement until active work permits it.

**Blocked by:** [31 - Install npm-distributed component packages](31-install-npm-distributed-component-packages.md); [32 - Install Git-distributed component packages](32-install-git-distributed-component-packages.md); [33 - Run scheduled and continuing background work](33-run-scheduled-and-continuing-background-work.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US72, US73, US74, US75. Planned scenarios T08, T12, T15. Gates G3, G4.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Provide global/per-extension automatic-update controls and manual update actions for supported npm/Git tracking semantics.
- [ ] Check host/API/platform/dependency compatibility and leave pins/local sources untouched; report incompatible or unavailable updates.
- [ ] Define explicit activation boundaries for active commands, persistent views and background services; defer rather than replace midway through a command.
- [ ] Exercise successful replacement and startup failure with existing Retry/logs/no-rollback behavior; do not claim code rollback reverses migrations.

## Scope and prerequisites

No historical-version picker, universal version availability or Pane application auto-update.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
