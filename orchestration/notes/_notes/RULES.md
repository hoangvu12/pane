# Rules for every subagent on this run (read fully before doing anything)

The user is playing games on this machine. These rules are absolute.

1. **Nothing Rust-related runs.** No cargo of any kind (build, check, test, nextest, clippy, fmt, xtask, run, metadata, doc, tree), no rustc, rustup or rust-analyzer, no guest or sample builds (`pane_js.py` included), no benchmarks, no measurement scripts (`scripts/measure-linux.sh`, `proc_tree.py`, any new measurement script), and never launch Pane. Write code by careful reading, following the surrounding code's APIs, imports and idioms. Check every API you call exists with that signature by reading its definition (in this repo, or in the vendored/registry sources under `%USERPROFILE%\.cargo\registry\src\` — reading those files is fine, running tools is not). Tests are written, not run. Format by hand in rustfmt's style (4-space indent, 100-column max width unless `rustfmt.toml` says otherwise, trailing commas, import grouping as the surrounding file).
2. **No CI, no GitHub writes.** Never `git push`, never `gh workflow run`, never create/edit a PR, never comment on, label, assign or close an issue. `gh issue view` / `gh api` GET is fine.
3. **Stay in your worktree.** Edit only inside the worktree you were given (under `C:\Users\ADMIN\Desktop\nguyenvu\pane-wt\`). Never touch `C:\Users\ADMIN\Desktop\nguyenvu\pane`, `pane-wt\pr`, `pane-wt\main-read`, or another ticket's worktree. Never use bare `git stash`. Do not create or remove worktrees or branches yourself.
4. **Commits.** Commit on your ticket branch with `git commit -s` (sign-off). Several commits are fine; the merger squashes them. Do not rebase or merge other branches unless told to.
5. **Read first:** `CLAUDE.md`, `CONTEXT.md` (glossary — use its vocabulary), `docs/agents/ci.md`, `docs/agents/domain.md`, then your ticket (`_notes/tickets/<n>.md`) and its spec (`_notes/specs/<n>.md`). Follow references to ADRs and docs your ticket names. Tickets cite file:line pointers from 104a5dac; lines may have shifted.
6. **Docs style.** Match the repository's documents: plain, concrete English, the glossary's terms, no marketing. Update the documents your ticket names.
7. **Prebuilt artifacts.** `guests/prebuilt/*.wasm` (the JS/TS sample components) are checked against a digest of `wit/*.wit` (byte-exact, comments included), `guests/js`, the JS/TS sample sources and the toolchain pins (`tools/componentize-js/pane_js.py`, `inputs`). If you change any of those, the prebuilt samples become stale and need a rebuild that cannot be done now: do not try; say so in your report. Same for any Rust guest you change (they are built at test time, which is fine, but say so).
8. **Benchmarks and measurements:** write the code and scripts only; numbers are taken later with the user's consent. Leave acceptance items needing numbers or native runs unchecked and say so.

## Your report (final message, and also written to `_notes/reports/<ticket>.md`)

Keep it short and concrete:
- branch and commit SHAs;
- what you built, per acceptance criterion: done (written, unverified) / not done / needs numbers / needs native evidence / needs CI;
- files touched (list);
- every API or behaviour you were unsure of and could not confirm by reading (the next person compiling must look there first);
- whether prebuilt JS/TS samples or guests need a rebuild, and why;
- spec ambiguities you resolved and how.
