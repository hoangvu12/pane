# 12 - Cancel in-flight work during reload and disable

**What to build:** Reloading or disabling an extension with pending async work stops managed resources and prevents old replies from changing the current UI.

**Blocked by:** [10 - Replace a running extension through manual reload](10-replace-a-running-extension-through-manual-reload.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US35, US54, US57, US64. Planned scenarios T06, T09. Gates G2, G3.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Define and exercise cancellation and completion ownership for pending guest calls, streams and futures across guest generations.
- [ ] Delay a result until after replacement/disable and prove it cannot mutate the current view or start new host operations under the stopped generation.
- [ ] Exercise overlapping calls, cancellation/error completion and repeated cycles from both language families, checking observable resource release.
- [ ] Document bounded cleanup behavior and unresolved runtime limits rather than treating the prior 20 sequential calls as concurrency proof.
- [ ] Run shared lifecycle regression checks on the native contributor OS matrix; platform-specific cleanup adapters must not leak Windows assumptions into the common lifecycle contract.

## Scope and prerequisites

Native process helpers and fatal shared-runtime termination are separate follow-ups.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
