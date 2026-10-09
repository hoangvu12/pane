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
 * assistive technology reads it by, the minimum minor version of the UI
 * component set it needs, and the element drawn when Pane does not know
 * the node. */
export interface NodeProps {
  key?: string;
  name?: string;
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

/**
 * The view a command's `openView` answers with: `component` rendered as
 * its tree, its state kept across renders, its listeners called by the
 * events Pane sends. `props`, when given, are passed to the component.
 */
export declare function createView<Props extends Record<string, unknown>>(
  component: (props: Props) => Children,
  props?: Props,
): {
  render(context: string): Promise<{ tree: string; refreshAfterMs: number | null }>;
  handleEvent(event: {
    render: number;
    key: string;
    callback: number;
    payload: string;
  }): Promise<{ push: unknown; replace: unknown; pop: string | null }>;
};
