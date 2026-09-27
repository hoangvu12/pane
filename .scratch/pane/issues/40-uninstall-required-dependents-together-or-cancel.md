# 40 - Uninstall required dependents together or cancel

**What to build:** A user removing a required dependency reviews the affected set and its saved-data choices before uninstalling.

**Blocked by:** [39 - Disable required dependents together or cancel](39-disable-required-dependents-together-or-cancel.md); [36 - Uninstall an extension with an explicit saved-data choice](36-uninstall-an-extension-with-an-explicit-saved-data-choice.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US30, US31, US61, US63; scenarios T13, T20; gates G4, G5. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Reuse required-dependent traversal but present Uninstall All/Cancel and the applicable durable-data choices.
- [ ] Cancel before mutation preserves installations and data; confirmation removes only the displayed required set with the established uninstall semantics.
- [ ] Report partial removal failures against actual package states rather than claiming an all-or-nothing result that did not occur.
- [ ] Reinstall the dependency alone and verify removed dependents are not restored automatically; optional integrations remain independent.

## Scope

One dependent-removal operation using existing package/data behavior.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 39 supplies required-dependent traversal and the displayed affected set.
- 36 supplies guest-independent removal and the durable-data choice.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
