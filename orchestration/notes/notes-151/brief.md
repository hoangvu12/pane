# Brief for every subagent on spec #151 (pane-app/pane)

## Pointers
- Spec: `spec-151.md` here (copy of issue #151). Tickets: `ticket-<n>.md` here (copies of #152-#159).
- ADR 0041 (not yet on main; PR #119): `adr-0041.md` here. Glossary/ADR amendments from PR #119: `pr119-context-and-adr-amendments.diff`. Research: `raycast-deep-dive.md`.
- Codebase map for this spec: `codebase-map.md` here (written by the exploration agent; may arrive a little after you start).
- Repo docs: AGENTS.md, CONTEXT.md (glossary), docs/agents/ci.md (CI tiers — read it), docs/agents/domain.md, docs/operations.md, docs/dependencies.md, docs/adr/0005, 0011.
- Ticket results of earlier tickets: `git log` on the spec branch `spec/151-capabilities` (squash commits titled `… (#NNN)`).

## Absolute rules
- NEVER run cargo, rustc, rustfmt, xtask, npm builds, or any Rust toolchain locally (the user games on this machine). All compile/test feedback comes from GitHub Actions.
- rustfmt is checked in CI: format by hand (max_width 100, fn_call_width 60, chain_width 60; check `rustfmt.toml`/`.rustfmt.toml` if present). Imports sorted as rustfmt would.
- Work only in your own worktree folder. Never touch C:\Users\ADMIN\Desktop\nguyenvu\pane, other pane-wt folders, or pane-wt\_notes. Never bare `git stash`.
- Commit as: `git -c user.name=hoangvu12 -c user.email=hggaming91@gmail.com commit -s -m "..."`. Never put any other email in a request to an outside service.
- Push ONLY your `pi-subagent/151-...` branch (`git push -u origin <branch>`). Never push spec/** or main.
- CI economy: each plain push runs the quick tier (fmt + cargo check on Linux/Windows, ~13 min; rebuilds JS/TS prebuilt samples if stale and uploads `js-guests` artifact). Iterate on plain pushes. Poll with small commands: `gh run list --repo pane-app/pane --branch <branch> --limit 3`, `gh run view <id> --repo pane-app/pane --log-failed | tail -80`. No long sleeps (short sleeps of ≤60s between polls are fine; prefer doing useful work while waiting).
- If the quick tier uploaded `js-guests`, commit it: `gh run download <run> --repo pane-app/pane -n js-guests -D guests/prebuilt` before the verify run.
- When you believe the branch is done: ONE verify run: `gh workflow run ci-fast.yml --repo pane-app/pane --ref <branch> -f verify=true`. To recheck single tests after a fix: `-f verify=false -f tests='test(/name/)'`, then one final verify if code changed substantially. Superseded runs auto-cancel.
- Known flake that counts as green: macOS pasteboard/clipboard watcher test (#231). Also see the Flakes section in docs/agents/ci.md.
- Do not comment on, close, or tick issues. Do not open PRs.

## Style
- Match surrounding code: comment density, naming, idioms, test style (tests drive `pane_core::Launcher` with real samples; window tests with real key events).
- Docs are plain, short sentences like the existing docs.
- Proposed defaults in the spec are accepted; decide small things yourself and list decisions in your final report.

## Final report (keep it short)
Branch name, final commit SHA, verify run URL and result, a 5-10 line summary of what was built, decisions taken, and anything left undone or risky.
