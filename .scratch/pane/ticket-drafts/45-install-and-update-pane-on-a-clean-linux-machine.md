# 45 - Install and update Pane on a clean Linux machine

**What to build:** A package for the selected Linux baseline acquires runtime/default artifacts without developer tools and offers user-controlled app updates.

**Blocked by:** [38 - Offer Pane updates for user-initiated installation](38-offer-pane-updates-for-user-initiated-installation.md); [42 - Use application, file and quicklink actions on Linux](42-use-application-file-and-quicklink-actions-on-linux.md); [43 - Use clipboard controls and quick access on Linux](43-use-clipboard-controls-and-quick-access-on-linux.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US02, US15, US16, US42, US76, US81. Planned scenarios T16, T23, T25. Gates G6, G7.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Choose and document a bounded initial Linux packaging target consistent with the validated desktop/runtime baseline.
- [ ] On a clean target environment, install and invoke default features and the helper fixture without manual programming runtimes/package managers/compilers.
- [ ] Verify acquisition retry/cache and user-initiated app-update success/failure behavior without overwriting the user's development environment.
- [ ] State packaging/OS prerequisites and unsupported targets clearly; do not claim every Linux packaging ecosystem is covered.

## Scope and prerequisites

No distribution repository publishing or universal distro/desktop support.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
