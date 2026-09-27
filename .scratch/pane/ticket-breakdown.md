# Pane ticket breakdown for review

Draft: awaiting user approval of granularity and blocking edges.
Parent: [Pane specification](spec.md).
Prepared and revised: 2026-09-28.
Contributor requirement: [user clarification and acceptance boundary](contributor-platform-requirement.md).

This is a complete proposed breakdown, not published ready-for-agent issues and not authorization to execute implementation. Each linked draft is a separate, self-contained slice with scope, blockers, acceptance criteria and evidence requirements. After approval, publish one file per ticket in the configured local issues directory. Preserve the parent spec unchanged.

## Review summary

- 48 slices cover all 82 user stories, all eight gates and all 25 planned acceptance scenarios.
- The initial frontier is 01 (runtime candidate) and 02 (native shell). Existing prototype evidence is the starting point, not proof of production readiness.
- The first integrated milestone is 04: real JS/TS and Rust commands interacting with the native UI.
- Development reload, resource cleanup and recovery follow as separate behaviors; standard forms and custom views also have separate slices.
- macOS (05) and Linux (06) build/run real guests immediately after 04. Contributor workflow 07 gates the broad shared extension work: all three systems must build Pane and guest examples and run native checks.
- Windows preview readiness (46) requires that early three-platform contributor baseline. It does not wait for later macOS/Linux feature integrations, installers or release-readiness checks (40-45, 47-48). Platform work belongs in the shared project; these are dependency branches, not deferred or isolated source forks.
- Shared build tools, SDK examples and development reload must work on Windows/macOS/Linux. Portable commands and a native automated check matrix are required; the CI provider is not predetermined.
- macOS/Linux work requires real target environments. Those are explicit execution prerequisites, not assumed resources available in this Windows workspace.
- Gate/validation tickets produce concrete runnable evidence or a resolved decision; an unresolved failure is not completion.
- Source/SDK/backend licensing choices and measured budgets remain explicit decisions. QuickJS is not locked in by this draft.
- No broad prefactoring ticket is needed: the workspace has prototypes rather than a production architecture to restructure.
- A release-readiness ticket verifies an assembled candidate; it does not authorize external publication or absorb unbounded fixes.
- The testing approach in the spec remains a proposal pending feedback. This review concerns ticket size/dependencies and does not silently adopt an engine, license split or numeric budget.

## Proposed tickets

Numbers are in dependency order. Blockers list direct prerequisites only; inherited prerequisites are omitted. Parallel branches do not imply separate source forks or a global serial queue. Revision 2 renumbers unpublished drafts; the previous plan is [preserved](ticket-breakdown-revision-1.json).

### Runtime and first native interaction

1. **[Validate a maintainable WASI3 runtime candidate](ticket-drafts/01-validate-a-maintainable-wasi3-runtime-candidate.md)**

   **Blocked by:** None.

   **Delivers:** A reproducible WASI3 runtime candidate for JS, TS and Rust, with portable build inputs and correct fresh-instance initialization, ready for early native validation on all three operating systems.

