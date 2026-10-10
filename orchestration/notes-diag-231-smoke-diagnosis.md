# Diagnosis — `Smoke (macos-15)` fails `pane-smoke-file.txt is not present` on first-try main runs

Symptom under diagnosis:

```
smoke/clipboard-data/extensions/clipboard-history.json: pane-smoke-file.txt is not present
```

Failing step: release matrix `ci.yml` → `Smoke (macos-15)` → `Native GUI smoke (macOS)`
(`bash scripts/smoke-macos.sh smoke`), the clipboard phase's copied-file check.

Runs examined (read-only): 37933807886 (a9b86edb, failed), 37941509627 attempt 1
(a1f1ece8, failed) and attempt 2 (passed), 38020412941 (99acb416, failed),
38023610572 (28084480, first try failed per the task; its rerun was still in flight
while diagnosing), plus 37907199550 (26c8e224, passed first-try) for the differential.

**Verdict: this is not a regression from #277 (or #275/#192). It is the smoke-side
face of the open runner-environment issue #231 — "macOS runners stopped reporting a
file copied by another program to the pasteboard", since ~20:30 UTC on 2026-10-08 —
which had been masked on main by #192's compact file breaking the smokes' earlier
greps (#275's commit message documents that). #275 (landed in the same push as #277)
made the smokes reach the file-copy step again, so the failure "reappeared with #277".**

---

## 1. The exact sequence that fails

`scripts/smoke-macos.sh`, clipboard phase (data dir `$out/clipboard-data`, i.e.
`smoke/clipboard-data` — set at :1938; `history=$extensions/clipboard-history.json`
at :1943):

1. `:2082` `history_action '1 Hour'` — the Clipboard History view sets retention to
   3600 s; this deletes the 2-hour-old `pane-smoke-enabled` record and is written at
   once (a command's change, `HistoryStore::write_staged`). The smoke verifies it
   (`:2083` `wait_for '"retentionSeconds":3600'`, then the `kept_texts` check
   asserting exactly `pane-smoke-after-restart,pane-smoke-resumed`). All of this
   **passed** in every failing run — the store, its write thread and the watcher are
   healthy up to here.
2. `:2086` the smoke creates `$out/clipboard-home/pane-smoke-file.txt`.
3. `:1963` `copy_file() { osascript -e "set the clipboard to (POSIX file \"$1\")"; sleep 1; }`
   — an **external, short-lived process** takes ownership of the general pasteboard,
   declares `public.file-url`, and exits within ~0.5 s.
4. `:2089` `wait_for "$history" pane-smoke-file.txt present` — greps the history file
   every 0.1 s for 10 s (`wait_for`'s default `tries=100`, :311), then prints the
   failure line and exits 1.

What Pane does between 3 and 4 (`crates/pane-core/src/clipboard/macos.rs`):

- The watcher thread polls `NSPasteboard::generalPasteboard().changeCount()` every
  `POLL` = 250 ms (macos.rs:88, `Watcher::run`).
- On a new count, `Watcher::observe` (macos.rs:~440) sets `read_through = count`
  (**the change is consumed**), takes the capture ticket, and reads the pasteboard.
  For a file copy whose data is not yet readable it loops until
  `FILE_READY_WAIT` = 2 s (macos.rs:93), re-reading every poll
  (`waiting_for_file = types.is_empty() || (types contains public.file-url && files.is_none())`,
  `observation()`, macos.rs:~505).
- `files()` (macos.rs:~570) reads the URLs via `pasteboardItems()` and each item's
  `stringForType(public.file-url)`. If that yields nothing for the whole 2 s, the
  observation falls through `files.or_else(text).or_else(image).unwrap_or(Content::Other)`
  → **`Content::Other`**.
- `CaptureSink::observed` (`clipboard.rs:639`) → `store.capture(...)` →
  `accept_any` (`clipboard.rs:265-`) → `Content::Other` → `Err(Skip::NotText)` →
  `continue` — **nothing is kept, and nothing is logged** (a report of "no readable
  content" is a normal outcome; only a panic is caught-and-lost silently, macos.rs:~460).
- The consumed changeCount is never re-read: `read_through == count` means the
  watcher will not look at that change again, even when the smoke keeps waiting 10 s.

So the smoke's 10 s `wait_for` cannot ever succeed once the 2 s file-ready window
has expired without data: the copy is already permanently, silently dropped.

## 2. Log and artifact evidence

### Step log (38020412941, job "Native GUI smoke (macOS)")

```
04:00:19.47Z  smoke/287-clipboard-after-restart.png: 786 pixels … (restart phase OK)
04:00:21.30Z  5 screenshots show different Pane windows
04:00:38.03Z  smoke/400-clipboard-expired.png: 716 pixels … (start sweep removed pane-smoke-kept)
04:01:08.62Z  smoke/clipboard-data/extensions/clipboard-history.json: pane-smoke-file.txt is not present
04:01:08.71Z  ##[error]Process completed with exit code 1.
```

The 30.6 s between the `400-clipboard-expired` check and the failure is exactly the
scripted sleeps of Delete Entry / `1 Hour` / `open_history` (~19 s) + `copy_file`
(~1.5 s) + the full 10 s `wait_for`. 37941509627 attempt 1 shows the identical gap
(14:44:34.93 → 14:45:05.64, 30.7 s). The passing attempt 2 finished the *whole rest
of the phase* (Clear History, final copy, distinct check) 35.9 s after its
`400-clipboard-expired` check — i.e. there the file record landed within ~1 s of the
copy. Nothing in the failing runs is slow; the record simply never appears.

### Evidence artifact `gui-smoke-macOS` (uploaded even on failure)

`smoke/clipboard-data/extensions/clipboard-history.json` (38020412941 **and**
37933807886, byte-for-byte the same shape):

```json
{"version":1,"packages":{"default:clipboard-history":{"capture":"on",
"retentionSeconds":3600,"items":[
 {"id":5,"text":"pane-smoke-after-restart","copiedAt":1791604812174},
 {"id":2,"text":"pane-smoke-resumed","copiedAt":1791604727647}],
"nextId":6}}}
```

- The file record is absent and **`nextId` is still 6** — ids are never reused
  (`PackageHistory::keep`, history.rs:~380), so `keep()` never ran for the file
  copy: the capture never reached the store. This rules out every
  "written late / written then deleted" theory in one stroke.
- The state matches the smoke's last passing assertion exactly (`kept_texts` after
  `402`): the store is consistent, not torn.
- `clipboard-data/logs/pane.log`: five `Pane 0.1.0 started` lines (03:58:11,
  03:58:57, 03:59:32, 04:00:02, 04:00:21), **no diagnostics** — no
  "could not save the clipboard history", no watcher problems.
- `smoke/stderr.log`: zero clipboard/pasteboard/watch lines (only the expected
  diagnostics of unrelated phases).
- `copiedAt` of the last text record (id 5) = 04:00:12.174Z, 46 s before the file
  copy: the watcher of that Pane run had already proven itself on a text copy —
  the watcher thread is alive; only the *file* copy is lost.

### The differential that breaks the "deterministic first-try failure" framing

- 37907199550 — the push run of **26c8e224 (#275 only)** at 2026-10-09 08:50:
  `Smoke (macos-15)` **passed on attempt 1** (job 113743635604, run_attempt 1,
  08:50:40 → 09:32:28). a9b86edb differs from 26c8e224 only by #277's change to
  `crates/pane-core/tests/clipboard_view.rs` — a test file that is not part of the
  smoke binary or script. Same code, one hour earlier: **pass**. The step is not
  deterministically broken by any commit.
- Failing vs. passing attempts of 37941509627: **identical runner image**
  (`macos-15-arm64` 20260907.0337) on both (job logs, "Runner Image" lines).
  Not an image roll.
- Scoreboard of every recorded execution of this step on main:
  37907199550 a1 pass · 37933807886 a1 fail · 37941509627 a1 fail / a2 pass ·
  38020412941 a1 fail · 38023610572 a1 fail. Host/run-intermittent, ~70% fail.

### The pre-existing twin: issue #231

`gh issue view 231` (open, `needs-triage`): since ~20:30 UTC on **2026-10-08**
(seven hours *before* #192 landed, a day before #275/#277):

- `clipboard_adapter_macos::the_watcher_reports_this_tests_changes_until_dropped`
  panics with **"the externally copied file is reported"** on every `ci-fast.yml`
  verify run since then (37839293712 and its rerun, 37840686625, 37851442856).
- **main's release run 37846622722 fails `Smoke (macos-15)` with the exact message
  under diagnosis here** — on 2026-10-08, before #192/#275/#277 existed.
- "Plain-text copies are still reported; only the copied file is missing. Linux and
  Windows are unaffected." Same image within those runs (20260828.587).

That test (`crates/pane-core/tests/clipboard_adapter_macos.rs:245-269`) reproduces
the smoke's mechanism precisely — it runs
`osascript -e 'set the clipboard to (POSIX file …)'` twenty times with
13 ms-increment offsets "crossing the watcher's polling boundary while AppleScript
declares and fills the pasteboard", and asserts the watcher reports
`Content::Files` each time. It fails on the affected runners; the *same test's*
in-process sections (the test's own `write_files`, and its own board-level
`setString_forType` fill after `declareTypes_owner`) still pass. So the read path
works; only a file URL supplied by a **short-lived external process** is missing.

## 3. Mechanism (most likely, and what it is not)

**Mechanism.** The smoke copies a file through a process that dies immediately
(`osascript`), which declares `public.file-url` on the pasteboard (one changeCount
bump) but whose data — on the runners affected since 2026-10-08 ~20:30 UTC — never
becomes readable to Pane: `type_names()` lists the type while `files()`
(`pasteboardItems()` + `stringForType`) returns nothing, for the whole
`FILE_READY_WAIT` (2 s). At the deadline `Watcher::observe` reports
`Content::Other` and consumes the change (`read_through = count`, never re-read);
`accept_any` skips it (`Skip::NotText`); the store never sees a change (`nextId`
stays 6, no write, no diagnostic). The smoke's 10 s grep then fails. On unaffected
runners the data is readable within the window and the record lands as id 6 within
~1 s (the passing artifacts). Plain text written by the same osascript mechanism is
served eagerly and keeps working in every failing run — exactly #231's "only the
copied file is missing".

The environmental break sits *below* the runner image version (fail and pass share
20260907.0337; #231's failures were on 20260828.587) and affects only data supplied
by an owner that exits at once — consistent with the pasteboard server no longer
resolving/caching a lazily-supplied file URL from a dead owner. #179 (6cf03af5,
2026-10-08 09:29 UTC) added `FILE_READY_WAIT` precisely because AppleScript
announces a file copy before its data is filled without another changeCount bump
(the test asserts "Filling a declared type does not increment that change count",
clipboard_adapter_macos.rs:238); the environment change four hours later pushed
that race past recovery on most hosts.

**Why it looks like "first-try fails since #277".** Pure timing:

- 2026-10-08 20:30: environment breaks (first victim: 37846622722, see #231).
- 2026-10-09 06:44: #192 lands → every smoke now dies *earlier*, at the
  pretty-form greps (`"capture": "paused"`), masking the file step.
- 2026-10-09 08:47: #275 (26c8e224) fixes the greps → its own push run
  (37907199550) reaches the file step and **passes** (lucky runner).
- 2026-10-09 13:00: #277 (a9b86edb) lands in the next push together with #275 →
  every main first-try since then (4 runs) hit affected runners; the one rerun
  (15:12) hit an unaffected one.

#277 itself changed one test file only (`git show a9b86edb --stat`:
`crates/pane-core/tests/clipboard_view.rs`, +16/−3) and cannot affect the smoke;
its start-sweep subject is a red herring here — the sweep ran at the 04:00:21 start,
37 s before the copy, was verified by the smoke's own post-start `kept_texts` check,
and `nextId` proves no record was ever created to sweep.

## 4. Alternative hypotheses, ranked and rejected

1. **#277's start sweep deletes the smoke's record** (the task's framing) —
   rejected: test-only change; the sweep is verified to have run correctly
   (`400-clipboard-expired` + the `kept: …` check passed); `nextId: 6` proves no
   record was ever kept, let alone swept.
2. **The record is kept but written late (#192's 500 ms batching) or the write
   fails** — rejected: `nextId: 6` means `keep()` never ran; the retention change
   (a command write) landed promptly; no write failures in `pane.log` or
   `stderr.log`; a batched copy write is ~0.5–1.5 s, not >10 s.
3. **The smoke greps the wrong (compact) form, #275's territory** — rejected: the
   three greps #275 fixed all pass before the failure; the failing grep is a plain
   path-substring grep that matches either form.
4. **The `deletions` ticket fences the capture out** (`CaptureSink::reading` /
   `capture`'s `state.deletions == deletions`) — rejected: the only deletion
   (`1 Hour` removing `pane-smoke-enabled`) completed and was verified ~4 s before
   the copy; nothing deletes during the file-ready wait; and this cannot explain
   #231's adapter-test failure, which involves no history store at all.
5. **The watcher thread died** — rejected: text copies were reported by the same
   watcher 46 s earlier (id 5, copiedAt 04:00:12.174Z vs. copy at ~04:00:58) and
   throughout the phase; #231 states text is still reported while only files go
   missing; a dead thread loses everything.
6. **A runner image roll** — rejected: identical image on the failing and passing
   attempts (20260907.0337); #231's earlier failures were on 20260828.587.
7. **`copy_file`'s osascript failed silently** — rejected: `set -euo pipefail`
   aborts on an osascript error, and the script reached `wait_for`; #231's test
   asserts the osascript exit status and fails only on the missing report.

## 5. Proposed fix

**Primary (targeted, unblocks the smoke) — `scripts/smoke-macos.sh:1963`:** keep
the copying process alive across Pane's file-ready window, matching the real-world
shape of a file copy (Finder, a long-lived owner) rather than a copy whose owner
dies before the server resolves its data:

```bash
# Copies the file at path $1, as Finder's Copy does (a file URL). The copy's
# process stays alive past the watcher's file-ready wait (FILE_READY_WAIT, 2 s):
# the pasteboard serves a declared file URL's data from its living owner, and an
# osascript that exits at once can leave the type declared but its data never
# readable (#231).
copy_file() { osascript -e "set the clipboard to (POSIX file \"$1\")" -e "delay 2"; sleep 1; }
```

Risks: adds ~2 s to one step; if the affected hosts never serve the data even while
the owner lives, this changes nothing and the copy must move to another mechanism
(drive Finder, or a small long-lived helper) — the current evidence cannot
distinguish those two worlds (see §6); it also relaxes the step's claim to cover
"a program that copies a file and exits", but on the affected hosts such a copy is
unreadable to every consumer, not only to Pane.

**Secondary (app hardening, only if warranted by #231's root cause) —
`crates/pane-core/src/clipboard/macos.rs`, `Watcher::observe`:** when the deadline
passes with a declared-but-unsupplied `public.file-url`, do not consume the change
permanently: re-check the same changeCount on later polls for one further bounded
window before reporting `Content::Other`. Risks: delays genuine `Content::Other`
reports by the extra window; holds the capture ticket longer, fencing deletions
for that time (the "Keep the first ticket" invariant at macos.rs:~455); adds state
to `observe`. Unverifiable from here whether any host's data arrives after 2 s.

**Tracking:** the runner-side root cause stays with open issue #231 (this diagnosis
adds: the failure is host/run-intermittent — 2 of 7 recorded attempts passed —
and is not tied to the runner image version). Per the task rules I did not comment
on the issue.

Also worth noting for `docs/agents/ci.md`'s flakes section: the smoke steps have no
in-job retry (unlike nextest), so "first-try fail / rerun pass" is the only retry
signal the macOS smoke offers — and here it pointed at a per-run host coin flip,
not a code race.

## 6. Evidence I could NOT get

- **The pasteboard's actual state on a failing runner** (types vs. readable data at
  the moment of the copy): the smoke captures no pasteboard dump — `pbpaste` reads
  text only, and the runner is gone after the job. A one-line
  `osascript -e 'clipboard info'`/`pasteboardItems` dump in the failing branch of
  `wait_for` (a smoke change, not made here — read-only task) would provide it.
- **What changed on the hosts at 2026-10-08 ~20:30 UTC**: below the image version;
  no runner changelog is visible from the repo or job logs.
- **Runner host identity** for fail vs. pass (GitHub does not expose the VM name in
  the readable logs), so "host-dependent vs. per-run race" cannot be separated
  from outside.
- **Whether another application could paste that copied file** on an affected
  runner — the discriminating experiment between "the data is absent for every
  reader" (fix belongs in the smoke's copy) and "Pane's item-level read path misses
  it" (fix belongs in `files()`); needs an interactive check on an affected host.
- **The outcome of 38023610572's rerun** (still in flight when I stopped looking)
  and of any rerun of 37933807886 / 38020412941 (none recorded).

## 7. Feedback-loop note (diagnosing-bugs skill)

No local red-capable loop could be built: the bug lives in the macOS pasteboard
environment of GitHub's macOS runners, the work was read-only, and no macOS
desktop is reachable from here. The existing loops that are already red-capable on
this exact bug are (a) `crates/pane-core/tests/clipboard_adapter_macos.rs::the_watcher_reports_this_tests_changes_until_dropped`
under `PANE_TEST_REAL_CLIPBOARD=1` on a macOS runner (red on affected hosts since
2026-10-08, per #231), and (b) the smoke step itself. Everything above is
differential log/artifact analysis around those loops; the proposed primary fix
should be validated by making (a) and (b) green on an affected runner, ideally
after adding the pasteboard-state dump of §6 to keep the evidence for the next
diagnosis.
