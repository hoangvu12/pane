Landed on `main` in PR #274 (merge commit `bff03cdd196dac475850cb5bc2337f0123519818`), which keeps every ticket's commit reachable:

- #191 — `30959c47cc5e41efbafbea60ff5d902da08e2359`
- #189 — `5a0357065c46a7c472d1d0b6c25218910f2413bb`
- #190 — `a8a1e67c6ea4bd69dabe515b7040e4a650a4131e`
- #192 — `9f34c60dd385853201f6ba4fe481a39b9fb2487f`
- review and formatting — `3da35f36`, `d61822af`

The branch was rebuilt onto the main that already carries #129 (PR #271) and #126 (PR #273): the spec/129 content it had carried through its own merge commits was dropped as already upstream, leaving its own ticket commits, each rebased and conflict-resolved against the newer main (#192's `to_json` merges #130's leave-it-out rule from the #129 review with the batching and the sealed count; #190 keeps #212's extension-log field).

CI: quick tier [37896410002](https://github.com/pane-app/pane/actions/runs/37896410002) green; the previously hung `file_actions::rust::root_searchs_file_results_have_the_same_actions` was probed directly ([37897140611](https://github.com/pane-app/pane/actions/runs/37897140611), all four variants pass on Windows in about 3 s); the verify run [37898374301](https://github.com/pane-app/pane/actions/runs/37898374301) is green on Linux and Windows — including Windows shard 3/3, where the earlier integration had hung about 3 h — with the only red the known macOS pasteboard flake #231.

Still open, by the tickets' own rules: the Linux `hidden-idle` before/after numbers (the workload runs in the release matrix's `Smoke (ubuntu-24.04)` after this merge; the pre-merge baseline is in the measurements document as pending), the Windows measurement script's first real run, and the user's-machine baselines — all needing the user's consent. The tickets' number rows stay unticked for that.
