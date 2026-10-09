// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Declarations for `pane:extension/run` in wit/run.wit: what Windows'
// Run dialog (Win+R) runs — a command line naming a program by its name
// or path with arguments, a Control Panel applet, a management console, a
// document, a folder, a network path, a `shell:` or `ms-settings:`
// address or another registered scheme, with environment variables
// expanded — run for the command, through Windows' own elevation prompt
// when asked, together with the Run dialog's own history, which the
// command and Windows' Run dialog share in both directions. Windows
// only. Only a command whose package.json sets `"pane": { "run": true }`
// imports it.

/** `pane:extension/run@0.1.0`. */
declare module "pane:extension/run@0.1.0" {
  /**
   * Why a run or a history call did not answer, as the call throws it
   * (an object whose `payload` is one of these): `tag` tells the cases
   * apart.
   *
   * - `not-available`: running what the Run dialog runs is Windows-only,
   *   and this Pane may reach no system at all; nothing went wrong, and
   *   the command can do something else;
   * - `failed`: it went wrong. A run that ran but whose history could not
   *   be written says that, saying it ran;
   * - `declined`: the user declined Windows' elevation prompt: nothing
   *   ran, and nothing was recorded.
   *
   * `val` says why for people.
   */
  export type RunError =
    | { tag: "not-available"; val: string }
    | { tag: "failed"; val: string }
    | { tag: "declined"; val: string };

  /**
   * Runs the command line `line` as the Run dialog reads it, through
   * Windows' own elevation prompt when `elevated` (which the user may
   * decline). A run that answered is recorded in the Run dialog's
   * history, the trimmed line; a failed run, or a declined elevation,
   * records nothing. On failure it throws an object whose `payload` is
   * the {@link RunError}.
   */
  export function run(line: string, elevated: boolean): void;

  /**
   * The Run dialog's history, newest first, as the user typed it: what
   * ran in Windows' Run dialog and in Pane alike. On failure it throws an
   * object whose `payload` is the {@link RunError}.
   */
  export function history(): string[];

  /**
   * Removes `line` from the Run dialog's history, matched ignoring case,
   * so it is gone from Windows' Run dialog too; a line that is not in the
   * history throws an object whose `payload` is the {@link RunError}.
   */
  export function deleteFromHistory(line: string): void;
}
