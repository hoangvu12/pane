# Spec #207 progress

Milestone branch: `spec/207-official-extensions` (pushed as `pi-subagent/207-official-extensions`), worktree `pane-wt/spec-207`, cut at origin/main 26c8e224.
Milestone tickets: 278, 279, 280, 282, 284, 285, 286. Out of this milestone: 283 (deferred picker), 287 (blocked by person's step), 281 (person's step under #128).

## Waves

- Wave 1 (running):
  - #278 First setup installs the default extensions from pinned commits — implementer job-0cf87f77b4c6, worktree `pane-wt/spec-207-t278`, branch `pi-subagent/207-278-first-setup-pins`.
  - #279 The five pane-app repositories — implementer job-cac737d08533, worktree `pane-wt/spec-207-t279`, branch `pi-subagent/207-279-five-repositories` (no pane commits; fresh history for the repos — pane history has another author's email that must not be exposed).
  - #280 Official marking in Settings — implementer job-5c6f042321e9, worktree `pane-wt/spec-207-t280`, branch `pi-subagent/207-280-official-marking`.
- Wave 2: #282 pin the release commits (needs 278+279; needs the five tags+commits from #279's results comment).
- Wave 3: #284 smokes serve pinned repositories (needs 282).
- Wave 4: #285 sources leave guests (needs 284).
- Wave 5: #286 glossary + CI notes (needs 284+285).
- Then: code-review pass, fix wave, ONE verify run on the spec branch, PR ready, STOP before merge, report.

## Merge rules

- Each ticket merges onto `spec/207-official-extensions` as ONE commit: `git merge --squash <ticket-branch>` then `git -c user.name=hoangvu12 -c user.email=hggaming91@gmail.com commit -s -m "<title> (#NNN)"` with a body saying what it does.
- Merge order in wave 1: 278 first, then 280 (279 has no pane commits).
- After each wave's merges: single push of the spec branch (pushes auto-cancel superseded quick-tier runs).
- After merge: append the squashed commit hash to the ticket's results comment and close the ticket as completed.

## Decisions recorded

- User decisions (2026-11-05 approval) — posted as comment on #207 (issuecomment-6082076862) and to be repeated in the PR body:
  1. Five repos under pane-app, created now.
  2. Install all five at first setup via the Git client; NOT bundled; no picker (deferred to #283).
  3. downloads.pane.sh never deployed → repos + pinned commits are the only default-install path; artifact source keeps application updates only.
- Fresh history for the five repos (no subtree split): pane's history contains another author's email; the no-other-email rule forbids exposing it.
- The repos' CI builds pane-extension via a [patch.crates-io] to a pinned pane checkout until the crate is published (#281, person's step); #287 switches them to the published crate.
- Dev override for pins: proposed `PANE_DEFAULTS` (a JSON pins file, dev builds only), mirroring PANE_ARTIFACTS (which stays, for application updates only).

## Notes

- Code map: notes-207/code-map.md. Ticket bodies: notes-207/tickets/.
- Ticket graph edges set on GitHub (sub-issues of #207; #281 sub-issue of #128; blocked_by edges per the graph).
- #269 unblocks when this lands: its needs (record keeps repository/tag/commit/pinned; loopback test repos) are in #278.
