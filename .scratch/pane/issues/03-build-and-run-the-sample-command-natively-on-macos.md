# 03 - Build and run the sample command natively on macOS

**What to build:** A macOS contributor builds Pane and the Rust/JS/TS samples locally, then completes the native action/result interaction.

**Blocked by:** [02 - Run JS and TS versions of the native sample command](02-run-js-and-ts-versions-of-the-native-sample-command.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US02, US03, US81; scenarios T04, T05, T22, T25; gates G1, G2, G7. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Use a fresh native macOS checkout with stated OS/architecture prerequisites; build Pane and all sample guests without a Windows machine or mandatory PowerShell.
- [ ] Use the same documented task names as Windows, with native prerequisite/setup differences documented separately. Add native build/contract checks and a reproducible GUI smoke command suitable for the eventual CI runner.
- [ ] Complete native rendering, keyboard/focus and guest action/result checks; record text-input/accessibility findings for the current controls. Cross-compilation and simulated OS flags do not count as native execution.
- [ ] State the tested macOS version and architecture; fix the bounded native boot/process/path assumptions needed for this sample.
- [ ] Record actual native results. Missing runner or GUI access leaves this slice incomplete; there is no requirement for an account or CI provider that has not been configured.

## Scope

One existing sample interaction on one stated native baseline. Full OS integrations and packaging have their own slices.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 02 supplies the launch-language samples and portable build contract to exercise natively.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
