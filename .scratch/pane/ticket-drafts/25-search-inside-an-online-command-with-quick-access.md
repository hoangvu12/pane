# 25 - Search inside an online command with quick access

**What to build:** A user reaches an online-search command through an alias, hotkey or fallback, and receives cancellable results inside that command.

**Blocked by:** [12 - Cancel in-flight work during reload and disable](12-cancel-in-flight-work-during-reload-and-disable.md); [20 - Discover and invoke commands through root search](20-discover-and-invoke-commands-through-root-search.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US10, US11, US40. Planned scenarios T03, T09. Gates G2.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Use a controlled HTTP/service fixture through the real guest host-capability path; document supported networking/library behavior rather than assuming Node APIs.
- [ ] Keep service-content requests inside the selected command; root typing does not automatically query every integration.
- [ ] Provide the minimal alias/hotkey/fallback configuration and Windows dispatch needed to demonstrate each access route, with conflicts/unavailability explained.
- [ ] Handle pending-query cancellation and expected service/network errors without automatically pausing the whole extension.

## Scope and prerequisites

No commercial service account dependency, all-platform global-shortcut claim or automatic global online search.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
