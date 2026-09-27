# 29 - Install required extension dependencies

**What to build:** Installing a local fixture extension shows and installs its compatible missing required dependencies while preserving optional, disabled and pinned choices.

**Blocked by:** [19 - Call explicit operations across extension languages](19-call-explicit-operations-across-extension-languages.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US25, US26, US27, US28, US29. Planned scenarios T12, T14. Gates G4.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Extend the manifest/addressing contract with required versus optional dependencies and compatible version/platform requirements.
- [ ] Show the required set before installing; optional integrations remain absent unless separately requested, and deliberately disabled dependencies are not re-enabled.
- [ ] Explain unavailable/pinned/conflicting dependencies; define bounded cycle and partial-install behavior with no general multi-version solver.
- [ ] Verify the resulting cross-extension call works and failed resolution does not leave misleading ready-to-run state.

## Scope and prerequisites

Controlled/local package sources first; remote acquisition integrations follow without changing these semantics.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
