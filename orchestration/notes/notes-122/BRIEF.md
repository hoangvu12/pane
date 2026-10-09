# Implementer brief — spec #122 (Root search like Raycast), pane-app/pane

You implement ONE ticket of spec #122 in your own git worktree, on your own branch. Pointers, not copies:

- Spec: `gh issue view 122 --repo pane-app/pane`. Your ticket: `gh issue view <n> --repo pane-app/pane`.
  All ticket bodies are also saved in `C:\Users\ADMIN\Desktop\nguyenvu\pane-wt\notes-122\tickets.md`.
- Repo docs (in your worktree): AGENTS.md, docs/agents/ci.md, docs/agents/domain.md, CONTEXT.md (glossary),
  docs/adr/0030-*.md, docs/adr/0031-*.md, docs/research/raycast-deep-dive.md (root search sections), docs/root-search.md.
- Notes from earlier implementers: `notes-122/*.md` (read the ones relevant to your area first).
  Before you finish, write `notes-122/<ticket>-notes.md`: a SHORT map (≤40 lines) of the files/types/seams you
  touched or learned, for later implementers. No prose about process.

## Absolute rules

- NEVER run cargo, rustc, rustfmt, xtask, nextest or any Rust toolchain locally (the user games on this machine).
  Do not run npm builds of the guests either. ALL compile/test feedback comes from GitHub Actions.
- rustfmt is checked in CI (`cargo fmt --all --check`): format by hand, carefully, in rustfmt's default style with
  this repo's rustfmt.toml (fn_call_width 60, chain_width 60). Read rustfmt.toml. Match the surrounding code.
- Stay inside your worktree. Never touch `C:\Users\ADMIN\Desktop\nguyenvu\pane`, other worktrees, or `pane-wt\_notes`.
  Never bare `git stash`.
- Commit: `git -c user.name=hoangvu12 -c user.email=hggaming91@gmail.com commit -s -m "..."`.
  Never put any other email in a request to an outside service.
- Push only your own branch (`pi-subagent/122-...`): `git push -u origin HEAD`. Never push `spec/**` or `main`.
- Do not merge anything, do not open PRs, do not comment on, tick or close issues.

## CI loop (see docs/agents/ci.md)

- A plain push runs ci-fast's quick tier (fmt + `cargo check --all-targets`, Linux+Windows, ~13 min). Iterate on those.
  Poll with small commands: `gh run list --repo pane-app/pane --branch <branch> --limit 3`, then
  `gh run view <id> --repo pane-app/pane --log-failed | Select-Object -Last 80` (or tail) for failures. No long sleeps:
  use short waits (e.g. `Start-Sleep 240`) between polls at most.
- When you believe the branch is done (quick tier green), run ONE verify:
  `gh workflow run ci-fast.yml --repo pane-app/pane --ref <branch> -f verify=true`.
  Recheck single tests with `-f verify=false -f tests='test(/name/)'`. Superseded runs auto-cancel.
- If you changed WIT/the SDKs/JS samples so `guests/prebuilt` goes stale, the quick tier uploads a `js-guests`
  artifact; commit it (`gh run download <run> -n js-guests -D guests/prebuilt`) before the verify run.
- Known flakes that count as green: the macOS pasteboard/clipboard watcher test (#231); `command_search.rs`
  `a_service_that_stalls_is_given_up_on_within_the_limits`. A failure surviving retries: rerun failed jobs once
  (`gh run rerun <id> --repo pane-app/pane --failed`) before diagnosing.

## How to work

- Tests drive the user-visible behaviour through the ticket's named seams (core `Launcher` integration tests,
  window tests with real key events). Follow the prior-art test files the ticket names. Write tests with the code.
- Keep to the ticket's scope; do the docs/glossary updates the ticket lists. Match the code's comment density,
  naming and idiom. Decide reasonable things yourself; list decisions in your final report.
- Other tickets of this spec are being built in parallel on sibling branches; keep changes focused to reduce
  merge conflicts (no drive-by refactors or reformatting of untouched code).

## Final report (your last message, short)

Branch, final commit SHA, verify run URL and its result, a 3–6 line summary of what was built, decisions made,
anything left undone or doubtful.
