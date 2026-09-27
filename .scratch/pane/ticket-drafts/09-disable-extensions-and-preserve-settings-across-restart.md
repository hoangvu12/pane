# 09 - Disable extensions and preserve settings across restart

**What to build:** A user disables a local extension in the UI, restarts Pane, and can re-enable it with its saved settings intact.

**Blocked by:** [08 - Install and run local extension packages](08-install-and-run-local-extension-packages.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US57, US58, US59. Planned scenarios T01, T10. Gates G3, G5.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Introduce the smallest managed settings/durable-state contract needed by the fixture and persist user-selected enablement independently of whether an instance is running.
- [ ] Disable removes the fixture's contributions and stops its currently supported host-managed work; restarting Pane does not reactivate it.
- [ ] Re-enable restores settings and normal contributions without changing the source identity or silently enabling other installations.
- [ ] Exercise restart and two distinct-source copies with the same title; ownership follows identity, not display name.

## Scope and prerequisites

Full async cancellation, background services, credentials, cache deletion and clipboard expiry have their own slices.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
