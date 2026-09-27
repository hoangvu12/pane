# 07 - Keep a working contributor workflow on all three operating systems

**What to build:** Contributors on Windows, macOS and Linux have a documented build/run/test workflow backed by native automated checks before the shared extension system grows.

**Blocked by:** [05 - Establish the macOS contributor baseline early](05-establish-the-macos-contributor-baseline-early.md); [06 - Establish the Linux contributor baseline early](06-establish-the-linux-contributor-baseline-early.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US02, US03, US19, US32, US33, US38, US81. Planned scenarios T04, T05, T25. Gates G1, G2, G7.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Run the same documented developer workflow on fresh native Windows/macOS/Linux environments to build Pane, build JS/TS/Rust guests, launch the sample interaction and run relevant checks; document OS-specific dependencies separately.
- [ ] Provide portable task entry points and an automated native OS matrix suitable for the eventual CI provider, with equivalent local commands. CI service choice and available runners are not assumed.
- [ ] Separate headless compile/contract checks from native GUI smoke results. Missing runners or failed native runs remain visible blockers to the contributor baseline; cross-compiling is not runtime proof.
- [ ] Define the change policy: common SDK/runtime/UI/build changes preserve all three contributor baselines, while OS adapters stay explicit. No hardcoded developer paths or mandatory Windows shell in shared tooling.
- [ ] Keep the scope to contributor build/run/testing. Full default-feature parity, native installers and independent preview releases follow in later tickets.

## Scope and prerequisites

Requires access to actual native test environments/runners for all three OS families. This adds no unapproved external CI account, publication, or infrastructure provisioning.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
