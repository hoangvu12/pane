// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Declarations for `pane:extension/system-commands` in
// wit/system-commands.wit: the session and power commands — locking the
// screen, logging out, restarting, shutting down, sleeping, hibernating,
// turning the displays off and starting the screen saver. Most commands
// use them through `@pane-app/extension/system-commands`
// (system-commands.d.ts).

/** `pane:extension/system-commands@0.1.0`. */
declare module "pane:extension/system-commands@0.1.0" {
  /**
   * What a command ended in: the state the system is in now (`done`), or
   * why nothing changed (`explained`). Its text is what the command shows
   * the user.
   */
  export type Outcome = { tag: "done"; val: string } | { tag: "explained"; val: string };

  /** Locks the screen; the user's session stays as it is. */
  export function lockScreen(): Outcome;

  /**
   * Logs the user out. Applications are asked to close and given the
   * chance to save, not forced (the user's choice, ADR 0040).
   */
  export function logOut(): Outcome;

  /**
   * Restarts the computer, forcing applications closed (the user's
   * choice, ADR 0040).
   */
  export function restart(): Outcome;

  /** Powers the computer off, forcing applications closed as a restart does. */
  export function shutDown(): Outcome;

  /**
   * Sleeps the computer: on one that enters Modern Standby when its
   * displays turn off, it turns them off and Windows enters its standby;
   * otherwise it suspends the computer.
   */
  export function sleep(): Outcome;

  /**
   * Hibernates the computer, which a hibernation file makes possible;
   * without one, nothing changes and the answer says so.
   */
  export function hibernate(): Outcome;

  /** Turns the displays off. */
  export function turnOffDisplays(): Outcome;

  /**
   * Starts the screen saver the user chose; when none is set, nothing
   * changes and the answer says so.
   */
  export function startScreenSaver(): Outcome;
}
