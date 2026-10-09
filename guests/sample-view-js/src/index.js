// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pane's designed view sample in JavaScript: a counter whose screen is
// the tree its view's render answers — a column of a text and a row of
// buttons, drawn by Pane, not by the guest (docs/designed-tree.md, ADR
// 0036). The behaviour matches the Rust and TypeScript samples
// (sample-view, sample-view-ts): the same text, the same buttons.
//
// The view is written as elements (the JSX runtime's `jsxs`), which is
// what JSX compiles to (the TypeScript sample); the components and hooks
// come from `@pane-app/extension/view`.
import { jsxs } from "@pane-app/extension/jsx-runtime";
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
  return jsxs(Column, {
    gap: "m",
    children: [
      jsxs(Text, { style: "title", level: "primary", children: [`Count: ${count}`] }),
      jsxs(Row, {
        gap: "s",
        children: [
          jsxs(Button, {
            onClick: () => setCount(count + 1),
            children: ["Increment"],
          }),
          jsxs(Button, {
            onClick: () => setCount(Math.max(0, count - 1)),
            children: ["Decrement"],
          }),
          jsxs(Button, {
            tone: "destructive",
            onClick: () => setCount(0),
            children: ["Reset"],
          }),
        ],
      }),
    ],
  });
}

/** The command's one view: the counter, starting at zero. */
export const command = {
  async openView() {
    return createView(Counter);
  },
};
