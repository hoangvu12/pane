# Orchestration state for spec #151 (for the coordinating agent — update as state changes)

Last updated: 2026-10-09 20:15 local (after resuming impl-153).

## Fixed facts
- Main worktree: C:\Users\ADMIN\Desktop\nguyenvu\pane-wt\spec-151, branch spec/151-capabilities, pushed as pi-subagent/151-capabilities (NEVER push spec/**).
- Draft PR: https://github.com/pane-app/pane/pull/276 (head pi-subagent/151-capabilities, body: notes-151\pr-body.md).
- Notes: C:\Users\ADMIN\Desktop\nguyenvu\pane-wt\notes-151\ (brief.md = absolute rules for every subagent; codebase-map.md; ticket-15*.md; spec-151.md; adr-0041.md).
- Merge choreography per finished ticket: in spec-151, `git merge --squash <ticket-branch>`, then `git -c user.name=hoangvu12 -c user.email=hggaming91@gmail.com commit -s` with title "… (#NNN)" and a body saying what it does; push `git push origin spec/151-capabilities:refs/heads/pi-subagent/151-capabilities`; remove the ticket worktree (`git worktree remove ../spec-151-tNNN`, `git branch -D <branch>`); launch the next frontier tickets with new worktrees (`git worktree add ../spec-151-tNNN -b pi-subagent/151-NNN-slug <merge-commit>`).
- Ticket graph: #152 → #153 → {#154, #155, #156, #157} → (#158 after #153+#156) → (#159 after #158). Claim with gh issue edit <n> --repo pane-app/pane --add-assignee @me when started (#152, #153 already claimed).
- ONE verify run at the very end on pi-subagent/151-capabilities: gh workflow run ci-fast.yml --repo pane-app/pane --ref pi-subagent/151-capabilities -f verify=true. macOS pasteboard flake (#231) counts as green. Then STOP: no merge, no checkbox ticking, no results comments. Report PR link + verify link + plain-language summary.

## Known failure mode (happened twice)
The parent pi session dying KILLS background subagent processes mid-run (their .pi-subagent-locks go stale at the exact death moment; uncommitted worktree changes survive). Recovery: check `~/.pi/agent/sessions/--C--Users-ADMIN-Desktop-nguyenvu-pane-wt-spec-151-tNNN--/.pi-subagent-locks/*` timestamps (stale = not updating for minutes); if stale, `rm -rf` the lock and relaunch the Agent with the same session name (e.g. impl-153), agent "implementer", cwd the ticket worktree, and a continuation prompt that says: process was killed, review git status in your worktree, continue from where you left off (all absolute rules still apply). The child session context survives in C:\Users\ADMIN\.pi\agent\sessions\...

## Ticket status
- #152 DONE, merged as 3307d40f "Let commands wait for a required dependency that cannot serve them (#152)" (squash of pi-subagent/151-152-wait-on-dependencies, tip 7f9001fd; verify run 37909850146 green). Worktree and local branch removed; the remote branch pi-subagent/151-152-wait-on-dependencies stays on origin.
- #153 IN PROGRESS: session impl-153 (job-e4ea6f5c98c9), worktree spec-151-t153, branch pi-subagent/151-153-capabilities (tip 3392594d + uncommitted work; killed once at 19:16 by session death, resumed 20:15). Implementer must: fix quick-tier compile + js-guests rebuild failures, finish capabilities tests/docs/fixtures, ONE verify, then report.
- #154/#155/#156/#157: NOT STARTED (blocked by #153). Launch all four in parallel when #153 merges: worktrees spec-151-t154/155/156/157, branches pi-subagent/151-154-provider-choice, -155-install-plans, -156-capability-waiting, -157-manage-extensions.
- #158: blocked by #153+#156 (+#136/#137/#138/#139, all closed).
- #159: blocked by #158 (+#143/#138, closed).

## After all 8 tickets merge
Careful self-review (or code-review process) of pi-subagent/151-capabilities; fix findings in one pass (can be an implementer subagent on a new ticket-style branch, or direct in the spec worktree); final verify run; update PR body if needed; report and STOP.
