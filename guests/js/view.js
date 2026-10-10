// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The designed view runtime: JSX elements become the nodes of the tree
// Pane renders (docs/designed-tree.md, ADR 0036), and every UI component
// of the component set — the layout primitives, the shared components,
// Markdown — is a value an author spreads into a layout. A function
// component runs per render, with the state of `useState`, `useRef` and
// `useMemo` kept per instance (by its place in the tree, and its key when
// it gives one), and Pane asks for the tree again after each event, so
// the component reads the state the event's listener changed.
//
// `createView(component)` makes the view a command's `openView` answers
// with: it implements `render(context)` and `handleEvent(event)` for
// Pane's envelope, hiding the wire format and the callback ids. The ids
// name each render's listeners; the tables of the last two renders are
// kept, so a press of what the user could see is delivered even if the
// view has rendered since, and an older one is dropped. React is not
// used: its scheduler needs timers that run only inside calls.
//
// A listener may navigate (#239), returning what `push`, `replace`, `pop`
// or `Push` answer: the pushed or replacing view is a view of the same
// kind, and the `onPop` a push was given runs when the pushed view pops,
// with the result its pop answered. The back key pops a view without
// asking; its pop event carries no result. A root node's
// `navigationTitle` names the view, shown where a screen's title is.
//
// The tree is JSON with the version of the UI component set it targets;
// a Pane that renders another major refuses it naming both versions, and
// one that knows less draws what it understands.

/** The version of the UI component set this SDK writes. */
const COMPONENT_SET = "1.1";

/** What the view answers for `refresh-after-ms` until timers land. */
const NO_REFRESH = null;

/** The empty outcome: what a handler that navigates nowhere answers. */
const NOTHING_NEXT = { push: null, replace: null, pop: null };

/** The callback id of the pop event, the event Pane sends a view when the
 * one above it popped: an id no tree names (this SDK's ids start at 1),
 * its payload `{"pop": <result>}`, the result the pop answered (`null`
 * when the back key popped, so no view's result reached the one below). */
const POP_CALLBACK = 0;

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
/** Children drawn over each other, each placed. */
export const Stack = builtin("stack");
/** A scrolling region. */
export const Scroll = builtin("scroll");
/** Space: it grows to fill what it is given. */
export const Spacer = builtin("spacer");
/** A hairline rule. */
export const Divider = builtin("divider");
/** One line of text, its children spelling it or its spans. */
export const Text = builtin("text");
/** One styled or linked span of a text's content. */
export const Span = builtin("span");
/** A button, pressed by its `onClick`. */
export const Button = builtin("button");
/** A link, pressed by its `onClick`. */
export const Link = builtin("link");
/** One icon, by name, file, URL or system reference. */
export const Icon = builtin("icon");
/** One icon on Pane's tile. */
export const IconTile = builtin("icon-tile");
/** One image, its children standing in for it while it loads. */
export const Image = builtin("image");
/** A rich row: a title, a subtitle, an icon and accessories. */
export const RichRow = builtin("rich-row");
/** One keycap: the key its cap shows. */
export const Keycap = builtin("keycap");
/** A key sequence: the keys its caps show. */
export const KeySequence = builtin("key-sequence");
/** One tag: a short label in a chip. */
export const Tag = builtin("tag");
/** One badge: a short count in a filled chip. */
export const Badge = builtin("badge");
/** One toggle: on or off, changed by its `onChange`. */
export const Toggle = builtin("toggle");
/** One checkbox: checked or not, changed by its `onChange`. */
export const Checkbox = builtin("checkbox");
/** A segmented control, its choice changed by its `onChange`. */
export const Segmented = builtin("segmented");
/** One slider, adjusted by its `onChange`. */
export const Slider = builtin("slider");
/** One progress bar. */
export const Progress = builtin("progress");
/** One loading indicator. */
export const Loading = builtin("loading");
/** Markdown, its children spelling its source. */
export const Markdown = builtin("markdown");
/** A card of children, on Pane's own card surface. */
export const Card = builtin("card");
/** A section header: a title over a group. */
export const SectionHeader = builtin("section-header");
/** A metadata list: rows of a label and its value. */
export const MetadataList = builtin("metadata-list");
/** An empty state: an icon, a title and a description. */
export const EmptyState = builtin("empty-state");
/** One text input. */
export const TextInput = builtin("text-input");
/** One password field. */
export const PasswordInput = builtin("password-input");
/** One text area. */
export const TextArea = builtin("text-area");
/** A select, its choice changed by its `onChange`. */
export const Select = builtin("select");

