## Parent

https://github.com/hoangvu12/pane/issues/151

## What to build

An extension replaced while the user works with it keeps what it had in memory when it supports that, and the screen that was open opens again. This defines ADR 0004's "explicit extension support" for temporary state (ADR 0041). It is size M.

**Opting in.** A component opts in by exporting `snapshot` and `restore` in the lifecycle interface that holds `activate` (#158). Installing detects the exports without running guest code. A JavaScript or TypeScript package sets a flag in its `package.json` so that the build links them, as for services.

**When.** A handoff happens on Reload, on an Update (made by the user or automatic) and on a development-mode reload, only once the replacement has passed its checks and is about to be installed. A replacement or build that fails changes nothing and takes no snapshot. It never happens after a crash, a pause, a failure to start, Retry, a disable followed by an enable, a runtime crash or hang, or a restart of Pane.

**Taking the snapshot.**

- Each running instance that exports `snapshot` and is idle (no call pending in it) is asked for one before the old generation ends. An instance busy with a call is stopped as today and gives none.
- The deadline is 1 second. A late snapshot is dropped and the replacement goes ahead, so a handoff never delays a reload by more than that.
- Each snapshot is limited to 1 MiB, and a larger one is dropped.
- Snapshots are kept only in memory, never written to disk.

**Restoring.**

- The new code's instance of the same component file restores the snapshot on its first start, before any other call. A snapshot with no matching component is discarded.
- An error from `restore` discards the state and is not a failure: the extension starts fresh.
- A trap in `restore` is a crash. During a reload's start it is a startup failure, so the package is paused and the state is lost.
- Development mode's diagnostics report a dropped, oversized, late or rejected snapshot. Elsewhere it is silent.
- A continuing service's task hands over the same way, so the new instance's first cycle finds the restored state.

**Format.** Opaque bytes the author versions. The SDKs offer helpers that serialise a value: serde in Rust, JSON in JavaScript and TypeScript.

**Reopening the screen (confirmed by the user: for every view command, whether or not the package opts in).**

- When one of the package's command screens was on display at the replacement, Pane opens that command again on the new code with its original launch record. The setup gate applies if the new code adds required preferences.
- Only the command's root view is reopened. Views it had pushed are not, unless the author restores them from the snapshot.
- Host-owned state is kept where the new tree has the same keys (#121): the command's search text, the selection, scroll and inputs.
- An automatic update never applies while a screen is on display, so this concerns Reload, development-mode reload and an update the user made.

**Samples:** a handoff sample in Rust, JavaScript and TypeScript that keeps a counter and a draft in memory and opens a screen to reopen.

**Docs:** reload and generations (ADR 0004's rule now defined), development mode, services and update. The glossary term state handoff goes through /domain-modeling.

## Acceptance criteria

- [ ] The counter and draft are kept across Reload, a user's Update, an automatic update and a development-mode reload, in all three languages.
- [ ] Nothing is handed over after a crash, a pause, a failed start, Retry, disable then enable, or a restart of the launcher.
- [ ] A failing replacement or build takes no snapshot and changes nothing.
- [ ] A busy instance gives no snapshot and is stopped as today.
- [ ] A snapshot over 1 MiB and one that misses the 1-second deadline are dropped, the reload goes ahead, and development mode's diagnostics report each (Rust fixtures).
- [ ] A `restore` that errs starts fresh. One that traps during a reload's start pauses the package (Rust fixtures).
- [ ] A continuing service's restored state is visible in its first cycle.
- [ ] The open screen of a package that does not opt in reopens with its original launch record and keeps its search text, selection and inputs where the keys match. Pushed views are not reopened.
- [ ] Core tests through `Launcher`. Prior art: the reload, develop, update, services and pausing suites.
- [ ] Window tests with real key events: the screen reopens after a development-mode reload. Prior art: the develop window tests.
- [ ] Prebuilt sample artifacts are rebuilt.
- [ ] The documents and the glossary entry above are updated.
- [ ] CI: done when one ci-fast verify run is green on the ticket branch. The release matrix is neither dispatched nor waited for.

## Blocked by

- #158
- #143
- #138

