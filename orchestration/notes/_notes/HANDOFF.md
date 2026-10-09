# Handoff: specs #129, #126, #188 of hoangvu12/pane — from "written and compiled" to "tested in CI"

Previous chat: "Implement core specs #129, #126, #188 (code only, no Rust runs, no CI)". Everything there is done; this chat takes over **testing through CI**.

## Read first
- Repo docs: `CLAUDE.md`, `docs/agents/ci.md` (the CI tiers — essential), `docs/agents/issue-tracker.md`.
- Notes folder (this folder, `C:\Users\ADMIN\Desktop\nguyenvu\pane-wt\_notes\`): `specs\`, `tickets\` (local copies of the issues), `reports\` (one report per ticket and per review-fix pass: what to check first when something fails), `review-*.md` (review findings, all addressed), `decisions-for-user.md`.

## Where things are
Three spec branches, each in its own worktree, all local, **never pushed**:

| Branch | Worktree | Head | Commits (first-parent, oldest first) |
|---|---|---|---|
| `spec/129-quick-fixes` | `pane-wt\spec-129` | `ac493647` | #182 `2e197255`, #131 `58cd3471`, #130 `91120f13`, #132 `90ee3158`, #133 `23594486`, review fixes `3de3d890`, rustfmt `ac493647` |
| `spec/126-file-index-targets` | `pane-wt\spec-126` | `8a7ed5b7` | #183 `45c4d133`, #184 `06cdd92f`, #185 `08ec288c`, #186 `3a011b68`, #187 `c2851c2c`, review fixes `e04f5839`, rustfmt + clippy `8a7ed5b7` |
| `spec/188-idle-cost` | `pane-wt\spec-188` | `33bdb48a` | #191 `09b23548`, #189 `09786c69`, #190 `09c53776`, merge of spec/129 `544852c4`, #192 `0aa36ede`, merge of spec/129 fixes `28333ba9`, review fixes `25a3f982`, rustfmt `33bdb48a` |

- Base: origin/main `104a5dac`. spec/188 **contains spec/129** (two merge commits) because #192 is blocked by #130 → spec/129 must land on main first.
- Ticket branches `pi-subagent/<spec>-<ticket>-<slug>` and `pi-subagent/<spec>-review-fixes` still exist locally (squashed into the spec branches; their worktrees are removed).
- Shared build cache: `CARGO_TARGET_DIR=C:\Users\ADMIN\Desktop\nguyenvu\pane-wt\_target` (seeded from the user's checkout; use it for any local cargo run).

## Verified so far (Windows only, this machine)
On all three branches: `cargo fmt --all --check`, `cargo check --locked --workspace --all-targets`, `cargo clippy --locked --workspace --all-targets -- -D warnings` all pass.

## NOT verified
- **No test has ever run** (guests or workspace), on any system.
- **macOS and Linux cfg-gated code never compiled** (no cross C compiler here; `zstd-sys` build script stops a cross `cargo check`).
- **Prebuilt JS/TS samples are stale on spec/129 and spec/188**: #130 changed `wit/data.wit` and `guests/js/data.d.ts`. The `ci-fast` quick tier rebuilds them as the `js-guests` artifact; commit it (`gh run download <run> -n js-guests -D guests/prebuilt`) before a verify run.
- No benchmark/measurement numbers (#183, #185, #186, #187, #189, #190) — only on the user's machine **with the user's consent** (they game on it).
- Native evidence (Explorer restart, screen readers, real crash/logoff, real share/USB) — release-validation, not merge gates.

## What the user approved now (2026-10-09)
The plan below: push to `pi-subagent/**` branches to run `ci-fast` (quick tier on push; one `verify=true` run via `gh workflow run ci-fast.yml --ref <branch> -f verify=true` when believed done), fix failures, repeat. **Still ask before:** opening/editing PRs, commenting on/closing/labelling issues, pushing any non-`pi-subagent/**` branch (that triggers ci-branch.yml), dispatching `ci.yml`, running benchmarks/measurements or launching Pane on this machine. Never touch `C:\Users\ADMIN\Desktop\nguyenvu\pane` (user's checkout), `pane-wt\pr`, or other sessions' worktrees (`pane-wt\adr-ext-authoring`, `pane-wt\t128-212`, …). Never bare `git stash`. Commits signed off (`git commit -s`).

## The plan (one spec at a time, spec/129 first)
1. In `pane-wt\spec-129`: create `pi-subagent/129-quick-fixes` at `spec/129-quick-fixes` and push it (`git push -u origin pi-subagent/129-quick-fixes`). Poll with `gh run list --branch pi-subagent/129-quick-fixes --repo hoangvu12/pane` (small commands, no long sleeps).
2. When the quick tier finishes, download and commit the `js-guests` artifact into `guests/prebuilt` (signed off), push.
3. Fix any compile failures on Linux; push; repeat. Then `gh workflow run ci-fast.yml --ref pi-subagent/129-quick-fixes -f verify=true` (lints + tests on Linux and Windows, macOS compile + macOS clipboard test). Fix failures (use `_notes\reports\<ticket>.md` "check first" lists), recheck single tests with `-f tests='test(/name/)'` per ci.md, then one final verify run. Fast-forward `spec/129-quick-fixes` to the green head.
4. spec/126: same with `pi-subagent/126-file-index-targets`. Integration note: spec/126 has one new `eprintln!` (merge-panic logging, file_index) that must become `crate::diagnostic!` once spec/129 is merged with it (spec/129's diagnostics test forbids raw stderr writes); spec/126 alone has no diagnostics module, so leave it until integration.
5. spec/188: first merge the final (green) spec/129 into spec/188 again if spec/129 changed, then same CI loop with `pi-subagent/188-idle-cost`. Its prebuilt samples come from spec/129's rebuilt ones via the merge.
6. Report to the user per spec: run links, what failed and how it was fixed, what still needs numbers/native evidence. Then ask about PRs (milestone PRs land with a merge commit; spec/129 before spec/188) and results comments on the issues.

## Open items to mention in the final report
- #190: no tests that closing a view or search count as in flight.
- #189: Windows measurement script can still be offered an update; never run.
- #191/#192/#133/#131 etc.: see each `reports\*.md` and `reports\*-review.md` "partially" entries.
- `decisions-for-user.md`: #184 added roots on shares/USB left out unless the switch is on (spec stories 49/51/52 suggest adding is the opt-in); #184 volume check also applied on Linux (NFS/FUSE home left out by default). Still undecided by the user.
- Unconfirmed claim: `crates/pane-core/tests/clipboard_view.rs` on main may fail after 2026-10-12 due to a hard-coded date (its restart helper uses the manual clock, so probably not).
- Toolchain: `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin` std were added to 1.98.1 for a cross-check attempt; removable with `rustup target remove`.
