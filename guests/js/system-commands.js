// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The session and power commands for a JS/TS command
// (`@pane-app/extension/system-commands`), through
// `pane:extension/system-commands@0.1.0` (wit/system-commands.wit):
// locking the screen, logging out, restarting, shutting down, sleeping,
// hibernating, turning the displays off and starting the screen saver.
// Bundled into the command that imports it, like any npm module; only a
// command whose bundle uses it imports the interface. Each function
// answers what it ended in — the state the system is in now, or why
// nothing changed — never throwing for an operation that cannot happen on
// this system, or failed: show the text in a HUD
// (`@pane-app/extension/feedback`), and confirm the destructive ones
// first, as the System Commands default extension does (ADR 0040).

import {
  hibernate as doHibernate,
  lockScreen as doLockScreen,
  logOut as doLogOut,
  restart as doRestart,
  shutDown as doShutDown,
  sleep as doSleep,
  startScreenSaver as doStartScreenSaver,
  turnOffDisplays as doTurnOffDisplays,
} from "pane:extension/system-commands@0.1.0";

/**
 * What a command ended in: the state the system is in now (`done`, with
 * the text to show), or why nothing changed (`explained`, with why): an
 * object `{ state: "done" | "explained", text }`, as
 * `@pane-app/extension/system-commands`'s types say
 * (system-commands.d.ts).
 */

/** An answered host call as an outcome. */
function answer(outcome) {
  return outcome.tag === "done"
    ? { state: "done", text: String(outcome.val) }
    : { state: "explained", text: String(outcome.val) };
}

/**
 * Locks the screen; the user's session stays as it is, and signing in
 * brings it back. Answers "Locking the screen".
 */
export function lockScreen() {
  return answer(doLockScreen());
}

/**
 * Logs the user out. Applications are asked to close and given the chance
 * to save, not forced (the user's choice, ADR 0040). Answers "Logging
 * out", or why nothing changed.
 */
export function logOut() {
  return answer(doLogOut());
}

/**
 * Restarts the computer, forcing applications closed: an application with
 * unsaved work cannot hold a restart up (the user's choice, ADR 0040).
 * Answers "Restarting", or why nothing changed.
 */
export function restart() {
  return answer(doRestart());
}

/**
 * Powers the computer off, forcing applications closed as a restart does.
 * Answers "Shutting down", or why nothing changed.
 */
export function shutDown() {
  return answer(doShutDown());
}

/**
 * Sleeps the computer: on one that enters Modern Standby when its
 * displays turn off, it turns them off and Windows enters its standby as
 * it does by itself; otherwise it suspends the computer. Answers
 * "Sleeping", or why nothing changed.
 */
export function sleep() {
  return answer(doSleep());
}

/**
 * Hibernates the computer, which a hibernation file makes possible;
 * without one, nothing changes and the answer says so ("Hibernation is
 * not available on this computer: there is no hibernation file").
 */
export function hibernate() {
  return answer(doHibernate());
}

/**
 * Turns the displays off. Answers "Turning off the displays", or why
 * nothing changed.
 */
export function turnOffDisplays() {
  return answer(doTurnOffDisplays());
}

/**
 * Starts the screen saver the user chose; when none is set, nothing
 * changes and the answer says so. Answers "Starting the screen saver".
 */
export function startScreenSaver() {
  return answer(doStartScreenSaver());
}
