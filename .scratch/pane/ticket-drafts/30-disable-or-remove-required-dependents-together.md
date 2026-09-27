# 30 - Disable or remove required dependents together

**What to build:** Disabling or uninstalling a dependency shows affected required dependents and applies the confirmed whole-set action or cancels cleanly.

**Blocked by:** [28 - Manage cache, credentials and uninstall data in the UI](28-manage-cache-credentials-and-uninstall-data-in-the-ui.md); [29 - Install required extension dependencies](29-install-required-extension-dependencies.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US30, US31. Planned scenarios T13, T20. Gates G4, G5.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Compute the required dependent closure and show Disable All/Cancel or Uninstall All/Cancel; optional integrations do not enter the required set.
- [ ] Cancel preserves state; confirmed operations apply lifecycle and data-retention choices consistently to the displayed set.
- [ ] Re-enabling or reinstalling only the dependency does not re-enable/reinstall dependents automatically.
- [ ] Keep runtime failure-pausing separate from user-requested removal, and test cycles/shared dependencies according to the bounded policy.

## Scope and prerequisites

No automatic repair, migration or silent dependent restoration.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
