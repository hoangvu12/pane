## Parent

https://github.com/pane-app/pane/issues/207

## What to build

The spec's first-setup choice, deferred by the user's decision for this milestone (first setup installs all five defaults, with no screen). First setup lists the official extensions (title, description) with the default extensions ticked; the user unticks what they do not want.

An unticked default is recorded as the user's choice, as an uninstall is, and is not acquired again; it can be installed later from the Extensions group in Settings. Without a connection, first setup says why it could not set up the defaults and offers each one again, as it does today.

## Acceptance criteria

- [ ] First setup shows the official extensions with the five defaults ticked, their titles and descriptions
- [ ] An unticked default is not acquired, and the choice is recorded as an uninstall is
- [ ] A default left out is installable later from Settings' Extensions group, and the choice holds across restarts
- [ ] Without a connection, first setup explains itself and offers each default again
- [ ] A window test of the screen
- [ ] CI: the `ci-fast.yml` verify run is green on the ticket branch

## Blocked by

- #278 — First setup installs the default extensions from the commits this release pins
