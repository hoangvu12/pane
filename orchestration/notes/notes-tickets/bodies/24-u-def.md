## Parent

https://github.com/pane-app/pane/issues/127 (Extension updates; [ADR 0045](https://github.com/pane-app/pane/blob/main/docs/adr/0045-official-extensions-live-in-their-own-repositories.md))

## What to build

Default extensions — identified as `default:<id>` — become eligible for automatic updates on the same terms as an unpinned npm package: enabled, not paused, not turned off. Per ADR 0045 (the spec issue's comment), a default extension's update reads its **repository's release tags** (`v<semver>`) newer than the installed version — the official extensions live in repositories of their own in the `pane-app` organization, and each Pane release pins the commit it sets up — skipping any whose `apiVersion` this Pane cannot run (ADR 0046). The payload is fetched with Pane's own Git client, its integrity from the commit id, and is downloaded, checked and staged exactly as first setup acquires one, applied at the same safe activation boundary under the same controls and semantics: the default identity, saved data, disabled state, hotkeys and aliases kept, the old generation ended.

An update never re-enables a disabled extension, and a default the user uninstalled is still never re-acquired. A payload needing a newer Pane lands in Skipped with "needs a newer Pane" (saying so when Pane's own application update exists), and the outcome of every default-extension check lands in the update results. Default extensions get the per-package "Update automatically" switch on their page in Settings, like npm and Git packages. Release builds read only the published repositories; development builds and tests use repositories served on a loopback address, never Pane's real downloads.

## Acceptance criteria

- [ ] A default extension is updated from a newer release tag with its data and controls kept (tests serve the repositories from 127.0.0.1, not a local artifact source)
- [ ] One disabled, paused, turned off or uninstalled-with-kept-data default is not updated automatically
- [ ] A payload whose `apiVersion` this Pane cannot run lands in Skipped with "needs a newer Pane"
- [ ] The per-package "Update automatically" switch covers default extensions, in the results of the global and per-package controls
- [ ] A new version that passes its checks but fails to start is paused with Retry and recorded under Failed, not rolled back
- [ ] An update that installs required dependencies changes nothing if any part fails
- [ ] Launcher public-interface tests against loopback repositories cover the above

## Blocked by

- #256 — Record update results, show them, and announce background failures
- #207 — Official extensions: move the defaults into pane-app repositories and install the commits each release pins (https://github.com/pane-app/pane/issues/207)
