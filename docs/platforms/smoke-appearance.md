# Behavior smoke appearance

The Windows, macOS and Linux smoke scripts force `PANE_THEME=dark` and
`PANE_MATERIAL=opaque`, including their installed-application and update phases.
They record that choice in `system.txt`. This keeps behavior screenshots
independent of the desktop background and the caller's appearance environment.
These runs do not establish light-theme readability or native blur; those remain
separate native acceptance work for [#61](https://github.com/hoangvu12/pane/issues/61).

`scripts/check_screenshot.py` recognizes the shared dark panel's connected
neutral surfaces, including its sheen, inset edges and darker footer. A flat
background crop would omit parts of the new header and footer. Like the previous
checker, this assumes the launcher is visible and its panel is the largest
connected matching region, without an adjoining desktop surface of the same
colors. Captures must retain that controlled condition.

Text checks follow the dark semantic roles. The `selected` check requires a
broad connected row wash and retains the 3,000-pixel minimum; a sheen, hover or
small neutral icon alone cannot pass. The `progress` check is confined to the
footer because progress and unavailable reasons now share the warning color.
The `subtitle` check excludes the heading and footer because subtitles and idle
hints now share the muted color. Extension drawing checks still use their
authored hex colors, with the original pixel thresholds and pointer lookup.
Result/error, absent-color, distinct-frame and same-frame checks remain active.

The same-frame comparison permits at most one interior pixel whose channels
each differ by at most one 8-bit level. This bound comes from
[CI run 36951745142](https://github.com/hoangvu12/pane/actions/runs/36951745142):
`4-result-2.png` and `25-search-result.png` both show the same TypeScript
command and answer, but exact equality failed on each platform. In Windows
artifact `11204826276`, the only difference within the existing 12-pixel edge
crop is `(178, 92, 46)` versus `(178, 92, 47)` at panel coordinate `(310, 406)`.
In macOS artifact `11204369046`, it is `(54, 55, 58)` versus `(53, 54, 58)` at
`(160, 116)`; other changes are confined to the already-excluded outer edge.
This is consistent with final color rounding. All other interior pixels must
still match exactly, and image dimensions must match. There is no text mask
or percentage allowance. Comparing the captured Rust or JavaScript answers
against the TypeScript answer still fails on both platforms. Offline tests
also reject a two-level change in one pixel and one-level changes in two pixels.

The install-preview wait now requires positive metadata evidence below the
package heading. The old root-search blue border no longer exists. This check
is only for the initial `--install` window: it assumes the prototype's default
760-logical-pixel width and scales the sampling band with the captured width
for display scaling. It is not a general resized-window screen classifier.

Offline checker regression tests use synthetic frames to exercise missing
results, desktop changes, missing selection, wrong-region text, preview waiting
at 1x/2x and unchanged custom swatches:

```sh
python -m unittest discover -s scripts -p test_check_screenshot.py
```

This script adaptation has not been validated by running the integrated
launcher. Per the Windows-first integration order, native desktop smokes and
the final all-OS CI run are deferred until the presentation and dependency
changes are integrated. In particular, confirm the preview sampling band
against real package captures on each OS before treating it as native evidence.
