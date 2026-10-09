## Parent

https://github.com/hoangvu12/pane/issues/151

## What to build

An extension can add root search rows, set timers, watch folders, or provide a capability once the user has signed in, and none of it outlives the extension. Each is an **owned registration**, a WIT resource the guest owns (ADR 0041). Declarations in `pane.json` are unchanged. It is size L.

**The contract.** A registration lasts until the first of: the guest dropping the handle, the instance holding it going away (crash, stopped call, unresponsive stop, search cancellation), or the generation ending. Each is tagged with its package, instance and generation and records its undo in the generation's undo list from #136. Dropping it earlier removes that entry. Using a handle of an ended generation is refused ("this code of the extension was replaced…").

**The four kinds.**

- **Dynamic root item.** A root search row under one of the package's commands, in #120's item shape (id, title, subtitle, icon, accessories, actions), updatable through its handle, ranked like an indexed result. Its actions call the command's event entry point. An item that declares a mode is a **dynamic command**: invoking it launches its command with a launch record naming the item's id. Quick slots, aliases and hotkeys hold it by command and item id, and while it is not registered they say so.
- **Timer.** `after` or `every`, from 1 second (confirmed by the user) to 30 days. Each firing is a call into the component's event export with the timer's tag, under the generation's crash, pause and unresponsive rules. Firings due while one is pending are coalesced.
- **Folder watcher.** A path, recursive or not, through the native watcher development mode uses. Changes are coalesced for half a second and delivered to the event export with the changed paths. An overflow is one "rescan" event.
- **Run-time provision.** A `provides` entry marked `atRunTime` is provided only while the package holds a provision for it, served by the instance holding it. A provision the manifest does not declare and mark is refused. Dropping it, or its instance going, withdraws the provider at once, and consumers fall back or wait.

**While waiting.** Timers and watchers of a waiting command's package run none of its code. Their firings are coalesced into one, delivered when it comes back.

**Limits.** For each package: 1,000 dynamic root items, 64 timers, 16 watchers and 16 provisions. One beyond a limit is refused with the limit named. A dynamic item's size is bounded as an indexed result's is.

**Activation entry point (confirmed: an opt-in exception to lazy activation, amending ADR 0005).** `pane.json` may declare `activate`, naming a component that exports it. Pane calls it when the package may run and is not waiting: at install, enable, start, reload, update, Retry and on coming back from waiting, and again when the instance that ran it is dropped while the generation continues unpaused. A trap is a crash, and during a reload's start a startup failure. Without `activate`, registrations live as long as the instance that made them.

**SDKs.** A Rust handle undoes itself on `Drop`. A JavaScript or TypeScript handle has `dispose()` and `Symbol.dispose`, so `using` works; an unreachable one is undone when its instance goes. The event export is a callback table the SDK keeps, as for actions.

**Samples:** a registrations sample in Rust, JavaScript and TypeScript with a dynamic root item updated by a timer, a folder watcher on a fixture folder, a run-time provision made after a "sign in" action, and `activate`.

**Docs:** generations, operations, schedules (guest timers, amending ADR 0024) and the manifest reference. The glossary gains dynamic root item, dynamic command, run-time provision and activation entry point through /domain-modeling.

## Acceptance criteria

- [ ] Each kind is undone on drop, on instance loss, and on each generation end (disable, pause, reload, update, uninstall), and the undo list is empty afterwards through #136's test hook.
- [ ] A handle used after a reload is refused with the message (Rust fixture).
- [ ] A dynamic root item appears, updates and disappears in root search. Its actions and its dynamic command run, and a quick slot, alias and hotkey of it say why while it is not registered.
- [ ] Timers below 1 second or above 30 days are refused. Firings are coalesced while one is pending and while the package waits.
- [ ] A watcher delivers coalesced changes and an overflow as "rescan".
- [ ] A run-time provision makes the package a provider only while held. Consumers fall back or wait when it is dropped. An undeclared or unmarked provision is refused.
- [ ] Registrations beyond each limit are refused, naming the limit (Rust fixtures).
- [ ] `activate` is called at each moment above and again after its instance is dropped. A trapping `activate` counts towards pausing (Rust fixture).
- [ ] Core tests through `Launcher` with the sample in all three languages. Prior art: the stopping, pausing, reload and develop suites.
- [ ] Window tests with real key events: a dynamic root item's row and its action.
- [ ] Prebuilt sample artifacts are rebuilt.
- [ ] The documents and glossary entries above are updated.
- [ ] CI: done when one ci-fast verify run is green on the ticket branch. The release matrix is neither dispatched nor waited for.

## Blocked by

- #136
- #153 (added: run-time provisions are capability provides)
- #156
- #137
- #139
- #138

