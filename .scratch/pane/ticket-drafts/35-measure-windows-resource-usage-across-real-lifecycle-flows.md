# 35 - Measure Windows resource usage across real lifecycle flows

**What to build:** A repeatable Windows measurement report shows actual process-tree costs for idle Pane, inactive/active extensions, startup and lifecycle churn.

**Blocked by:** [15 - Recover when the shared runtime crashes or hangs](15-recover-when-the-shared-runtime-crashes-or-hangs.md); [20 - Discover and invoke commands through root search](20-discover-and-invoke-commands-through-root-search.md); [33 - Run scheduled and continuing background work](33-run-scheduled-and-continuing-background-work.md).

**Status:** draft - awaiting breakdown approval

**Type:** validation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US04, US14. Planned scenarios T02, T24. Gates G7.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Use the production shell/runtime and representative installed packages, separating cold compilation/start, warm start, idle, active commands and continuing services.
- [ ] Measure the entire process tree and managed-resource release over repeated calls/reload/disable rather than treating CLI peaks as additive per-extension memory.
- [ ] Propose explicit resource/latency targets with evidence and scope, recording any needed user choice without inventing an earlier numeric promise.
- [ ] Keep measurements reproducible with documented workload/build conditions and surface failures; do not claim all-platform performance.

## Scope and prerequisites

A baseline and targeted investigation of observed issues, not unlimited optimization or a broad benchmark suite.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
