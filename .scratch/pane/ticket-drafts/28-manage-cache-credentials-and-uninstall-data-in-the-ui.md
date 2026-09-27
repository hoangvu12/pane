# 28 - Manage cache, credentials and uninstall data in the UI

**What to build:** Users can clear an extension's cache, uninstall it with a durable-data choice, and later remove retained data without its code installed.

**Blocked by:** [09 - Disable extensions and preserve settings across restart](09-disable-extensions-and-preserve-settings-across-restart.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US60, US61, US62, US63, US64. Planned scenarios T20. Gates G5.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Complete managed ownership categories for settings, content, cache and locally managed credentials with a representative fixture for each; define storage/migration ownership and error behavior.
- [ ] Clear cache preserves durable data/credentials; uninstall removes managed code/cache/local credentials and explicitly offers the durable-data choice.
- [ ] Keep data-management UI functional when extension code is absent or broken; protect developer source folders and unrelated documents.
- [ ] Test distinct source identities sharing a title and later reinstall; do not silently merge ownership or claim remote credential revocation.

## Scope and prerequisites

No automatic data-migration rollback, arbitrary external-data deletion or cross-source migration service.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
