# 02 - Run JS and TS versions of the native sample command

**What to build:** An author builds JS and TS samples whose actions update the same native view as the Rust sample.

**Blocked by:** [01 - Run one Rust command in Pane's native window](01-run-one-rust-command-in-pane-s-native-window.md); [P1](../planning-prerequisites.md#p1).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US32, US33, US36, US38, US39, US50; scenarios T04, T05, T06; gates G1, G2. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] After prerequisite P1 supplies a viable initialized JS component, build the JS and TS samples with a documented toolchain and run them through the existing Pane host contract.
- [ ] Use one compatible library in the sample and show a normal validation error; compare guest-visible results and event behavior with Rust without promising Node/npm compatibility.
- [ ] Verify independent fresh instances, the native async path and P3-only imports using P1's regression controls. Keep engine-specific objects out of the author API.
- [ ] Ship the example source, build/run instructions and public contract checks with the bindings; normal sample execution uses prebuilt components.

## Scope

One existing command expressed in both JS and TS; TS transpilation shares the JS runtime. No new UI family or general library-compatibility audit.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 01 supplies the real native host/view/action contract that the JS/TS sample reuses.
- P1 must establish a viable initialized JS backend/toolchain before this UI integration can rely on it.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