/** A fragment: its children are drawn where it sits, unwrapped. */
export const Fragment = Symbol.for("pane.extension.fragment");

/** The render in progress, where the hooks write; `null` outside one. */
let rendering = null;

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
  if (given.navigationTitle !== undefined) node.navigationTitle = given.navigationTitle;
  if (given.requires !== undefined) node.requires = given.requires;
  if (given.fallback != null) {
    // The fallback is drawn in the node's place when Pane does not know
    // it, with its own children under the node's place.
    const fallback = [];
    write(given.fallback, path, callbacks, cells, used, fallback);
    node.fallback = fallback[0];
  }
  // The style every node carries: its sizing and its surface, with its
  // hover and pressed variants, and its place in a stack.
  writeStyle(node, given);
  switch (name) {
    case "column":
    case "row":
    case "card": {
      for (const field of ["gap", "padding", "align", "justify"]) {
        if (given[field] !== undefined) node[field] = given[field];
      }
      if (given.wrap !== undefined) node.wrap = given.wrap;
      node.children = children;
      return node;
    }
    case "stack": {
      if (given.align !== undefined) node.align = given.align;
      node.children = children;
      return node;
    }
    case "scroll": {
      if (given.orientation !== undefined) node.orientation = given.orientation;
      node.children = children;
      return node;
    }
    case "divider": {
      if (given.orientation !== undefined) node.orientation = given.orientation;
      return node;
    }
    case "spacer": {
      return node;
    }
    case "text": {
      // The children spell the text, or its spans: a Span element among
      // them makes the text one of spans.
      const spans = [];
      const plain = [];
      let linked = false;
      for (const child of asList(given.children)) {
        if (isElement(child) && child.type === Span) {
          linked = true;
          spans.push(spanNode(child, path, callbacks));
        } else if (linked) {
          spans.push(spanNode(child, path, callbacks));
        } else {
          plain.push(child);
        }
      }
      if (linked) {
        node.spans = spans;
      } else {
        node.text = textOf(given.children);
      }
      if (given.style !== undefined) node.style = given.style;
      if (given.level !== undefined) node.level = given.level;
      if (given.color !== undefined) node.color = given.color;
      if (given.size !== undefined) node.size = given.size;
      if (given.weight !== undefined) node.weight = given.weight;
      if (given.truncate !== undefined) node.truncate = given.truncate;
      return node;
    }
    case "button": {
      node.label = textOf(given.children);
      if (given.tone !== undefined) node.tone = given.tone;
      if (given.icon !== undefined) node.icon = given.icon;
      if (given.keys !== undefined) node.keys = given.keys;
      if (given.enabled !== undefined) node.enabled = given.enabled;
      if (typeof given.onClick === "function") {
        const id = callbacks.size + 1;
        callbacks.set(id, given.onClick);
        node.onPress = id;
      }
      return node;
    }
    case "link": {
      node.label = textOf(given.children);
      if (given.color !== undefined) node.color = given.color;
      if (typeof given.onClick === "function") {
        const id = callbacks.size + 1;
        callbacks.set(id, given.onClick);
        node.onPress = id;
      }
      return node;
    }
    case "icon":
    case "icon-tile": {
      node.icon = given.icon ?? {};
      if (given.size !== undefined) node.size = given.size;
      return node;
    }
    case "image": {
      node.image = given.image ?? {};
      if (given.size !== undefined) node.size = given.size;
      if (given.fit !== undefined) node.fit = given.fit;
      node.children = children;
      return node;
    }
    case "rich-row": {
      node.title = textOf(given.children);
      if (given.subtitle !== undefined) node.subtitle = given.subtitle;
      if (given.icon !== undefined) node.icon = given.icon;
      if (given.accessories !== undefined) node.accessories = given.accessories;
      if (typeof given.onClick === "function") {
        const id = callbacks.size + 1;
        callbacks.set(id, given.onClick);
        node.onPress = id;
      }
      return node;
    }
    case "keycap": {
      node.key = textOf(given.children);
      return node;
    }
    case "key-sequence": {
      node.keys = given.keys ?? [];
      return node;
    }
    case "tag":
    case "badge": {
      node.text = textOf(given.children);
      if (given.color !== undefined) node.color = given.color;
      return node;
    }
    case "toggle":
    case "checkbox": {
      if (name === "toggle") node.on = given.on === true;
      else node.checked = given.on === true;
      if (given.label !== undefined) node.label = given.label;
      if (typeof given.onChange === "function") {
        const id = callbacks.size + 1;
        callbacks.set(id, given.onChange);
        node.onChange = id;
      }
      return node;
    }
    case "segmented":
    case "select": {
      node.options = given.options ?? [];
      if (given.value !== undefined) node.value = given.value;
      if (given.label !== undefined) node.label = given.label;
      if (typeof given.onChange === "function") {
        const id = callbacks.size + 1;
        callbacks.set(id, given.onChange);
        node.onChange = id;
      }
      return node;
    }
    case "slider": {
      node.value = given.value ?? 0;
      if (given.min !== undefined) node.min = given.min;
      if (given.max !== undefined) node.max = given.max;
      if (given.step !== undefined) node.step = given.step;
      if (given.label !== undefined) node.label = given.label;
      if (typeof given.onChange === "function") {
        const id = callbacks.size + 1;
        callbacks.set(id, given.onChange);
        node.onChange = id;
      }
      return node;
    }
    case "progress": {
      node.value = given.value ?? 0;
      if (given.label !== undefined) node.label = given.label;
      return node;
    }
    case "loading": {
      if (given.label !== undefined) node.label = given.label;
      return node;
    }
    case "markdown": {
      node.markdown = textOf(given.children);
      return node;
    }
    case "section-header": {
      node.title = textOf(given.children);
      if (given.note !== undefined) node.note = given.note;
      return node;
    }
    case "metadata-list": {
      node.items = given.items ?? [];
      return node;
    }
    case "empty-state": {
      if (given.title !== undefined) node.title = given.title;
      if (given.description !== undefined) node.description = given.description;
      if (given.icon !== undefined) node.icon = given.icon;
      node.children = children;
      return node;
    }
    case "text-input":
    case "password-input":
    case "text-area": {
      node.value = textOf(given.children);
      if (given.placeholder !== undefined) node.placeholder = given.placeholder;
      if (given.label !== undefined) node.label = given.label;
      if (typeof given.onChange === "function") {
        const id = callbacks.size + 1;
        callbacks.set(id, given.onChange);
        node.onChange = id;
      }
      return node;
    }
    default:
      throw new Error(`the tree has no ${name} component`);
  }
}

