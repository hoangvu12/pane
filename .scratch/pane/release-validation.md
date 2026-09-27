# Pane release validation

This is an assembled-candidate checklist, not an oversized implementation ticket. Each feature slice must already demonstrate its own complete behavior. A failed release check produces a narrowly scoped corrective ticket; it does not turn this checklist into an open-ended development task. Nothing here authorizes publishing a release.

## Inputs and platform independence

All candidates require the early Windows/macOS/Linux contributor baseline and the shared feature slices. Each platform additionally requires its own application discovery, hotkeys, clipboard adapter, installer and app updater. Native evidence must match the exact claimed OS/architecture/desktop combination.

| Candidate | Platform feature inputs | Additional decision/evidence inputs |
| --- | --- | --- |
| Windows | [20](issues/20-launch-a-windows-application-from-root-search.md), [28](issues/28-open-an-extension-command-with-a-global-hotkey-on-windows.md), [31](issues/31-capture-opt-in-clipboard-history-on-windows.md), [47](issues/47-install-pane-and-acquire-its-calculator-on-windows.md), [50](issues/50-install-a-pane-application-update-by-user-choice-on-windows.md) | P2 licensing; P3 measurements/targets for Windows; shared feature evidence |
| macOS | [21](issues/21-launch-a-macos-application-from-root-search.md), [29](issues/29-open-an-extension-command-with-a-global-hotkey-on-macos.md), [33](issues/33-capture-opt-in-clipboard-history-on-macos.md), [48](issues/48-install-pane-and-acquire-its-calculator-on-macos.md), [51](issues/51-install-a-pane-application-update-by-user-choice-on-macos.md) | P2 licensing; P3 measurements/targets for macOS; shared feature evidence |
| Linux | [22](issues/22-launch-a-linux-application-from-root-search.md), [30](issues/30-open-an-extension-command-with-a-global-hotkey-on-linux.md), [34](issues/34-capture-opt-in-clipboard-history-on-linux.md), [49](issues/49-install-pane-and-acquire-its-calculator-on-linux.md), [52](issues/52-install-a-pane-application-update-by-user-choice-on-linux.md) | P2 licensing; P3 measurements/targets for Linux; shared feature evidence |

Shared contracts can originate in a complete native slice: for example, the first clipboard history/storage implementation is exercised on Windows, and the later capture adapters reuse its tested retention behavior. This is an explicit technical prerequisite. There is no dependency on another platform's installer, app updater, completed benchmark or release approval.

## Candidate checks

- [ ] Start from a clean supported environment; install the assembled default-feature artifact set with clipboard history initially off. The early calculator-only installer smoke is not sufficient for release readiness.
- [ ] Collect current evidence for the applicable T01-T25 scenarios in the [spec](spec.md#observable-acceptance-scenarios), using real representative JS/TS/Rust guests. Reuse feature results when they apply to the exact candidate; rerun where integration or artifact changes invalidate them.
- [ ] Verify default features are individually disableable; check root search, native app/file/URL actions, online command routing and shortcut availability.
- [ ] Check UI input, focus/IME/accessibility and the standard/custom author examples on the native target. Report actual limitations instead of claiming parity from compilation.
- [ ] Check reload, cancellation, helper cleanup, failed startup, crash/hang recovery and no replay of side effects; no stale-generation result changes the current view.
- [ ] Check source identity, required dependencies, enablement/pins, data/cache/uninstall ownership, clipboard expiry and update behavior, including failed acquisition/replacement.
- [ ] Verify author examples are reproducible from that OS using the documented build/test/development commands. Sample authoring and packaging documentation must already ship with their feature slices; this check must not create a second SDK.
- [ ] Run the documented P3 resource workload on this platform, measuring the full process tree and comparing with its recorded targets. A Windows result is not a proxy for macOS or Linux.
- [ ] Check P2 notices and distribution obligations against the actual dependency/artifact versions. Record outstanding signing or packaging credentials honestly.
- [ ] Record exact versions, native environments, results and known limits. Missing environments, unresolved consequential choices or failing requirements keep the corresponding readiness claim open.

## Scope and coverage

This checklist owns the assembled release aspect of T25, the sustained measurement aspect of T24, final G1-G8 evidence and the current-artifact recheck of US82. It supplements feature checks; it is not evidence that any scenario passed. No runtime tests or releases were performed during ticket revision.
