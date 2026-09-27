# Pane handoff

Updated 2026-09-28. This handoff covers Pane, the cross-platform launcher and its extension system.

The project is published at [hoangvu12/pane](https://github.com/hoangvu12/pane). The repository contains planning documents and research prototypes; there is no installable Pane application yet.

## Start here

1. Read [current decisions](current-decisions.md) for the reconciled Q1-Q41 index, runtime status and open work.
2. Read [extension policies](extension-policy-proposal.md) for detailed accepted behavior.
3. Use [the interview](launcher-design-interview.md) and relevant [ADRs](adr/) for rationale and history. Earlier proposals inside the interview are historical, not competing current instructions.
4. See [record limits](current-decisions.md#record-limits) for the limits of the saved discussion and evidence.

The [Pane specification](../.scratch/pane/spec.md) is published with `Status: ready-for-agent`: 82 user stories, eight engineering gates and 25 planned acceptance scenarios. Its proposed testing boundary remains a recommendation pending feedback. Following the user's criticism of the earlier slicing, [the 52 published implementation issues](../.scratch/pane/issues/) form revision 3, with [planning prerequisites](../.scratch/pane/planning-prerequisites.md) and [release validation](../.scratch/pane/release-validation.md) tracked separately. The [cross-platform contributor requirement](../.scratch/pane/contributor-platform-requirement.md) remains accepted: native macOS/Linux sample builds and runs (03-04), including contributor commands and native checks, precede broad shared features. The user requested correction of the tracker structure; all 52 issues now live in `.scratch/pane/issues/` with `ready-for-agent` status and their existing blockers. The parent spec is unchanged; no production implementation or new runtime tests have been performed.

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

Next: work published issues whose numbered blockers and linked planning prerequisites are complete. `ready-for-agent` records specification readiness, not completion of blockers. The first implementation frontier is 01, one Rust command completing a real native action/result interaction. P1 JS feasibility can be resolved independently and blocks JS/TS slice 02. Early native contributor checks follow in 03-04. P2 licensing and P3 measured-target work gate corresponding distribution/readiness claims, not arbitrary unrelated feature work. Platform installers and updaters have independent dependency branches; release-wide evidence is a checklist rather than a large implementation ticket. Real native environments remain execution prerequisites. Creating or publishing drafts does not settle unresolved choices or authorize a release.

## Historical records

The interview, research reports and superseded ADRs retain Pane's design history. Use the current decision index for the accepted direction. Earlier notes referenced pre-audit snapshots, but those snapshots are absent from this repository; do not rely on them as recoverable evidence.