2. **[Open Pane's portable native core without extensions](ticket-drafts/02-open-pane-s-portable-native-core-without-extensions.md)**

   **Blocked by:** None.

   **Delivers:** A minimal GPUI CE shell with portable build/run entry points and OS-specific code behind explicit adapters, usable without a guest runtime.

3. **[Run a Rust command through the native UI](ticket-drafts/03-run-a-rust-command-through-the-native-ui.md)**

   **Blocked by:** 01, 02.

   **Delivers:** A Rust WASI3 fixture command displays a list in Pane, receives a real user action, and returns an updated view.

4. **[Run JS and TS commands through the same UI contract](ticket-drafts/04-run-js-and-ts-commands-through-the-same-ui-contract.md)**

   **Blocked by:** 03.

   **Delivers:** JS and TS fixture extensions drive the same native list/action interaction as Rust, using the candidate engine behind Pane's API.

### Early macOS/Linux contributors and shared developer workflow

5. **[Establish the macOS contributor baseline early](ticket-drafts/05-establish-the-macos-contributor-baseline-early.md)**

   **Blocked by:** 04.

   **Delivers:** A macOS contributor can build the core and JS/TS/Rust guest examples, run native checks and exercise real view/event round trips before shared SDK and package work expands.

6. **[Establish the Linux contributor baseline early](ticket-drafts/06-establish-the-linux-contributor-baseline-early.md)**

   **Blocked by:** 04.

   **Delivers:** A Linux contributor can build the core and JS/TS/Rust guest examples, run native checks and exercise real view/event round trips on an explicitly supported desktop baseline.

7. **[Keep a working contributor workflow on all three operating systems](ticket-drafts/07-keep-a-working-contributor-workflow-on-all-three-operating-systems.md)**

   **Blocked by:** 05, 06.

   **Delivers:** Contributors on Windows, macOS and Linux have a documented build/run/test workflow backed by native automated checks before the shared extension system grows.

### Local development, lifecycle and recovery

8. **[Install and run local extension packages](ticket-drafts/08-install-and-run-local-extension-packages.md)**

   **Blocked by:** 07.

   **Delivers:** A user selects a supported local package in Pane, sees its identity and compatibility, installs it and invokes its command.

9. **[Disable extensions and preserve settings across restart](ticket-drafts/09-disable-extensions-and-preserve-settings-across-restart.md)**

   **Blocked by:** 08.

   **Delivers:** A user disables a local extension in the UI, restarts Pane, and can re-enable it with its saved settings intact.

10. **[Replace a running extension through manual reload](ticket-drafts/10-replace-a-running-extension-through-manual-reload.md)**

   **Blocked by:** 09.

   **Delivers:** Manual reload replaces one extension while Pane and an unrelated extension stay open, preserving saved data and reporting failed replacement startup.

11. **[Build and reload on save for all launch languages](ticket-drafts/11-build-and-reload-on-save-for-all-launch-languages.md)**

   **Blocked by:** 10.

   **Delivers:** Saving a JS, TS or Rust development extension rebuilds and reloads that extension, with build diagnostics shown in Pane.

12. **[Cancel in-flight work during reload and disable](ticket-drafts/12-cancel-in-flight-work-during-reload-and-disable.md)**

   **Blocked by:** 10.

   **Delivers:** Reloading or disabling an extension with pending async work stops managed resources and prevents old replies from changing the current UI.

13. **[Invoke and clean up a prebuilt native helper](ticket-drafts/13-invoke-and-clean-up-a-prebuilt-native-helper.md)**

   **Blocked by:** 12.

   **Delivers:** A component command invokes an OS-matched prebuilt helper, shows its result, and stops the managed process on cancel, disable or reload.

14. **[Pause an identified broken extension through the UI](ticket-drafts/14-pause-an-identified-broken-extension-through-the-ui.md)**

   **Blocked by:** 12.

   **Delivers:** Pane skips an identified failing extension, shows a toast and persistent status, and allows Retry without requiring a CLI.

15. **[Recover when the shared runtime crashes or hangs](ticket-drafts/15-recover-when-the-shared-runtime-crashes-or-hangs.md)**

   **Blocked by:** 13, 14.

   **Delivers:** The native core stays recoverable after the runtime stops responding, with honest runtime-level diagnostics and no blind replay of user actions.

16. **[Explain platform-limited extensions and actions](ticket-drafts/16-explain-platform-limited-extensions-and-actions.md)**

   **Blocked by:** 08.

   **Delivers:** Pane displays supported-OS information and explains unavailable actions while leaving supported actions usable.

### Native views, composition and root search

17. **[Submit a native form from JS/TS and Rust](ticket-drafts/17-submit-a-native-form-from-js-ts-and-rust.md)**

   **Blocked by:** 07.

   **Delivers:** A native extension form accepts input, validates it, and returns a visible guest result through the same contract in JS/TS and Rust.

18. **[Drive a custom interactive view from an extension](ticket-drafts/18-drive-a-custom-interactive-view-from-an-extension.md)**

   **Blocked by:** 17.

   **Delivers:** A JS/TS and Rust extension each drive one custom interactive visualization or control with real input and guest-updated rendering.

19. **[Call explicit operations across extension languages](ticket-drafts/19-call-explicit-operations-across-extension-languages.md)**

   **Blocked by:** 09.

   **Delivers:** An installed JS/TS command calls a Rust operation and a Rust command calls JS/TS, displaying structured results and meaningful target errors.

20. **[Discover and invoke commands through root search](ticket-drafts/20-discover-and-invoke-commands-through-root-search.md)**

   **Blocked by:** 09.

   **Delivers:** Root search finds installed command metadata and activates only the selected enabled extension.

### Default features and data

21. **[Launch Windows applications from a default extension](ticket-drafts/21-launch-windows-applications-from-a-default-extension.md)**

   **Blocked by:** 16, 20.

   **Delivers:** Pane finds installed Windows applications and launches a selected result through an independently disableable default extension.

22. **[Show calculator answers as an extension feature](ticket-drafts/22-show-calculator-answers-as-an-extension-feature.md)**

   **Blocked by:** 20.

   **Delivers:** Typing a supported expression into root search returns a calculator result from a disableable default extension.

23. **[Create and invoke persistent quicklinks](ticket-drafts/23-create-and-invoke-persistent-quicklinks.md)**

   **Blocked by:** 17, 20.

   **Delivers:** A user creates a quicklink in a native form, finds it in root search and invokes it after restarting Pane.

24. **[Search and open files through a default extension](ticket-drafts/24-search-and-open-files-through-a-default-extension.md)**

   **Blocked by:** 12, 16, 20.

   **Delivers:** Pane searches a documented local file scope and opens a selected Windows result, with cancellation as the query changes.

25. **[Search inside an online command with quick access](ticket-drafts/25-search-inside-an-online-command-with-quick-access.md)**

   **Blocked by:** 12, 20.

   **Delivers:** A user reaches an online-search command through an alias, hotkey or fallback, and receives cancellable results inside that command.

26. **[Capture clipboard history only when enabled](ticket-drafts/26-capture-clipboard-history-only-when-enabled.md)**

   **Blocked by:** 09, 16.

   **Delivers:** A Windows clipboard extension starts off, captures supported clipboard content only when enabled, and supports visible pause/disable controls.

27. **[Expire and delete clipboard history predictably](ticket-drafts/27-expire-and-delete-clipboard-history-predictably.md)**

   **Blocked by:** 26.

   **Delivers:** Clipboard history has configurable finite retention and deletion controls whose effects remain correct while disabled and after downtime.

28. **[Manage cache, credentials and uninstall data in the UI](ticket-drafts/28-manage-cache-credentials-and-uninstall-data-in-the-ui.md)**

   **Blocked by:** 09.

   **Delivers:** Users can clear an extension's cache, uninstall it with a durable-data choice, and later remove retained data without its code installed.

### Dependencies, distribution and background work

29. **[Install required extension dependencies](ticket-drafts/29-install-required-extension-dependencies.md)**

   **Blocked by:** 19.

   **Delivers:** Installing a local fixture extension shows and installs its compatible missing required dependencies while preserving optional, disabled and pinned choices.

30. **[Disable or remove required dependents together](ticket-drafts/30-disable-or-remove-required-dependents-together.md)**

   **Blocked by:** 28, 29.

   **Delivers:** Disabling or uninstalling a dependency shows affected required dependents and applies the confirmed whole-set action or cancels cleanly.

31. **[Install npm-distributed component packages](ticket-drafts/31-install-npm-distributed-component-packages.md)**

   **Blocked by:** 29.

   **Delivers:** A user installs a supported npm package through Pane without manually installing npm or Node, then runs its component command.

32. **[Install Git-distributed component packages](ticket-drafts/32-install-git-distributed-component-packages.md)**

   **Blocked by:** 29.

   **Delivers:** A user installs a supported Git-sourced package or explicit revision through Pane without manual Git/compiler setup.

33. **[Run scheduled and continuing background work](ticket-drafts/33-run-scheduled-and-continuing-background-work.md)**

   **Blocked by:** 14.

   **Delivers:** Enabled fixture extensions can schedule work or run an explicit background service, while unused installations remain inactive and disable stops managed activity.

34. **[Update eligible extensions without interrupting commands](ticket-drafts/34-update-eligible-extensions-without-interrupting-commands.md)**

   **Blocked by:** 31, 32, 33.

   **Delivers:** Compatible published extensions update automatically under user controls, respecting pins/local copies and staging replacement until active work permits it.

### Measurements, licensing, setup and author experience

35. **[Measure Windows resource usage across real lifecycle flows](ticket-drafts/35-measure-windows-resource-usage-across-real-lifecycle-flows.md)**

   **Blocked by:** 15, 20, 33.

   **Delivers:** A repeatable Windows measurement report shows actual process-tree costs for idle Pane, inactive/active extensions, startup and lifecycle churn.

36. **[Resolve first-party licensing and distribution notices](ticket-drafts/36-resolve-first-party-licensing-and-distribution-notices.md)**

   **Blocked by:** 13.

   **Delivers:** A concrete first-party application/SDK/component license split and dependency-notice plan fit Pane's provisional Zed-style direction.

37. **[Install Pane and acquire a default feature on Windows](ticket-drafts/37-install-pane-and-acquire-a-default-feature-on-windows.md)**

   **Blocked by:** 13, 22.

   **Delivers:** A clean Windows machine installs a small Pane bootstrap and automatically acquires the runtime and a runnable calculator/default-feature package.

38. **[Offer Pane updates for user-initiated installation](ticket-drafts/38-offer-pane-updates-for-user-initiated-installation.md)**

   **Blocked by:** 37.

   **Delivers:** Pane announces an available application update and installs it only after the user chooses, with a clear outcome if acquisition or replacement fails.

39. **[Validate author onboarding and distributable examples](ticket-drafts/39-validate-author-onboarding-and-distributable-examples.md)**

   **Blocked by:** 11, 13, 18, 31, 32, 33.

   **Delivers:** A contributor on Windows, macOS or Linux can independently build Pane and JS/TS/Rust examples, run checks, use hot reload and prepare supported packages from a fresh checkout.

### Platform integrations and installers

40. **[Use application, file and quicklink actions on macOS](ticket-drafts/40-use-application-file-and-quicklink-actions-on-macos.md)**

   **Blocked by:** 21, 23, 24.

   **Delivers:** The existing default app/file/quicklink workflows use real macOS discovery and open actions with accurate availability reporting.

41. **[Use clipboard controls and quick access on macOS](ticket-drafts/41-use-clipboard-controls-and-quick-access-on-macos.md)**

   **Blocked by:** 25, 27.

   **Delivers:** The existing opt-in clipboard and command quick-access flows work on macOS where available and explain unavailable OS integrations.

42. **[Use application, file and quicklink actions on Linux](ticket-drafts/42-use-application-file-and-quicklink-actions-on-linux.md)**

   **Blocked by:** 21, 23, 24.

   **Delivers:** The existing default app/file/quicklink workflows use Linux desktop discovery and open actions on the recorded support matrix.

43. **[Use clipboard controls and quick access on Linux](ticket-drafts/43-use-clipboard-controls-and-quick-access-on-linux.md)**

   **Blocked by:** 25, 27.

   **Delivers:** Clipboard and shortcut integration works on the explicitly supported Linux desktop combinations, with honest explanations where OS restrictions prevent it.

44. **[Install and update Pane on a clean macOS machine](ticket-drafts/44-install-and-update-pane-on-a-clean-macos-machine.md)**

   **Blocked by:** 38, 40, 41.

   **Delivers:** A macOS package acquires runtime/default artifacts without developer tools and offers app updates under the user-initiated policy.

45. **[Install and update Pane on a clean Linux machine](ticket-drafts/45-install-and-update-pane-on-a-clean-linux-machine.md)**

   **Blocked by:** 38, 42, 43.

   **Delivers:** A package for the selected Linux baseline acquires runtime/default artifacts without developer tools and offers user-controlled app updates.

### Independent preview-readiness checks

46. **[Verify the complete Windows preview](ticket-drafts/46-verify-the-complete-windows-preview.md)**

   **Blocked by:** 21, 23, 24, 25, 27, 30, 34, 35, 36, 38, 39.

   **Delivers:** A Windows preview candidate has a complete acceptance record, assembled default features and explicit known limits, ready for a separate release decision.

47. **[Verify the complete macOS preview](ticket-drafts/47-verify-the-complete-macos-preview.md)**

   **Blocked by:** 30, 34, 35, 36, 39, 44.

   **Delivers:** A macOS preview candidate has its own complete native acceptance/resource record and stated support limits, independently of Windows release timing.

48. **[Verify the complete Linux preview](ticket-drafts/48-verify-the-complete-linux-preview.md)**

   **Blocked by:** 30, 34, 35, 36, 39, 45.

   **Delivers:** A Linux preview candidate has native acceptance/resource evidence for its named desktop/package combinations, independently of other platform releases.

## Coverage and dependency checks

[Machine-readable checks](ticket-breakdown-checks.json) verify all 82 stories, 25 scenarios and eight gates have owners, that the graph is acyclic, and that early cross-platform contributor support gates shared feature development. These are planning checks, not evidence of implemented support.

| Gate | Ticket owners |
| --- | --- |
| G1 | 01, 03, 04, 05, 06, 07, 11, 39, 46, 47, 48 |
| G2 | 02, 03, 04, 05, 06, 07, 12, 13, 17, 18, 19, 20, 21, 25, 39, 46, 47, 48 |
| G3 | 09, 10, 11, 12, 13, 14, 15, 24, 33, 34, 46, 47, 48 |
| G4 | 08, 16, 19, 20, 29, 30, 31, 32, 34, 39, 46, 47, 48 |
| G5 | 09, 10, 23, 26, 27, 28, 30, 46, 47, 48 |
| G6 | 31, 32, 37, 38, 44, 45, 46, 47, 48 |
| G7 | 05, 06, 07, 16, 21, 24, 26, 35, 40, 41, 42, 43, 44, 45, 46, 47, 48 |
| G8 | 36, 46, 47, 48 |

## Approval requested

Does the granularity feel right, are the blockers genuine prerequisites, and should any slices be merged or split? The user-invoked [to-tickets skill](C:/Users/ADMIN/.agents/skills/to-tickets/SKILL.md) explicitly says: "Iterate until the user approves the breakdown." Only approved drafts will be published as ready-for-agent issues.

## Comments

2026-09-28: Revised against the existing spec and the user's cross-platform contributor clarification. No parent-issue/spec changes, runtime tests or implementation were performed.
