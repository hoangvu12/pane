## Parent

https://github.com/pane-app/pane/issues/207

## What to build

The domain vocabulary and the CI notes describe the default-install path this milestone built, and stop describing the one that never worked.

- The glossary's "Artifact source" and "Acquired artifact" are reworded for application updates only: an artifact source is where Pane reads the index of its own application updates, and nothing about default extensions is acquired from it.
- "Default extension" describes what a fresh install now does: every default is installed at first setup from the commit of its repository's release tag that the Pane release pins — the choice screen is deferred (#283), so all five are installed and disabling is the opt-out.
- "Official extension" keeps ADR 0045's meaning, and now has a marking to point at (#280).
- The CI notes describe what the smokes and the packaging task now do: the smokes clone the pinned repositories and serve them on 127.0.0.1, the packaging task writes no default-extension payloads, and the resource workload's hidden phases serve the repositories.
- Where the handoff and decision notes stand, they record where this work stands.

## Acceptance criteria

- [ ] The glossary's entries are reworded; no entry says a default extension comes from the artifact source
- [ ] The CI notes match what the smokes, the packaging task and the workload's hidden phases do
- [ ] The handoff notes record the state of the official extensions
- [ ] CI: a documentation-only push runs the summary and nothing else, and it is green

## Blocked by

- #284 — Pane's smokes install the defaults from their pinned repositories served on this computer
- #285 — The five extensions' sources leave Pane's repository, the samples replace them as fixtures
