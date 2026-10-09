Implements specification #122 (Root search like Raycast) through its 13 tickets. Root search becomes Raycast's root search, adapted to Pane's vocabulary and small core:

- Fuzzy, accent-blind matching with a Search sensitivity setting and highlighted matches (#193), ranked with Raycast's comparator, keywords, alternate titles and same-name disambiguation (#197), which learns from what the user chooses: frecency and learned queries (ADR 0030), with per-result and global reset and an off switch (#199, #200).
- The blank query shows the pins first, then commands and applications ordered by frecency (#199).
- The first fallback is preselected when nothing else matches (ADR 0031) (#194).
- A query's list is published once its providers answered or 200 ms passed, late answers merge without flicker, and Enter, Tab, Ctrl+K, Ctrl+digit and action shortcuts wait briefly for the published list (#201, #203); per-keystroke guest and re-rank costs are cut (#202).
- Typed web addresses and paths are understood, commands may appear only for them (#195), and a typed folder's entries are listed and browsed with Tab and Shift+Tab (#204).
- The calculator answers colours, date and time words, and percentages (#196).
- Commands that declare arguments show inline fields after the query, with alias and space jumping into them (#205), and Up recalls recent queries (#206).

Closes #122
Closes #193
Closes #194
Closes #195
Closes #196
Closes #197
Closes #199
Closes #200
Closes #201
Closes #202
Closes #203
Closes #204
Closes #205
Closes #206

Signed-off-by: hoangvu12 <hggaming91@gmail.com>
