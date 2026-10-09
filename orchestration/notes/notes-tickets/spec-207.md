# Official extensions: move the defaults into pane-app repositories and install the commits each release pins
## Problem Statement

Pane's five default extensions (calculator, applications, quicklinks, files, clipboard history; #60) live in this repository's `guests/`, built against its `wit/` by path, and a release build acquires them at first setup from Pane's artifact source, `https://downloads.pane.sh/`, which was never deployed. So a Pane installed today finishes first setup with no extensions at all, and nobody outside this repository can work on the extensions everyone uses.

[ADR 0045](https://github.com/pane-app/pane/blob/main/docs/adr/0045-official-extensions-live-in-their-own-repositories.md) decides that official extensions live in repositories of their own in the [`pane-app`](https://github.com/pane-app) organization, and that each Pane release pins the commits of the defaults it sets up. This specifies the move.

## Solution

- Each default extension moves to its own public repository: `pane-app/calculator`, `pane-app/applications`, `pane-app/quicklinks`, `pane-app/files`, `pane-app/clipboard-history`. Each builds against the published SDK (#128) and publishes release revisions tagged `v<semver>` that hold its built components.
- Pane's build names, for each default, its repository, its release tag and that tag's commit. At first setup Pane fetches those commits with its own Git client (ADR 0021) and installs them, keeping each one's default identity.
- First setup shows the official extensions with the defaults ticked; the user unticks what they do not want.
- Pane marks official extensions as its own in Settings.
- Default extensions update from their repositories' newer compatible release tags (#127, adjusted).

## User Stories

1. As a new user, I want Pane's first setup to install its default extensions from where they are actually published, so that a fresh install works.
2. As a new user, I want to see the official extensions at first setup with the defaults ticked, so that I get the everyday features without choosing, and can leave out what I do not want.
3. As a new user, I want a default I left out to stay out, and to be installable later from Settings, so that my choice holds and is not final.
4. As a new user without a connection, I want first setup to say why it could not set up the defaults and offer each again, so that I can recover once I am online.
5. As a launcher user, I want official extensions marked as Pane's own in Settings, so that I can tell them from community extensions.
6. As a launcher user, I want a default extension's data, quick slots, hotkeys and aliases kept when it moves to its repository, so that nothing I set up is lost.
7. As a launcher user, I want clipboard history to keep recording from the first start and Files to keep using the file index, so that the defaults behave as before.
8. As a contributor, I want to work on an official extension in a small repository of its own, with its own issues and pull requests, so that I need not clone Pane to fix the calculator.
9. As a maintainer, I want each Pane release to name exactly which commit of each default it installs, so that a new install always gets the versions that release was tested with.
10. As a maintainer, I want each official extension's repository to build, check and tag its releases in CI, so that a release revision always holds built components.
11. As a contributor, I want first setup tested against repositories served on 127.0.0.1, so that no check reaches GitHub.

## Implementation Decisions

- **The repositories.** Created empty in `pane-app`. Each takes its extension's source from `guests/<name>` and `guests/packages/<name>` with its history if practical, a `pane.json` at its root, and CI that builds the component with the published SDK on every pull request and, on a version tag, commits the built component to a release revision and tags it `v<semver>`.
- **The pins.** A file in this repository (proposed: `crates/pane/defaults.json`) lists `{ id, title, repository, tag, commit }` for each default; `default_extensions()` reads it at build time. A test checks that each pin is well formed. Moving a pin is a reviewed change.
- **Acquisition.** First setup fetches each pinned commit through the Git client (ADR 0021), so a commit id pins the bytes, and installs it with the identity `default:<id>` and its Git source recorded. The artifact source's index and tarballs are no longer used for default extensions; the artifact source stays for Pane's own application updates.
- **The first-setup choice.** A screen listing the official extensions (title, description), defaults ticked. Unticked defaults are recorded as the user's choice, as an uninstall is, and are not acquired again. Settings' Extensions group offers them later.
- **Official marking.** An extension whose identity is a default identity, or whose Git source is in `pane-app`, is marked official on its page and in lists.
- **Updates.** #127's default-extension updates read the repository's release tags newer than the installed version, compatible with this Pane's extension API ([ADR 0046](https://github.com/pane-app/pane/blob/main/docs/adr/0046-every-breaking-change-to-the-extension-api-gets-a-new-version.md)), instead of the artifact source's index.
- **Removal from this repository.** The five extensions' sources leave `guests/` once their repositories release; the samples stay. Tests that used the five as fixtures use samples or pinned fixtures instead.
- **Glossary.** "Artifact source" and "Acquired artifact" are reworded for application updates only.

## Testing Decisions

- Core tests serve the five repositories from 127.0.0.1 with `git upload-pack`: first setup installs the pinned commits with default identities; an unticked default is not acquired; a connection failure offers retry rows; an existing install acquired from the artifact source keeps its identity and data.
- A window test of the first-setup screen.
- A test that the pins file names every default extension once.

## Out of Scope

- Publishing the SDK (#128), which this needs.
- Collections (#198); every official extension is a one-extension repository.
- Hosting Pane's own application updates.

## Blocked by

- #128, so the extensions build outside a Pane checkout.



## COMMENTS

