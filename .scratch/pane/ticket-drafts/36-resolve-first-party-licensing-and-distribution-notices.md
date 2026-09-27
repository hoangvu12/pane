# 36 - Resolve first-party licensing and distribution notices

**What to build:** A concrete first-party application/SDK/component license split and dependency-notice plan fit Pane's provisional Zed-style direction.

**Blocked by:** [13 - Invoke and clean up a prebuilt native helper](13-invoke-and-clean-up-a-prebuilt-native-helper.md).

**Status:** draft - awaiting breakdown approval

**Type:** decision

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US82. Planned scenarios . Gates G8.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Audit the actual chosen runtime, renderer, SDK and helper dependencies against primary license sources, distinguishing first-party terms from third-party obligations.
- [ ] Propose exact application/SDK/component terms and attribution ownership; preserve the GPL-3.0-or-later direction without claiming it forbids paid forks.
- [ ] Resolve any consequential remaining license choice with the user before applying terms or marking the decision complete; do not infer that WIT/WASI determines derivative-work questions.
- [ ] Record and validate the notices/source-obligation procedure for a representative artifact; future dependency changes trigger targeted review.

## Scope and prerequisites

No trademark clearance, universal legal opinion or automatic licensing of third-party extensions.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
