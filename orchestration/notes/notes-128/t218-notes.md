# Ticket #218 notes — JS/TS packages in the shared build crate (branch pi-subagent/128-218-js-build)

Working tree: `C:/Users/ADMIN/Desktop/nguyenvu/pane-wt/spec-128-t218` (worktree of pane-app/pane at
origin/main 26c8e224). All Rust feedback comes from GitHub Actions (cargo never runs locally).

## What was vendored, from where

- `tools/componentize-js/componentize-qjs/` = upstream andreiltd/componentize-qjs 0.4.5 @
  `e563c6d6ae50b087980414015663ca9c948c09bb` (codeload tarball sha256 `80effd71…a1e233`, verified
  against pins.json), with the patch queue applied **in-source**: 0001, 0002, 0004, 0005 (0003
  retired, see below). Upstream's Apache-2.0 `LICENSE` is kept at the vendored root; the patch
  files stay in `tools/componentize-js/patches/` as the record of what was applied (0003's
  retirement is recorded in `pins.json` under `retired_patches`, while `patches` still lists all
  five — patch_digests() digests the full list, which is what the committed `wasm-parts.json`
  records, so fresh and committed records stay comparable).
- `crates/core` (the `componentize-qjs` lib) is a member of the root workspace, built with the
  repository's stable Rust. `crates/runtime` (wasm32-wasip3-only, nightly+wasi-sdk) is EXCLUDED
  from the workspace and built only by `pane_js.py wasm-parts` from a cache copy. Upstream's CLI
  crate, napi, npm, tests and workspace root are not vendored; `examples/p3_build.rs` (upstream:
  Pane's file that pane_js.py used to copy in) is vendored as the standalone componentizer.
- The committed wasm parts: `tools/componentize-js/wasm-parts/{runtime.wasm, libc.so,
  wasm-parts.json}` downloaded from the `componentizer-wasm-parts` artifact of componentizer.yml
  run 37872096148 (main, 2026-10-09, head 23874e88). Digests verified: runtime
  `6c1706c03b4499dffd554eea98934929313b4b42040bb2eae4c9949ca10a4d3a`, libc
  `a8d7b7073fcece887a573c1b7b4cd3b1b215da579ed7ab2f1dff8baf76b7fae9` — both match the run's
  wasm-parts.json. Note: these bytes were built WITH patch 0003 applied; 0003 only touched
  crates/core, so the runtime/libc are unaffected by its retirement.

## Wasmtime 47 → 49.0.1 and the patches

- The vendored core crate pins wasmtime/wasmtime-wasi `=49.0.1` (the repo's version, same features
  as upstream's 47 pin: component-model + async; wasi p2+p3), wasmtime-wizer 49.0.1
  (default-features off, features component-model+wasmtime — no rayon/clap/wasmprinter), wasm-compose
  0.258.0 and wit-dylib 0.258.0 (default-features false kills wasm-compose's serde/yaml deps).
  API drift checked against the actual 49.0.1 sources (downloaded the crates): `p2::pipe`,
  `WasiCtxBuilder`, `p3::add_to_linker`, `define_unknown_imports_as_traps`, wizer's
  `instrument_component`/`snapshot_component`/`WasmtimeWizerComponent` are all unchanged in 49.
- **0003-stub-async-host-imports.patch retired**: wasmtime 49's `define_unknown_imports_as_traps`
  stubs `async func` imports with concurrent traps itself (verified in the 49.0.1 source, with a
  comment at the call site). 0001 (P3 libc link, in core), 0002 (reseed), 0004 (task-context stack
  pointer via wit-dylib `DylibOpts`), 0005 (native output) are applied in-source.
- 0001's `QJS_P3_LIBC` env read in the core crate was extended: `ComponentizeOpts::libc:
  Option<&'a [u8]>` passes the libc as BYTES (better than the prompt's "path explicitly" — no temp
  file, no process-global env, no unsafe set_var); `None` falls back to `QJS_P3_LIBC` as before,
  which the standalone `p3_build` keeps using. Chosen because the linked backend has the committed
  libc embedded and the spawn backend points the env at its own file.
- The vendored build.rs is rewritten: no ureq download, no wasi-sdk/binaryen, no runtime building —
  Pane always passes the runtime explicitly (`Runtime::Custom`), so the four built-in runtime
  constants are emitted as `&[]` (documented in the build script and the vendored README) rather
  than embedding the 1.25 MB runtime a second time beside pane-build's own copy.
- p3_build's `#[tokio::main]` was replaced by a hand-built current-thread runtime
  (`enable_all`) so tokio needs only the `rt` feature — `tokio-macros` is not in the current
  Cargo.lock and adding it would have meant another lock edit for no benefit.

## The one build path (pane-build) and its two componentize backends

`crates/pane-build/src/js.rs` ports pane_js.py's build(): staging (SDK written beside the package
so `file:../js` installs; package files refreshed, `node_modules` kept with the lock marker),
`npm ci --ignore-scripts --no-audit --no-fund` when the lock digest changed, the package's own
`tsc` (staged node_modules) when it has a tsconfig, esbuild (staged node_modules) around the
adapter entry (port of adapted_entry), world choice by what the bundle imports (uses_http /
uses_programs ported without a regex crate), WIT assembly from embedded files, then componentize.
`build_js_command(job, package, out, componentizer)` is public: pane-build's `Build` impl (one
call per component pane.json names) and `cargo xtask js-guests` (one per sample) are the same
code. esbuild runs through its CLI (`node node_modules/esbuild/bin/esbuild`); NOTE: esbuild
requires `--external:wasi:*` (colon) — `--external wasi:*` and `--external=wasi:*` are both
rejected. bundle.mjs is deleted; p3_build.rs moved into the vendored tree.

