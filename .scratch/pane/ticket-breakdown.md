# Pane implementation ticket index

**Status:** ready-for-agent
**Revision:** 3, 2026-09-28.
**Parent:** [Pane specification](spec.md), unchanged.

The earlier 48-ticket breakdown followed the format but mixed large validation/decision tasks with feature slices. This revision has **52 implementation slices**, each with a concrete outcome, acceptance checks, scope and a reason for every blocker. The count is not a quality claim: the review must still assess whether each slice fits one fresh context window.

## What changed

- The first outcome is a real Rust command responding through native UI. JS/TS support and actual macOS/Linux contributor builds follow immediately; native contributor baselines gate broader shared features.
- Runtime feasibility, licensing and measured-target decisions are explicit [planning prerequisites](planning-prerequisites.md), not disguised feature tickets. Only P1 blocks a numbered slice (02); P2/P3 gate corresponding distribution/readiness claims.
- Build-on-save adapters, scheduled tasks/services, cache/uninstall/deletion, dependent disable/uninstall and OS hotkeys are separate outcomes.
- Each OS has its own app discovery, installer and app-update slices. A macOS/Linux installer or updater does not wait for a Windows equivalent.
- A URL quicklink and configured-folder search each have one shared end-to-end contract with native checks on the three baselines; there are no bundled app/file/quicklink platform tickets.
- Author examples, reproduction instructions and targeted checks accompany the features that introduce them. There is no catch-all onboarding implementation ticket.
- Release-wide evidence and per-platform resource measurements live in the [release checklist](release-validation.md). Failures create bounded corrective work; no platform waits for a different platform's benchmark or release approval.

## Reading and execution rules

Read the [contributor requirement](contributor-platform-requirement.md) and parent/current decisions. The runtime backend and testing-boundary feedback remain as documented; this revision does not silently settle them. The initial implementation frontier is **01**, with P1 feasibility work independently resolvable. The 52 slices are published as individual files under `issues/`. Work any ticket whose numbered blockers and linked planning prerequisites are complete. `ready-for-agent` describes specification readiness; it does not mean the ticket is unblocked. Ticket 02 remains blocked by both 01 and P1 until P1's exit evidence is recorded in the planning prerequisites.

Each ticket implements all layers necessary for its stated outcome. A tiny initial list/action contract is introduced inside the first working interaction; there is no separate "build the SDK", "build the UI" or "write all tests" ticket. No wide refactor or prefactoring phase is justified by this prototype-only workspace.

## Published tickets

Numbers are dependency order, not a mandatory serial queue. A blocked-by entry names a direct prerequisite; inherited prerequisites are omitted. P1 is the external feasibility prerequisite described above.

### First complete interaction and native contributors

