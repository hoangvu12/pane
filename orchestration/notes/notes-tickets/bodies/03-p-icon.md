## Parent

https://github.com/pane-app/pane/issues/123 (Launcher polish; ADR 0035)

## What to build

The icon rules of the specification are already mostly shipped: an application's own icon draws bare with a same-size neutral placeholder until it arrives, file rows draw the system's icon for their type bare, an extension's image icon draws as supplied with its mask, a package without an icon gets the generated first-letter tile, and pinned slots, compact pins and the Actions panel's header follow the same rules (#139, #142, #172). This ticket completes the one rule that remains: **a built-in glyph an extension names draws on Pane's neutral tile**, so a command that names one of Pane's glyphs reads as a command, while image icons stay bare. It also adds the guard tests that keep the whole bare/tile split true across the surfaces.

## Acceptance criteria

- [ ] A built-in glyph an extension names draws on Pane's neutral command tile, at the row's, a slot's and the Actions header's sizes
- [ ] An extension's image icon still draws bare, with its mask; a package without an icon still gets the first-letter tile
- [ ] Guard tests: an application row with an icon has no tile and one without keeps a same-size placeholder; a command row keeps its tile; pins, compact pins and the Actions panel's header follow the same rules
