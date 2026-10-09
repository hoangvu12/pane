=== #193 [OPEN] Match root search fuzzily and without accents, with a Search sensitivity setting and highlights assignees=
## Parent

https://github.com/hoangvu12/pane/issues/122

## What to build

Root search finds results by fuzzy, accent-blind matching with a sensitivity setting, as the parent's "Matching (the core's search module)" section describes. "vsc" finds Visual Studio Code, "cafe" finds "Café", "tieng viet" finds "Tiếng Việt", and the matched letters are highlighted. Ranking keeps today's order among the results that now match; the new comparator is #197. Size M-L.

- **Normalisation:** NFC, then transliteration through a bundled table (é→e, đ/Đ→d, ß→ss, ligatures split, covered scripts romanised), then lowercase and collapsed whitespace. Texts are normalised once per results build, as today (`Keys`, `crates/pane-core/src/search.rs` ~104-135). Aliases keep their own caseless comparison (`same_text`), not transliterated. `file_index/text.rs`'s `fold` already folds accents (NFKD, combining marks dropped). Reuse or share it where it fits, but it does not cover đ, ß or romanisation.
  - Choose the table: a crate such as `deunicode` or `any_ascii`, or Pane's own.
  - Record why in `docs/root-search.md`.
  - Check its licence against the project's licence policy (the release matrix's licences job). A crate that fails it is not used.
