Landed on `main` in PR #271 (merge commit `2ae9e598`) as `95c8dd9daf0c9f570ada42365baaec2780ef8dbb`, with the review fixes in `c8737999` (rotation robustness, word-boundary redaction, URL redaction, the WRITING guard, redacted Copy diagnostics, kept-kind counting) and the verify follow-ups in `e75e3e7a` and `4a6d01a3`.

Results, per acceptance criterion (unit tests in `pane-core/src/diagnostics/*`, launcher/window tests in `crates/pane/tests/crash_record.rs` and `tray.rs`; all green in the verify run [37881048674](https://github.com/pane-app/pane/actions/runs/37881048674), whose only red is the macOS pasteboard flake #231):

- Redaction replaces the home path and the user's and computer's names in any case; names shorter than 3 characters are left alone — `diagnostics/redact.rs` (the review pass made matching whole-word/path-component and left the identity-free names `admin`, `root`, `user`, `pane`, `localhost`, `administrator` alone).
- Rotation at the size cap, at most 5 older files — `log.rs::the_log_rotates_at_its_size_and_keeps_at_most_five_older_files`, plus a log that cannot move keeps the older files.
- An 11th line from one site within a minute is held back with one "left out" line — `log.rs` rate-limit tests, flushed at a clean finish.
- The marker's decisions with a fake process table — `diagnostics/marker.rs`: ended process → unexpected end; running → left alone; reused id with another start time → ended.
- A stale marker shows the notice row in root search, dismissing removes it — `crash_record.rs::a_start_after_a_crash_lists_the_notice_and_dismissing_it_removes_it` (dismissed through the Actions panel's "Dismiss Notice" entry).
- A clean quit through the tray's Quit leaves no marker — `tray.rs` (runs on every system).
- A diagnostic that would name a query or a clipboard item never puts that text in the log — `clipboard/history.rs` and `launcher/icon_loads.rs` tests; the web-image fallback names `host:port` only.
- Every stderr message still reaches stderr and the log through one path — `diagnostics.rs::nothing_else_writes_to_standard_error` scans both crates' `src` and fails on any other `eprintln!`/`eprint!`/`stderr()`; it ran green on Linux and Windows.
- A panic's message and location appear in the log — `a_panic_is_written_to_the_log_with_its_message_and_location`.
- The About page shows the notice and Copy diagnostics includes the log's location (redacted) — `crash_record.rs::the_about_page_shows_the_notice_and_the_diagnostics_name_the_log`.

Native evidence still pending (release validation, #84): `WM_ENDSESSION` on Windows, macOS's termination notification, SIGTERM/SIGINT/SIGHUP on a real process (the handlers are installed and unit-tested, but the system's delivery is not exercised), a real crash leaving a marker, and Console showing `~/Library/Logs/Pane`.
