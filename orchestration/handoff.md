# Pane orchestrator handoff — VPS takeover, 2026-10-09 ~15:10 UTC

You are the new orchestrator for **pane-app/pane** (GitHub; `gh --repo pane-app/pane`). Your predecessor ran on the user's gaming PC; the user migrated the fleet to this machine. The whole local fleet was parked and its state pushed to this branch (`pi-subagent/orchestration-state`). Your job: drive every open GitHub issue to implementation via side chats, verify through CI, merge, and clean up.

This doc has: mission, absolute rules, current GitHub state, per-spec fleet state + WIP map, spawn-prompt skeletons, user decisions, endgame checklist, operating notes. Read it fully.

## Bootstrap (do once, before spawning chats)

0. Prereqs the user already handled: Roboco + pi installed with the model/provider config, `gh` authenticated as hoangvu12, a clone of pane-app/pane registered as Roboco project `pane` (your cwd).
1. This machine mirrors the old PC's pi/Roboco setup — same user agents (implementer / explore / merger, in `~/.pi/agent/agents/`) and the same `~/.claude/skills` process docs (implement-spec, to-tickets, to-spec, triage, handoff). The bundle carries only state — notes, WIP patches, and this doc — nothing to install. If a process doc turns out to be missing, the originals are on the old PC.
2. Recreate the workspace layout next to the repo: `pane-wt/` holding one worktree per spec branch (`spec-NNN`) plus per-ticket worktrees (`spec-NNN-tNNN`, or `spec-NNN/.worktrees/tNNN` for #122). Spec branches live locally as `spec/NNN-…` and are pushed for CI as `pi-subagent/NNN-…` mirrors — NEVER push `spec/**` (slow branch tier). Most spec branches are at main with no unique commits (just `git worktree add` a fresh branch off origin/main); the exceptions are listed per-spec below.
3. Apply the WIP patches: each `orchestration/wip/<worktree>.patch` + `<worktree>.status.txt` + `<worktree>-untracked/` maps to a ticket worktree — `git apply` the patch and copy the untracked files in. These are snapshots taken at ~15:05 UTC; the parked chats may have committed and pushed newer WIP (message `WIP: …`) — prefer their pushed branches where they exist.
4. Read the repo docs the process references: `AGENTS.md`, `docs/agents/issue-tracker.md`, `docs/agents/ci.md`, `docs/agents/triage-labels.md`, `docs/agents/domain.md`, glossary `CONTEXT.md`.

## Rules (user-set, absolute)

1. **No local Rust by default** — the no-cargo rule was about the user's gaming PC. On this machine it is *probably* fine, but the user has NOT explicitly opted in: keep the rule (all compile/test feedback from GitHub Actions) until you ask once and record the answer.
2. **CI economy**: pushes to `pi-subagent/**` run the fast tier (quick compile check, minutes); iterate there. ONE verify run per branch at the end: `gh workflow run ci-fast.yml --repo pane-app/pane --ref <branch> -f verify=true`. Never push `spec/**`. Superseded runs auto-cancel; cancel hung runs early. Poll with small `gh` commands, no long sleeps.
3. **Merging** (docs/agents/ci.md): small PRs land **squashed**; milestone/spec PRs land with a **merge commit**. Results comments on the spec + each ticket citing commits; tick acceptance checkboxes; close blockers as completed. **After every merge, watch the main-branch CI run** (release matrix incl. smokes — ~40 min) and fix forward immediately if red; PR-branch CI does not always run the smokes (that's how #277 regressed, see below).
4. **Commits**: author `hoangvu12 <hggaming91@gmail.com>`, signed off (`git -c user.name=hoangvu12 -c user.email=hggaming91@gmail.com commit -s`). Never expose any other email to outside services.
5. **The known macOS pasteboard flake (#231)** — the macOS clipboard watcher test failing once under plain cargo test — **counts as green**. A failure surviving its nextest retries is real.
6. Keep YOUR context lean: implementation happens in side chats; you orchestrate, merge, comment, clean up. Hand off (the `handoff` skill) around ~30% context.
7. User preference: **don't message chats that are actively working**. Resume only stalled ones. Chats report to you when done.
8. The user games on the old PC and runs this machine for the fleet — keep this machine's load sane too (the chats are API-bound; that's fine).

## Current GitHub state (as of 2026-10-09 ~15:05 UTC)

- **main = a1f1ece8, CI green** (run 37941509627, rerun of failed jobs completed success ~15:49 UTC). One watch-item below.
- **Open PR: #276** (DRAFT) — spec #151 Capabilities, head `pi-subagent/151-capabilities`. Not ready; ticket #153 mid CI-fix.
- Landed earlier (do not redo): PRs #257 (#217), #271 (spec #129), #273 (spec #126), #274 (spec #188), #272/#275 (fixes), #119 (Raycast research + ADRs 0030-0041), **#277** (date-bomb fix — now regressed, see URGENT), **#288** (deferred measurements evidence, docs-only).
- Measurement tickets #183/#185/#186/#187/#189/#190 are CLOSED with results comments. **#185 is a MISS**: query p95 24-28 ms vs the 10 ms target (Document kind-filter + multi-segment shapes are the drivers; reproducible, reported on the ticket). Tell the user; do not silently reopen.
- 25 tickets published for specs #123/#125/#127 (#245-#270) with correct sub-issue/blocker graphs on GitHub — the issue graph is authoritative for what remains.

## Watch-item: a flaky macOS smoke after #277 (main is green — no fix mission)

#277 ("Keep the restarting clipboard test's records past the start sweep", merged a9b86edb) coincided with two consecutive macOS Smoke failures on main (runs 37933807886 and 37941509627): final failing check **`smoke/clipboard-data/extensions/clipboard-history.json: pane-smoke-file.txt is not present`** — the smoke's copied file record missing from the clipboard-history data after an app restart. A rerun of 37941509627's failed jobs on the SAME commit **passed** — flake signature, not a confirmed regression; the old-PC date-fix chat reached the same conclusion before it was parked. Windows smoke also failed once and passed later (independent flake).

What to do: nothing now. After each merge, watch main's CI run to completion; if that exact check fails again (especially twice), investigate the #277 start-sweep area — the record being dropped, or a storage format the smoke's grep no longer matches (#275 updated the smokes' greps for the compact file, so there is precedent for matching a smoke to a deliberate format change, NOT for hiding data loss) — and fix forward, gating the fix with ONE verify run before merging.

## Fleet — per-spec state (all parked; spawn one chat per spec)

Spawn pattern (Roboco `create_chats`, project `pane`, kind `side`, one per spec): a mission prompt like the #122 skeleton below, filled from the per-spec block. The chats use the implement-spec process (`~/.claude/skills/implement-spec/SKILL.md`), run implementer subagents per ticket in separate worktrees, squash-merge finished tickets into the spec branch as one signed-off `… (#NNN)` commit, push the spec branch's `pi-subagent/NNN-…` mirror, open a draft PR early, ONE verify run at the end, then **STOP before merging and report** (PR link + verify run + summary). You merge, comment, tick, close, archive.

Skeleton: "You are implementing spec #NNN in pane-app/pane ('<title>') following the implement-spec process at ~/.claude/skills/implement-spec/SKILL.md, using the repo's user subagents (Agent tool: 'implementer' per ticket, 'explore' for recon). Read AGENTS.md, docs/agents/*.md, the spec issue and its ticket graph (GitHub issues are authoritative), ADRs as relevant, and the notes in pane-wt/notes-NNN/. Worktree pane-wt/spec-NNN on branch spec/NNN-… (push CI mirror pi-subagent/NNN-…). [per-spec state + WIP pointers]. Absolute rules: [rules 1-5 above]."

- **#151 Capabilities** — spec branch `spec/151-capabilities` has local-only commit 3307d40f ("#152" squash; safety-pushed to `pi-subagent/151-capabilities` = PR #276 head — verify). Ticket #153 in flight on `pi-subagent/151-153-capabilities` (ef5bba91, mid fix-rounds; a verify/dispatch run may be in progress). Worktree `spec-151-t153` clean. Check #151's sub-issues for remaining tickets. Notes: `notes-151/`.
- **#121 Extension UI** — frontier #235 (tree views, JSX runtime, Rust builder, counter sample), then #236/#237/#239 etc. (issue graph authoritative; 9 tickets total). Ticket worktree `spec-121-t235` (branch `pi-subagent/121-235-tree-views`) at base with one untracked `wit-spec.md` (a WIT draft from a dead implementer — in `wip/spec-121-t235-untracked/`). Notes `notes-121/` has the codebase map, `salvage-235.patch` (1.7k lines of earlier WIP: WIT envelope design ×2 resolving `open-view` as `open-designed-view`, a 785-line strict parser `runtime/designed.rs`, runtime/launcher plumbing — none of it ever compiled), `salvage-235-designed.rs`, `implementer-brief.md`, `pr-body.md`.
- **#122 Root search** — frontier #193/#194/#195/#196; then #197 (after #193), #201 (after #194), #204 (after #195); then #199 (after #197), #200 (after #199), #202/#203 (after #201), #205 (after #203), #206 (after #200+#205). Four ticket worktrees exist under `spec-122/.worktrees/` (t193-t196, branches `pi-subagent/122-19N-…`), all at base with **no commits**; only t194 has WIP (7 files — patch in `wip/spec-122_.worktrees_t194.patch`). NO codebase map was ever written (the explorer died) — run the explore agent first. Notes: `notes-122/` (BRIEF.md, tickets.md, spec body, pr-body).
- **#123 Launcher polish** — tickets #245-#251 then #258. Ticket worktrees `spec-123-t245…t251`: t246 pushed (1e02e7f8, CI iterating "Match rustfmt…"), t247 pushed (631d42e5), t245/t249/t250 have uncommitted WIP (patches in `wip/`), t248/t251 clean at base. Notes: `notes-123/`.
- **#125 Windows power** — tickets #252-#268; **#270 excluded** (user's own by-hand check, labelled `ready-for-human`). Ticket worktrees `spec-125-t252…t255` all clean at base (implementers were just starting). Notes: `notes-125/`.
- **#127 Extension updates** — implements #256 then #267 then STOPS and reports. **#269 is blocked by spec #207** — after #207 lands, steer the #127 chat (or a small new chat) to finish #269. An implementer had just started #256 when the fleet was parked: its WIP (a `launcher/update_results.rs` + a launcher.rs edit, never compiled) is snapshotted as `wip/temp-job-78785d4db4ff-127-update.patch` (+ untracked copy). Notes: `notes-127/` (+ `notes-127-spec127.json`).
- **#128 pane-ext authoring** — remaining tickets #214/#218/#219/#221/#222/#223/#224/#225; already landed #212/#213/#215/#216/#217/#220. Ticket worktrees: t214 (WIP 20 files), t218 (WIP 102 files — the big JS/TS build pipeline; its downloaded wasm deps are in `notes-128/t218-downloads/` ~20MB), t224 (WIP 21 files) — patches for all three in `wip/`. Notes: `notes-128/` (build-map, app-map, per-ticket notes).
- **#207 Official extensions** — user-APPROVED design (see Decisions). Tickets first, then implementation; it creates the five `pane-app/<name>` repos. In flight: t278 (WIP 24 files, patch in `wip/`), t279 (clean), t280 pushed (388bfe1b, CI at 14:56 UTC in progress). Notes: `notes-207/` (+ tmp json).
- **Date-bomb fix / #277 regression** — see URGENT above.
- **Measurements** — DONE and closed out. Nothing to do; PR #288 landed, comments posted. (#185 miss to report to the user at the end.)

Workspace reference: everything lives under `pane-wt/` (spec-NNN spec branches, spec-NNN-tNNN ticket worktrees, notes-NNN folders, `_notes` = finished specs' reports). The old PC also had a shared build cache `_target` (not migrated; only relevant if local cargo is ever allowed).

## User decisions (recorded, don't re-ask)

- **#207 approved**: five default extensions move to `pane-app/calculator|applications|quicklinks|files|clipboard-history`; Pane installs all five right after app install (first setup, **no onboarding UI yet**, not bundled); `downloads.pane.sh` never existed — the repos + pinned commits via Pane's own Git client (ADR 0021) are the only default-install path.
- **#270** stays `ready-for-human` (user's own by-hand check).
- Machine use for benchmarks was granted on the OLD PC only (measurements are done anyway).
- Specs #198/#208 remain needs-triage (not blocking; ask user before starting).
- #66/#67 (macOS vibrancy / Linux opaque launcher) remain open, need native verification — user's call at the end.

## Operating notes

- Engine restarts cut chat turns and wipe background subagent jobs. A chat idle with a stale `lastMessageAt`, waiting on background jobs that will never report, is STALLED — resume it with a short `send_message` stating current state and "continue". Check with `list_chats` (project pane) + `read_chat` tails + process liveness.
- The implement-spec chats **STOP before merging** and report: PR link + verify run + summary. You then: squash or merge-commit per ci.md, post results comments citing commits, tick checkboxes, archive the chat, remove its worktrees and branches (remote `pi-subagent/*` auto-delete on merge if the PR used --delete-branch).
- If a chat's CI run hangs: cancel early, rerun failed jobs once, use targeted runs `-f verify=false -f tests='test(/name/)'`.
- GitHub issue numbers are authoritative; PRs and issues share the numbering space.
- Cleanup still owed from the old machine (do at endgame): remove stale worktrees `pane-wt/core-specs` and `pane-wt/pr` (registry + folders, finished #120 work), local branch `backup/129-pre-rebase`, the `measure` worktree (measurements done), and the `orch-push` worktree + `pi-subagent/orchestration-state` branch once the migration is confirmed. Ask the user about `notes-*`/`_notes` folders before deleting anything.

## Endgame checklist (when waves report done)

1. Merge verified PRs (#276 and the wave PRs), comment, tick, close — watching main's CI after each merge (see the flaky-smoke watch-item above).
2. After #207 lands: finish #269 (steer the #127 chat), then spec #127 closes.
3. Clean up: remove all `pane-wt/spec-*` worktrees and local `spec/*`, `pi-subagent/*` branches (plus the items in Operating notes).
4. Archive every finished side chat.
5. Report to the user: what landed (PR links), the measurements' pass/miss (**#185 misses 10 ms p95 — 24-28 ms, Document kind-filter + multi-segment shapes**), what's left (#66/#67, #270 by hand, #198/#208 triage, native/benchmark caveats in `notes/_notes/`).
