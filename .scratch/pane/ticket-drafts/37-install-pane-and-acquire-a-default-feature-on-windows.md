# 37 - Install Pane and acquire a default feature on Windows

**What to build:** A clean Windows machine installs a small Pane bootstrap and automatically acquires the runtime and a runnable calculator/default-feature package.

**Blocked by:** [13 - Invoke and clean up a prebuilt native helper](13-invoke-and-clean-up-a-prebuilt-native-helper.md); [22 - Show calculator answers as an extension feature](22-show-calculator-answers-as-an-extension-feature.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US15, US16, US18, US42. Planned scenarios T23. Gates G6.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Build the end-to-end initial acquisition flow using controlled downloadable artifacts, with progress, cancellation/error and retry behavior documented and visible.
- [ ] Do not bundle runtime/default-feature payloads as a hidden assumption; reuse cached compatible artifacts and preserve core setup/management UI when a download fails.
- [ ] Run without manually installed Node, Rust, npm, Git or compilers, including a supported prebuilt helper fixture; avoid changing global developer-tool settings.
- [ ] Record installed/downloaded artifact ownership and compatibility checks and validate an interrupted/resumed setup on a clean Windows environment.

## Scope and prerequisites

First installer slice supplies a representative default; full default-feature assembly is verified by release tickets. No public upload or signing-account provisioning.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
