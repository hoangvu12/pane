// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pane's designed view sample in JavaScript: a counter whose screen is
// the tree its view's render answers — a column of a text and a row of
// buttons, drawn by Pane, not by the guest (docs/designed-tree.md, ADR
// 0036). The behaviour matches the Rust and TypeScript samples
// (sample-view, sample-view-ts): the same text, the same buttons.
//
// The `components` command of the same package answers a gallery of
// every component of the UI component set (#237): the layout primitives,
// the shared components, the tokens and the raw values, with a toggle to
// show the tree changes. Its fields hold state (#238): typing edits at
// once, the view echoes the value back, "Clear" sets it (the
// extension's value wins), and "Reorder" moves the keyed fields around,
// their state with them. And its `loading` command answers a view that
// loads something on open (#243): a loading state drawn at once, and what
// the load answered the moment it arrives — the arrival asks for the
// drawing itself, with no timer to wait for.
//
// The view is written as elements (the JSX runtime's `jsxs`), which is
// what JSX compiles to (the TypeScript sample); the components and hooks
// come from `@pane-app/extension/view`.
import { jsxs } from "@pane-app/extension/jsx-runtime";
import { waitFor } from "wasi:clocks/monotonic-clock@0.3.0";
import {
  Badge,
  Button,
  Card,
  Checkbox,
  Column,
  Divider,
  EmptyState,
  Icon,
  IconTile,
  Image,
  KeySequence,
  Keycap,
  Link,
  Loading,
  Markdown,
  MetadataList,
  PasswordInput,
  Progress,
  RichRow,
  Row,
  Scroll,
  SectionHeader,
  Segmented,
  Select,
  Slider,
  Spacer,
  Span,
  Stack,
  Tag,
  Text,
  TextArea,
  TextInput,
  Toggle,
  createView,
  usePending,
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

/** The gallery of components the `components` command shows. */
function Components() {
  const [on, setOn] = useState(false);
  const [name, setName] = useState("typed");
  const [notes, setNotes] = useState("two lines");
  const [swapped, setSwapped] = useState(false);
  const nameField = jsxs(TextInput, {
    key: "name",
    value: name,
    placeholder: "Type here",
    label: "Name",
    onInput: (value) => setName(value),
  });
  const notesField = jsxs(TextArea, {
    key: "notes",
    value: notes,
    label: "Notes",
    onChange: (value) => setNotes(value),
  });
  return jsxs(Scroll, { key: "gallery", grow: 1,
    children: [
      jsxs(Column, {
        gap: "l",
        children: [
          jsxs(Text, { style: "heading", children: ["The UI component set"] }),
          // A stack: a badge over an icon tile, placed.
          jsxs(Stack, {
            place: "top-end",
            children: [
              jsxs(IconTile, { icon: { builtin: "layers" } }),
              jsxs(Badge, {
                place: "bottom-end",
                offset: { x: 4, y: 4 },
                children: ["4"],
              }),
            ],
          }),
          // A text with spans, one a link.
          jsxs(Text, {
            children: [
              "Accept the ",
              jsxs(Span, { onClick: () => {}, children: ["terms"] }),
              jsxs(Span, { code: true, children: [" before continuing."] }),
            ],
          }),
          jsxs(Row, {
            name: "Marks",
            gap: "s",
            children: [
              jsxs(Icon, { icon: "star", size: "l" }),
              // A raw blue, corrected for contrast.
              jsxs(Icon, { icon: { builtin: "bell", tint: "#88ccff" } }),
              jsxs(Keycap, { children: ["ctrl"] }),
              jsxs(KeySequence, { keys: ["ctrl", "shift", "p"] }),
              jsxs(Tag, { color: "blue", children: ["beta"] }),
              jsxs(Badge, { children: ["3"] }),
            ],
          }),
          jsxs(Card, {
            gap: "s",
            children: [
              jsxs(RichRow, {
                subtitle: "A tree Pane renders",
                icon: { builtin: "layers" },
                accessories: [{ text: "new", tag: true }],
                onClick: () => {},
                children: ["Pane"],
              }),
            ],
          }),
          // Controls, one of them live.
          jsxs(Column, {
            gap: "s",
            children: [
              jsxs(SectionHeader, {
                note: "Every one focusable",
                children: ["Controls"],
              }),
              jsxs(Toggle, {
                on,
                label: "Dark mode",
                onChange: () => setOn(!on),
              }),
              jsxs(Checkbox, {
                on,
                label: "Remember",
                onChange: () => setOn(!on),
              }),
              jsxs(Segmented, {
                options: [
                  { value: "daily", label: "Daily" },
                  { value: "weekly", label: "Weekly" },
                ],
                value: "daily",
                label: "Digest",
                onChange: () => {},
              }),
              jsxs(Select, {
                options: [
                  { value: "daily", label: "Daily" },
                  { value: "weekly", label: "Weekly" },
                ],
                value: "daily",
                label: "Pick",
                onChange: () => {},
              }),
              jsxs(Slider, { value: 0.4, label: "Volume", onChange: () => {} }),
              jsxs(Progress, { value: 0.7, label: "Installed" }),
              jsxs(Loading, { label: "Checking" }),
            ],
          }),
          // The fields, live and keyed (#238): the name field hears its
          // value as the user types (the view echoing it back, which
          // never fights the typing), the notes field on its commits,
          // "Clear" sets both (the extension's value replacing the text),
          // and "Reorder" swaps the two fields, whose keys keep their
          // state.
          jsxs(Column, {
            gap: "s",
            children: [
              jsxs(SectionHeader, { note: "Live, keyed", children: ["Fields"] }),
              // "Reorder" swaps the fields' places, not their state: the
              // keys keep each field's text, caret and focus.
              ...(swapped ? [notesField, nameField] : [nameField, notesField]),
              jsxs(Row, {
                gap: "s",
                children: [
                  jsxs(Button, {
                    onClick: () => {
                      setName("");
                      setNotes("");
                    },
                    children: ["Clear"],
                  }),
                  jsxs(Button, {
                    onClick: () => setSwapped(!swapped),
                    children: ["Reorder"],
                  }),
                ],
              }),
              jsxs(Text, { children: [`Echo: ${name}`] }),
              jsxs(PasswordInput, { label: "Secret" }),
            ],
          }),
          // Markdown.
          jsxs(Markdown, {
            children: [
              "# Markdown\n\nSome *prose*, `code` and [a link](https://pane.dev).\n\n- [x] drawn\n- [ ] still to do\n",
            ],
          }),
          jsxs(MetadataList, {
            items: [
              { label: "Author", value: "Vu", onClick: () => {} },
              { label: "Tags", tags: ["one", "two"] },
              { separator: true },
              { label: "Kind", value: "sample" },
            ],
          }),
          jsxs(EmptyState, {
            title: "Nothing here",
            description: "The gallery is over",
            icon: { builtin: "search-minus" },
            children: [jsxs(Button, { onClick: () => {}, children: ["Start over"] })],
          }),
          // An image, with a placeholder while it stands in.
          jsxs(Image, {
            image: { builtin: "image" },
            size: "xl",
            fit: "cover",
            children: [jsxs(Text, { level: "tertiary", children: ["Loading…"] })],
          }),
          // Raw values: a surface with a variant, a tone, a corrected
          // colour and an exact one.
          jsxs(Row, {
            gap: "s",
            children: [
              jsxs(Text, {
                level: "secondary",
                background: "danger",
                radius: "m",
                hover: { background: "accent" },
                children: ["Surface"],
              }),
              jsxs(Link, { onClick: () => {}, children: ["A link"] }),
            ],
          }),
          jsxs(Row, {
            gap: "s",
            children: [
              jsxs(Text, { color: "#88ccff", children: ["Corrected"] }),
              jsxs(Text, { color: { raw: "#ff6363" }, children: ["Exact"] }),
            ],
          }),
          jsxs(Divider, {}),
          jsxs(Spacer, {}),
        ],
      }),
    ],
  });
}

/** What the loading sample loads: held back for a moment, as work from a
 * service would be, so the loading state shows once. */
const load = () => waitFor(300_000_000).then(() => "Pane drew this the moment it arrived");

/** The loading sample: a loading state drawn at once, and what the load
 * answered the moment it arrives. */
function LoadingView() {
  const what = usePending(load);
  if (what === undefined) {
    return jsxs(Text, { level: "secondary", children: ["Loading…"] });
  }
  return jsxs(Column, {
    gap: "m",
    children: [
      jsxs(Text, { style: "title", children: ["Loaded"] }),
      jsxs(Text, { level: "secondary", children: [what] }),
    ],
  });
}

/** The command's views: the counter, the gallery, or the loading sample. */
export const command = {
  async openView(commandId) {
    if (commandId === "sample") {
      return createView(Counter);
    }
    if (commandId === "components") {
      return createView(Components);
    }
    if (commandId === "loading") {
      return createView(LoadingView);
    }
    throw new Error("this command opens no designed view");
  },
};
