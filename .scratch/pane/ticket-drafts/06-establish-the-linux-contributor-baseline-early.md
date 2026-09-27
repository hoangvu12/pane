# 06 - Establish the Linux contributor baseline early

**What to build:** A Linux contributor can build the core and JS/TS/Rust guest examples, run native checks and exercise real view/event round trips on an explicitly supported desktop baseline.

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

- [ ] Use a real Linux environment and a fresh source checkout; document architecture, distribution, desktop/display protocol and native prerequisites without requiring Windows or PowerShell.
- [ ] Build the guest examples on Linux and complete native rendering plus real P3 event/result round trips; cross-compilation alone is insufficient.
- [ ] Establish an initial support matrix proposal for the tested desktop combinations, explicitly recording Wayland/X11 limitations instead of promising universal coverage.
- [ ] Fix bounded boot/runtime integration issues; larger issues remain blockers, and input/IME/accessibility findings are retained.
- [ ] Record reproducible developer commands and fix bounded shared build/path/process assumptions; this early gate is not postponed until Linux packaging or feature parity.

## Scope and prerequisites

Requires real Linux test access. No claim that one desktop test proves all Linux desktops or packaging formats.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