- **The scorer:** subsequence placement scoring 4 at the text's start, 3 at a word start, 2 elsewhere, 1 separator to separator, gap cost 1, the separators listed, unplaceable query separators skipped, exact normalised equality as its own outcome, and the quick in-order rejection for queries over two characters. It replaces `Query::rank`/`title_rank`'s every-word rule (`search.rs` ~171-227) as the match test.
- **What is matched:**
  - the title, each alternate title (already on indexed results from #124: `indexed.rs` ~243-246), the subtitle, the package title of an installed command (as today), and the composites "title subtitle" and "subtitle title";
  - the "/" rule;
  - aliases as today.
  
  Keywords join in #197.
- **Search sensitivity:**
  - Low, Medium (1.5·(n−2)+4) and High (above 2n, the default) in Pane's host settings (`crates/pane-core/src/host_settings.rs`), applied on the next keystroke;
  - a control on Settings' Launcher page (`crates/pane/src/features/settings/launcher.rs`), registered in its `entries` for Settings search.
- **Highlight:** the title characters of the best placement, drawn as today (`presentation.rs` ~248 → `ui/result_row.rs` ~134). A result found by another text highlights nothing.
- Until #197 lands, results that match are ordered by today's comparator, extended to rank a fuzzy title match after today's six steps.

## Acceptance criteria

- [ ] Core tests through `Launcher` (prior art: `crates/pane-core/tests/search.rs`, `launcher.rs`), covering the parent's matching cases:
  - abbreviations, word starts and gaps;
  - French accents, Vietnamese including đ, and ß;
  - title-and-subtitle spans;
  - each sensitivity level admitting and rejecting the documented cases;
  - highlight ranges;
  - aliases unchanged.
- [ ] Window tests with real key events (prior art: `crates/pane/tests/window.rs`, `launcher_settings.rs`): the sensitivity control changes the results live and is found through Settings search, and the highlight is drawn on the matched characters.
- [ ] `docs/root-search.md`'s matching section, "Not done" list (accent folding) and Settings documentation are updated.
- [ ] CI: the branch tier is green, and a `ci-fast` verify run is green. No release matrix.

## Blocked by

None - can start immediately


=== #194 [OPEN] Select the first fallback when nothing else matches assignees=
## Parent

https://github.com/hoangvu12/pane/issues/122

## What to build

When nothing but fallbacks is listed, the first fallback is selected, so Enter sends it the query (ADR 0031; on [PR #119's branch](https://github.com/hoangvu12/pane/blob/research/raycast-deep-dive/docs/adr/0031-the-first-fallback-is-preselected-when-nothing-else-matches.md) until it lands). This follows the parent's "Fallbacks (ADR 0031)" section. Size S.

- For a non-blank query where only fallbacks are listed, the first fallback (in the order the user turned them on) is selected. Enter sends it the trimmed query. Today `aliases::first_choice` skips fallbacks (`launcher/aliases.rs` ~296-306), and `move_selection` handles the case where nothing is selected (`launcher.rs` ~1907-1921).
- The no-results notice stays above the fallbacks. Its wording no longer says nothing is selected (`crates/pane/src/features/root_search/layouts.rs` ~21-61).
- When anything else is listed, fallbacks stay unselected below it.
- A fallback row is an ordinary row for number hints and Ctrl+digit.
- A command with arguments may be a fallback only if its first argument is text and every other is optional. The query fills that argument (#120's fallback text, `arguments.rs` `accept_fallback_text`). Fallback rows never show argument fields.
- The rule for late results moving a preselected fallback's selection is #201's publishing rule; until then, today's selection rule applies.
- **Docs:** `docs/root-search.md` and `docs/aliases.md` (a fallback is no longer never selected), and the glossary's Fallback and No-results notice entries through `/domain-modeling`.

## Acceptance criteria

- [ ] Core tests through `Launcher` (prior art: `crates/pane-core/tests/aliases.rs`):
  - with nothing else matching, the first fallback is selected and Enter sends the query;
  - with a result listed, no fallback is selected;
  - a fallback with arguments whose first is text receives the query, and one whose first is not text is not offered as a fallback.
- [ ] Window tests with real key events (prior art: `crates/pane/tests/aliases.rs`): typing a phrase and Enter runs the first fallback; the notice is shown above it; the preselected fallback's accessibility node is reported as the selection.
- [ ] Docs and glossary updated as above.
- [ ] CI: the branch tier is green, and a `ci-fast` verify run is green. No release matrix.

## Blocked by

None - can start immediately


=== #195 [OPEN] Understand typed web addresses and paths, and let commands appear only for them assignees=
## Parent

https://github.com/hoangvu12/pane/issues/122

## What to build

Root search understands typed web addresses and paths, and commands can declare that they appear only for them. This follows the parent's "Understanding the typed query" section: query analysis, "Where a command appears", and the URL and path rows of "Default extensions supply the rows". Typed-folder listing is #204, and colours, dates and percentages are #196. Size M-L.

- **Query analysis** belongs to the launcher and runs once per query change. It classifies a query as URL-like (an absolute URL with a scheme, or a bare domain with "https://" inferred) or path-like (a drive letter and separator, `\\`, `~` resolved to the home folder, `/`, or `file://`), with the parent's grammar.
- **Manifest:** a command may declare `when` (blank query, only while searching, or both) and `matches` (title, URL-like or path-like) in `pane.json` (`packages.rs` `CommandJson` → `ManifestCommand`). This is an additive extension API change. A command declared for URL-like or path-like queries:
  - is listed only for such queries, without title matching, below Results and above Files, in install order;
  - receives the parsed address or resolved path as its fallback text (`launch.rs` `LaunchRecord.fallback_text`).
- **Default extensions:**
  - Quicklinks (`guests/quicklinks`, `guests/packages/quicklinks/pane.json`) gains "Open in Browser" and "Create Quicklink" (address prefilled) for URL-like queries;
  - Files (`guests/files`) gains "Open" and "Reveal in File Explorer" for path-like queries. A program follows #126's rule: Enter shows it, Run runs it.
- **Docs:** the manifest reference, `docs/root-search.md`, `docs/quicklinks.md` and `docs/files.md`.

## Acceptance criteria

- [ ] Core tests through `Launcher` (prior art: `crates/pane-core/tests/quicklinks.rs`, `files.rs`, with their link and file opener seams):
  - query analysis cases (URLs, bare domains, non-domains such as "1.5", Windows and Unix paths, `~`, `file://`);
  - a `matches: url` sample command listed only for URL-like queries and receiving the parsed address;
  - the Quicklinks rows opening and prefilling;
  - the Files rows opening and revealing (a program revealed, never run);
  - `when` honoured.
  
  Samples in Rust, JavaScript and TypeScript for the manifest fields.
- [ ] Prebuilt artifacts for changed guests and samples are rebuilt.
- [ ] Release-validation evidence, not a merge gate: opening a typed URL and a typed path natively on Windows, through a smoke phase.
- [ ] Docs updated as above.
- [ ] CI: the branch tier is green, and a `ci-fast` verify run is green. No release matrix.

## Blocked by

None - can start immediately


=== #196 [OPEN] Answer colours, date and time words, and percentages in the calculator assignees=
## Parent

https://github.com/hoangvu12/pane/issues/122

## What to build

The calculator default extension answers colours, date and time words, and percentages. This follows the parent's "Colours", "Date and time words" and "Percentages" bullets. Size M.

- **Colours:** "#RGB", "#RGBA", "#RRGGBB", "#RRGGBBAA", `rgb()`/`rgba()`, `hsl()`/`hsla()` and `oklch()`. Colour names are not understood.
  - The answer is a computed answer card with a swatch, under the section title "Color". The card exists (`crates/pane/src/features/root_search/layouts.rs` `answer_card` ~78, fed by `ComputedAnswer` in `launcher/presentation.rs` ~105-112), but has no swatch field yet: add one, through the computed-answer contract, additively.
  - Enter copies uppercase hex. The Actions panel offers hex, RGB, HSL and OKLCH.
- **Date and time words:** "now", "time", "today", "date", "tomorrow" and "yesterday" answer at once with the local date or time in the system's format, under "Date & Time". Enter copies it. The Actions panel offers ISO 8601 and a Unix timestamp.
- **Percentages:** "p% of x", "p% off x", "p% on x", "x + p%", "x − p%" and "x as a % of y", within the calculator's number rules (`guests/calculator/src/expression.rs`).
- Units, currencies, time zones and date arithmetic stay out.

## Acceptance criteria

- [ ] Core tests through `Launcher` (prior art: `crates/pane-core/tests/calculator.rs`):
  - each colour form parsed and copied in each format;
  - colour names not answered;
  - each date and time word with the clock controlled;
  - each percentage form;
  - the Actions panel's copy items.
- [ ] Window test: the colour card draws a swatch, and its accessible name gives the colour's value.
- [ ] Prebuilt calculator artifacts are rebuilt.
- [ ] `docs/root-search.md` (the calculator's scope) is updated.
- [ ] CI: the branch tier is green, and a `ci-fast` verify run is green. No release matrix.

## Blocked by

None - can start immediately


=== #197 [OPEN] Rank root search with Raycast's comparator, find results by keywords and other names, and tell same-named rows apart assignees=
## Parent

https://github.com/hoangvu12/pane/issues/122

## What to build

Root search orders matches with the parent's comparator (the parent's "Ranking" section), finds results by their other names and keywords, and tells same-named rows apart (the parent's "Alternate titles, keywords and same-name disambiguation"). Learning's steps come with #199. Here they compare equal. Size M.

- **The comparator** steps 1, 2, 4, 5, 8, 10, 11, 12 and 13:
  - alias;
  - exact title or alternate title for queries over three characters;
  - exact subtitle;
  - alias prefix;
  - best text score;
  - title score;
  - kind priority (commands above links above applications above files);
  - provider order;
  - title with numeric collation ("Item 2" before "Item 10").
  
  Steps 3, 6, 7 and 9 (learned queries, frecency) are left as seams that #199 fills. It replaces `ranked_matches`' stable sort by `Rank` (`crates/pane-core/src/search.rs` ~47-55, ~233-247).
- **Sections and placement** are as the parent says. Computed results and file results keep their places, and only "Results" is ordered by the comparator (`launcher.rs` `root_rows` ~4571-4627, `launcher/presentation.rs` ~285-320, ~414).
- **Keywords:** a command may declare `keywords` in `pane.json` (`packages.rs` `CommandJson` ~672-705 → `ManifestCommand`). Keywords and indexed results' keywords (from #124) are matched and rank as subtitles, and alternate titles as titles.
- **Executable names** are already supplied by the applications provider as alternate titles, with #124's generic-name exclusion (`applications/names.rs`). Check that they are scored, and do not duplicate the exclusion.
- **Same-name rows:** when two listed rows have the same normalised title, each shows what tells it apart, per the parent's rule:
  - an installed command's package source after its subtitle;
  - an indexed result's own distinguishing subtitle (#124's `distinction`);
  - otherwise the supplying command's title.
- The extension API change is additive (a manifest field). Update the manifest reference.

## Acceptance criteria

- [ ] Core tests through `Launcher`:
  - each comparator step, in order, against a pair that differs only there;
  - kind priority;
  - numeric collation;
  - a command found by a manifest keyword;
  - an indexed result found by an alternate title and by a keyword (a sample indexed provider);
  - two copies of a package and two same-named applications showing their distinctions.
  
  Samples in Rust, JavaScript and TypeScript where a manifest field is read.
- [ ] Prebuilt sample artifacts are rebuilt if any sample changes.
- [ ] `docs/root-search.md` (ranking) and the manifest reference are updated.
- [ ] CI: the branch tier is green, and a `ci-fast` verify run is green. No release matrix.

## Blocked by

- #193


=== #198 [OPEN] Collections: install one, some or all of the extensions a Git repository lists assignees=
## Problem Statement

A Git-distributed package must keep its `pane.json` at the repository's root, so one repository is one extension. An author who keeps several extensions in one repository cannot publish them, and a user cannot install one of them. [ADR 0044](https://github.com/pane-app/pane/blob/main/docs/adr/0044-a-git-repository-holds-one-extension-or-a-collection.md) decides that a repository may also be a **collection**; this specifies it.

## Solution

- A repository or folder whose root holds `pane-collection.json` is a collection: it lists its extensions by **extension id** and folder, and may map renamed or removed ids.
- Installing a collection shows its extensions, to install some or all; `git:<repository>#<id>` installs one.
- Each extension installed from a collection is a package of its own (identity `git:<host>/<path>#<id>`, ADR 0012 as amended), with its own preview, managed copy, updates, disabled state, uninstall and data.
- Each extension of a collection is released on its own with tags `<id>/v<semver>`; Check for Update finds the newest one.
- Pane fetches only the chosen extensions' folders where the host allows it.

## User Stories

1. As an extension author, I want to publish several extensions from one repository, so that I need not keep a repository for each.
2. As an extension author, I want to list my collection's extensions in one file at its root, so that samples and test fixtures in my repository are never offered as extensions.
3. As an extension author, I want to move an extension's folder without breaking installs, so that I can reorganize my repository.
4. As an extension author, I want to rename an extension's id, or retire it, and have installed copies follow or be told, so that I can evolve my collection.
5. As an extension author, I want to release one extension of my collection without releasing the others, so that a fix to one never forces a version on the rest.
6. As a launcher user, I want to install a collection and choose which of its extensions I get, so that I take only what I use.
7. As a launcher user, I want to install one extension of a collection by its address and id, so that a link to it installs just that.
8. As a launcher user, I want each extension from a collection to appear, update, disable and uninstall on its own, so that a collection is never one block I must take or leave whole.
9. As a launcher user, I want each extension's install preview to show what it uses, as any extension's does, so that choosing several still shows me each one.
10. As a launcher user, I want one extension that cannot be installed to be explained while the others install, so that one fault does not block the rest.
11. As a launcher user, I want an extension that was removed from its collection to keep running and to be reported, so that it never disappears silently.
12. As a launcher user, I want installing from a large repository to download only what I install where the host allows, so that it stays fast.
13. As a launcher user, I want a local folder that is a collection to install the same way, so that I can try a collection before it is published.
14. As a contributor, I want collections tested against repositories served on 127.0.0.1, so that no check reaches a real Git host.

## Implementation Decisions

- **The index.** `pane-collection.json` at the root: `extensions` (each `{ "id", "path" }`) and optional `renamed` (`{ "<old id>": "<new id>" | null }`). Ids are lowercase letters, digits and hyphens, unique, and never reused for another extension. Paths are plain relative folders inside the revision (the same name checks as ADR 0021's tree entries; no `..`). A root holding both `pane.json` and `pane-collection.json`, an unknown field, a duplicate id or path, or a path naming no package is refused with what is wrong. The index holds no titles or versions; each package's manifest does.
- **Self-contained extensions.** Only the files under an extension's folder reach its managed copy; a component outside it is missing, and the package is explained as source-only.
- **Addresses.** `git:<repository>` naming a collection opens the choice; `git:<repository>#<id>` names one extension; a reference follows as ADR 0021 reads it (`git:github.com/owner/tools#clock@refs/tags/clock/v1.2.0`). A `#<id>` on a one-extension repository is refused. A local folder takes `#<id>` the same way.
- **The choice.** The install flow in Settings (ADR 0043) lists the collection's extensions with each package's icon, title, description and version, nothing ticked; then each one ticked goes through the ordinary install preview and install, in order. One that fails is explained and the others continue; the result lists what was installed and what was not.
- **Identity and records.** `git:<host>/<path>#<id>`, normalized as ADR 0021 normalizes the repository. `installed.json` records the id beside the Git source fields. Duplicate installs of the same identity are refused, as today.
- **Updates.** For an extension installed from a tag `<id>/v<semver>`, Check for Update and the updater list the repository's tags (`ls-refs` with the prefix `refs/tags/<id>/`), take the highest stable semver above the installed version, fetch that revision, check that its index still lists the id (following `renamed`), and pin the commit. A tracked branch is fetched again (ADR 0021). An id renamed to another id is followed and the identity is kept; an id gone or renamed to `null` is reported in the update results (#127) and the installed copy keeps running.
- **Fetching.** When the server advertises `filter`, fetch the commit with `filter blob:none`, walk the trees to the chosen folders, and fetch those blobs; otherwise fetch the whole revision under ADR 0021's limits. Every object is still checked against its id. One fetch serves every extension chosen from the same revision.
- **Development mode** on a collection folder develops one extension of it, chosen by id.

## Testing Decisions

- Core tests with repositories made by `git` and served from 127.0.0.1 (as `repositories.rs` does): install one by id, install several, a bad index (each refusal), a missing path, a component outside the folder, a moved folder keeping identity, a renamed id followed on update, a removed id reported, prefixed-tag updates choosing the right version among other extensions' tags, and a server without `filter` falling back to a whole revision.
- A window test of the choice list and the per-extension previews.
- No test reaches a real Git host.

## Out of Scope

- Collections on npm (one npm package stays one extension).
- A catalog or gallery of collections.
- Moving the official extensions into their repositories ([ADR 0045](https://github.com/pane-app/pane/blob/main/docs/adr/0045-official-extensions-live-in-their-own-repositories.md)) and extension API versioning ([ADR 0046](https://github.com/pane-app/pane/blob/main/docs/adr/0046-every-breaking-change-to-the-extension-api-gets-a-new-version.md)), which get specifications of their own.

## Further Notes

The research behind ADR 0044 compared Claude Code plugin marketplaces, Home Assistant add-on repositories, pre-commit, Zed, Go modules, Helm, Pi, GitHub Actions, Neovim plugin managers, Obsidian and HACS. Proposed details in ADR 0044 (the address form, independent installs of a choice, subtree fetching, tag-based updates) are the defaults this specification takes and may change in review.




=== #199 [OPEN] Learn from what the user chooses: frecency and learned queries in ranking, and the blank query ordered by frecency assignees=
## Parent

https://github.com/hoangvu12/pane/issues/122

## What to build

Root search learns from what the user chooses (ADR 0030; on [PR #119's branch](https://github.com/hoangvu12/pane/blob/research/raycast-deep-dive/docs/adr/0030-root-search-learns-from-what-the-user-chooses.md) until it lands), and the blank query shows the pins, then commands and applications by frecency. This follows the parent's "Learning from choices (ADR 0030)" and "Ranking" sections. The user's controls for it are #200. Size L.

- **A use** is recorded when the user invokes a root result from root search and its action was dispatched, with the query then in the field, normalised as matching normalises it.
  - Not recorded: global-hotkey opens, computed answers and other computed results, file results, fallback rows, Pane's own install and management rows.
  - A quick slot invoked from the pinned home records a use with no query (proposed).
- **Frecency:** a ten-day half-life, a floor of 1, and a use adds 1 to the decayed score. It uses the launcher's clock (`Launcher::with_clock`, `ManualClock` in `crates/pane-core/src/clipboard.rs` ~514-580).
- **Learned queries:** the last three distinct, newest first. They count only while frecency is above 1 and the result was last opened within 17 days.
- **The comparator's** steps 3, 6, 7 and 9 and the no-query order are filled in, through the seams #197 left.
- **The blank query:** the pinned home first (ADR 0027), then commands and applications in the no-query order, under "Commands", with no Suggestions section. Quick slots get no boost while typing.
- **Identity:** a command by its command id, an indexed result by its id under its command, and applications by #124's identity (`applications/identity.rs`). Never a title or a position.
- **The record:** Pane's own, through `launcher/choices.rs` (`Choices`, `Record`, `save`) like aliases:
  - versioned, atomic and unreadable-never-overwritten;
  - uninstall forgets a package's entries, and disable keeps them;
  - entries decayed to 1 and older than 17 days are dropped.
- Visits that change no visible order do not re-sort the list on screen.
- **Docs:** `docs/root-search.md` (sections no longer promise no recent use), and the glossary's Result section and Pinned home entries plus a "Learned ranking" term, through `/domain-modeling`.

## Acceptance criteria

- [ ] Core tests through `Launcher` with the manual clock:
  - uses recorded and not recorded per the rules;
  - learned queries winning;
  - the 17-day and frecency gates;
  - decay over simulated days;
  - uninstall forgetting and disable keeping;
  - survival across a restart over the same data folder;
  - an unreadable record reported and not replaced;
  - an application keeping its ranking across an update into a new version folder (a fixture).
- [ ] Core test: the blank query lists the pins, then commands and applications by frecency under "Commands".
- [ ] Window test with real key events: choosing the second of two equal results a few times puts it first for that query.
- [ ] Docs and glossary updated as above.
- [ ] CI: the branch tier is green, and a `ci-fast` verify run is green. No release matrix.

## Blocked by

- #197


=== #200 [OPEN] Let the user reset what root search learned, per result and for all, and turn learning off assignees=
## Parent

https://github.com/hoangvu12/pane/issues/122

## What to build

The user controls what root search learned, as the parent's "Learning from choices", "Controls" bullet and its "Settings and records" section describe. Size S-M.

- **"Reset Ranking"** in the Actions panel of every root result that can be learned (no default key). It clears that result's frecency and learned queries and says "Ranking reset for <title>". Add it as a result action (`launcher/actions.rs` `ResultAction`) or a Pane-performed action (`launcher/own_actions.rs`), whichever matches how similar root-search actions are added.
- **Settings' Launcher page:**
  - "Reset ranking…", with a confirmation;
  - a "Learn from what I choose" switch, on by default, registered with Settings search. Turned off, no uses are recorded and ranking acts as if none were (what was learned is kept until reset). The same switch also stops search history, which #206 adds.
- The switch lives in Pane's host settings, with the page's existing commit and rollback path.

## Acceptance criteria

- [ ] Core tests through `Launcher`: Reset Ranking clears one result only; reset-all clears everything; with the switch off, nothing is recorded and the order ignores what was learned; turning it on again uses what was kept.
- [ ] Window tests with real key events: Reset Ranking from the Actions panel and its toast; "Reset ranking…" with its confirmation; the switch found through Settings search.
- [ ] `docs/root-search.md` and the Settings documentation are updated.
- [ ] CI: the branch tier is green, and a `ci-fast` verify run is green. No release matrix.

## Blocked by

- #199


=== #201 [OPEN] Publish a query's list when its providers answered or 200 ms passed, and merge late answers without flicker assignees=
## Parent

https://github.com/hoangvu12/pane/issues/122

## What to build

A query's list is published once its providers answered or 200 ms passed, and late answers are merged without flicker. This follows the parent's "Publishing a query's list, provider budgets, and keys that wait" section, except the keys, which are #203. Size M.

- **Publishing:**
  - ranking command metadata and kept indexed results stays synchronous;
  - for computed results, the launcher publishes the new query's list when every provider asked for it has answered, or 200 ms after the query changed, whichever comes first;
  - until then, the window keeps the previous list while the field shows what was typed.
  
  Today each provider's answer calls `relist_root` (`launcher.rs` `show_one_root_result` ~2251-2297, `relist_root` ~4632-4645), and there is no notion of a published list.
- **Late answers** after the budget are merged into the published list, coalesced within 16 ms. If the first row was selected and a merge puts another row first, the selection moves to it. Otherwise it stays on its row by id. A late computed answer never takes the selection from a row the user moved to. This includes a preselected fallback (#194) giving way to a late result.
- **Stale answers** are discarded as today (`search_epoch`).
- **The launcher exposes** whether the current query's list is published, for #203.
- Calls still run one at a time on the runtime. The budget bounds the wait, not the scheduling.

## Acceptance criteria

- [ ] Core tests through `Launcher` with the slow fixture (`guests/fixtures/faulty`, the "0 + 0" query; prior art: `crates/pane-core/tests/calculator.rs`, `root_providers.rs`), using the clock where the budget is timed:
  - a slow provider holds the list up to the budget, then is merged late;
  - the selection rule for late merges, both with the first row selected and with a moved selection;
  - a preselected fallback giving way;
  - stale answers discarded;
  - "published" reported correctly.
- [ ] Window test: the list does not flicker through intermediate states while providers answer within the budget.
- [ ] `docs/root-search.md` describes publishing and budgets.
- [ ] CI: the branch tier is green, and a `ci-fast` verify run is green. No release matrix.

## Blocked by

- #194


=== #202 [OPEN] Stop paying a guest instance per keystroke, a full re-rank per answer and an applications list per show in root search assignees=
## Parent

https://github.com/hoangvu12/pane/issues/122

## What to build

Typing in root search no longer costs a guest instance per keystroke, a full re-rank per provider answer, or a full applications list after every show. What the user sees is unchanged. The work comes from a code read on 104a5dac (the idle-cost specification #188 leaves root search's per-keystroke cost to this specification). Size M.

What the read found:

- **A superseded call drops its instance.** Each keystroke cancels the pending root-results call (`launcher.rs` `until_cancelled` ~4734-4746 → `runtime.rs` `root_results` ~3658, `run_guest_until` ~3924). A cancelled call drops the guest instance (`Halt::Cancelled`, ~4043-4047), so the next keystroke instantiates the component again (`start_instance` ~4293). Wasmtime cannot cancel a call and keep its instance (`docs/command-search.md`).
- **Every provider answer re-ranks every root row.** `relist_root` → `root_rows` (`launcher.rs` ~4571-4645) re-ranks commands and indexed results and clones each row, once per answer.
- **Indexed results are asked again after every show.** `show_root` marks every index stale (`launcher.rs` ~3206, `launcher/indexed.rs` ~49-53), so the first query after each show re-asks every indexed provider, and the applications list crosses the wasm boundary whole (`indexed.rs` ~114-135). The applications provider already has change notifications (`application_changes`).

What to build (direction; choose with measurements):

- Ask computed providers after a short quiet period (proposed: 30-50 ms after the last keystroke, well inside #201's 200 ms budget), so a burst of keystrokes asks once. Cancel a call that has not answered within the budget only when a newer query needs the runtime.
- Merge a provider's answer into the published list without re-ranking the rows that did not change: rank the static rows once per query, then insert the computed sections.
- Re-ask an indexed provider only when its data may have changed: for applications, on its change notifications. Providers without notifications keep today's once-per-show refresh.
- No visible behaviour changes: #201's publishing and selection rules hold, and stale answers are never shown.

## Acceptance criteria

- [ ] A core test (with a test hook or counter) types a ten-character query one key at a time at typing speed, and the calculator's component is instantiated at most twice. The final list is the same as before.
- [ ] A core test: a provider's answer merges without re-ranking the static rows (a counter), and the list equals a full re-rank.
- [ ] A core test: showing and hiding root search repeatedly with no application change does not re-ask the applications provider. An application change does.
- [ ] The publishing, slow-fixture and stale-answer tests of #201 stay green unchanged.
- [ ] `docs/root-search.md` describes when providers are asked.
- [ ] CI: the branch tier is green, and a `ci-fast` verify run is green. No release matrix.

## Blocked by

- #201


=== #203 [OPEN] Hold Enter, Tab, Ctrl+K, Ctrl+digit and action shortcuts until the current query's list is published assignees=
## Parent

https://github.com/hoangvu12/pane/issues/122

## What to build

Keys pressed right after typing act on the list for what was typed. This follows the parent's "Keys that wait" bullet. Size M.

- While the list for the exact current query is not yet published (#201 exposes this), these keys are held for at most 300 ms, then applied to the selection at that moment:
  - Enter;
  - Tab and Shift+Tab;
  - Ctrl+K;
  - Ctrl+digit;
  - the selected row's action shortcuts;
  - Space while the query could still be an alias.
- Characters typed meanwhile are applied in order. An auto-repeated Enter never runs a second action.
- The window owns holding and replaying. The launcher only reports "published".
- Where keys are handled today:
  - Enter: `app.rs` `confirm` (~481), wired at ~1954-1973;
  - Tab: `FocusNext`, `lib.rs` ~68;
  - Ctrl+K: `features/actions_panel.rs` ~499;
  - Ctrl+digit: `features/quick_slots.rs` ~460;
  - the query field: `features/root_search/mod.rs` ~80-115.

## Acceptance criteria

- [ ] Window tests with real key events and the slow fixture (prior art: `crates/pane/tests/window.rs`, `command_search.rs`):
  - typing and pressing Enter at once runs the published first row, not the previous list's;
  - Ctrl+K, Ctrl+digit and Tab wait the same way;
  - characters typed during a hold land in order ("ec hello");
  - a held key is applied after at most 300 ms when a provider never answers;
  - an auto-repeated Enter runs once.
- [ ] `docs/root-search.md` describes the held keys.
- [ ] CI: the branch tier is green, and a `ci-fast` verify run is green. No release matrix.

## Blocked by

- #201


=== #204 [OPEN] List a typed folder's entries in root search and browse them with Tab and Shift+Tab assignees=
## Parent

https://github.com/hoangvu12/pane/issues/122

## What to build

A typed folder path ending in a separator lists that folder's entries, browsed with Tab and Shift+Tab. This follows the parent's "Typed-folder listing" bullet. Size M.

- **A new host capability** lists a folder the user typed. No grant is needed, since the user named it, and ADR 0034's file access covers it (no new ADR). Its bounds are like the scan policy's:
  - direct entries only;
  - name order, folders first;
  - at most 500 entries, with a partial listing saying so.
  
  It is separate from the granted-folder `list-folder` (`wit/files.wit` ~73, ADR 0017), which stays unchanged for other packages. It is offered in Rust, JavaScript and TypeScript.
- **The Files extension** lists the typed folder's entries as file results for a path-like query naming an existing folder and ending in a separator.
- **The window:**
  - Tab on a selected folder row completes the query to that folder's path with a trailing separator;
  - Shift+Tab removes the last path component;
  - Enter opens the selected entry with the system's handler, under #126's program rule.
  
  Tab here takes precedence over focusing argument fields only when the row is a typed-folder entry.

## Acceptance criteria

- [ ] Core tests through `Launcher` with a fixture folder (prior art: `crates/pane-core/tests/files.rs`):
  - entries listed folders first, in name order;
  - 500 entries plus a partial notice;
  - a missing folder listing nothing;
  - `~` resolved;
  - the capability from each language.
- [ ] Window tests with real key events: Tab completes to a folder; Shift+Tab goes up; Enter opens through the recording opener (a program revealed, not run).
- [ ] Prebuilt artifacts for changed guests and samples are rebuilt.
- [ ] `docs/files.md` and `docs/root-search.md` are updated.
- [ ] CI: the branch tier is green, and a `ci-fast` verify run is green. No release matrix.

## Blocked by

- #195


=== #205 [OPEN] Show inline argument fields after the query, and jump into them with an alias and a space assignees=
## Parent

https://github.com/hoangvu12/pane/issues/122

## What to build

A command that declares arguments shows up to three inline fields after the query when it is selected, and an alias followed by a space jumps into them. This follows the parent's "Inline arguments" section. Size L.

- **Built on #120's arguments:** `crates/pane-core/src/arguments.rs` (`ManifestArgument`, `MAX_ARGUMENTS`, `fill`, `first_missing`) and the launch record (`launch.rs`). Today root search asks for missing required values through the arguments form (`launcher/argument_form.rs`, whose doc names #122's inline fields). In root search the fields replace the form. The form stays wherever a command is invoked outside root search (quick slots, hotkeys) with required values missing.
- **Fields:**
  - shown for the selected row only;
  - text, password (masked) and dropdown (leading empty choice, then its options);
  - each sized to its value or placeholder;
  - an "optional" marker on optional fields;
  - the query field's placeholder becomes the command's title.
- **Keys:**
  - Tab, Shift+Tab, and Left and Right at the field edges move between the query and the fields;
  - Up and Down do nothing inside a field;
  - Escape returns to the query with its text selected.
- **Enter:**
  - runs the primary action with the values;
  - with a required field blank, moves focus to it instead;
  - a required field is marked missing only after it was left blank once.
  
  Ctrl+digit and Tab-as-confirm on a row with blank required fields focus them.
- **Values survive re-ranking** for arguments with the same name and type, and a dropdown's value while it is still a choice.
- **Remembered dropdown choices:** Pane's own record (`launcher/choices.rs`), per command, dropped when no longer a choice.
- **Alias and space:**
  - for a command with arguments, focus moves to its first empty field, carrying the text typed after the space (including text typed before the list was published, through #203's hold). Alias and Tab does the same;
  - for a command without arguments, it opens at once: a view command opens with the text after the space in its search field, and a no-view command runs;
  - a command that accepts fallback text but declares no arguments keeps today's alias-plus-text row (`launcher/aliases.rs` `rows_sending_after_alias` ~259-274).
- **Accessibility:** each field is a labelled editable node inside the search combo box's group, with its required or optional state. A password field exposes no value.
- **Docs:** `docs/root-search.md`, `docs/aliases.md`, the arguments documentation, and the glossary's Alias and Query-taking command entries plus an argument-field term, through `/domain-modeling`, reconciled with #120's "argument".

## Acceptance criteria

- [ ] Core tests through `Launcher` (prior art: `crates/pane-core/tests/arguments.rs`, `aliases.rs`):
  - values passed to the command;
  - the required-field refusal;
  - remembered dropdown choices kept across a restart and dropped when gone;
  - values surviving a re-rank;
  - a fallback's query filling the first text argument;
  - alias and space opening a command without arguments at once;
  - a fallback-text command keeping its row.
- [ ] Window tests with real key events (prior art: `crates/pane/tests/arguments.rs`, `aliases.rs`):
  - Tab and arrows into and between fields;
  - Enter with a blank required field;
  - the missing mark only after leaving;
  - password masking;
  - "tr hello" landing in the first field;
  - the fields' accessibility nodes.
- [ ] Docs and glossary updated as above.
- [ ] CI: the branch tier is green, and a `ci-fast` verify run is green. No release matrix.

## Blocked by

- #203


=== #206 [OPEN] Recall recent queries with Up on an empty query assignees=
## Parent

https://github.com/hoangvu12/pane/issues/122

## What to build

Up on an empty query recalls recent queries, with their argument values. This follows the parent's "Recent queries" section. Size S-M.

- **Up** recalls the most recent query and its argument values when:
  - the query is empty;
  - the first row, or no row, is selected;
  - the press is not an auto-repeat.
  
  While the restored query is unchanged, Up again goes further back. Any other key ends the walk. Otherwise Up moves the selection as today.
- **Recorded** when a non-blank query is cleared: by Escape, by a clear, by returning to root, or by the launcher closing with its search cleared.
  - Consecutive duplicates are not added.
  - At most 64 entries are kept.
  - Password values are recorded as empty.
- **The record:** Pane's own, through `launcher/choices.rs`, on this computer only.
- **Settings' Launcher page** gains "Reset search history", registered with Settings search. #200's "Learn from what I choose" switch also stops recording history.

## Acceptance criteria

- [ ] Core tests through `Launcher`:
  - recording rules (clearing, duplicates, the 64 cap, passwords blank);
  - the switch stopping recording;
  - reset;
  - survival across a restart.
- [ ] Window tests with real key events: Up restores the previous query with its argument values; repeated Up walks back; typing ends the walk; Up with another row selected or a typed query moves the selection.
- [ ] `docs/root-search.md` and the Settings documentation are updated.
- [ ] CI: the branch tier is green, and a `ci-fast` verify run is green. No release matrix.

## Blocked by

- #200
- #205


