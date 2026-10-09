# spec/129 review-fix pass (review-129.md, 28 items)

Branch `pi-subagent/129-review-fixes` (worktree `pane-wt\t129-review`), on spec/129's head 23594486.
Commits (all `-s`): 8ca9c809 (22, 23), 4331f528 (1-6, 25, 28), 0f1997c7 (7-11), cb456916 (12-15, 24, 26), e9c08d8a (16-21, 27).
Nothing was compiled, tested, linted or run (RULES.md). Every item below is "written, unverified".

## Items

1. Fixed. `log.rs` `rotate` moves `pane.log` aside (`pane.rotating.log`) first; if that fails nothing older is shifted or deleted and the next try waits until the log grew by another cap (`Inner::rotate_at`). If the aside file cannot become `pane.1.log` it goes back. Test `a_log_that_cannot_move_keeps_the_older_files` (blocks the move with a folder at the aside name). docs/pausing.md.
2. Fixed. Redaction matches only as a whole word / path component (a needle that starts or ends with a letter/digit needs a non-alphanumeric neighbour there); case-insensitive and >=3 chars kept. Also: user/computer names that identify nobody and are words Pane's messages use (`admin`, `administrator`, `localhost`, `pane`, `root`, `user`) are not redacted (a home folder named after one still becomes `~`) — boundaries alone cannot stop a user called "pane" from turning every "Pane" into `<user>`. Tests added; docs/pausing.md and the module doc record it.
3. Fixed (Windows). `smoke-windows.ps1` `Stop-Pane` now removes `running-<pid>.json` from the logs folder (`PANE_DATA_DIR\logs`, else `%LOCALAPPDATA%\Pane\logs`) after `Stop-Process`. Not a defect elsewhere: smoke-linux.sh, smoke-macos*.sh and measure-linux.sh stop Pane with `kill` (SIGTERM), which the signal handler treats as a clean quit (Pane is exec'd directly, also via `env -i`); hud/system smokes start once on fresh folders; capture-pane-windows.ps1 closes the window first. Not run.
4. Fixed. `report_fallback` replaces the exact URL by `host:port`, then every URL-like text (`scheme://…` up to a space, quote or bracket) by `<url>`. Tests for escaped/redirect URLs and `without_urls`. docs/pausing.md.
5. Fixed. `diagnostics::writing()` sets the WRITING guard; `to_log`, `start`'s `begin_run` and `finish` go through it.
6. Fixed. New `pane_core::diagnostics::redacted(text)` (redactor of this system, cached); About's Copy diagnostics redacts the log folder and, for the same reason, the data folder. `crash_record.rs` updated (compares with the redacted path, asserts the home path is absent). docs/pausing.md.
7. Fixed. `kept_now` keeps each kind's read file (`KindKept`); `Kept::describe` counts without decrypting (uninstall confirmation, dependents, delete-retained confirmation); new `Kept::describe_unreadable` tries only the described identity's values and is used by Manage extensions' retained rows. Unit test updated; docs/extension-data.md.
8. Fixed for the reported path. `ExtensionData::remove` protects the re-read file's plain values (outside the lock) before staging; `DataJson::to_json` writes version 1 when every value is still plain on a protecting system (DPAPI encrypted none) instead of plain values in version 2 (an empty file stays v2, as the smoke's `Token-Kept` expects). Integration test `a_removal_after_a_failed_conversion_protects_what_it_writes` (Windows). Partially: a *mixed* file (DPAPI failing for some values only) would still write `{"plain": …}` in v2 until the next start; `set` does not re-protect leftover plain values of other packages (they are protected in memory at start already unless DPAPI failed).
9. Fixed. `HistoryJson::to_json`: an item that cannot be sealed is left out of that write with a diagnostic (reason only), kept in memory and tried again at the next write; the rest (deletions included) is written. Never written plain. Edit kept inside `to_json` (no caller change), for #192. No test: a DPAPI failure cannot be simulated. docs/clipboard-history.md.
10. Fixed. `guests/js/data.d.ts` wording now matches `wit/data.wit`. data.d.ts is a prebuilt input: the JS/TS prebuilt samples are stale (they already were) and need a rebuild later.
11. Fixed. `credentials.rs` damaged-token test first damages another credential key ("other") and checks the token still reads, and that "other" is kept as it was after Sign in.
12. Fixed. `Settings::set_tray_visible`: on `TrayError::Refused` it calls `tray.set_visible(self.chosen.tray_visible)` again (the entry is already in that state, so only `wanted` is reset). The sign-in case is untouched (start path). Unit test `a_refused_change_rolled_back_to_the_preference_follows_it_after_explorer_restarts` (FakeShell gained `refuse_deletes`); `tray.rs` window test now expects the re-applied call and checks a refused show too.
13. Fixed. `icon_guid` hashes the path without the verbatim prefix (`\\?\C:\…` → `C:\…`, `\\?\UNC\s\…` → `\\s\…`); test pins both forms. Pinned GUID values unchanged.
14. Fixed. Broadcasts (`TaskbarCreated`, theme, display) that find the entry borrowed (menu's modal loop) are kept once each in a thread-local `DEFERRED` and applied after the menu returns (end of the TRAY_MESSAGE handler) and after served requests. No unit test (window-procedure level).
15. Fixed. No code keeps the window open (Escape at an empty root hides it whatever the hotkey's state). hotkeys.md now says: the row says "Not active: <reason>"; the launcher opens at start as always, and once hidden the tray/menu bar entry's Open Pane shows it again where there is one.
16. Fixed. Only toasts and outcomes (Result/Error) reach the announcer (`announcer::says_message`, used by app.rs, Clipboard History and Search Files); Running/progress are the strip's own. A selection already waiting keeps its deadline when another message comes (cap = STATUS_LEAD after it was made). Unit tests.
17. Fixed. `Held` records what the list showed (key, selection, field text); at release it is said only if the current listing still shows that, else dropped. Unit tests (panel opened then closed; typing began).
18. Fixed. A list that opens silently (root search) clears the text. Element id: checked, not a defect — both renders put the `announcer` id under the same id'd ancestors (material panel and split-view root divs carry no id), so GPUI's GlobalElementId, hence the AccessKit NodeId, is the same; no change.
19. Fixed. `frame(None)` (form/custom view) clears `over` and typing; a text equal to the current one is cleared for one frame and set the next (`Announcer::say`/`again`, frame answers `Some(now)` so the window draws again at once). Documented in the module doc and root-search.md. Unit tests.
20. Fixed. Command, CommandSearch, Extensions: "<title>, n results". Package, Network/Program/Pause/Build/Runtime details, Confirm, Hotkey: new `Opening::Titled` — the title, then the row ("Uninstall X? Cancel, 1 of 2"; a title ending in ?/./! is not followed by another full stop).
21. Mostly fixed. `several_fast_moves` sends each Down in its own frame, records the text each time, and checks the last text survives after the lead and settle times. Typing's "said once" counts changes over the sampled frames. New tests: a sample's real toast said, then a move said at once (actions sample, "Open: <first note>"); footer menu (focus on "Pane menu", 1 of 1, Down, said again when reopened); Clipboard History in window.rs (field keeps focus, rows' place/size, Down, "No results" for a filter). ", unavailable" in a command's list was already covered in window.rs. Not done: toast-and-selection in the *same* frame with a real toast — the toast arrives from the runtime thread, so the frame order races the key; documented on the test, which keeps setting the message through the launcher. The one-frame clearing is observable only in unit tests (window tests run until parked).
22. Fixed. `#[serde(default = "BTreeMap::new", …)]` on `FileJson::preferences` (confirmed serde_derive 1.0.229 adds `V: Default` for a plain `default`).
23. Fixed. Doc comment moved inside `thread_local!` onto `TEST_SINK`.
24. Fixed. `version_4.then_some(POINT { … })` (cheap, side-effect free).
25. Fixed. `report_line`'s site is the text before the first `:` or `(`. Unit test with a varying reason.
26. Fixed. objc2-foundation features now list `NSGeometry` and `objc2-core-foundation` (NSSize needs both); Cargo.lock already has the edge, no lock change.
27. Fixed. CONTEXT.md: Local credential (protection per system), Unreadable credential, Pane's log, Crash notice, Announcer.
28. Fixed. `pane_core::diagnostics::CRASH_NOTICE`, used by diagnostics.rs, crash_notice.rs and about.rs (tests keep their literal).

## Files touched
CONTEXT.md; crates/pane-core/Cargo.toml; crates/pane-core/src/{clipboard/history.rs, diagnostics.rs, diagnostics/log.rs, diagnostics/redact.rs, extension_data.rs, launcher/crash_notice.rs, launcher/icon_loads.rs, launcher/retained.rs, tray/windows.rs}; crates/pane-core/tests/credentials.rs; crates/pane/src/{app.rs, settings.rs, features/announcer.rs, features/clipboard_history.rs, features/search_files.rs, features/settings/about.rs}; crates/pane/tests/{announcements.rs, crash_record.rs, tray.rs, window.rs}; docs/{clipboard-history.md, extension-data.md, hotkeys.md, pausing.md, root-search.md}; guests/js/data.d.ts; scripts/smoke-windows.ps1.

## Check first when compiling
- extension_data.rs: `default = "BTreeMap::new"`; `KindKept` / `describe_counting`; `to_json`'s new version rule (could change a Windows test expecting v2 for an all-plain file — none found).
- Let-chains with a bool condition: announcer.rs `if let Some(held) = self.held.take_if(..) && held.of == shown`.
- diagnostics.rs `redacted` (`OnceLock<Redactor>`), `writing(|| …)`; `report_line` uses `split([':', '('])`.
- redact.rs `matches_at(before, text, needle)` and the `IDENTIFY_NOBODY` filter closure (`Fn(&&String) -> bool`, copied into two `filter` calls).
- icon_loads.rs `without_urls` (`trim_end_matches`/`find` with closures).
- history.rs `to_json`: `let sealed_only: BTreeMap<String, PackageHistory>;` deferred init.
- tray/windows.rs: `with_entry` now returns bool; `DEFERRED` thread_local `RefCell<Vec<Broadcast>>`; `without_verbatim_prefix` (`strip_prefix` on `Vec<u16>` with a temporary).
- log.rs rotation test depends on 67-byte lines (comment says so).
- window.rs clipboard test uses `super::wait::until`, `super::accessible_nodes`, `super::typing_settles`, `super::until_announced`, `super::a11y::a11y`, and assumes record titles equal the copied texts ("third", "second") and the field's label "Search clipboard history".
- announcements.rs: actions-sample test assumes the first row's primary action is "Open" with toast "Open: <title>"; footer menu opened by clicking `footer-menu`.

## Rebuilds
JS/TS prebuilt samples: stale because `guests/js/data.d.ts` changed (already stale on spec/129). No Rust guest changed.
