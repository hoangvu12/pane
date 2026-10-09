Landed on `main` in PR #273 (merge commit `d5534f450d4db1ee110d08d7c7ed45befb5cd8c1`), which keeps every ticket's commit reachable:

- #183 — `fe2316a87f414a36b5a739977d706b5909f0a9f7`
- #184 — `b0a345f18d9eda064b2a357a1aa3c9c317981a07`
- #185 — `4c4c39b7d15f25b2efea12c1314d95295b38aba1`
- #186 — `8a8723568e67d32b4363adf79b90e99260db4bbc`
- #187 — `79aad9351d260390b6bebfd93141f39ceba4652f`
- review, formatting and the diagnostic-path follow-up — `047ef515`, `86e52687`, `63f1e8ed`

CI: quick tier [37888447203](https://github.com/pane-app/pane/actions/runs/37888447203) and verify run [37889559123](https://github.com/pane-app/pane/actions/runs/37889559123), both green on every leg (Linux and Windows lints and tests, macOS check).

Not included, by the tickets' own rule: benchmark numbers on the user's machine (each ticket says to ask first; the machine is also used for games). The tickets' number rows stay unticked; the guard's CI run over a 20,000-entry generated tree is the only measurement that ran, and its report is in the shard-2 log of any branch-tier run. Native evidence for shares and USB drives is release validation.
