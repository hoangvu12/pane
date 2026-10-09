# Pane default-extension machinery map (branch `spec/207-official-extensions` @ 26c8e22, off origin/main)

Written by the explore agent for spec #207's work; exact paths and symbols as of the branch point.

## 1. `guests/` layout

**The five defaults' sources** — all Rust, one component each, in the `guests` Cargo workspace (`guests/Cargo.toml`, resolver 3):
- `guests/calculator/src/lib.rs` + `src/expression.rs` (struct `Calculator`, `pane_extension::export!`)
- `guests/applications/src/lib.rs` (struct `Applications`)
- `guests/quicklinks/src/lib.rs` + `src/links.rs` + `src/transfer.rs` (struct `Quicklinks`)
- `guests/files/src/lib.rs` (struct `Files`)
- `guests/clipboard-history/src/lib.rs` (struct `ClipboardHistory`)
- Workspace dependency: `pane-extension = { path = "pane-extension" }` — path dependency on the in-tree SDK; the SDK's WIT (`guests/pane-extension/wit/`) is a checked copy of root `wit/` (`xtask/src/main.rs::same_files`, run by `cargo xtask sdks`).

**`pane.json`** (the package manifest) lives NOT in the source folders but in `guests/packages/<name>/`:
- `guests/packages/calculator/pane.json` — Calculator 0.5.0, `apiVersion "0.1"`, `icon.svg`, one command `calculator`, `mode: "provider"`, `rootResults: true`, component `calculator.wasm`
- `guests/packages/applications/pane.json` — 0.5.0, provider command `applications`, `indexedResults: true`
- `guests/packages/quicklinks/pane.json` — 0.5.0, 4 commands (`quicklinks` view+indexed, `create` view, `import`/`export` no-view), each its own svg
- `guests/packages/files/pane.json` — 0.8.0, `"fileIndex": true`, command `files` (search+rootResults)
- `guests/packages/clipboard-history/pane.json` — 0.6.0, 3 preferences (`keepHistoryFor`, `pauseRecording`, `disabledApplications`), command `clipboard-history`, platforms all three
- Tile icons (28×28 SVGs) live here too: `icon.svg`, `files/search.svg`, `quicklinks/{search,create,import,export}.svg`; held by `crates/pane/tests/default_icons.rs` (const `DEFAULTS: [DefaultPackage; 5]`).

