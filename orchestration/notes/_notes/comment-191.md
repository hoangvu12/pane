Landed on `main` in PR #274 (merge commit `bff03cdd`) as `30959c47cc5e41efbafbea60ff5d902da08e2359`, with the review fixes in `3da35f36`.

Results, per acceptance criterion (all ran green in the verify run [37898374301](https://github.com/pane-app/pane/actions/runs/37898374301), Windows and Linux shards; its only red is the known macOS pasteboard flake #231):

- Core tests with the recording extractor, in `crates/pane-core/tests/application_icons.rs`:
  - a restart over the same cache extracts nothing for unchanged applications — `after_a_restart_an_unchanged_applications_icon_is_drawn_from_the_cache_and_not_extracted`;
  - a shortcut whose target changed is extracted again (and its icon file's growth changes the fingerprint) — `a_restart_extracts_nothing_unchanged_and_a_shortcut_whose_target_changed_again`;
  - a picture past the refresh age (7 days, on a `ManualClock`: nothing at 7 days minus an hour, each picture once at 7 days plus an hour) is extracted again in the background, while the kept picture still draws at once — `a_picture_past_the_refresh_age_is_extracted_again_in_the_background`;
  - an on-demand row still draws from the cache at once (covered by both tests above, with extractions held behind the gate).
- Windows: a shortcut's fingerprint includes its icon location or target — `a_shortcuts_fingerprint_follows_its_target_or_its_own_icon_location` ran in the Windows shards; the packaged-app adapter's fingerprint covers the manifest and the logo files (also asserted in the Windows shards); the app-bundle and desktop-entry adapters' shared code is compiled and unit-tested on every system, and the macOS adapter compiled in the verify run's macOS leg.
- An old cache index is read without loss — `an_index_an_earlier_pane_wrote_is_read_with_every_picture_old` and `an_index_an_earlier_pane_wrote_is_read_without_loss` (`INDEX_VERSION` stays 3; `Kept.extracted` is `#[serde(default)]`).
- `docs/applications.md` describes the new refresh rule (the per-system fingerprint list, the week age, the background refresh, the one-time re-extraction after the upgrade).
- CI: quick tier [37896410002](https://github.com/pane-app/pane/actions/runs/37896410002) and the verify run above, green.

Cost note for the record: because the fingerprint format changed, every cached icon is re-extracted once after this lands; that coincides with the old index's pictures reading as old anyway.
