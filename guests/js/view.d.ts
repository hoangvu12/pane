// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The designed view runtime's types (view.js): every UI component of the
// component set with every property named — a property an author
// misspells fails the TypeScript build — the element type JSX produces,
// the hooks, and `createView`, the view a command's `openView` answers
// with.

/** The named distances of the UI component set. */
export type Space = "xs" | "s" | "m" | "l" | "xl" | "xxl";

/** How a container's children are laid out along its cross axis. */
export type Align = "start" | "center" | "end" | "stretch" | "baseline";

/** How a container's children share its main axis. */
export type Justify = "start" | "center" | "end" | "space-between" | "space-around";

/** The style of a text: its size, weight and family. */
export type TextStyle = "heading" | "title" | "body" | "caption" | "mono" | "small-mono";

/** The level of a text: its colour, through the alpha of the text colour. */
export type TextLevel = "primary" | "secondary" | "tertiary" | "quaternary";

/** The tone of a button. */
export type Tone = "default" | "secondary" | "ghost" | "accent" | "destructive";

/** One of the nine places a stack's child is put at. */
export type Place =
  | "top-start"
  | "top"
  | "top-end"
  | "start"
  | "center"
  | "end"
  | "bottom-start"
  | "bottom"
  | "bottom-end";

/** Which way a scroll scrolls, or a divider runs. */
export type Orientation = "vertical" | "horizontal";

/** How an image fits the box it is given. */
export type Fit = "contain" | "cover" | "fill";

/** The padding inside a node: a token for all sides, or `{x, y}` or each
 * side naming some of them. */
export type Padding =
  | Space
  | {
      x?: Space;
      y?: Space;
      top?: Space;
      right?: Space;
      bottom?: Space;
      left?: Space;
    };

/** One length: a space token, pixels, or a fraction of the parent as
 * `"1/2"` or `"50%"`. */
export type Length = Space | number | `${number}px` | `${number}/${number}` | `${number}%`;

/** A corner radius: a token, or pixels. */
export type Radius = "s" | "m" | "l" | "full" | number | `${number}px`;

/** How big an icon or an image is: a token, or pixels. */
export type IconSize = "s" | "m" | "l" | "xl" | number | `${number}px`;

/** One colour: a tone's name, or a raw colour (`#RGB`, `#RRGGBB`,
 * `#RRGGBBAA`, `rgb()`, `rgba()`, `hsl()`, `hsla()`). */
export type Color = Tone | (string & {});

/** A colour, or a light and dark pair of them. */
export type ColorOrPair = Color | { light: Color; dark: Color };

/** One colour a node draws with: corrected for contrast, a pair, or one
 * given exactly — `{"raw": …}` — with correction off. */
export type Paint = ColorOrPair | { raw: ColorOrPair };

/** An icon, as the list tree names one: a reicon name, an object with
 * `builtin`, `path`, `light` and `dark`, `url`, `file` or `application`,
 * each with `tint`, `mask`, `fallback` and `tooltip` (see
 * `@pane-app/extension/icons`). */
export type IconSource =
  | string
  | {
      builtin?: string;
      filled?: boolean;
      path?: string;
      light?: string;
      dark?: string;
      url?: string;
      file?: string;
      application?: string;
      tint?: Paint;
      mask?: "circle" | "rounded-rectangle";
      fallback?: IconSource;
      tooltip?: string;
    };

/** One element of the tree: what JSX evaluates to. */
export interface Element {
  readonly type: unknown;
  readonly props: Record<string, unknown>;
  readonly key: string | number | undefined;
}

/** What every node's properties may name: its stable key, the name
 * assistive technology reads it by, the navigation title the view is
 * named by when the node is the tree's root, the minimum minor version
 * of the UI component set it needs, the element drawn when Pane does not
 * know the node, and the style every node carries — its sizing, its
 * surface with its hover and pressed variants, and its place in a
 * stack. */
export interface NodeProps {
  key?: string;
  name?: string;
  /** What names the view, when this node is the tree's root: shown where
   * a screen's title is. Ignored on any other node. */
  navigationTitle?: string;
  requires?: number;
  fallback?: Element | null;
  /** How much the node grows to fill the space its parent shares out. */
  grow?: number;
  /** How much the node shrinks when its parent runs out of space. */
  shrink?: number;
  /** The node's flex basis. */
  basis?: Length;
  /** The node's width. */
  width?: Length;
  /** The node's height. */
  height?: Length;
  /** The node's least width. */
  minWidth?: Length;
  /** The node's most width. */
  maxWidth?: Length;
  /** The node's least height. */
  minHeight?: Length;
  /** The node's most height. */
  maxHeight?: Length;
  /** The node's width over its height. */
  aspectRatio?: number;
  /** The node's background colour. */
  background?: Paint;
  /** The node's border: how wide, and in which colour. */
  border?: { width?: Length; color?: Paint };
  /** The node's corner radius. */
  radius?: Radius;
  /** The node's opacity, 0 to 1. */
  opacity?: number;
  /** The surface the node draws while the pointer is over it, restating
   * any of its properties. */
  hover?: SurfaceProps;
  /** The surface the node draws while it is pressed, restating any of
   * its properties. */
  pressed?: SurfaceProps;
  /** Where in a stack the node is placed. */
  place?: Place;
  /** How far the node sits from its place in a stack. */
  offset?: Length | { x?: Length; y?: Length };
  children?: Children;
}

