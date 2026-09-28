# Extension data

Added for [#39](https://github.com/hoangvu12/pane/issues/39) (US60, US64, T20,
G5; contributions, not whole-gate claims). It implements the accepted
[Clear cache](extension-policy-proposal.md#disable-cache-data-and-uninstall)
behavior: remove disposable data and keep settings, user content and
credentials, controlled by Pane rather than by running the extension.

## Four kinds of data

An installed package's commands keep string values by key through Pane, one
`pane:extension` interface per kind ([`wit/data.wit`](../wit/data.wit), world
`extension-with-data`;
Rust `pane_guest::{settings, content, cache, credentials}`, JS/TS modules
`pane:extension/<kind>@0.1.0`, typed in
[`guests/js/data.d.ts`](../guests/js/data.d.ts)). Each interface has
the same `get` and `set`; the kind is what decides what Pane's management
actions do with a value.

| Kind | What it is for | File in `extensions/` | Clear cache |
|---|---|---|---|
| Settings | The user's choices for the extension | `settings.json` | kept |
| Content | The extension's own durable records, such as notes or history | `content.json` | kept |
| Cache | Values the extension can compute or download again | `cache.json` | removed |
| Local credentials (`credentials`) | Secrets kept on this computer, such as a sign-in token | `credentials.json`, readable only by the user on macOS and Linux (mode 0600) | kept |

The settings sample in [Rust](../guests/sample-settings/src/lib.rs),
[JavaScript](../guests/sample-settings-js/src/index.js) and
[TypeScript](../guests/sample-settings-ts/src/index.ts) keeps one value of
each: its greeting style, a note ("Save a note"), the last greeting ("Greet
me") and a token ("Sign in"). "Show what Pane keeps" answers with all four.

## Ownership

- Pane owns the files. Each holds every installed package's values of its
  kind under the package identity's key, so values belong to the source
  identity, never the title or the managed copy: two copies with the same
  title have separate data, and an update keeps it. Nothing is kept for a
  command built into Pane (`get` and `set` return an error).
- The extension owns its keys and the meaning of its values. Pane never reads
  them except to hand them back.
- Everything is kept while the package is disabled, updated or Pane is not
  running. A disabled package cannot `set` any kind.
- The files live next to `installed.json` in Pane's data folder, not in Pane's
  own compile cache (`cache_dir`, compiled components keyed by their bytes,
  which Pane manages itself and which Clear cache does not touch). An
  extension's cache is its data, owned by its identity like the other kinds;
  it is only disposable.
- Nothing outside these files is Pane's to delete: the package's source
  folder, files an extension wrote elsewhere, remote accounts and sessions.

## Migration

- Pane migrates the files' format: each has a `version` (now 1). A file of
  another version, or one that cannot be read, is reported to the extension
  by `get` and `set` and never overwritten.
- The extension migrates its own values when their meaning changes between
  its versions, in its own code. Pane keeps values across an update and does
  not transform them.
- A cache needs no migration: an extension should treat a cache value it
  cannot use as missing and make it again, since the user can clear it at any
  time.

## Clearing an extension's cache

Manage extensions lists, after each package's enable/disable row, a row
"Clear cache of <title>" per package, in the same order; its subtitle names
the source, so copies with the same title can be told apart. Choosing it asks
first: "Clear the cache of <title>?", the source, and "Pane deletes the data
this extension keeps as its cache. Its settings, content and credentials are
kept, and the extension does not run." Choosing Clear cache deletes that
identity's entry from `cache.json` and returns to the list with "Cleared the
cache of <title>"; Cancel or Esc returns without deleting. Pane reads
`cache.json` again just before, so another Pane process's values saved
meanwhile are kept.

- No guest runs: the package may be disabled, its component may not load and
  Pane's runtime may not have started. Its running instance, if any, is left
  as it is; the next `cache.get` returns nothing, but the instance may save a
  value it still holds, so the outcome then reads "Cleared the cache of
  <title>. A running instance may write it again until it stops."
- Other identities' caches, every other kind of data and every file outside
  `cache.json` stay as they were.
- If `cache.json` cannot be read, nothing is deleted and the error says so:
  "Could not clear the cache of <title>: Cannot read <path>: <reason>.
  Nothing was deleted. That file holds only extension caches: repair or delete
  it, then clear the cache again." Pane reads the file again on the next
  attempt, so no restart is needed. If it cannot be written, the error names
  the file and asks the user to check that Pane can write its folder.

## Checks

- `crates/pane-core/tests/clear_cache.rs`, for each language's settings sample:
  each kind before and after clearing, and after a restart; Cancel and Esc;
  two copies with the same title, with the source folders and a user document
  unchanged byte for byte; a disabled package whose managed component was
  replaced by garbage, cleared by a launcher without a runtime; an unreadable
  `cache.json`, unchanged with every other file, then cleared once deleted;
  another Pane's cache write between reading and clearing kept; the running
  instance note; `credentials.json` created, and rewritten, with mode 0600
  (Unix).
- `crates/pane/tests/install.rs`: the rows, confirmation text, Esc and the
  outcome in the native window.
- The native GUI smokes, screenshots 40 to 43: every kind shown, the
  confirmation, the outcome, and the cached greeting gone with the other
  kinds still shown and still in their files.

## Limits

- `get` and `set` only: an extension cannot delete one value or list its keys.
- Local credentials are plain text in Pane's data folder, not in the
  system's keychain ([decision](current-decisions.md#cross-cutting-details-preserved)).
  On macOS and Linux `credentials.json` is created with mode 0600, so other
  users of the computer cannot read it; on Windows it has its folder's
  permissions (normally the user's own profile). Nothing protects them from
  other extensions or programs running as the same user: the policy is
  lifecycle behavior, not secret isolation. Deleting a local credential never
  revokes a remote session.
- Clear cache covers `cache.json` only; an extension that writes cache files
  elsewhere has to manage them itself.
- The files are replaced atomically but not locked, as with settings: two
  Pane processes on one data folder can lose each other's last write.
- Uninstall with a data choice ([#40](https://github.com/hoangvu12/pane/issues/40))
  and deleting retained data ([#41](https://github.com/hoangvu12/pane/issues/41))
  build on these kinds: removing an identity's entry from each file is the
  same operation as clearing its cache.
