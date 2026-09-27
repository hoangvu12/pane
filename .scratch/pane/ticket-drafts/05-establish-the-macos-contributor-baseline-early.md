# 05 - Establish the macOS contributor baseline early

**What to build:** A macOS contributor can build the core and JS/TS/Rust guest examples, run native checks and exercise real view/event round trips before shared SDK and package work expands.

**Blocked by:** [04 - Run JS and TS commands through the same UI contract](04-run-js-and-ts-commands-through-the-same-ui-contract.md).

**Status:** draft - awaiting breakdown approval

**Type:** validation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US02, US03, US81. Planned scenarios T04, T05, T22, T25. Gates G1, G2, G7.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Use a real macOS environment and a fresh source checkout to build/run GPUI CE and the runtime; document minimum OS/architecture and native prerequisites without requiring a Windows machine.
- [ ] Build the guest examples on macOS and complete real P3 execution plus native UI input/result round trips; cross-compilation or mocked OS values do not pass.
- [ ] Fix only bounded boot/runtime integration issues and record larger incompatibilities as explicit blockers instead of calling the platform supported.
- [ ] Check initial focus/text input/IME/accessibility behavior relevant to the existing controls and carry findings into the platform adapter tickets.
- [ ] Record reproducible developer commands and fix bounded build/path/process assumptions in the shared code; this gate completes before the contributor-workflow milestone and broad shared-feature development.

## Scope and prerequisites

Requires access to a real macOS test environment. This is not full feature or installer certification.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
