## Parent

https://github.com/hoangvu12/pane/issues/151

## What to build

A command that needs a capability nobody can serve says so and comes back by itself, as #152 made commands do for required dependencies. A consumer that declares `use: "all"` gets every provider's answer. It is size M.

**Waiting on capabilities.** The waiting computation of #152 gains capabilities. A command waits while a required capability of its package has no provider that is installed, enabled, not paused and not waiting.

- **Narrowing.** A use with `commands` gates only those commands. The package's other commands stay available, and its status says some commands wait.
- **Chains.** A capability a waiting package provides does not count as provided, so its own consumers wait in turn. The reason names what is actually missing ("Needs acme:translate@1: DeepL Translate is disabled", or "…: no extension provides it").
- **A provider that is also a consumer** of the same capability never serves itself. It waits until another provider can serve.
- **Optional uses never gate.** A call to one without a provider answers as #153's errors say, and `available` lets the command hide the feature.
- **Enter on a waiting command** shows the reason with fix rows as in #152, plus "Install <default> (named by <title>)" when the use names a default, or "Install an extension that provides <capability>", which opens the install forms.
- **Everything else is #152's.** Nothing of a waiting command runs, waiting never counts towards pausing, and the command comes back without any action when a provider can serve again.

**Fan-out.** A call to every provider answers a list with each provider's identity, title, and result or error.

- Providers are called in turn, each as its own call in the chain: the chosen provider first, then the others in install order. Whichever of this ticket and the provider-choice ticket (3) lands second wires "chosen first".
- Providers that are disabled, paused, waiting or for another system are skipped. With none available, the answer is an empty list.
- Fanning out a `use: "one"` capability is refused. A plain call to a `use: "all"` capability reaches the chosen provider.
- A use with `use: "all"` never waits: it degrades to an empty list.

**Samples:** the `pane-samples:greet@1` consumers in Rust, JavaScript and TypeScript gain a fan-out item that lists each provider's answer labelled with its title.

**Native smoke.** The phase that installs a provider and a consumer in two languages (added by #154 or here, whichever lands first) disables the provider so the consumer waits, then enables it so it comes back.

**Docs:** operations (fan-out, waiting on capabilities) and dependencies.

## Acceptance criteria

- [ ] A consumer waits while its required capability has no provider, then while its only provider is disabled, paused or waiting. It comes back on install, enable or Retry with nothing done to the consumer.
- [ ] Disabling one provider while another can serve leaves consumers running.
- [ ] A `commands`-narrowed use gates only those commands.
- [ ] A chain through capabilities (A uses a capability only B provides, and B waits for C) names the root cause.
- [ ] A cycle through capabilities waits as a whole when one member cannot run, and runs when all are healthy.
- [ ] A package that provides and uses one capability waits until another provider can serve (Rust fixture).
- [ ] Optional uses never gate, and `available` reflects providers coming and going.
- [ ] Enter on a waiting command shows the install-default or install-a-provider row, and that row opens the install forms.
- [ ] Fan-out answers each available provider in order, skips disabled, paused, waiting and other-system providers, and answers an empty list when none is available.
- [ ] Fanning out a `use: "one"` capability is refused.
- [ ] Core tests through `Launcher` with the samples in all three languages, covering each transition through the row, the refused view, the schedule (manual clock), the service and root results. Prior art: the operations and #152's suites.
- [ ] Window tests with real key events: a row waiting on a capability, and its fix rows.
- [ ] Prebuilt sample artifacts are rebuilt.
- [ ] The native smoke phase is extended. Release-validation evidence, not a merge gate.
- [ ] The documents above are updated.
- [ ] CI: done when one ci-fast verify run is green on the ticket branch. The release matrix is neither dispatched nor waited for.

## Blocked by

- #153

