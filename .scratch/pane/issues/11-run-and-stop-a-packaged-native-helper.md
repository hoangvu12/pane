# 11 - Run and stop a packaged native helper

**What to build:** A component command invokes an OS-matched prebuilt helper, shows its result, and stops the managed process on cancel, disable or reload.

**Blocked by:** [10 - Discard late guest results after reload or disable](10-discard-late-guest-results-after-reload-or-disable.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US40, US41, US42, US64; scenarios T09, T22; gates G2, G3. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Add the narrow host API and package declaration needed to select and launch a prebuilt helper with managed input/output, completion and errors.
- [ ] Provide and run a prebuilt helper fixture for each supported contributor OS/architecture baseline; no compilation occurs on the end-user path, and absent/wrong-target artifacts are explained.
- [ ] Cancel/disable/reload while the helper runs and verify cleanup of supported managed processes/handles, keeping saved data intact.
- [ ] Keep the extension entry point WASI3 and document detached-descendant limits; do not promise arbitrary process containment.
- [ ] Include one helper-backed command and its author/package instructions; check the host's public result/error and cleanup behavior, with real helper processes on each supported baseline.

## Scope

One echo or bounded file-processing helper. Supported OS/architecture artifacts are explicit; no arbitrary library compatibility claim.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 10 supplies pending-work cancellation and ownership of late generation results.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
