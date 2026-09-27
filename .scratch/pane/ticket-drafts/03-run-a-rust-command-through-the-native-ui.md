# 03 - Run a Rust command through the native UI

**What to build:** A Rust WASI3 fixture command displays a list in Pane, receives a real user action, and returns an updated view.

**Blocked by:** [01 - Validate a maintainable WASI3 runtime candidate](01-validate-a-maintainable-wasi3-runtime-candidate.md); [02 - Open Pane's portable native core without extensions](02-open-pane-s-portable-native-core-without-extensions.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US33, US34, US35, US36, US38. Planned scenarios T04, T05. Gates G1, G2.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Define only the minimal versioned host/guest operations, identifiers and list/action view contract needed for this complete round trip; expose the same public behavior to integration checks and UI.
- [ ] Execute the real component through the selected managed-runtime boundary, keeping the core responsive and unavailable-runtime errors visible.
- [ ] A GPUI action reaches guest code and its response updates the view; a JSON-file replacement or logged click alone does not pass.
- [ ] Document and test the first IPC/serialization contract and lifecycle owner without exposing raw GPUI or engine objects as the public SDK.
- [ ] Keep host/guest protocols and process launching independent of Windows-only handles, path syntax or shell assumptions; preserve common behavior for the early native macOS/Linux gates.

## Scope and prerequisites

One list/action path; no full custom-view language or final commitment to every process-topology detail.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
