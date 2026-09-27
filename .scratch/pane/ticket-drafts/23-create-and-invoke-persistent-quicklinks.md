# 23 - Create and invoke persistent quicklinks

**What to build:** A user creates a quicklink in a native form, finds it in root search and invokes it after restarting Pane.

**Blocked by:** [17 - Submit a native form from JS/TS and Rust](17-submit-a-native-form-from-js-ts-and-rust.md); [20 - Discover and invoke commands through root search](20-discover-and-invoke-commands-through-root-search.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US07, US12. Planned scenarios T01, T03, T10. Gates G5.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Implement the narrow create/edit/remove and invocation flow through a default extension, documenting supported target types.
- [ ] Persist quicklink data under extension ownership and report invalid/unavailable targets clearly.
- [ ] Disabling hides contributions without deleting saved quicklinks; re-enabling/restoring Pane makes retained entries available.
- [ ] Exercise form submission, persisted restart, search and a real supported Windows target action.

## Scope and prerequisites

No browser sync, cloud accounts or workflow-builder scope.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
