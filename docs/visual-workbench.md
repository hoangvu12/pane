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

A scenario lives in `crates/pane/src/visual_fixture.rs`. It is a name, its client size, whether the reference authors that state, its rows, and its steps (`capture`, `pointer`, `key`, `type`, `click`). A `click` names an element the fixture declares (the footer's Actions button): the native helper posts a press and release at its center, and the reference capture clicks the same element in the board. A scenario can also name a static `board` (the Actions board): that board authors its one state, so the reference side captures it as authored and takes no steps, while the native side takes the steps that reach it. Both helpers follow the same steps. The fixture also replays them over its own state model, so the manifest declares, per capture, which rows show, which is selected, where the pointer is and where everything lies.

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

To add a scenario, append it to `SCENARIOS`. A scenario can name its own appearance (`theme`), which overrides the run's `-Theme` for it alone, and can ask for the frame checks (`frame`). Use only production components in the fixture's render, and add a branch to the comparison for any new component family. If the reference board already exists but the native component doesn't, the board is listed in `pending_scenarios()` with the ticket that registers it: calculator and empty #96, Appearance #98, pinned strip #101, clipboard #102. Asking for a pending scenario fails with that ticket's URL, so it can never pass silently. Store and the snap HUD are source-only references this milestone.

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
- **Section labels.** Pane labels a blank query's rows "Commands" and claims no recent use (#100). The reference's "Suggested · From your recent use" (and its pinned strip, #101) above the same rows is an accepted content difference; a label both sides show is compared for its type and place.
- **No reference counterpart.** The unavailable and long-content states are native-only: captured and cropped, never compared. Both carry an alias and keys. The long row's trailing parts are measured against their declaration, including a check that no text ink reaches the gap before the alias chip.
- **A row that can grow.** A row holding an unavailable reason declares only a height floor. The rows after it declare no exact position, and its own trailing parts (alias, keys, kind) aren't measured; its crop is the evidence.
- **Input path.** Pointer and keys are posted window messages (see above), not `SendInput`. The real pointer path is the smoke's job.
- **Fixture composition.** The leaf components are production code: `result_row`, `search_header`, `action_button`, `key_sequence` (through the launcher's own binding adapter, `keyboard::key_sequence`), `tile_at`, the panel and footer materials, the result list (`ui::shell::result_list`, which the launcher's screens lay their rows out in too), the footer's row, mark, hint and buttons (`ui::footer`) and the Actions panel (`actions_panel::compose`, `anchored`, `dimmer`), and the Settings shell (`ui::settings_shell`, laid out by the Settings window's own `features::settings::compose`, titlebar and panel included). The arrangement around them is the fixture's own: the search wrapper and the footer strip (the Settings shell's arrangement is the window's own, shared whole). They share the launcher's layout tokens, but a layout change made directly in `LauncherWindow::render` would not reach the fixture. Perturbations change rendering values, so they cannot show that either. Keep shared layout in shared functions and constants as the port proceeds.
- **Kerning.** Browsers kern by default and the reference's Geist is laid out kerned. GPUI on Windows passes DirectWrite an explicit feature list without `kern`, so Pane asks for it in its typography (`Typography::features`): unkerned, its 12.5px footer text ran about 2% wider than the reference's (#95). The fixture shapes the text it declares with the same features.
- **Shadows and draw order.** GPUI orders each primitive by the primitives its box overlaps, and a drop shadow's box leaves out its blur, so content that only the blur reaches can sort below the shadow and be darkened by it. The reference draws the footer above the Actions panel's shadow; Pane's footer row carries an imperceptible fill (0.1% black) whose box overlaps the shadow's, so its content sorts above it (#95).
- **The Actions panel.** While it is open, the results lie under its dimmer and their right ends under the panel, so the rows' and labels' own checks are made in the captures with it closed. Its entries are compared by what they do (the primary action, the hotkey, the alias): the reference lists operations Pane has no contract for (Pin, #101; new window, file manager, quit, hide; #100), an accepted content difference. The reference's board runtime never delivers the `ref` its logic focuses the panel's search with, so its capture focuses that field the way the logic would. Text edges in the footer are read by coverage (a pixel's worth of ink per column or row) rather than core pixels: Chrome rounds Geist's ascent, which moves its 12.5px text half a pixel lower than GPUI's.
- **The Settings shell (#97).** The fixture draws the board's own sidebar labels (General, Appearance, Hotkeys & Aliases, Plugins with its count, Window Manager, Clipboard, Privacy, About), not Pane's seven pages: the Settings window's real sections are checked by its window tests (`crates/pane/tests/settings.rs`). Pane has no counterpart for the board's palette, shield and info glyphs, so the Appearance, Privacy and About sections draw a stand-in, compared by place only. The page shows the board's heading, subtitle and preview caption; the Appearance controls and preview are #98's. On Windows the titlebar carries the platform's minimize, maximize and close buttons in place of the board's lone close glyph; they are not compared. A static board takes no steps, except that the Settings board's sections take pointer steps (`.nav:hover` is CSS, so the authored state is otherwise untouched). Its fills are read as overlays against the fill right beside them: the sidebar's black 10% against the page past its rule, the search well's black 24% against the sidebar below it.
- **Fixture data.** The rows, their order and their actions match the reference root board. Its pinned strip and section labels are not production content yet (#101), and neither is its placeholder copy. Those differences show up as parity failures (row `top in client`, placeholder ink), not as matched data.
