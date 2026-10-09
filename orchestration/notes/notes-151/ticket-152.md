## Parent

https://github.com/hoangvu12/pane/issues/151

## What to build

Today, when a required dependency is disabled outside Disable all, paused after crashing, or uninstalled outside Uninstall all, its dependents stay listed as if nothing were wrong: their commands open and fail, their schedules answer `disabled`, their services cycle against a target that cannot answer, and their root results ask a package that cannot serve. This ticket makes them **waiting commands** (ADR 0041), using only the dependencies `pane.json` already declares. No manifest field is added. It is size M.

**Who waits.** A command of an enabled, unpaused package waits while one of its package's required dependencies is missing, disabled, paused or waiting itself. Optional dependencies never make a command wait. Pane computes this from the manifests and the packages' states: every enabled, unpaused package starts as able to run, and any package with an unmet requirement is removed, repeating until nothing changes. So a cycle of healthy packages runs, and a cycle with one member missing waits as a whole. It is recomputed on install, uninstall, enable, disable, pause, Retry, reload and update, and the parts of Pane that run a package's work are told when it changes.

**What waiting means.**

- The command's view, run entry point, actions, arguments and setup screen do not run.
- Its schedule's ticks are skipped and not replayed, its service does not cycle, and root search does not ask for its root or indexed results.
- A package waiting as a whole answers its published operations with `unavailable` ("<title> is waiting for <what>").
- Waiting ends no generation and stops no instance. A call or cycle already running finishes, and an open screen stays, its calls answering as calls do.
- Waiting never counts towards pausing.

**How it shows.** Waiting is a reason kind of its own beside paused and other-system, shown as a paused command's reason is:

- the root search row stays, with "Needs <title>, which is <state>" (not installed, disabled, paused, or waiting for something else);
- a chain names what is actually missing ("Needs Notes Sync, which waits for Auth: Auth is disabled");
- Enter shows the reason with fix rows: "Enable <title>", "Retry <title>" or "Open Manage extensions";
- its quick slots, aliases, global hotkeys and fallbacks say the same and run nothing.

**Coming back.** When the requirement is met again, with nothing for the user to do, the row is ordinary again, a schedule starts from a full interval, a service's first cycle runs at once in the instance it still has, and root search asks for its results on the next query.

**Behaviour that changes.** Enabling a dependent whose required dependency is disabled is still allowed, but the dependent now waits instead of its calls answering `disabled`. When Pane pauses a required dependency, its dependents now wait instead of having their calls refused, and come back on Retry. Disable all and Uninstall all are unchanged.

**Docs:** dependencies (its limits lose "no Pane-side view" for the root search row; Manage extensions' rows come with the Manage extensions ticket, 6), pausing, schedules, services and generations.

## Acceptance criteria

- [ ] A dependent waits, through its row, while its required dependency is disabled, paused, uninstalled (through `Launcher::uninstall`), or itself waiting, and comes back on enable, Retry or install without any action on the dependent.
- [ ] A reload or update of the dependency that fails to start makes its dependents wait. A successful one brings them back.
- [ ] While waiting, the command's view is refused with the reason, its schedule's ticks are skipped (manual clock and `wait_for_schedules`), its service does not cycle (`wait_for_services`), and its root and indexed results are not asked for.
- [ ] On coming back, the schedule starts from a full interval, the service cycles at once, and root search asks on the next query.
- [ ] A package waiting as a whole answers its published operations `unavailable` with the reason.
- [ ] Quick slot, alias, global hotkey and fallback of a waiting command show the reason and run nothing.
- [ ] Waits three deep name the root cause. A cycle of healthy packages runs, and a cycle with one member missing waits as a whole.
- [ ] Optional dependencies never make a command wait.
- [ ] Waiting never counts towards pausing, ends no generation, and leaves an open screen and calls in flight alone.
- [ ] The disable-dependents, uninstall-dependents and pausing suites are adjusted to the new behaviour: a paused dependency's dependents now wait.
- [ ] Core tests through `Launcher` with real samples and Rust fixtures (the cycles and the three-deep chain). Prior art: the dependencies, disable dependents, uninstall dependents, pausing, schedules, services, quick slots and aliases suites.
- [ ] Window tests with real key events: a waiting row's text, and Enter showing the reason and fix rows.
- [ ] The documents listed above are updated.
- [ ] CI: done when one ci-fast verify run is green on the ticket branch. The release matrix is neither dispatched nor waited for.

## Blocked by

None — can start immediately

