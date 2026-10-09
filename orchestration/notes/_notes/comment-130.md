Landed on `main` in PR #271 (merge commit `2ae9e598`) as `84834bdfce004e2da8bc1933ca178b5f5ba6f44b`, with the review fixes in `c8737999` and the verify follow-ups (`e75e3e7a`, `4a6d01a3`, `9256aa4e`, `79d03f59`).

Results, per acceptance criterion (all run on the verify tier, [37881048674](https://github.com/pane-app/pane/actions/runs/37881048674) — Windows and Linux test shards green; the run's only red is the macOS pasteboard flake #231):

- A token reads back unchanged and the file's bytes lack its text — `tests/credentials.rs::a_token_reads_back_and_is_written_as_this_system_protects_it` (Windows).
- A version-1 file converts to version 2 at start with its tokens readable — `a_version_1_file_is_converted_at_start` (Windows).
- A conversion whose write fails keeps version 1 working and is retried at the next start — `a_conversion_that_cannot_be_written_is_tried_again_at_the_next_start` (Windows; it blocks the atomic write by holding the file open).
- A damaged value gives the explaining error from `credentials.get`, other values still read, `set` replaces it, Pane never drops it — `a_damaged_token_is_explained_kept_and_replaced_by_signing_in` and `extension_data.rs::a_value_that_cannot_be_read_is_explained_and_kept_until_replaced` (Windows).
- Manage extensions counts "… 1 unreadable" — `manage_extensions_counts_an_unreadable_credential` (Windows, on a retained identity).
- Clear cache, uninstall and retained-data deletion work without decrypting — the existing `clear_cache.rs` and `uninstall.rs` checks now run over the protected file; reads, counts and removals use keys and stored forms only.
- An unknown version is refused and not overwritten — `a_credentials_file_of_an_unknown_version_is_refused_and_kept`, plus unit tests for each kind's version rule.
- Clipboard history: a kept item reads back, the file holds no plain text, an earlier file converts at start, a damaged item is explained while others read — `tests/clipboard.rs::the_history_is_encrypted_on_disk_and_a_damaged_item_is_explained` (JS and TS, Windows) and `clipboard/history.rs` unit tests.
- macOS and Linux: the existing credential and clipboard history tests stay green (they ran in the Linux shards; the files stay version 1 there).
- Documents state the protection per system and what it does not protect against — `docs/extension-data.md`, `wit/data.wit` (mirrored into the Rust SDK's copy in `79d03f59`), `guests/js/data.d.ts` (its sample components rebuilt and committed in `9256aa4e`), `docs/clipboard-history.md`, `docs/platforms/windows.md`, `docs/current-decisions.md`.

Not covered here and still pending, as the spec plans: the DPAPI behaviour under a real password reset, a real copied data folder and the Windows smoke's credential phases (release validation, #84); the macOS Keychain and Linux Secret Service slices (later, flagged slices; the documents say macOS and Linux keep the user-only file today). No benchmark numbers are involved in this ticket.
