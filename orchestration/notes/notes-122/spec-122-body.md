## Problem Statement

Pane's root search finds a result only when every word of the query appears in its title, subtitle or package title, and ranks matches by a fixed six-step order. That is predictable, but it is not how people search a launcher they use a hundred times a day:

- Abbreviations and loose spellings find nothing. "vsc" does not find Visual Studio Code, "clhis" does not find Clipboard History, and "cafe" does not find "Café". A Vietnamese user typing without diacritics ("tieng viet", "duong") finds none of the titles written with them.
- Root search never learns. A user who types "c" every morning and opens Calculator must still arrow past whatever ranks first, every time. Two results that match equally well stay in install order forever, however often the user picks the second.
- When nothing matches, the fallbacks are listed but nothing is selected, so Enter does nothing. The user who typed a search phrase meant for a web-search fallback has to press Down first, every time.
- Enter can act on a stale list. A user who types quickly and presses Enter can run the row that was first before a provider's answer arrived; a moment later the calculator's answer or a newly listed result takes the top.
- An installed application is found only by its Start-menu name. "code" does not find the editor whose program is `code.exe`, and two applications with the same name are two identical rows.
- Root search does not understand what was typed. A web address, a bare domain such as "github.com", a folder path, a colour such as "#FF6B35", "now", or "20% off 80" produce no useful row, although each has an obvious meaning.
- A command that needs input gets one free-text string at most (a query-taking command through its alias or as a fallback). It cannot ask for two values, a choice from a list, or a secret, and the user cannot see what it expects before invoking it.
- The user cannot get back a query they typed a minute ago; it is gone once the field is cleared.
- A slow provider decides how long the list stays unsettled, and nothing bounds how long the list waits for it.

Raycast, which Pane learns from, has solved each of these in a mature product, and its Windows build shows exactly how (see `docs/research/raycast-deep-dive.md`).

## Solution

Root search becomes Raycast's root search, adapted to Pane's vocabulary and small core:

