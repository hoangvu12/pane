## Parent

https://github.com/hoangvu12/pane/issues/151

## What to build

Manage extensions shows what is broken, why, and how to fix it, where today it cannot show a package whose required dependency is missing or disabled, or packages that require each other in a cycle. It is size S to M.

**The extension list.** A package's status line gains "Enabled · Waiting for <what>", or "Enabled · Some commands wait for <what>" when the requirement is narrowed to some commands. A user can find the broken extensions without opening each.

**A package's details** gain:

- **Unmet requirements.** Each required dependency and required capability that is unmet, with the chain down to what is actually missing ("Needs Notes Sync, which waits for acme:auth@1: no extension provides it").
- **A fix row beside each.** "Enable <title>", "Retry <title>", "Install <default> (named by <title>)", "Choose a provider in Settings", or "Install an extension that provides <capability>", which opens the install forms. A fix applies at once, and the requirement row disappears when the package comes back.
- **What it provides.** Each capability, marked "chosen" or "not chosen", with the consumers of each.
- **Cycles.** Each cycle the package is part of, found from the declarations (required dependencies and required capabilities): "Requires itself through B and C: they wait together if one cannot run, and Disable all or Uninstall all affects them together". Healthy cycles are shown too: they run, as ADR 0041 decides.

The rows read the waiting model (#152 and #156) and the provider choice (#154). They are data, and their wording lives only in the display layer. Dependency rows work from #152. Whichever of this ticket and #156 lands second wires the capability requirement rows and their fix rows, and whichever of this ticket and #154 lands second wires "chosen" and "Choose a provider in Settings".

**Docs:** dependencies (its limits lose "no Pane-side view of installed packages whose required dependency was disabled or removed") and the Manage extensions documentation.

## Acceptance criteria

- [ ] The status line shows "Waiting for" for a package waiting as a whole, and "Some commands wait for" for a narrowed requirement.
- [ ] A package's details list each unmet required dependency and capability, with the chain three deep naming the root cause.
- [ ] Each fix row does what it says: Enable and Retry bring the dependents back, Install <default> runs the install, Choose a provider opens Settings' Capabilities section, and Install an extension that provides opens the install forms.
- [ ] Provided capabilities show chosen or not chosen and their consumers.
- [ ] A cycle of healthy packages and a cycle with one member missing are each shown, on every member.
- [ ] Optional requirements never show as unmet.
- [ ] Core tests through `Launcher` cover the requirement, fix, provider and cycle data, with Rust fixtures for the cycles and the three-deep chain. Prior art: the dependencies and disable dependents suites.
- [ ] Window tests with real key events: Manage extensions' waiting status, requirement, provider and cycle rows, and each fix action. Prior art: the window and install window tests.
- [ ] The documents above are updated.
- [ ] CI: done when one ci-fast verify run is green on the ticket branch. The release matrix is neither dispatched nor waited for.

## Blocked by

- #153

