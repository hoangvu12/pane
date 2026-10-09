## Problem Statement

Pane's extensions can already call one another. A package publishes operations in its `pane.json`, and another calls them through Pane, either by the target's package identity or by the id of a dependency it declares. That works, but it ties every consumer to one exact package. Six problems follow from it, for users and for authors.

**A consumer must name its provider.** An author who wants "translate this", "find my password" or "send to my notes app" has to name the one package that does it. Nothing lets several packages offer the same thing so that the user can pick. A user who prefers another translation service, or who already has a notes extension installed, cannot plug it in. Two extensions that both translate cannot both be offered, and an author has to fork a consumer to change its vendor.

**Broken requirements show up only as errors.** A command that cannot work because what it needs is gone keeps failing instead of saying so. When a required dependency is disabled outside "Disable all", paused after crashing, or uninstalled outside "Uninstall all", its dependents stay listed as if nothing were wrong:

- their commands open and fail;
- their scheduled work runs and answers `disabled`;
- their continuing services cycle against a target that cannot answer;
- their root results ask a package that cannot serve.

The user meets errors they did not cause and nothing tells them "this needs X". When X comes back, nothing marks the dependents as working again either.

**Manage extensions cannot show what is broken.** It cannot show a package whose required dependency is missing or disabled ([dependencies](https://github.com/hoangvu12/pane/blob/main/docs/dependencies.md#limits) lists this as a limit). Nor does it show packages that require each other in a cycle.

**An extension cannot register anything at run time.** Every long-lived contribution must be declared in `pane.json`, and that is how Pane keeps cleanup complete today. So an extension cannot:

- add a root search row while a timer runs ("Focus: 12:30 left");
- add a command for each workspace the user has;
- set a timer of its own;
- watch a folder;
- start providing something only once the user has signed in.

As the extension API grows, each such API will need the same promise: that disabling, reloading or updating removes everything it registered.

**Each host subsystem undoes its own work in its own way.** Hotkeys, helpers, schedules, services, indexes and watchers each reconcile from state or watch the generation for themselves. A new subsystem can forget to.

**A replacement loses all live state.** A Reload, an Update or a development-mode reload drops everything the package kept in memory and closes its open screen. An author iterating in development mode navigates back to the same screen after every save. A timer the extension kept starts again from zero. A service's task forgets what it was watching.

## Solution

Pane takes Cordis's model of composition ([cordiverse/cordis](https://github.com/cordiverse/cordis); [arXiv 2608.25512](https://arxiv.org/abs/2608.25512)) and builds it natively in its Rust host for its WASI 0.3 components, as Mimir does. It takes none of Cordis's code (ADR 0041).

- **Capabilities, brokered by Pane.** A package declares in `pane.json` that it provides a named, versioned capability, such as `acme:translate@1`, made of operations. A consumer declares the capabilities it uses, required or optional, and calls one by its name. Pane routes each call to the provider the user picked in Settings. Until the user picks, it uses the first one installed, or the default the consumer named, which Pane installs when no provider is installed. A consumer that declares that it uses every provider gets each provider's answer.
- **Commands that wait for what they need.** A command whose required capability or required dependency is missing, disabled, paused or waiting itself stays listed and says what it needs ("Needs DeepL Translate, which is disabled"). Its view, schedule, service and root results do not run. It comes back by itself when what it needs returns. Optional capabilities degrade gracefully.
- **Owned registrations.** An extension can register things at run time, each held as a WIT resource it owns:
  - dynamic root items and commands;
  - timers;
  - folder watchers;
  - providers registered at run time.

  Dropping the resource undoes the registration, and so does the generation ending. A package can opt in to an activation entry point so that its registrations are made without waiting for the user.
- **State kept across a replacement.** On Reload, Update and development-mode reload, a package can opt in to hand a small snapshot of its state to its new code. The screen that was open opens again. Neither happens after a crash or a pause.
- **What is broken, shown where it can be fixed.** For each package, Manage extensions shows:
  - its missing or broken required capabilities and dependencies, with why and how to fix each (enable, retry, install a provider);
  - the capabilities it provides;
  - the cycles it is part of.

The host's single undo list per generation, which makes cleanup complete for every subsystem, is built with ticket #136's runtime work.

## User Stories

### Choosing who provides a capability

1. As a launcher user, I want two extensions that do the same job (two translators, two notes apps) to be installable side by side, so that I am not forced to choose at install time.
2. As a launcher user, I want to pick which installed extension provides a capability in Settings, so that every extension using it uses the one I prefer.
3. As a launcher user, I want a capability with one provider to need no choice from me, so that the common case has no setup.
4. As a launcher user, I want the first provider I installed to be used until I choose another, so that installing a second one changes nothing behind my back.
5. As a launcher user, I want my choice of provider to be kept across restarts, updates and reloads, so that I make it once.
6. As a launcher user, I want changing the provider to take effect on the next call, without reloading or restarting the extensions that use it, so that switching is instant.
7. As a launcher user, I want Settings to tell me when the provider I chose cannot serve right now and which one is used instead, so that I understand which service answered.
8. As a launcher user, I want uninstalling the provider I chose to forget that choice, so that a stale choice does not linger.
9. As a launcher user, I want to see which extensions use a capability and which provide it, so that I understand what depends on what.
10. As a launcher user, I want an extension that searches across every provider (for example "search all my notes apps") to show results from each, labelled with the provider, so that I can use them together.
11. As a launcher user, I want a provider that is disabled, paused or waiting to be skipped when a search asks every provider, so that one broken provider does not break the search.

### Installing, disabling and uninstalling

12. As a launcher user, I want the install preview to list the capabilities an extension uses and who provides each, so that I know what it relies on before installing.
13. As a launcher user, I want an extension that needs a capability nobody provides to be installable anyway, and to wait for a provider, so that the order in which I install extensions does not matter.
14. As a launcher user, I want Pane to install the default provider an extension names when no installed extension provides that capability, so that a new extension works at once.
15. As a launcher user, I want an already installed provider to be used rather than the extension's named default being installed beside it, so that I do not collect duplicates.
16. As a launcher user, I want the install preview to tell me when an extension provides a capability that another installed extension already provides, so that I know a choice exists.
17. As a launcher user, I want disabling or uninstalling the last provider of a capability to tell me which extensions will wait for it, so that I am not surprised afterwards.
18. As a launcher user, I want disabling or uninstalling one provider while another remains to change nothing for the extensions that use it, so that I can try providers freely.
19. As a launcher user, I want to disable a required dependency alone and let the extensions that need it wait, as well as disabling them all, so that I can switch something off briefly without switching its dependents off one by one.

### Waiting commands

20. As a launcher user, I want a command whose required capability or dependency is missing to stay listed and say what it needs, so that I know why it cannot run.
21. As a launcher user, I want the reason to name the provider or dependency and its state (not installed, disabled, paused, waiting for something else), so that I know what to fix.
22. As a launcher user, I want pressing Enter on a waiting command to show the reason with an action to fix it (Enable, Retry, Install a provider, Open Manage extensions), so that I can fix it from where I am.
23. As a launcher user, I want a waiting command to come back by itself as soon as what it needs returns, so that I do not have to enable or reload anything else.
24. As a launcher user, I want a waiting command's scheduled work not to run while it waits, so that it does not produce errors I did not cause.
25. As a launcher user, I want a waiting command's continuing service to pause while it waits and start again at once when it can, so that it neither fails nor stays off.
26. As a launcher user, I want a waiting command's root results and indexed results not to be asked for while it waits, so that root search stays clean and fast.
27. As a launcher user, I want a quick slot, alias, global hotkey or fallback of a waiting command to say why it cannot run, so that every way in explains itself.
28. As a launcher user, I want a command that uses a capability only optionally to keep working without it, so that an optional extra never blocks the main feature.
29. As a launcher user, I want an extension whose required dependency is paused to wait rather than fail, and to come back when I retry the dependency, so that one crash does not leave errors everywhere.
30. As a launcher user, I want a chain of waits (A needs B, which needs C, which is disabled) to name what is actually missing, so that I fix the root cause.
31. As a launcher user, I want an extension that requires a capability only for some of its commands to keep its other commands available, so that one missing piece does not hide everything.
32. As a launcher user, I want waiting never to count as a crash or pause anything, so that a missing provider does not mark an extension as broken.
33. As a launcher user, I want a screen I have open to stay open when what it needs goes away, with its calls explaining why they fail, so that I do not lose what I was doing.

### Manage extensions

34. As a launcher user, I want each extension's details in Manage extensions to list its required capabilities and dependencies that are missing or broken, with the reason, so that I can see at a glance what is wrong.
35. As a launcher user, I want a fix row beside each broken requirement (Enable <title>, Retry <title>, Install <default provider>, Choose a provider), so that fixing it is one step.
36. As a launcher user, I want the extension list to mark a waiting extension ("Enabled · Waiting for acme:translate@1"), so that I can find the broken ones without opening each.
37. As a launcher user, I want Manage extensions to show when extensions require each other in a cycle, and what that means (they wait, are disabled and uninstalled together), so that cycles are not invisible.
38. As a launcher user, I want an extension's details to list the capabilities it provides and whether it is the chosen provider, so that I know what it does for others.

### Owned registrations

39. As a launcher user, I want an extension to add rows to root search while something is going on (a running timer, an active download), so that I can see and act on it from root search.
40. As a launcher user, I want dynamic rows to disappear as soon as the extension removes them, or when I disable, reload, update or uninstall it, so that nothing stale stays behind.
41. As a launcher user, I want to pin, alias or give a hotkey to a dynamic command, and have it say why it cannot run while it is not registered, so that dynamic commands work like declared ones.
42. As a launcher user, I want an extension's timers and folder watchers to stop the moment the extension is disabled, paused, reloaded, updated or uninstalled, so that nothing keeps running behind my back.
43. As a launcher user, I want a timer or watcher of a waiting extension not to run its code until the extension can run again, so that waiting really means nothing runs.
44. As a launcher user, I want an extension that registers too much (thousands of rows or timers) to be refused politely, so that one extension cannot slow Pane down.
45. As a launcher user, I want an extension to start providing a capability only once it can (for example after I sign in), so that a provider that is not ready is not chosen.

### State across a reload or update

46. As a launcher user, I want an extension updated or reloaded while I use it to keep what it had in memory when it supports that (a running timer, a draft, a selection), so that an update does not reset my work.
47. As a launcher user, I want the screen I had open when an extension was reloaded to open again, so that I am back where I was.
48. As a launcher user, I want state never to be carried over after a crash or a pause, so that the state that broke an extension does not break it again.
49. As a launcher user, I want an extension that cannot restore its state to start fresh rather than fail, so that a handoff problem never stops it working.

### Authors

50. As an extension author, I want to declare the capabilities my package provides in `pane.json`, with their operations, so that other packages can use them without knowing my package.
51. As an extension author, I want to declare the capabilities my package uses, each required or optional, with the operations I call, so that Pane can check, install, wait and explain for me.
52. As an extension author, I want to call a capability by its name from Rust, JavaScript and TypeScript with the same meaning, so that my language does not limit me.
53. As an extension author, I want a capability's operations served by the same entry point as my published operations, told which capability they were called through, so that one component can serve both.
54. As an extension author, I want to name a default provider source that Pane installs when no provider is present, so that my extension works out of the box.
55. As an extension author, I want to call every provider of a capability and get each one's answer or error labelled with the provider, so that I can merge results.
56. As an extension author, I want to ask whether an optional capability has an available provider, and which, so that I can show or hide the feature that uses it.
57. As an extension author, I want to narrow a required capability to the commands that need it, so that my other commands stay available while it is missing.
58. As an extension author, I want calls to a capability I did not declare to be refused with a message telling me to declare it, so that Pane's view of my requirements stays complete.
59. As an extension author, I want the errors of a capability call to tell "no provider installed", "all providers disabled", "provider paused or waiting" and the operation's own failure apart, so that my command can explain each.
60. As an extension author, I want my namespace to keep my capability names apart from other authors', so that two authors' capabilities do not collide.
61. As an extension author, I want to publish a new major version of a capability beside the old one, so that old consumers keep working while new ones move on.
62. As an extension author, I want to register a root item, a timer, a watcher or a run-time provider and get back a handle I own, so that dropping the handle undoes it.
63. As an extension author, I want my registrations undone for me when my package's generation ends, so that I never write cleanup code for disable, reload, update or uninstall.
64. As an extension author, I want using a handle from replaced code to fail clearly, so that stale code cannot act on new state.
65. As a JavaScript or TypeScript author, I want handles to have `dispose()` and work with `using`, so that I can release them deterministically rather than waiting for garbage collection.
66. As an extension author, I want an opt-in activation entry point that Pane calls when my package's code may run, and again if my instance was dropped, so that my registrations exist without waiting for the user.
67. As an extension author, I want timer and watcher events delivered to my component as calls that belong to my generation, so that the existing rules for crashes, pauses and generations apply.
68. As an extension author, I want to opt in to handing a snapshot of my state to my new code on reload and update, and to restore it before anything else runs, so that my users keep their place.
69. As an extension author, I want the snapshot to be opaque bytes I version myself, with SDK helpers that serialise a value, so that I control the format across my releases.
70. As an extension author in development mode, I want the open screen to reopen and my state to be restored after every successful build, so that iterating does not mean navigating back each time.
71. As an extension author, I want a snapshot that is too large, too slow or rejected by my new code to be reported in development mode's diagnostics, so that I can fix it.
72. As an extension author, I want `pane.json` mistakes in `provides` and `uses` refused at install with a clear message (a malformed capability name, an operation the component does not serve, a run-time provider for a capability not declared, a duplicate), so that I find them before users do.

## Implementation Decisions

### Decision provenance

- **Explicit user decisions (2026-10-06), recorded in ADR 0041** ("Extensions compose through capabilities that Pane brokers, taking Cordis's ideas but not its code"):
  - Pane does not adopt the Cordis library: there is no Node host and no Cordis inside guests. Pane builds Cordis's model natively, as Mimir does.
  - Capabilities by name, written `<namespace>:<name>@<major>`, with the host as broker. Consumers require a capability rather than a package. The user picks a provider in Settings, and the default is the first one installed or a default the consumer declares. Pane fans out where the use says so.
  - Waiting commands and services, schedules and root-results providers, with the reason ("needs X") shown. They come back by themselves, and optional capabilities degrade gracefully.
  - Owned registrations as WIT resources, undone when dropped or when the generation ends. Declarative contributions stay as they are.
  - One undo list per generation inside the host, folded into #136.
  - An opt-in, size-limited state handoff on Reload, Update and development-mode reload, with the open screen reopened. Never after a crash or a pause.
  - Manage extensions shows missing or broken required capabilities and dependencies, and cycles.
  - Not adopted: automatic re-application of dependents, hot reload through the JS module cache, and Proxy-based injection.
- **Decided in ADR 0041 under the user's decision.** A capability generalises how operations are addressed. It does not replace operations, and it reuses their call, JSON input and result, error kinds, chains and generation ownership. ADR 0041 amends ADR 0011, ADR 0012, ADR 0005, ADR 0004, ADR 0024 and ADR 0025, and leaves ADR 0002, 0010, 0013, 0036 and 0037 unchanged. A cycle of healthy packages does not wait, unlike Cordis's, because Pane starts nothing in dependency order.
- **Coordination with other specifications (not the user's).**
  - The undo list is #136's, and the owned-registration slice is blocked by #136.
  - Timer and watcher events are calls into the guest, served by the runtime #136 makes concurrent.
  - Dynamic root items use the item, action and icon shape of the "Extension commands like Raycast" specification (#120) and are drawn as its List items.
  - The reopened screen relies on host-owned state kept by key, from the "Extension UI you can design" specification (#121).
  - The manifest schema and the authoring check of the "Making extensions easier to build" specification (#128) gain `provides`, `uses` and `activate`.
- **Proposed defaults in this specification (not separately confirmed).** Each is marked where it appears:
  - the manifest field names;
  - the capability name grammar and the reserved `pane` namespace;
  - falling back to another provider while the chosen one cannot serve;
  - that installing never stops for a missing capability;
  - the "Disable only" row;
  - narrowing a use to commands;
  - the first registration kinds and their limits;
  - the activation entry point and when it is called again;
  - the timer bounds;
  - the snapshot limit, deadline and idle-only rule;
  - reopening the screen without opting in;
  - the wording of rows.

### Capabilities

- **Name.** `<namespace>:<name>@<major>`. The namespace and the name use lowercase letters, digits and `-`, and the major is a positive integer.
  - The namespace is the author's, conventionally their npm scope or domain. Pane keeps no registry of namespaces (ADR 0002 has no central store) (proposed).
  - The `pane` namespace is reserved for Pane's own default extensions (proposed).
  - The major version changes when a change breaks consumers (ADR 0010).
  - There are no ranges or negotiation, as with operations' versions. A package may provide several majors of one capability by declaring each.
- **Providing.** A package declares `provides` in `pane.json`. Each entry names the capability, the component serving it and its operations. A short shape that encodes the decision (proposed field names):

  ```json
  "provides": [
    { "capability": "acme:translate@1", "component": "translate.wasm",
      "operations": ["translate", "languages"] },
    { "capability": "acme:notes@1", "component": "notes.wasm",
      "operations": ["search", "create"], "atRunTime": true }
  ],
  "uses": [
    { "capability": "acme:translate@1", "operations": ["translate"],
      "default": "npm:@acme/deepl-translate" },
    { "capability": "acme:notes@1", "operations": ["search"], "use": "all",
      "optional": true },
    { "capability": "acme:speech@1", "operations": ["speak"],
      "commands": ["read-aloud"] }
  ]
  ```

- **Serving.** The component serves a capability's operations through the existing `run-operation` export. Pane passes the operation qualified by its capability (`acme:translate@1/translate`), so one component can tell a capability call from a call by identity to an operation of the same name.
  - Installing checks the export without running guest code, as it does for published operations.
  - A capability's operations are reached only through the capability. A package that also wants them callable by identity publishes them in `operations` too.
  - `platforms` works as on operations. Elsewhere the package is not a provider of that capability.
- **Providing at run time.** An entry marked `atRunTime` is provided only while the package holds a run-time provision for it, as an owned registration (below). An entry not so marked is provided whenever the package can run. Providing at run time a capability the manifest does not declare is refused. This keeps every possible provider known without running code, so install plans, cycles and Settings work from the manifests alone (proposed).
- **Using.** A package declares `uses`. Each entry names the capability, the operations it calls, and optionally:
  - `optional: true`, as for dependencies;
  - `use: "one"` (the default) or `"all"`;
  - `default`: a source, as a dependency's source is written, to install when no provider is installed;
  - `commands`: the command ids that need it. Without `commands`, a required use gates the whole package (proposed).

  A package never resolves its own provision for its own use. It gets another provider, or waits (proposed).
- **Validation at install.** The package is refused with the reason when:
  - a name is malformed;
  - an operation is listed twice;
  - a component is missing or lacks the export;
  - `commands` names an unknown command;
  - `default` is a source the package's own origin cannot name (the `local:` rule of dependencies);
  - the same capability is provided twice at one major.

### Calling a capability

The SDKs hide the WIT, which is typed beside today's `call`. A short shape that encodes the decisions (proposed names):

```wit
call-capability: async func(capability: string, operation: string, input: string)
  -> result<string, call-error>;
record provider-answer { provider: string, title: string, answer: result<string, call-error> }
call-every: async func(capability: string, operation: string, input: string)
  -> list<provider-answer>;
providers: func(capability: string) -> list<provider>;   // available providers, chosen first
```

- **Routing.** Every call is resolved when it is made, against the packages as they are at that moment. It then behaves as a call by identity: the same JSON limits, the same chain rules (a provider already in the chain is refused, at most 8 deep), the same generation ownership, and lazy start of the target.
- **Refused calls.** A call to a capability or operation the caller's manifest does not declare is `refused`, and the message says to declare it. A `use: "all"` capability called with `call-capability` reaches the chosen provider, and a `use: "one"` capability may not be fanned out (proposed).
- **Errors.** The existing kinds are reused, and each message names the capability:
  - `not-found`: no installed package provides it.
  - `disabled`: every provider is disabled.
  - `unavailable`: every provider is paused or waiting, or none supports this system. Calling a provider that is waiting returns `unavailable` too.
  - `failed`, `crashed` and `refused`: as for any call.
- **Fan-out.** `call-every` calls each available provider in turn, the chosen one first and then in install order, each as its own call in the chain. It answers each provider's identity, title and result or error. Providers that are disabled, paused, waiting or for other systems are skipped. With no available provider it answers an empty list.
- **Asking first.** `providers` answers the available providers with their identities and titles, the chosen one first. The SDKs use it to offer an `available(capability)` helper for optional uses.

### Choosing a provider

- **The choice.** The user's choice is Pane's own record: for each capability at its major, the chosen provider's identity. It follows the house record rules, as aliases and quick slots do. It is never extension data.
- **Settings.** The Settings window's Extensions page gains a Capabilities section (proposed placement). It lists each capability that has two or more installed providers, with a dropdown of providers, and the consumers that use it. Changing the dropdown applies at once, to the next call.
- **The default, until the user chooses.** It is the first provider installed, in the order of the installed record. A consumer's `default` is installed only when no provider is installed, so it becomes the first. A later install never changes which provider is used.
- **Falling back (proposed).** While the chosen provider is disabled, paused, missing or waiting, calls go to the next available provider in the default order. Settings says "<chosen> is disabled; using <other>". Calls return to the chosen one when it can serve again. Consumers wait only when no provider can serve. The alternative, waiting for the chosen provider, is listed under Further Notes.
- **Uninstalling the chosen provider** forgets the choice.
- **Fan-out uses** ignore the choice except for the order they call in.

### Installing, disabling and uninstalling

- **The install preview.** The preview plans `uses` with the dependency planner:
  - "Uses acme:translate@1: provided by DeepL Translate (installed)";
  - "…: no installed extension provides it; Pane installs <default>, which <title> names" (planned, claimed and installed as a missing required dependency is, and rolled back the same way);
  - "…: no installed extension provides it; <title> waits until one does";
  - "Provides acme:translate@1 (DeepL Translate provides it too; choose in Settings)".
- **What stops an install.** A missing required capability without a default never stops an install (proposed: the package waits, so install order does not matter). A `default` that cannot be installed stops the install, as an uninstallable required dependency does.
- **Updates.** An update plans the new copy's `uses` the same way.
- **Disable all and Uninstall all.** These keep their closure over required dependencies. Capabilities do not add packages to it, since another provider may serve. When the package being disabled or uninstalled is the last available provider of a capability that enabled packages require, the question gains a line: "Caller and Other will wait for acme:translate@1 until another extension provides it".
- **Disabling a required dependency alone (proposed).** The question for a required dependency gains a row between Disable all and Cancel: "Disable only <title>; the extensions that require it wait for it". This lifts the dependencies limit that offered no way to disable a dependency while keeping its dependents.

### Waiting

- **When a command waits.** A command of an enabled, unpaused package waits while any of these is not met:
  - each required dependency of its package (or narrowed to it) is installed, enabled, not paused and not waiting;
  - each required capability of its package (or narrowed to it) has at least one provider that is installed, enabled, not paused and not waiting. A run-time provider counts only while its provision is held.
- **Computing who waits.** Waiting is computed by the host from the manifests and states as a greatest fixed point. Every enabled, unpaused package starts as able to run, and any package with an unmet requirement is removed, repeating until nothing changes. So a cycle of healthy packages runs, and a cycle with one member missing waits as a whole. This is recomputed whenever a package is installed, uninstalled, enabled, disabled, paused, retried, reloaded or updated, and whenever a run-time provision is made or dropped.
- **What waits does not run.** A waiting command does not run:
  - its view, run entry point, actions, arguments or setup screen;
  - its schedule's ticks, which are skipped and not replayed;
  - its service's cycles;
  - its root or indexed results;
  - its timers and watchers (below).

  A capability its package provides does not count as provided, and published operations of a package waiting as a whole answer `unavailable` ("<title> is waiting for acme:translate@1").
- **Waiting is not ending.** Waiting ends no generation and stops no instance or call already running. A cycle or call in flight finishes. An open screen stays, and its calls answer as calls do.
- **Coming back.** When the requirement is met again, the command comes back without the user doing anything:
  - its row is ordinary again;
  - a schedule starts from a full interval;
  - a service's first cycle runs at once, with its task in the instance it still has;
  - root search asks for its results on the next query.
- **How a waiting command shows.** It is shown as a paused command's reason is shown, with a reason kind of its own beside paused and other-system:
  - its root search row stays with "Needs <what>, which is <state>";
  - Enter shows the reason and the fix rows;
  - its quick slots, aliases, global hotkeys and fallbacks say the same and run nothing.
- **Not a failure.** Waiting never counts towards pausing.
- **Optional uses never gate.** A call to an optional capability or dependency without a provider answers as above.
- **Behaviour that changes.**
  - Today, enabling a dependent whose required dependency is disabled is allowed, and its calls answer `disabled`. It is still allowed, but the dependent now waits.
  - Today, when Pane pauses a required dependency, its dependents stay enabled and their calls are refused. They now wait, and come back on Retry.
  - Disable all and Uninstall all are unchanged.

### Owned registrations

- **The contract.** A registration is a WIT resource the guest owns. Its lifetime is the shortest of three:
  - the guest dropping the handle;
  - the instance holding it going away, through a crash, a stopped call, an unresponsive stop or a search cancellation;
  - the generation ending.

  Every registration is tagged with its package, instance and generation. Using a handle whose generation ended is refused ("this code of the extension was replaced…"), as host imports already refuse stale code.
- **Undoing.** On the host, every registration records how it is undone in the generation's undo list (#136). When the generation ends, Pane runs the list newest first. Dropping a registration earlier removes its entry. Declarations in `pane.json` are untouched by this.
- **The first kinds.** This specification delivers four (proposed set). Subscriptions to other extensions' events, and later kinds, follow the same contract when their APIs come:
  - **Dynamic root item.** A root search row under one of the package's commands. It has an id, title, subtitle, icon, accessories and actions in #120's item shape, and can be updated through its handle. It is matched and ranked like an indexed result, and its actions call the command's event entry point. A dynamic item may instead declare a mode, which makes it a **dynamic command**. Invoking it launches its command with a launch record naming the item's id. Quick slots, aliases and hotkeys hold it by command and item id, and while it is not registered they say so, as a quick slot of a missing target does.
  - **Timer.** `after` or `every`, from 1 second to 30 days (proposed, the bounds of schedules and services; a view that needs to update faster uses #121's `refresh-after-ms`). Each firing is a call into the component's event export with the timer's tag. It belongs to the generation and follows the crash, pause and unresponsive rules. Firings that fall due while one is pending, or while the package waits, are coalesced into one and not replayed.
  - **Folder watcher.** A path, recursive or not, through the native watcher development mode uses. Changes are coalesced (proposed: half a second) and delivered as calls to the event export with the paths that changed. An overflow is delivered as one "rescan" event.
  - **Run-time provision.** Provides a capability its manifest marks `atRunTime`, served by the instance that holds it. Dropping it, or that instance going away, withdraws the provider at once, and consumers fall back or wait.
- **Limits (proposed).** For each package: 1,000 dynamic root items, 64 timers, 16 watchers and 16 provisions. A registration beyond a limit is refused with the limit named. The host also bounds a dynamic item's size as it bounds an indexed result's.
- **Activation (proposed).** A package may declare one activation entry point (`"activate": "<component>"`), which exports `activate`.
  - Pane calls it when the package's code may run and the package is not waiting: at install, enable, start, reload, update, Retry and on coming back from waiting.
  - Pane calls it again when the instance that ran it is dropped while the generation continues and the package is not paused.
  - A trap in it is a crash, counted towards pausing. During a reload's start, a trap is a startup failure.
  - This is an exception to lazy activation that the author opts into, as a continuing service is (ADR 0005, amended by ADR 0041).
  - Without `activate`, registrations are made from ordinary calls and live as long as the instance that made them.
- **In the SDKs.** In Rust, a handle is a value whose `Drop` undoes it. In JavaScript and TypeScript, a handle has `dispose()` and `Symbol.dispose`, so `using` works. A handle that is merely unreachable is not undone until its instance goes, since garbage collection is not deterministic. The event export is a callback table the SDK maintains, as for actions.

### The generation's undo list

Each generation gets one undo list inside the host. Every subsystem that acts for a generation registers its teardown there:

- hotkey registration;
- native helpers and system programs;
- scheduled and service runs;
- index entries;
- application and file watchers;
- clipboard capture;
- the owned registrations above.

Pane runs the list newest first when the generation ends, and a test hook checks that it is empty afterwards. It is built as part of ticket #136's runtime work, not by this specification's tickets. This specification's owned-registration slice registers its entries there and depends on it.

### State handoff and reopening the screen

- **Opting in.** A component opts in by exporting `snapshot` and `restore` in a lifecycle interface. Installing detects the export without running code. A JavaScript or TypeScript package sets a flag in its `package.json` so that the build links it, as for services. A short shape:

  ```wit
  interface lifecycle {
    activate: async func();
    snapshot: async func() -> option<list<u8>>;
    restore: async func(state: list<u8>) -> result<_, string>;
  }
  ```

- **When.** A handoff happens on Reload, on an Update (made by the user or automatic) and on a development-mode reload. It happens only once the replacement has passed its checks and is about to be installed: a replacement or build that fails changes nothing and needs no snapshot. It never happens after a crash, a pause, a failure to start, Retry, a disable followed by an enable, a runtime crash or hang, or a restart of Pane.
- **Taking the snapshot (proposed).**
  - Each running instance that exports `snapshot` and is idle is asked for one before the old generation ends. Idle means no call is pending in it. An instance busy with a call is stopped as today and gives none.
  - The deadline is 1 second. A snapshot not answered by then is dropped and the replacement goes ahead, so a handoff never delays a reload by more than that.
  - Each snapshot is limited to 1 MiB, the limit on operation JSON. A larger one is dropped.
  - A snapshot is kept only in memory, never written to disk.
- **Restoring.** The new code's instance of the same component file restores the snapshot on its first start, before any other call. A snapshot with no matching component is discarded.
  - An error from `restore` discards the state and is not a failure.
  - A trap in `restore` is a crash. During a reload's start, it is a startup failure, so the package is paused and the state is lost.
  - Development mode's diagnostics report a dropped, oversized, late or rejected snapshot. Elsewhere it is silent.
- **Format.** The snapshot is opaque bytes, which the author versions. The SDKs offer helpers that serialise a value (serde in Rust, JSON in JavaScript and TypeScript).
- **A continuing service's task** hands over the same way, so the new instance's first cycle finds the restored state.
- **Reopening the screen (proposed: for every view command, whether or not the package opts in).**
  - When one of the package's command screens was on display at the replacement, Pane opens that command again on the new code with its original launch record. The setup gate applies if the new code adds required preferences.
  - Only the command's root view is reopened. Views it had pushed are not, unless the author restores them from the snapshot.
  - Host-owned state is kept where the new tree has the same keys (#121): the command search text, the selection, scroll and inputs.
  - An automatic update never applies while a screen is on display, so this concerns Reload, development-mode reload and an update the user made.

### Manage extensions and Settings

- **Status.** A package's status line in the extension list gains "Enabled · Waiting for <what>", or "Enabled · Some commands wait for <what>" when the requirement is narrowed to some commands.
- **A package's details.** They gain rows for:
  - each required capability and dependency that is unmet, with the chain down to what is actually missing ("Needs Notes Sync, which waits for acme:auth@1: no extension provides it");
  - the fix row for it: "Enable <title>", "Retry <title>", "Install <default> (named by <title>)", "Choose a provider in Settings", or "Install an extension that provides acme:translate@1", which opens the install forms;
  - the capabilities it provides, with "chosen" or "not chosen", and the consumers of each;
  - each cycle it is part of: "Requires itself through B and C: they wait together if one cannot run, and Disable all or Uninstall all affects them together".
- **The Capabilities section in Settings** is described under Choosing a provider.
- **Wording.** Every row's wording is proposed. The wording lives only in the display layer: the plan and the waiting state are data, as the dependency plan is.

### Modules touched

- **The package manifest reader and validation:** `provides`, `uses` and `activate`.
- **The dependency planner and the dependents traversal:** capabilities in previews, defaults, confirmation lines and "Disable only".
- **A new waiting model in the core.** It computes the fixed point and answers each command's reason. The registry, root search, the scheduler, the services runner, the updater's boundary and Manage extensions consult it, and a change hook tells them when it changes.
- **The operation router:** capability resolution, fan-out, the provider choice record and fallback.
- **The extension runtime:** the registration resources and their event export, `activate`, `snapshot` and `restore`, and stale-handle refusal. It relies on #136's undo list and concurrency.
- **The launcher:** reopening the screen after a replacement, and the waiting reason in rows, quick slots, aliases and hotkeys.
- **The window:** waiting rows, Manage extensions rows, and the Settings Capabilities section.
- **The SDKs:** Rust `pane-guest` and the JS/TS declarations and runtime, covering capability calls, `available`, handles, the event table, the lifecycle exports and the snapshot helpers.
- **The documents** that change when the slices land: operations, dependencies, generations, services, schedules, development mode and pausing.

## Testing Decisions

- **What a good test does.** It drives Pane the way users and authors meet it: through `pane_core::Launcher` with real sample extensions, or through real key events in a real window. It asserts what is listed, run, shown, kept and refused. It does not assert internal structures or record formats beyond the documented records.
- **The primary seam is `pane_core::Launcher` with real samples in Rust, JavaScript and TypeScript**, as the operations samples are tested in all three languages today.
  - **The capability samples.** A provider of `pane-samples:greet@1` exists in each language, each answering with its own language's name, and a consumer exists in each language. It uses the capability with a required use, a fan-out item, and an optional use of a capability nobody provides. So every pairing of languages is exercised.
  - **The registrations sample**, in three languages. It has a dynamic root item updated by a timer, a folder watcher on a fixture folder, a run-time provision made after a "sign in" action, and `activate`.
  - **The handoff sample**, in three languages. It keeps a counter and a draft in memory, and opens a screen to reopen.
  - **Rust fixtures** cover the edges:
    - a cycle of healthy packages and a cycle with one member missing;
    - waits three deep;
    - a stale handle used after a reload;
    - registrations beyond each limit;
    - an `activate` that traps;
    - a snapshot over 1 MiB, one that misses its deadline, and a `restore` that errs or traps;
    - a provider that is also its own consumer.
- **Cases the core tests cover.**
  - **Routing:** to the only provider; to the user's choice, kept across a restart of the launcher on the same data folder; to the first installed until a choice is made; falling back while the chosen one is disabled or paused, and returning to it; forgetting the choice on uninstall.
  - **Fan-out:** order and skipped providers.
  - **Refused calls:** an undeclared capability or operation is refused.
  - **Error kinds:** not installed, all disabled, all paused or waiting, other system.
  - **Install plans:** a default installed only when no provider is installed, waiting when there is none, a default that cannot be installed stopping the install, and rollback.
  - **Confirmations:** the last-provider line, and "Disable only".
  - **Waiting:** the transitions for disable, enable, pause, Retry, uninstall, install, reload, update and run-time provision changes, each through a command's row, its view refused with the reason, its schedule (with the manual clock and `wait_for_schedules`), its service (with `wait_for_services`) and its root results not asked for, and all coming back. Commands narrowed to a requirement, and optional uses that never gate.
  - **Changed behaviour:** a paused dependency's dependents now wait, and the existing disable and uninstall dependents suites are adjusted to it.
  - **Registrations:** undone on drop, on instance loss, on each kind of generation end, and on waiting (timers and watchers held, then coalesced). Stale handles refused. Limits. `activate` called and called again. The undo list empty after every generation end, through #136's test hook.
  - **Handoff:** kept across Reload, Update and development-mode reload, never across a crash, pause, Retry, enable or restart; the deadline and the size limit; `restore` errors and traps; the screen reopened with its launch record and host-owned state.
  - **Prior art:** the operations, dependencies, disable dependents, uninstall dependents, stopping, schedules, services, pausing, reload, develop and update suites, and the quick slots and aliases suites for targets that say why they cannot run.
- **Window tests in the Pane crate's tests, with real key events.** They cover:
  - a waiting row's text and Enter showing the reason and fix rows;
  - Manage extensions' waiting status, requirement, provider and cycle rows, and their fix actions;
  - the Settings Capabilities dropdown changing the provider;
  - the screen reopening after a development-mode reload.

  Prior art: the window, settings, develop and install window tests.
- **Native smokes.** One phase installs a provider and a consumer in two languages, disables the provider so the consumer waits, enables it again so it comes back, and switches providers in Settings. Prior art: the dependencies and disable-dependents smoke phases.
- **One seam per feature:** core behaviour in the core tests, drawing in the window tests. No platform adapter is needed: watchers reuse development mode's watcher, already tested.

## Out of Scope

- **Cordis itself**, a Node host, or any JavaScript framework inside guests. Also not taken: automatic re-application of dependents when a provider is replaced, hot reload through module-cache eviction, and Proxy-based injection (the user's decision).
- **Per-consumer interception and realms** (Cordis's `intercept` and `isolate`). For example, a provider choice for each consumer, or read-only access for one consumer. The choice is per capability.
- **Events published between extensions**, and subscriptions to them. Their API comes later and follows the owned-registration contract.
- **Schemas Pane validates for a capability's input and result** beyond JSON. Version ranges and negotiation. A central registry or catalog of capabilities or namespaces.
- **State handoff after a crash, a pause or across restarts.** Migrating saved data between versions. Undoing external effects.
- **The undo list itself and the concurrent runtime**, which are #136's. Real push re-rendering, which is the "Extension UI you can design" specification's (#121).
- **The authoring check, schema and templates** for the new fields, which are the "Making extensions easier to build" specification's (#128). This specification only defines the fields.
- **Pane's own default extensions** publishing capabilities in the `pane` namespace. The namespace is reserved, and what to publish is decided later.

## Further Notes

- **Sources.** The research read Cordis at `cordiverse/cordis` main (4.0.0 release candidate, with its loader, include and HMR packages), the paper *A Programming Paradigm for Spatiotemporal Composability* (Shi, Zhang and Cui, arXiv 2608.25512, 92 pp.) and Mimir 0.2.9 as installed. Mimir's source is private, so its owned-resource registrations and the contributions it checks by owner and generation are inferred from its installed artifacts.
- **Ideas taken from the paper:**
  - revertible effects owned by one scope (§5): owned registrations and the undo list;
  - reactive coeffects: waiting;
  - the service broker (§6.2): capabilities;
  - static cycle detection (§6.5): the cycle rows;
  - namespacing against key collisions (§6.6): capability names;
  - Wasm imports as the language-independent form (§6.4, §6.7).

  Pane takes the ideas, not Cordis's code (ADR 0041 gives the reasons: the cost of a Node host, Rust and JS guests in separate Wasm sandboxes, and teardown by dropping the store, which is already stronger).
- **Confirmed by the user (2026-10-06), after this specification was first published:**
  - calls fall back to the next provider while the chosen one cannot serve (rather than waiting for it);
  - the open screen reopens after a reload for every package; only the in-memory state handoff is opt-in;
  - the activation entry point is an opt-in exception to lazy activation;
  - the "Disable only" row and the 1-second timer minimum, as proposed.
- **Glossary.** ADR 0041 adds the glossary entries Capability, Provider, Waiting command and Owned registration, and revises Operation and Reload. Further terms go through `/domain-modeling` with the tickets: dynamic root item, dynamic command, run-time provision, activation entry point and state handoff.
- **Sizes and order (proposed):**
  1. Waiting on required dependencies, with no new manifest fields (M). It changes the behaviour for paused and disabled dependencies first.
  2. Capabilities: manifest, routing, errors and samples (L).
  3. Provider choice in Settings, with the default and fallback (M).
  4. Install plans with defaults, confirmation lines and "Disable only" (M).
  5. Waiting on capabilities, and fan-out (M).
  6. Manage extensions rows and cycles (S to M).
  7. Owned registrations, the four kinds and `activate`, blocked by #136 (L).
  8. State handoff and reopening the screen (M).

  Slices 1 to 6 do not need #136.


---

The decision is recorded in [ADR 0041](https://github.com/hoangvu12/pane/blob/main/docs/adr/0041-extensions-compose-through-capabilities-that-pane-brokers.md) (added by [#119](https://github.com/hoangvu12/pane/pull/119)). Sources of the ideas: [Cordis](https://github.com/cordiverse/cordis) and its paper ([arXiv 2608.25512](https://arxiv.org/abs/2608.25512)).

