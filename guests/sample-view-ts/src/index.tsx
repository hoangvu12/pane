// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pane's designed view sample in TypeScript: a counter whose screen is
// the tree its view's render answers — a column of a text and a row of
// buttons, drawn by Pane, not by the guest (docs/designed-tree.md, ADR
// 0036). The behaviour matches the Rust and JavaScript samples
// (sample-view, sample-view-js): the same text, the same buttons.
//
// The view is written as JSX: every property of every component is
// type-checked, so a misspelling fails the build (the `jsxImportSource`
// in tsconfig.json makes the transform use the SDK's runtime).
import type { Command } from "@pane-app/extension";
import {
  Button,
  Column,
  Row,
  Text,
  createView,
  useState,
} from "@pane-app/extension/view";

/** The counter the screen shows. */
function Counter() {
  const [count, setCount] = useState(0);
  return (
    <Column gap="m">
      <Text style="title" level="primary">
        {`Count: ${count}`}
      </Text>
      <Row gap="s">
        <Button onClick={() => setCount(count + 1)}>Increment</Button>
        <Button onClick={() => setCount(Math.max(0, count - 1))}>Decrement</Button>
        <Button tone="destructive" onClick={() => setCount(0)}>
          Reset
        </Button>
      </Row>
    </Column>
  );
}

/** The command's one view: the counter, starting at zero. */
export const command: Command = {
  async openView() {
    return createView(Counter);
  },
};
