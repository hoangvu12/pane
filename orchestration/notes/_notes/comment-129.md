Landed on `main` in PR #271 (merge commit `2ae9e59845e1ac2c07d48f4b9231c84e96a238a5`), which keeps every ticket's commit reachable:

- #182 — `af9ace44d01e4ec079ec9fa44646cb064de2ce0d`
- #131 — `36f87602a680c0286153a199f4f6a669067c1a71`
- #130 — `84834bdfce004e2da8bc1933ca178b5f5ba6f44b`
- #132 — `e4b6bf0f424d2ae34d1056147c11f97b40467b2c`
- #133 — `95c8dd9daf0c9f570ada42365baaec2780ef8dbb`
- review and CI follow-ups — `c8737999`, `9e0f24ae`, `e75e3e7a`, `4a6d01a3`, `9256aa4e` (the rebuilt JS/TS samples), `79d03f59` (the Rust SDK's copy of `wit/`, for the `sdks` check #220 added)

CI: the quick tier is green on the final head ([37880435160](https://github.com/pane-app/pane/actions/runs/37880435160)); the verify run [37881048674](https://github.com/pane-app/pane/actions/runs/37881048674) is green on Linux and Windows (lints and all six test shards) and its only red is the known macOS pasteboard flake #231 (`Check (macos-15)` fails only `clipboard_adapter_macos::the_watcher_reports_this_tests_changes_until_dropped`; the macOS compile of the tray and hotkey adapters passed) — the same standard #213's finish used.

Still open beyond this merge, by design:
- Native evidence (Explorer restart, screen readers, real crash/logoff, real share/USB behaviour) belongs to the release-validation pass #84; the tray and screen-reader phases are recorded in `docs/evidence/settings-79/native-validation.md`.
- The macOS Keychain and Linux Secret Service slices for credentials were explicitly out of scope (later, flagged slices).
- The macOS pasteboard flake #231 is a runner-pool problem, not Pane code; it also fails `main`'s release runs.
