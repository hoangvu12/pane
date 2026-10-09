Implements spec #151, "Capabilities between extensions: providers Pane brokers, commands that wait for what they need, owned registrations and state kept across a reload" (ADR 0041), as eight squashed ticket commits.

What it does:

- **Waiting commands** (#152): a command whose required dependency is missing, disabled, paused or waiting itself stays listed, says what it needs, runs none of its work, and comes back by itself when the dependency returns. Waiting is a reason kind of its own beside paused; pausing a dependency now makes its dependents wait instead of refusing their calls.
- **Capabilities** (#153): `provides` and `uses` in `pane.json` name `<namespace>:<name>@<major>` capabilities. Calls by capability name are routed through Pane, behave as calls by identity, and answer `not-found`, `disabled` and `unavailable` naming the capability. Samples in Rust, JavaScript and TypeScript exercise every pairing.
- **Provider choice** (#154): the user picks the provider of a capability in Settings; the first installed is the default; calls fall back while the chosen provider cannot serve; the choice is kept across restarts and forgotten on uninstall.
- **Install plans** (#155): the preview plans `uses`, installs a consumer's default provider when none is installed, warns when disabling or uninstalling the last provider, and offers "Disable only" for a required dependency whose dependents wait.
- **Waiting on capabilities and fan-out** (#156): commands wait while no provider can serve a required capability (narrowed by `commands`, optional uses never gate), and `call-every` fans a call out to every available provider.
- **Manage extensions** (#157): status lines and detail rows for unmet requirements with fix actions, provided capabilities with chosen or not chosen, and cycles from the declarations.
- **Owned registrations** (#158): dynamic root items, timers, folder watchers and run-time provisions as owned WIT resources, undone on drop, instance loss or generation end; an opt-in `activate` entry point; limits per package.
- **State handoff** (#159): an opt-in `snapshot`/`restore` across Reload, Update and development-mode reload, and the open command screen reopened for every package.

Closes #151
Closes #152
Closes #153
Closes #154
Closes #155
Closes #156
Closes #157
Closes #158
Closes #159

Signed-off-by: hoangvu12 <hggaming91@gmail.com>
