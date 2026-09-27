# 01 - Validate a maintainable WASI3 runtime candidate

**What to build:** A reproducible WASI3 runtime candidate for JS, TS and Rust, with portable build inputs and correct fresh-instance initialization, ready for early native validation on all three operating systems.

**Blocked by:** None - can start immediately after breakdown approval and a separate implementation request.

**Status:** draft - awaiting breakdown approval

**Type:** validation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US32, US34, US35, US39, US50. Planned scenarios T04, T06. Gates G1.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Start from the saved P3-only host and patched-JS/Rust evidence; reproduce the snapshot-random-state defect before changing it, and verify the fix across multiple fresh instances.
- [ ] Run real library, async and filesystem cases with only P3 interfaces registered; retain a mixed P2/P3 guest as a rejecting control.
- [ ] Pin a reproducible author toolchain and platform-neutral build entry point; declare per-OS prerequisites and runtime/SDK artifact selection without hardcoded Windows paths, PowerShell-only orchestration or changing global toolchains. Record measured artifact/initialization costs, library limitations and patch-maintenance options.
- [ ] Record the candidate and unresolved risks explicitly. QuickJS is not predetermined; do not satisfy this ticket by silently falling back to Node, native entry points or WASI2.
- [ ] Record initial Windows evidence separately from pending macOS/Linux evidence; the candidate cannot be called contributor-ready across platforms until the early native gates pass.

## Scope and prerequisites

This proves the runtime candidate, not production IPC, cancellation, the full SDK or all-platform support.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
