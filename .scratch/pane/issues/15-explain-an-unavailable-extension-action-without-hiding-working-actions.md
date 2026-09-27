# 15 - Explain an unavailable extension action without hiding working actions

**What to build:** Pane displays supported-OS information and explains unavailable actions while leaving supported actions usable.

**Blocked by:** [05 - Install and run a local extension package](05-install-and-run-a-local-extension-package.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US03, US18, US43, US44, US81; scenarios T22; gates G4, G7. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Add simple supported-OS/artifact metadata and an action availability result with a user-facing reason, without a general rule language.
- [ ] Demonstrate an extension with working and unavailable actions and a package with no compatible artifact.
- [ ] Keep installation/execution compatibility behavior consistent with the manifest and avoid silently successful no-op actions.
- [ ] Check actual platform results on the declared contributor baselines; package target declarations are not evidence of native support.
- [ ] Check the supported and unavailable action paths through the public host interface and a native UI smoke.

## Scope

No automatic porting, platform spoofing as proof, or full release support matrix yet.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 05 supplies installed package identity, compatible artifacts and a real invocable guest.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
