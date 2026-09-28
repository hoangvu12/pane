# Platform availability

Added for [#19](https://github.com/hoangvu12/pane/issues/19) (US03, US18,
US43, US44, US81, T22, G4, G7; contributions, not whole-gate claims). It
implements Q34 as accepted: [simple supported-OS metadata and per-action
availability explanations, preserving functioning actions, with no general
compatibility-rule language](extension-policy-proposal.md#platform-specific-extensions-and-actions).

There are two declarations, both a plain list of operating systems
(`windows`, `macos`, `linux`), and nothing else: no versions, architectures,
desktops, ranges or conditions.

## A package's supported systems

`pane.json` may list the systems the package supports:

```json
{ "manifestVersion": 1, "title": "Hello", "apiVersion": "0.1",
  "platforms": ["windows", "linux"], "commands": [ ... ] }
```

- Omitted: every system Pane runs on. Present: at least one known name; an
  empty list or an unknown name is an invalid manifest.
- A package that does not list this system has no compatible artifact here.
  It is explained before anything is installed or run ("Not available on
  Linux: this package supports only Windows and macOS"), with no Install
  row; installing it anyway (`--install`, [`Launcher::install_package`]) and
  updating to it are refused with the same reason. This check comes before
  the source-only check, so a Windows-only source package is explained as
  Windows-only on Linux.
- A package for this system shows "Supported systems: Windows and Linux (this
  system)" on its package screen.
- An installed package whose managed copy does not list this system (for
  example a data folder copied from another machine) stays listed in root
  search with the reason, like any installed copy that cannot load.

The components themselves are the same WASI 0.3 components on every system;
the declaration is the author's statement of where the package works, and it
is not evidence that it does ([record limits](current-decisions.md#record-limits)).

## An action's supported systems

An item of a command's list view may list the systems its action (or form)
works on, in [`wit/extension.wit`](../wit/extension.wit):
`platforms: option<list<platform>>`; in Rust
`platforms: Some(vec![Platform::Windows])`, in JavaScript/TypeScript
`platforms: ["windows"]`. None (omitted) means every system.

On a system the item does not list, the launcher computes the item's
availability result, [`Row::unavailable`], a user-facing reason: "Not
available on Linux: this action supports only Windows" ("... supports no
operating system" for an empty list). The item:

- stays listed and selectable, so the rest of the command keeps working and
  the reason can be read;
- shows the reason as a third line of the row, in amber, with a dimmed title;
- when activated (Enter or a click), shows the reason as the status error and
  never calls `run-action` or opens the item's form, so nothing looks
  successful while doing nothing.

The command decides nothing about platforms itself: it cannot see which system
it runs on (its WASI context exposes none), so the launcher applies the
declaration. A guest that is called for an item anyway (for example through
the runtime directly) is not stopped by Pane.

### Accessibility

The row is a `ListBoxOption` named by its title whose description is its
subtitle followed by the reason, and after activation the `Status` node
carries the reason; both are checked through GPUI's accessibility tree in the
window tests. The row is also marked disabled (`aria_disabled`), but GPUI
CE's debug accessibility tree does not report that state, so it is not
checked, and no screen reader has been run.

## Samples

The Rust, JavaScript and TypeScript samples each add two items after the
form: "Windows-only action" (`windows`) and "macOS and Linux action"
(`macos`, `linux`). On every system one runs and the other is explained. The
package with no compatible artifact is built by the tests and the GUI smokes
from the Rust sample with the two other systems in `platforms`.

## Checks

- Public host interface: `crates/pane-core/tests/samples.rs` (each sample, on
  the system the test runs on), `tests/launcher.rs` (an unavailable form does
  not open), `tests/packages.rs` (a package only for the other systems, a
  package for this one, an installed copy for the other systems, invalid
  lists). Expected results come from the test binary's own target OS, never
  from a simulated one.
- Window: `crates/pane/tests/window.rs`, each sample at Pane's window size:
  the unavailable row is scrolled into view, rendered with its reason,
  described to assistive technology and explained on Enter; the action for
  this system and the others still run.
- Native GUI smokes, identical steps on all three systems (screenshots 13 to
  15): the sixth and seventh items of the Rust command after a restart, then
  `--install` of a package listing the other two systems. Expected colors
  differ by system: Windows expects the Windows-only action to answer and the
  other to be explained; macOS and Linux the reverse. Run locally on Linux
  X11 ([evidence](platforms/linux.md#platform-availability-19)); the macOS
  and Windows steps have not run yet, so availability on those systems is
  unverified natively.

## Limits

- No architecture declaration: components are architecture-independent WASI.
  Native helper artifacts matching OS/architecture belong to #15.
- A root-search command cannot declare platforms in `pane.json`; only whole
  packages and list items can. Aliases, fallbacks and other surfaces do not
  exist yet.
- Other runtime availability (a missing app or setting) is not modelled; a
  command reports that as an error from its action.
- Whether an unavailable item should be hidden instead is left open by Q34;
  Pane lists and explains it.
- Disabling an extension (#10) is a separate state and is not an
  availability result.

[`Launcher::install_package`]: ../crates/pane-core/src/launcher.rs
[`Row::unavailable`]: ../crates/pane-core/src/launcher.rs
