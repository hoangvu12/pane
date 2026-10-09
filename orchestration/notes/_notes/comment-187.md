Landed on `main` in PR #273 (merge commit `d5534f45`) as `79aad9351d260390b6bebfd93141f39ceba4652f`, with the review fixes in `047ef515` (the oldest-run tombstone merge, a merge given up leaving no file, a panicking merge caught and reported — now through the diagnostic path in `63f1e8ed`) and `86e52687`.

Results, per acceptance criterion:

- Tests, all in `store::tests` and green in the Windows and Linux shards of the verify run [37889559123](https://github.com/pane-app/pane/actions/runs/37889559123):
  - a merge held just before it puts its segment in place loses none of the changes that arrive meanwhile (one flushed, one in memory), and #185's brute-force reference queries stay current throughout — `changes_arriving_during_a_merge_are_kept_and_queries_meanwhile_are_current`, covering a merge from the oldest segment (tombstones dropped) and one above it (carried on);
  - a restart in the middle of a merge — the index folder copied while the merge is held — opens to a readable index with every change, the unlisted merged segment deleted — `a_restart_in_the_middle_of_a_merge_finds_every_change_through_the_log`;
  - the run choice and the background merging are covered by `a_few_neighbouring_segments_of_similar_size_are_merged_at_a_time` and `many_segments_are_merged_a_few_at_a_time_in_the_background`.
- The folder-id table is read once per open on Windows: `indexer::tests::the_catch_up_and_the_watch_share_one_read_of_the_folder_ids` (every system, through the fake source) and `a_restart_on_windows_reads_the_folder_ids_once` (the real NTFS journal on the runner's volume; both ran green in the verify run's Windows shards). `FileIndex` counts its reads (`folder_id_reads()`).
- `docs/files.md` describes the tiered background merges: the merge thread, the neighbouring-run policy, the locks (the writer never held while writing), tombstones kept or dropped, and a merge cut short by closing.
- CI: quick tier ([37888447203](https://github.com/pane-app/pane/actions/runs/37888447203)) and verify run ([37889559123](https://github.com/pane-app/pane/actions/runs/37889559123)) green on every leg.

The unticked row is the 1 s catch-up after 10,000 changes on the user's machine (no numbers taken; ask first). macOS's catch-up is recorded as not run, as the ticket asks. One cost noted in the report: `Drop` for `FileIndex` joins the merge thread, so a drop mid-merge waits for the item in hand.
