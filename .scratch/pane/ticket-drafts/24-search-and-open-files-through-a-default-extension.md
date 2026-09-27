# 24 - Search and open files through a default extension

**What to build:** Pane searches a documented local file scope and opens a selected Windows result, with cancellation as the query changes.

**Blocked by:** [12 - Cancel in-flight work during reload and disable](12-cancel-in-flight-work-during-reload-and-disable.md); [16 - Explain platform-limited extensions and actions](16-explain-platform-limited-extensions-and-actions.md); [20 - Discover and invoke commands through root search](20-discover-and-invoke-commands-through-root-search.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US08, US12. Planned scenarios T01, T03, T09, T22. Gates G3, G7.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Choose and document the initial searchable scope and observation/indexing policy; show real file results through the provider contract.
- [ ] Query changes, view closure and disable cancel managed work so late results cannot replace a newer query.
- [ ] Open a selected file through platform integration and surface access/unavailable errors honestly.
- [ ] Verify disabled behavior and bounded fixtures plus a real Windows open; keep file search independently disableable.

## Scope and prerequisites

No whole-disk indexing guarantee, content-search engine or arbitrary network-filesystem coverage.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