1. **[Run one Rust command in Pane's native window](issues/01-run-one-rust-command-in-pane-s-native-window.md)**

   **Blocked by:** None.

   **Delivers:** A user opens Pane, runs a bundled Rust sample command, selects a result, and sees the guest's response in the native window.

2. **[Run JS and TS versions of the native sample command](issues/02-run-js-and-ts-versions-of-the-native-sample-command.md)**

   **Blocked by:** 01, P1.

   **Delivers:** An author builds JS and TS samples whose actions update the same native view as the Rust sample.

3. **[Build and run the sample command natively on macOS](issues/03-build-and-run-the-sample-command-natively-on-macos.md)**

   **Blocked by:** 02.

   **Delivers:** A macOS contributor builds Pane and the Rust/JS/TS samples locally, then completes the native action/result interaction.

4. **[Build and run the sample command natively on Linux](issues/04-build-and-run-the-sample-command-natively-on-linux.md)**

   **Blocked by:** 02.

   **Delivers:** A Linux contributor builds Pane and the Rust/JS/TS samples locally, then completes the native action/result interaction.

### Installation, lifecycle and recovery

5. **[Install and run a local extension package](issues/05-install-and-run-a-local-extension-package.md)**

   **Blocked by:** 03, 04.

   **Delivers:** A user selects a supported local package in Pane, sees its identity and compatibility, installs it and invokes its command.

6. **[Disable an extension and retain its settings after restart](issues/06-disable-an-extension-and-retain-its-settings-after-restart.md)**

   **Blocked by:** 05.

   **Delivers:** A user disables a local extension in the UI, restarts Pane, and can re-enable it with its saved settings intact.

7. **[Reload one extension without restarting Pane](issues/07-reload-one-extension-without-restarting-pane.md)**

   **Blocked by:** 06.

   **Delivers:** Manual reload replaces one extension while Pane and an unrelated extension stay open, preserving saved data and reporting failed replacement startup.

8. **[Reload a Rust extension after saving valid source](issues/08-reload-a-rust-extension-after-saving-valid-source.md)**

   **Blocked by:** 07.

   **Delivers:** A Rust author edits the sample, saves it, and sees the new behavior while Pane stays open.

9. **[Reload JS and TS extensions after saving valid source](issues/09-reload-js-and-ts-extensions-after-saving-valid-source.md)**

   **Blocked by:** 07.

   **Delivers:** A JS/TS author edits the sample, saves it, and sees the new behavior while Pane stays open.

10. **[Discard late guest results after reload or disable](issues/10-discard-late-guest-results-after-reload-or-disable.md)**

   **Blocked by:** 07.

   **Delivers:** Reloading or disabling an extension with pending async work stops managed resources and prevents old replies from changing the current UI.

11. **[Run and stop a packaged native helper](issues/11-run-and-stop-a-packaged-native-helper.md)**

   **Blocked by:** 10.

   **Delivers:** A component command invokes an OS-matched prebuilt helper, shows its result, and stops the managed process on cancel, disable or reload.

12. **[Pause an attributable broken extension and offer Retry](issues/12-pause-an-attributable-broken-extension-and-offer-retry.md)**

   **Blocked by:** 10.

   **Delivers:** Pane skips an identified failing extension, shows a toast and persistent status, and allows Retry without requiring a CLI.

13. **[Keep recovery controls usable after a runtime crash](issues/13-keep-recovery-controls-usable-after-a-runtime-crash.md)**

   **Blocked by:** 11, 12.

   **Delivers:** A user sees that the extension runtime stopped, can open management and explicitly retry without replaying an action.

14. **[Recover through the UI when a guest stops responding](issues/14-recover-through-the-ui-when-a-guest-stops-responding.md)**

   **Blocked by:** 13.

   **Delivers:** A user can recover from a non-cooperating guest without closing Pane or losing saved data.

### Availability, views and composition

15. **[Explain an unavailable extension action without hiding working actions](issues/15-explain-an-unavailable-extension-action-without-hiding-working-actions.md)**

   **Blocked by:** 05.

   **Delivers:** Pane displays supported-OS information and explains unavailable actions while leaving supported actions usable.

16. **[Submit and validate a native form from an extension](issues/16-submit-and-validate-a-native-form-from-an-extension.md)**

   **Blocked by:** 03, 04.

   **Delivers:** A native extension form accepts input, validates it, and returns a visible guest result through the same contract in JS/TS and Rust.

17. **[Choose a color in an extension-owned interactive view](issues/17-choose-a-color-in-an-extension-owned-interactive-view.md)**

   **Blocked by:** 16.

   **Delivers:** A user adjusts a small color picker, the guest receives the change, and the native view displays the selected value.

18. **[Call an explicit operation in another extension](issues/18-call-an-explicit-operation-in-another-extension.md)**

   **Blocked by:** 06.

   **Delivers:** An installed JS/TS command calls a Rust operation and a Rust command calls JS/TS, displaying structured results and meaningful target errors.

### Root search and default actions

19. **[Find and invoke installed commands through root search](issues/19-find-and-invoke-installed-commands-through-root-search.md)**

   **Blocked by:** 06.

   **Delivers:** Root search finds installed command metadata and activates only the selected enabled extension.

20. **[Launch a Windows application from root search](issues/20-launch-a-windows-application-from-root-search.md)**

   **Blocked by:** 15, 19.

   **Delivers:** A user finds an installed Windows application in Pane, launches it, and can disable that default extension.

21. **[Launch a macOS application from root search](issues/21-launch-a-macos-application-from-root-search.md)**

   **Blocked by:** 15, 19.

   **Delivers:** A user finds an installed macOS application in Pane, launches it, and can disable that default extension.

22. **[Launch a Linux application from root search](issues/22-launch-a-linux-application-from-root-search.md)**

   **Blocked by:** 15, 19.

   **Delivers:** A user finds an installed Linux application in Pane, launches it, and can disable that default extension.

23. **[Show a calculator result in root search](issues/23-show-a-calculator-result-in-root-search.md)**

   **Blocked by:** 19.

   **Delivers:** Typing a supported expression into root search returns a calculator result from a disableable default extension.

24. **[Create and invoke a persistent URL quicklink](issues/24-create-and-invoke-a-persistent-url-quicklink.md)**

   **Blocked by:** 16, 19.

   **Delivers:** A user saves a URL quicklink, finds it in root search after restarting Pane, and opens it in their default browser.

25. **[Find and open a file within a selected folder](issues/25-find-and-open-a-file-within-a-selected-folder.md)**

   **Blocked by:** 10, 15, 16, 19.

   **Delivers:** A user chooses a folder, searches its supported files through a default extension, and opens a result.

### Online commands and quick access

26. **[Search an online service inside its command](issues/26-search-an-online-service-inside-its-command.md)**

   **Blocked by:** 10, 19.

   **Delivers:** Opening an online extension command and entering a query displays service results without querying it during ordinary root search.

27. **[Invoke a command through an alias or explicit fallback](issues/27-invoke-a-command-through-an-alias-or-explicit-fallback.md)**

   **Blocked by:** 26.

   **Delivers:** A user configures an alias or fallback for an installed command and invokes it from root search.

28. **[Open an extension command with a global hotkey on Windows](issues/28-open-an-extension-command-with-a-global-hotkey-on-windows.md)**

   **Blocked by:** 19, 15.

   **Delivers:** A user assigns a supported Windows shortcut and opens the selected command while another application has focus.

29. **[Open an extension command with a global hotkey on macOS](issues/29-open-an-extension-command-with-a-global-hotkey-on-macos.md)**

   **Blocked by:** 19, 15.

   **Delivers:** A user assigns a supported macOS shortcut and opens the selected command while another application has focus.

30. **[Open an extension command with a global hotkey on Linux](issues/30-open-an-extension-command-with-a-global-hotkey-on-linux.md)**

   **Blocked by:** 19, 15.

   **Delivers:** A user assigns a supported Linux shortcut and opens the selected command while another application has focus.

### Clipboard history

31. **[Capture opt-in clipboard history on Windows](issues/31-capture-opt-in-clipboard-history-on-windows.md)**

   **Blocked by:** 06, 15.

   **Delivers:** A Windows clipboard extension starts off, captures supported clipboard content only when enabled, and supports visible pause/disable controls.

32. **[Expire and delete saved clipboard history](issues/32-expire-and-delete-saved-clipboard-history.md)**

   **Blocked by:** 31.

   **Delivers:** Clipboard history has configurable finite retention and deletion controls whose effects remain correct while disabled and after downtime.

33. **[Capture opt-in clipboard history on macOS](issues/33-capture-opt-in-clipboard-history-on-macos.md)**

   **Blocked by:** 32.

   **Delivers:** A macOS user enables clipboard history, sees a captured item, and can pause, delete or expire it through Pane.

34. **[Capture opt-in clipboard history on Linux](issues/34-capture-opt-in-clipboard-history-on-linux.md)**

   **Blocked by:** 32.

   **Delivers:** A Linux user enables clipboard history, sees a captured item, and can pause, delete or expire it through Pane.

### Data and required dependencies

35. **[Clear an extension's cache without deleting saved data](issues/35-clear-an-extension-s-cache-without-deleting-saved-data.md)**

   **Blocked by:** 06.

   **Delivers:** A user clears one extension's disposable cache while its settings, content and local credentials remain intact.

36. **[Uninstall an extension with an explicit saved-data choice](issues/36-uninstall-an-extension-with-an-explicit-saved-data-choice.md)**

   **Blocked by:** 35.

   **Delivers:** A user removes an extension and chooses whether to keep its durable data.

37. **[Delete retained data after its extension is uninstalled](issues/37-delete-retained-data-after-its-extension-is-uninstalled.md)**

   **Blocked by:** 36.

   **Delivers:** A user finds retained extension data in management and deletes it while the extension code is absent.

38. **[Install missing required dependencies with a local extension](issues/38-install-missing-required-dependencies-with-a-local-extension.md)**

   **Blocked by:** 18.

   **Delivers:** Installing a local fixture extension shows and installs its compatible missing required dependencies while preserving optional, disabled and pinned choices.

39. **[Disable required dependents together or cancel](issues/39-disable-required-dependents-together-or-cancel.md)**

   **Blocked by:** 38.

   **Delivers:** A user disabling a required dependency sees affected extensions and chooses Disable All or Cancel.

40. **[Uninstall required dependents together or cancel](issues/40-uninstall-required-dependents-together-or-cancel.md)**

   **Blocked by:** 39, 36.

   **Delivers:** A user removing a required dependency reviews the affected set and its saved-data choices before uninstalling.

### Distribution, background work and updates

41. **[Install and run an npm-distributed component package](issues/41-install-and-run-an-npm-distributed-component-package.md)**

   **Blocked by:** 38.

   **Delivers:** A user installs a supported npm package through Pane without manually installing npm or Node, then runs its component command.

42. **[Install and run a Git-distributed component package](issues/42-install-and-run-a-git-distributed-component-package.md)**

   **Blocked by:** 38.

   **Delivers:** A user installs a supported Git-sourced package or explicit revision through Pane without manual Git/compiler setup.

43. **[Run a scheduled extension task and stop it on disable](issues/43-run-a-scheduled-extension-task-and-stop-it-on-disable.md)**

   **Blocked by:** 12.

   **Delivers:** A user enables a scheduled task, sees its result/status in Pane, and disables it to stop future runs.

44. **[Run a continuing extension service and stop it on disable](issues/44-run-a-continuing-extension-service-and-stop-it-on-disable.md)**

   **Blocked by:** 12.

   **Delivers:** A user starts an enabled background service, sees its status, and stops it through extension management.

45. **[Update an eligible npm extension at a safe activation boundary](issues/45-update-an-eligible-npm-extension-at-a-safe-activation-boundary.md)**

   **Blocked by:** 41, 43, 44.

   **Delivers:** A user receives a compatible npm extension update under their chosen update controls, without replacing an active command.

46. **[Update a tracked Git extension without changing its identity](issues/46-update-a-tracked-git-extension-without-changing-its-identity.md)**

   **Blocked by:** 42, 45.

   **Delivers:** A user updates a tracked Git package through the existing controls while pinned revisions remain unchanged.

### Native installation and application updates

47. **[Install Pane and acquire its calculator on Windows](issues/47-install-pane-and-acquire-its-calculator-on-windows.md)**

   **Blocked by:** 11, 23.

   **Delivers:** A clean Windows machine installs Pane, acquires compatible runtime/default artifacts, and runs a calculator command without developer tools.

48. **[Install Pane and acquire its calculator on macOS](issues/48-install-pane-and-acquire-its-calculator-on-macos.md)**

   **Blocked by:** 11, 23.

   **Delivers:** A clean macOS machine installs Pane, acquires compatible runtime/default artifacts, and runs a calculator command without developer tools.

49. **[Install Pane and acquire its calculator on Linux](issues/49-install-pane-and-acquire-its-calculator-on-linux.md)**

   **Blocked by:** 11, 23.

   **Delivers:** A clean Linux machine installs Pane, acquires compatible runtime/default artifacts, and runs a calculator command without developer tools.

50. **[Install a Pane application update by user choice on Windows](issues/50-install-a-pane-application-update-by-user-choice-on-windows.md)**

   **Blocked by:** 47.

   **Delivers:** A Windows user sees an update notification and chooses whether to install it.

51. **[Install a Pane application update by user choice on macOS](issues/51-install-a-pane-application-update-by-user-choice-on-macos.md)**

   **Blocked by:** 48.

   **Delivers:** A macOS user sees an update notification and chooses whether to install it.

52. **[Install a Pane application update by user choice on Linux](issues/52-install-a-pane-application-update-by-user-choice-on-linux.md)**

   **Blocked by:** 49.

   **Delivers:** A Linux user sees an update notification and chooses whether to install it.

## Coverage and former draft numbers

[Machine-readable mappings](ticket-breakdown-checks.json) identify each requirement's contributing slices and decision/release work, along with old-to-new draft numbers. All 82 user stories and 25 scenario identifiers have mapped contributors. This is traceability, not proof of behavioral coverage or completed gates. The old revision 2 is recoverable from Git commit `ebe95dc`; no published issue numbers have been changed.

| Former draft | Current disposition |
| --- | --- |
| 01 | P1 feasibility/candidate input; Rust native integration is in 01 |
| 07 | Contributor commands and native checks are part of 01, 03 and 04; ongoing portability is a shared completion requirement |
| 35 | P3 measured targets plus per-platform release validation |
| 36 | P2 licensing decision and distribution evidence |
| 39 | Author examples/docs/checks ship with language, form/custom view, helper, reload and package slices; final reproduction check is in release validation |
| 46 | Windows release-validation checklist |
| 47 | macOS release-validation checklist |
| 48 | Linux release-validation checklist |

The other former drafts map to the implementation slices in the JSON. Existing research, design decisions, saved artifacts and the parent spec are preserved.

## Publication

2026-09-28: The user requested correction of the tracker structure after the structural review. Published all 52 existing slices under `issues/` with `ready-for-agent` status, retaining their numbering, scope and blockers.

The [to-tickets skill](C:/Users/ADMIN/.agents/skills/to-tickets/SKILL.md) supplies the per-ticket publication format. Structural checks verify required fields, numbering, dependency references and local file links. Publication does not establish runtime feasibility or completion. The testing-boundary proposal remains separately labeled pending feedback.

## Comments

2026-09-28: Revision 3 responds to the user's finding that the earlier breakdown did not follow the skill's slicing discipline. This revision changes planning artifacts only; no runtime tests or application implementation were performed.
