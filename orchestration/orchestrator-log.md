# Orchestrator log — decisions and standing orders (new machine)

## 2026-10-09 ~17:45 UTC — user decisions (recorded by the takeover orchestrator)

1. **No local Rust / JS/TS anything on this machine either** — the no-cargo rule
   from the old PC is confirmed for the VPS: never run cargo, rustc, rustfmt,
   clippy, nextest, xtask, npm guest builds or anything else that loads the
   CPU. All compile/test feedback comes from GitHub Actions. (This answers the
   open question in the handoff's rule 1: the user did NOT opt in.)
2. **No manual CI dispatches while implementing.** CI runs only as the
   automatic quick tier triggered by a push to a `pi-subagent/**` branch —
   always the FAST workflows (`ci-fast.yml`), never the slow ones (`ci.yml`
   release matrix, `ci-branch.yml`). No chosen-tests, verify or Componentizer
   dispatches mid-implementation. The ONE exception: the single verify
   dispatch at the end of a spec branch when it is believed done (repeated
   after fixing what it found).
3. **After the current fleet is done and cleaned up**, survey the remaining
   open GitHub issues for `ready-for-agent` ones (excluding `ready-for-human`
   like #270, #281, and the needs-triage specs #198/#208 unless the user says
   otherwise) and spawn new implement-spec side chats with exactly the same
   pattern and rules, one per spec/issue group.

Propagated to: ~/.claude/skills/implement-spec/SKILL.md (rewritten rules),
notes-121/implementer-brief.md, notes-122/BRIEF.md, and a steering message to
all eight running spec chats (2026-10-09 ~17:50 UTC).

## Fleet spawned 2026-10-09 16:48 UTC

spec-151 (chat 910b9103…), spec-121 (1ca4073a…), spec-122 (1764413a…),
spec-123 (49fab629…), spec-125 (c381a234…), spec-127 (1cd1e6ff…),
spec-128 (baa33785…), spec-207 (d8dacf8f…) — parent: this orchestrator chat
(1b7f927c… "Fleet Orchestration Takeover").

## 2026-10-09 ~18:00 UTC — model for new chats

All new chats (the remaining-issues wave and any future spawns) use the same
model as the current fleet and the orchestrator itself:
`iroha/dashscope/glm-5.3`, reasoning `high`, harness `pi`, project `pane`,
kind `side` with parent = this orchestrator chat.

## 2026-10-10 ~00:30 UTC — operating lesson: steers do not reach marathon turns

Roboco steer-mode messages to a chat queue in its live mailbox and are only
delivered when the chat's turn ENDS. The spec chats run single turns of many
hours (sleep-poll loops), so rule updates sent mid-turn sit undelivered and
their implementers keep the old instructions. To deliver a rule change to a
working chat: either wait for its natural turn end, or `interrupt_chat` (the
turn is cut; queued messages then process; state survives in notes + git).
Applied 2026-10-10 ~00:25 UTC: interrupted 122/151/207/128 so the no-dispatch
correction would land. spec-123's turn had already been cut at 22:13 (transient
error) with #248 merged-unpushed and its #251 implementer dead — resumed with
an explicit state message.

## 2026-10-10 ~04:10 UTC — #207 merged to main; release-matrix found a real bug

PR #291 merged (merge commit 99acb416); results comments, checkbox ticks and
closes done for #278/#280/#282/#285/#286/#207. #284 held open: its smoke-leg
AC is "release matrix after merge". Run 38020412941: macOS smoke failed with
exactly the documented #231/#277 pasteboard flake signature (counts green);
ubuntu Smoke's workload step failed for real — measure-linux.sh writes
$out/no-default-pins.json before `mkdir -p $out` (ordering bug from the #207
rework; the workload only runs in the release matrix, so no earlier tier saw
it). spec-207 chat (kept on standby) is doing the fix-forward on a branch off
main; small PR, squash-merge, then the next release-matrix run is the first
real exercise of the clone-and-serve hidden phases. #269 handoff queued to
the #127 chat (blocked_by #256 clears when #127's PR lands).

## 2026-10-10 ~05:45 UTC — spec #207 FULLY CLOSED OUT

- PR #291 merged (99acb416); fix-forward PR #296 squash-merged (28084480)
  for the measure-linux.sh mkdir-ordering bug the first release-matrix run
  found.
- Main green (run 38023610572, attempt 2): all three smoke legs + the
  workload's hidden phases (the clone-and-serve pins machinery's first real
  exercise) passed.
- Results comments, checkbox ticks, closes: #278, #280, #282, #284, #285,
  #286, #207 — all completed. #279 was already closed.
- Cleanup: spec-207 worktree + local spec/207-official-extensions branch
  removed; all six remote pi-subagent/207-* ticket branches deleted;
  spec-207 chat ARCHIVED.
- Open thread: the macOS smoke pasteboard check
  (clipboard-history.json / pane-smoke-file.txt) fails FIRST-TRY on every
  main release-matrix run since #277 (4 runs) but passes on same-commit
  reruns — treated as a race, not a flake. Diagnosis chat spawned
  (e2edd057, "diagnose macOS smoke pasteboard failure", read-only, writes
  to notes-diag-231-smoke/diagnosis.md).
- #269 handoff queued to the #127 chat (its blocker #256 clears when
  #127's PR lands).
