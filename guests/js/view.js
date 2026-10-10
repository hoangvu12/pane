// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The designed view runtime: JSX elements become the nodes of the tree
// Pane renders (docs/designed-tree.md, ADR 0036), and the four UI
// components this component set has — Column, Row, Text and Button — are
// values an author spreads into a layout. A function component runs per
// render, with the state of `useState`, `useRef` and `useMemo` kept per
// instance (by its place in the tree, and its key when it gives one), and
// Pane asks for the tree again after each event, so the component reads
// the state the event's listener changed. A view that changes by itself
// asks for that: `useInterval(ms, run)` runs `run` each time `ms` passes
// while the view is open, and `usePending(load)` presents data on its way
// as loading state (#236) — the tree Pane is asked to draw again is what
// asks.
//
// `createView(component)` makes the view a command's `openView` answers
// with: it implements `render(context)` and `handleEvent(event)` for
// Pane's envelope, hiding the wire format and the callback ids. The ids
// name each render's listeners; the tables of the last two renders are
// kept, so a press of what the user could see is delivered even if the
// view has rendered since, and an older one is dropped. React is not
// used: its scheduler needs timers that run only inside calls.
//
// The tree is JSON with the version of the UI component set it targets;
// a Pane that renders another major refuses it naming both versions, and
// one that knows less draws what it understands.

/** The version of the UI component set this SDK writes. */
const COMPONENT_SET = "1.0";

/** How long a loading state waits for its data before it is asked for
 * again: the floor of Pane's refreshes. */
const PROMPT_REFRESH_MS = 100;

/** The empty outcome: what the navigation stack lands with (#239). */
const NOTHING_NEXT = { push: null, replace: null, pop: null };

/**
 * A built-in component of the tree, named `name`: a function only so JSX
 * and TypeScript accept it as a component; the renderer reads the `node`
 * property instead of calling it.
 */
function builtin(name) {
  const component = () => {
    throw new Error(`the ${name} component is drawn by the tree runtime, never called`);
  };
  component.node = name;
  return component;
}

/** A column: children below each other. */
export const Column = builtin("column");
/** A row: children beside each other. */
export const Row = builtin("row");
/** One line of text. */
export const Text = builtin("text");
/** A button, pressed by its `onClick`. */
export const Button = builtin("button");

/** A fragment: its children are drawn where it sits, unwrapped. */
export const Fragment = Symbol.for("pane.extension.fragment");

/** The render in progress, where the hooks write; `null` outside one. */
let rendering = null;

/** The ticks and pending data this render registers (see `useInterval`
 * and `usePending`); `null` outside one. */
let collecting = null;

/**
 * State the component keeps between renders: `[value, set]`. `set` stores
 * the next value (a function is called with the current one); Pane asks
 * for the tree again after each event, which is when it is read.
 */
export function useState(initial) {
  const host = rendering ?? throwOutside("useState");
  const index = host.cursor++;
  if (index === host.cells.length) {
    host.cells.push(typeof initial === "function" ? initial() : initial);
  }
  const cells = host.cells;
  const set = (next) => {
    cells[index] = typeof next === "function" ? next(cells[index]) : next;
  };
  return [cells[index], set];
}

/** A value the component keeps between renders, mutable in place. */
export function useRef(initial) {
  const host = rendering ?? throwOutside("useRef");
  const index = host.cursor++;
  if (index === host.cells.length) {
    host.cells.push({ current: initial });
  }
  return host.cells[index];
}

/** A value computed once, recomputed when `deps` changes. */
export function useMemo(make, deps) {
  const host = rendering ?? throwOutside("useMemo");
  const index = host.cursor++;
  if (index === host.cells.length) {
    host.cells.push({ deps: deps ?? [], value: make() });
  }
  const cell = host.cells[index];
  if (!sameDeps(cell.deps, deps)) {
    cell.deps = deps ?? [];
    cell.value = make();
  }
  return cell.value;
}

/**
 * Runs `run` once each time `ms` passes while the view is open
 * (`useInterval(1000, ...)`, #236): Pane asks for the tree again after
 * `ms`, and the run happens in that render, before the tree is drawn, so
 * the tree shows what it changed. The view's clock is Pane's: an interval
 * is a request to be drawn again, so the time a render takes, or a hidden
 * view, delays the next run — which the next drawing then serves, once,
 * rather than replaying the runs that passed.
 *
 * Each render registers the run afresh, so the interval always runs the
 * listener the newest render named, and a view that stops rendering an
 * interval stops asking for it.
 */
export function useInterval(ms, run) {
  if (rendering === null) throwOutside("useInterval");
  collecting.ticks.push({ ms, run });
}

