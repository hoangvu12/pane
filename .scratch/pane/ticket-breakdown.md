# Pane implementation ticket index

**Status:** ready-for-agent
**Parent:** [Pane specification](spec.md)

52 implementation issues. Each issue owns its behavior, acceptance criteria, scope and blocker rationale.

## Execution

Start with **01**. Work any issue whose numbered blockers and linked [planning prerequisites](planning-prerequisites.md) are complete. `ready-for-agent` describes specification readiness, not whether blockers are complete. Ticket 02 requires both 01 and P1; record P1 exit evidence before starting it. P2 and P3 gate corresponding distribution and performance claims.

Follow the [contributor requirement](contributor-platform-requirement.md), [current decisions](../../docs/current-decisions.md), and [release checklist](release-validation.md). The proposed testing boundary remains pending feedback.

## Issues

| Issue | Blocked by |
| --- | --- |
| [01 - Run one Rust command in Pane's native window](issues/01-run-one-rust-command-in-pane-s-native-window.md) | None |
| [02 - Run JS and TS versions of the native sample command](issues/02-run-js-and-ts-versions-of-the-native-sample-command.md) | 01, P1 |
| [03 - Build and run the sample command natively on macOS](issues/03-build-and-run-the-sample-command-natively-on-macos.md) | 02 |
| [04 - Build and run the sample command natively on Linux](issues/04-build-and-run-the-sample-command-natively-on-linux.md) | 02 |
| [05 - Install and run a local extension package](issues/05-install-and-run-a-local-extension-package.md) | 03, 04 |
| [06 - Disable an extension and retain its settings after restart](issues/06-disable-an-extension-and-retain-its-settings-after-restart.md) | 05 |
| [07 - Reload one extension without restarting Pane](issues/07-reload-one-extension-without-restarting-pane.md) | 06 |
| [08 - Reload a Rust extension after saving valid source](issues/08-reload-a-rust-extension-after-saving-valid-source.md) | 07 |
| [09 - Reload JS and TS extensions after saving valid source](issues/09-reload-js-and-ts-extensions-after-saving-valid-source.md) | 07 |
| [10 - Discard late guest results after reload or disable](issues/10-discard-late-guest-results-after-reload-or-disable.md) | 07 |
| [11 - Run and stop a packaged native helper](issues/11-run-and-stop-a-packaged-native-helper.md) | 10 |
| [12 - Pause an attributable broken extension and offer Retry](issues/12-pause-an-attributable-broken-extension-and-offer-retry.md) | 10 |
| [13 - Keep recovery controls usable after a runtime crash](issues/13-keep-recovery-controls-usable-after-a-runtime-crash.md) | 11, 12 |
| [14 - Recover through the UI when a guest stops responding](issues/14-recover-through-the-ui-when-a-guest-stops-responding.md) | 13 |
| [15 - Explain an unavailable extension action without hiding working actions](issues/15-explain-an-unavailable-extension-action-without-hiding-working-actions.md) | 05 |
| [16 - Submit and validate a native form from an extension](issues/16-submit-and-validate-a-native-form-from-an-extension.md) | 03, 04 |
| [17 - Choose a color in an extension-owned interactive view](issues/17-choose-a-color-in-an-extension-owned-interactive-view.md) | 16 |
| [18 - Call an explicit operation in another extension](issues/18-call-an-explicit-operation-in-another-extension.md) | 06 |
| [19 - Find and invoke installed commands through root search](issues/19-find-and-invoke-installed-commands-through-root-search.md) | 06 |
| [20 - Launch a Windows application from root search](issues/20-launch-a-windows-application-from-root-search.md) | 15, 19 |
| [21 - Launch a macOS application from root search](issues/21-launch-a-macos-application-from-root-search.md) | 15, 19 |
| [22 - Launch a Linux application from root search](issues/22-launch-a-linux-application-from-root-search.md) | 15, 19 |
| [23 - Show a calculator result in root search](issues/23-show-a-calculator-result-in-root-search.md) | 19 |
| [24 - Create and invoke a persistent URL quicklink](issues/24-create-and-invoke-a-persistent-url-quicklink.md) | 16, 19 |
| [25 - Find and open a file within a selected folder](issues/25-find-and-open-a-file-within-a-selected-folder.md) | 10, 15, 16, 19 |
| [26 - Search an online service inside its command](issues/26-search-an-online-service-inside-its-command.md) | 10, 19 |
| [27 - Invoke a command through an alias or explicit fallback](issues/27-invoke-a-command-through-an-alias-or-explicit-fallback.md) | 26 |
| [28 - Open an extension command with a global hotkey on Windows](issues/28-open-an-extension-command-with-a-global-hotkey-on-windows.md) | 19, 15 |
| [29 - Open an extension command with a global hotkey on macOS](issues/29-open-an-extension-command-with-a-global-hotkey-on-macos.md) | 19, 15 |
| [30 - Open an extension command with a global hotkey on Linux](issues/30-open-an-extension-command-with-a-global-hotkey-on-linux.md) | 19, 15 |
| [31 - Capture opt-in clipboard history on Windows](issues/31-capture-opt-in-clipboard-history-on-windows.md) | 06, 15 |
| [32 - Expire and delete saved clipboard history](issues/32-expire-and-delete-saved-clipboard-history.md) | 31 |
| [33 - Capture opt-in clipboard history on macOS](issues/33-capture-opt-in-clipboard-history-on-macos.md) | 32 |
| [34 - Capture opt-in clipboard history on Linux](issues/34-capture-opt-in-clipboard-history-on-linux.md) | 32 |
| [35 - Clear an extension's cache without deleting saved data](issues/35-clear-an-extension-s-cache-without-deleting-saved-data.md) | 06 |
| [36 - Uninstall an extension with an explicit saved-data choice](issues/36-uninstall-an-extension-with-an-explicit-saved-data-choice.md) | 35 |
| [37 - Delete retained data after its extension is uninstalled](issues/37-delete-retained-data-after-its-extension-is-uninstalled.md) | 36 |
| [38 - Install missing required dependencies with a local extension](issues/38-install-missing-required-dependencies-with-a-local-extension.md) | 18 |
| [39 - Disable required dependents together or cancel](issues/39-disable-required-dependents-together-or-cancel.md) | 38 |
| [40 - Uninstall required dependents together or cancel](issues/40-uninstall-required-dependents-together-or-cancel.md) | 39, 36 |
| [41 - Install and run an npm-distributed component package](issues/41-install-and-run-an-npm-distributed-component-package.md) | 38 |
| [42 - Install and run a Git-distributed component package](issues/42-install-and-run-a-git-distributed-component-package.md) | 38 |
| [43 - Run a scheduled extension task and stop it on disable](issues/43-run-a-scheduled-extension-task-and-stop-it-on-disable.md) | 12 |
| [44 - Run a continuing extension service and stop it on disable](issues/44-run-a-continuing-extension-service-and-stop-it-on-disable.md) | 12 |
| [45 - Update an eligible npm extension at a safe activation boundary](issues/45-update-an-eligible-npm-extension-at-a-safe-activation-boundary.md) | 41, 43, 44 |
| [46 - Update a tracked Git extension without changing its identity](issues/46-update-a-tracked-git-extension-without-changing-its-identity.md) | 42, 45 |
| [47 - Install Pane and acquire its calculator on Windows](issues/47-install-pane-and-acquire-its-calculator-on-windows.md) | 11, 23 |
| [48 - Install Pane and acquire its calculator on macOS](issues/48-install-pane-and-acquire-its-calculator-on-macos.md) | 11, 23 |
| [49 - Install Pane and acquire its calculator on Linux](issues/49-install-pane-and-acquire-its-calculator-on-linux.md) | 11, 23 |
| [50 - Install a Pane application update by user choice on Windows](issues/50-install-a-pane-application-update-by-user-choice-on-windows.md) | 47 |
| [51 - Install a Pane application update by user choice on macOS](issues/51-install-a-pane-application-update-by-user-choice-on-macos.md) | 48 |
| [52 - Install a Pane application update by user choice on Linux](issues/52-install-a-pane-application-update-by-user-choice-on-linux.md) | 49 |

## Coverage

[Coverage and dependency data](ticket-breakdown-checks.json) maps the 82 user stories, 25 acceptance scenarios and engineering gates to implementation issues and supporting work. These references do not demonstrate passing behavior. Previous draft versions are available in Git history.

## Comments

2026-09-28: Revision 3 responds to the user's finding that the earlier breakdown did not follow the skill's slicing discipline. This revision changes planning artifacts only; no runtime tests or application implementation were performed.

2026-09-28: Published the 52 issues at the user's request, then removed obsolete draft data and duplicate ticket prose. Scope, acceptance criteria and unresolved decisions are unchanged.
