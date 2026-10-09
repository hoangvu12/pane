## Parent

https://github.com/pane-app/pane/issues/207

## What to build

A Pane installed from today's package finishes its first setup with its five default extensions, acquired by Pane's own Git client from the commits this Pane release pins — not from the artifact source, which was never deployed and no longer serves default extensions at all.

- A pins file, embedded in the build, lists every default extension this Pane build sets up: its id, its title, its repository, its release tag and that tag's commit:

  ```json
  [
    { "id": "calculator", "title": "Calculator",
      "repository": "https://github.com/pane-app/calculator",
      "tag": "v0.5.0", "commit": "<full commit id>" }
  ]

  ```

  The five are the calculator, applications, quicklinks, files and clipboard history. Until their repositories release (#279), the committed pins name the five repositories with placeholder commits; #282 moves them to the real ones.

- At first setup Pane acquires each missing default by fetching exactly the pinned commit from the pinned repository with its own Git client (ADR 0021), and installs what it wrote out as a package from a downloaded folder is installed: read and checked, planned, claimed, written into a managed copy — with the default extension's identity (`default:<id>`) and its Git source recorded (repository, tag, commit, pinned). That record is what #269 will read to update a default from its repository's newer release tags. All five are installed: the first-setup picker is deferred (#283), so there is no choice screen; disabling a default remains the opt-out.

- A connection that fails is explained, and each default that could not be acquired is offered again as a row in root search, as today. An installed default, and one whose data an uninstall the user chose keeps, is not acquired again.

- The artifact source's default-extension machinery is removed: the index entries and payload tarballs for defaults, the payload cache under the acquired folder, and the development override that named a source for them. The artifact source itself remains, for Pane's own application updates only, and its development override stays for that. A development build names the default extensions' repositories instead through a new override: a pins file that replaces the embedded one, so the smokes can point a development build at repositories served on this computer. Without the override, the embedded pins are used — as a release build uses them.

- An install that acquired its defaults from the artifact source (an older Pane) keeps them: their identity, saved data, quick slots, hotkeys and aliases are unchanged, and they are not re-acquired.

Tests make the repositories with `git` and serve them from 127.0.0.1 with `git upload-pack`, as Pane's Git tests do; no check reaches a real Git host.

## Acceptance criteria

- [ ] First setup installs each pinned default from its repository's pinned commit, with the default identity and its Git source recorded (tests serve the repositories from 127.0.0.1)
- [ ] A pinned revision's package is installed through the same path as a package from a folder, its manifest and built components checked as any package's are
- [ ] A connection failure during acquisition explains itself and offers each failed default again as a retry row
- [ ] An installed default, and one whose data an uninstall kept, is not acquired again
- [ ] An install that acquired a default from the artifact source keeps it, its identity and its data
- [ ] A test validates the pins file: every default extension named exactly once, the five ids, and well-formed repository, tag and commit
- [ ] No default-extension index entries, payload tarballs or payload cache remain; application updates still check and download through the artifact source
- [ ] A development build takes its pins from a file it names, pointing at repositories on a loopback address; a release build cannot
- [ ] CI: the `ci-fast.yml` verify run is green on the ticket branch

## Blocked by

- None — can start immediately.
