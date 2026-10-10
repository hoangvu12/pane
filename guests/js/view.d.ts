// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The designed view runtime's types (view.js): the four UI components
// with every property named — a property an author misspells fails the
// TypeScript build — the element type JSX produces, the hooks, and
// `createView`, the view a command's `openView` answers with.

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

/** One element of the tree: what JSX evaluates to. */
export interface Element {
  readonly type: unknown;
  readonly props: Record<string, unknown>;
  readonly key: string | number | undefined;
}

/** What every node's properties may name: its stable key, the name
 * assistive technology reads it by, the navigation title the view is
 * named by when the node is the tree's root, the minimum minor version
 * of the UI component set it needs, and the element drawn when Pane does
 * not know the node. */
export interface NodeProps {
  key?: string;
  name?: string;
  /** What names the view, when this node is the tree's root: shown where
   * a screen's title is. Ignored on any other node. */
  navigationTitle?: string;
  requires?: number;
  fallback?: Element | null;
  children?: Children;
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

/** A text's properties. */
export interface TextProps extends NodeProps {
  style?: TextStyle;
  level?: TextLevel;
}

/** A button's properties. */
export interface ButtonProps extends NodeProps {
  tone?: Tone;
  onClick?: () => unknown;
}

/** A column: children below each other. */
export declare const Column: (props: ColumnProps) => Element;

/** A row: children beside each other. */
export declare const Row: (props: RowProps) => Element;

/** One line of text. */
export declare const Text: (props: TextProps) => Element;

/** A button, pressed by its `onClick`. */
export declare const Button: (props: ButtonProps) => Element;

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
 * waits before asking again (ignored until timers land). */
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