/**
 * Data the view is waiting for, as ordinary loading state (#236):
 * `usePending(load)` answers `undefined` while the work `load` started has
 * not answered — render a loading state for that — and its answer once it
 * has. The first render starts the work and answers `undefined`, so the
 * loading state is shown at once; Pane is asked for the tree again
 * promptly, and that render awaits the work, so its answer is drawn as
 * soon as it is there. The work is awaited inside the guest call, which
 * Pane bounds as any call (a work that never answers is the view's
 * failure, not a hung launcher).
 *
 * The work runs once; a view that wants it again renders another one.
 */
export function usePending(load) {
  const host = rendering ?? throwOutside("usePending");
  const index = host.cursor++;
  if (index === host.cells.length) {
    host.cells.push({ value: undefined, work: Promise.resolve().then(load) });
  }
  const cell = host.cells[index];
  collecting.pendings.push(cell);
  return cell.value;
}

/** Throws the "outside a render" error a hook's host is missing. */
function throwOutside(name) {
  throw new Error(`${name} was called outside a component's render`);
}

/** Whether two dependency lists are the same, shallowly. */
function sameDeps(a, b) {
  const other = b ?? [];
  return a.length === other.length && a.every((dep, index) => dep === other[index]);
}

/**
 * Runs `component` with `props`, its hook state kept in `cells` at `path`:
 * state the component had last render is still there, in hook order.
 */
function runComponent(component, props, cells, path, used) {
  let own = cells.get(path);
  if (own === undefined) {
    own = [];
    cells.set(path, own);
  }
  used.add(path);
  const previous = rendering;
  rendering = { cells: own, cursor: 0 };
  try {
    return component(props ?? {});
  } finally {
    rendering = previous;
  }
}

/**
 * `value`, an element or a list of them, as nodes appended to `into`:
 * each child's place under `parentPath` is its key, else its index, so
 * component instances keep their state across renders (as React's do:
 * keys keep a reordered instance's state, positions share one without).
 */
function write(value, parentPath, callbacks, cells, used, into) {
  if (value === null || value === undefined || typeof value === "boolean") {
    return;
  }
  if (typeof value === "string" || typeof value === "number") {
    into.push({ type: "text", text: String(value) });
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((child, index) =>
      writeChild(child, index, parentPath, callbacks, cells, used, into),
    );
    return;
  }
  writeChild(value, 0, parentPath, callbacks, cells, used, into);
}

/** One child at `index` under `parentPath`, as a node in `into`. */
function writeChild(child, index, parentPath, callbacks, cells, used, into) {
  if (child === null || child === undefined || typeof child === "boolean") {
    return;
  }
  if (typeof child === "string" || typeof child === "number") {
    into.push({ type: "text", text: String(child) });
    return;
  }
  if (Array.isArray(child)) {
    write(child, parentPath, callbacks, cells, used, into);
    return;
  }
  const at = child.key === undefined ? String(index) : String(child.key);
  const path = `${parentPath}/${at}`;
  const { type, props } = child;
  if (type === Fragment) {
    write((props ?? {}).children, path, callbacks, cells, used, into);
    return;
  }
  if (typeof type === "function") {
    if (typeof type.node === "string") {
      into.push(builtinNode(type.node, props, path, callbacks, cells, used));
      return;
    }
    // A function component: run it and write what it rendered, its
    // children's places under its own.
    const rendered = runComponent(type, props, cells, path, used);
    write(rendered, path, callbacks, cells, used, into);
    return;
  }
  throw new Error(`a tree's element type must be a component, not ${typeof type}`);
}

/** One built-in node, with the properties `props` gives it. */
function builtinNode(name, props, path, callbacks, cells, used) {
  const given = props ?? {};
  const children = [];
  write(given.children, path, callbacks, cells, used, children);
  const node = { type: name };
  if (given.key !== undefined) node.key = given.key;
  if (given.name !== undefined) node.name = given.name;
  if (given.requires !== undefined) node.requires = given.requires;
  if (given.fallback != null) {
    // The fallback is drawn in the node's place when Pane does not know
    // it, with its own children under the node's place.
    const fallback = [];
    write(given.fallback, path, callbacks, cells, used, fallback);
    node.fallback = fallback[0];
  }
  switch (name) {
    case "column":
    case "row": {
      if (given.gap !== undefined) node.gap = given.gap;
      if (given.padding !== undefined) node.padding = given.padding;
      if (given.align !== undefined) node.align = given.align;
      if (given.justify !== undefined) node.justify = given.justify;
      if (given.wrap !== undefined) node.wrap = given.wrap;
      node.children = children;
      return node;
    }
    case "text": {
      node.text = textOf(given.children);
      if (given.style !== undefined) node.style = given.style;
      if (given.level !== undefined) node.level = given.level;
      return node;
    }
    case "button": {
      node.label = textOf(given.children);
      if (given.tone !== undefined) node.tone = given.tone;
      if (typeof given.onClick === "function") {
        const id = callbacks.size + 1;
        callbacks.set(id, given.onClick);
        node.onPress = id;
      }
      return node;
    }
    default:
      throw new Error(`the tree has no ${name} component`);
  }
}

