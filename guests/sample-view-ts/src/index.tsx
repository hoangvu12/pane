// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pane's designed view sample in TypeScript: a counter whose screen is
// the tree its view's render answers — a column of a text and a row of
// buttons, drawn by Pane, not by the guest (docs/designed-tree.md, ADR
// 0036). The behaviour matches the Rust and JavaScript samples
// (sample-view, sample-view-js): the same text, the same buttons.
//
// The `components` command of the same package answers a gallery of
// every component of the UI component set (#237): the layout primitives,
// the shared components, the tokens and the raw values, with a toggle to
// show the tree changes.
//
// The view is written as JSX: every property of every component is
// type-checked, so a misspelling fails the build (the `jsxImportSource`
// in tsconfig.json makes the transform use the SDK's runtime).
import type { Command } from "@pane-app/extension";
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

/** The gallery of components the `components` command shows. */
function Components() {
  const [on, setOn] = useState(false);
  return (
    <Scroll>
      <Column gap="l">
        <Text style="heading">The UI component set</Text>
        {/* A stack: a badge over an icon tile, placed. */}
        <Stack place="top-end">
          <IconTile icon={{ builtin: "layers" }} />
          <Badge place="bottom-end" offset={{ x: 4, y: 4 }}>
            4
          </Badge>
        </Stack>
        {/* A text with spans, one a link. */}
        <Text>
          Accept the <Span onClick={() => {}}>terms</Span>
          <Span code> before continuing.</Span>
        </Text>
        <Row name="Marks" gap="s">
          <Icon icon="star" size="l" />
          {/* A raw blue, corrected for contrast. */}
          <Icon icon={{ builtin: "bell", tint: "#88ccff" }} />
          <Keycap>ctrl</Keycap>
          <KeySequence keys={["ctrl", "shift", "p"]} />
          <Tag color="blue">beta</Tag>
          <Badge>3</Badge>
        </Row>
        <Card gap="s">
          <RichRow
            subtitle="A tree Pane renders"
            icon={{ builtin: "layers" }}
            accessories={[{ text: "new", tag: true }]}
            onClick={() => {}}
          >
            Pane
          </RichRow>
        </Card>
        {/* Controls, one of them live. */}
        <Column gap="s">
          <SectionHeader note="Every one focusable">Controls</SectionHeader>
          <Toggle on={on} label="Dark mode" onChange={() => setOn(!on)} />
          <Checkbox on={on} label="Remember" onChange={() => setOn(!on)} />
          <Select
            options={[
              { value: "daily", label: "Daily" },
              { value: "weekly", label: "Weekly" },
            ]}
            value="daily"
            label="Digest"
            onChange={() => {}}
          />
          <Slider value={0.4} label="Volume" onChange={() => {}} />
          <Progress value={0.7} label="Installed" />
          <Loading label="Checking" />
        </Column>
        <Column gap="s">
          <SectionHeader>Fields</SectionHeader>
          <TextInput placeholder="Type here" label="Name" onChange={() => {}}>
            typed
          </TextInput>
          <PasswordInput label="Secret" />
          <TextArea label="Notes">two lines</TextArea>
        </Column>
        {/* Markdown. */}
        <Markdown>
          {"# Markdown\n\nSome *prose*, `code` and [a link](https://pane.dev).\n\n- [x] drawn\n- [ ] still to do\n"}
        </Markdown>
        <MetadataList
          items={[
            { label: "Author", value: "Vu", onClick: () => {} },
            { label: "Tags", tags: ["one", "two"] },
            { separator: true },
            { label: "Kind", value: "sample" },
          ]}
        />
        <EmptyState title="Nothing here" description="The gallery is over" icon={{ builtin: "search-minus" }}>
          <Button onClick={() => {}}>Start over</Button>
        </EmptyState>
        {/* An image, with a placeholder while it stands in. */}
        <Image image={{ builtin: "image" }} size="xl" fit="cover">
          <Text level="tertiary">Loading…</Text>
        </Image>
        {/* Raw values: a surface with a variant, a tone, a corrected
            colour and an exact one. */}
        <Row gap="s">
          <Text level="secondary" background="danger" radius="m" hover={{ background: "accent" }}>
            Surface
          </Text>
          <Link onClick={() => {}}>A link</Link>
        </Row>
        <Row gap="s">
          <Text color="#88ccff">
            Corrected
          </Text>
          <Text color={{ raw: "#ff6363" }}>Exact</Text>
        </Row>
        <Divider />
        <Spacer />
      </Column>
    </Scroll>
  );
}

/** The command's views: the counter, or the gallery. */
export const command: Command = {
  async openView(commandId: string) {
    if (commandId === "sample") {
      return createView(Counter);
    }
    if (commandId === "components") {
      return createView(Components);
    }
    throw new Error("this command opens no designed view");
  },
};
