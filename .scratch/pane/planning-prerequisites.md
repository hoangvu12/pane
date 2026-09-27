# Pane planning prerequisites

These are unresolved decisions or bounded feasibility checks, separate from the implementation slices in the [breakdown](ticket-breakdown.md). They are not published ready-for-agent feature tickets, and writing them does not mark the work complete. The [parent spec](spec.md) remains unchanged.

## P1

**Question:** Which maintainable JS component backend and author toolchain can supply the required correctly initialized P3-only sample?

**Blocks:** [02 - Run JS and TS versions of the native sample command](issues/02-run-js-and-ts-versions-of-the-native-sample-command.md). The first Rust/native slice can proceed independently using the saved Rust feasibility evidence.

**Inputs:** [QuickJS executable checkpoint](../../docs/research/qjs-p3-port-spike/README.md), [runtime alternatives](../../docs/research/js-p3-backend-options.md), [Rust std checkpoint](../../docs/research/p3-std-spike/README.md), and [current runtime decisions](../../docs/current-decisions.md#runtime-direction-and-evidence).

**Bounded work:** Reproduce the saved fresh-instance random-state defect and prove correct initialization for the candidate; rerun representative library/async/filesystem positive cases and a rejecting mixed P2/P3 control. Record the exact reproducible JS/TS and Rust build inputs, artifact/initialization costs, native host/toolchain availability and remaining library limitations. Document the maintenance/upstream strategy. This is a focused feasibility decision using existing probes, not a mandate to implement an open-ended engine fork.

**Exit:** A reproducible candidate and regression evidence suitable for the native sample. If the candidate cannot pass, record the concrete blocker and compare bounded alternatives before choosing a replacement. Do not silently substitute WASI2 or managed Node. A consequential backend-maintenance commitment still requires the user's decision; ordinary pinned-version and implementation details do not require a new approval ritual. QuickJS is not predetermined.

**Coverage:** US32, US34, US35, US39, US50; T04, T06; G1. Current status: unresolved; no new execution in this revision.

## P2

**Question:** What exact first-party application/SDK/component terms and distribution notices implement the provisional Zed-style direction?

**Blocks:** The release/distribution decision for each platform, not local implementation or controlled installer tests.

**Bounded work:** Using the actual selected dependencies, audit primary license sources; propose the precise application/SDK/component split, ownership of notices and source-obligation procedure for representative artifacts. GPL-3.0-or-later is provisional and does not prohibit compliant paid forks. Do not infer legal conclusions from using WIT/WASI alone.

**Exit:** The remaining consequential choice is resolved with the user, exact terms and notices are applied to their intended components, and the representative distribution procedure is checked. Changes to dependencies require a targeted notice/obligation recheck. No licensing change is made by this planning document.

**Coverage:** US82; G8. Current status: unresolved.

## P3

**Question:** Which measured resource/latency targets define an acceptable preview on each supported baseline?

**Blocks:** A performance/readiness claim for that platform. It does not block a different platform merely because Windows measurements are unfinished.

**Bounded work:** Reuse a single documented workload and collect full-process-tree measurements for cold/warm start, idle core, installed-but-unused extensions, active commands, continuing work and repeated reload/disable. Record build settings and native OS/architecture. Propose targets from these measurements and record the decision; no numeric budget is implied by the older CLI peaks.

**Exit:** Evidence, targets and known limits are recorded for the platform being claimed. Run its check in [release validation](release-validation.md); failures become narrow corrective work, not an unlimited optimization ticket.

**Coverage:** US04, US14, US81; T02, T24, T25; G7. Current status: no production measurements or targets yet.

## Existing testing-boundary proposal

The spec proposes a high public host interface with real guests, supplemented by native UI and installer checks. Its user-feedback check is still open; this revision does not claim approval. Feature criteria describe observable behavior and do not prescribe private renderer/engine layouts. See the [spec's testing-boundary note](spec.md#testing-boundary-check).
