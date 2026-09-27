# 33 - Run scheduled and continuing background work

**What to build:** Enabled fixture extensions can schedule work or run an explicit background service, while unused installations remain inactive and disable stops managed activity.

**Blocked by:** [14 - Pause an identified broken extension through the UI](14-pause-an-identified-broken-extension-through-the-ui.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US14, US45, US57, US79. Planned scenarios T02, T09, T17. Gates G3.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Define the minimal activation declaration and lifetime behavior for one scheduled task and one continuing service, including restart behavior.
- [ ] Demonstrate guest-visible results/status without blocking the UI or permanently activating unrelated installed extensions.
- [ ] Disable/reload cancels supported timers/subscriptions/tasks; expected errors and repeated fatal failures follow the existing pause/retry policy.
- [ ] Record schedule defaults, duplicate-activation prevention and idle-retention choices explicitly; do not infer numeric policies from prototypes.

## Scope and prerequisites

No general workflow scheduler or guarantee of undoing external effects.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
