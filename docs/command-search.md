# Searching an online service inside its command

Added for [#30](https://github.com/hoangvu12/pane/issues/30): US11, US39,
US40; T03, T09; contributions to G2 and G3, not claims that they pass. An
extension command that searches an online service gets a **search field of
its own** once the user opens it; Pane sends it the text typed there and
lists what it finds. [Root search](root-search.md) never asks it: text typed
in root search reaches no such command, and so no service. Web requests go
through `wasi:http@0.3.0`, which Pane hosts for every command
([ADR 0017](adr/0017-extensions-reach-the-network-through-wasi-http.md),
proposed). The same on every system.

## What the user sees

Root search lists the command by its title like any other. Enter opens it:
the query field stays on screen, empty and focused (placeholder "Search"),
above the command's own list (its `get-view` items). Typing sends the text,
trimmed, to the command; while it answers the status says "Running…" and
the rows listed stay; its results then replace the rows, the first
selected. Enter on a result runs the command's action for it, whose answer
shows in the status line, as for any item. A blank field shows the
command's own list again without asking it. Nothing found: "No results for
“…”". Escape clears the text, then leaves the command. Forms of the
command's own items open and return as usual.

## Stopping searches and late answers

Each change of the text starts a new search and stops the one before, in
the runtime as well as in the launcher:

- A search still queued behind other calls is never started.
- A search waiting inside the guest (on its web request) is stopped there:
  the guest's call and its instance are dropped, as when a
  [generation](generations.md) ends, so the connection closes (the fixture
  service sees its client hang up) and nothing after the guest's `await`
  runs. The next search starts a new instance: in-memory state is lost.
- An answer that arrives anyway, or an older search's error, is never
  shown: the launcher's search epoch discards it.

Leaving the command (Escape, root search, another screen) stops its search
the same way. Disabling, reloading, updating or pausing the package stops it
through its generation, as for any call.

## Errors

An error the command answers with (the service unreachable, a status other
than 200, an answer it cannot read) is shown as the status error in place of
results, rows cleared: "The extension reported an error: Could not reach the
service at http://127.0.0.1:8740: connection refused", or "…: The service
answered 503: the registry is down for maintenance". It is an ordinary
operation error, so it never counts towards [pausing](pausing.md) the
extension, however often it happens; a stopped search is not a failure
either. A trap while searching is a crash, as for any call.

## For extension authors

In `pane.json`, a command sets `"search": true`; its component then also
exports `pane:extension/command-search` ([wit/search.wit](../wit/search.wit)):
`search(command, query) -> result<list<search-result>, string>`, where a
result has an `id` (passed to `run-action` when activated), a `title` and
an optional `subtitle`. Installing checks the export, as for the other
optional exports. Rust: `pane_guest::search::Guest` and
`pane_guest::search::export!`; JS/TS: export `commandSearch` with
`"pane": { "search": true }` in `package.json`.

Web requests: Rust `pane_guest::http::get(url, headers)`, JS/TS
`get(url, headers)` from `@pane/extension/http`, each returning the status,
headers and whole body; errors are readable ("connection refused"). Both
are thin wrappers over the standard `wasi:http@0.3.0` client
(`wasi:http/client.send`), whose bindings are available for anything else
(other methods, bodies, streaming). Libraries that build on `wasi:http` or
on these helpers work; ones that open sockets themselves or need Node.js's
or a browser's `fetch` do not. The samples read JSON with `serde_json`
(`no_std` + `alloc`) in Rust and `JSON.parse` in JS/TS. See
[guests/README.md](../guests/README.md#searching-inside-a-command).

The samples, **Package search** (`guests/sample-search`, `-js`, `-ts`,
packaged in `guests/packages/sample-search*`), search the **fixture
service**: a made-up package registry on 127.0.0.1
([crates/pane-core/tests/support/service.rs](../crates/pane-core/tests/support/service.rs);
`cargo run -p pane-core --example fixture_service`, port 8740, the
samples' default address). A form item changes the address (kept in the
extension's settings). `GET /search?q=` lists packages whose name contains
the text; a text starting with `slow` is held for ten seconds; `down`
answers 503; `GET /packages/<name>` gives the details Enter shows.

## Checks

- `crates/pane-core/tests/command_search.rs`, for the Rust, JavaScript and
  TypeScript samples against the fixture service on a free port of
  127.0.0.1: typing in root search (the command's title, a package name,
  `slow`, `down`) sends the service nothing, while the same text in the
  command does; results, details, an encoded text, nothing found and a blank
  text; a newer search stops the one the service holds (the service sees the
  hang-up well before its ten seconds) and the older answer never replaces
  the newer; an answer to an older search arriving after the newer one is
  not shown; Escape and leaving the command stop a search; an unreachable
  address four times, then the service's 503, are errors and the extension
  keeps working; a manifest saying `"search": true` for a component without
  the export is refused at install.
- `crates/pane/tests/command_search.rs`: the same through the window with
  real key events: the query field becomes the command's, the results are
  rendered, Escape clears then leaves.
- The native smokes' search phase (screenshots 160 to 168; Linux run, see
  [Linux](platforms/linux.md#searching-inside-a-command-30)), against the
  fixture service's log.

No test or smoke reaches beyond 127.0.0.1.

## Limits

- **Provisional, pending user confirmation:** the manifest key
  `"search": true` and interface name `command-search`; results are plain
  rows whose action is `run-action` (no forms, custom views, platforms or
  Pane-performed actions such as opening a link); a search is sent on every
  change of the text, with no debounce, and each stop drops the instance;
  rows stay listed while a search runs; errors clear the rows; the error
  text keeps the "The extension reported an error:" prefix.
- The runtime still serves calls one at a time: while one extension's
  search waits for its service, other extensions' calls wait too (#29, #18).
  Only the outermost call watches the stop: an operation a searching guest
  waits for runs to its end first.
- Network access is not gated: extensions are trusted code (ADR 0002), and
  any extension may use `wasi:http`, from any call, including root-results
  providers. Only this command kind is kept out of root search.
- HTTP/1.1 only; no proxy settings; TLS trusts the system's certificates
  (read once per Pane process); default timeouts 10 s to connect and 30 s
  for the response head. Wasmtime's `wasi:http` 0.3 is marked experimental
  upstream.
- `wasi:http` types are not declared in TypeScript (`wasi.d.ts`); use
  `@pane/extension/http` or type them yourself.
- HTTPS against a real service, and the macOS and Windows smokes, have not
  been run.
