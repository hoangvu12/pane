# 27 - Expire and delete clipboard history predictably

**What to build:** Clipboard history has configurable finite retention and deletion controls whose effects remain correct while disabled and after downtime.

**Blocked by:** [26 - Capture clipboard history only when enabled](26-capture-clipboard-history-only-when-enabled.md).

**Status:** draft - awaiting breakdown approval

**Type:** implementation

**Parent:** [Pane specification](../spec.md).

**Spec coverage:** User stories US67, US68, US69. Planned scenarios T10, T21. Gates G5.

## Context

Pane is a small GPUI CE launcher with trusted extensions. JS/TS and Rust are launch languages; extension entry points require pure WASI3. WIT/Wasmtime is the current runtime direction, with QuickJS provisional. These are product requirements and existing Windows probe evidence, not a completed SDK.

Read the [current decisions](../../../docs/current-decisions.md) and the parent spec's relevant stories, implementation decisions and testing scenarios before work. The parent defines all referenced US/T/G identifiers and carries source/ADR links. Preserve its deferred scope and unresolved decisions.

The [cross-platform contributor requirement](../contributor-platform-requirement.md) applies from the early native milestone: shared tooling and behavior must preserve Windows, macOS and Linux contributor baselines. Record native evidence separately from compilation; platform-specific release packaging may finish independently.

## Acceptance criteria

- [ ] Select and document a finite default retention recommendation, obtaining a user decision if it changes unresolved product expectations; implement the configurable policy.
- [ ] Expire records independently of executing the extension and enforce expiry before display after downtime; re-enable does not restart retention.
- [ ] Provide per-item/bulk deletion, Clear history and Disable and delete history with their distinct future-capture effects.
- [ ] Use an injectable clock for expiry scenarios plus a real capture/delete smoke; coordinate writes during deletion to avoid immediately reviving removed history.

## Scope and prerequisites

No forensic erasure promise or deletion of the OS's current clipboard as a side effect of managed-history removal.

Demonstrate the narrow outcome through public behavior and real relevant guests/platform integrations. Use the spec's proposed host-interface checks plus native UI/installer checks where applicable; their organization remains open to user feedback. Introduce supporting internals within this slice rather than creating a broad horizontal foundation project.

A consequential unresolved product choice requires a decision before dependent work. A failed validation or unavailable native test environment does not count as completion. Record the concrete blocker or propose a smaller follow-up rather than expanding this ticket indefinitely.

## Completion and evidence

Record the observed outcome, checks performed, artifact/version/platform scope and remaining limitations. Each acceptance criterion must be satisfied before the ticket is considered done. Labeling a proposal or compiling a binary is not evidence for an untested behavior. Do not execute another ticket or publish a release merely because this slice finishes.

## Comments

2026-09-28: Revised after the user required early support for contributors on all three operating systems. Draft only; no implementation has started.
