# Pausing a broken extension

Added for [#16](https://github.com/hoangvu12/pane/issues/16) (US77, US78,
US79, US80, T17, G3; contributions, not a claim that the whole scenario or
gate passes). When one installed extension keeps failing, Pane pauses it,
says so, and offers Retry, while the rest of Pane keeps running. No command
line is needed. The model builds on [generations](generations.md): pausing
ends the package's generation.

## What pauses a package

Only a failure Pane can attribute to one package: its code, in the
package's current generation, failed on its own.

| Failure | Paused | Why |
| --- | --- | --- |
| A component could not be loaded or instantiated, or a reload's new code trapped as it started (a [startup failure](../CONTEXT.md)) | At once | Starting it again would fail the same way. |
| A guest call trapped (a crash): opening a command, an action, a form, a custom view's event or drawing, root results, indexed results, or an operation another package called | On the **3rd crash within 5 minutes** | A broken command stops failing soon; one bad input does not stop an extension that otherwise works. |

These are explicit choices, not measurements (`CRASHES_BEFORE_PAUSE` and
`CRASH_WINDOW` in
[`launcher/pausing.rs`](../crates/pane-core/src/launcher/pausing.rs)):

- Calls that answer between crashes **do not** start the count again: a
  user opens the command (which answers) before each crashing action, so a
  reset on any answer would never pause it. Crashes more than 5 minutes
  apart are not counted together, so rare crashes of a long-running Pane
  never add up to a pause.
- The count is kept in memory for the package's current generation: a
  restart, disable, reload, update or Retry starts it afresh. Only the pause
  itself is recorded.

What is **not** a failure of the package:

- An **error the extension answers with** ("sign in first", a refused form,
  a refused view): an expected outcome of code that runs. It never pauses
  it, however often it happens.
- A **call stopped** because its generation, or that of a caller in its
  chain, ended (disable, reload, update, uninstall). A package serving an
  operation for a stopped caller loses its instance and restarts afresh on
  its next call, as after a crash ([generations](generations.md#what-stopping-costs));
  that is not counted.
- A **crash of an operation's target** counts for the target only; its
  caller answered.
- A failure of the **runtime itself**, with no attributable package, pauses
  nothing; recovering from it is [#17](https://github.com/hoangvu12/pane/issues/17),
  and a guest that stops responding is
  [#18](https://github.com/hoangvu12/pane/issues/18).

## What a paused package does

- Its generation ends (`End::Paused`): its pending calls stop, its
  instances and views are dropped, and its code can no longer read or save
  data or call operations. A command of it that is open closes.
- Its commands stay in root search, listed and selectable, saying "<title>
  is paused after an error; retry it in Manage extensions"; activating one
  shows that and runs nothing. It computes no root results and supplies no
  indexed ones. Another package calling its operations is answered
  `unavailable`: "<title> is paused after an error; retry it in Manage
  extensions".
- The status line (the launcher's toast) says "<title> crashed 3 times
  within 5 minutes and is paused" (or "could not start and is paused"),
  that its saved data is kept, and where Retry and the details are.
- **Manage extensions…** lists it as "Enabled · Paused after crashing" (or
  "Enabled · Failed to start"), with a **Retry starting <title>** row whose
  subtitle holds the details (the last crash's message and backtrace). The
  diagnostics of a reload that fails to start also go to standard error.
- Its settings, content, cache and credentials are kept. Clearing its cache,
  uninstalling it and the rest of Manage extensions work, since none of them
  runs it.
- Other packages keep running.

Paused is **not** disabled: disabled is the user's choice and adds nothing
to root search; paused is Pane's, and says why.

## Persistence and how a pause ends

The pause is recorded in `installed.json` beside the package's record, with
the cause, the details, the package's version and its managed copy
(`"paused": { "after": "crashes", "why": …, "version": …, "code": "3" }`).
After a restart the package is still paused, for that code: its commands are
explained and nothing of it runs, so a known broken package is not started
again blindly.

| Action | Effect |
| --- | --- |
| **Retry** | Starts the same code in a new generation, asking each available command for its view (as a reload does). If that fails to start, it is paused again. The crash count starts afresh. |
| Reload or update | New code, which has not failed: the pause and its record go. |
| Disable | Ends the pause too (as a disable ended a startup failure before): enabled again, the package starts afresh. |
| Uninstall | The record goes with the package. |

A reload whose new code fails to start ([#11](https://github.com/hoangvu12/pane/issues/11))
is now one such pause: the Retry and diagnostics it offered are these, and
it holds across a restart. The earlier code is still not restored.

## Author example and tests

- The settings samples' **Crash** item
  ([Rust](../guests/sample-settings/src/lib.rs),
  [JavaScript](../guests/sample-settings-js/src/index.js),
  [TypeScript](../guests/sample-settings-ts/src/index.ts)) crashes on
  purpose: a Rust panic traps; in JavaScript and TypeScript, resolving an
  action with something other than a string does (throwing is an error the
  extension answers with).
- [`crates/pane-core/tests/pausing.rs`](../crates/pane-core/tests/pausing.rs):
  three crashes pause each language's sample, with its data kept, the pause
  held after a restart and Retry starting it; errors it answers with never
  pause it; the crash count starts afresh after a restart; another package
  keeps running; disabling or reloading ends the pause; a reload that fails
  to start is paused across a restart; a root result provider that keeps
  crashing is paused and asked no more.
- [`crates/pane-core/tests/operations.rs`](../crates/pane-core/tests/operations.rs):
  a target that keeps crashing is paused and its caller is not; a target
  stopped with its caller again and again is not paused.
- [`crates/pane/tests/install.rs`](../crates/pane/tests/install.rs): in the
  window, three crashes show the toast and the paused command's reason, and
  Retry in the extension list starts it again.

## Limits

- The toast is the launcher's status line, replaced by the next action's
  outcome; the lasting status is in Manage extensions. When a crash of an
  operation's target pauses it, the caller's own answer (which reports the
  crash) is shown instead of the toast.
- A guest computing without yielding is not preempted, so it cannot crash
  or be paused before it yields (#18).
- The crash count is not kept across restarts; a package that crashes
  twice per session is never paused.
- Nothing here is platform-specific (it lives in `pane-core`); it has run on
  Linux, and runs in `cargo xtask ci` on Windows, macOS and Linux.
