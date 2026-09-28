# Aliases and fallbacks

Added for [#31](https://github.com/hoangvu12/pane/issues/31): US10, US11;
T03; contributions to G2, not claims that it passes. The user gives an
installed command an **alias** or makes it a **fallback** in Manage
extensions, and reaches it from [root search](root-search.md) in fewer
steps. Text typed in root search reaches a command that **takes a query**
only when the user invokes it that way: root search never sends it while
the user types, so no command (an online service, say) sees text meant for
another. Root-search routing only; [global hotkeys](hotkeys.md) are
separate. The same on every system: no system API is involved.

## Giving a command an alias

In **Manage extensions…**, after the hotkey rows, every command of an
enabled package has a row "Alias for &lt;command&gt;", subtitled with its alias
("“ec”") or "None · A word that finds it in root search", then its package's
source (copies of a package may share titles). Enter opens the form "Alias
for Echo" with one field, filled with the current alias; Save alias applies
it at once, records it and returns to the extension list with "Typing “ec”
now finds Echo". Leaving the field empty removes the alias ("Echo has no
alias now"). Refused, with the reason next to the field and the form kept:

| Alias | Explanation |
| --- | --- |
| With a space | "An alias is one word, without spaces" |
| Over 32 characters | "An alias has at most 32 characters" |
| Another command's, compared as root search compares text (case, NFC) | "“GR” is already the alias of Greeting: change it there first, or choose another" |

In root search, a query that is the alias lists the command first, above
every other result, computed results included; Enter opens it as usual.
For a command that takes a query, the alias, a space and more text ("ec
hello world") list a row titled with the command, subtitled "Send “hello
world” · alias ec", first and selected; Enter sends "hello world" (the text
after the alias, trimmed) and shows the command's answer as the result,
root search staying as it was. Text after the alias of a command that takes
no query sends nothing (the query is then matched as usual).

## Making a command a fallback

A command that takes a query also has a row "Fallback: Echo", "Off · Offer
it below the results for any text typed" or "On · …". Enter turns it on
("Echo is now offered for any text typed in root search") or off ("Echo is
no longer a fallback"). For any query that is not blank, each fallback is
listed **below every other result**, in the order the user turned them on,
subtitled "Send “zqx” · fallback". A fallback row is **never selected by
itself**: when nothing else matches, root search shows "No results for
“zqx”" above the fallbacks with nothing selected, so Enter does nothing, as
before; Down (or Up, or a click) selects one and Enter sends the whole
query, trimmed.

## What keeps and removes them

- Pane's own records, not the extension's data: `aliases.json` beside
  `installed.json`, by command id (the package identity's key and the
  manifest's command id), `{ "version": 1, "aliases": { "local:/…#echo":
  "ec" }, "fallbacks": ["local:/…#echo"] }`. Written atomically, one change
  at a time, each write holding the latest choices; a change that cannot be
  written goes back to what was last recorded. An unreadable file is not
  overwritten (a change then explains why).
- By command id, so copies of a package from other sources, even with the
  same titles, have their own; each is told apart by its source in Manage
  extensions.
- **Disabling** a package removes its commands' aliases and fallbacks from
  root search at once; they stay recorded, and their rows stay in Manage
  extensions with "Not active: Query sample is disabled". Changing them
  there does not enable the package, and nothing else does: only the user's
  enable in Manage extensions brings them back. A **paused** package's
  command stays reachable by its alias and fallback rows, which explain the
  pause and run nothing, as its own row does.
- An **update** or **reload** keeps them. A command the new copy no longer
  has, or one that no longer takes a query, is shown as not active: "Alias
  “gn” and fallback of a missing command · Not active: Query sample has no
  command `gone` now; Enter forgets it", or "On · Not active: Echo no
  longer takes a query".
- **Uninstall** forgets them, whether its saved data is kept or deleted
  (they are Pane's records). A record for a package that is not installed
  (a record edited by hand) is shown as a missing command too.
- Two commands with the same alias (only in a record edited by hand): both
  rows say "Not active: another command has the same alias", and root
  search uses neither.

## For extension authors

Any installed command can be given an alias, in Rust, JavaScript or
TypeScript alike; nothing is declared. Taking a query needs `"takesQuery":
true` on the command in `pane.json` and the export
`pane:extension/query-command` ([`wit/query.wit`](../wit/query.wit)),
`run-query(query) -> result<string, string>`, checked at install without
running it ("its manifest says it takes a query, but it does not export
pane:extension/query-command@0.1.0 …"). The [author guide](../guests/README.md#a-command-that-takes-a-query)
shows it in Rust (`pane_guest::query`); [`guests/sample-query`](../guests/sample-query)
is Echo, the query sample. An optional export, so the extension API stays
0.1 and no earlier component is refused. JavaScript and TypeScript cannot
export it yet (their build does not include it; adding it rebuilds every
prebuilt JS/TS sample).

## Checks

- Launcher public interface ([`crates/pane-core/tests/aliases.rs`](../crates/pane-core/tests/aliases.rs)),
  with the real query sample: an alias set in its form is recorded; the
  alias alone lists Echo first and "ec hello  world " lists the row that
  sends "hello  world", with Echo's guest not started while typing; Enter
  shows its answer and keeps root search; an error answer is shown; kept
  after a restart; the form starts with the alias and an empty one removes
  it; an alias ranks above a computed result (the calculator's). A fallback
  is listed last for any text, not selected, Enter then does nothing and
  Echo does not start; Down and Enter send the query; not for a blank query;
  turned off again. Refusals (another command's alias in other case, a
  space, 33 characters); a command that takes no query found by its alias,
  with no row for text after it and no fallback row. A conflict in a record
  edited by hand shown and neither alias used. Disabling removes both from
  root search and shows them as not active; changing them does not enable
  the package, nor does a restart; enabling brings both back. Two copies of
  the package from different folders: the alias finds and runs only the
  copy it was given to (checked with the runtime's running components). A
  missing command's choices shown and forgotten; uninstall forgets them; a
  manifest declaring `takesQuery` for a component without the export
  refused at install.
- Window ([`crates/pane/tests/aliases.rs`](../crates/pane/tests/aliases.rs)),
  on GPUI's test platform with real key events: the alias typed in its form
  and saved, the form reopened filled with it, the fallback turned on; "ec
  hello" and Enter show Echo's answer; "zqx" shows "No results" and the
  fallback unselected, Enter changes nothing, Down and Enter send "zqx".
- Native GUI smokes, one identical phase on all three systems (screenshots
  66 to 74): with data folders of their own, install the query sample, set
  the alias "ec" and the fallback in Manage extensions, send "ec hello" and,
  from the fallback chosen with Down, "zqx"; check `aliases.json`; restart,
  disable the extension and check that "ec hello" gives the same screen as
  a Pane with nothing installed. See the
  [Linux](platforms/linux.md#aliases-and-fallbacks-31),
  [macOS](platforms/macos.md#aliases-and-fallbacks-31) and
  [Windows](platforms/windows.md#aliases-and-fallbacks-31) notes for where
  it has run.

## Limits

- One alias per command, only for installed packages' commands (not the
  samples this build supplies), one word, compared after lowercasing and NFC
  like titles; no aliases for indexed or computed results (applications,
  quicklinks).
- The fallback rows are not ranked or limited: every fallback is listed for
  every non-blank query. No per-command placeholder or argument parsing: the
  command gets one string.
- The answer is shown as text on the status line; a command that takes a
  query cannot open its view with the query, or a form or list for it.
- The query is sent when the row is invoked and the call is not cancelled
  when the user types on (a late answer is still shown, like "Opened …");
  calls run one at a time on the runtime thread (#29, #18).
- JavaScript and TypeScript commands cannot take a query (above).
- Screen readers: fallback rows are ordinary options of the results list;
  with none selected, the combo box itself is reported as focused. No
  screen reader was run ([root search](root-search.md#accessibility)).
