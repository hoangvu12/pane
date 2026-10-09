## Parent

https://github.com/pane-app/pane/issues/123 (Launcher polish; ADR 0035)

## What to build

The launcher's text levels become the primary text colour at strengths — primary (titles, the query) 100%, secondary (subtitles, kind labels, footer labels, keycap labels) 60%, tertiary (section labels, the placeholder, disabled text) 40%, faint marks 20%, separators 10% — wherever that stays legible on the surface beneath. For each theme, material and background-image case Pane supports, a level whose alpha form would fall below its contrast floor keeps its present opaque value: 4.5:1 for primary and secondary text, 3:1 for tertiary text and placeholders. The check is made against each surface's own tint, so glass over an unknown desktop is judged on its tint alone, as the existing legibility floor for the glass tint is. The Settings window's text roles are unchanged.

## Acceptance criteria

- [ ] Secondary, tertiary, faint and separator text follow the alpha strengths wherever they pass their floor
- [ ] Theme tests check that each text role's chosen form meets its floor for every theme, material and background-image case
- [ ] A role that would fail its floor keeps its accepted opaque colour
- [ ] The Settings window's text roles are unchanged
