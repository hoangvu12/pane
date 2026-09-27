# 44 - Install and update Pane on a clean macOS machine

**What to build:** A macOS package acquires runtime/default artifacts without developer tools and offers app updates under the user-initiated policy.

**Blocked by:** [38 - Offer Pane updates for user-initiated installation](38-offer-pane-updates-for-user-initiated-installation.md); [40 - Use application, file and quicklink actions on macOS](40-use-application-file-and-quicklink-actions-on-macos.md); [41 - Use clipboard controls and quick access on macOS](41-use-clipboard-controls-and-quick-access-on-macos.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US02, US15, US16, US42, US76, US81. Planned scenarios T16, T23, T25. Gates G6, G7.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Adapt the proven bootstrap/acquisition/update flow to the chosen macOS packaging baseline and architecture-specific artifacts.
- [ ] On a clean macOS environment, install/run defaults plus the supported helper fixture without Node/Rust/npm/Git/compilers.
- [ ] Verify retry/cache behavior and an application update's no-action/install/failure cases, including core availability when runtime acquisition fails.
- [ ] Record signing/distribution prerequisites honestly; lack of required credentials or artifacts is a release blocker rather than simulated completion.

## Scope and prerequisites

No external publication or credential/account setup without the necessary authorization; release readiness remains separate.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