Backends behind one `Componentizer` seam (`Toolchains.componentizer` replaces `python` +
`componentize_js`):
- `Linked` — in-process call of the vendored crate (feature `componentizer`, on for pane-ext,
  xtask, and pane-core's dev-dependencies). Embeds the committed runtime.wasm/libc.so via
  include_bytes! and passes `Runtime::Custom` + `libc: Some(bytes)`; driven on a per-call
  current-thread tokio runtime.
- `Binary(Option<PathBuf>)` — spawns `componentize-qjs-p3` with the p3_build argv + QJS_P3_LIBC.
  The folder is PANE_COMPONENTIZER when set (this beats Linked even when one is linked in, so the
  componentizer workflow exercises a platform-built binary through the same code), else the
  package's own installed `@pane-app/cli` platform package, else (app-only, see below) a checkout
  folder.

`Toolchains::from_env(default_folder)`: PANE_COMPONENTIZER → Binary(Some); else Linked when the
feature is compiled in; else the default folder when it holds the three parts; else Binary(None)
(package lookup). PANE_PYTHON and PANE_COMPONENTIZE_JS are gone.

## @pane-app/cli platform-package lookup layout (for #219 to match)

The spawn backend (what an installed Pane uses) looks in the package's own
`node_modules/@pane-app/cli-<pane_target::Target::id()>/` — e.g.
`node_modules/@pane-app/cli-linux-x86_64`, `cli-windows-aarch64`, `cli-macos-arm64` — for the
three files:

- `componentize-qjs-p3` (+ `.exe` on Windows) — speaks the p3_build argv:
  `<wit> <world> <js> <runtime.wasm> <out.wasm>` and env `QJS_P3_LIBC` naming the libc.
- `runtime.wasm`
- `libc.so`

`PANE_COMPONENTIZER` names a folder with the same three files (a checkout's
`target/guests/componentizer`, assembled by `cargo xtask guests`, which builds the p3_build
example). #219's platform packages can carry these files beside whatever else they ship (e.g.
the pane-ext binary); the lookup only needs these three. Missing → "…npm install provides…".

## Who uses which backend (deliberate; matches the spec's package-owned componentizer)

- pane-ext: links the componentizer (regular dep with the feature).
- pane (the app): does NOT — its development mode uses the spawn backend: the package's own
  @pane-app/cli platform package, or (built from a checkout) `target/guests/componentizer` as
  the default folder passed by pane/src/main.rs (so `cargo run -p pane` + `cargo xtask guests`
  works, replacing the old checkout pane_js.py default).
- pane-core's tests: the linked backend via a dev-dependency (feature-gated), so the un-gated
  develop_builds.rs JS/TS legs and the new pane-ext tests/dev.rs TS leg need Node only; one new
  leg (a_typescript_package_with_pane_cli_installed_builds_with_its_componentizer) drives the
  spawn backend with the platform-package layout staged from target/guests/componentizer and
  asserts the "npm install" explanation for a package without it.

## xtask and CI

- `cargo xtask js-guests` → pane-build's build (Linked unless PANE_COMPONENTIZER) + manifest.json
  (new toolchain block: pins, patch digests, runtime/libc sha256 from wasm-parts.json, esbuild +
  typescript from the tools/componentize-js lock of record, "pane-build's JavaScript build …
  Wasmtime 49.0.1"). `--check` runs the staleness check alone.
- `cargo xtask guests` also builds the p3_build example and assembles
  `target/guests/componentizer/` (binary + wasm parts).
- `cargo xtask ci-lints`' prebuilt-samples check is now `js_guests::check()` (digests + staleness
  + npm license allowlist + esbuild/typescript version match against the record) — no Python.
