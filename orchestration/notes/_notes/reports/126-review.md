# spec/126 review fixes: report

Branch `pi-subagent/126-review-fixes` (worktree `pane-wt\t126-review`), based on spec/126's head `c2851c2c`. Three signed-off commits:

- `63fa5a4e`: #186 correctness and #184 volumes (items 4–12, 17, 19–21).
- `6936b1e9`: store (items 13–15, plus the tests for 16).
- `918c790b`: benchmark, xtask, CI comment, CONTEXT.md and docs (items 1–3, 18, 22, 23).

Nothing was compiled, run or benchmarked (RULES.md). All tests are written, not run.

## Item by item

### #183, the benchmark

1. **Fixed.**
   - `FileIndex::hold_merges` is now `pub`. It moved out of the `cfg(test)` impl and is documented as for the benchmark and tests.
   - The benchmark holds merges off while it streams and times the "several segments and changes in memory" shape: 5 segments plus the memtable, as #183 asks. It lets them go for #187's "while changes arrive" row.
   - Row labels are unchanged. `STREAM_BATCHES` docs, the printed note, docs/files.md and ci.md are updated.
2. **Fixed (comment only).** It now says what is sampled: the first entry of each batch, which is each folder walked, and every 400th entry after it.
   - The code was kept, so the query set stays the same as #174/#183's.
3. **Fixed.**
   - The deep row keeps its label and its #186 meaning (the folders above it kept).
   - A new row, "…deep folder under ignore files, nothing kept (every folder above read again)…", runs `reindex(…, nothing_kept: true)`. It calls `forget_all_kept()` before each timed run.
   - The guard checks the new row too.

### #184, volumes

4. **Fixed, partially.**
   - `volume::ask_within` asks on a helper thread `pane-volume-kind` and waits at most `VOLUME_ANSWER` (2 s). No answer in time means `Network`.
   - The answer is cached per root with `answered: false`. `Scope::roots_not_answering()` lists those roots, and the coordinator puts them in the status reason at open.
   - Mount checks (`Scope::volume_kind`) use the same bounded call.
   - **Left:** with the switch on, a timed-out root is reconciled like a share. `reconcile` stats the root itself (`meta_at`) with no time limit, so a share that stays stalled can still block the coordinator there. Only listing is bounded. This is documented under Limits.
5. **Fixed.**
   - `root_volume` asks only once the root is a directory. A root that is away answers `Local`, uncached, so it keeps its entries, and is asked again later.
   - `roots_away` (search) now also hides the entries of roots the rules leave out. That covers a removable drive plugged in mid-run: its entries are hidden, and they go at the next open.
17. **Fixed.**
   - New `VolumeKind::Unknown`. A failed statfs, a path that is not a valid CString, or Windows with no volume root gives `Unknown`.
   - Unknown counts as "another volume": left out unless the switch is on, for mounts and for existing roots.
   - Windows `drive_kind` mapping is unchanged (0/1 → Local).

The two user decisions (the Linux root check; added roots left out unless the switch is on) were not touched.

### #186, ignore-context cache

6. **Fixed, with stated limits.**
   - `resolve` now also records:
     - `CatchUp::gone`: (folder, id) of each nameless deleted entry;
     - `CatchUp::unresolved_folders`: parent ids of unresolved records.
   - NTFS re-checks a folder when an entry left it that the index did not hold and whose id names nothing now (`journal::gone_unheld`). That catches a deleted `.gitignore` or `.git`.
   - It looks up each unresolved folder's full path by id (`Names::path`, new `imp::path_by_id`), at most 4,096 per catch-up. When the path is `X/.git/info` (`repository_of_info`), it re-checks `X`.
   - It walks a touched folder that the index did not hold as a folder (for example, its hidden attribute was removed).
   - `Caught::Changes` gained `recheck`.
   - **Deliberately narrower than suggested:**
     - Only `.git/info` counts, not every unresolved record under `.git`. Git's own work there (objects, refs, index) would re-check whole repositories after every commit.
     - "Every folder in `listed`" was not re-checked. Every atomic save or Office lock file would re-walk the folder, often the whole home folder, at every start.
   - **Remaining limits (documented):**
     - An ignore file renamed away while Pane was stopped.
     - Lookups past 4,096 folders.
   - The "drop its kept context" part is moot at catch-up: everything kept is dropped once watching starts.
   - docs/files.md's claim is corrected.