/** The text `children` spell: strings and numbers, joined. */
function textOf(children) {
  if (children === null || children === undefined) return "";
  if (typeof children === "string") return children;
  if (typeof children === "number") return String(children);
  if (Array.isArray(children)) return children.map(textOf).join("");
  throw new Error("a text's children must be strings or numbers");
}

/**
 * The view a command's `openView` answers with: `component` rendered as
 * its tree, its state kept across renders, its listeners called by the
 * events Pane sends. `props`, when given, are passed to the component.
 */
export function createView(component, props = {}) {
  /** The hook cells of each component instance, by its place in the tree. */
  const cells = new Map();
  /** The listeners of the last two renders, by render number and id. */
  const tables = new Map();
  /** The ticks and pending data the last render registered. */
  let registered = { ticks: [], pendings: [] };
  /** What the last render asked Pane for: "none", a "prompt" refresh to
   * await pending data, or the refresh the view asked for ("asked"). */
  let ask = "none";
  /** Whether an event was handled since the last render: its render is
   * not one that answers a refresh. */
  let afterEvent = false;
  /** The newest render asked, to number a context that names none. */
  let newest = 0;
  return {
    async render(context) {
      const number = numberOf(context, newest + 1);
      newest = Math.max(newest, number);
      /** The render's listeners, by the ids its tree names. */
      const callbacks = new Map();
      /** The component places this render visited, to drop the rest. */
      const used = new Set();
      // This render answers the refresh the last one asked for, unless an
      // event's answer asked for it: the ticks run and the pending work is
      // awaited before the tree is drawn, so it shows what they changed.
      // Ticks run only on the view's own ask, not on the prompt a loading
      // state asks for, which awaits the data instead.
      const answering = ask !== "none" && !afterEvent;
      const ticking = answering && ask === "asked";
      afterEvent = false;
      if (answering) {
        if (ticking) {
          for (const { run } of registered.ticks) await run();
        }
        for (const cell of registered.pendings) {
          if (cell.value === undefined) cell.value = await cell.work;
        }
      }
      /** What this render registers, replacing the last render's. */
      const collected = { ticks: [], pendings: [] };
      const root = [];
      collecting = collected;
      try {
        write({ type: component, props, key: undefined }, "", callbacks, cells, used, root);
      } finally {
        collecting = null;
      }
      for (const path of cells.keys()) {
        if (!used.has(path)) cells.delete(path);
      }
      tables.set(number, callbacks);
      // Keep the last two renders' tables: an event raised on the tree the
      // user saw is delivered even if the view has rendered since.
      for (const render of tables.keys()) {
        if (render < number - 1) tables.delete(render);
      }
      if (root.length !== 1) {
        throw new Error("a view renders one root node");
      }
      registered = collected;
      // What Pane is asked to wait before the tree again: the soonest the
      // view's intervals ask for, and promptly while data is pending.
      let refreshAfterMs = null;
      for (const { ms } of collected.ticks) {
        refreshAfterMs = Math.min(refreshAfterMs ?? Infinity, ms);
      }
      const waiting = collected.pendings.some((cell) => cell.value === undefined);
      if (waiting) {
        refreshAfterMs = Math.min(refreshAfterMs ?? Infinity, PROMPT_REFRESH_MS);
      }
      ask = refreshAfterMs === null ? "none" : waiting ? "prompt" : "asked";
      return {
        tree: JSON.stringify({ version: COMPONENT_SET, root: root[0] }),
        refreshAfterMs,
      };
    },
    async handleEvent(event) {
      afterEvent = true;
      // A press of a button the tree named: the table of the render the
      // user saw holds it. An older event is stale, dropped.
      const run = tables.get(event.render)?.get(event.callback);
      if (run !== undefined) {
        await run();
      }
      return NOTHING_NEXT;
    },
  };
}

/** The render number `context` names, or `fallback` when it says none. */
function numberOf(context, fallback) {
  try {
    const named = JSON.parse(context)?.render;
    return typeof named === "number" && named > 0 ? named : fallback;
  } catch {
    return fallback;
  }
}
