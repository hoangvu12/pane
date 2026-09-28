# Extension packages from npm

Added for [#45](https://github.com/hoangvu12/pane/issues/45) (US16–US18,
US20–US22, T11, T12, G4, G6; contributions), following
[ADR 0012](adr/0012-pi-style-source-identity.md)'s npm identity and the
proposed [ADR 0017](adr/0017-pane-downloads-npm-packages-itself.md). A Pane
extension can be published to npm as an ordinary npm package holding its
`pane.json` and built components. Pane downloads and installs it itself:
the user needs no Node.js, npm or compiler, and nothing in the package runs
to install it.

## Installing one

- **Install extension from npm…**, the second-to-last row of root search,
  opens a form of Pane's own with one field, the npm package: its name
  (`@pane-samples/greeter`, `greeter`) and optionally an exact version
  (`greeter@1.2.3`). **Show package** downloads it and shows the preview;
  Back returns to root search, discarding a download in progress.
- `pane --install npm:<name>[@<version>]` opens the same preview, as
  `pane --install <folder>` does for a folder.

The preview is the one a folder has ([packaging](../guests/README.md#packaging-and-installing-a-local-extension)),
with lines of its own:

- "Source: npm package @pane-samples/greeter": the identity.
- "npm version: 0.1.0, the latest", or "npm version: 1.2.3, the version you
  named: installing pins it to that version".
- "Downloaded: <tarball address>, matching its sha512 integrity from the
  registry".
- "Runs only the WebAssembly components its pane.json names, in Pane: no
  Node.js, npm install scripts or npm dependencies", and, when its
  `package.json` declares them, "Not used: its `postinstall` script and its
  npm dependencies, which its package.json declares; Pane never runs or
  installs them".

Install copies the package into Pane as a local folder is copied (only
`pane.json`, the components it names and this system's helper files) and
records it in `installed.json` as `"npm": "<name>", "npmVersion": "<version>"`,
with `"pinned": true` when a version was named. Its commands then run like
any other. What was downloaded is removed once the install ends; a preview
left without installing leaves its download until Pane next starts.

## Identity, versions and updates

The identity is the npm name without version (`npm:@pane-samples/greeter`
as a key): an npm copy and a local folder with the same code are two
packages, and every version of one name is one package.

- **A second install is refused**: `Launcher::install_npm` of an installed
  name, whatever the version named, answers "Already installed from npm
  package @pane-samples/greeter; use Update to replace the installed copy".
- **Choosing it again offers Update**: the preview says "Installed: npm
  version 0.1.0 of this package" (", pinned" when it is) and its row is
  **Update**, "Replace the installed copy with npm version 0.2.0, pinned" or
  "…, the latest". An update keeps the identity, its data, whether it is
  disabled, its hotkeys and aliases, as a folder's update does.
- **Pins**: naming a version installs exactly it and records it as pinned;
  updating without a version takes the latest and records it unpinned
  again. Nothing updates a package by itself yet (automatic updates of
  unpinned packages, Q21, are later work); the pin is recorded for them.
- A version is exact (`1.2.3`, `1.2.3-beta.1`, `1.2.3+build`); ranges
  (`^1.0.0`) and other tags (`next`) are refused before anything is asked of
  the registry. There is no picker of earlier versions (Q32).
- An npm package has no **Reload** or **Develop** row in Manage extensions:
  it has no source folder. Its **Retry** row (after a failure to start),
  disable, clear cache, uninstall and retained data work as for a folder;
  kept data stays with the npm name and installing it again, at any
  version, finds it.

## Dependencies from npm

A `pane.json` dependency's `source` may be `npm:<name>` or
`npm:<name>@<version>` ([dependencies](dependencies.md#declaring)), in a
local or an npm package. The plan, claims and rollback are #42's:

- A missing required npm dependency is downloaded while the preview is
  worked out, listed as "Requires: Greeter from npm, installed with it from
  npm:@pane-samples/greeter" and installed first; with a version in its
  source, that version, pinned.
- An installed one is used as it is, whatever version it has and whatever
  the latest is: installing another package never updates it (it counts as
  pinned) and never downloads it again. A disabled or paused one stays so.
- One that cannot be downloaded or installed stops the install before
  anything changes: "Nothing was installed: Caller requires `greeter` from
  npm package nobody, which cannot be installed: npm package nobody was not
  found in the registry …".
- Its record is `{ "id": "greeter", "npm": "<name>" }`, and a call by the
  dependency id reaches the installed package with that name.
- A package from npm cannot name a `local:` folder, which is on its
  author's computer: "… comes from npm but names the local folder
  `local:../helper` as its dependency `helper`; a package published to npm
  can depend only on packages from npm".
- An operation call by identity takes `npm:<name>` as it takes
  `local:<folder>`.

## What is refused

Each is explained on the preview, which then offers nothing, and nothing is
installed or left unpacked:

| Case | Status |
| --- | --- |
| No such package or version | "npm package nobody was not found in the registry https://registry.npmjs.org/", "… has no version 9.9.9; its latest is 0.1.0" |
| The registry cannot be reached | "Could not reach the npm registry … for <name>: <reason>" |
| No sha512 integrity (only a `shasum`) | "… has no sha512 integrity in the registry, which Pane needs to check its download" |
| The tarball is elsewhere | "Pane does not download <name>@<version>: its tarball address … is not on the registry …: Pane downloads a package only from the registry that describes it, over HTTPS" |
| The download does not match | "The download of npm package <name>@<version> does not match the sha512 integrity the registry gives (it is sha512-…); nothing was installed" |
| Too large | more than 16 MiB of metadata, a 64 MiB tarball, 256 MiB unpacked or 10,000 entries |
| An unsafe entry | "… cannot be unpacked safely: its tarball contains a symbolic link, `package/x`; Pane unpacks only files and folders" (also hard links, devices and named pipes), or "… contains `package/../../x`, which climbs out with `..`; Pane unpacks only paths inside the package" (also absolute paths, `\`, `:`, control characters, empty or `.` parts, names ending in `.` or a space, and Windows device names such as `con` or `nul`), or a file appearing twice |
| Another package's tarball | "The tarball of npm package <name>@<version> holds <other>@<version>, not the package asked for" |
| Not a Pane extension | "npm package <name>@<version> is not a Pane extension: it has no pane.json. Pane installs npm packages published as Pane extensions (a pane.json and the WebAssembly components it names); it does not run other npm packages, which need Node.js and npm" |
| Published without its build | "npm package <name>@<version> was published without the built component dist/x.wasm of "<command>": its author must build it and include it in the package before publishing. Pane does not build npm packages or run their install scripts (its package.json has `postinstall` and `prepare`, which Pane never runs)" |

Then every check a folder gets applies: its manifest, WASI 0.3 imports, the
extension API shape, its platforms and its helpers for this system
([compatibility](platform-availability.md), [helpers](helpers.md)).

## The registry

Pane uses `https://registry.npmjs.org/` over HTTPS, verifying its
certificate with the operating system's verifier, through the proxy that
`ALL_PROXY`, `HTTPS_PROXY` or `HTTP_PROXY` names (respecting `NO_PROXY`;
the Windows and macOS system proxy settings are not read). Only a registry on this computer can replace
it, for tests and development: `Launcher::with_npm_registry(Registry::local(url))`,
and `PANE_NPM_REGISTRY=http://127.0.0.1:<port>/` in development builds
(release builds ignore it). An address elsewhere is refused. The tests and
native smokes serve their fixtures from such a registry
([`npm_registry.rs`](../crates/pane-core/tests/support/npm_registry.rs),
[`scripts/npm_registry.py`](../scripts/npm_registry.py)); none of them
reaches the network.

## Publishing one

[Author instructions](../guests/README.md#publishing-a-package-to-npm) and
the controlled sample, [`guests/npm/greeter`](../guests/npm/greeter)
(`@pane-samples/greeter`, `"private": true` so that `npm publish` refuses
it), which `cargo xtask guests` assembles and packs on each contributor
system into `target/guests/npm/pane-samples-greeter-0.1.0.tgz`, the tarball
`npm pack` makes of it; it is never published.
[`sample-dependencies-npm`](../guests/packages/sample-dependencies-npm/pane.json)
is a local package requiring it.

## Checks

- Unit tests in [`npm.rs`](../crates/pane-core/src/npm.rs): names and exact
  versions, the loopback-only registry, the tarball's origin, sha512
  integrity, and unpacking (top folder, no execute bit, paths outside the
  package, names systems read differently, links, devices, pipes,
  duplicates, sizes, entry counts, damaged tarballs).
- [`crates/pane-core/tests/npm.rs`](../crates/pane-core/tests/npm.rs), with
  a local registry: preview, install, running its command and after a
  restart without downloading again; the form; the identity (a second
  install refused, Update, pins and unpinning); a local package requiring
  an npm one, installed with it and called by id; an installed one used as
  it is (no newer tarball fetched) and a disabled one kept disabled; a
  pinned dependency source; a dependency that cannot be downloaded; an npm
  package naming a local folder; every refusal in the table; install
  scripts never run; kept data reclaimed by the name; no Reload or Develop
  rows.
- [`crates/pane/tests/npm.rs`](../crates/pane/tests/npm.rs): the form,
  preview, Install and Update in the native window at Pane's size, the
  Update row in view below the longer details, and the command running.
- The native smokes' own phase (frames 260 to 266;
  [Linux](platforms/linux.md#npm-packages-45)).

## Limits

- Only the public registry, without credentials: no private or scoped
  registries, `.npmrc`, or enterprise mirrors; no registry setting for
  users.
- No automatic updates of unpinned packages yet, and no check for a newer
  version other than choosing the package again.
- The HTTPS path to the real registry is not exercised by the checks, which
  never reach the network; it was not run against registry.npmjs.org.
- Downloads are not resumed or kept across starts; an interrupted one
  starts again. Another Pane starting on the same data folder removes a
  download a preview made, and the install then downloads it again.
- The integrity is the registry's own; npm signatures and provenance are
  not checked.
- Unpacking reads the whole tarball into memory (at most 64 MiB).
- The system proxy settings of Windows and macOS are not read, only the
  proxy environment variables.
