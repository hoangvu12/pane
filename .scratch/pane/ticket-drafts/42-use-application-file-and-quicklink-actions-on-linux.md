# 42 - Use application, file and quicklink actions on Linux

**What to build:** The existing default app/file/quicklink workflows use Linux desktop discovery and open actions on the recorded support matrix.

**Blocked by:** [21 - Launch Windows applications from a default extension](21-launch-windows-applications-from-a-default-extension.md); [23 - Create and invoke persistent quicklinks](23-create-and-invoke-persistent-quicklinks.md); [24 - Search and open files through a default extension](24-search-and-open-files-through-a-default-extension.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US03, US05, US07, US08, US44. Planned scenarios T01, T03, T09, T22. Gates G7.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Implement the Linux adapters required by the established application/file/quicklink contracts, including actual desktop integration on supported targets.
- [ ] Demonstrate real discovery/launch, file search/open and quicklink actions with cancellation and individually disableable features.
- [ ] Report unavailable desktop functionality with simple reasons while preserving usable actions.
- [ ] Keep platform scope explicit and test natively; do not broaden the support claim to untested distributions/desktops.

## Scope and prerequisites

Reuse provider behavior; no comprehensive desktop-integration framework.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
