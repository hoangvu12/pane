# Brief for every implementer of spec #121

Spec: https://github.com/pane-app/pane/issues/121 (`gh issue view 121 --repo pane-app/pane`). Your ticket body: `notes-121/ticket-<n>.md` (or `gh issue view <n>`). Decisions: `docs/adr/0036-*.md`, `docs/research/raycast-deep-dive.md` (Extension UI bridge). Codebase map: `notes-121/codebase-map.md` — read it first, it saves you exploration. Repo rules: `AGENTS.md`, `docs/agents/ci.md`, `docs/agents/domain.md`, `CONTEXT.md` (use its vocabulary).

## Absolute rules

- NEVER run cargo, rustc, rustfmt, clippy, xtask, npm/node builds or any toolchain locally — the user games on this machine. All compile/test feedback comes from GitHub Actions.
- Work only in your own worktree folder on your own branch. Never touch other pane-wt folders, the user's checkout (`nguyenvu\pane`) or `pane-wt\_notes`. Never bare `git stash`.
- rustfmt is checked in CI; format by hand carefully (there is NO `rustfmt.toml` — CI runs default rustfmt, whose `fn_call_width`/`chain_width` are 60: calls and chains longer than 60 columns break one item per line with trailing commas).
- Commits: `git -c user.name=hoangvu12 -c user.email=hggaming91@gmail.com commit -s -m "..."`. Never put any other email anywhere.
- Push only your `pi-subagent/121-...` branch: `git push -u origin <branch>`. NEVER push `spec/**` or `main`.

## CI loop

- A plain push runs the quick tier (fmt + `cargo check --workspace --all-targets` on Linux/Windows, ~13 min). Iterate on those. Poll with small commands: `gh run list --repo pane-app/pane --branch <branch> --limit 3`, `gh run view <id> --repo pane-app/pane --log-failed | Select-Object -Last 120`. Never long sleeps; short polls only.
- If JS/TS sample sources, the JS SDK or the WIT changed, the quick tier rebuilds `guests/prebuilt` and uploads artifact `js-guests`: commit it with `gh run download <run> --repo pane-app/pane -n js-guests -D guests/prebuilt` before you finish, or the milestone verify's prebuilt-samples check fails.
- Do NOT run the verify tier on your own branch. The coordinator runs ONE verify run on the milestone branch at the end; that run answers the ticket's CI criterion. To check your new tests while iterating: a chosen-tests run, `gh workflow run ci-fast.yml --repo pane-app/pane --ref <branch> -f tests='test(/name/)'` (this is not the verify tier). Superseded runs auto-cancel. A failure that survives retries: `gh run rerun <id> --repo pane-app/pane --failed` once before diagnosing. The macOS clipboard watcher test failing once (#231) counts as green.
- Done = the quick tier (plain push) is green on your final pushed commit, and any tests you added pass in a chosen-tests run.

## Scope and style

- Implement your ticket fully against its acceptance criteria; tests at the seams the spec names (pane-core `Launcher` with real samples in Rust, JS and TS; window tests in the pane crate). Tests assert observable behaviour, not the JSON wire format.
- Match surrounding code: naming, comment density, idioms. Keep new modules deep and cohesive.
- Make reasonable decisions yourself; list them in your final report.
- Do not edit GitHub issues or PRs, do not tick checkboxes, do not post comments.

## Final report (your last message, short)

Branch name, final commit SHA, CI run URLs and conclusions, a 5–10 line summary of what was built, decisions you made, and anything left undone or that later tickets must know (file paths). Also append that report to `notes-121/done-<ticket>.md` so later implementers can read it.
