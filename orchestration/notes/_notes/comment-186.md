Landed on `main` in PR #273 (merge commit `d5534f45`) as `8a8723568e67d32b4363adf79b90e99260db4bbc`, with the review fixes in `047ef515` (the `.git/info` re-check, the re-check queue in turns, a hung folder's entries kept, repositories watched with their folders on Linux) and `86e52687`.

Results, per acceptance criterion (all in `crates/pane-core/tests/file_index.rs` and the file index's unit tests, run green in the Windows and Linux shards of the verify run [37889559123](https://github.com/pane-app/pane/actions/runs/37889559123)):

- Adding a `.gitignore` line hides the matching file after the index settles and removing it shows it again, across several batches before, between and after — `a_gitignore_line_hides_what_it_matches_and_removing_it_shows_it_again`.
- A new `.git` folder starts applying its ignore rules — `a_new_repository_starts_applying_its_ignore_rules` (made and deleted, with later batches), plus the unit tests for re-checks queued from catch-up and the repository map.
- Changing the user's excluded patterns applies without a restart, across batches — `the_users_excluded_patterns_apply_without_a_restart_across_batches` (the coordinator builds a new `Scope`; the global ignore is noticed by size and mtime once per batch).
- The check at Enter shares the cache — `a_file_an_ignore_file_hides_since_it_was_found_is_explained_not_opened` (`Indexer::checked` uses `admits_kept`, with a fresh `Scope` when the global ignore changed).
- `docs/files.md`'s "Catching up and watching" has "The ignore rules are kept between batches": what is kept, who uses it, and every rule that drops it (a changed folder, an overflow, watching starting, a quarantine, a global-ignore change, the 20,000-folder cap).
- CI: quick tier ([37888447203](https://github.com/pane-app/pane/actions/runs/37888447203)) and verify run ([37889559123](https://github.com/pane-app/pane/actions/runs/37889559123)) green on every leg.

The unticked row is the 10 ms re-index p95 and the 1 s visibility on the user's machine: no numbers were taken (ask first; the machine is also used for games). The benchmark's re-index rows were rewritten to copy the indexer's real per-batch steps, so the numbers will mean what the shipped code does when they are taken.
