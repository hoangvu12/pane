# 04 - Run JS and TS commands through the same UI contract

**What to build:** JS and TS fixture extensions drive the same native list/action interaction as Rust, using the candidate engine behind Pane's API.

**Blocked by:** [03 - Run a Rust command through the native UI](03-run-a-rust-command-through-the-native-ui.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US32, US33, US36, US38, US39, US50. Planned scenarios T04, T05, T06. Gates G1, G2.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Produce separate working JS and TS author examples; both generate P3-only components and complete the same event/result round trip as Rust.
- [ ] Demonstrate compatible library use and errors through the real host, retaining correct fresh-instance initialization from the runtime validation.
- [ ] Expose a minimal ergonomic binding for each launch language without leaking the engine-specific ABI into author-facing operations.
- [ ] Compare observable behavior across languages and document actual differences; do not claim general Node/npm compatibility.
- [ ] Provide portable JS/TS/Rust example build and execution instructions with platform-specific prerequisites explicit; test tooling must not require a Windows checkout to prepare artifacts for another OS.

## Scope and prerequisites

No Python/C#, universal library layer or unchanged Raycast/Pi API.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