- **Fuzzy matching with a sensitivity threshold.** The core scores how well the query's letters fall into a result's title, alternate titles, subtitle and keywords: letters at the very start and at word starts score more, gaps cost. A Search sensitivity setting (Low, Medium, High) decides how good a score must be to count as a match. Text is transliterated before comparison, so accents, Vietnamese diacritics and similar marks never stand between the user and a result.
- **Root search learns from what the user chooses** (the user's decision 3, recorded in ADR 0030). Each root result the user invokes earns frecency, a score that decays with a ten-day half-life, and Pane remembers the last few queries the user typed before choosing it. The next time the user types such a query, that result comes first. The user can reset what was learned for one result or for all, and turn learning off.
- **The blank query shows the pins, then commands and applications ordered by frecency** (the user's decision), so what the user opens most is at the top before anything is typed. There is no Suggestions section.
- **When nothing matches, the first fallback is selected** (the user's decision 4, recorded in ADR 0031). Enter then sends the query to it.
- **Enter acts on the list for what was typed.** Each search provider has a short time budget; the list for a query is published once its providers answered or the budget ran out, and late answers are merged in. Enter, Tab, Ctrl+K, Ctrl+digit and a row's action shortcuts typed before the list for the current query is published wait for it, briefly, then act on what it selected.
- **Results can be found by their other names, and same-named results are told apart.** A result may carry alternate titles (an application's program name, which the "Applications done properly" specification (#124) produces) and keywords; root search scores them and shows what distinguishes same-named rows.
- **Root search understands typed addresses, paths, colours, the time and percentages.** URLs and bare domains offer to open them; a typed path opens it, and a folder path lists its entries, browsed with Tab and Shift+Tab; colours show a swatch and copy in several formats; "now", "today" and "time" answer at once; "52% of 900" and "20% off 80" are calculated.
- **Inline argument fields.** A command that declares arguments (at most three: text, password, dropdown) shows them right after the query when it is selected. Tab moves into them, Enter runs the command with their values, and typing its alias and a space jumps straight into the first. Typing the alias and a space of a command without arguments opens it at once, as Raycast does.
- **Up recalls recent queries.** With an empty query and the first row selected, Up restores the previous queries, with their argument values, one at a time.

## User Stories

### Matching

1. As a launcher user, I want "vsc" to find Visual Studio Code, so that I can open what I mean with a few letters.
2. As a launcher user, I want letters that start the title or a word of it to count more than letters in the middle of a word, so that "cb" finds Clipboard before a title that merely contains a c and a b.
3. As a launcher user, I want a query whose letters are scattered through a long title to rank below a tight match, so that loose matches do not crowd out good ones.
4. As a launcher user, I want to choose how strict matching is (Low, Medium, High), so that I can trade more results for fewer surprises.
5. As a launcher user, I want "cafe" to find "Café" and "Café" to find "Cafe", so that accents never decide whether I find something.
6. As a Vietnamese-speaking user, I want "tieng viet" to find "Tiếng Việt" and "duong" to find "Đường", so that I can type without diacritics.
7. As a user of other scripts, I want titles in scripts Pane's transliteration covers to be found by their romanisation, so that I can search them from a Latin keyboard.
8. As a launcher user, I want a query to match across a result's title and subtitle together ("utub vid" finding "Search YouTube Videos"), so that I can type the words I remember from either.
9. As a launcher user, I want the letters the query matched to be highlighted in the title, so that I can see why a result is listed.
10. As a launcher user, I want an exact title match to win when I typed more than a few characters, so that typing a full name always gives me that name first.
11. As a launcher user, I want a command found by its alias to stay first, above everything else, so that the aliases I set keep their meaning.
12. As a launcher user, I want a command whose alias starts with what I typed to rank high, so that a half-typed alias already finds it.
13. As a launcher user, I want commands to rank above applications, and applications above files, when they match equally well, so that ties resolve the way I usually mean them.

### Learning from choices

14. As a launcher user, I want the result I open most often for a short query to come first for that query, so that "c" opens Calculator after I chose it a few times.
15. As a launcher user, I want root search to remember the last few queries I typed before opening each result, so that my own abbreviations work even when they match poorly.
16. As a launcher user, I want what I used recently and often to rank above what I used once long ago, so that the ranking follows my habits as they change.
17. As a launcher user, I want an old habit to fade, so that a result I stopped using stops taking the top.
18. As a launcher user, I want opening a command with its own global hotkey not to change its ranking in root search, so that ranking reflects what I search for.
19. As a launcher user, I want copying a calculator answer or opening a file not to be learned as a habit, so that one-off rows do not distort my ranking.
20. As a launcher user, I want to reset what root search learned about one result from its Actions panel, so that I can undo a ranking I do not like.
21. As a launcher user, I want to reset everything root search learned from Settings, so that I can start again.
22. As a privacy-minded user, I want to turn learning off, so that Pane records nothing about what I choose.
23. As a privacy-minded user, I want what Pane learns to stay on this computer and never be sent anywhere, so that my habits stay mine.
24. As a user, I want what Pane learned about an extension's results to be forgotten when I uninstall it, and kept when I only disable it, so that it behaves like my aliases and hotkeys.
25. As a user, I want an application to keep its ranking after it updates itself into a new version folder, so that updates do not reset my habits.
26. As a launcher user, I want my pinned quick slots not to jump to the top of a typed query by being pinned, so that typed results stay about what I typed.

### Fallbacks and the moment I press Enter

27. As a launcher user, I want the first fallback selected when nothing else matches, so that typing a phrase and pressing Enter sends it to my first fallback.
28. As a launcher user, I want the no-results notice to stay above the fallbacks, so that I still see that nothing matched.
29. As a launcher user, I want the fallbacks to stay unselected below real results when something does match, so that Enter still opens the best match.
30. As a launcher user, I want Enter pressed right after typing to act on the results for what I typed, not on the previous list, so that fast typing never opens the wrong thing.
31. As a launcher user, I want Ctrl+K, Ctrl+digit, Tab and an action's own shortcut to wait for the current results the same way, so that every key acts on what I see.
32. As a launcher user, I want that wait to be short and bounded, so that a slow extension never makes Enter feel stuck.
33. As a launcher user, I want text I type while a key is waiting to keep its order, so that "ec hello" typed quickly still lands as typed.
34. As a launcher user, I want the list not to flicker as each provider answers, so that the row under my eye stays put.
35. As a launcher user, I want a provider's late answer to be added once it arrives, keeping my selection if I moved it, so that slow results are not lost and do not steal my place.

### Other names, same names

36. As a launcher user, I want to find an application by its program name ("code", "msedge"), so that I can use the name I know.
37. As a launcher user, I want a generic program name such as "setup" or "launcher" not to find every application that has it, so that program names help rather than clutter.
38. As an extension author, I want my indexed results to carry alternate titles and keywords, so that users find them by the words they use.
39. As an extension author, I want my commands to declare keywords, so that a user typing a related word ("rm" for Uninstall) finds them.
40. As a launcher user, I want two results with the same title to show what tells them apart (their source, their program, their folder), so that I pick the right one.

### Understanding what I typed

41. As a launcher user, I want typing a web address to offer opening it in my browser, so that root search doubles as an address bar.
42. As a launcher user, I want a bare domain such as "github.com" to be understood as a web address, so that I need not type "https://".
43. As a launcher user, I want a typed web address to offer creating a quicklink for it, so that I can save it in one step.
44. As a launcher user, I want typing a file or folder path to offer opening it, so that I can reach any place on disk directly.
45. As a launcher user, I want a folder path ending in a separator to list that folder's entries, so that I can browse from the search field.
46. As a keyboard user, I want Tab on a listed folder to complete the query to it and Shift+Tab to go up a level, so that I can browse without leaving the keyboard.
47. As a launcher user, I want "~" to mean my home folder in a typed path, so that paths I know from terminals work.
48. As a designer, I want typing "#FF6B35", "rgb(255, 107, 53)" or "hsl(18, 100%, 60%)" to show a swatch, so that I can check a colour instantly.
49. As a designer, I want to copy a typed colour as hex, RGB, HSL and other formats, so that I can convert between them without a tool.
50. As a launcher user, I want "now", "today", "time", "tomorrow" and "yesterday" to answer with the date or time, so that I can copy them quickly.
51. As a launcher user, I want "52% of 900", "20% off 80", "15% on 40" and "80 + 20%" to be calculated, so that everyday percentages need no calculator app.
52. As an extension author, I want my command to appear only for typed addresses or only for typed paths, receiving the parsed address or path, so that I can build URL and path tools without matching titles.

### Inline arguments

53. As a launcher user, I want a command that needs input to show its fields right after my query when I select it, so that I see what it expects before running it.
54. As a launcher user, I want Tab to move into the first empty field and Shift+Tab into the last, so that filling them is quick.
55. As a launcher user, I want Tab, Shift+Tab, and Left and Right at the field's edges to move between the query and the fields, so that I can correct either.
56. As a launcher user, I want Enter with a required field empty to move me to it instead of running, so that I am never sent a half-filled command.
57. As a launcher user, I want a field I left empty to be marked only after I left it, so that I am not scolded before typing.
58. As a launcher user, I want a password field to hide what I type, so that secrets stay private on screen.
59. As a launcher user, I want a dropdown field to remember my last choice for that command, so that I rarely change it.
60. As a launcher user, I want an optional field marked as optional, so that I know I may leave it.
61. As a launcher user, I want typing a command's alias and a space to put me in its first field, carrying what I type next into it, so that "tr hello" fills a translate command in one motion.
62. As a launcher user, I want the search field's placeholder to show the selected command's title while its fields show, so that I know what I am filling in.
63. As a launcher user, I want what I typed into the fields to survive the list being re-ranked, so that late results do not erase my input.
64. As a launcher user, I want a fallback with arguments to receive my query in its first text field, so that fallbacks keep working with commands that ask for more.
65. As a screen-reader user, I want each argument field announced with its name and whether it is required, so that I can fill them without seeing them.

### Recent queries

66. As a launcher user, I want Up on an empty query to restore my previous query, so that I can repeat a search.
67. As a launcher user, I want repeated Up to walk further back through my recent queries, so that I can reach one from a while ago.
68. As a launcher user, I want a restored query to bring back its argument values except passwords, so that repeating a command is one key.
69. As a privacy-minded user, I want to clear my search history from Settings, so that past queries are gone.
70. As a launcher user, I want Up to keep moving the selection when a row other than the first is selected or I have typed something, so that the history never hijacks navigation.

### The blank query and opening by alias

71. As a launcher user, I want the blank query to show my pins, then the commands and applications I open most, ordered by frecency, so that what I use daily is on screen before I type.
72. As a launcher user, I want no separate Suggestions section on the blank query, so that the list stays one list under my pins.
73. As a launcher user, I want typing a command's alias and a space to open a command without arguments at once, as Raycast does, so that an alias is as quick as a hotkey.

## Implementation Decisions

### Scope and decision provenance

- **The user's decisions (2026-10-06).** Root search learns from what the user chooses, with Raycast's learned queries (the last queries used per result) and frecency with decay; this reverses Pane's earlier avoidance of recent-use ranking (decision 3, ADR 0030). When nothing matches, the first fallback is preselected, matching Raycast (decision 4, ADR 0031). The general direction: take as many of Raycast's behaviours as possible, and where in doubt choose the more flexible option for extension authors.
- **Scope set by the user's list of specifications:** fuzzy scoring (start, word-start, elsewhere, gap costs) and a sensitivity threshold setting; learned queries and frecency; fallback preselection; Enter waiting briefly for the current query's results; transliteration (accents, Vietnamese); executable names as alternate titles; same-name disambiguation; URLs and bare domains, file paths (Tab to browse), colours, now/time and percentages; inline argument fields (at most three) after the query with Tab, alias and space filling the first; Up recalling recent queries; a per-provider time budget with late results merged.
- **The user's decision on the blank query (2026-10-06):** the blank query shows the pins, then commands and applications ordered by frecency (ADR 0030 left this to this specification); no Suggestions section.
- **Proposed defaults the user delegated ("like Raycast, as flexible as possible"):** Search sensitivity Low, Medium and High, defaulting to High (Raycast's stored default); learning with a "Reset Ranking" action per result, a reset-all button and an off switch in Settings, the off switch also covering query history; alias and space on a command without arguments opening it at once, like Raycast; the URL and path rows provided by default extensions (the core stays small), recognised by the core's query classification; typed folder listing (Tab to browse) covered by ADR 0034's file access, with no new ADR.
- **Coordination call (made by the coordinator):** alternate titles and same-name disambiguators are produced by the "Applications done properly" specification (#124) (fields on applications and indexed results); this specification scores and displays them. Neither duplicates the other's work.
- **Proposed defaults.** Every number below (scores, thresholds, half-life, the three learned queries, the 17-day window, budgets of 200 and 300 ms, 64 history entries) is Raycast for Windows 2.6's measured behaviour, proposed as Pane's default; so are the place of the new rows and the parsers' exact grammar. They are not separately confirmed by the user, and are marked where a choice was made between alternatives.
- **What this builds on and supersedes.** ADR 0030 (root search learns from what the user chooses) reverses #23's omission of frequency, recency and per-user ranking and #94/#100's stance that root search claims no recent use; it fixes the shape of learning (learned queries and decaying frecency, keyed by identity as quick slots are under ADR 0026, weighed in Raycast's order, kept as Pane's own record) and leaves the values, the blank query and the forgetting controls to this specification, which settles them as above. ADR 0031 (the first fallback is preselected when nothing else matches) supersedes #31's "a fallback is never selected by itself". Together they change the glossary and the root search documentation: a result section no longer promises that nothing reflects recent use (sections still only label the list; the Results section's order now reflects learning); a fallback and the no-results notice no longer promise that nothing is selected. This specification also does what #23's deliberately-not-done list left out (abbreviation matching, accent folding, manifest keywords), except locale-aware case folding and true typo tolerance (edit distance), which stay out. As ADR 0030 says, where computed results and file results are placed (#27, #29) does not change. ADR 0006 already left the ranking algorithm to implementation; the core still owns matching, ranking and aggregation, and no extension learns or ranks anything.
- **Dependencies on sibling specifications.** Argument declarations, the launch record (arguments, fallback text, launch type) and several actions per result (where Reset Ranking and colour formats live) are the "Extension commands like Raycast" specification (#120)'s contract; this specification shows and fills arguments in root search and adds root-search actions to the Actions panel. Application identity by resolved target, the source of executable names, the generic-name exclusion list, application disambiguators and the indexed-results fields that carry alternate titles and keywords are the "Applications done properly" specification (#124)'s (coordinator's call); this specification scores and displays them, through the matching and disambiguation mechanisms below. The home-folder file index is the "Instant file search" specification (#126)'s (ADR 0034); typed-path browsing here lists only the folder the user typed. Narrator announcement of the selected row is the "Quick fixes" specification (#129)'s.

### Matching (the core's search module)

- **Normalisation.** Both the query and every matched text are normalised the same way: Unicode NFC, transliteration to a Latin, accent-free form through a bundled transliteration table (é to e, đ and Đ to d, ß to ss, ligatures split; scripts the table covers romanised), lowercase, and runs of whitespace collapsed. Each result's normalised texts are computed once when root search's results are built, not per keystroke, as today. Aliases keep their own caseless comparison (full Unicode case folding, not transliterated), so recorded aliases mean exactly what they meant.
- **The scorer.** A result text and a query are compared as a subsequence with a score. Separators are whitespace and `- . / ( ) [ ]`. A query character matched at the text's first position scores 4; at a word start (after a separator) 3; elsewhere 2; a separator matched to a separator 1. Each gap between two consecutive matched positions costs 1; adjacency is free. A query separator that cannot be placed is skipped (counted, not failing); a letter that cannot be placed means no match. The best placement's score counts. An exact normalised equality is its own outcome. Capital letters inside a word are not word starts in root search. For a query longer than two characters, a quick in-order check rejects texts that cannot contain it before scoring.
- **Search sensitivity.** A threshold on the score, with n the number of query characters not skipped: Low accepts any subsequence match; Medium requires a score of at least 1.5·(n−2)+4; High requires a score above 2n. It is a Launcher-page setting, recorded in Pane's host settings, applied on the next keystroke. **Default: High** (proposed default, delegated), which Raycast's fresh installation stores (its manual names Medium; its code chooses High).
- **What is matched.** A result matches if its alias matches (exactly or as a prefix of the alias), or if one of these passes the threshold: its title, each alternate title, its subtitle, each keyword, and the composites "title subtitle" and "subtitle title" (so a query can span both). An installed command is also matched by its package's title, as today, ranked with its subtitle. A query starting with "/" treats the first "/" in titles as a space.
- **Highlighting.** The row highlights in the accent the title characters of the best placement when the title matched; a result found by an alternate title, keyword or subtitle highlights nothing in its title.

### Ranking

- **The comparator**, for two matching results, first difference wins:
  1. the query is the result's alias;
  2. the query is longer than three characters and is exactly the title or an alternate title (two such: learned-query match, then frecency, then the no-query order);
  3. the query is exactly one of the result's learned queries (tie: frecency, then the no-query order);
  4. the query is exactly the subtitle (tie: frecency, then title score);
  5. the alias starts with the query;
  6. a learned query starts with the query (tie: frecency);
  7. the query starts with a learned query of at least three characters (an "overbounds" match; the longer learned query wins; ignored when the query is more than three characters longer than it);
  8. the higher of the title, alternate-title and subtitle scores;
  9. higher frecency;
  10. higher title score;
  11. higher kind priority: commands (installed and Pane's own) above links (quicklinks), above applications, above files;
  12. the provider's own order;
  13. title, compared with numeric collation ("Item 2" before "Item 10").
- **The no-query order** (the last tiebreak): frecency, then having an alias, then kind priority, then provider order, then title. **The blank query** is the choice ADR 0030 left to this specification, and the user has made it: the pinned home (ADR 0027) first, then the commands and applications in the no-query order, so what the user opens most rises to the top. The list keeps its "Commands" label; no "Suggestions" or "Recent" section is added. Root search's documentation and the glossary's Pinned home entry stop saying that the blank query claims no recent use.
- **Sections and placement.** Sections keep Pane's names and roles, and, as ADR 0030 says, where computed results and file results are placed does not change: computed results (a computed answer under its command's title, such as "Calculator", and now "Color" and "Date & Time"; other computed results in the order their commands give) above the title matches; "Results" with its count, ordered by the comparator; then rows declared for typed addresses or paths (below); then "Files"; then "Fallbacks". Sections still only label. Computed results are not scored against the threshold, since their extension already chose them.
- **Quick slots get no boost while typing.** The pinned home is unchanged; a pinned result ranks in a typed query like any other.

### Learning from choices (ADR 0030)

- **A use** is recorded, as ADR 0030 decides, when the user invokes a root result from root search (Enter, a click, its number chord, and an argument field's Enter) and its action was dispatched (not refused as unavailable or paused), together with the query then in the field, normalised as matching normalises it. **Proposed:** invoking a quick slot from the pinned home records a use with no query. Not recorded: a result opened by its global hotkey; results without a lasting identity (computed answers, other computed results, file results); fallback rows (a fallback is chosen because nothing matched; proposed); Pane's own install and management rows; rows the user did not invoke.
- **Frecency** is a score with a ten-day half-life and a floor of 1: the score decays as score·2^(−elapsed/10 days); a use adds 1 to the decayed score and re-anchors it. A result never used scores 1. Time is the launcher's clock (the existing clock seam), so tests control it.
- **Learned queries:** the last three distinct non-empty queries used to open the result, newest first. They take part in ranking only while the result's frecency is above 1 and it was last opened within 17 days.
- **Identity** (ADR 0030, as quick slots under ADR 0026): a registered command by its command id; an indexed result by its own id under the command that supplies it; never a row's title or position. An application's id comes from the "Applications done properly" specification (#124) (its resolved target, version folders wildcarded) so that its history survives updates.
- **The record** is Pane's own (not extension data), beside its settings, aliases and quick slots: versioned, validated, written atomically one change at a time, an unreadable record never overwritten and reported, kept on this computer and never sent. Uninstalling a package forgets its entries (exactly its own, by the package part of the id, as aliases are forgotten); disabling keeps them; an entry whose score has decayed to 1 and was last opened more than 17 days ago is dropped, since it no longer affects ranking.
- **Controls (proposed defaults, delegated).** The Actions panel of every root result that can be learned offers "Reset Ranking" (no default key), which clears that result's frecency and learned queries and says "Ranking reset for <title>". Settings' Launcher page offers "Reset ranking…" for everything, with a confirmation, and a "Learn from what I choose" switch, on by default, which also covers query history (below); turned off, Pane records no uses and ranks as if no use had been recorded (what was learned is kept until reset). Visits that would not change any visible order do not re-sort the list on screen.

### Fallbacks (ADR 0031)

- As ADR 0031 decides: for a non-blank query where nothing but fallbacks is listed, the first fallback (in the order the user turned them on, which #31 set and ADR 0031 keeps) is selected; Enter sends it the whole query, trimmed. The no-results notice stays above it, unchanged in wording except that it no longer says nothing is selected. When anything else is listed, fallbacks stay below it unselected until the user moves to them.
- A fallback row is an ordinary row for number hints and Ctrl+digit.
- When results arrive after the first fallback was preselected and the user has not moved the selection, the selection moves to the new first row (the rule for late answers below); Enter's wait (below) makes that safe.
- A command with arguments can be a fallback only if its first argument is text and every other argument is optional; the query then fills its first text argument (the "Extension commands like Raycast" specification (#120)'s fallback text). Fallback rows never show argument fields.

### Publishing a query's list, provider budgets, and keys that wait

- **Publishing.** Ranking command metadata and kept indexed results is synchronous, as today. For computed results, the launcher publishes the new query's list when every provider asked for that query has answered, or 200 ms after the query changed, whichever comes first; until then the window keeps showing the previous list, while the search field shows what was typed at once. A provider that answers after the budget is merged into the published list; merges arriving close together are coalesced (within 16 ms) into one update.
- **Late answers and selection.** If the first row was selected and a merge puts a different row first, the selection moves to the new first row; otherwise the selection stays on its row by id (today's rule). A computed answer's section arriving late does not take the selection from a row the user moved to.
- **Stale answers** are discarded exactly as today: a newer query, or leaving root search, cancels a provider's pending call, and an older query's answer is never shown.
- **The runtime.** Provider calls still run one at a time on the shared extension runtime; the budget bounds how long the list waits, not how calls are scheduled. A provider that misses the budget keeps running until its answer or cancellation. When the "Extension commands like Raycast" specification (#120)'s runtime slice lands (a call waiting on the network no longer holds other packages' calls), a provider waiting on the network no longer holds the others, without changing this behaviour.
- **Keys that wait.** While the list for the exact current query is not yet published, Enter, Tab and Shift+Tab, Ctrl+K, Ctrl+digit, the selected row's action shortcuts (the "Extension commands like Raycast" specification (#120)), and Space when the query could still be an alias, are held until it is published, for at most 300 ms, then applied to the selection at that moment. Characters typed meanwhile are applied in order. An auto-repeated Enter never runs a second action. The launcher exposes whether the current query's list is published; the window owns holding and replaying keys.

### Alternate titles, keywords and same-name disambiguation

- **Indexed results** may carry alternate titles and keywords in addition to title and subtitle. The optional fields on the indexed-results interface are added by the "Applications done properly" specification (#124) (coordinator's call), so earlier components keep working; this specification scores them: alternate titles rank as titles, keywords as subtitles.
- **Commands** may declare keywords in the package manifest. This reverses the earlier "no manifest keywords" choice. Keywords are an author's search terms, distinct from the user's aliases.
- **Executable names** are alternate titles that the "Applications done properly" specification (#124)'s applications provider supplies (the program name without its extension, and any execution alias), except names that are generic or shared by several installed applications; the exclusion list is the "Applications done properly" specification (#124)'s.
- **Same-name rows.** When two or more listed rows have the same normalised title, each shows what tells it apart: for an installed command, its package's source after its subtitle (as alias and fallback rows already do); for an indexed result, the distinguishing subtitle its provider supplies (for applications, the "Applications done properly" specification (#124) computes it — the program name, else the folder, else the full path — and it is shown as the subtitle itself, so nothing is added twice); otherwise its supplying command's title after its subtitle.

### Understanding the typed query

- **Query analysis** is the core's, once per query change. A query is **URL-like** when it has no spaces, is not path-like, and either parses as an absolute URL with a scheme, or contains a dot that is not its last character and whose part before the first "/" is a plausible host (then "https://" is inferred). It is **path-like** when it starts with a drive letter and a separator, with "\\\\" (a network path), with "~" (resolved to the user's home folder), with "/", or with "file://".
- **Where a command appears.** A command may declare in the package manifest when it appears in root search (with a blank query, only while searching, or both) and what it matches: its title as usual, only URL-like queries, or only path-like queries. A command declared for URL-like or path-like queries is listed only for such queries, without title matching, below Results and above Files, in install order, and receives the parsed address or resolved path as its fallback text when invoked. This is the flexible extension point for authors; the default extensions below are its first users.
- **Default extensions supply the rows**, so the core stays small (ADR 0001; proposed default, delegated), recognised by the core's query classification above: the Quicklinks extension adds "Open in Browser" and "Create Quicklink" (the address prefilled) for URL-like queries, fitting the "Extension commands like Raycast" specification (#120)'s split of Quicklinks into separate commands; the Files extension adds "Open" and "Reveal in File Explorer" for path-like queries and lists a typed folder's entries; the calculator adds colours, date and time words, and percentages as computed answers.
- **Typed-folder listing.** For a path-like query naming an existing folder and ending in a separator, the Files extension lists that folder's entries as file results through a new host capability that lists a folder the user typed (no grant is needed: the user named it), within bounds like the scan policy's (direct entries only, name order, folders first, at most 500 entries, a partial listing saying so). Tab on a selected folder row completes the query to that folder's path with a trailing separator; Shift+Tab removes the last path component; Enter opens the selected entry with the system's handler (a program follows the "Instant file search" specification (#126)'s rule: Enter shows it, Run runs it). This capability is covered by ADR 0034's file access; no new ADR is needed (proposed default, delegated).
- **Colours.** "#RGB", "#RGBA", "#RRGGBB", "#RRGGBBAA", "rgb()"/"rgba()", "hsl()"/"hsla()" and "oklch()" are understood; colour names are not (Raycast deliberately ignores them, since "red" is also a word). The answer is drawn as a computed answer card with a swatch; Enter copies the colour as uppercase hex, and the Actions panel offers copying it as hex, RGB, HSL and OKLCH.
- **Date and time words.** "now", "time", "today", "date", "tomorrow" and "yesterday" answer at once with the local date or time in the system's format; Enter copies it, and the Actions panel offers ISO 8601 and a Unix timestamp. Date arithmetic and time zones stay out of scope.
- **Percentages.** "p% of x", "p% off x" (x reduced by p%), "p% on x" (x increased by p%), "x + p%", "x − p%" and "x as a % of y" are calculated within the calculator's existing number rules. Units and currencies stay out (CONTEXT: Pane has no unit conversion).

### Inline arguments

- **Shown** in the search field, after the query, for the selected row only, when its command declares arguments (the "Extension commands like Raycast" specification (#120): at most three, of text, password and dropdown). Each field is sized to its value or placeholder; a password field masks its value; a dropdown shows its choice or placeholder and lists a leading empty choice followed by its options; an optional argument carries an "optional" marker; a required one carries none. While they show, the query field's placeholder is the command's title.
- **Keys from the query:** Tab focuses the first empty argument (or the first, if none is empty); Shift+Tab focuses the last; Right with the caret at the end of the query focuses the first. **Inside an argument:** Tab goes to the next, and from the last back to the query; Shift+Tab to the previous, and from the first back to the query; Left with the caret at the start goes to the previous field; Right at the end to the next; Up and Down do nothing (the list does not move while typing an argument); Escape returns focus to the query with its text selected, then the usual Escape applies. Backspace in an empty argument does nothing special.
- **Enter** with every required argument filled runs the primary action with the values. With a required argument blank, focus moves to the next blank required argument and nothing runs; Enter inside a focused blank required argument marks it and says "Enter <placeholder>" in the status line. A required argument is marked as missing only after it has been left blank at least once, and the mark clears when it gets a value. Ctrl+digit and Tab-as-confirm on a row with blank required arguments focus them instead of running.
- **Values survive re-ranking:** when the list is rebuilt and the same row is selected again, values are kept for arguments with the same name and type, and a dropdown's value only while it is still a choice.
- **Remembered dropdown choices:** the last choice of each command's dropdown argument is remembered in Pane's own record and offered again, dropped when it is no longer a choice.
- **Alias and space.** When the query becomes the alias of a command that declares arguments followed by a space, the alias stays in the field, focus moves to that command's first empty argument, and text typed after the space (including text typed before the list was published) goes into the first argument if it is text or password, caret at its end. Alias and Tab does the same. **A command without arguments** opens as soon as its alias and a space are typed, like Raycast (proposed default, delegated): a view command opens its screen, and text typed after the space goes into its search field; a no-view command runs. A command that accepts fallback text (the "Extension commands like Raycast" specification (#120)'s successor to the query-taking command) but declares no arguments keeps today's row instead, listed first and sending the text after the alias when the user presses Enter, since that text is its input. Typing alone still sends no text to a command; the alias and space is the user's invocation of that command, as Enter is.
- **Accessibility:** each argument field is a labelled editable node inside the search combo box's group, named by its placeholder, with its required or optional state; a password field exposes no value.

### Recent queries

- Up, with an empty query and the first row (or no row) selected, and not an auto-repeat, restores the most recent query and its argument values; while the restored query is unchanged, Up again restores the one before. Any other key ends the walk. When a row other than the first is selected, or a query is typed, Up moves the selection as today.
- A query is recorded when it is cleared while not blank: Escape, the field emptied by a clear, returning to root, or the launcher closing with its search cleared. Consecutive duplicates are not added. At most 64 entries are kept. Password argument values are recorded as empty.
- The history is Pane's own record on this computer, never sent. Settings' Launcher page offers "Reset search history". The "Learn from what I choose" switch also stops recording history (one switch for both; proposed default, delegated).

### Settings and records

- Settings' Launcher page gains Search sensitivity (Low, Medium, High), "Learn from what I choose", "Reset ranking…" and "Reset search history", each registered with Settings search.
- Pane's own new records: what root search learned (frecency, last-opened time and learned queries per result identity), the search history, and the remembered dropdown choices. All follow the existing record rules (versioned, validated, atomic, one write at a time, unreadable never overwritten, uninstall forgets a package's entries).

### Modules

- **The core's search module** becomes a deep module with a small interface: given a query, the candidates with their normalised texts and kinds, what was learned about them, and the sensitivity, it returns the ordered sections and each row's highlight. Normalisation, transliteration, the scorer, the threshold and the comparator live behind it.
- **The launcher** owns query analysis, the learning record and its recording rules, the history, remembered dropdown choices, publishing with budgets, and whether the current query's list is published. It exposes Reset Ranking and the root-search actions through the Actions panel's existing result actions.
- **The extension contract** gains command keywords and the "when" and "matches" declarations in the package manifest; a minor, additive change to the extension API. The optional alternate titles and keywords on indexed results are the "Applications done properly" specification (#124)'s change, used here.
- **The window** owns argument fields, holding and replaying keys, Up recall, Tab path browsing, and drawing the highlight from the core's ranges.
- **Default extensions:** Quicklinks (URL commands), Files (path commands and typed-folder listing), the calculator (colours, date and time words, percentages).

## Testing Decisions

- A good test drives what a user does and checks what the user sees: type a query, read the ordered titles, sections and selection; press keys, read what ran. Tests never inspect scores, record layouts or private state.
- **The primary seam is the core's launcher interface** (the one the existing search, alias, calculator, quicklinks and files integration suites drive), with real sample extensions in Rust, JavaScript and TypeScript:
  - matching: abbreviations, word starts, gaps, transliteration (French accents, Vietnamese including đ, ß), title-and-subtitle spans, each sensitivity level admitting and rejecting the documented cases, highlight ranges;
  - ranking: each comparator step in order against a pair that differs only there, kind priority, numeric title collation;
  - learning: uses recorded and not recorded per the rules (hotkey-opened, computed answers, files, fallbacks), learned queries winning, the 17-day and frecency gates, decay over simulated days through the launcher's clock, reset per result and for all, the off switch, uninstall forgetting and disable keeping, survival across a restart with the same data folder, an unreadable record reported and not replaced;
  - the blank query: the pins, then commands and applications ordered by frecency under the "Commands" label, with no Suggestions section;
  - fallbacks: the first preselected when nothing else matches, Enter sending the query, the notice kept, unselected when results exist, late results taking the selection;
  - publishing: a slow provider (the existing slow fixture) holding the list up to the budget and merging late, the selection rule, stale answers discarded;
  - alternate titles, keywords and same-name disambiguators from a sample indexed provider and two copies of a package;
  - query analysis and the URL, path, colour, date and percentage rows from the default extensions, with the link opener and file opener seams that the quicklinks and files suites already fake;
  - arguments: values passed to the command, required-argument refusal, remembered dropdown choices, survival across re-ranking, fallback text into the first argument.
- **The window tests** with real key events cover what is drawn and what keys do: typing and Enter immediately while a provider is slow (Enter runs the published first row, not the previous list), held keys replayed in order, Tab and arrows into and between argument fields, alias and space jumping into the first argument (and opening a command without arguments at once), Up recall and its walk, Tab and Shift+Tab browsing a typed folder, the highlight drawn on the matched characters, the preselected fallback's accessibility node reported as the active descendant, and the sensitivity setting changing results live. Prior art: the window, aliases and command-search window suites.
- **Native checks** only where the window harness cannot see: opening a typed URL with the system's handler and a typed path in File Explorer, through the existing smoke scripts' phases, on Windows first.
- One seam per feature: matching, ranking and learning are tested at the launcher, never again in the window; key handling is tested in the window.

## Out of Scope

- A "Suggestions" or recently used section on the blank query, recently used files, calendar events, and a Favorites boost while typing.
- True typo tolerance (edit distance), locale-aware case folding, and camel-case word starts in root search.
- AI: Tab to Quick AI, "@" mentions, AI fallbacks.
- Inline translation, unit and currency conversion, time zones and date arithmetic, calculator history, and syntax highlighting of expressions.
- Reordering fallbacks, default fallbacks, a cog to manage fallbacks from the Fallbacks section, and fallback-only commands.
- Form drafts offered from root search, deeplinks and "Configure Deeplink…".
- Disabling individual commands, hiding a result from root search, and aliases or hotkeys on applications and other indexed results.
- Provider backoff after repeated timeouts and a deferred-lookup cache for network-backed inline answers.
- File content search and the home-folder file index (the "Instant file search" specification (#126)); application discovery, identity, icons and the generic-name list (the "Applications done properly" specification (#124)); the argument and launch-record contract and the Actions panel's sections and shortcuts (the "Extension commands like Raycast" specification (#120)); Narrator announcements (the "Quick fixes" specification (#129)).
- Querying online services from root search, which stays inside commands (command search), unchanged.

## Further Notes

- Research: `docs/research/raycast-deep-dive.md` (root search, keyboard and arguments), with the earlier `docs/research/raycast-root-search.md`, `raycast-inspiration.md` and `raycast-favorites.md`. Raycast facts used here: its matcher scores 4/3/2 with a gap cost of 1 over the separators listed; Medium is 1.5·(n−2)+4 and High is above 2n; its fresh install stores High; frecency decays with a ten-day half-life; it keeps the last three learned queries per item, used only within 17 days and when frecency is above 1, ignoring overbounds terms more than three characters shorter than the query; a use is recorded 100 ms after an item runs, never for one opened by its own hotkey nor for calculator, colour, file or fallback-web-search rows; providers get 200 ms, late answers are batched every 16 ms, and Enter, Tab, Ctrl+K, number keys and action shortcuts wait up to 300 ms for the current query's results; Up restores up to 64 queries with their arguments, blanking passwords; inline arguments are at most three, with the keys above; alias and space jumps into the first argument; dropdown choices are remembered per command; the first fallback is selected when nothing matches; applications' program names are alternate titles unless generic or shared.
- Glossary updates to make with ADR 0030 and ADR 0031 (through domain modeling): Result section, Pinned home and its Avoid line, No-results notice, Fallback, Alias (alias and space now enters arguments), Query-taking command (relation to arguments, with the "Extension commands like Raycast" specification (#120)); new terms for learned ranking (proposed: "Learned ranking", avoiding "suggestions" and "history") and for argument fields (Raycast's "argument" is currently an Avoid term of Query-taking command and must be reconciled with the "Extension commands like Raycast" specification (#120)).
- **Settled since the first draft:**
  1. The blank query: the pins, then commands and applications ordered by frecency, with no Suggestions section (the user's decision).
  2. Alias and space on a command without arguments opens it at once, as Raycast does (proposed default, delegated).
  3. Search sensitivity's default is High, Raycast's stored default (proposed default, delegated).
  4. Learning has a Reset Ranking action per result, a reset-all button and an off switch, and the off switch also covers search history (proposed default, delegated).
  5. Typed-folder listing is covered by ADR 0034's file access; no ADR of its own (proposed default, delegated).
  6. The URL and path rows are default extensions' commands, recognised by the core's query classification (proposed default, delegated).
- **Open question:** whether non-answer computed results (such as links an extension computes for the query) should eventually be ranked by the core with learning, which would need an amendment to ADR 0030's "placement does not change" (not proposed here).


---

Decisions this specification relies on are recorded in [ADRs 0030–0040](https://github.com/hoangvu12/pane/tree/main/docs/adr) (added by [#119](https://github.com/hoangvu12/pane/pull/119)); the evidence is [docs/research/raycast-deep-dive.md](https://github.com/hoangvu12/pane/blob/main/docs/research/raycast-deep-dive.md).

