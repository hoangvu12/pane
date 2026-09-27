# 01 - Run one Rust command in Pane's native window

**What to build:** A user opens Pane, runs a bundled Rust sample command, selects a result, and sees the guest's response in the native window.

**Blocked by:** None.

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US01, US04, US12, US13, US33, US34, US35, US36, US38; scenarios T01, T04, T05; gates G1, G2. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Build the minimal GPUI CE window and one Rust P3-only fixture from a fresh Windows checkout using documented, portable commands and explicit prerequisites. Show an empty or unavailable-runtime state that leaves core navigation usable. Add its native build/contract check to the repository's automated checks; record GUI smoke evidence separately.
- [ ] Introduce only the host/guest contract and runtime ownership needed for a list, one action and its updated result. A native click or keyboard action must execute real guest code; a JSON file replacement is insufficient.
- [ ] Check the round trip through the highest public host interface plus a native UI smoke; reject a mixed P2/P3 control and show guest/runtime errors without freezing the window.
- [ ] Include the runnable author example and reproduction commands in this slice. Keep common build/process/path code portable; macOS/Linux native proof follows immediately in the early platform slices.

## Scope

One command, one list and one action. No full SDK, package installation, custom drawing or background services. The saved Rust probe supplies the candidate toolchain; record the exact versions actually used.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
