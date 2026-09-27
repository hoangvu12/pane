# 47 - Verify the complete macOS preview

**What to build:** A macOS preview candidate has its own complete native acceptance/resource record and stated support limits, independently of Windows release timing.

**Blocked by:** [30 - Disable or remove required dependents together](30-disable-or-remove-required-dependents-together.md); [34 - Update eligible extensions without interrupting commands](34-update-eligible-extensions-without-interrupting-commands.md); [35 - Measure Windows resource usage across real lifecycle flows](35-measure-windows-resource-usage-across-real-lifecycle-flows.md); [36 - Resolve first-party licensing and distribution notices](36-resolve-first-party-licensing-and-distribution-notices.md); [39 - Validate author onboarding and distributable examples](39-validate-author-onboarding-and-distributable-examples.md); [44 - Install and update Pane on a clean macOS machine](44-install-and-update-pane-on-a-clean-macos-machine.md).

**Status:** draft - awaiting breakdown approval

**Type:** validation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US02, US03, US81. Planned scenarios T01, T02, T03, T04, T05, T06, T07, T08, T09, T10, T11, T12, T13, T14, T15, T16, T17, T18, T19, T20, T21, T22, T23, T24, T25. Gates G1, G2, G3, G4, G5, G6, G7, G8.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Execute the applicable full acceptance matrix on the claimed clean macOS baseline with all default features assembled and supported native helper artifacts.
- [ ] Verify standard/custom views, input/IME/accessibility, lifecycle/recovery and package/data/update behavior natively; reuse test contracts, not Windows evidence.
- [ ] Measure the macOS process tree and confirm platform-specific targets/notices/distribution prerequisites; report untested combinations explicitly.
- [ ] Keep failures or missing environments/credentials as blockers and propose bounded follow-up fixes; do not publish or declare readiness from a compile-only result.

## Scope and prerequisites

Release verification on the selected baseline, not open-ended porting or a dependency on the Windows preview approval.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
