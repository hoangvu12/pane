# 18 - Drive a custom interactive view from an extension

**What to build:** A JS/TS and Rust extension each drive one custom interactive visualization or control with real input and guest-updated rendering.

**Blocked by:** [17 - Submit a native form from JS/TS and Rust](17-submit-a-native-form-from-js-ts-and-rust.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US37, US38. Planned scenarios T05. Gates G2.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Choose one bounded example such as a color picker or small graph and define the minimal layout/drawing/event behavior it requires.
- [ ] Complete native input-to-guest-to-view round trips with usable focus and the accessible representation appropriate to that control.
- [ ] Define resource/event ownership for view replacement and closure; avoid exposing raw GPUI objects or engine internals as the SDK.
- [ ] Document the supported contract and limitations without turning the probe's JSON format into the whole public UI API by accident.
- [ ] Validate the example's rendering and interaction on the early supported native OS baselines; shared custom-view changes must not silently regress macOS or Linux.

## Scope and prerequisites

No arbitrary GPUI API parity, web browser embedding or complete drawing framework.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
