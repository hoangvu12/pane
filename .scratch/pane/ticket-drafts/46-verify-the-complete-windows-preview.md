# 46 - Verify the complete Windows preview

**What to build:** A Windows preview candidate has a complete acceptance record, assembled default features and explicit known limits, ready for a separate release decision.

**Blocked by:** [21 - Launch Windows applications from a default extension](21-launch-windows-applications-from-a-default-extension.md); [23 - Create and invoke persistent quicklinks](23-create-and-invoke-persistent-quicklinks.md); [24 - Search and open files through a default extension](24-search-and-open-files-through-a-default-extension.md); [25 - Search inside an online command with quick access](25-search-inside-an-online-command-with-quick-access.md); [27 - Expire and delete clipboard history predictably](27-expire-and-delete-clipboard-history-predictably.md); [30 - Disable or remove required dependents together](30-disable-or-remove-required-dependents-together.md); [34 - Update eligible extensions without interrupting commands](34-update-eligible-extensions-without-interrupting-commands.md); [35 - Measure Windows resource usage across real lifecycle flows](35-measure-windows-resource-usage-across-real-lifecycle-flows.md); [36 - Resolve first-party licensing and distribution notices](36-resolve-first-party-licensing-and-distribution-notices.md); [38 - Offer Pane updates for user-initiated installation](38-offer-pane-updates-for-user-initiated-installation.md); [39 - Validate author onboarding and distributable examples](39-validate-author-onboarding-and-distributable-examples.md).

**Status:** draft - awaiting breakdown approval

**Type:** validation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US01, US04, US12, US13, US81. Planned scenarios T01, T02, T03, T04, T05, T06, T07, T08, T09, T10, T11, T12, T13, T14, T15, T16, T17, T18, T19, T20, T21, T22, T23, T24, T25. Gates G1, G2, G3, G4, G5, G6, G7, G8.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Run the spec's Windows acceptance matrix against a clean install of the assembled candidate, including all individually disableable defaults and clipboard initially off.
- [ ] Verify lifecycle/recovery, data, dependency and update scenarios through real user/host flows; the representative early calculator-only installer is insufficient.
- [ ] Check current dependency notices, artifact availability and measured resource targets; retain exact build/OS evidence and reproducible failure records.
- [ ] Mark readiness only if applicable criteria pass; unresolved consequential decisions or failures remain blockers and get narrowly scoped follow-up work. Do not publish a release as part of this ticket.

## Scope and prerequisites

Bounded release verification, not a catch-all implementation/fix-everything ticket or macOS/Linux gate.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
