# 49 - Install Pane and acquire its calculator on Linux

**What to build:** A clean Linux machine installs Pane, acquires compatible runtime/default artifacts, and runs a calculator command without developer tools.

**Blocked by:** [11 - Run and stop a packaged native helper](11-run-and-stop-a-packaged-native-helper.md); [23 - Show a calculator result in root search](23-show-a-calculator-result-in-root-search.md).

**Status:** ready-for-agent

**Parent:** [Pane specification](../spec.md).

**Spec links:** US15, US16, US18, US42; scenarios T23, T22; gates G6, G7. These are contributions to the referenced requirements, not claims that the whole gate or scenario passes here.

## Context

Pane is a small GPUI CE launcher with trusted JS/TS and Rust extensions using pure WASI 0.3 and component-native async. Default features are disableable extensions; AI is optional. Use the same author-facing behavior across Windows, macOS and Linux, with explicit native availability differences. QuickJS remains provisional.

Use the [specification](../spec.md), [current decisions](../../../docs/current-decisions.md) and [contributor requirement](../contributor-platform-requirement.md) for the shared contract. A source link is supporting context, not a prescribed implementation file layout.

## Acceptance criteria

- [ ] Build one documented Linux package/bootstrap for the declared baseline, with the minimum platform installation and acquisition UI; complete the real calculator command.
- [ ] Acquire runtime/default-feature payloads over the network with progress, retry and compatible cache reuse. Interrupt the initial download and recover while core setup/management remains available.
- [ ] Run the supported prebuilt helper fixture without requiring Node, Rust, npm, Git or compilers on the end-user machine. Record artifact ownership and compatibility checks.
- [ ] Include controlled artifact sources and a clean-machine native smoke. Record signing/package prerequisites honestly; unprovided credentials or test machines remain execution prerequisites.

## Scope

One platform installer and one representative default feature. Other default features already have their own slices; assembled release validation is separate. Source adapters may share code without depending on another OS installer.

Implement the UI, guest/host behavior, persistence or OS integration, author example and observable checks needed for this outcome within this slice. Do not leave a layer for a later ticket to make the stated outcome work. For shared changes, retain the early native contributor checks on all three OSes; report actual native evidence and remaining limits.

## Why these blockers

- 11 supplies real managed helper artifacts and cleanup behavior used by this flow.
- 23 supplies the actual default calculator feature used to prove installation.

## Comments

2026-09-28: Revision 3 narrows this draft to one observable outcome. Runtime decision work and release-wide checks are tracked separately. No implementation or native validation has been performed for this ticket.

2026-09-28: Published to the local issues tracker at the user's request to fix the tracker structure. Existing blockers and acceptance criteria still apply; implementation has not started.
