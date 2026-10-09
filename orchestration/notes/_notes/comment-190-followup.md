Follow-up: the Linux run the last criterion asks for has now happened — in CI, on `main` after PR #274 landed. The release matrix's `Smoke (ubuntu-24.04)` runs the resource workload ([37907199550](https://github.com/pane-app/pane/actions/runs/37907199550), commit `26c8e224`), and its `hidden-idle` phase (60 s sampled, defaults recorded, 30 s settled) shows, per thread name:

| Thread | Wake-ups/s | CPU s |
|---|---|---|
| `pane-runtime-ep` (the epoch ticker) | **0.0** | 0.0 |
| `pane-runtime-wa` (the watchdog) | **0.0** | 0.0 |
| all of Pane, whole phase | 3.99 total | — |

The only threads that woke at all were `blocking-1` and `pane-file-watch` at 2.0/s each; the runtime timers, the extension threads, the file index and every other named thread sat at zero — the condvar sleep (#190) holding while nothing runs. For comparison, `idle-core` (the window open) shows 64.5 wake-ups/s, mostly the renderer's unnamed threads.

The row stays unticked because what remains is the part that needs the user: the Windows run (only with consent, as in #189), the before/after pair against a pre-#190 build on the user's machine, and recording the numbers in `docs/research/resource-measurements.md`'s Evidence section, whose rows are still pending. The `resource-targets.json` ceilings for the new phase remain null until those baselines exist.
