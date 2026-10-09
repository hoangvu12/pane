## Parent

https://github.com/hoangvu12/pane/issues/151

## What to build

The install preview tells the user what a new extension relies on and who provides it. Install order stops mattering, and the user can switch a required dependency off briefly without switching its dependents off one by one. It is size M.

**The install preview** plans `uses` with the dependency planner. Each line is data in the plan, and its wording lives only in the display layer:

- "Uses acme:translate@1: provided by DeepL Translate (installed)". An installed provider, even a disabled one, is used rather than the consumer's default being installed beside it.
- "Uses acme:translate@1: no installed extension provides it; Pane installs <default>, which <title> names". The default is planned, claimed, installed and rolled back as a missing required dependency is.
- "Uses acme:translate@1: no installed extension provides it; <title> waits until one does".
- "Provides acme:translate@1 (DeepL Translate provides it too; choose in Settings)".

**What stops an install.** A missing required capability without a default never stops an install: the package waits for a provider. A `default` that cannot be installed stops the install before anything changes, as an uninstallable required dependency does. Optional uses are listed, and their defaults are never installed.

**Updates** plan the new copy's `uses` the same way.

**Disable all and Uninstall all** keep their closure over required dependencies. Capabilities add no package to it, since another provider may serve. When the package being disabled or uninstalled is the last available provider of a capability that enabled packages require, the question gains a line: "Caller and Other will wait for acme:translate@1 until another extension provides it". Disabling or uninstalling one provider while another remains adds no line.

**Disable only (confirmed by the user).** The question shown when disabling a required dependency gains a row between Disable all and Cancel: "Disable only <title>; the extensions that require it wait for it". Choosing it disables that package alone, and its dependents wait (#152) and come back when it is enabled again. Uninstall keeps Uninstall all and Cancel.

**Docs:** dependencies (the plan's capability lines, defaults, the last-provider line, and the limit "no way to disable a required dependency while keeping its dependents" removed) and the install documentation.

## Acceptance criteria

- [ ] The plan for a consumer whose capability is provided by an installed package (enabled or disabled) installs nothing more and names the provider.
- [ ] The plan installs the consumer's default only when no provider is installed. The default is then the first provider, and is used.
- [ ] A consumer whose required capability has no provider and no default installs, and its plan says it waits.
- [ ] A default that cannot be installed stops the install with nothing changed. A failure after the default was installed rolls it back.
- [ ] Optional uses never install their default.
- [ ] An update's plan covers the new copy's `uses`.
- [ ] A package that provides an already provided capability shows the "choose in Settings" line.
- [ ] Disabling or uninstalling the last available provider adds the waiting line, naming the enabled consumers. With another provider left, there is no line, and Disable all's closure is unchanged.
- [ ] "Disable only" disables just the dependency. Its dependents wait, then come back when it is enabled.
- [ ] Core tests through `Launcher` cover each plan and confirmation as data. Prior art: the dependencies, disable dependents, uninstall dependents and update suites.
- [ ] Window tests with real key events: the install preview's capability lines, and choosing "Disable only" from the extension list. Prior art: the install window tests.
- [ ] The documents above are updated.
- [ ] CI: done when one ci-fast verify run is green on the ticket branch. The release matrix is neither dispatched nor waited for.

## Blocked by

- #153

