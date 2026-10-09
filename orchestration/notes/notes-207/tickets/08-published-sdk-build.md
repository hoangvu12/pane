## Parent

https://github.com/pane-app/pane/issues/207

## What to build

Once the `pane-extension` crate is published to crates.io (#281), the five repositories' CI builds against it as the published SDK: the interim Pane-checkout patch (#279) is removed from every workflow, and a release revision built from the published crate is tagged in each repository. An author contributing to an official extension then needs no Pane checkout at all, which is the point of ADR 0045.

## Acceptance criteria

- [ ] Each repository's CI builds its component with `pane-extension` from crates.io, with no Pane checkout
- [ ] The patch and its documentation are gone from every workflow
- [ ] A release revision built from the published crate is tagged in each repository, and Pane's pins move to it in a reviewed change
- [ ] Each repository's CI is green after the switch

## Blocked by

- #281 — Publish pane-extension to crates.io and @pane-app/extension to npm
