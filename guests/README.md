# Extension guests

Extensions are WebAssembly components implementing the `pane:extension`
contract in [`wit/extension.wit`](../wit/extension.wit). Pane registers only
WASI 0.3 interfaces; a component that imports WASI 0.2 (for example through
Rust's standard library on `wasm32-wasip2`) is rejected with an explanation.

- `pane-guest`: Rust bindings for the contract. `no_std`, so only WASI 0.3 is
  imported; it supplies the allocator, a trapping panic handler and
  `cabi_realloc`.
- `sample-rust`, `sample-js`, `sample-ts`: the same sample command in Rust,
  JavaScript and TypeScript. All three show the same items and give the same
  answers and errors; the contract tests in `crates/pane-core/tests/samples.rs`
  and `crates/pane/tests/window.rs` hold each of them to that.
- `sample-settings`: a Rust command that keeps a chosen greeting style in
  Pane's settings ([Keeping settings](#keeping-settings)); the fixture for
  disabling and re-enabling a package.
- `js`: `@pane/extension`, TypeScript declarations for the contract
  (`pane.d.ts`) and the WIT world JS/TS commands are built against.
- `prebuilt`: the JS and TS sample components, committed so that tests and
  `cargo run -p pane` need no JavaScript toolchain, with `manifest.json`
  recording their hashes and build inputs.
- `packages`: the samples' package manifests (`pane.json`). `cargo xtask
  guests` puts each one with its built component in
  `target/guests/packages/<name>/`, a ready-to-install package.
- `fixtures/faulty`: test fixture whose actions return an error or trap.
- `fixtures/mixed-p2`: negative control that imports WASI 0.2 and must be rejected.

## Writing a Rust command

The [sample](sample-rust/src/lib.rs) is the complete example. A command is a
`cdylib` crate depending on `pane-guest` that implements two async functions:

```rust
#![no_std]

use pane_guest::alloc::{string::String, vec};
use pane_guest::{Guest, Item, View};

struct Hello;
pane_guest::export!(Hello);

impl Guest for Hello {
    async fn get_view() -> Result<View, String> {
        Ok(View {
            title: "Hello".into(),
            items: vec![Item { id: "hi".into(), title: "Say hi".into(), subtitle: None }],
        })
    }

    async fn run_action(_item_id: String) -> Result<String, String> {
        Ok("hi!".into())
    }
}
```

Returning `Err` shows the message as an error; a panic traps the guest, which
Pane reports and recovers from by starting a fresh instance on the next call.
WASI 0.3 interfaces are available through the
[`wasip3`](https://docs.rs/wasip3/0.9.0/wasip3/) crate with
`default-features = false`; the sample awaits `wasi:clocks` this way.

Build with the pinned toolchain (`wasm32-wasip2` is the compiler target name;
the emitted component imports only WASI 0.3):

```sh
cargo xtask guests
# or, for the guests workspace only:
cd guests && cargo build --release --target wasm32-wasip2
```

To try a rebuilt sample in the launcher, `cargo run -p pane` from the root; set
`PANE_EXTENSIONS_DIR` to a directory containing `sample_rust.wasm`,
`sample_js.wasm` and `sample_ts.wasm` to use different builds. To run your
own command, make it a package and install it; see
[Packaging and installing a local extension](#packaging-and-installing-a-local-extension).

Toolchain used: Rust 1.98.1, `wit-bindgen` 0.62.0, `wasip3` 0.9.0+wasi-0.3.0;
host Wasmtime and wasmtime-wasi 49.0.1.

### Keeping settings

A command of an installed package can keep string values between runs with
`pane_guest::settings` (the `pane:extension/settings` interface in
[`wit/settings.wit`](../wit/settings.wit)). The
[settings sample](sample-settings/src/lib.rs) saves the greeting style the
user picks:

```rust
use pane_guest::settings;

settings::set("greeting-style", "formal")?;          // Result<(), String>
let style: Option<String> = settings::get("greeting-style")?;
```

- Values belong to the installed package's source identity, not its title
  or managed copy: two installed copies of the same package have separate
  settings, and an update keeps them.
- They are kept while the package is disabled and while Pane is not running,
  and the command sees them again when the package is enabled. While it is
  disabled nothing of the package runs, and once it is disabled a call still
  finishing from before cannot save: `set` fails instead of writing.
- Pane keeps them in `extensions/settings.json` in its data folder. If that
  file cannot be read, `get` and `set` return the reason and Pane does not
  overwrite the file.
- A command built into Pane rather than installed from a package has no
  settings: `get` and `set` return an error.
- A component that does not import `settings` is unaffected; it is built for
  the `extension` world as before. `extension-with-settings` adds the import
  within extension API 0.1, so a component that uses settings needs a Pane
  with this change. JavaScript and TypeScript commands cannot import
  settings yet.

## Writing a JavaScript or TypeScript command

The [JavaScript](sample-js/src/index.js) and
[TypeScript](sample-ts/src/index.ts) samples are complete examples. A command
is an npm package whose `main` module exports `command` with two async
functions. Pane's types come from `@pane/extension` (a `file:../js`
development dependency); they describe plain values, not engine objects:

```ts
import type { Command } from "@pane/extension";
import { waitFor } from "wasi:clocks/monotonic-clock@0.3.0";

export const command: Command = {
  async getView() {
    return { title: "Hello", items: [{ id: "hi", title: "Say hi" }] };
  },
  async runAction(itemId) {
    if (itemId !== "hi") throw new Error(`unknown item: ${itemId}`);
    await waitFor(10_000_000); // 10 ms; the command suspends meanwhile
    return "hi!";
  },
};
```

Throwing (a rejected promise) shows the error's message, or a thrown string,
as an error. Returning a value of the wrong type, such as `undefined` from
`runAction`, traps the guest, which Pane reports and recovers from as for
Rust. npm dependencies are bundled into the component; the samples use
[Zod](https://zod.dev) 4.6.5 (`zod/mini`) and show its validation failure as a
normal error. Only ECMAScript built-ins are available, not Node.js or browser
APIs; WASI 0.3 imports declared by [the world](js/wit/world.wit) (currently
`wasi:clocks/monotonic-clock`) are imported by name and typed in
[`js/wasi.d.ts`](js/wasi.d.ts). `Math.random`, `Date.now()` and
`performance.now()` are fresh in each instance.

**Snapshot caveat.** A component is built by running the module once and
snapshotting the engine, so module top-level code runs at build time, on the
build machine, and every instance starts from its result. Keep top-level code
to pure setup such as schemas and constants: secrets, IDs, timestamps, random
values or anything else meant to differ per instance belong inside
`getView`/`runAction`. For the same reason, rebuilt components are never
byte-identical.

Build commands, from the repository root:

```sh
cargo xtask js-guests        # rebuild guests/prebuilt/ and target/guests/ from the samples
python3 tools/componentize-js/pane_js.py build <package dir> <out.wasm>   # any command package
python3 tools/componentize-js/pane_js.py check   # are the prebuilt samples current and their npm licenses permissive?
```

A build installs the package's locked dependencies into a staging copy,
type-checks it with TypeScript when it has a `tsconfig.json` (the JS sample is
checked through JSDoc), bundles it with esbuild and componentizes it. The TS
sample's `.ts` source is transpiled by esbuild and runs on the same runtime as
JS. The first run builds the toolchain (about five minutes), later runs take
seconds. Everything downloaded or built goes to `PANE_JS_TOOLCHAIN_DIR`,
default `~/.cache/pane/componentize-js` on Linux,
`~/Library/Caches/pane/componentize-js` on macOS and
`%LOCALAPPDATA%\pane\componentize-js` on Windows; the source tree is not
written except for the output. After editing a sample, run
`cargo xtask js-guests` and commit the updated `guests/prebuilt/`.

Prerequisites, in addition to the Rust ones in the [README](../README.md):

- Python 3.12 or later (`python3`; on Windows use `python` in the commands
  above; set `PYTHON` for `cargo xtask` if yours is named differently), git,
  and Node.js 22 or later with npm. `cargo xtask ci` also runs the `check`
  subcommand, so it needs Python too.
- The build installs Rust `nightly-2026-09-27` with `rust-src` through rustup,
  and downloads wasi-sdk 34 for the host (x86_64 or arm64, all three OSes).
- **Windows:** `python` from python.org or the Microsoft Store and Node.js
  from nodejs.org; run from a normal shell. **macOS:** Xcode Command Line
  Tools already provide git; install Python and Node.js from their installers
  or Homebrew. **Linux:** the distribution's `python3`, `git`, `nodejs` and
  `npm` (Node.js 22+, for example through nvm).

Toolchain used: upstream [componentize-qjs](https://github.com/andreiltd/componentize-qjs)
0.4.5 at `e563c6d6` with the two patches in
[`tools/componentize-js/patches`](../tools/componentize-js/patches), its
QuickJS runtime built with `nightly-2026-09-27` for `wasm32-wasip3` against
wasi-sdk 34, the componentizer built with Rust 1.98.1, esbuild 0.28.2 and
TypeScript 7.0.2. Every JS component imports the same 20 WASI 0.3 interfaces
through its libc, whatever the source uses, and is about 4.3 MB. Only Linux
x86_64 builds have been run; the scripts avoid OS-specific paths, but Windows
and macOS builds are unverified. See
[tools/componentize-js](../tools/componentize-js/README.md) for the patch queue.

## Packaging and installing a local extension

A package is a folder with a `pane.json` manifest at its root and the built
component of each command it lists. The same format serves Rust, JavaScript
and TypeScript: Pane sees only components.

```json
{
  "manifestVersion": 1,
  "title": "Hello",
  "version": "1.0.0",
  "apiVersion": "0.1",
  "commands": [
    {
      "id": "hello",
      "title": "Say hi",
      "subtitle": "Optional second line in root search",
      "component": "target/wasm32-wasip2/release/hello.wasm"
    }
  ]
}
```

- `manifestVersion` (required): the manifest format, currently `1`. A newer
  number is refused with "a newer Pane is needed".
- `title` (required): the display title. It is not the package's identity.
- `version` (optional): shown before installing and after an update.
- `apiVersion` (required): the `pane:extension` contract the components are
  built against, `MAJOR.MINOR` (this Pane provides `0.1`, from
  [`wit/extension.wit`](../wit/extension.wit)). Before 1.0 the minor version
  must match; from 1.0, any minor version up to Pane's in the same major.
- `commands` (required, at least one): `id` unique in the package, `title`,
  optional `subtitle`, and `component`, a relative path inside the package
  folder (no `..`, no absolute path) to a built component.

Unknown fields are ignored. The component must exist when you install: a
package whose component is not built is refused as source-only, with the
missing path. Pane then checks each component without running it: it must
compile, import only WASI 0.3 and export the extension interface.

Where the component comes from is up to your build. A standalone Rust crate
can point `component` at `target/wasm32-wasip2/release/<name>.wasm` inside
the crate folder after `cargo build --release --target wasm32-wasip2`. A
JavaScript or TypeScript package can build into its own folder with
`python3 tools/componentize-js/pane_js.py build <package dir> <package dir>/dist/<name>.wasm`
and point at `dist/<name>.wasm`. The repository's samples live in one Cargo
workspace and a prebuilt folder, so their manifests are in
[`packages/`](packages) and `cargo xtask guests` assembles each with its
component into `target/guests/packages/<name>/`.

To install, choose **Install extension from folder…** at the end of root
search, pick the package folder, check the source, version, commands and
compatibility Pane shows, and press Enter on **Install**. The package's
commands appear in root search, the first one selected. From the command line,
`cargo run -p pane -- --install target/guests/packages/sample-rust` (or
`pane --install <folder>`) opens the same screen, which also helps where no
folder picker is available: on Linux the picker is the desktop portal
(`xdg-desktop-portal`), and without one Pane shows why it could not open it.

What installing does:

- **Identity.** The package is identified by its folder's absolute path as
  the operating system resolves it (`std::fs::canonicalize`): symbolic links
  and `..` are followed, and on file systems that ignore letter case or
  Unicode normalization (the defaults on Windows and macOS) the stored
  spelling is used, so two spellings of one folder are one package. Pane does
  no case folding or normalization of its own, so on a case-sensitive Linux
  file system `Hello` and `hello` are two packages. On Windows the `\\?\`
  prefix is dropped. A folder path that is not valid Unicode is refused.
  Moving or renaming the folder makes it a different package.
- **Copy.** `pane.json` and the listed components (nothing else) are copied
  into Pane's data folder, under `extensions/packages/<n>/`, and recorded in
  `extensions/installed.json`. Your folder is never written, and the
  installed copy keeps working if the folder changes or is deleted. The data
  folder is `%LOCALAPPDATA%\Pane\data` on Windows,
  `~/Library/Application Support/Pane` on macOS and `$XDG_DATA_HOME/pane`
  (default `~/.local/share/pane`) on Linux; `PANE_DATA_DIR` overrides it.
- **Duplicates and updates.** Installing a folder that is already installed is
  refused. Choosing it again shows **Update** instead, which replaces the
  installed copy with the folder's current contents under the same identity,
  whatever its new title or version. Two different folders are two packages,
  even with identical contents, and nothing is merged or switched between
  them.
- **Listing.** Installed commands are listed from the manifests alone; no
  guest runs until you open a command. A damaged installed copy stays listed
  with its problem.
- **Disabling.** **Manage extensions…**, the last row of root search once a
  package is installed, lists every installed package with whether it is
  enabled and its source, so copies with the same title can be told apart.
  Enter disables or enables the selected one; only that installation
  changes. A disabled package's commands leave root search (they are not
  shown greyed out), an open command of it closes, and its running
  instances are dropped, so none of its code runs. The choice is recorded in
  `installed.json` (`"disabled": true`) and holds after restarting Pane and
  after an Update. Its settings are kept, and enabling it brings its
  commands back with them. The package stays installed at the same identity;
  choosing its folder again shows it as disabled.

Uninstalling and rebuilding on save are not implemented yet; to pick up a
rebuilt component, choose the folder again and Update.

Known limits of local packages so far:

- The manifest cannot yet declare the operating systems a package supports
  (Q34's supported-OS metadata); that is deferred to a later ticket, so a
  package is offered on every OS.
- An update is not coordinated with a command that is running or open: the
  replaced copy's code is dropped, so an open command of the package loses
  its state and may fail until you open it again from root search. Staged
  activation that waits for running commands comes with reload (#11, #14).
- Disabling does not cancel a call already running in the package: it
  finishes (its answer is discarded once its screen is gone, and it cannot
  save settings), then its instance is dropped. Cancelling async work,
  background services, timers and hotkeys are not part of the extension API
  yet and come with their own tickets. Disabling does not yet consider
  packages that depend on the disabled one (#43).
