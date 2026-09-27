# Pre-spec context continuity audit

2026-09-28. Historical audit of Pane's launcher-design discussion, requested because the grilling session underwent repeated context compaction. This audit reconciles saved records; it does not recreate unavailable conversation text or authorize implementation. Current status is in the [handoff](HANDOFF.md).

## Conclusion and limits

The saved interview contains entries for **every numbered question Q1-Q41**, along with later unnumbered runtime and recovery corrections. Fourteen ADRs, the detailed policy document, research notes and core prototype artifacts are present. The current visible user instructions agree with the reconciled direction in [current decisions](current-decisions.md).

This supports writing a spec from recorded decisions, with explicit unresolved items. It does **not** establish that every original sentence, rejected alternative or rationale survived compaction. Many early user replies say only "do rec" or "like Pi"; their exact original assistant proposals are not all present in the current conversation. For those, the saved interview/ADRs are the available secondary record. Question-number coverage proves coverage of the record, not lossless conversation retention.

## What was inspected

- Current visible user messages, including WASI3, JS/TS plus Rust, GPUI CE, trust/capability priority, stopping experiments, Q36-Q41, UI recovery and setup choices.
- The complete saved interview, detailed extension policies, handoff, root glossary and all 14 ADRs.
- The aggregate WASI3 validation report and linked Rust std, QuickJS port, P3-only host and GPUI probe descriptions.
- Presence/parsing of key saved JSON results, presence of the integration patch and GPUI screenshots, numbered-question coverage and document links.
- Engineering configuration: local Markdown tracker, default triage roles, single-context docs, both AGENTS.md and CLAUDE.md at the user's request.

Primary research was not repeated against the internet and runtime experiments were not rerun. This is an audit of retained decisions/evidence, not a new compatibility or benchmark result. It does not independently re-audit every historical external source.

The [recorded verification pass](pre-spec-context-audit-checks.json) reported 41 question entries in both the interview and decision index, all 14 ADRs, matching agent entrypoints, and no broken targets/heading anchors among 198 local links across 20 checked documents. Its archive-presence claim is not supported by the current repository: those snapshots are absent. The original check values are retained as a historical report with a correction attached, not a current verification result. Saved prototype results report success; they were read, not re-executed.

## Contradictions corrected

| Risk in the saved documents | Reconciliation |
| --- | --- |
| Handoff/policy bodies still called managed Node, Node workers and native Rust entry points current. | Mark their ADRs historical/superseded as baseline; current direction is WASI3 with WIT/Wasmtime under evaluation and optional native helpers. |
| QuickJS was described as the next backend to test, despite completed patched tests and a later pause. | Record passed bounded tests, provisional engine status and deferred remaining experiments. |
| A lower interview section still called Q36-Q41 unanswered and recommended offline bundling, automatic app downloads, permissive licensing and deferring naming. | Label it historical; current answers are internet-first setup, user-initiated app updates, provisional Zed-style licensing and Pane. |
| CLI-first recovery and unresolved overall license intent remained in the latest-answer list. | Record automatic UI recovery as accepted direction; keep detailed mechanics proposed, and the precise license split open. |
| Handoff combined mutually incompatible phase instructions and outdated "no libraries tested" claims. | Replace with a short current entry point and a linked decision index. The previously reported archive is absent from the current repository. |
| Prototype successes could be mistaken for a complete SDK, guest hot reload or cross-platform support. | Separate requirements from evidence and list each probe's limits. |

## Preserved evidence

Verified these nine files exist; JSON files parse:

- [QuickJS import/execution validation](research/qjs-p3-port-spike/validation-results.json).
- [QuickJS repeated calls and error cases](research/qjs-p3-port-spike/host-results.json).
- [QuickJS measurements](research/qjs-p3-port-spike/measurements.json).
- [Reviewable integration patch](research/qjs-p3-port-spike/runtime-port.patch).
- [Rust std result](research/p3-std-spike/result.json).
- [P3-only host controls](research/p3-only-host/initial-results.json).
- [GPUI recorded results](research/wasi03-gpui-spike/evidence/result.json).
- [Initial GPUI screenshot](research/wasi03-gpui-spike/evidence/initial.png).
- [Replaced-view GPUI screenshot](research/wasi03-gpui-spike/evidence/reloaded.png).

Source scripts, WIT and result notes remain alongside the experiments. Some toolchains, upstream checkouts and executable artifacts live under Windows temporary storage; this audit does not promise those directories will persist. At the time of the audit there was no Git repository/remote. The project has since been committed and published as [hoangvu12/pane](https://github.com/hoangvu12/pane).

## Preservation and reading order

Earlier notes claimed that pre-edit handoff, interview and policy snapshots had been preserved. The referenced archive is absent from the current repository. Do not assume those snapshots can be recovered or recreate them from summaries. The current interview and ADRs retain recorded history, with the limits stated above.

Read [current decisions](current-decisions.md), then [policies](extension-policy-proposal.md), then relevant ADRs/interview/research. The spec has already been produced; use the handoff for the current phase. [Domain configuration](agents/domain.md) points future sessions to these records. Keep unresolved contracts and validation work explicit rather than inferring approval from old recommendations.

## What still needs a decision or validation

See [the explicit open list](current-decisions.md#explicitly-unresolved-or-deferred). It includes final JS backend/maintenance, UI/WIT/IPC contracts, lifecycle/cancellation/recovery, packaging/dependency/update mechanics, API versioning, OS baselines/resource targets and exact licensing. Catalog/history-picker deferrals and later language support remain intentional scope choices, not missing answers.

The main grilling round can remain complete for now without asserting that all technical design is finalized. No missing numbered product answer was identified; no guarantee of a complete verbatim history is possible from these retained records alone.
