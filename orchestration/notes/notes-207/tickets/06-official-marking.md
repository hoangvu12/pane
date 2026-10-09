## Parent

https://github.com/pane-app/pane/issues/207

## What to build

Settings marks official extensions as Pane's own, wherever it lists extensions (ADR 0043, ADR 0045). An extension is official when its identity is a default extension's, or when its recorded Git source is a repository in the `pane-app` organization — installed by Pane at first setup or by the user by hand alike. The extension's page in Settings and the Extensions group's list both say so, in the accepted UI's own vocabulary.

An extension from any other source is not marked.

## Acceptance criteria

- [ ] A default extension's page and its row in the Extensions group's list are marked official
- [ ] An extension installed by hand from a repository under `pane-app` is marked official
- [ ] An extension from any other source (another Git host, npm, a folder) is not marked
- [ ] CI: the `ci-fast.yml` verify run is green on the ticket branch

## Blocked by

- None — can start immediately.
