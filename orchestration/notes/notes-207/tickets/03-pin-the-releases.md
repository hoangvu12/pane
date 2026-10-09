## Parent

https://github.com/pane-app/pane/issues/207

## What to build

The pins this Pane build sets up name the five repositories' real release revisions. After #279 moved the extensions into their repositories and tagged their initial releases, the pins file's every entry holds the repository, the tag and that tag's commit of the release it was tested with. Moving a pin is a reviewed change; this ticket is that review, for the first time.

A release's pins are what a fresh install acquires, so they are moved with the release that tested them, not between releases: this milestone's PR is what carries them.

## Acceptance criteria

- [ ] Every pin names the repository, tag and commit of its repository's initial release revision
- [ ] A test validates the pins: every default extension named exactly once, the five ids, a repository under `pane-app`, a `v<semver>` tag, a full commit id
- [ ] A development build with the pins-file override still takes its pins from the override, not the file
- [ ] CI: the `ci-fast.yml` verify run is green on the ticket branch

## Blocked by

- #278 — First setup installs the default extensions from the commits this release pins
- #279 — Move the five default extensions into their pane-app repositories with release CI