**Build** — `cargo xtask guests` (`xtask/src/main.rs::guests()`):
1. `cargo build --locked --release --target wasm32-wasip2` in `guests` (component names `calculator`, `applications`, `quicklinks`, `files`, `clipboard_history` are in the `workspaces` list) and `guests/fixtures/mixed-p2`.
2. Copies components to `target/guests/*.wasm`; copies committed JS/TS components from `guests/prebuilt/` (only `sample_*_{js,ts}.wasm` + `manifest.json` + `NOTICE.md` — the defaults are NOT prebuilt, they're Rust).
3. Assembles ready-to-install packages in `target/guests/packages/<name>/` via `SAMPLE_PACKAGES` (59 entries, includes the five defaults) + `copy_package_files` (everything beside `pane.json`: icons, `HELP.md`, `assets/`).
4. Builds `pane-echo` native helper (`echo_helper`) into helper sample packages; assembles npm sample (`target/guests/npm/greeter`, packed tgz) and Git sample (`target/guests/git/greeter` with `dist/git_greeter.wasm`).
- `js-guests` rebuilds `guests/prebuilt/` from source then re-runs `guests`. `ci-tests` runs `guests()` first. `ci-lints` runs `cargo fmt --all --check` in `guests`, `guests/fixtures/mixed-p2`, `guests/helpers/echo`, `guests/hello-rust`, plus `pane_js check`, `sdks()`, clippy.

**Every place the five defaults' sources/manifests are referenced:**
- `xtask/src/main.rs`: `workspaces` component list, `SAMPLE_PACKAGES`
- `xtask/src/package.rs`: `const DEFAULTS: [(&str,&str); 5]` (calculator/applications/quicklinks/files/clipboard-history), `assemble_payloads` (reads `target/guests/packages/<package>`), test `the_default_extensions_ship_the_images_their_icons_name` (reads `guests/packages/<package>/pane.json`)
- `.github/workflows/ci.yml`: `licenses` job (cargo-deny in `guests`), `lints`/`tests`/`smoke`/`package` jobs (all run `cargo xtask guests`), `changes`/`js-guests` paths-filter (`guests/**`)
- `scripts/measure-linux.sh`: `unused_packages=(calculator quicklinks applications files …)` installed from `target/guests/packages`; `default_extensions=(calculator applications quicklinks files clipboard-history)` acquired from `target/dist/artifacts` in the hidden (#189) phase; requires `target/guests/packages/$package/pane.json` and `$artifacts/pane-defaults.json` (skips hidden phase without the latter)
- `scripts/smoke-{linux.sh,macos.sh,windows.ps1}`: install `target/guests/packages/calculator|applications|quicklinks` by hand; the installer phase (#53/#51/#52) runs `cargo xtask package-<os> --dev` and serves `target/dist/artifacts` via `scripts/artifact_server.py` + `PANE_ARTIFACTS` for first-setup acquisition of all five
- Tests (see area 10); docs: `guests/README.md` (per-extension paragraphs), `docs/current-decisions.md` Q6, `docs/HANDOFF.md`, `README.md`

## 2. `default_extensions()` and `defaults.rs`

- `crates/pane/src/lib.rs` ~line 116: `pub fn default_extensions() -> Vec<pane_core::DefaultExtension>` — ids `"calculator"`, `"applications"`, `"quicklinks"`, `pane_core::search_files::FILES` (= "files"), `pane_core::clipboard_view::CLIPBOARD_HISTORY` (= "clipboard-history"); titles "Calculator", "Applications", "Quicklinks", "Files", "Clipboard History". Pinned by test `the_default_set_is_the_five_default_extensions_without_the_samples` in the same file.
- `crates/pane-core/src/defaults.rs` (~590 lines) — the acquisition machinery:
  - `pub const PUBLISHED: &str = "https://downloads.pane.sh/"` (not deployed); `INDEX_FILE = "pane-defaults.json"`; `INDEX_FORMAT: u64 = 1`; `MAX_INDEX = 1<<20`; `ATTEMPTS = 3`; `RETRY_AFTER = [500ms, 1s]`
  - `pub struct DefaultExtension { pub id: String, pub title: String }`
  - `pub(crate) fn fetch(source, id, downloads, acquired, progress) -> Result<Fetched, Failed>` → `acquire`: read index → find entry → cache lookup (`extensions/acquired/<id>/<version>-<integrity16>.tgz`, reused only if integrity still matches) → download → `npm::unpack` into a `Download` folder → `keep_payload`
  - `Fetched { download: Download, origin: DefaultOrigin{id, version, integrity} }`; `Index { entries: Vec<Entry>, application: Option<serde_json::Value> }`; `Entry { id, version, file, integrity (sha512-<base64>), size }`; `read_index`, `write_payload` (`.part` + rename), `remove_abandoned_parts`
  - `with_retries`/`failed`/`interrupted`/`answer` are shared with `crate::application_update`
- Identity: `crates/pane-core/src/packages.rs` ~line 183 `PackageIdentity::default_extension(id)` → `Source::Default { default }` → `key()` = `default:<id>`; `DefaultRecordJson { defaultVersion }` flattened into the `installed.json` record beside `"default": "<id>"`; `SourcePackage::fingerprint` uses the payload integrity.

## 3. First-setup acquisition path

- `crates/pane/src/main.rs` ~160–195: debug builds take the source from `PANE_ARTIFACTS` (`ArtifactSource::from_dev_env()`; unset → **no defaults acquired at all in dev**), release builds use `ArtifactSource::published()`; then `launcher.with_defaults(source, pane::default_extensions())`. After the window opens: `cx.spawn(async { acquiring.acquire_defaults().await }).detach()` — pure background flow. **There is no first-setup screen and no tick/untick UI anywhere**; CONTEXT.md's "offered ticked at first setup" and ADR 0045's untick-records-user-choice are greenfield.
- `crates/pane-core/src/launcher/acquire.rs` (~360 lines): `Defaults { source, extensions: Arc<[DefaultExtension]> }`; `Acquisitions { in_flight, failed: Vec<FailedAcquisition> }` with `begin_flow/end_flow/succeeded/failed/retryable`; `Launcher::with_defaults`, `acquire_defaults()`, `default_missing()` (not installed AND no retained data — a disabled default is installed, so never re-acquired; an uninstalled-with-retained-data one is not re-acquired), `acquire_missing()` (loops the five, `remove_abandoned_parts` first, status "Acquiring the <title>: <n>% of <size>"), `retry_acquiring(id)`, `acquire_one()` (fetch off-thread → `SourcePackage::read_default` → `check_components` → `plan_dependencies` → `install_plan(store, package, plan, &Mode::Install, None, …)` → `put_installed`).
- On failure: `FailedAcquisition` recorded, `Status::Error("Could not set up the <title>: <why>")` + `crate::diagnostic!`; root search gets a retry row — `crates/pane-core/src/launcher.rs` ~3575: `state.acquisitions.retryable()` → `Row { id: "acquire:<id>", title: "Set up <title>", subtitle: <why> }`.
- Folders: `crates/pane-core/src/launcher.rs` — `DOWNLOADS_DIR = "downloads"`, `ACQUIRED_DIR = "acquired"`, both under the data folder's `extensions/` beside managed copies.

## 4. ArtifactSource

- Lives in `crates/pane-core/src/defaults.rs`: `pub struct ArtifactSource { origin: crate::http::Origin }`; `published()` / `local(url)` (loopback literal only, test/debug) / `from_dev_env()` (`PANE_ARTIFACTS`); `index_url()`, `payload_url(file)`, `get()` through `crate::http` (`get_blocking_progressing`).
- Serves: index `pane-defaults.json` + payload tarballs `<id>-<version>.tgz`.
- **Application updates** use it too: `crates/pane-core/src/application_update.rs` — `with_application_update(APP_VERSION, source, exe)` (main.rs), start-up `check_application_update()` reads the index's `application` entry (version/file/integrity/size/target), offers a root-search row; only the user's choice downloads/installs (512 MiB cap, staged swap, `.old` rename). Reuses `defaults::{with_retries, answer, failed, interrupted}`.
- Production: `xtask/src/package.rs::assemble_payloads` packs `target/guests/packages/<name>` → `target/dist/artifacts/<id>-<version>.tgz` (fixed `PACKED_MTIME` for reproducible integrity) + `write_index` (formatVersion 1, `defaults` entries, optional `application` entry) + `serve_package`.
- Test fakes: `crates/pane-core/tests/support/artifacts.rs` — `Artifacts::start()` (raw 127.0.0.1 TCP server), `publish(id, version, files)`, `publish_application[_tgz]`, `serve_index`, `fail_status`/`drop_after`/`stall`/`corrupt`, `requests()`. Smokes use `scripts/artifact_server.py` (Python `http.server` over `target/dist/artifacts`) with `PANE_ARTIFACTS`.

## 5. Pane's Git client (ADR 0021)

- `crates/pane-core/src/git.rs` (~2718 lines), no `git` program/library/config: smart HTTP protocol v2 over the one HTTP stack (`crate::http`, which gained a POST).
- Public API: `pub struct Repository { name, url, loopback, ssh }` (`name()` = `host[:port]/path`, `url()`, `written_as_ssh()`); `pub struct GitSpec { repository, reference: Option<String> }` with `GitSpec::parse` (`git:`/`git+`, https, ssh, scp-form, scheme-less; loopback `http://` only under `cfg(test, debug_assertions)`); `pub enum GitRef { Default{branch}, Branch, Tag, Commit }`; `pub struct GitRevision { reference, commit }` with `pinned()`, `ref_name()`, `from_record()`, `asked_as()`, `short_commit()`, `describe()`; `pub struct GitOrigin { repository, revision, advertised, subject, lfs_pointers }` with `caution()`; `pub struct InstalledGit { url, revision }`; `pub(crate) struct Fetched { download, origin }`; `pub(crate) fn fetch(spec, downloads) -> Result<Fetched, String>`; `pub(crate) fn fetch_within(spec, downloads, limits)`; `pub(crate) fn resolve_reference(repository, reference)` (updater's tracked-branch check without fetching); `pub fn ignore_case_on_host_for_tests(host_port)`.
- Flow: `Remote::connect` (`GET /info/refs?service=git-upload-pack`, requires v2 + SHA-1) → `resolve` (ls-refs; default branch / branch / tag / 40-hex commit) → `fetch` (`want <commit>`, `deepen 1`) → `read_pack` (checksum, zlib, deltas, SHA-1 with `sha1-checked` collision detection) → `check_out` into a `Download` folder under `extensions/downloads/` (refuses symlinks, submodules, `.git`/`git~1` entries in any case/HFS-ignoring form, case-colliding names, npm-refused names; nothing written executable). Limits: `MAX_REFS` 16 MiB, `MAX_PACK` 64 MiB, `MAX_OBJECTS` 20k, `MAX_UNPACKED`/`MAX_INFLATED` 256 MiB, `MAX_ENTRIES` 10k, `MAX_DEPTH` 32, `MAX_DELTA_CHAIN` 4096. `USER_AGENT = "git/pane-<version>"`. Case-insensitive paths on github.com, gitlab.com, bitbucket.org, codeberg.org.
- Install path: `crates/pane-core/src/launcher/install.rs` — `fetch_git(spec)` calls `git_source::fetch(spec, downloads)` then `SourcePackage::read_git(fetched)` (`crates/pane-core/src/packages.rs` ~1555; explains source-only revisions and LFS pointers); `preview_git`/`install_git`/`Request::Git`; `pane --install git:<repo>[@<ref>]` (`crates/pane/src/main.rs::ToPreview::Git`).
- `installed.json` (packages.rs ~2260, `GitRecordJson`): `"git": "<host/path>"` (identity) + `"gitUrl"`, `"gitRef"` (`refs/heads/…`/`refs/tags/…`, absent for default branch and commit-id), `"gitCommit"` (full id), `"pinned"` (tag/commit → true; serde-skipped when false). `GitRevision::from_record(ref_name, commit, pinned)` reconstructs.
- **Tests serving repos from 127.0.0.1 with git upload-pack**: `crates/pane-core/tests/repositories.rs` (install/preview/update/pin flow, `Dirs::greeter()` commits source-only `main` + release branch `release` tagged `v0.1.0`) and `crates/pane-core/tests/update.rs` (auto-updates: tracked branch moves, pinned tag never checked). Helper `crates/pane-core/tests/support/repo_server.rs`: `git_in(dir, home)` (env-clear, `GIT_CONFIG_NOSYSTEM=1`, fixed author/committer/dates), `Repo::{init, commit, head, tag, add_entry, write_object, commit_raw_tree}`, `greeter_files(guests, built)` (reads `target/guests/git/greeter`), `Server::{start, serve, set_mode, requests}` — a hand-rolled HTTP server that shells out to `git upload-pack --stateless-rpc [--advertise-refs] .` per request (modes Normal/ServiceLine/VersionZero/Redirect/SignIn/LongListing). Repos are created with the real `git` program on the runner (only tests/smokes run git; Pane itself never does). Smokes use `scripts/repository_server.py` + `scripts/check_git_record.py`.

## 6. Extension install flow from a downloaded folder

- `SourcePackage::read_default(fetched)` (packages.rs ~1615): reads `pane.json` from the `Download` folder, sets identity `PackageIdentity::default_extension(&id)`, `default: Some(DefaultOrigin)`, keeps the `Download` alive (removed when dropped).
- The launcher then runs the exact same path as a folder package (`acquire_one`): `check_components` (type-check against the API + WASI 0.3-only imports) → `plan_dependencies` (`crates/pane-core/src/dependencies.rs` Planner) → `install_plan` (`crates/pane-core/src/launcher/install.rs` — claims identities, writes the **managed copy** under `extensions/packages/<dir>`, records the package) → `put_installed` (live).
- `Download` (`crates/pane-core/src/downloads.rs`): reference-counted temp folder under `extensions/downloads/`, removed after install/preview/failure.
- Record shape (`RecordJson`, packages.rs ~2180): flattened `Source` (local/npm/git/default) + per-source record (`NpmRecordJson`, `GitRecordJson`, `DefaultRecordJson`) + `dir`, `disabled`, `paused`, `dependencies`, `network`, `programs`, `disabled_commands`. Retained data records keep source + title.

## 7. Settings' Extensions group (ADR 0043)

- `crates/pane/src/features/settings/extensions.rs` (~2500 lines): sidebar **Extensions** group after Pane's own pages; `sidebar_entries` — + menu (folder/npm/Git), one entry per installed extension (icon, title, paused/broken/updating mark); group page (installed list, automatic-updates control, runtime rows, retained data, install sources); per-extension page (large icon/title/description/source + mark, enable switch, Actions… menu — Check for Update, Reload, Clear Cache, Reset Confirmations, Show Source Folder, Uninstall, Retry/pausing/development/network/programs —, automatic-updates switch, preferences card, commands with alias/hotkey/own switch).
- All operations come typed from the core: `pane_core::Launcher::extension_operations` / `run_extension_operation`; confirmations and previews are core screens drawn in Settings. Core side: `crates/pane-core/src/launcher/extensions.rs` (marks `ExtensionMark`, `OperationKind`).
- **No 'official' marking exists today** anywhere — new work; would go in the sidebar entries + group-page rows + extension page header, with core-side marking on `InstalledPackage`/operation data.

## 8. The SDKs

- Rust SDK: `guests/pane-extension` — crate `pane-extension` v0.1.0 (version follows the extension API, ADR 0046; `wit-bindgen` =0.62.0, dlmalloc, serde/serde_json alloc, no_std). Publishes `/src`, `/wit` (a checked copy of root `wit/`), README, licenses. Checked by `cargo xtask sdks`: `same_files(root wit, guests/pane-extension/wit)` + `cargo publish --dry-run --allow-dirty --target wasm32-wasip2`. **Not published yet** ("a person's step, never CI's (#128)").
- npm SDK: `guests/js` — `@pane-app/extension` v0.1.0 (files: `*.js`, `*.d.ts`, `wit/world.wit`, licenses; exports `./http ./feedback ./system ./preferences ./icons ./programs`); `npm pack --pack-destination target/sdks` in `sdks()`. **Not published yet.**
- Guests build against them **by path only** today: Rust via workspace `path = "pane-extension"`; JS/TS samples via `"@pane-app/extension": "file:../js"` devDependency and world `guests/js/wit/world.wit`. ADR 0045's consequence: moving the five out needs the published SDK and WIT (#128).
- `sdks()` runs in `ci-lints` (every CI leg). `crates/pane-ext` exists with only `pane-ext dev` implemented (`new`/`check`/`pack` print "not available yet"; ADR 0047, #128); `crates/pane-build` is the shared build crate. #220 and #269 have no references in the repo — they are GitHub issues not reflected in code/docs at this commit.

## 9. Extension API versioning (ADR 0046)

- `crates/pane-core/src/packages.rs` line 40: `pub const EXTENSION_API: (u64, u64) = (0, 1)` (the `pane:extension@0.1.0` WIT package). Manifest `apiVersion` (e.g. "0.1") is parsed and checked by `api_compatible` at manifest read (~858) → `PackageError::IncompatibleApi` ("needs Pane extension API <required>, but this Pane provides 0.1"). Before 1.0, exact minor match required.
- Component-shape safety net: `crates/pane-core/src/runtime.rs` ~695 type-checks each component at load → "built for an older extension API shape" error. Negative fixtures: `guests/fixtures/old-api` (own older WIT copy), `guests/fixtures/mismatched-api`.
- No previous-version adapter exists yet (ADR 0046's support window is proposed); today only exact 0.1 loads.

## 10. Tests using the five defaults as fixtures

- `crates/pane-core/tests/`: `calculator.rs` (installs `target/guests/packages/calculator`), `applications.rs`, `quicklinks.rs`, `search_files.rs` (`COMMAND = "default:files#files"`; installs the files package), `clipboard_view.rs` (`COMMAND = "default:clipboard-history#clipboard-history"`, `PackageIdentity::default_extension("clipboard-history")`), `clipboard.rs`, `file_index.rs`, `file_actions.rs`, `memory.rs` (`memory_peaks_of_the_samples_and_default_extensions` — the five names), `installer.rs` (acquisition from `Artifacts` fake: cache, retries, `a_default_extension_that_left_the_default_set_stays_until_uninstalled` with legacy `helper-sample`, `a_disabled_default_extension_is_not_acquired_again`), `application_update.rs` (defaults + application entry), `extension_pages.rs`, `update.rs`.
- `crates/pane/tests/`: `default_icons.rs` (installs copies of the five assembled packages, checks tiles), `window.rs` (~3380: `with_defaults` over a local source), `search_files.rs`.
- Identity-based host behaviour that survives only if `default:<id>` is kept: `launcher/clipboard_view.rs` ~1115 (only `default:clipboard-history` gets the Clipboard History view), `launcher/search_files.rs` ~534 (only `default:files` gets the Search Files view), `launcher/clipboard_settings.rs` ~49, `clipboard/history.rs` ~123 (Pane records clipboard history for that identity), file index runs while an enabled `fileIndex: true` package (files) is installed.

## 11. CI workflows

- `.github/workflows/ci.yml` (push to main + dispatch; concurrency per branch): `windows-renderer`, `macos-renderer`, `licenses` (cargo-deny on root + `guests` + `guests/hello-rust`), `changes` (dorny/paths-filter; `js-guests` fires on `guests/** tools/** wit/** Cargo.lock rust-toolchain.toml ci.yml .github/actions/**`), `lints` (3 OS, `cargo xtask ci-lints`; on main also `ci-tests --no-run` for the cache), `tests` (3 OS × 3 nextest hash shards, `cargo xtask ci-tests --partition`), `smoke` (3 OS: `cargo build -p pane` + `cargo xtask guests`, then `smoke-linux.sh`/`smoke-windows.ps1`/`smoke-macos.sh` + `measure-linux.sh` on Linux which reuses `target/dist/artifacts` the smoke's package phase left), `package` (3 OS: `cargo xtask guests` + `cargo xtask package-<system>`, uploads `target/dist`), `js-guests` + `js-guests-tests` (rebuild prebuilt from source; tests with `PANE_TEST_JS_BUILDS=1`), `passed` aggregate.
- `.github/workflows/ci-branch.yml` (branch tier: lints + test shards), `ci-fast.yml` (pi-subagent worktrees), `componentizer.yml` (componentizer for 6 platforms, #215/#128).
- Smoke's default-extension use: earlier phases install `target/guests/packages/{calculator,applications,quicklinks}` by hand; the installer phase runs `cargo xtask package-<os> --dev`, serves `target/dist/artifacts` via `artifact_server.py` + `PANE_ARTIFACTS` on a clean machine, asserts all five acquired, `extensions/acquired/calculator` holds exactly one file, `installed.json` records them, and the calculator answers `6*7`; the clipboard phase re-serves artifacts for `default:clipboard-history`; the update phase builds `--package-version 99.0.0` for the application entry and the update install.

## Spec-207-adjacent open work

- ADR 0045 (accepted 2026-10-08): one repo per extension in the `pane-app` org named by id, release tags `v<semver>` (ADR 0044), Pane release pins per-extension commits, first setup fetches exactly those commits with the ADR 0021 client, `default:<id>` identity kept, first-setup list with tick/untick (deferred by the user's 2026-11-05 decision), between-releases updates from newer release tags under "#127's updater and controls"; artifact source remains only for application updates.
- ADR 0046: official extensions released for a new API version before the Pane release that pins it. ADR 0047/#128: `pane-ext` (only `dev` today), `@pane-app/cli`, template repo `pane-app/extension-template`. #127 (updater) and #220/#269: referenced only from ADR 0045 / not present in the repo at all.

## Risks / notes

1. **Blast radius of moving the five**: 6+ pane-core test files, 3 pane test files, three OS smokes, `measure-linux.sh`/`measure-windows.ps1`, xtask (`workspaces`, `SAMPLE_PACKAGES`, `DEFAULTS`, `assemble_payloads`, one package.rs test), CI `licenses`/`lints`/paths-filter, `memory.rs`.
2. `xtask package-*`'s `assemble_payloads` + index `defaults` section + `PANE_ARTIFACTS` serving are the whole acquired-artifact pipeline; #207 retires them for extensions (application entry stays) — `defaults.rs`, `acquire.rs`, `support/artifacts.rs`, `artifact_server.py`, and the smoke/measure hidden phases all need rework or removal.
3. Identity vs source: `SourcePackage::read_git` hard-codes `PackageIdentity::git(...)`; keeping `default:<id>` with a Git origin needs a new read path and a new record shape (`DefaultRecordJson` has only `defaultVersion`; Git fields `gitUrl/gitRef/gitCommit/pinned` would sit beside `"default": "<id>"`).
4. Pinned-tag semantics conflict: the current updater treats a tag as **pinned** and never checks it (`update.rs` asserts no request at all); ADR 0045 wants between-release updates from newer release tags — a new tracked-tag concept is needed (#269).
5. First-setup tick/untick UI is greenfield (no screen exists; acquisition is a detached background task with status-line and retry rows); "unticked recorded as user choice" can lean on the existing retained-data check in `default_missing`.
6. Git-client reuse is easy: `git::fetch(spec, downloads)` + `SourcePackage::read_git` + install flow all exist; tests can reuse `support/repo_server.rs`/`repository_server.py` to serve `pane-app/<id>` repos from 127.0.0.1 with release tags (github.com path case-insensitivity only applies to real hosts; `ignore_case_on_host_for_tests` covers loopback).
7. SDK publication is a prerequisite (ADR 0045 consequence): both SDKs are checked-as-packagable but unpublished; guests currently depend on them by path only.
8. Licensing: `guests/` is Apache-2.0 OR MIT with a CI cargo-deny leg; the moved repos need their own licensing and the `same_files(wit, guests/pane-extension/wit)` WIT-copy check stays in the main repo.
9. `installed.json` compat: an install that acquired a default as an artifact (old builds) keeps its `default` record; the new path must keep the same identity so clipboard-history recording, the Search Files / Clipboard History views, quick slots, aliases and hotkeys survive unchanged.
