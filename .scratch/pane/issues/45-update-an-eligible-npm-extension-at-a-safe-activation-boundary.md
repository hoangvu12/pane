# 45 - Update an eligible npm extension at a safe activation boundary

**What to build:** A user receives a compatible npm extension update under their chosen update controls, without replacing an active command.

**Blocked by:** [41 - Install and run an npm-distributed component package](41-install-and-run-an-npm-distributed-component-package.md); [43 - Run a scheduled extension task and stop it on disable](43-run-a-scheduled-extension-task-and-stop-it-on-disable.md); [44 - Run a continuing extension service and stop it on disable](44-run-a-continuing-extension-service-and-stop-it-on-disable.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US72, US73, US74, US75; scenarios T15, T08; gates G3, G4. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Provide global/per-extension automatic-update controls and a manual update action; use controlled registry versions to demonstrate availability and selection.
- [ ] Check host/API/platform/dependency compatibility and respect explicit pins, disabled targets and local-source copies.
- [ ] Stage replacement until the documented safe boundary for commands, persistent views and managed background work; the running command must finish without mid-command replacement.
- [ ] Exercise successful activation and replacement-startup failure with Retry/logs and no automatic rollback. Validate controls through the UI and public package/lifecycle behavior.

## Scope

The shared update behavior and one npm source adapter. No Git tracking or history picker.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 41 supplies npm identity, acquisition, pins and compatible runnable package selection.
- 43 supplies real scheduled work whose lifetime an update must preserve.
- 44 supplies real continuing work whose lifetime an update must preserve.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