/** The surface a node draws, as a variant of it restates. */
export interface SurfaceProps {
  background?: Paint;
  border?: { width?: Length; color?: Paint };
  radius?: Radius;
  opacity?: number;
}

/** What a node's children may be: elements, strings, numbers and lists of
 * them, nested. */
export type Children = unknown;

/** A column's properties. */
export interface ColumnProps extends NodeProps {
  gap?: Space;
  padding?: Padding;
  align?: Align;
  justify?: Justify;
  wrap?: boolean;
}

/** A row's properties. */
export interface RowProps extends NodeProps {
  gap?: Space;
  padding?: Padding;
  align?: Align;
  justify?: Justify;
  wrap?: boolean;
}

/** A stack's properties. */
export interface StackProps extends NodeProps {
  /** Where a child that says none is placed. */
  align?: Place;
}

/** A scroll's properties. */
export interface ScrollProps extends NodeProps {
  orientation?: Orientation;
}

/** A divider's properties. */
export interface DividerProps extends NodeProps {
  orientation?: Orientation;
}

/** A spacer's properties: the style it carries is all it is. */
export interface SpacerProps extends NodeProps {}

/** A text's properties. Its children spell its content, or its spans. */
export interface TextProps extends NodeProps {
  style?: TextStyle;
  level?: TextLevel;
  color?: Paint;
  size?: number;
  weight?: number;
  truncate?: boolean;
}

/** One span of a text's content. Its children spell it. */
export interface SpanProps {
  style?: TextStyle;
  level?: TextLevel;
  color?: Paint;
  code?: boolean;
  onClick?: () => unknown;
  children?: Children;
}

/** A button's properties. Its children spell its label. */
export interface ButtonProps extends NodeProps {
  tone?: Tone;
  icon?: IconSource;
  keys?: string[];
  enabled?: boolean;
  onClick?: () => unknown;
}

/** A link's properties. Its children spell its label. */
export interface LinkProps extends NodeProps {
  color?: Paint;
  onClick?: () => unknown;
}

/** An icon's properties. */
export interface IconProps extends NodeProps {
  icon?: IconSource;
  size?: IconSize;
}

/** An icon tile's properties. */
export interface IconTileProps extends NodeProps {
  icon?: IconSource;
  size?: IconSize;
}

/** An image's properties. Its children stand in for it while it loads. */
export interface ImageProps extends NodeProps {
  image?: IconSource;
  size?: IconSize;
  fit?: Fit;
}

/** A rich row's properties. Its children spell its title. */
export interface RichRowProps extends NodeProps {
  subtitle?: string;
  icon?: IconSource;
  accessories?: { text: string; tag?: boolean; color?: Paint }[];
  onClick?: () => unknown;
}

/** A keycap's properties. Its children spell its key. */
export interface KeycapProps extends NodeProps {
  children?: Children;
}

/** A key sequence's properties. */
export interface KeySequenceProps extends NodeProps {
  keys: string[];
}

/** A tag's properties. Its children spell its text. */
export interface TagProps extends NodeProps {
  color?: Paint;
}

/** A badge's properties. Its children spell its text. */
export interface BadgeProps extends NodeProps {
  color?: Paint;
}

/** A toggle's properties. */
export interface ToggleProps extends NodeProps {
  on?: boolean;
  label?: string;
  onChange?: () => unknown;
}

/** A checkbox's properties. */
export interface CheckboxProps extends NodeProps {
  on?: boolean;
  label?: string;
  onChange?: () => unknown;
}

/** One option of a segmented control or a select. */
export interface Option {
  value: string;
  label?: string;
}

/** A segmented control's properties. */
export interface SegmentedProps extends NodeProps {
  options: Option[];
  value?: string;
  label?: string;
  onChange?: () => unknown;
}

/** A select's properties. */
export interface SelectProps extends NodeProps {
  options: Option[];
  value?: string;
  label?: string;
  onChange?: () => unknown;
}

