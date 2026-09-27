# 10 - Replace a running extension through manual reload

**What to build:** Manual reload replaces one extension while Pane and an unrelated extension stay open, preserving saved data and reporting failed replacement startup.

**Blocked by:** [09 - Disable extensions and preserve settings across restart](09-disable-extensions-and-preserve-settings-across-restart.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US52, US54, US55, US56. Planned scenarios T07, T08. Gates G3, G5.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Replace actual guest code and assign generation ownership; demonstrate a changed guest result through the native UI without restarting Pane.
- [ ] Clean up the old instance before starting the validated replacement and retain managed saved data; transient restoration occurs only when explicitly supported.
- [ ] If replacement startup fails, report failure with Retry/diagnostics and managed cleanup, without restoring the old version automatically.
- [ ] Keep build/artifact validation failure before replacement distinct from startup failure after replacement; verify an unrelated active extension remains usable in ordinary reload.

## Scope and prerequisites

No automatic rollback or reversal of migrations/side effects; filesystem watchers/build invocation are next.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
