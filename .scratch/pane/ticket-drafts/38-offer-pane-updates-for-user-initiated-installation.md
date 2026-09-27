# 38 - Offer Pane updates for user-initiated installation

**What to build:** Pane announces an available application update and installs it only after the user chooses, with a clear outcome if acquisition or replacement fails.

**Blocked by:** [37 - Install Pane and acquire a default feature on Windows](37-install-pane-and-acquire-a-default-feature-on-windows.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US76. Planned scenarios T16, T23. Gates G6.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Use controlled update metadata/artifacts to demonstrate availability notification, the install action and version transition.
- [ ] Do not automatically download/install/restart the application from the notification; distinguish this policy from automatic extension updates.
- [ ] Define installed-runtime compatibility and app/runtime acquisition coordination without silently changing user extension pins or enablement.
- [ ] Verify no-action behavior and a failed update path leave an explainable recoverable installation; document restart timing and active-work handling.

## Scope and prerequisites

No forced upgrades, public release publishing or historical-version browser.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