/** The style properties every node carries, written from `given`. */
function writeStyle(node, given) {
  for (const field of [
    "grow",
    "shrink",
    "basis",
    "width",
    "height",
    "minWidth",
    "maxWidth",
    "minHeight",
    "maxHeight",
    "aspectRatio",
    "background",
    "border",
    "radius",
    "opacity",
    "place",
  ]) {
    if (given[field] !== undefined) node[field] = given[field];
  }
  if (given.offset !== undefined) node.offset = given.offset;
  if (given.hover !== undefined) node.hover = given.hover;
  if (given.pressed !== undefined) node.pressed = given.pressed;
}

/** One span of a text's content, from `child` — a Span element or plain
 * text — with the listener its `onClick` names. */
function spanNode(child, path, callbacks) {
  const given = isElement(child) ? (child.props ?? {}) : { children: child };
  const span = { text: textOf(given.children) };
  for (const field of ["style", "level", "color"]) {
    if (given[field] !== undefined) span[field] = given[field];
  }
  if (given.code === true) span.code = true;
  if (typeof given.onClick === "function") {
    const id = callbacks.size + 1;
    callbacks.set(id, given.onClick);
    span.onPress = id;
  }
  return span;
}

/** Whether `value` is an element the tree draws. */
function isElement(value) {
  return value !== null && typeof value === "object" && typeof value.type !== "undefined";
}

