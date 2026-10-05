# Visual workbench: Windows reference/native comparison

The workbench ([#91](https://github.com/hoangvu12/pane/issues/91)) is how every visual ticket of the UI port ([#90](https://github.com/hoangvu12/pane/issues/90)) shows its work. It renders Pane's **production** GPUI components in a native fixture window. It also opens the hash-pinned authored reference in headless Chrome. Both sides are put in the same named states with the same fixture data, captured at the same logical size, and compared. It is a Windows-only, local tool. No CI, other OS or release matrix runs it.

## One command

From the repository root, in PowerShell:

```powershell
./scripts/visual-workbench.ps1                  # everything, into .scratch/visual-workbench/run-<stamp>
./scripts/visual-workbench.ps1 -RealAppSmoke    # plus one real search interaction with pane.exe
./scripts/visual-workbench.ps1 -Scenario root-selected,keycap-windows -SkipSensitivity
```

It needs Rust, Node 24 (for its global `WebSocket`), Python 3 with Pillow, and Chrome at `C:/Program Files/Google/Chrome/Application/chrome.exe` (`-Chrome` overrides the path). In order, it:

1. Checks the reference's SHA-256 (`docs/evidence/ui-prototype/reference/launcher.html`, `F7E81E03…0BB4`) and refuses any other bytes.
2. Builds the fixture: `cargo build -p pane --bin pane-visual-fixture --locked -j 1` (`-SkipBuild` reuses the last build).
3. Writes the scenario registry: the fixture's `--registry`. Both capture helpers follow its steps.
4. Captures the reference: `scripts/visual-workbench/reference-capture.mjs`.
5. Captures the fixture: `scripts/visual-workbench/native-capture.ps1`.
6. Compares the two: `scripts/visual-workbench/compare.py`, writing `compare/report.json`, `compare/summary.md`, and per capture a side-by-side, an amplified diff, an overlay and crop pairs.
7. Proves the comparison is sensitive. It re-captures with `--perturb row-padding-plus-4`, `--perturb selected-fill`, `--perturb hover-fill` and `--perturb nav-selected-fill` (the Settings sidebar's selected section, #97), and checks that each flips the checks it drives from passed to failed (`compare-<perturbation>/`). The run fails if any fault goes unnoticed. The faults are rendering faults: the manifest keeps declaring the intended values.
8. With `-RealAppSmoke`, runs `scripts/visual-workbench/real-app-smoke.ps1`. That builds and starts the real `pane.exe`, types `install`, presses Down, opens the selection with Enter (the npm install form, which fetches nothing until submitted), goes back with Escape, then presses Escape again, and captures each step. It then asserts on the captures with `scripts/check_screenshot.py`: every step up to the form changed the window, the first Escape returned to exactly the root search the form was opened from, and the second returned to exactly the rest state.

`workbench-run.json` records the command, the revision, whether the tree was dirty, the binaries' hashes and each step's outcome. Each scenario's `native-run.json` records the fixture's DPI, client and window rects, the process exit, and cleanup. `reference-manifest.json` records the Chrome version, every board's glass rect and the DOM state at each capture.

## What it starts, and what it cleans up

- **Fixture.** Every scenario gets its own fixture process with a temporary data directory and scratch `LOCALAPPDATA`. The fixture loads no extensions and reads no installed commands. Its rows are presentation data and never become anyone's installed-app list. The script closes that process (`WM_CLOSE`, then that PID only) and deletes that temporary directory.
- **Reference.** The reference is served from a loopback-only HTTP server on a random port (board frames are cross-origin under `file://`). Chrome runs headless with a fresh temporary profile. The script ends only that Chrome's process tree (`taskkill /PID <pid> /T /F`) and deletes only that profile.
- **Your desktop is not touched.** The fixture opens without activation. As soon as its window exists it moves past the right edge of the virtual screen, so it covers nothing and your pointer cannot reach it. Input is posted to the fixture's own window: `WM_MOUSEMOVE` at a step's client point, `WM_KEYDOWN`/`WM_KEYUP`, and `WM_CHAR` for typing. GPUI's message loop handles these exactly like typed keys. Captures use `PrintWindow(PW_CLIENTONLY | PW_RENDERFULLCONTENT)`, which reads the window's own composition even off-screen. So you can keep using the machine during a run, and no cursor glyph appears in any capture. This doesn't exercise the OS's own hit-testing and focus arbitration. The real-app smoke and `scripts/capture-pane-windows.ps1` cover those.
- **Real-app smoke.** `pane.exe` opens centered and active, as it does for a user. That is the one visible moment of a run; the smoke then parks it off-screen the same way.
- **Output.** Everything is written under `-OutputDir`. `.scratch/` is untracked.

## Scale: 100% only

At 96 DPI (100%) one logical pixel is one image pixel. That is the only scale this workbench has been measured at. The fixture reads its DPI per window and the native helper verifies the client is the scenario's logical size times that scale. The comparison converts native measurements back to logical pixels. Reference captures always use device scale factor 1. **125% and 150% have not been run.** No measurement at those scales exists, and a run at them must be recorded as new evidence rather than assumed equivalent. Images are never rescaled; a size mismatch is a failed check.

## Sizes

The reference boards' glass panels measure:

| Board | Size |
|---|---|
| Root, Actions, calculator, empty | 760×518 |
| Settings | 1120×720 |
| Clipboard | 940×600 |
| Store, snap HUD | 1160×740, 740×396 |

Every native scenario declares its client size, and the native helper refuses a client of any other size. The reference capture saves every board's panel at its own size, with Windows labels, under `reference/boards/`. Those are the saved references for the scenarios later tickets register.

## Scenarios: the registration seam

A scenario lives in `crates/pane/src/visual_fixture.rs`. It is a name, its client size, whether the reference authors that state, its rows, its pins, and its steps (`capture`, `pointer`, `key`, `type`, `click`, `point`). A `click` names an element the fixture declares (the footer's Actions button): the native helper posts a press and release at its center, and the reference capture clicks the same element in the board. A `point` names one the same way (a pinned slot, `slot-<n>`) and only moves the pointer there, since a slot's click launches it. A scenario over the root board's rows shows the board's five authored pins (`ROOT_PINS`) above them while its query is blank, as the reference does (#101); a scenario with no pins shows no home. A scenario can also name a static `board` (the Actions board): that board authors its one state, so the reference side captures it as authored and takes no steps, while the native side takes the steps that reach it. Both helpers follow the same steps. The fixture also replays them over its own state model, so the manifest declares, per capture, which rows show, which is selected, where the pointer is and where everything lies.

| Scenario | States captured | Reference |
|---|---|---|
| `root-rest` | rest | yes |
| `root-hover` | rest → pointer on row 1, which selects it | yes |
| `root-pointer-keys` | pointer on row 1 → Down (row 2 selected, row 1 keeps its hover wash under the resting pointer) → move again over row 1 (#94) | yes |
| `root-selected` | rest → Down → Down | yes |
| `root-selected-hover` | Down, Down → pointer on the selected row | yes |
| `root-focus` | rest → type `clip` → Escape | yes |
| `root-actions` | Down (Clipboard History) → the Actions button opens its panel → type `alias` (one entry) → type `zz` (none: "No actions match") → Escape (closes the panel only) (#95) | yes |
| `actions-panel` | the static Actions board: `fig` typed, Figma selected, its panel open (#95) | yes (static board) |
| `root-unavailable` | rest → Down (a row with its reason) | native-only |
| `root-long-content` | truncation | native-only |
| `launcher-frame` | the frame at rest: panel size, inset edges, corner, header/list/footer boundaries, footer padding (#92) | yes |
| `launcher-frame-light` | the same frame in the derived light palette | native-only |
| `launcher-frame-narrow` | 480×360: rest → Down ×7 (the last row scrolled into view above the footer) | native-only |
| `tile-sizes` | the icon tile at 28, 42 and 18px, as an application and as a command (#93) | native-only |
| `keycap-windows` | Ctrl K, Win Alt ←, Ctrl Shift V, the accent ↵ and the compact Ctrl 1 (#93); then Shift ↵, Ctrl ↵, Ctrl Shift P | yes for the first five (the root board's key groups) |
| `settings-shell` | the Settings board at 1120×720: the titlebar, the sidebar's search and sections (Appearance selected), the page's heading block and column caption → the pointer over General (hover) → the pointer over the selected Appearance (#97) | yes (static board; its sections take the pointer) |
| `settings-shell-narrow` | the same shell at 760×520: the sidebar keeps its 232px, the page's columns collapse into one (#97) | native-only |
| `empty-state` | the static empty board: `kubectx` matching nothing, the no-results notice over three fallbacks, none selected → Down (the first selected); the board's store suggestions (#96) | yes (static board) |
| `calculator-card` | the static calculator board: `72 in to cm` answered by the selected card, with the board's units, chips and recent calculations (#96) | yes (static board) |
| `answer-plain` | a computed answer as production draws it: `6*7` and 42, no captions or chips, under "Calculator" | native-only |
| `answer-long` | an expression too long for the 34px values: they step down to fit their columns | native-only |
| `empty-no-fallbacks` | the notice with no fallback under it | native-only |
| `clipboard-rest` | the clipboard board at rest: Pinned, Today and Yesterday, the code clip selected and previewed (#102) | yes (the clipboard board, driven) |
| `clipboard-previews` | a click on Standup, the lime color, the link, the screenshot: each selects (never copies) and its preview follows | yes |
| `clipboard-keys` | Down (lime) → five more Downs reach the last clip, scrolled into view | yes |
| `clipboard-filter` | `zzz` (no clip, no preview, "Nothing selected") → Escape (cleared) → the Text tab | yes |
| `clipboard-production` | production's text records, All/Text, what is kept, Copy/Delete/Manage → Down ×3 (a long record's preview) | native-only |
| `clipboard-off` | nothing kept, history off: the note, Turn on, no preview, no primary action | native-only |
| `clipboard-narrow` | the split view in the launcher's own 760×518 → the last record selected | native-only |

To add a scenario, append it to `SCENARIOS`. A scenario can name its own appearance (`theme`), which overrides the run's `-Theme` for it alone, and can ask for the frame checks (`frame`). Use only production components in the fixture's render, and add a branch to the comparison for any new component family. If the reference board already exists but the native component doesn't, the board is listed in `pending_scenarios()` with the ticket that registers it: Appearance #98. Asking for a pending scenario fails with that ticket's URL, so it can never pass silently. Store and the snap HUD are source-only references this milestone.

The fixture's data matches the reference root board: Suggested then Commands, with the footer action following the selected row's kind. The fixture is not the launcher's wiring. That is covered by `the_production_scenario_edits_searches_selects_opens_and_back_navigates` in `crates/pane/tests/window.rs`, plus the real-app smoke.

## Reading the report

`compare/summary.md` has three sections, and a list of accepted discrepancies:

- **harness-native** — the fixture's declared layout and colors against what its capture measures. It validates the harness and should pass completely. A failure means the fixture didn't reach the declared state or the measurement is wrong.
- **harness-reference** — the reference DOM's rects, classes and computed colors against what its capture measures. It should also pass completely.
- **parity** — native against reference for the same component in the same state. Today this fails broadly; #92–#99 close the gap. Each failure names the property, both values, the delta and the limit.

- **Accepted discrepancies** — parity checks that still fail, each carrying the disposition its ticket recorded for a measured platform limit (the first: Windows' launcher corner, #92). They are counted apart from the failures and listed with their reason; they never count as passed.

Limits are the ticket's proposed ones: 1 logical px for edges and baselines, and 2 channel levels for deterministic flat fills. Translucent washes are compared as backdrop-relative overlay alpha, measured column by column against the gaps beside the row. That survives the opaque native panel versus the reference's glass over a blurred wallpaper. Glyph checks use only core pixels (60% of the way from background to text color), so antialiased edges never decide a result. Glyph core colors have their own 4-level limit, and the accent caret is excluded from header ink.

## Limitations

- **Glass is reviewed separately.** Strict color checks run in the opaque material (`-Material opaque`, the default). A glass run can be captured with `-Material glass`, but its colors depend on the desktop behind it, so it belongs to a matched-backdrop review, never a strict check. That review needs the window *on screen*: Windows applies the acrylic blur, the window's rounded corners and its shadow while composing the desktop, and `PrintWindow` reads the window's own content before that. So it cannot run off-screen like every other capture here, and it is not run without the operator's agreement (see [#92's evidence](evidence/ui-92/README.md)).
- **Corners.** Windows rounds the launcher window itself (the DWM corner preference; Microsoft documents 8px for it), so the fixture's captures show square corners where the reference's panel curves at 18px. The comparison measures that and lists it as an accepted discrepancy.
- **Labels are controlled.** Both sides show the same effective bindings with `platform: 'Windows'` set on every board; default macOS captures are never used as Windows goldens. Both draw one cap per key in Geist Mono 11/500 (#93). The reference embeds Geist and Geist Mono as 225-glyph subsets without arrows or the return symbol, so its `←` and `↵` come from a system fallback face while Pane draws Geist Mono's own. Their label positions are listed as accepted discrepancies.
- **Text rasterization.** Pane's text is rendered with subpixel (ClearType-style) antialiasing; Chrome's headless captures are grayscale. Glyph checks use core pixels and their own limit, so colored edge fringes never decide a result, but they are visible in the crops. A thin stem can fall across two pixel columns on one side and in one column on the other, so a label's ink top (the alias chip's) counts two neighbouring pixels' coverage together rather than reading core pixels alone.
- **Selection policy.** On both sides pointer movement selects the row (#94), and a pointer that only repeats its position does not. Each capture's selected and hover-only rows are compared by name. A hover-only wash, the 3.5% one that shows where the keys moved the selection off the row the pointer rests on, is compared by its alpha: on the reference's glass it is too faint to edge reliably.
- **Section labels.** Pane labels a blank query's rows "Commands" and claims no recent use (#100). The reference's "Suggested · From your recent use" above the same rows is an accepted content difference, and so is the place of a row below it, one label lower there; a label both sides show ("Pinned", "Commands") is compared for its type and place. The pinned home's slots are compared slot by slot (`harness_pinned`, `parity_pinned`): fill, box, title, tile and chord, and whether the home shows at all.
- **No reference counterpart.** The unavailable and long-content states are native-only: captured and cropped, never compared. Both carry an alias and keys. The long row's trailing parts are measured against their declaration, including a check that no text ink reaches the gap before the alias chip.
- **A row that can grow.** A row holding an unavailable reason declares only a height floor. The rows after it declare no exact position, and its own trailing parts (alias, keys, kind) aren't measured; its crop is the evidence.
- **Input path.** Pointer and keys are posted window messages (see above), not `SendInput`. The real pointer path is the smoke's job.
- **Fixture composition.** The leaf components are production code: `result_row`, `search_header`, `action_button`, `key_sequence` (through the launcher's own binding adapter, `keyboard::key_sequence`), `tile_at`, the panel and footer materials, the result list (`ui::shell::result_list`, which the launcher's screens lay their rows out in too), the footer's row, mark, hint and buttons (`ui::footer`) and the Actions panel (`actions_panel::compose`, `anchored`, `dimmer`), and the Settings shell (`ui::settings_shell`, laid out by the Settings window's own `features::settings::compose`, titlebar and panel included). The arrangement around them is the fixture's own: the search wrapper and the footer strip (the Settings shell's arrangement is the window's own, shared whole). They share the launcher's layout tokens, but a layout change made directly in `LauncherWindow::render` would not reach the fixture. Perturbations change rendering values, so they cannot show that either. Keep shared layout in shared functions and constants as the port proceeds.
- **Kerning.** Browsers kern by default and the reference's Geist is laid out kerned. GPUI on Windows passes DirectWrite an explicit feature list without `kern`, so Pane asks for it in its typography (`Typography::features`): unkerned, its 12.5px footer text ran about 2% wider than the reference's (#95). The fixture shapes the text it declares with the same features.
- **Shadows and draw order.** GPUI orders each primitive by the primitives its box overlaps, and a drop shadow's box leaves out its blur, so content that only the blur reaches can sort below the shadow and be darkened by it. The reference draws the footer above the Actions panel's shadow; Pane's footer row carries an imperceptible fill (0.1% black) whose box overlaps the shadow's, so its content sorts above it (#95).
- **The Actions panel.** While it is open, the results lie under its dimmer and their right ends under the panel, so the rows' and labels' own checks are made in the captures with it closed. Its entries are compared by what they do (the primary action, the hotkey, the alias): the reference lists operations Pane has no contract for (new window, file manager, quit, hide; #100), an accepted content difference. A group label and the rule above it are placed against the first entry below them that both sides list (the "Pane" group's Pin to Quick Slot), since the reference's extra entries above them make its panel taller. The reference's board runtime never delivers the `ref` its logic focuses the panel's search with, so its capture focuses that field the way the logic would. Text edges in the footer are read by coverage (a pixel's worth of ink per column or row) rather than core pixels: Chrome rounds Geist's ascent, which moves its 12.5px text half a pixel lower than GPUI's. The reference's two boards draw the open Actions button's label in two colors (white on the static Actions board, `.fbtn`'s #D9DADD on the root board's live button). Pane draws it white, as the static board does, so its label's left edge is accepted against the root board's while their colors differ.
- **The Settings shell (#97).** The fixture draws the board's own sidebar labels (General, Appearance, Hotkeys & Aliases, Plugins with its count, Window Manager, Clipboard, Privacy, About), not Pane's seven pages: the Settings window's real sections are checked by its window tests (`crates/pane/tests/settings.rs`). Pane has no counterpart for the board's palette, shield and info glyphs, so the Appearance, Privacy and About sections draw a stand-in, compared by place only. The page shows the board's heading, subtitle and preview caption; the Appearance controls and preview are #98's. On Windows the titlebar carries the platform's minimize, maximize and close buttons in place of the board's lone close glyph; they are not compared. A static board takes no steps, except that the Settings board's sections take pointer steps (`.nav:hover` is CSS, so the authored state is otherwise untouched). Its fills are read as overlays against the fill right beside them: the sidebar's black 10% against the page past its rule, the search well's black 24% against the sidebar below it.
- **Result boards (#96).** The empty and calculator boards are static. A scenario that renders one starts with the board's query already in its field and its rows listed as authored, and composes the production result layouts (`ui::result_layouts`, through `features::root_search::layouts`): the notice heading the list, the answer card in its first row's place, the history rows, the suggestions after the rows. The fixture declares each part (the card's values centered in their columns from shaped text with their tracking), and the comparison measures them on both sides: the notice's disc and lines, the card's box, fill, accent ring, values, captions, arrow, chips and rule, the suggestions' tiles, pills and titles. The card's edges are read against the panel just outside each one, since the reference's glass above the card is lighter than the list's padding beside it. The reference's units, conversions, recent calculations and store suggestions are fixture content with no provider in Pane (#100). Pane's notice copy, the fallbacks waiting for a deliberate selection (the board preselects one and draws their titles plain), the Fallbacks label's missing "Reorder in Settings" and both boards' footer tips (paste into the previous app, install from the store) are accepted content differences. With nothing selected the fixture shows no primary action, as it always has (the launcher shows its unusable "Open command", dimmed), so the footer's rule is measured only beside one.
- **The clipboard board (#102).** It is driven like the root board (its `input.q` focused, its rows and tabs clicked by what they show, `CLICK_TARGETS`), and captured with its own DOM state (`clipboardState`). The fixture's reference variant shows the board's own clips, labels and buttons — its links, images, colors, Pinned group, "Paste to Obsidian" and privacy caption are fixture-only; production keeps text alone (#100) — and the native-only `clipboard-production`, `clipboard-off` and `clipboard-narrow` show production's own content through the same parts. The reference scrolls a clip the keys select 8px past the list's edge; GPUI scrolls the least that shows it, an accepted difference in `clipboard-keys/last`. The image preview's hatch is an SVG pattern, not a repeating gradient, and its phase is not compared.
- **Icons through text's contrast.** GPUI draws every monochrome sprite, SVG icons included, through its text's grayscale contrast and gamma correction (`gpui_render`'s `fragment_monochrome_sprite`). That lifts a stroke's anti-aliased edge pixels, while a stroke's fully covered pixels match Chrome's. So a glyph's box inside its tile is read by coverage, against each row's own gradient, and its core pixel count (the stroke weight check) is accepted only where the native glyph is heavier. A lighter one, such as a 1.6 stroke where the reference draws 2, still fails. The same lift moves a thin diagonal's tips (a slash's) past a core threshold, so a row title's height is its type's body: the rows holding a quarter of its fullest row's ink, by coverage. The no-results notice's lines are compared by top and baseline, so a descender one copy has and the other lacks (#96's accepted copy) doesn't decide their size.
- **Half-pixel edges.** The reference's app tile edge is half a pixel of white 28%, which Chrome rasterizes at 100% as the tile's outermost pixel at about 14%. GPUI draws next to nothing for an inset spread under a pixel, so Pane draws that pixel: a 1px ring of white 14% (`Theme::tile_app_edge`).
- **The pinned home (#101).** Measurement follows these rules:
  - A slot's box is read against the panel just outside each of its edges, inside the strip's 8px column gap. The reference's glass shades down the slot, and a scan wider than the gap would read the neighbouring slot.
  - A slot's key hint lies over its tile's corner (in the reference too), so a cap's edges and fill are read around the tile and its drop shadow.
  - In the narrow frame's 85.6px slots, the hint covers the columns a tile's top is read down. There the tile's top and height are not measured; its left and width are.
  - An ellipsized title is checked to lie inside its line rather than centered.
  - Slots the scrolled list carries out of view are not measured, as rows aren't.
  - The "Pinned" label's caps inherit the label's .01em tracking, as the reference's `.kbd` inherits `.label`'s, and the fixture declares them with it.
  - An unavailable slot (Pane's own state) keeps its tile and title with the reason right under the title, 2px from the tile, so the three fit the slot's 76px inside its paddings.
- **Static boards' query.** A result board's query is in the field from the start, with its caret after it as typing leaves it. A caret at the field's start moves GPUI's text a pixel right.
- **Fixture data.** The rows, their order and their actions match the reference root board, and so do its five pins (authored for the comparison only; a fresh Pane pins nothing). Its "Suggested" label is not production content (#100), and neither is its placeholder copy. Those differences show up as accepted or failed parity checks (the rows below "Suggested", placeholder ink), not as matched data.
