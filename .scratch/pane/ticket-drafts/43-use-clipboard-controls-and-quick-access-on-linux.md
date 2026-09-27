# 43 - Use clipboard controls and quick access on Linux

**What to build:** Clipboard and shortcut integration works on the explicitly supported Linux desktop combinations, with honest explanations where OS restrictions prevent it.

**Blocked by:** [25 - Search inside an online command with quick access](25-search-inside-an-online-command-with-quick-access.md); [27 - Expire and delete clipboard history predictably](27-expire-and-delete-clipboard-history-predictably.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US03, US10, US44, US65, US66, US67, US68, US69, US70, US71. Planned scenarios T03, T10, T21, T22. Gates G7.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Implement supported clipboard observation/shortcut routes for the chosen matrix and declare action unavailability for unsupported combinations.
- [ ] Exercise opt-in, pause/disable, retention, deletion and conflict/error paths natively on every claimed combination.
- [ ] Preserve other extension actions when clipboard or global shortcut facilities are unavailable; do not treat a successful X11 case as proof of Wayland support.
- [ ] Keep capture local and document sensitive-marker/app-exclusion limitations without promising universal secret detection.

## Scope and prerequisites

No attempt to support every compositor or bypass OS restrictions.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
