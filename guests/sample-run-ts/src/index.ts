// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pane's Run sample in TypeScript: a command that runs what Windows' Run
// dialog (Win+R) runs through `pane:extension/run`, sharing the Run
// dialog's own history. Answers and errors match the Rust sample
// (guests/sample-run) and the JavaScript one.
import type { Command, LaunchRecord, LaunchType, List } from "@pane-app/extension";
import { showToast } from "@pane-app/extension/feedback";
import { deleteFromHistory, history, run } from "pane:extension/run@0.1.0";

/** What the no-view commands say when they are sent no command line. */
const NOTHING =
  "Run was sent no command line: give it an alias or make it a fallback in Settings, then type " +
  "a command line in root search";

/** Calls a host function, turning its error into a plain message. */
function host<T>(call: () => T): T {
  try {
    return call();
  } catch (error) {
    const payload = (error as { payload?: unknown }).payload;
    if (payload !== null && typeof payload === "object" && "tag" in payload) {
      throw new Error(String((payload as { val?: unknown }).val));
    }
    throw typeof payload === "string" ? new Error(payload) : error;
  }
}

/**
 * Runs the command line `text` was sent, elevated when `elevated`,
 * showing a toast that says what ran (a launch in the background shows
 * nothing).
 */
async function runLine(
  text: string | null | undefined,
  elevated: boolean,
  launchType: LaunchType,
): Promise<void> {
  const line = text?.trim();
  if (line == null || line === "") throw new Error(NOTHING);
  host(() => run(line, elevated));
  if (launchType !== "background") {
    showToast({
      title: elevated ? `Ran ${line} as an administrator` : `Ran ${line}`,
    });
  }
}

/** Removes `line` from the shared history, saying so. */
async function deleteLine(line: string): Promise<void> {
  host(() => deleteFromHistory(line));
  showToast({ title: "Deleted from Run's history" });
}

export const command: Command = {
  async run(id: string, launch: LaunchRecord): Promise<void> {
    let elevated: boolean;
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

  async render(): Promise<List> {
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

  async runSearchResult(id: string): Promise<void> {
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
