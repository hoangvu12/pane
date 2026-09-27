# 42 - Install and run a Git-distributed component package

**What to build:** A user installs a supported Git-sourced package or explicit revision through Pane without manual Git/compiler setup.

**Blocked by:** [38 - Install missing required dependencies with a local extension](38-install-missing-required-dependencies-with-a-local-extension.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US16, US17, US18, US20, US21, US22, US23, US24; scenarios T11, T12; gates G4, G6. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Define and exercise the supported Git artifact/source layout and acquisition path; distinguish runnable releases from source-only revisions.
- [ ] Canonicalize equivalent host/repository transport forms without including ref in identity; cross-source copies remain independent.
- [ ] Support the selected explicit revision/ref inputs, define tracked-ref versus pin semantics, and explain unavailable artifacts or compatibility failures.
- [ ] Integrate dependency handling and preserve user-owned local development copies; do not infer Pi personal/project scopes or automatic source merging.
- [ ] Include a source-to-artifact author example and controlled repository fixture; a documented source-only failure must not be presented as successful normal-user installation.

## Scope

No guarantee every tag is installable, no history browser, and no external repository publishing.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 38 supplies required-versus-optional dependency relationships and conflict/cycle policy.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
