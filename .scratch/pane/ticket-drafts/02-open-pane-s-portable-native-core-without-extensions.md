# 02 - Open Pane's portable native core without extensions

**What to build:** A minimal GPUI CE shell with portable build/run entry points and OS-specific code behind explicit adapters, usable without a guest runtime.

**Blocked by:** None - can start immediately after breakdown approval and a separate implementation request.

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US01, US04, US12, US13. Planned scenarios T01. Gates G2.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Create the minimal GPUI CE shell under the Pane name with a Windows native smoke; keep shared code buildable on Windows, macOS and Linux and isolate platform-specific initialization behind adapters.
- [ ] Show meaningful empty/setup states with no extensions and no downloaded guest runtime; ordinary users do not need a console window.
- [ ] Keep default features and AI out of the permanent feature implementation; later tickets supply extension contributions.
- [ ] Document equivalent build/run commands and prerequisites for all three OS families without requiring Windows or PowerShell on Unix systems. Native macOS/Linux boot and contributor validation are required early follow-ups, not deferred release-port work.

## Scope and prerequisites

No application search, catalog, guest execution, installer or visual polish project.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
