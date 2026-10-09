Landed on `main` in PR #273 (merge commit `d5534f45`) as `fe2316a87f414a36b5a739977d706b5909f0a9f7`, with the review fixes in `047ef515` and `86e52687`.

Results, per acceptance criterion:

- The benchmark reports misses, typed-out prefixes, the kind filters and inside-word fragments as their own rows (11 kinds, per-kind p50/p95/p99 on both shapes plus an "all kinds" row covering every query) — `crates/pane-core/examples/file_index_bench.rs`; the query set is sorted and seeded so it is fixed between runs.
- Both index shapes are timed and reported: one segment (first pass after opening | warm) and several segments with changes in memory (first pass after the stream | warm), with the actual segment count and change count printed under the table.
- The re-index rows cover a deep path under ignore files, both with the folders above kept and (the review pass's addition) with nothing kept, and both are guarded.
- The reduced run is in CI with ceilings and `docs/agents/ci.md` says so: `cargo xtask file-index-guard` runs in `ci-branch.yml`'s Linux shard 2 after the tests (dev profile over a generated 20,000-entry tree; ceilings 180 s first index, 400 B/entry, 2.5 s query and re-index p95s).
- CI: the quick tier ([37888447203](https://github.com/pane-app/pane/actions/runs/37888447203)) and the verify run ([37889559123](https://github.com/pane-app/pane/actions/runs/37889559123)) are green on every leg, including the Windows file-index tests.

The one unticked row is the user's-machine baseline: no numbers were taken (the ticket says to ask first, and the machine is also used for games). The CI guard's log holds the dev-profile report on a shared runner; the release-profile numbers on the user's machine remain to be recorded, into `docs/evidence/files-174`, whenever the user agrees to run the benchmark.
