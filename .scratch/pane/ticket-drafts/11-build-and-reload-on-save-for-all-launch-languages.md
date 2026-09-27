# 11 - Build and reload on save for all launch languages

**What to build:** Saving a JS, TS or Rust development extension rebuilds and reloads that extension, with build diagnostics shown in Pane.

**Blocked by:** [10 - Replace a running extension through manual reload](10-replace-a-running-extension-through-manual-reload.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US19, US51, US53. Planned scenarios T07, T08. Gates G1, G3.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Connect a local development source to the language-appropriate build and the existing replacement path; developer tools may be prerequisites.
- [ ] A failed build preserves the old working guest and reports useful diagnostics; a successful build visibly changes behavior.
- [ ] Handle a newer save arriving during a build without installing an obsolete output over the latest source; document the minimal coalescing/ownership rule.
- [ ] Do not swap or disable a published copy automatically, and apply the same observable lifecycle to JS, TS and Rust.
- [ ] Demonstrate save/build/reload and build-error recovery on Windows, macOS and Linux, using native watchers/process invocation and the same author-facing workflow. Missing native evidence remains explicit.

## Scope and prerequisites

No production auto-updater or arbitrary build-system compatibility promise.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
