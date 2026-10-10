// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pane's timer sample in TypeScript: a clock whose screen changes by
// itself — the view's tree asks Pane to render it again after every
// second (`refresh-after-ms`, #236) — with its caption arriving as pending
// data shown first as a loading state (`usePending`). The behaviour
// matches the Rust and JavaScript samples (sample-timer,
// sample-timer-js): the same texts, the same timings.
//
// The view is written as JSX: every property of every component is
// type-checked, so a misspelling fails the build (the `jsxImportSource`
// in tsconfig.json makes the transform use the SDK's runtime).
import type { Command } from "@pane-app/extension";
import {
  Column,
  Text,
  createView,
  useInterval,
  usePending,
  useState,
} from "@pane-app/extension/view";

/** What the timer says while its caption is still loading. */
const LOADING = "Loading…";

/** The work that fills the screen when it answers: the timer's caption. */
const loadCaption = () => Promise.resolve("A second at a time");

/** The timer the screen shows: a caption loaded before it runs, and the
 * seconds passed since, one per refresh. */
function Timer() {
  const caption = usePending(loadCaption);
  const [elapsed, setElapsed] = useState(0);
  useInterval(1000, () => setElapsed((seconds) => seconds + 1));
  if (caption === undefined) {
    return <Text level="secondary">{LOADING}</Text>;
  }
  return (
    <Column gap="m">
      <Text level="secondary">{caption}</Text>
      <Text style="title">{`Elapsed: ${elapsed}s`}</Text>
    </Column>
  );
}

/** The command's one view: the timer, loading its caption first. */
export const command: Command = {
  async openView() {
    return createView(Timer);
  },
};