/** `value` as the list of children it is. */
function asList(children) {
  if (children === null || children === undefined) return [];
  if (Array.isArray(children)) return children.flat();
  return [children];
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
 *
 * A listener may navigate, returning what [`push`], [`replace`], [`pop`]
 * or [`Push`] answer (or an object of the same shape): the pushed or
 * replacing view is a `createView` view too, and the `onPop` a push was
 * given runs when the pushed view pops, with the result its pop
 * answered, the view re-rendering after.
 */
export function createView(component, props = {}) {
  /** The hook cells of each component instance, by its place in the tree. */
  const cells = new Map();
  /** The listeners of the last two renders, by render number and id. */
  const tables = new Map();
  /** The `onPop` the view's last push registered, run when it pops. */
  let onPop = null;
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
      const root = [];
      write({ type: component, props, key: undefined }, "", callbacks, cells, used, root);
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
      return {
        tree: JSON.stringify({ version: COMPONENT_SET, root: root[0] }),
        refreshAfterMs: NO_REFRESH,
      };
    },
    async handleEvent(event) {
      // The pop event: the view above this one popped. Its payload is the
      // result that pop answered; the view re-renders after it, as after
      // every event.
      if (event.callback === POP_CALLBACK) {
        const run = onPop;
        onPop = null;
        if (run !== null) {
          await run(popResultOf(event.payload));
        }
        return NOTHING_NEXT;
      }
      // A press of a button the tree named: the table of the render the
      // user saw holds it. An older event is stale, dropped.
      const run = tables.get(event.render)?.get(event.callback);
      let navigation = null;
      if (run !== undefined) {
        navigation = await run();
      }
      // What the listener answered next: at most one of a push, a replace
      // and a pop is acted on — a pop first, then a replace, then a push,
      // as Pane does; the `onPop` a push was given stays here, never sent.
      const given = navigation === null || typeof navigation !== "object" ? null : navigation;
      const outcome = { push: null, replace: null, pop: null };
      if (given !== null) {
        if (given.push != null) outcome.push = viewOf(given.push);
        if (given.replace != null) outcome.replace = viewOf(given.replace);
        if (given.pop !== undefined) outcome.pop = given.pop ?? null;
        if (typeof given.onPop === "function") onPop = given.onPop;
      }
      return outcome;
    },
  };
}

/** The result the pop event's payload carries: `"…"` for a pop that
 * answered one, `null` for a pop that carried none (the back key's). */
function popResultOf(payload) {
  try {
    const result = JSON.parse(payload)?.pop;
    return typeof result === "string" ? result : null;
  } catch {
    return null;
  }
}

/** The view `target` names: a view `createView` made, or an element,
 * rendered as the tree of a new view of its own. */
function viewOf(target) {
  if (target !== null && typeof target === "object" && typeof target.render === "function") {
    return target;
  }
  if (target !== null && typeof target === "object" && typeof target.type === "function") {
    return createView(target.type, target.props);
  }
  throw new Error("a push or replace names what it opens: an element or a view");
}

/** What a listener returns to push a view above this one: `target`, an
 * element or a view, this view staying below it. `onPop`, when given, runs
 * with the result the pushed view's pop answered — or none, the back
 * key's — before this view re-renders. */
export function push(target, onPop) {
  return { push: viewOf(target), onPop };
}

/** What a listener returns to replace this view with `target`: this view
 * is dropped, the views below it staying. */
export function replace(target) {
  return { replace: viewOf(target) };
}

/** What a listener returns to pop this view, answering `result` (or an
 * empty one) to the view below. */
export function pop(result) {
  return { pop: result ?? "" };
}

/** A press that pushes a view — the Raycast-style `Action.Push`, as a
 * button's `onClick`: `Push(<Detail />)` where Raycast writes
 * `<Action.Push target={<Detail />} />`. `target` is an element or a view;
 * `onPop`, when given, runs with the result the pushed view's pop
 * answered (or none, the back key's) before this view re-renders. */
export function Push(target, onPop) {
  return () => push(target, onPop);
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
