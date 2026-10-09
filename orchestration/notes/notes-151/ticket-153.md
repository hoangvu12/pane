## Parent

https://github.com/hoangvu12/pane/issues/151

## What to build

A consumer can call "translate" without naming the package that translates. A **capability** is a named, versioned set of operations, written `<namespace>:<name>@<major>` (such as `acme:translate@1`). Any installed package may provide it, and other packages use it by its name (ADR 0041, amending ADR 0011 and ADR 0012). A capability is a second way to address operations: the call, its JSON input and result, its error kinds, the chain rules and generation ownership are those of operations. Calls by package identity and dependencies stay. It is size L.

**Names.** The namespace and the name use lowercase letters, digits and `-`, and the major is a positive integer. The namespace is the author's (by convention their npm scope or domain), and Pane keeps no registry of namespaces. The `pane` namespace is reserved for Pane's own default extensions. There are no ranges and no negotiation. A package may provide several majors of one capability by declaring each.

**Providing.** `pane.json` gains `provides`: each entry names the capability, the component serving it and its operations. The component serves them through the export that serves published operations, and Pane passes the operation qualified by its capability (`acme:translate@1/translate`), so one component can tell a capability call from a call by identity to an operation of the same name. Installing checks the export without running guest code. A capability's operations are reached only through the capability. `platforms` works as on operations: elsewhere, the package does not provide that capability.

**Using.** `pane.json` gains `uses`: each entry names the capability and the operations it calls, and may say `optional`, `use` (`one`, the default, or `all`), `default` (a source, written as a dependency's source is) and `commands` (the command ids that need it). This ticket reads and validates all of them. Installing a default, fan-out and waiting come in #155 and #156. Installing never stops because no provider is installed.

**Validation at install.** The package is refused, with the reason, for a malformed name, an operation listed twice, a missing component or one without the export, `commands` naming an unknown command, a `default` the package's own origin cannot name (the `local:` rule of dependencies), or the same capability provided twice at one major.

**Calling.** The SDKs gain a call to a capability by name and a way to ask for its available providers (identity and title), with an `available(capability)` helper for optional uses.

- A call is resolved when it is made, against the packages as they are then. Until the provider-choice ticket (3) lands, it goes to the first provider, in install order, that can serve. Then it behaves as a call by identity: the same JSON limits, a provider already in the chain refused, at most 8 deep, the same generation ownership, and lazy start of the target.
- A package never serves its own use. It gets another provider.
- A call to a capability or operation the caller's manifest does not declare is `refused`, and the message says to declare it.
- Errors reuse the existing kinds, and each message names the capability: `not-found` when no installed package provides it, `disabled` when every provider is disabled, and `unavailable` when every provider is paused or waiting or none supports this system. `failed`, `crashed` and `refused` are as for any call.

**Samples:** a provider of `pane-samples:greet@1` in Rust, JavaScript and TypeScript, each answering its own language's name, and a consumer in each language with a required use of it and an optional use of a capability nobody provides, so every pairing of languages is exercised.

**Docs:** operations gains capabilities, and the manifest reference gains `provides` and `uses`. The authoring check and schema are #128's.

## Acceptance criteria

- [ ] `provides` and `uses` are read from `pane.json`, and each validation failure above refuses the install with its reason.
- [ ] A consumer in each language calls the provider in each language by capability name, and the provider sees the qualified operation.
- [ ] A capability's operations are not reachable by identity unless also published in `operations`.
- [ ] A call goes to the first provider in install order that can serve. A later install does not change it.
- [ ] Undeclared capabilities and operations are refused with the message to declare them.
- [ ] `not-found`, `disabled` and `unavailable` (paused, waiting, other system) each name the capability.
- [ ] Chain rules hold across capability calls: a provider already in the chain is refused, and the depth limit applies.
- [ ] A package that provides and uses one capability is never routed to itself (Rust fixture).
- [ ] The providers query and `available` answer the available providers. The optional use of a capability nobody provides answers `not-found` and gates nothing.
- [ ] Core tests through `Launcher` with the samples in all three languages. Prior art: the operations and dependencies suites.
- [ ] Prebuilt sample artifacts are rebuilt.
- [ ] CI: done when one ci-fast verify run is green on the ticket branch. The release matrix is neither dispatched nor waited for.

## Blocked by

- #152

