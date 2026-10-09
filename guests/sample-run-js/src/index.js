// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pane's Run sample in JavaScript: a command that runs what Windows' Run
// dialog (Win+R) runs through `pane:extension/run`, sharing the Run
// dialog's own history. Answers and errors match the Rust sample
// (guests/sample-run) and the TypeScript one.
// @ts-check
import { showToast } from "@pane-app/extension/feedback";
import {
  deleteFromHistory,
  history,
  run,
} from "pane:extension/run@0.1.0";

/** What the no-view commands say when they are sent no command line. */
const NOTHING =
  "Run was sent no command line: give it an alias or make it a fallback in Settings, then type " +
  "a command line in root search";

/**
 * Calls a host function, turning its error into a plain message.
 * @template T
 * @param {() => T} call
 * @returns {T}
 */
function host(call) {
  try {
    return call();
  } catch (error) {
    const payload = /** @type {{ payload?: unknown }} */ (error).payload;
    if (payload !== null && typeof payload === "object" && "tag" in payload) {
      throw new Error(String(/** @type {{ val?: unknown }} */ (payload).val));
    }
    throw typeof payload === "string" ? new Error(payload) : error;
  }
}

/**
 * Runs the command line `text` was sent, elevated when `elevated`,
 * showing a toast that says what ran (a launch in the background shows
 * nothing).
 * @param {string | null | undefined} text
 * @param {boolean} elevated
 * @param {import("@pane-app/extension").LaunchType} launchType
 * @returns {Promise<void>}
 */
async function runLine(text, elevated, launchType) {
  const line = text?.trim();
  if (line == null || line === "") throw new Error(NOTHING);
  host(() => run(line, elevated));
  if (launchType !== "background") {
    showToast({
      title: elevated ? `Ran ${line} as an administrator` : `Ran ${line}`,
    });
  }
}

/**
 * Removes `line` from the shared history, saying so.
 * @param {string} line
 * @returns {Promise<void>}
 */
async function deleteLine(line) {
  host(() => deleteFromHistory(line));
  showToast({ title: "Deleted from Run's history" });
}

/** @type {import("@pane-app/extension").Command} */
export const command = {
  async run(id, launch) {
    let elevated;
    if (id === "run") {
      elevated = false;
    } else if (id === "elevated") {
      elevated = true;
    } else if (id === "history") {
      throw new Error("`history` opens a screen; it has no run entry point");
    } else {
      throw new Error(`unknown command: ${id}`);
    }
    await runLine(launch.fallbackText, elevated, launch.launchType);
  },

  async render() {
    const entries = host(history);
    if (entries.length === 0) {
      return {
        title: "Run History",
        items: [
          {
            id: "empty",
            title: "Nothing has been run yet",
            subtitle: "What you run in Pane or the Run dialog appears here",
          },
        ],
      };
    }
    return {
      title: "Run History",
      items: entries.map((line) => ({
        id: `entry:${line}`,
        title: line,
        subtitle: "Enter runs it again",
        onAction: () => runLine(line, false, "user-initiated"),
        actions: [
          {
            title: "Delete",
            style: "destructive",
            onAction: () => deleteLine(line),
          },
        ],
      })),
    };
  },

  async runSearchResult(id) {
    if (!id.startsWith("entry:")) throw new Error(`unknown item: ${id}`);
    await runLine(id.slice("entry:".length), false, "user-initiated");
  },

  async submitForm() {
    throw { message: "this sample has no forms" };
  },

  async openView() {
    throw new Error("this sample has no custom views");
  },
};