/** A slider's properties. */
export interface SliderProps extends NodeProps {
  value: number;
  min?: number;
  max?: number;
  step?: number;
  label?: string;
  onChange?: () => unknown;
}

/** A progress bar's properties. */
export interface ProgressProps extends NodeProps {
  value: number;
  label?: string;
}

/** A loading indicator's properties. */
export interface LoadingProps extends NodeProps {
  label?: string;
}

/** Markdown's properties. Its children spell its source. */
export interface MarkdownProps extends NodeProps {
  children?: Children;
}

/** A card's properties. */
export interface CardProps extends NodeProps {
  gap?: Space;
  padding?: Padding;
  align?: Align;
  justify?: Justify;
  wrap?: boolean;
}

/** A section header's properties. Its children spell its title. */
export interface SectionHeaderProps extends NodeProps {
  note?: string;
}

/** One row of a metadata list. */
export interface MetadataItem {
  label?: string;
  value?: string;
  tags?: string[];
  separator?: boolean;
  onClick?: () => unknown;
}

/** A metadata list's properties. */
export interface MetadataListProps extends NodeProps {
  items: MetadataItem[];
}

/** An empty state's properties. Its children are its actions. */
export interface EmptyStateProps extends NodeProps {
  title: string;
  description?: string;
  icon?: IconSource;
}

/** One text field's properties. Its children spell its value. */
export interface TextInputProps extends NodeProps {
  placeholder?: string;
  label?: string;
  onChange?: () => unknown;
}

/** A column: children below each other. */
export declare const Column: (props: ColumnProps) => Element;

/** A row: children beside each other. */
export declare const Row: (props: RowProps) => Element;

/** A stack: children drawn over each other, each placed. */
export declare const Stack: (props: StackProps) => Element;

/** A scrolling region. */
export declare const Scroll: (props: ScrollProps) => Element;

/** Space: it grows to fill what it is given. */
export declare const Spacer: (props: SpacerProps) => Element;

/** A hairline rule. */
export declare const Divider: (props: DividerProps) => Element;

/** One line of text, its children spelling it or its spans. */
export declare const Text: (props: TextProps) => Element;

/** One styled or linked span of a text's content. */
export declare const Span: (props: SpanProps) => Element;

/** A button, pressed by its `onClick`. */
export declare const Button: (props: ButtonProps) => Element;

/** A link, pressed by its `onClick`. */
export declare const Link: (props: LinkProps) => Element;

/** One icon, by name, file, URL or system reference. */
export declare const Icon: (props: IconProps) => Element;

/** One icon on Pane's tile. */
export declare const IconTile: (props: IconTileProps) => Element;

/** One image, its children standing in for it while it loads. */
export declare const Image: (props: ImageProps) => Element;

/** A rich row: a title, a subtitle, an icon and accessories. */
export declare const RichRow: (props: RichRowProps) => Element;

/** One keycap: the key its cap shows. */
export declare const Keycap: (props: KeycapProps) => Element;

/** A key sequence: the keys its caps show. */
export declare const KeySequence: (props: KeySequenceProps) => Element;

/** One tag: a short label in a chip. */
export declare const Tag: (props: TagProps) => Element;

/** One badge: a short count in a filled chip. */
export declare const Badge: (props: BadgeProps) => Element;

/** One toggle: on or off, changed by its `onChange`. */
export declare const Toggle: (props: ToggleProps) => Element;

/** One checkbox: checked or not, changed by its `onChange`. */
export declare const Checkbox: (props: CheckboxProps) => Element;

/** A segmented control, its choice changed by its `onChange`. */
export declare const Segmented: (props: SegmentedProps) => Element;

/** One slider, adjusted by its `onChange`. */
export declare const Slider: (props: SliderProps) => Element;

/** One progress bar. */
export declare const Progress: (props: ProgressProps) => Element;

/** One loading indicator. */
export declare const Loading: (props: LoadingProps) => Element;

/** Markdown, its children spelling its source. */
export declare const Markdown: (props: MarkdownProps) => Element;

/** A card of children, on Pane's own card surface. */
export declare const Card: (props: CardProps) => Element;

/** A section header: a title over a group. */
export declare const SectionHeader: (props: SectionHeaderProps) => Element;

/** A metadata list: rows of a label and its value. */
export declare const MetadataList: (props: MetadataListProps) => Element;

/** An empty state: an icon, a title and a description. */
export declare const EmptyState: (props: EmptyStateProps) => Element;

/** One text input. */
export declare const TextInput: (props: TextInputProps) => Element;

/** One password field. */
export declare const PasswordInput: (props: TextInputProps) => Element;

/** One text area. */
export declare const TextArea: (props: TextInputProps) => Element;

/** A select, its choice changed by its `onChange`. */
export declare const Select: (props: SelectProps) => Element;

