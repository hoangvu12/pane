# 17 - Choose a color in an extension-owned interactive view

**What to build:** A user adjusts a small color picker, the guest receives the change, and the native view displays the selected value.

**Blocked by:** [16 - Submit and validate a native form from an extension](16-submit-and-validate-a-native-form-from-an-extension.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US37, US38; scenarios T05; gates G2. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Implement the minimal shared drawing/layout and pointer/keyboard event contract needed for this color picker; keep raw renderer/engine objects out of the SDK.
- [ ] Provide equivalent guest examples and public behavior checks for JS/TS and Rust; show focus and an accessible representation of the chosen value.
- [ ] Close or replace the view during interaction and verify that resources and events belong only to the current view.
- [ ] Demonstrate native interaction on the supported contributor baselines and document the example's supported contract.

## Scope

One custom control, with its author example. No graphing framework or general-purpose drawing editor.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 16 supplies native input, submission, focus and guest event handling.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
