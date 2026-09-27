# 39 - Validate author onboarding and distributable examples

**What to build:** A contributor on Windows, macOS or Linux can independently build Pane and JS/TS/Rust examples, run checks, use hot reload and prepare supported packages from a fresh checkout.

**Blocked by:** [11 - Build and reload on save for all launch languages](11-build-and-reload-on-save-for-all-launch-languages.md); [13 - Invoke and clean up a prebuilt native helper](13-invoke-and-clean-up-a-prebuilt-native-helper.md); [18 - Drive a custom interactive view from an extension](18-drive-a-custom-interactive-view-from-an-extension.md); [31 - Install npm-distributed component packages](31-install-npm-distributed-component-packages.md); [32 - Install Git-distributed component packages](32-install-git-distributed-component-packages.md); [33 - Run scheduled and continuing background work](33-run-scheduled-and-continuing-background-work.md).

**Status:** draft - awaiting breakdown approval

**Type:** validation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US19, US32, US33, US39, US40, US41, US42, US46, US50, US51. Planned scenarios T04, T05, T07, T11, T14, T22. Gates G1, G2, G4.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Use fresh author workspaces on each supported Windows/macOS/Linux contributor baseline with documented OS prerequisites; build Pane and JS/TS/Rust examples without relying on a Windows machine, a developer's temporary checkout or undocumented global tools.
- [ ] Cover a standard view, one custom interaction, an explicit cross-extension operation and optional prebuilt helper through a small shared example set; reference implemented background behavior.
- [ ] Demonstrate local development and installation from controlled npm/Git artifacts while preserving independent source identities and compatibility metadata.
- [ ] Document supported library limits, API evolution responsibilities and package publication format without actually publishing packages or promising abandoned-extension maintenance.
- [ ] Keep a common documented build/test/run/development workflow and native automated checks on all three OS families; report provider/runner availability honestly and retain equivalent local commands.

## Scope and prerequisites

Documentation/examples and author-path verification; missing functionality is a concrete follow-up, not an invitation to invent another SDK.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
