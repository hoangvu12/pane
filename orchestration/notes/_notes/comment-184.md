Landed on `main` in PR #273 (merge commit `d5534f45`) as `b0a345f18d9eda064b2a357a1aa3c9c317981a07`, with the review fixes in `047ef515` (a bounded volume answer, roots away asked again, `VolumeKind::Unknown`).

Results, per acceptance criterion:

- Unit tests through the `VolumeKinds` seam, with the real calls compiled per platform and a fake answer in tests: a share or removable root added or as the home folder is left out by default and included by the switch (`indexer::tests::a_root_on_a_network_share_or_a_removable_drive_is_left_out_until_other_volumes_are_included`, `a_home_folder_on_a_network_share_…`, the scope and walker tests, `volume::tests` for the per-platform classification). An included share is watched never, reconciled periodically; the stick is watched. The macOS leg of the verify run compiled the real `statfs` calls.
- The churn valve counts only paths the rules admit: a burst inside `node_modules`, an ignored folder and a hidden folder never quarantines anything while the same burst in an indexed folder still does (`churn_in_folders_the_index_leaves_out_never_takes_anything_out`).
- `docs/files.md` has a "Network shares and removable drives" section with the per-system table, the scope and walker rules, the catch-up/watch note and the Limits.
- CI: quick tier ([37888447203](https://github.com/pane-app/pane/actions/runs/37888447203)) and verify run ([37889559123](https://github.com/pane-app/pane/actions/runs/37889559123)) green on every leg.

The unticked row is the release-validation one (a real share or USB drive on Windows), which the ticket itself marks as not a merge gate and needing the user's consent.

Two decisions recorded in the ticket and left as the documents say them: added roots on other volumes are left out by default (in tension with spec stories 51/52, which read the other way — still undecided for the user), and the volume check applies on Linux too, so an NFS or FUSE home or an added FAT/exFAT root is left out by default there.
