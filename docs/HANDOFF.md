# Pane handoff

Updated 2026-09-28 after revising the ticket breakdown for early cross-platform contributors.

## Start here

1. Read [current decisions](current-decisions.md) for the reconciled Q1-Q41 index, runtime status and open work.
2. Read [extension policies](extension-policy-proposal.md) for detailed accepted behavior.
3. Use [the interview](launcher-design-interview.md) and relevant [ADRs](adr/) for rationale and history. Earlier proposals inside the interview are historical, not competing current instructions.
4. Use [the audit](pre-spec-context-audit.md) for what was checked and what cannot be recovered from a summary.

The [Pane specification](../.scratch/pane/spec.md) is published with `Status: ready-for-agent`: 82 user stories, eight engineering gates and 25 planned acceptance scenarios. Its proposed testing boundary remains a recommendation pending feedback. The user then invoked `/to-tickets`; the revised [48-ticket breakdown](../.scratch/pane/ticket-breakdown.md) and individual drafts are prepared for review. The user's [cross-platform contributor clarification](../.scratch/pane/contributor-platform-requirement.md) is an acceptance addendum: native macOS/Linux build-and-run gates (05-06) and the common contributor workflow (07) precede broad shared feature development. Publication as ready-for-agent issues awaits approval required by that skill. The parent spec is unchanged; no production implementation or new runtime tests have been performed.

## Current direction

- **Pane** is the chosen product name; the existing workspace is still named `kyoko`.
- Small, general-purpose, extensible launcher targeting Windows/macOS/Linux, using **GPUI CE**. Contributors on all three systems need working native build/run/test workflows early. Only Windows prototypes have been tested; staggered previews remain accepted, with the early contributor baseline required before shared feature expansion.
- Trusted extensions, capability priority, own SDK and npm/Git/local distribution. JS/TS **and Rust** at launch; Python/C# later.
- **WASI 0.3 required.** WIT/Wasmtime is the current architecture under evaluation. Earlier Node workers/native Rust entry-point choices are superseded as the implementation baseline; optional prebuilt native helpers remain accepted.
- **QuickJS is provisional**, with temporary integration patches, unresolved snapshot random-state initialization, and no production backend commitment.
- Default app launching, calculator, quicklinks, file search and optional clipboard history are disableable. AI belongs in extensions.
- Internet-first runtime/default-feature acquisition is preferred over bundling their payloads in the installer. Ordinary supported-package users install no runtime/compiler manually.
- Compatible extension updates are automatic with controls; Pane application updates notify the user, who chooses installation.
- Automatic UI recovery should skip an identified broken extension and notify the user. Detailed attribution/pausing/retry is proposed, not validated; shared runtime crashes may have no identifiable single culprit.
- Licensing follows Zed's primarily GPL-3.0-or-later direction provisionally; exact component/SDK split and dependency review remain open. This does not prohibit compliant paid forks.

Detailed identity, dependency, disable/data, API stability, search, reload and update decisions are in the [decision index](current-decisions.md). Do not replace them with generic assumptions about Pi, Raycast or VS Code.

## Evidence and limits

The [validation checkpoint](research/wasi03-validation.md) links the Rust, JS and GPUI experiments.

- [Rust std](research/p3-std-spike/README.md): P3-only filesystem/environment/clock execution with pinned source-built toolchain.
- [QuickJS port](research/qjs-p3-port-spike/README.md): real libraries, async streams and 20 sequential calls pass; source patch and raw results saved.
- [Embedded host](research/p3-only-host/README.md): P3-only registration; stock mixed JS rejected.
- [GPUI Windows probe](research/wasi03-gpui-spike/README.md): guest JSON rendering, JSON replacement and clicks. Not live guest hot reload or production IPC.

Remaining validation includes runtime reseeding, cancellation/concurrency, reload/disable cleanup, persistent resource measurements, host/UI integration and macOS/Linux. Benchmark numbers are scoped CLI/prototype measurements, not product budgets.

Tool locations and scratch paths are recorded with the probes, including [P3 tool metadata](research/p3-native-toolchain-local.json). Some compiler/toolchain/checkouts and built binaries live under Windows temporary storage. Their presence is not a durable source-control guarantee. Repository source scripts, reports and the patch are the recovery path; runtime/toolchain installation must not silently alter the user's global Rust default.

## Workflow configuration

Both [AGENTS.md](../AGENTS.md) and [CLAUDE.md](../CLAUDE.md) are present by explicit user choice. Shared configuration:

- [Local Markdown issue tracker](agents/issue-tracker.md).
- [Default triage labels](agents/triage-labels.md).
- [Single-context domain documentation](agents/domain.md).

Next: obtain approval or edits to the drafted size/dependencies, then publish one file per approved ticket to the local issues directory. The initial proposed frontier is 01 (runtime validation) and 02 (portable native core). Per-ticket implementation is a subsequent user-invoked phase. Windows preview 46 depends on the early macOS/Linux contributor baseline, but not later platform feature/installer/release work (40-45, 47-48). These are branches in the dependency graph, not separate source forks. Real native test environments remain prerequisites. Unresolved gates remain explicit prerequisites; drafting or publication does not authorize external releases or silently settle engine/licensing choices.

## Preservation

The older, contradictory handoff and the pre-audit interview/policy files are preserved in [the audit archive](archive/pre-spec-audit-2026-09-28/). That archive is historical evidence; its relative links retain their original locations and its contents are not current instructions.
