# 31 - Install npm-distributed component packages

**What to build:** A user installs a supported npm package through Pane without manually installing npm or Node, then runs its component command.

**Blocked by:** [29 - Install required extension dependencies](29-install-required-extension-dependencies.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US16, US17, US18, US20, US21, US22. Planned scenarios T11, T12. Gates G4, G6.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Acquire supported runnable artifacts from a controlled npm registry and a representative real source where appropriate; surface download/package/compatibility errors.
- [ ] Canonical identity is npm package name without version; explicit version/pin inputs and duplicate-versus-update behavior remain distinct.
- [ ] Resolve required/optional extension dependencies through the existing policy, respecting pins and disabled targets.
- [ ] Do not assume the source distribution implies a Node runtime or universal npm compatibility; source-only build requirements are explained.

## Scope and prerequisites

No catalog, historical-version picker, publishing to npm or arbitrary package build support.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