7. **Partially fixed.**
   - `Scope::keep_nothing_under` / `kept_nothing_under` (an `unkept` set). `admits_kept` reads fresh under those folders, so the check at Enter does too.
   - Folders added to the set:
     - network roots, at open;
     - Linux `Unwatched` folders;
     - every kept root when `watch()` fails;
     - watched roots that are away when watching starts.
   - The set carries over to rebuilt scopes (`scope_with`).
   - A reconcile pass re-checks a folder it reads again (its time changed) that holds an ignore file or repository (item 12's mechanism).
   - **Not done:** force-re-reading every unwatched or network folder on every 5-minute pass. That would read the whole tree each time. An ignore file edited in place in such a folder is documented as a limit.
8. **Fixed (Linux).**
   - `add_folders` also watches `folder/.git/info` when it exists. If that watch fails, the folder is reported unwatched, so nothing is kept for it.
   - When a repository is made while Pane runs, the coordinator calls `add_folders` again for its folder.
   - Test `changes::linux::tests::a_repositorys_own_ignore_rules_are_watched_with_its_folder`, plus the integration test `a_repositorys_own_exclude_file_applies_while_pane_runs`.
9. **Fixed.**
   - Re-checks are queued in `Coordinator::recheck` (`queue_recheck` dedupes against folders above and below).
   - `do_walks` reads `RECHECK_FOLDERS` (64) folders per turn through `recheck_some`.
   - `run` then polls the channel with a zero timeout and handles any batch before the next turn.
   - Each turn's changes are applied immediately, and live changes re-read the disk, so ordering is safe.
   - Re-checks wait for the launcher to be shown or 60 s, like other walks.
10. **Fixed.** After an overflow the root is still reconciled at once, and is also queued for a full re-check.
11. **Fixed by construction.** The re-check now uses reconcile's comparison, whose `hung_folders` is uncapped. Hung folders keep their entries. Test `reconcile::tests::a_recheck_keeps_what_a_folder_that_does_not_answer_holds`.
12. **Fixed, with what remains documented.**
   - `Reconciled::recheck` lists folders read again (time changed) that hold `.gitignore`, `.ignore` or `.git`.
   - It is fed back from Linux's catch-up, the do_walks reconciles and the periodic reconcile.
   - **Remaining:** an ignore file deleted, or edited in place, while Pane was stopped.
20. **Done.** `recheck_folders` and `outermost` are gone. A re-check is `reconcile::recheck`: the same `compare` with `read_all` and a budget.

### #185 / #187

13. **Fixed.**
   - `oldest_run_due`: when no tier merge is due and the segments above the oldest carry more than max(4,096, oldest/32) tombstones, all segments are merged together. This runs on the merge thread.
   - Deletion records are not counted: there is no per-segment count without a format change.
14. **Fixed.**
   - New `segment::write_unless(…, cancel)`. It returns `Interrupted` right after the docs loop, before sorting and fsync, and deletes the temporary file.
   - `write_merged` uses it, so closing waits only for the item in hand.
15. **Fixed.**
   - `catch_unwind` around `merge_due`. A panic prints `eprintln!("pane: a merge of the file index's segments panicked")`, the same idiom as generation.rs, and breaks the turn. The `Turn` guard marks it not running, and the next turn runs.
   - A cfg(test) `Turns::panic_next` hook drives the test.
16. **Done, as targeted unit tests.** There is no constant override.
   - `a_short_word_stops_gathering_entries_at_its_limit`: 50,010 distinct `a…` words, checked in the memtable and in a segment.
   - `a_query_matching_more_entries_than_it_reads_still_finds_the_best_first`: 1,201 matches, `candidates` capped at 1,000, the newest included and first.

### Standards

18. **Fixed.** CONTEXT.md's "Index scope" now names network shares and removable drives (left out by default), and says an included share is reconciled, not watched.
19. **Fixed.** In `catch_up`, the watch's Vec is now `watched`; `indexed_folders`' parameter is `table`.
21. **Fixed.** `IndexerConfig::scope(rules)` and `Coordinator::scope_with(rules)` (which also carries over the unkept set) are used in open, quarantine and the global-ignore rebuild. `checked()` keeps its own one-off fresh scope.
22. **Fixed.** `CARGO_TARGET_DIR`, then `CARGO_BUILD_TARGET_DIR`, then `target`. A comment notes that a config-file `build.target-dir` is not read (Pane's sets none).
23. **Fixed.** The tests/file_index.rs module doc is rewrapped, and the ci-branch.yml header is reflowed.

## New tests (written, not run)

- **reconcile:**
  - a re-check reads a few folders at a time under the rules as they are now (an in-place `.gitignore` edit);
  - a folder read again holding an ignore file is named for a re-check;
  - a re-check keeps a hung folder's entries.
- **indexer:**
  - an overflow re-checks an in-place ignore edit;
  - a folder the catch-up asks to re-check is re-checked;
  - a home `.ignore` re-checks 128+ folders in turns while a change meanwhile is applied;
  - the check at Enter re-reads the rules of an unwatched folder.
- **scope:**
  - nothing is kept of a folder not watched live;
  - a root away is asked again once back;
  - a volume that does not answer, or answers Unknown, is left out (about 4 s, two 2-s timeouts).
- **volume:** `ask_within` (away, kind, no answer); on Unix, a missing path is Unknown.
- **journal:** `gone`, `unresolved_folders`, `gone_unheld`, `repository_of_info`.
- **store:** the oldest-run rule, a panicking merge, the short-word stop, the candidates cap.
- **segment:** a write given up leaves no file.
- **linux:** `.git/info` is watched.
- **tests/file_index.rs:** `.git/info/exclude` applies while Pane runs.

## Files touched

- `crates/pane-core/src/file_index.rs`
- `crates/pane-core/src/file_index/changes.rs`
- `crates/pane-core/src/file_index/changes/linux.rs`
- `crates/pane-core/src/file_index/changes/macos.rs`
- `crates/pane-core/src/file_index/changes/ntfs.rs`
- `crates/pane-core/src/file_index/indexer.rs`
- `crates/pane-core/src/file_index/indexer/tests.rs`
- `crates/pane-core/src/file_index/journal.rs`
- `crates/pane-core/src/file_index/reconcile.rs`
- `crates/pane-core/src/file_index/scope.rs`
- `crates/pane-core/src/file_index/segment.rs`
- `crates/pane-core/src/file_index/store.rs`
- `crates/pane-core/src/file_index/volume.rs`
- `crates/pane-core/tests/file_index.rs`
- `crates/pane-core/examples/file_index_bench.rs`
- `xtask/src/main.rs`
- `.github/workflows/ci-branch.yml`
- `CONTEXT.md`
- `docs/files.md`
- `docs/agents/ci.md`

## Check first when compiling

- **`cargo fmt`.** Formatting was done by hand. Most likely to differ:
  - the `let (Some(known), Some(path)) = … else {` in `Scope::root_volume`;
  - the `match` guard in `ntfs::newly_admitted`;
  - the `lock(..).status.reason = Some(format!(…))` block in `Coordinator::open`;
  - the `sets_rules` closure.
- **Closure-to-dyn coercions:**
  - `&mut |folder: &std::path::Path| … collect::<HashSet<u64>>()` and `&mut |id| names.name(id).is_some()` passed as `&mut dyn FnMut(…)` (ntfs.rs, journal tests);
  - `volumes(path)` called through `&Arc<dyn Fn>` in `ask_within`'s fallback.
- **Smaller API points:**
  - `.find_map(std::env::var_os)` in xtask;
  - `std::mem::take(&mut self.merges.turns().panic_next)` under `#[cfg(test)]`;
  - `HashSet<PathBuf>::contains(&Path)` in `admits_kept`.
- **Windows only (compiled only there):** `Names::path`, `imp::path_by_id` / `units_by_id`, `imp::volume_root` made `pub(super)`, and `PathBuf::join` of the id path.
  - Unconfirmed: that `FileNameInfo` gives the path below the volume root in the same letter case as the roots. If not, the `.git/info` lookup finds nothing, which is the old behaviour.
- **Clippy:**
  - `reconcile::compare` has 7 arguments (at the limit);
  - `#[cfg_attr(not(windows), allow(dead_code))]` on `gone_unheld` / `repository_of_info`.
- **Test timing:**
  - the store short-word test (50k entries applied, then flushed, in debug);
  - the scope volume test (about 4 s);
  - the indexer home-`.ignore` test (several re-check turns).
  - The panicking-merge test prints a panic message by design.

## Prebuilt samples and guests

No rebuild is needed: no WIT, guest, SDK or JS/TS sample or toolchain pin changed.

## Acceptance items still open

- All benchmark numbers (#183/#186/#187 targets): need numbers, with the user's consent.
- CI branch tier and `ci-fast` verify: need CI.
- Native evidence for shares and USB drives: not done.
