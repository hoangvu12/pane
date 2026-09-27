# 15 - Recover when the shared runtime crashes or hangs

**What to build:** The native core stays recoverable after the runtime stops responding, with honest runtime-level diagnostics and no blind replay of user actions.

**Blocked by:** [13 - Invoke and clean up a prebuilt native helper](13-invoke-and-clean-up-a-prebuilt-native-helper.md); [14 - Pause an identified broken extension through the UI](14-pause-an-identified-broken-extension-through-the-ui.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US77, US78, US80. Planned scenarios T18, T19. Gates G3.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Validate or revise the runtime supervision/process boundary using a killed process and a non-cooperating guest while several extensions are active.
- [ ] Keep core UI recovery accessible, suppress restart loops, and expose a bounded recovery action when the culprit cannot be attributed.
- [ ] Do not blame the last-active extension solely from a startup marker or activity record; distinguish interrupted startup from proven extension failure.
- [ ] Simulate response loss after a side effect and prove recovery does not replay it automatically; verify managed helper cleanup at this boundary.

## Scope and prerequisites

No security guarantee against arbitrary trusted external code, and no claim that recovery fixes a crash in Pane's own core.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
