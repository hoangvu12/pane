# spec/188 review fixes — report

Branch `pi-subagent/188-review-fixes` (worktree `pane-wt/t188-review`), from spec/188's head 28333ba9.
Commits: 7401e0eb (#192: 10, 11, 12, 16, 17, 18), a7338b42 + 683511ec (#192: 9), 8b2bfa91 (#189: 1, 2, 3, 13, 14, 15), 9cf67ab8 (#191: 4, 5), c6c5a0c4 (#190: 6, 7, 8).
Nothing compiled, run or pushed. All code is written but unverified, and all tests are written but not run.

## Per item

1. **Fixed.** `measure-linux.sh` skips only the hidden phase when `target/dist/artifacts/pane-defaults.json` is missing. It says so on stderr and records `"skipped": ["hidden-idle"]` in record.json. `proc_tree.py check` reports that phase as `skipped` and does not fail it. The hidden phase serves links to the payloads beside a copy of the index with its `application` entry removed, so the 99.0.0 package the smoke leaves is not offered as an update.
2. **Fixed (docs + warning).** resource-measurements.md says what the scratch `LOCALAPPDATA` leaves out of the number: the ClickOnce store (icon lookups, `click_once.rs`) and the `%LOCALAPPDATA%\Packages` watch (`start_menu.rs`). The script warns loudly, but does not stop, when another `pane` process runs. It still never touches that process. Not done: the Windows script still serves the index as it is, so it may offer an update. This is documented.
3. **Fixed.** `proc_tree.py` normalises thread names to their first 15 bytes in `wakeups` on both platforms (`thread_key`) and adds a `"names"` note. The selfcheck covers a long name, and the doc is updated.
4. **Fixed.** A `.lnk` that names an icon location now fingerprints its target too, because extraction falls back to the target. The Windows adapter test checks that the target is covered. applications.md is updated.
5. **Fixed.** The worker now rests after every background batch, including batches that only read fingerprints. On Linux, fingerprints share one `ThemeLookup` for 2 s. It reads the theme chain once and each theme's folders lazily, once, instead of reading the config and every `index.theme` for each application. `IconThemes::find` uses the same lookup. A unit test is added in theme.rs.
6. **Fixed.** `InFlight::drop` now has `debug_assert!(calls > 0 || thread::panicking())`.
7. **Partially.** A new runtime_timers test checks that a service cycle wakes both timers and that they wait again between cycles. It uses sample-service with the system clock, 1 cycle/s. The module doc notes that every `send` counts as in flight. Not added: CloseView and search. CloseView applies only to custom views, and I did not confirm that sample-settings opens one. sample-settings has no root search.
8. **Fixed.** The race test's doc now says it is a smoke test and that the wait/call lock discipline is what rules out a lost wake-up. It also says neither round type forces the interleaving. No seam was added.
9. **Fixed (unverified, Unix only).** The handler now only writes one byte to a self-pipe (O_NONBLOCK, CLOEXEC), and it saves and restores errno. Thread `pane-signals` runs the registered clean quit (`diagnostics::quit_cleanly_on_signals`, wired in `pane/src/main.rs` to `Launcher::quit_cleanly`) on its own thread, waits at most 2 s, unlinks the marker, restores SIG_DFL and re-raises. A second signal while the first is being handled is ignored, because the session end sends SIGTERM and SIGHUP back to back. If there is no pipe or thread, the handler falls back to the old direct unlink and re-raise. The signal path runs only `quit_cleanly`, not the tray, hotkey or runtime quit hooks; this is documented. pausing.md and clipboard-history.md are updated. No automated test: raising a signal would kill the test process.
10. **Fixed.** On a failed write, `write_latest` re-arms `due` with a backoff when the thread runs: 500 ms, then doubling, capped at 60 s. `failed_writes` resets on success. The expiry thread now has an `Ended` drop guard that clears `running`, including on a panic. `running` is set before spawning, and `ended()` is called if the spawn fails. Unit test: `a_failed_batched_write_is_tried_again`. The doc is updated.
11. **Fixed.** A refused reading uses one shared empty `Arc` (`Shown::refused`). Unit test added.
12. **Fixed.** The clean-quit test waits for earlier writes and records the write count. It asserts "batch still waiting" and "quit wrote it" only when the quit came within 500 ms of the copy. A slower machine skips that check instead of flaking.
13. **Fixed.** docs/agents/ci.md has a "resource workload" bullet covering the added time (about 2 min, at most about 12), the dependence on the smoke's payloads, the dropped application entry and the skip. The ci.yml step comment is updated to match.
14. **Fixed.** `kill … 2>/dev/null || true`. `PANE_TEST_FILE_INDEX_HOME` is assigned first and exported after.
15. **Fixed.** The C# sampler compiles under `PaneMeasure<sha256 of source, 8 bytes>`, created only if that type does not exist yet. Calls go through `$measure::`.
16. **Fixed.** `State::change` gathers images only when some item holds one. Afterwards it looks each one up (`names_image`) instead of building a second set.
17. **Fixed.** `HistoryStore::id()` (process-unique) is part of the `Projected` key. The cache is dropped when `clipboard_history()` finds the view closed, which the window calls as the screen changes. Unit and integration checks added.
18. **Fixed.** `capture` now pokes once, through a new `batch()` that returns whether a poke is needed.
19. **Not done.** No item here was a cheap or safe change without compiling. Refactoring `Shown`/the view touches the public view type. `kept_on_disk` exists only in clipboard_view.rs; only the 5-line `written` is duplicated. The `until` helpers live in different crates (unit vs integration) and have different signatures.

## Files touched
.github/workflows/ci.yml; crates/pane-core/src/{applications/icons.rs, applications/icons/theme.rs, applications/icons/windows.rs, clipboard/history.rs, diagnostics.rs, launcher/clipboard_view.rs, runtime/deadlines.rs}; crates/pane-core/tests/{application_icon_adapters.rs, clipboard_view.rs, runtime_timers.rs}; crates/pane/src/main.rs; docs/{agents/ci.md, applications.md, clipboard-history.md, pausing.md, research/resource-measurements.md}; scripts/{measure-linux.sh, measure-windows.ps1, proc_tree.py}.

## Check first when compiling
- `diagnostics.rs` `signals` (Linux and macOS only; the Windows build does not compile it): `libc::__errno_location` / `libc::__error`, `(&raw mut byte).cast()` into `read`/`write`, `fcntl` variadic calls, and the `Mutex<Option<Box<dyn FnOnce() + Send>>>` static. Also check that `Launcher` is `Send + 'static` for `quit_cleanly_on_signals` in main.rs. It should be, because launcher clones already move into std threads.
- history.rs: `Ended(&wake)` deref coercion from `&Arc<Wake>`, and borrowck on `State::change` (`file.items()` / `names_image` while `file` is `&mut`). Also check the test's use of the private `store.wake.wait_write`/`ended`.
- clipboard_view.rs: the `static NONE: OnceLock<Arc<[ClipboardRecord]>>` needs `ClipboardRecord: Send + Sync`. Also check the `let mut state` / let-else in `clipboard_history()`.
- theme.rs `ThemeLookup` (OnceLock inside a Vec) and the Linux `themes()` static cache in icons.rs.
- runtime_timers.rs's new test assumes sample-service cycles each second by the system clock with no other request in flight between cycles. Check `PackageIdentity::key()` and the "Installed Service sample" status text.
- Scripts were not run. measure-linux.sh's hidden phase body is deliberately left unindented inside `if`. The proc_tree.py selfcheck expects the new `"names"` key and skipped behaviour.

Prebuilt JS/TS samples and guests: untouched; no rebuild needed.