- ci-fast.yml: the js-guests job runs `cargo xtask js-guests --check`, and if stale
  `cargo xtask js-guests` + artifact upload — no toolchain cache, no Python, no nightly. Node 24
  added (setup-node) to the verify-tier Tests jobs, the chosen-tests job, ci-branch.yml's Tests,
  and ci.yml's Tests + js-guests (+js-guests-tests keeps its Node, loses the toolchain cache and
  PANE_TEST_JS_BUILDS).
- componentizer.yml: wasm-parts job gained `pane_js.py check-parts` (committed parts vs fresh
  build); the componentizer job rebuilds ALL samples via `PANE_COMPONENTIZER=… cargo xtask
  js-guests` (more coverage than the old single-sample componentize, same job), then the tests.
- The samples (37 JS/TS packages incl. hello-js/hello-ts) got esbuild 0.28.2 + typescript 7.0.2
  devDependencies and regenerated locks (npm install --package-lock-only, run locally).

## New Cargo.lock entries (hand-edited; versions chosen to match existing lock entries)

Modified existing entries: `pane-build` (+componentize-qjs, pane-target, sha2, tokio), `xtask`
(+pane-build), `wasmtime 49.0.1` (+wasm-wave, +wit-parser, from wasmtime-wizer's "wasmtime"
feature), `wit-component 0.258.0` (+wat, from the dummy-module feature).

New: componentize-qjs 0.4.5 (path), wasmtime-wizer 49.0.1, wit-dylib 0.258.0, wasm-compose
0.258.0, wasm-wave 0.258.0, wat 1.258.0, wast 258.0.0, oxc-resolver 11.21.3, petgraph 0.6.5,
fixedbitset 0.4.2, compact_str 0.9.1, castaway 0.2.4, dashmap 6.2.1, fast-glob 1.0.0,
json-strip-comments 3.1.0, nodejs-built-in-modules 1.0.0, simd-json 0.17.0, value-trait 0.12.1,
halfbrown 0.4.0, float-cmp 0.10.0, beef 0.5.2, logos 0.14.2, logos-derive 0.14.2,
logos-codegen 0.14.2. wit-dylib/wasm-compose were pinned to 0.258.0 (NOT the prompt's 0.258.3)
because 0.258.3 requires wit-parser ^0.258.3, forcing a bump of the repo's whole 0.258.0
wit-stack; 0.258.0 is what upstream's own Cargo.lock resolves and matches the existing lock
entries exactly. ureq/clap are not in the lock (stripped).

## Deviations from the prompt's recommendations (and why)

1. wit-dylib/wasm-compose 0.258.0 instead of 0.258.3 (above).
2. `ComponentizeOpts::libc` takes bytes, not a path (above).
3. The app (pane) does not enable the linked backend — it keeps the spec's package-owned
   componentizer for an installed Pane, with a checkout folder default standing in for
   `PANE_COMPONENTIZE_JS`'s old default. "In a checkout, development mode uses the linked
   backend" holds for `pane-ext dev` and the tests; `cargo run -p pane` uses the checkout's
   target/guests/componentizer (needs `cargo xtask guests`, which the docs now say).
4. The committed runtime.wasm/libc.so come from the pre-existing main CI run (not rebuilt): the
   vendored runtime source is byte-identical to the patched source that run built (0003 never
   touched it), so the digests are unchanged. The new check-parts step will verify this holds on
   every wasm-parts build — if a rebuild turns out non-deterministic, that step will say so (it
   is not a merge gate).
5. pane_js.py's wasm-parts builds the runtime from a cache COPY of the vendored runtime crate,
   without `--locked`: the crate is excluded from the root workspace (nothing else builds it), a
   committed lock for it could not be generated without cargo, and its output's digest is
   recorded in wasm-parts.json, which is what everything downstream verifies.
6. The js-guests staleness pre-check now builds xtask (the whole workspace) instead of running a
   Python script — the job always uses the rust setup, but never a toolchain cache, a nightly or
   Python.
7. The samples' esbuild/typescript pin of record stays tools/componentize-js/package-lock.json;
   js_guests::check verifies every sample's lock matches it (so the manifest's toolchain block
   names what actually built them).

## Status / iteration log

- (to be filled from CI runs)
