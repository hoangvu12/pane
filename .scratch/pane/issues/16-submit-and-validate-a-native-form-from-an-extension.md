# 16 - Submit and validate a native form from an extension

**What to build:** A native extension form accepts input, validates it, and returns a visible guest result through the same contract in JS/TS and Rust.

**Blocked by:** [03 - Build and run the sample command natively on macOS](03-build-and-run-the-sample-command-natively-on-macos.md); [04 - Build and run the sample command natively on Linux](04-build-and-run-the-sample-command-natively-on-linux.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US36, US38; scenarios T05; gates G2. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Add only the standard controls, focus and event semantics needed for a representative form and validation/error flow.
- [ ] Verify keyboard traversal, text editing and IME on the supported Windows/macOS/Linux contributor baselines through native interaction, with equivalent guest behavior from both language families.
- [ ] Establish and check concrete accessible labeling/focus behavior for the controls used; record unsupported accessibility behavior instead of claiming parity.
- [ ] Document the extension contract and meaningful observable checks, avoiding tests coupled to private GPUI node layouts.
- [ ] Include the runnable form example and author instructions with this feature, rather than deferring them to an onboarding ticket.

## Scope

One text field, one selection and one submit/error flow through JS/TS and Rust. Include the runnable author example; no complete widget library.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 03 establishes the early macOS contributor build/run baseline required before shared feature expansion.
- 04 establishes the early Linux contributor build/run baseline required before shared feature expansion.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
