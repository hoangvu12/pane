// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pane's system commands sample in TypeScript: a command that calls the
// session and power commands with `@pane-app/extension/system-commands`
// (#255) — Lock Screen, Log Out, Restart, Shut Down, Sleep, Hibernate,
// Turn Off Displays and Start Screen Saver — one item for each, which
// runs it and says what it answered in a toast: the state the system
// ended in ("Sleep: Sleeping"), or why nothing changed ("Hibernate:
// Hibernation is not available on this computer: there is no hibernation
// file").
//
// The sample calls each host function as it is: no confirmation, no HUD —
// the System Commands default extension (guests/system-commands) is the
// one that composes them with ADR 0037's confirm and HUD host functions,
// and a test drives this sample with a fake system, so nothing it runs
// ever reaches the real one. Items and answers match the Rust sample
// (guests/sample-system-commands) and the JavaScript one.
import { showToast } from "@pane-app/extension/feedback";
import {
  hibernate,
  lockScreen,
  logOut,
  restart,
  shutDown,
  sleep,
  startScreenSaver,
  turnOffDisplays,
  type Outcome,
} from "@pane-app/extension/system-commands";

/** One item of the list: the host function it calls, by what it is called. */
interface Step {
  id: string;
  title: string;
  subtitle: string;
  ask: () => Outcome;
}

/** The items of the list, one per session or power command. */
const STEPS: Step[] = [
  {
    id: "lock-screen",
    title: "Lock Screen",
    subtitle: "Locks the screen; your session stays on",
    ask: lockScreen,
  },
  {
    id: "log-out",
    title: "Log Out",
    subtitle: "Ends your session; applications are not forced closed",
    ask: logOut,
  },
  {
    id: "restart",
    title: "Restart",
    subtitle: "Restarts the computer; applications are forced closed",
    ask: restart,
  },
  {
    id: "shut-down",
    title: "Shut Down",
    subtitle: "Powers the computer off; applications are forced closed",
    ask: shutDown,
  },
  {
    id: "sleep",
    title: "Sleep",
    subtitle: "Sleeps the computer, as its Modern Standby signal decides",
    ask: sleep,
  },
  {
    id: "hibernate",
    title: "Hibernate",
    subtitle: "Hibernates the computer, or says that it cannot",
    ask: hibernate,
  },
  {
    id: "turn-off-displays",
    title: "Turn Off Displays",
    subtitle: "Turns the displays off",
    ask: turnOffDisplays,
  },
  {
    id: "start-screen-saver",
    title: "Start Screen Saver",
    subtitle: "Starts the screen saver, or says that none is set",
    ask: startScreenSaver,
  },
];

/**
 * Says what `ask` answered in a toast: a success when it happened, a
 * failure when nothing changed.
 */
const answer = (title: string, outcome: Outcome) =>
  showToast({
    style: outcome.state === "done" ? "success" : "failure",
    title: `${title}: ${outcome.text}`,
  });

export const command = {
  async render() {
    return {
      title: "System commands",
      items: STEPS.map((step) => ({
        id: step.id,
        title: step.title,
        subtitle: step.subtitle,
        onAction: () => Promise.resolve(answer(step.title, step.ask())),
      })),
    };
  },
};
