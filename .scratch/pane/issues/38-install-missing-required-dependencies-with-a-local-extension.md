# 38 - Install missing required dependencies with a local extension

**What to build:** Installing a local fixture extension shows and installs its compatible missing required dependencies while preserving optional, disabled and pinned choices.

**Blocked by:** [18 - Call an explicit operation in another extension](18-call-an-explicit-operation-in-another-extension.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US25, US26, US27, US28, US29; scenarios T12, T14; gates G4. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Extend the manifest/addressing contract with required versus optional dependencies and compatible version/platform requirements.
- [ ] Show the required set before installing; optional integrations remain absent unless separately requested, and deliberately disabled dependencies are not re-enabled.
- [ ] Explain unavailable/pinned/conflicting dependencies; define bounded cycle and partial-install behavior with no general multi-version solver.
- [ ] Verify the resulting cross-extension call works and failed resolution does not leave misleading ready-to-run state.

## Scope

Controlled/local package sources first; remote acquisition integrations follow without changing these semantics.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 18 supplies the real cross-extension operation used to prove dependency installation.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
