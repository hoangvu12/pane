Landed on `main` in PR #274 (merge commit `bff03cdd`) as `5a0357065c46a7c472d1d0b6c25218910f2413bb`, with the review fixes in `3da35f36`.

Results, per acceptance criterion:

- `summary.json` has `hidden-idle` with CPU share and wake-ups per thread name, and `idle-core` has the wake-ups — `scripts/proc_tree.py` samples every thread (`/proc/<pid>/task/<tid>/stat` and `status` on Linux), and `summary` adds `wakeups` to every phase (`{counted, total, per_second, by_thread}`, the per-thread numbers carrying `threads`, `count`, `per_second` and `cpu_seconds`).
- `scripts/resource-targets.json` lists the new phase with null ceilings (`hidden-idle`'s `rss_kb.max`, `cpu_share`, `wakeups.per_second` and `idle-core.wakeups.per_second`), and `check` reports them as pending and passes.
- `docs/research/resource-measurements.md` describes the phase, the wake-ups-by-thread section, the Windows script and the thread-name truncation on Linux (`pane-runtime-ep`, `pane-runtime-wa`).
- CI: the branch tier is green — quick tier [37896410002](https://github.com/pane-app/pane/actions/runs/37896410002), verify run [37898374301](https://github.com/pane-app/pane/actions/runs/37898374301) (Linux and Windows all green; the only red is the known macOS pasteboard flake #231). The measurement itself now runs in the release matrix's `Smoke (ubuntu-24.04)` after the smoke ([`docs/agents/ci.md`](https://github.com/pane-app/pane/blob/main/docs/agents/ci.md)), so the first CI-held numbers appear in main's next release run.

The two unticked rows:

- **The Windows script has never been run.** `scripts/measure-windows.ps1` is written (its C# sampler block compiles under Windows PowerShell 5.1; the PowerShell parser reports no errors) and runs against a scratch data folder with the same summary shape, but no Windows runner or user's machine has executed it. It needs a run with the user's consent.
- **The baseline is not recorded.** No numbers were taken on the user's machines; the measurements document's Evidence rows remain pending, as the ticket allows ("the code can merge on CI green before the numbers exist" is #183's wording; the same rule applies here).
