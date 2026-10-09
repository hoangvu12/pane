## Parent

https://github.com/pane-app/pane/issues/207

## What to build

The five default extensions live in public repositories of their own in the `pane-app` organization, each an ordinary one-extension Git repository (ADR 0044, ADR 0045) whose release tags hold its built component.

- Create the five repositories, public, under `pane-app`: `calculator`, `applications`, `quicklinks`, `files`, `clipboard-history`.
- Each takes its extension's source and its manifest package from Pane's repository today (its source folder in the guests tree and the package folder that holds its `pane.json` and icons), with its history if practical — a subtree split of its source, the package files brought in as commits — restructured so the repository's root holds the one-extension shape: its `pane.json`, its icons, its Rust source and manifest.
- Each repository's CI, on every pull request and push, builds its WebAssembly component against the `pane-extension` SDK and checks the built package. Until the SDK is published to crates.io (a person's step, #281), the build checks out Pane at a pinned commit and patches `pane-extension` to the crate there; the workflow documents the patch, and #287 removes it once the crate is published.
- On a `v<semver>` tag, each repository's CI builds the component, commits it as a release revision and tags it, so the tag's commit holds the built component beside the manifest and icons.
- Each repository is tagged with its initial release at the version its manifest names today.
- Each repository's page says what it is: a README pointing at Pane's documentation and issue tracker, and licenses matching the extension's.

Pane's own repository is not changed by this ticket: the sources' removal from it is #285, and the pins that name these repositories are #282.

## Acceptance criteria

- [ ] The five repositories exist, public, each holding its extension's source and manifest, with its history where it was practical to carry it
- [ ] Each repository's pull-request CI builds its component and checks the package, green on its initial content
- [ ] A `v<semver>` tag in one of them produces a release revision whose commit holds the built component, verified once on a scratch tag
- [ ] Each repository's initial release tag exists, and its commit holds the built component, the manifest and the icons
- [ ] The interim Pane-checkout patch is documented in each workflow, with the follow-up that removes it

## Blocked by

- None — can start immediately.
