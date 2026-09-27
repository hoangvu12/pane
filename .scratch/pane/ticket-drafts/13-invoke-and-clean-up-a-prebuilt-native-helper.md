# 13 - Invoke and clean up a prebuilt native helper

**What to build:** A component command invokes an OS-matched prebuilt helper, shows its result, and stops the managed process on cancel, disable or reload.

**Blocked by:** [12 - Cancel in-flight work during reload and disable](12-cancel-in-flight-work-during-reload-and-disable.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US40, US41, US42, US64. Planned scenarios T09, T22. Gates G2, G3.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Add the narrow host API and package declaration needed to select and launch a prebuilt helper with managed input/output, completion and errors.
- [ ] Provide and run a prebuilt helper fixture for each supported contributor OS/architecture baseline; no compilation occurs on the end-user path, and absent/wrong-target artifacts are explained.
- [ ] Cancel/disable/reload while the helper runs and verify cleanup of supported managed processes/handles, keeping saved data intact.
- [ ] Keep the extension entry point WASI3 and document detached-descendant limits; do not promise arbitrary process containment.

## Scope and prerequisites

One representative native-library/OS integration path, not every external program or platform artifact.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
