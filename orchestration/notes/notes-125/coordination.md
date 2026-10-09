# Spec #125 coordination notes (for implementers and the merge)

Shared conventions so four parallel tickets merge cleanly. Keep these shapes identical; the parent resolves conflicts by intent.

## WIT interface names

- `pane:extension/system` is **taken** by ADR 0037's system interface (`wit/system.wit`: copy/paste/front-application/selected-text, imported by every command). The front application, paste and selected text are implemented **inside** it (#253, #262). It is not the system-commands capability.
- New capabilities this spec adds (each its own `wit/<name>.wit`, package `pane:extension@0.1.0`, guest world `<name>-user`, composed onto the hosted world ladder as `programs`/`applications`/`clipboard` did — today's hosted top world is `extension-with-file-index` in `wit/file-index.wit`):
  - #254/#264: `pane:extension/run` (interface `run`) — run host function, Win+R history.
  - #255/#265/#266: `pane:extension/system-commands` (interface `system-commands`) — session, power, volume, mic, bin, appearance, devices. (The spec text says `pane:extension/system`; that path is taken, hence `system-commands`. Deviation recorded in the PR.)
  - #263: `pane:extension/windows` (interface `windows`) — window list and activate.
- Any new WIT file must also be added to `tools/componentize-js/pane_js.py`'s `PANE_WIT` list and mirrored to `guests/pane-extension/wit/` and `guests/js/wit/deps/pane-extension/`.

## New default extensions (ADR 0040)

- Ids: `run`, `system-commands`, `switch-windows` (package id = guest package folder under `guests/packages/`).
- Each declares `"platforms": ["windows"]` (package level in `pane.json`).
- Each joins **only the Windows default set**: `crates/pane/src/lib.rs::default_extensions()` becomes platform-aware (Windows lists the eight, other systems keep the five). Update the pinned test there. Same entry shape for all three: `pane_core::DefaultExtension { id: "run".into(), title: "Run".into() }`.
- `guests/Cargo.toml` workspace members: `run`, `system_commands`, `switch_windows` (snake_case crate names).
- `xtask/src/main.rs`: components list + `SAMPLE_PACKAGES` entries `("run", "run")`, `("system-commands", "system_commands")`, `("switch-windows", "switch_windows")`.
- `xtask/src/package.rs`: `DEFAULTS` grows to the eight, same shape.

## Shared constants and env vars

- Pane's injected-input tag (`dwExtraInfo` on SendInput): `pub const INJECTED_TAG: usize = 0x50414E45;` ("PANE"). #252 puts it in `crates/pane-core/src/hotkeys.rs` (the hook passes tagged events). If #253/#262 need it before #252's merge, define a local `const INJECTED_TAG: usize = 0x50414E45;` with a comment; the parent unifies at merge.
- Real-input opt-in tests: `PANE_TEST_REAL_INPUT=1`, gated in `.github/actions/setup/action.yml`'s opt-in step with exactly one line, so every Windows test job gets it:
  `if [ "$RUNNER_OS" = Windows ]; then echo "PANE_TEST_REAL_INPUT=1"; fi`
- Windows crate features are added to `crates/pane-core/Cargo.toml`'s `[target.'cfg(windows)'.dependencies]` as needed; keep additions minimal.

## Tests

- New pane-core test files: `crates/pane-core/tests/<name>.rs` **plus** `mod <name>;` in `crates/pane-core/tests/main.rs` (the `every_test_file_is_compiled` test enforces this). Same for `crates/pane/tests/`.
- Pure logic (recognizer, window predicate, Run parser, system-command decisions) lives in shared modules compiled on every system, tested on every system. Windows-API halves are `#[cfg(windows)]` (or the `#[cfg(any(windows, test))]` trick for Windows-format parsers that tests build stores for).
- Real-input adapter tests (keyboard/foreground/clipboard injection) gate on `PANE_TEST_REAL_INPUT` with the `opted_in()` + `eprintln!("skipped: …")` shape of `system_icon_adapters.rs`.
- Real extension through the launcher's public interface: the `clipboard_view.rs` pattern (support/artifacts.rs + publish + `with_defaults` + `acquire_defaults`), `#[cfg(target_os = "windows")]` for the Windows-only packages.

## CI

- Ticket branches `pi-subagent/125-<n>-<slug>` are in the fast tier: a plain push runs `Check (os)` (fmt + cargo check, Linux + Windows, ~13 min). Iterate there. Superseded runs auto-cancel.
- Never dispatch a verify run from a ticket (the parent runs one verify at the end on the spec branch).
- If a push rebuilds JS/TS guests (prebuilt stale), commit the `js-guests` artifact: `gh run download <run> -n js-guests -D guests/prebuilt`.
- The known macOS pasteboard flake (#231) counts as green; the `command_search.rs` stall flake is absorbed by retries.

## Commits

- Author `hoangvu12 <hggaming91@gmail.com>`, signed off: `git -c user.name=hoangvu12 -c user.email=hggaming91@gmail.com commit -s`. No other email anywhere.
- Format by hand to rustfmt defaults: max_width 100, fn_call_width 60, chain_width 60, 4-space indent, doc comments ~80 cols.
