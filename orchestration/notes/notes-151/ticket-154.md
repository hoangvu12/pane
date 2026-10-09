## Parent

https://github.com/hoangvu12/pane/issues/151

## What to build

Two extensions that do the same job can be installed side by side, and the user picks which one serves every consumer of the capability. It is size M.

**The choice.** For each capability at its major, Pane records the chosen provider's identity. It is Pane's own record and follows the house record rules, as aliases and quick slots do. It is never extension data. It is kept across restarts, updates and reloads, and forgotten when the chosen provider is uninstalled.

**The default, until the user chooses.** The first provider installed, in the order of the installed record. A later install never changes which provider is used, so installing a second provider changes nothing behind the user's back. A capability with one provider needs no choice.

**Falling back (confirmed by the user).** While the chosen provider is disabled, paused, missing or waiting, calls go to the next available provider in the default order. They return to the chosen one when it can serve again. Consumers wait only when no provider can serve, as the capability-waiting ticket (5) delivers.

**Settings.** The Settings window's Extensions page gains a Capabilities section. It lists each capability that has two or more installed providers, with:

- a dropdown of its providers;
- the consumers that use it;
- "<chosen> is disabled; using <other>" (or paused, waiting) while the choice falls back.

Changing the dropdown applies to the next call, without reloading or restarting any consumer.

**The providers query** from #153 answers the chosen (or default) provider first. Fan-out (#156) calls the chosen provider first, then the others in install order. Whichever of this ticket and #156 lands second wires that order.

**Native smoke.** A phase installs a provider and a consumer in two languages and switches providers in Settings. Whichever of this ticket and #156 lands first adds the phase, and the second extends it (#156 adds disabling the provider so the consumer waits, then enabling it so it comes back). Prior art: the dependencies and disable-dependents smoke phases.

**Docs:** operations (capabilities: the choice, default and fallback) and settings.

## Acceptance criteria

- [ ] With two providers and no choice, calls go to the first installed. Installing a third changes nothing.
- [ ] Choosing another provider routes the next call to it, with no reload or restart of the consumer.
- [ ] The choice is kept across a restart of the launcher on the same data folder, and across a reload and an update of the chosen provider.
- [ ] While the chosen provider is disabled, then paused, calls fall back to the next available provider in install order, and return to the chosen one when it is enabled or retried.
- [ ] Uninstalling the chosen provider forgets the choice, and calls go to the default order.
- [ ] The providers query answers the chosen provider first.
- [ ] Settings lists only capabilities with two or more installed providers, shows their consumers, and shows the fallback note while the chosen provider cannot serve.
- [ ] Core tests through `Launcher` with the `pane-samples:greet@1` providers in all three languages. Prior art: the operations, aliases and quick slots suites for records.
- [ ] Window tests with real key events: the Settings Capabilities dropdown changes the provider, and the next call answers from the new one. Prior art: the settings window tests.
- [ ] The native smoke phase above is added or extended. Release-validation evidence, not a merge gate.
- [ ] The documents above are updated.
- [ ] CI: done when one ci-fast verify run is green on the ticket branch. The release matrix is neither dispatched nor waited for.

## Blocked by

- #153

