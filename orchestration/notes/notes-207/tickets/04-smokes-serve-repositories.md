## Parent

https://github.com/pane-app/pane/issues/207

## What to build

Pane's smokes install the default extensions the way a real install does: from their pinned repositories, served on this computer. Nothing Pane runs reaches the network; the smoke scripts make the repositories on 127.0.0.1 and serve them with `git upload-pack`, as Pane's tests do.

- The smoke scripts (Linux, Windows, macOS) clone the five repositories at the commits the pins name — the release revisions hold the built components — and serve each clone from 127.0.0.1 over Git's smart HTTP protocol, with the repository server the smokes already run for their Git installs. The development build's pins override points at them. The clone is smoke setup, on the runner; the Pane under test fetches only from 127.0.0.1.
- The installed-package phase (clean machine, no development tools) finishes its first setup with all five defaults from those repositories, and the calculator answers `6*7` with 42, as it does today.
- The clipboard phases that wait for the clipboard history to record from the first start, and any phase that installs a default at first setup, do so from these repositories.
- The packaging task no longer assembles the default extensions' payloads: no payload tarballs, no index entries for defaults. The index the artifact source serves names only Pane's own application package. The application-update phases keep serving that index and checking and applying an update from it, through the artifact-source override as today.
- The resource and latency workload's hidden phases serve the five repositories (not the payload folder they used) and wait for the five defaults to be recorded; without them they skip and report so, as they do today.
- The measure scripts' notes and checks match the new serving, on every system they run on.

## Acceptance criteria

- [ ] The smoke's first setup on a clean machine installs all five pinned defaults from repositories served on 127.0.0.1, and the calculator answers
- [ ] The packaging task writes no default-extension payloads or index entries; the index it writes names only the application package
- [ ] The application-update smoke phases still check and apply an update served from the artifact source
- [ ] The resource workload's hidden phases wait for the five defaults installed from the served repositories
- [ ] Every system's smoke script serves the repositories from 127.0.0.1; the Pane under test reaches no network address
- [ ] CI: the branch tier is green on the ticket branch, and the smoke legs are checked (a manual `ci.yml` run naming the smoke jobs on the spec branch, or the release matrix after merge)

## Blocked by

- #282 — Pin the five repositories' release commits in this Pane build
