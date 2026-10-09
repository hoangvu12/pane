## Problem Statement

Pane already updates extensions by itself (Q21, accepted; #49 for npm packages, #50 for tracked Git branches). Its updater checks each eligible package a second after Pane starts and every 24 hours, stages a newer version after the checks an install makes, and applies it at a safe activation boundary, keeping the package's identity, saved data, disabled state, hotkeys and aliases. The user can turn it off globally with "Update extensions automatically" or per package, in Manage extensions and on the Settings Extensions page. The user's decision of 2026-10-06 (decision 5, "automatic, with a setting to turn them off") confirms this direction, and nothing in it is re-specified here.

What the user still cannot do:

- **Default extensions never update.** The calculator, applications, quicklinks, files and clipboard history (and the Windows default extensions ADR 0040 adds) are acquired from Pane's artifact source at first setup (#53), but a newer payload is taken only if the user uninstalls the extension and restarts Pane. So the extensions everyone has are the only ones that never update.
- **No "check now" and no "Update all".** Q21 accepted manual Update/Update all beside the automatic updates, but the only manual path is choosing one package's source again and pressing Update on its preview. A user who hears about a fix cannot ask Pane to fetch it now.
- **No overview of what happened.** Each background update says its outcome in the status line of root search or the extension list, one package at a time, and only if that screen is showing. There is no list of what was updated, what was skipped and why, and what failed, as Raycast's update results give (Updated, Skipped, Failed). A failure that happens while the launcher is hidden is never seen.

## Solution

- Default extensions update like npm and Git packages: Pane reads the artifact source's index on the updater's cadence and, when it names a different version of an installed default extension, downloads, verifies and stages that payload exactly as first setup acquires one, and applies it at the same safe boundary under the same controls.
- A **Check for Extension Updates** command in root search, and a **Check for updates** button on the Settings Extensions page, check every updatable package at once and apply what they find (Update all). Progress shows as a toast.
- Each pass, automatic or asked for, records its **update results**, grouped as Updated, Skipped and Failed. A view lists them; a failure in a background pass is announced the next time the launcher shows, so it is not lost.

Pane keeps its integrity checks, its install-time compatibility checks and its all-or-nothing replacement. A newer version whose code fails to start is not rolled back to the older one: the package is paused with Retry, as Q31 and ADR 0004 decide.

## User Stories

1. As a launcher user, I want Pane's default extensions to receive fixes and improvements automatically, so that the extensions I use most are not the ones left behind.
2. As a launcher user, I want a default extension's update to keep its settings, content, disabled state, hotkeys, aliases and quick slots, so that an update never undoes my setup.
3. As a launcher user, I want a default extension I disabled to stay disabled across its updates, so that disabling remains my opt-out.
4. As a launcher user, I want "Update extensions automatically" and each extension's own switch to govern default extensions too, so that one set of controls covers every extension.
5. As a launcher user, I want a default extension's update to be checked against Pane's published integrity before anything replaces it, so that a damaged or altered download is never installed.
6. As a launcher user, I want a default extension update that needs a newer Pane to be skipped with that reason, so that I know to update Pane itself.
7. As a launcher user, I want a "Check for Extension Updates" command in root search, so that I can fetch a fix as soon as I hear of it.
8. As a launcher user, I want a "Check for updates" button on the Settings Extensions page, with when Pane last checked, so that I can update from where I manage extensions.
9. As a launcher user, I want checking now to update every extension that has a newer compatible version, including ones whose automatic updates I turned off, so that "Update all" means all.
10. As a launcher user, I want checking now to leave pinned versions, pinned Git revisions, local folders and development copies alone and say why, so that my explicit choices hold.
11. As a launcher user, I want a toast showing progress while a check I asked for runs ("Checking 6 extensionsâ€¦", "Updating 2 of 3â€¦"), so that I know it is working.
12. As a launcher user, I want the check to end with a summary ("Updated 3 extensions", "Extensions are up to date", "Updated 2, 1 failed"), so that I know the result at a glance.
13. As a launcher user, I want a "View Details" action on that summary, so that I can see each extension's outcome.
14. As a launcher user, I want the results grouped as Updated, Skipped and Failed, empty groups hidden, so that I see first what changed and what needs attention.
15. As a launcher user, I want each updated extension to show its old and new version, so that I know what changed.
16. As a launcher user, I want each skipped extension to say why (pinned, a local or development copy, automatic updates off, disabled, paused, needs a newer Pane, not available on this system), so that skipping never looks like a fault.
17. As a launcher user, I want each failed extension to say what failed (its source could not be reached, the download did not match its integrity, the new revision holds only source, a dependency could not be installed) and that it keeps running its installed version, so that I am not worried about losing it.
18. As a launcher user, I want to retry a failed extension from the results, so that a passing network problem is easy to recover from.
19. As a launcher user, I want to copy a failure's details, so that I can report it to the extension's author.
20. As a launcher user, I want to open an extension's Settings card from its result row, so that I can change its update switch or pin.
21. As a launcher user, I want a background update that failed while the launcher was hidden to be announced once the next time I open Pane, so that failures are not silently lost.
22. As a launcher user, I want successful background updates to stay quiet, so that routine maintenance does not interrupt me.
23. As a launcher user, I want the latest results kept across restarts until the next pass, so that I can look at them later.
24. As a launcher user, I want an extension updated while I use another to leave my current screen alone, so that updating never interrupts work.
25. As a launcher user, I want a new version that fails to start to be paused with Retry and its details, as any failed start is, so that I can tell the author and Pane stays usable.
26. As a launcher user, I want an update that installs new required dependencies to install them all or change nothing, so that a half-applied update never breaks the extension.
27. As a launcher user, I want the update check not to slow Pane's start, so that opening Pane stays fast.
28. As a keyboard user, I want the results view to work like any list: search, arrows, Enter and the Actions panel, so that I need no pointer.
29. As an extension author, I want my package's `CHANGELOG.md`, when it ships one, to be viewable from the Updated row, so that users can see what my release changed.
30. As a contributor, I want default-extension updates tested against a local artifact source, so that no check reaches Pane's real downloads.

## Implementation Decisions

### What exists and stays

- Q21 and decision 5: automatic extension updates are on unless the user turns them off, with a global choice and per-package choices, recorded in `extensions/updates.json`. The updater thread, the safe activation boundary (no screen of the package on display, no user-requested call of it running), the "is updating" refusal in the moment of replacement, and the update semantics (identity, saved data, disabled state, hotkeys and aliases kept; the old generation ends) are #49/#50's and are unchanged.
- Installing a replacement is already all or nothing: the new managed copy is written beside the old one, the installed-package record switches in one write, the old copy is removed only afterwards, and an update that installs required dependencies removes again what it added if any part fails (#42). This specification keeps that transaction and adds no new rollback. In particular, **no automatic return to an older version**: a replacement that passes its checks but fails to start pauses the package with Retry and diagnostics (Q31, ADR 0004). The results record it under Failed with that explanation.
- The provisional choices of #49/#50 (the cadence, the one-second first wait, the one-second retry, not updating disabled or paused packages automatically, following a `latest` tag or tracked branch that moves backwards) remain provisional; decision 5 does not settle them. Kept here, except that the first automatic check moves to one minute after start, so the check never competes with Pane's own start (proposed default, delegated: "like Raycast, as flexible as possible"; the launcher's clock seam keeps tests deterministic).

### Default extensions update

- A default extension (identified as `default:<id>`, #53) becomes eligible for automatic updates on the same terms as an unpinned npm package: enabled, not paused, not turned off. On each check the updater reads the artifact source's index (`pane-defaults.json`) once for all default extensions; an entry whose version differs from the installed `defaultVersion` is downloaded, verified against the index's sha512 integrity and size, unpacked and checked exactly as first-setup acquisition does, planned as an install, and staged. It is applied at the safe boundary as any update.
- The payload cache under `extensions/acquired/` keeps the payload of the installed version and the staged one only, as it does today (another version's entry replaces the older).
- A default extension the user uninstalled is still never re-acquired (#53's rule), and an update never re-enables a disabled one.
- An index entry whose package needs a newer extension API than this Pane provides is Skipped with "needs a newer Pane"; when Pane's own application update exists, the reason says so.
- Release builds read only the published artifact source; development builds and tests use `PANE_ARTIFACTS` on a loopback address, as acquisition does.
- Default extensions get the per-package "Update automatically" switch in Manage extensions and on their Settings card, like npm and Git packages.

### Checking now (Update all)

- A Pane command, **Check for Extension Updates**, appears in root search, and the Settings Extensions page gains **Check for updates** beside "Update extensions automatically", with "Last checked â€¦" under it. Both start the same pass at once, whatever the cadence (the updater's existing "check now").
- A pass the user asked for includes every installed npm package that is not pinned, every tracked Git package and every default extension, whether or not its automatic updates are turned off, and also disabled and paused ones, since the user asked (proposed default, delegated) (an update keeps a disabled package disabled; an update of a paused package unpauses it, as the preview's Update row already does). Pinned versions, pinned Git revisions, local folders and development copies are listed as Skipped with their reason. An automatic pass keeps today's eligibility.
- Checks run a few packages at a time (proposed: four), each bounded by Pane's existing HTTP limits; staging and applying follow the existing path, so a package in use is applied when it becomes quiet and is listed as "Waiting until <title> is not in use" until then.
- Progress shows as one toast that the pass updates ("Checking for extension updatesâ€¦", "Updating 2 of 3â€¦"), ending as success ("Updated 3 extensions", "Extensions are up to date") or failure ("Updated 2, 1 failed") with a **View Details** action. The toast is the host's own use of the toast surface defined by the "Extension commands like Raycast" specification (#120); until that lands, the status line carries the same text and the results stay reachable from the command.

### Update results

- Each pass records its results as Pane's own record beside `updates.json` (not extension data): for each package considered, its identity, title, group and detail. **Updated**: the old and new version (or commit). **Skipped**: the reason. **Failed**: the explanation the status line gives today, ending "It keeps running its installed code", or the startup failure that paused it. The record of the latest pass that changed or failed anything is kept across restarts; a pass that found nothing new does not replace it.
- **The results view** is a Pane screen opened from View Details, from the Check for Extension Updates command once a pass has results, and from the Settings Extensions page. It lists the groups in the order Updated, Skipped, Failed, hiding empty ones, each row with the extension's icon, title, detail and a status tag, and a search field. Row actions: Show Extension (its Settings card), Copy Details, Retry (Failed rows; checks that package alone), Update Now (Skipped rows whose only reason is the user's switch), and Show Changes (Updated rows of a package that ships `CHANGELOG.md`, shown as Markdown once the extension UI's Markdown UI component exists and as plain text until then).
- **Background passes** stay silent when everything succeeds or is skipped. If any package failed, Pane announces it once, the next time the launcher is shown, with a failure toast ("1 extension update failed") carrying View Details; the announcement is not repeated for the same failure.
- The status-line messages of #49/#50 for single background updates are replaced by these results and the announcement.

### Modules touched

- The updater in the launcher core: a default-extension source beside npm and Git, the user-asked pass and its eligibility, the results record and the announcement state; reachable through `Launcher` (a "check now" call, the results, the view).
- First-setup acquisition: its download, verification and unpacking reused for staging an update.
- The window: the results view, the toast and its action, the Settings Extensions page's button and last-checked line.

## Testing Decisions

- Good tests drive the `Launcher` public interface against controlled sources and assert what users see and what is installed: the version, the kept data and controls, the results' groups and reasons. They never reach a real registry, repository or Pane's downloads.
- **Primary seam: `pane_core::Launcher`, extending `crates/pane-core/tests/update.rs`** (npm packages from the loopback registry, Git packages from `git`-made repositories served on 127.0.0.1, waiting with `Launcher::wait_for_updates` and the launcher's clock), plus the artifact-source fixture `crates/pane-core/tests/installer.rs` already serves for first setup. Cases: a default extension updated by itself with its data and controls kept; one disabled, paused, turned off or uninstalled-with-kept-data not updated automatically; an integrity mismatch and a payload needing a newer API landing in Failed and Skipped; check now including turned-off, disabled and paused packages and skipping pinned, local and development copies; a package in use waiting; the results' groups, details, persistence across restarts and replacement rules; Retry and Update Now; the background failure announced once; an update that installs a required dependency failing part-way and changing nothing; a new version that fails to start paused, not rolled back.
- **Window tests** in `crates/pane/tests` (prior art `update.rs`, `settings.rs`) with real key events: the command and the Settings button start a pass, the toast's progression and View Details, the results view's groups, search and actions.
- No new adapter or native smoke is needed: nothing here is platform-specific beyond what acquisition already covers.

## Out of Scope

- Automatic updates themselves, their controls, eligibility and safe boundary: shipped by #49/#50 under Q21.
- Returning to an earlier version after a failed start, and a version history or downgrade picker (Q31, ADR 0004, Q32).
- Updates of Pane itself (Q38, #54 to #56), including any change to their notify-then-choose policy.
- Removing extensions remotely (Raycast's withdrawn-extension removal): Pane has no store authority (Q35).
- Private registries and repositories, update channels or tags other than `latest`, and updating local folders (development mode reloads those).

## Further Notes

- Corrects `docs/research/raycast-deep-dive.md`'s "Pane updates on request" (its decision 5): Pane has updated npm and tracked Git packages by itself since #49/#50. No new ADR is needed for decision 5; Q21 records it.
- Raycast facts used: it updates extensions silently on a scheduled task (hourly, then four hours after a successful pass), with no off switch; its Check for Extension Updates command shows a pending toast, then "Update finished!" or "You're up to date!" with View Details, opening results grouped as Updated, Skipped, Failed (and Removed for extensions withdrawn from its store), with View Version History, Show Extension Details and Copy Details actions. Pane keeps its off switch and its longer cadence (provisional), and takes the on-demand check and grouped results.
- This specification is small and stays a separate specification (coordinator's call). It uses the toast of the "Extension commands like Raycast" specification (#120), drawn and timed as the "Launcher polish" specification (#123) describes (a success or failure toast hides after 3 s; the progress toast stays until the pass ends). The default extensions ADR 0040 adds (Switch Windows, System Commands, Run and Windows Settings) update as default extensions under the rules here.
- Glossary: "Update results" may deserve an entry in `CONTEXT.md`, and "Automatic updates" should name default extensions among the eligible packages; both through `/domain-modeling`.


---

Decisions this specification relies on are recorded in [ADRs 0030â€“0040](https://github.com/hoangvu12/pane/tree/main/docs/adr) (added by [#119](https://github.com/hoangvu12/pane/pull/119)); the evidence is [docs/research/raycast-deep-dive.md](https://github.com/hoangvu12/pane/blob/main/docs/research/raycast-deep-dive.md).
