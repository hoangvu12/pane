# 27 - Invoke a command through an alias or explicit fallback

**What to build:** A user configures an alias or fallback for an installed command and invokes it from root search.

**Blocked by:** [26 - Search an online service inside its command](26-search-an-online-service-inside-its-command.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US10, US11; scenarios T03; gates G2. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Persist a minimal alias and fallback setting under command/source identity; show conflicts or unavailable targets in configuration.
- [ ] Demonstrate both alias matching and explicit fallback selection, forwarding the query to the command only after invocation.
- [ ] Disabling the target removes usable bindings without silently re-enabling it; unrelated sources with the same title remain distinct.
- [ ] Verify configuration-to-invocation and disabled/missing-target cases through the host/UI path.

## Scope

Root-search routing only; global OS shortcut registration is separate.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 26 supplies a real query-taking command for alias and fallback invocation.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
