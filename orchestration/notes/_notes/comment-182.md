Landed on `main` in PR #271 (merge commit `2ae9e598`) as `af9ace44d01e4ec079ec9fa44646cb064de2ce0d`. Documentation only: `docs/hotkeys.md` and `docs/current-decisions.md`.

Checked against the code, as the acceptance criteria ask (paths at that commit):

- **Defaults** — `crates/pane-core/src/hotkeys.rs` `Shortcut::open_pane_default`: `alt+space` on macOS, `ctrl+alt+space` otherwise; `crates/pane/tests/shortcuts.rs` asserts both.
- **Where it is set and kept** — Settings' General page (`crates/pane/src/features/settings/general.rs`, "Open Pane hotkey" row), recorded in `settings.json`'s `open_pane` field (`crates/pane-core/src/host_settings.rs`), registered through the launcher at every start and on change, with a failed write rolled back (`crates/pane/src/settings.rs`). Independent of extensions: `crates/pane/tests/open_pane.rs` `the_hotkey_stays_available_while_the_runtime_has_failed`.
- **Recording and reset** — Enter/Space start, Escape/Tab/Shift-Tab and a click outside cancel, reset goes through the same checks and is offered only when not at the default.
- **Summon/focus/hide** — hide only when shown and active, otherwise summon (`crates/pane/src/app/presence.rs`), tested in `open_pane.rs` (toggle, held key, Settings focus).
- **Refusal wording** — the command side: "`{shortcut}` opens Pane itself: choose another shortcut for `{title}`, or change Pane's hotkey in Settings." (`crates/pane-core/src/launcher/hotkeys.rs`, asserted in `shortcuts.rs`); the Open Pane side: taken by a command, taken by another application, and the platform's own refusal text; a binding not registered shows "Not active: `<problem>`".
- **Wayland** — the adapter's own text, shown on the General page and on a refused recording (`open_pane.rs`).
- **Shortcuts page (#76)** — the Hotkey column records through `Launcher::set_hotkey` with the same checks and the same `hotkeys.json` (`crates/pane/src/features/settings/shortcuts.rs`); the extension page uses the same column.

One reading the document records: the old "Manage extensions" screen was removed by #168/ADR 0043, so command hotkeys are recorded on the command's extension page in Settings or inline on the Shortcuts page; the document says so. ADR 0039 is quoted as text with an issue link for #125 (the ADR file itself is not on `main` yet).

CI: the milestone's quick tier ([37880435160](https://github.com/pane-app/pane/actions/runs/37880435160)) and verify run ([37881048674](https://github.com/pane-app/pane/actions/runs/37881048674)) are green for the branch this landed through (Linux/Windows lints and tests; the verify run's only red is the known macOS pasteboard flake #231). As the criteria say, no verify run or release matrix is needed for this documentation change on its own.