/** A fragment: its children are drawn where it sits, unwrapped. */
export declare const Fragment: unique symbol;

/**
 * State the component keeps between renders: `[value, set]`. `set` stores
 * the next value (a function is called with the current one); Pane asks
 * for the tree again after each event, which is when it is read.
 */
export declare function useState<S>(initial: S | (() => S)): [S, (next: S | ((current: S) => S)) => void];

/** A value the component keeps between renders, mutable in place. */
export declare function useRef<T>(initial: T): { current: T };

/** A value computed once, recomputed when `deps` changes. */
export declare function useMemo<T>(make: () => T, deps: unknown[]): T;

/**
 * Runs `run` once each time `ms` passes while the view is open: Pane asks
 * for the tree again after `ms`, and the run happens in that render,
 * before the tree is drawn, so the tree shows what it changed. The view's
 * clock is Pane's, so the time a render takes, or a hidden view, delays
 * the next run — which the next drawing then serves, once, rather than
 * replaying the runs that passed. Each render registers the run afresh;
 * a view that stops rendering an interval stops asking for it.
 */
export declare function useInterval(ms: number, run: () => void | Promise<void>): void;

/**
 * Data the view is waiting for, as ordinary loading state: `undefined`
 * while the work `load` started has not answered — render a loading state
 * for that — and its answer once it has. The first render starts the work
 * and answers `undefined`, so the loading state is shown at once; Pane is
 * asked for the tree again promptly, and that render awaits the work. The
 * work runs once.
 */
export declare function usePending<T>(load: () => Promise<T>): T | undefined;

/** The event Pane sends a designed view: the id of the callback its tree
 * named, with the sequence number of the render whose tree the user saw
 * and the key of the node the event was raised on. */
export interface UiEvent {
  render: number;
  key: string;
  callback: number;
  payload: string;
}

/** What a designed view's `render` answers: its tree, and how long Pane
 * waits before asking again, which a view that changes by itself asks for
 * (see `useInterval` and `usePending`). */
export interface Rendered {
  tree: string;
  refreshAfterMs?: number | null;
}

/** What handling a designed view's event does next: the navigation the
 * answer carries, applied by Pane's navigation stack (#239). At most one
 * of a push, a replace and a pop is acted on — an answer that gives
 * several acts on its push first. */
export interface Outcome {
  push: unknown;
  replace: unknown;
  pop: string | null;
}

/** What a button's `onClick` (or any listener) may return, to navigate
 * (#239): at most one of a push, a replace and a pop. The `push` and
 * `replace` name what opens — an element or a view — and `onPop`, given
 * with a push, runs when the pushed view pops, with the result its pop
 * answered (or `undefined`, the back key's), the view re-rendering
 * after. */
export interface Navigation {
  push?: Element | DesignedView;
  replace?: Element | DesignedView;
  pop?: string | null;
  onPop?: (result: string | undefined) => unknown;
}

/** A designed view, as `createView` answers it (an object with a `render`
 * and a `handleEvent`), what a push or a replace opens. */
export interface DesignedView {
  render(context: string): Promise<Rendered>;
  handleEvent(event: UiEvent): Promise<Outcome>;
}

/** What a handler returns to push a view above this one: `target`, an
 * element or a view, this view staying below it. `onPop`, when given,
 * runs with the result the pushed view's pop answered — or `undefined`,
 * the back key's — before this view re-renders. */
export declare function push(
  target: Element | DesignedView,
  onPop?: (result: string | undefined) => unknown,
): Navigation;

/** What a handler returns to replace this view with `target`: this view is
 * dropped, the views below it staying. */
export declare function replace(target: Element | DesignedView): Navigation;

/** What a handler returns to pop this view, answering `result` (or an
 * empty one) to the view below. */
export declare function pop(result?: string): Navigation;

/** A press that pushes a view — the Raycast-style `Action.Push`, as a
 * button's `onClick`: `Push(<Detail />)` where Raycast writes
 * `<Action.Push target={<Detail />} />`. `target` is an element or a view;
 * `onPop`, when given, runs with the result the pushed view's pop
 * answered (or `undefined`, the back key's) before this view re-renders. */
export declare function Push(
  target: Element | DesignedView,
  onPop?: (result: string | undefined) => unknown,
): () => Navigation;

/** The view a command's `openView` answers with (pane.d.ts's
 * `DesignedView`): `component` rendered as its tree, its state kept
 * across renders, its listeners called by the events Pane sends.
 * `props`, when given, are passed to the component. */
export declare function createView<Props extends Record<string, unknown>>(
  component: (props: Props) => Children,
  props?: Props,
): {
  render(context: string): Promise<Rendered>;
  handleEvent(event: UiEvent): Promise<Outcome>;
};
