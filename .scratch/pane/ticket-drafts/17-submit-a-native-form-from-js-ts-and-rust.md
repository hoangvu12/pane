# 17 - Submit a native form from JS/TS and Rust

**What to build:** A native extension form accepts input, validates it, and returns a visible guest result through the same contract in JS/TS and Rust.

**Blocked by:** [07 - Keep a working contributor workflow on all three operating systems](07-keep-a-working-contributor-workflow-on-all-three-operating-systems.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US36, US38. Planned scenarios T05. Gates G2.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Add only the standard controls, focus and event semantics needed for a representative form and validation/error flow.
- [ ] Verify keyboard traversal, text editing and IME on the supported Windows/macOS/Linux contributor baselines through native interaction, with equivalent guest behavior from both language families.
- [ ] Establish and check concrete accessible labeling/focus behavior for the controls used; record unsupported accessibility behavior instead of claiming parity.
- [ ] Document the extension contract and meaningful observable checks, avoiding tests coupled to private GPUI node layouts.

## Scope and prerequisites

A representative standard form, not an exhaustive widget toolkit.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
