"""The visual workbench's comparison (#91): native fixture vs authored reference.

Reads a native capture run (one directory per scenario: fixture-manifest.json,
native-run.json and the capture PNGs) and a reference capture run
(reference-manifest.json and its PNGs), measures both images, and writes:

- report.json: every check, in three sections -
  harness-native     the fixture's declared layout/colors vs what its
                     capture measures (validates the harness; should pass),
  harness-reference  the reference DOM's rects/classes vs what its capture
                     measures (validates the reference side; should pass),
  parity             native vs reference for the same component in the same
                     state (expected to fail broadly until #92-#99 land);
  a failed check that carries a recorded disposition (a measured platform
  limit a ticket explicitly accepted, with its reason) is counted as
  "accepted" rather than "failed" - it still fails, visibly, with its note;
  plus native-only captures (states the reference never authors) and, with
  --baseline, the sensitivity section: checks that passed in the baseline
  run and fail in this one, with both values.
- summary.md: the same, readable.
- per capture: side-by-side (native | reference), an amplified difference
  image and one crop pair per compared component - at 1:1, never rescaled.

Measurement rules (the ticket's proposed limits):
- edges and baselines: 1 logical px;
- deterministic flat fills: 2 channel levels. Translucent washes are
  compared as backdrop-relative overlay alpha - (fill - bg) / (255 - bg)
  for a white wash, (bg - fill) / bg for a black one - measured against
  the local background next to them, which survives the panels' different
  backdrops (opaque native panel, blurred wallpaper behind the glass);
- glyphs: only core pixels count (luma past 60% of the way from the local
  background to the text color), so antialiased edges never decide a
  check; glyph core colors use their own limit (4 levels) since glyph
  cores are not flat fills; the accent-colored caret is excluded from ink.
Images are compared at the same logical size or not at all: a size
mismatch is itself a failed check, never rescaled away.

Requires Pillow. Usage:
  python scripts/visual-workbench/compare.py --native <dir> --reference <dir> --out <dir>
      [--baseline <report.json>] [--label <name>]
"""
import argparse
import json
import statistics
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw

LIMITS = {"edge_px": 1.0, "flat_fill_levels": 2.0, "glyph_core_levels": 4.0}
# The muted subtitle text both sides author (#8E8F94).
MUTED = (142, 143, 148)
# What the measuring needs to know about a palette: whether its washes and
# rules are lighter than the panel (the dark palette's white overlays) or
# darker (the derived light palette's black ones), and the text colors ink
# is looked for in. The reference is always the dark one.
DARK = {"lighter": True, "muted": MUTED, "query": (243, 243, 245), "placeholder": (134, 135, 140),
        "alias": (185, 186, 190)}


def palette(manifest):
    """The native capture's palette, from its manifest's declared colors."""
    colors = manifest["declared"]["colors"]
    return {
        "lighter": manifest.get("appearance", "dark") == "dark",
        "muted": hex_rgba(colors["textMuted"])[:3],
        "query": hex_rgba(colors.get("textQuery", "#F3F3F5FF"))[:3],
        "placeholder": hex_rgba(colors["textPlaceholder"])[:3],
        "alias": hex_rgba(colors.get("aliasText", "#B9BABEFF"))[:3],
    }
CORE_FRACTION = 0.6
# Below this overlay alpha (levels) a row counts as having no wash: the
# hover wash is 3.5% (~9 levels), so half of it separates the two.
NO_WASH_LEVELS = 4.5

# ----------------------------------------------------------------- pixels


def load(path):
    return Image.open(path).convert("RGB")


def luma(color):
    r, g, b = color[:3]
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def is_accent(color):
    """The lime accent (the caret, the reference's Enter cap)."""
    r, g, b = color[:3]
    return g > 150 and g - b > 60 and g - r > 15


def pixels(image, box):
    """The RGB pixels inside box = (left, top, right, bottom), clamped."""
    left, top, right, bottom = clamp_box(image, box)
    data = image.load()
    return [data[x, y] for y in range(top, bottom) for x in range(left, right)]


def clamp_box(image, box):
    left, top, right, bottom = (int(round(v)) for v in box)
    return (max(0, left), max(0, top), min(image.width, right), min(image.height, bottom))


def median_color(image, box):
    values = pixels(image, box)
    if not values:
        return None
    return tuple(statistics.median(channel) for channel in zip(*values))


def white_alpha(fill, background):
    """A white overlay's alpha, in levels (0-255), from the composited fill
    and the background it sits on: (fill - bg) / (255 - bg) per channel."""
    estimates = [
        (f - b) / (255 - b) * 255 for f, b in zip(fill, background) if 255 - b > 8
    ]
    return statistics.mean(estimates) if estimates else None


def black_alpha(fill, background):
    """A black overlay's alpha, in levels: (bg - fill) / bg per channel."""
    estimates = [(b - f) / b * 255 for f, b in zip(fill, background) if b > 8]
    return statistics.mean(estimates) if estimates else None


def lit_run(lit, center, gap=1):
    """[start, end) of the run of lit samples holding center (or the lit
    sample nearest it), bridging single unlit samples - a thin dark glyph
    stroke inside a wash does not split it, while the list's 2px gap still
    separates one row's wash from its neighbor's."""
    indices = [i for i, value in enumerate(lit) if value]
    if not indices:
        return None
    start = end = min(indices, key=lambda i: abs(i - center))
    while True:
        ahead = [i for i in range(end + 1, min(len(lit), end + 2 + gap)) if lit[i]]
        if not ahead:
            break
        end = ahead[-1]
    while True:
        behind = [i for i in range(max(0, start - 1 - gap), start) if lit[i]]
        if not behind:
            break
        start = behind[0]
    return start, end + 1


def wash_edges(image, approx, background, min_delta, margin=8, lighter=True):
    """The rect of a lighter wash around approx = (x, y, w, h).

    Left and right: along rows through its middle, the outermost pixels
    brighter than the background by min_delta (darker, for a darkening
    wash: lighter=False) - nothing else sits in the
    list's side padding, while inside the wash a tile's drop shadow can be
    darker than the wash. Top and bottom: along columns through its
    interior, the run of such pixels that holds the approximate center - a
    neighbor's wash two pixels away, or a rule above, is a separate run.
    The medians over all the scan lines are the edges. Returns (x, y, w, h)
    or None."""
    x, y, w, h = approx
    bg = luma(background)
    sign = 1 if lighter else -1
    data = image.load()
    lefts, rights, tops, bottoms = [], [], [], []
    xs = range(max(0, int(x - margin)), min(image.width, int(x + w + margin)))
    ys = range(max(0, int(y - margin)), min(image.height, int(y + h + margin)))
    for row in range(int(y + h * 0.35), int(y + h * 0.65) + 1):
        if 0 <= row < image.height:
            lit = [sign * (luma(data[c, row]) - bg) >= min_delta for c in xs]
            if any(lit):
                lefts.append(xs.start + lit.index(True))
                rights.append(xs.start + len(lit) - lit[::-1].index(True))
    inset = min(14, w / 4)
    step = max(1, int((w - 2 * inset) / 24))
    for column in range(int(x + inset), int(x + w - inset), step):
        if 0 <= column < image.width:
            run = lit_run([sign * (luma(data[column, r]) - bg) >= min_delta for r in ys], (y + h / 2) - ys.start)
            if run:
                tops.append(ys.start + run[0])
                bottoms.append(ys.start + run[1])
    if not (lefts and rights and tops and bottoms):
        return None
    left, right = statistics.median(lefts), statistics.median(rights)
    top, bottom = statistics.median(tops), statistics.median(bottoms)
    return (left, top, right - left, bottom - top)


def accent_pixels(image, box):
    return sum(1 for color in pixels(image, box) if is_accent(color))


def ink(image, box, background, text_color, exclude_accent=True):
    """Core glyph pixels inside box: luma past CORE_FRACTION of the way
    from the background to text_color. Returns the core pixels' bounding
    box (x, y, w, h) and their median color, or None."""
    left, top, right, bottom = clamp_box(image, box)
    bg, fg = luma(background), luma(text_color)
    threshold = bg + CORE_FRACTION * (fg - bg)
    data = image.load()
    xs, ys, colors = [], [], []
    for row in range(top, bottom):
        for column in range(left, right):
            color = data[column, row]
            if exclude_accent and is_accent(color):
                continue
            if (luma(color) >= threshold) if fg >= bg else (luma(color) <= threshold):
                xs.append(column)
                ys.append(row)
                colors.append(color)
    if not xs:
        return None
    # The color of the fullest cores: the top 5% by contrast, where the
    # glyph covers its pixels completely if anywhere.
    colors.sort(key=lambda c: abs(luma(c) - bg), reverse=True)
    core = colors[: max(1, len(colors) // 20)]
    return {
        "box": (min(xs), min(ys), max(xs) + 1 - min(xs), max(ys) + 1 - min(ys)),
        "color": tuple(statistics.median(channel) for channel in zip(*core)),
        "count": len(xs),
    }


def ink_top(image, box, background, text_color):
    """The first row inside box that glyph ink covers: where two
    neighboring pixels' coverage (how far each is from the background
    toward text_color) adds up past CORE_FRACTION. A thin stem straddling
    two columns covers neither pixel fully, so ink() alone misses it, and
    where it falls depends on the renderer's subpixel phase, not on where
    the glyph sits. Returns the row in image pixels, or None."""
    left, top, right, bottom = clamp_box(image, box)
    bg, fg = luma(background), luma(text_color)
    if fg == bg:
        return None
    data = image.load()
    for row in range(top, bottom):
        cover = [min(1.0, max(0.0, (luma(data[column, row]) - bg) / (fg - bg)))
                 for column in range(left, right)]
        if any(a + b >= CORE_FRACTION for a, b in zip(cover, cover[1:])):
            return row
    return None


def hline(image, x_range, y_range, background=None, lighter=True):
    """The y of the most distinct horizontal line across x_range within
    y_range (a hairline rule): the row whose median luma stands out most
    from the rows two pixels above and below it - a line, not a gradient
    or a step. The caller keeps y_range to a few pixels around where the
    rule belongs, so a wash's edge nearby is never taken for it."""
    data = image.load()
    # A narrow client is narrower than the usual stretch.
    columns = range(max(0, x_range[0]), min(image.width, x_range[1]), 3)

    def level(row):
        return statistics.median(luma(data[c, row]) for c in columns)

    # A hairline rule is lighter than the panel on both of its sides (darker,
    # in the light palette); the edge of a wash below it is lighter on one
    # side only.
    best, best_y = 0.0, None
    for row in range(max(2, y_range[0]), min(image.height - 2, y_range[1])):
        above, here, below = level(row - 2), level(row), level(row + 2)
        contrast = min(here - above, here - below) if lighter else min(above - here, below - here)
        if contrast > best:
            best, best_y = contrast, row
    return best_y


# ----------------------------------------------------------------- checks


class Report:
    def __init__(self):
        self.checks = []

    def check(self, section, scenario, capture, subject, prop, measured, expected, limit, unit, note=None,
              accepted=None):
        delta = None
        passed = False
        if measured is None or expected is None:
            note = (note + "; " if note else "") + "not measurable"
        elif isinstance(measured, str) or isinstance(expected, str):
            passed = measured == expected
        else:
            delta = round(measured - expected, 2)
            passed = abs(delta) <= limit
        entry = {
            "id": f"{section}/{scenario}/{capture}/{subject}/{prop}",
            "section": section,
            "scenario": scenario,
            "capture": capture,
            "subject": subject,
            "property": prop,
            "measured" if section != "parity" else "native": _round(measured),
            "expected" if section != "parity" else "reference": _round(expected),
            "delta": delta,
            "limit": limit,
            "unit": unit,
            "passed": passed,
        }
        if note:
            entry["note"] = note
        if accepted and not passed:
            entry["accepted"] = accepted
        self.checks.append(entry)
        return entry


def _round(value):
    if isinstance(value, float):
        return round(value, 2)
    if isinstance(value, tuple):
        return [_round(v) for v in value]
    return value


def hex_rgba(text):
    text = text.lstrip("#")
    return tuple(int(text[i : i + 2], 16) for i in (0, 2, 4, 6))


def css_rgba(text):
    """'rgba(255, 255, 255, 0.086)' or 'rgb(...)' -> (r, g, b, a 0-255)."""
    inner = text[text.index("(") + 1 : text.rindex(")")]
    parts = [float(p) for p in inner.replace("/", ",").split(",")]
    alpha = parts[3] if len(parts) > 3 else 1.0
    return (*parts[:3], alpha * 255)


def scaled(rect, scale):
    return tuple(v * scale for v in rect)


def as_tuple(rect):
    if rect is None:
        return None
    if isinstance(rect, dict):
        return (rect["x"], rect["y"], rect["width"], rect["height"])
    return tuple(rect)


# --------------------------------------------------------------- measuring


def panel_background(image, y, scale=1.0):
    """The panel's local background beside the list at height y: the list's
    left padding, clear of rows (x 3-8 logical)."""
    return median_color(image, (3 * scale, y - 3 * scale, 8 * scale, y + 3 * scale))


def row_wash(image, rect, scale=1.0, floor=False, lighter=True):
    """A row's wash, measured column by column: the strip just under its
    top edge (above the tile and the text) against the list gaps just above
    and below the row, interpolated to the strip's height. The panels'
    backdrops vary across and down the panel (the glass shows a blurred
    wallpaper under a fading sheen), so each column's wash is measured
    against the background directly beside it; the median over the columns
    is the row's. A row whose height is only a floor (it may grow) uses the
    gap above alone. Returns the median fill, background and overlay alpha
    (levels)."""
    x, y, w, h = scaled(rect, scale)
    data = image.load()
    top, bottom = int(round(y)), int(round(y + h))
    strip = top + int(round(4 * scale))
    if top - 1 < 0 or bottom >= image.height:
        return None
    weight = 0.0 if floor else (strip - (top - 1)) / (bottom - (top - 1))
    alphas, fills, backgrounds = [], [], []
    for column in range(int(x + 14 * scale), int(x + w - 14 * scale)):
        if not 0 <= column < image.width:
            continue
        above = data[column, top - 1]
        below = above if floor else data[column, bottom]
        background = tuple(a + (b - a) * weight for a, b in zip(above, below))
        fill = tuple(statistics.mean(data[column, r][i] for r in (strip - 1, strip, strip + 1)) for i in range(3))
        alpha = overlay_alpha(fill, background, lighter)
        if alpha is not None:
            alphas.append(alpha)
            fills.append(fill)
            backgrounds.append(background)
    if not alphas:
        return None
    return {
        "fill": tuple(statistics.median(c) for c in zip(*fills)),
        "background": tuple(statistics.median(c) for c in zip(*backgrounds)),
        "alpha": statistics.median(alphas),
    }


def measure_row(image, rect, washed, text_color, scale=1.0, floor=False, pal=DARK):
    """What the image shows for a row at the given rect."""
    result = {}
    lighter = pal["lighter"]
    wash = row_wash(image, rect, scale, floor, lighter)
    if wash is None:
        return result
    result["alpha"] = wash["alpha"]
    if washed:
        sign = 1 if lighter else -1
        delta = max(3.0, sign * (luma(wash["fill"]) - luma(wash["background"])) / 2)
        edges = wash_edges(image, scaled(rect, scale), wash["background"], delta, lighter=lighter)
        if edges:
            result["edges"] = tuple(v / scale for v in edges)
    x, y, w, h = scaled(rect, scale)
    title_box = (x + 44 * scale, y + 4 * scale, x + w * 0.55, y + h - 4 * scale)
    # Row titles keep accent pixels (the reference paints the matched part
    # of a title in the accent); the header's caret is excluded instead.
    found = ink(image, title_box, wash["fill"], text_color, exclude_accent=False)
    if found:
        bx, by, bw, bh = found["box"]
        result["title"] = {
            "left": (bx - x) / scale,
            "top": (by - y) / scale,
            "height": bh / scale,
            "color": found["color"],
        }
        # The subtitle: muted ink after the title's last core column (the
        # title's own pixels are brighter, so they are left out by position).
        sub = ink(image, (bx + bw + scale, y + 4 * scale, x + w * 0.7, y + h - 4 * scale), wash["fill"],
                  pal["muted"], exclude_accent=False)
        if sub:
            result["subtitleGap"] = (sub["box"][0] - (bx + bw)) / scale
    result["accent"] = accent_pixels(image, title_box)
    return result


def measure_header(image, rule_y, scale=1.0, pal=DARK):
    """The search header: its rule (searched within 4 px of rule_y) and
    the query's or placeholder's ink (the caret excluded)."""
    background = median_color(image, (300 * scale, 4 * scale, 700 * scale, 8 * scale))
    rule = hline(image, (int(200 * scale), int(600 * scale)),
                 (int((rule_y - 4) * scale), int((rule_y + 5) * scale)), background, pal["lighter"])
    text = ink(image, (44 * scale, 8 * scale, 500 * scale, 56 * scale), background, pal["query"])
    if text is None:  # the placeholder's muted grey
        text = ink(image, (44 * scale, 8 * scale, 500 * scale, 56 * scale), background, pal["placeholder"])
    return {
        "rule": rule / scale if rule is not None else None,
        "text": {
            "left": text["box"][0] / scale,
            "top": text["box"][1] / scale,
            "height": text["box"][3] / scale,
        }
        if text
        else None,
    }


def measure_footer(image, rule_y, scale=1.0, lighter=True):
    """The footer: its top rule (searched within 4 px of rule_y) and its
    tint, as a black overlay over the panel just above the rule. Both are
    read in the list's left padding (x 3-8), which no row, wash or footer
    control reaches - a selected row scrolled flush with the list's bottom
    edge would otherwise be taken for the panel above the rule."""
    background = median_color(image, (3 * scale, (rule_y - 4) * scale, 8 * scale, (rule_y - 2) * scale))
    rule = hline(image, (int(40 * scale), int(300 * scale)),
                 (int((rule_y - 4) * scale), int((rule_y + 5) * scale)), background, lighter)
    if rule is None:
        return {"rule": None, "alpha": None}
    fill = median_color(image, (3 * scale, rule + 4 * scale, 8 * scale, rule + 6 * scale))
    return {"rule": rule / scale, "alpha": black_alpha(fill, background), "fill": fill, "background": background}


def measure_cap(image, rect, scale=1.0, lighter=True):
    """A keycap's measured box and fill alpha near its declared rect."""
    x, y, w, h = scaled(rect, scale)
    background = median_color(image, (x - 8 * scale, y + h * 0.3, x - 3 * scale, y + h * 0.7))
    if background is None:
        return None
    fill = median_color(image, (x + 1.5 * scale, y + 2 * scale, x + 3 * scale, y + h - 2 * scale))
    if fill is None:
        return None
    sign = 1 if lighter else -1
    delta = max(3.0, sign * (luma(fill) - luma(background)) / 2)
    edges = wash_edges(image, (x, y, w, h), background, delta, margin=5, lighter=lighter)
    return {
        "edges": tuple(v / scale for v in edges) if edges else None,
        "alpha": overlay_alpha(fill, background, lighter) if fill else None,
    }


# ------------------------------------------------------------- comparing


def compare(native_dir, reference_dir, out_dir, label):
    report = Report()
    reference = json.loads(Path(reference_dir, "reference-manifest.json").read_text(encoding="utf-8-sig"))
    ref_scenarios = {s["name"]: s for s in reference.get("scenarios", [])}
    images = []
    native_only = []
    meta = {"label": label, "native": str(native_dir), "reference": str(reference_dir), "limits": LIMITS,
            "referenceSha256": reference.get("sha256"), "chrome": reference.get("chrome"), "scenarios": []}

    # The reference boards' own sizes: the three canonical clients.
    for board in reference.get("boards", []):
        if board.get("glass"):
            report.check("harness-reference", "boards", board["slug"], "glass", "size",
                         f"{board['image'][0]}x{board['image'][1]}", f"{board['glass'][2]}x{board['glass'][3]}", 0, "px")

    for scenario_dir in sorted(p for p in Path(native_dir).iterdir() if (p / "fixture-manifest.json").exists()):
        manifest = json.loads((scenario_dir / "fixture-manifest.json").read_text(encoding="utf-8-sig"))
        run = json.loads((scenario_dir / "native-run.json").read_text(encoding="utf-8-sig"))
        scenario = manifest["scenario"]
        name = scenario["name"]
        scale = (run.get("dpi") or 96) / 96
        colors = manifest["declared"]["colors"]
        text_title = hex_rgba(colors["textTitle"])[:3]
        selected_alpha = hex_rgba(colors["rowSelected"])[3]
        hover_alpha = hex_rgba(colors["rowHover"])[3]
        meta["scenarios"].append({"name": name, "dpi": run.get("dpi"), "client": run.get("client"),
                                  "material": manifest["material"], "perturbation": manifest["perturbation"]})
        ref = ref_scenarios.get(name)
        check_fonts(report, name, manifest)
        if not scenario["reference"]:
            native_only.append({"scenario": name, "captures": [c["file"] for c in run["captures"]],
                                "reason": "the reference authors no such state; captured as a native adaptation, not compared"})
        for declared in manifest["captures"]:
            capture = declared["name"]
            native_path = scenario_dir / f"{capture}.png"
            if not native_path.exists():
                report.check("harness-native", name, capture, "capture", "exists", "missing", "present", 0, "")
                continue
            native = load(native_path)
            client = scenario["client"]
            report.check("harness-native", name, capture, "client", "size",
                         f"{native.width / scale:g}x{native.height / scale:g}", f"{client[0]:g}x{client[1]:g}", 0, "logical px")
            ref_capture = None
            reference_image = None
            if ref:
                ref_capture = next((c for c in ref["captures"] if c["name"] == capture), None)
                if ref_capture:
                    reference_image = load(Path(reference_dir, ref_capture["file"]))
                    report.check("parity", name, capture, "client", "size",
                                 f"{native.width / scale:g}x{native.height / scale:g}",
                                 f"{reference_image.width}x{reference_image.height}", 0, "logical px",
                                 "compared at 1:1; never rescaled")
            crops = []
            if scenario["family"] == "root":
                compare_root(report, name, capture, declared, manifest, native, scale, ref_capture,
                             reference_image, text_title, selected_alpha, hover_alpha, crops)
            elif scenario["family"] == "tiles":
                compare_tiles(report, name, capture, manifest, native, scale, crops)
            else:
                compare_keycaps(report, name, capture, manifest, native, scale, ref_capture, reference_image, crops)
            images.append(write_images(out_dir, name, capture, native, reference_image, crops, scale))

    sections = {}
    for check in report.checks:
        section = sections.setdefault(check["section"], {"passed": 0, "failed": 0, "accepted": 0})
        section["passed" if check["passed"] else "accepted" if check.get("accepted") else "failed"] += 1
    return {"meta": meta, "totals": sections, "checks": report.checks, "nativeOnly": native_only,
            "images": images, "pending": pending(native_dir)}


def pending(native_dir):
    summary = Path(native_dir, "registry.json")
    if summary.exists():
        return json.loads(summary.read_text(encoding="utf-8-sig")).get("pending", [])
    return []


def compare_root(report, name, capture, declared, manifest, native, scale, ref_capture, reference_image,
                 text_title, selected_alpha, hover_alpha, crops):
    # ---- the native side against its declaration
    pal = palette(manifest)
    # While the Actions panel is open, the results lie under its dimmer and
    # their right ends under the panel: the rows' own checks belong to the
    # captures with it closed.
    covered = bool(declared.get("actions"))
    native_rows = {}
    for row in declared["rows"]:
        if covered:
            break
        if row["heightIsFloor"] and row["unavailable"] is None:
            continue
        if not row.get("visible", True):
            continue
        rect = as_tuple(row["rect"])
        washed = row["selected"] or row["hovered"]
        measured = measure_row(native, rect, washed, text_title, scale, floor=row["heightIsFloor"], pal=pal)
        native_rows[row["title"]] = (row, measured)
        subject = f"row:{row['title']}"
        if row["selected"]:
            expected_alpha, state = selected_alpha, "selected"
        elif row["hovered"]:
            expected_alpha, state = hover_alpha, "hovered"
        else:
            expected_alpha, state = 0, "rest"
        report.check("harness-native", name, capture, subject, f"wash alpha ({state})",
                     measured.get("alpha"), expected_alpha, LIMITS["flat_fill_levels"], "levels")
        if washed and not row["heightIsFloor"]:
            edges = measured.get("edges")
            for prop, index in (("left", 0), ("top", 1), ("width", 2), ("height", 3)):
                report.check("harness-native", name, capture, subject, f"wash {prop}",
                             edges[index] if edges else None, rect[index], LIMITS["edge_px"], "px")
        crops.append(("row-" + row["title"], rect, None))
    search = as_tuple(manifest["searchHeader"])
    header = measure_header(native, search[1] + search[3] - 1, scale, pal)
    report.check("harness-native", name, capture, "search-header", "rule y",
                 header["rule"], search[1] + search[3] - 1, LIMITS["edge_px"], "px")
    footer_rect = as_tuple(manifest["footer"])
    footer = measure_footer(native, footer_rect[1], scale, pal["lighter"])
    report.check("harness-native", name, capture, "footer", "rule y", footer["rule"], footer_rect[1], LIMITS["edge_px"], "px")
    # The tint is read against the list just above the rule, which the
    # open panel's dimmer darkens.
    if not covered:
        report.check("harness-native", name, capture, "footer", "tint alpha", footer["alpha"],
                     hex_rgba(manifest["declared"]["colors"]["footerTint"])[3], LIMITS["flat_fill_levels"], "levels")
    if declared.get("actionButton"):
        # The footer's buttons are transparent at rest (`.fbtn`), so they
        # have no edge to measure: their label starts at their padding, and
        # their keys are measured as key groups.
        button = as_tuple(declared["actionButton"])
        background = footer_strip_background(native, footer_rect, scale)
        label = ink_extent(native, (button[0], button[1], button[2] * 0.6, button[3]), background,
                           hex_rgba(manifest["declared"]["colors"]["footerButtonText"])[:3], scale)
        report.check("harness-native", name, capture, "footer-primary", "label ink left", label and label["left"],
                     button[0] + manifest["declared"]["geometry"].get("actionPaddingX", 8), LIMITS["edge_px"] * 1.5,
                     "px", "a glyph's side bearing allowed")
        measured = measure_cap(native, button, scale, pal["lighter"])
        report.check("harness-native", name, capture, "footer-primary", "wash alpha (rest)",
                     measured and measured["alpha"], 0, LIMITS["flat_fill_levels"], "levels")
        crops.append(("footer-primary", button, None))
    harness_footer(report, name, capture, declared, manifest, native, scale, pal, crops)
    harness_actions(report, name, capture, declared, manifest, native, scale, pal, crops)
    crops.append(("search-header", search, None))
    if manifest["scenario"].get("frame"):
        compare_frame(report, name, capture, declared, manifest, native, scale, ref_capture, reference_image, crops)
    for row in declared["rows"]:
        if row.get("visible", True) and not row["heightIsFloor"] and not covered:
            harness_row_trailing(report, name, capture, row, manifest, native, scale, crops)

    if ref_capture is None:
        return
    # ---- the reference side against its DOM
    state = ref_capture["state"]
    covered = covered or bool(state.get("panel"))
    ref_rows = {}
    for row in state["rows"]:
        if not row["visible"] or covered:
            continue
        rect = as_tuple(row["rect"])
        washed = row["selected"] or row["hovered"]
        measured = measure_row(reference_image, rect, washed, text_title)
        ref_rows[row["title"]] = (row, measured)
        subject = f"row:{row['title']}"
        dom_alpha = css_rgba(row["background"])[3]
        report.check("harness-reference", name, capture, subject, "wash alpha vs DOM background",
                     measured.get("alpha"), dom_alpha, LIMITS["flat_fill_levels"], "levels",
                     "the DOM serializes .085 as .086")
        if row["selected"]:
            edges = measured.get("edges")
            for prop, index in (("left", 0), ("top", 1), ("width", 2), ("height", 3)):
                report.check("harness-reference", name, capture, subject, f"wash {prop}",
                             edges[index] if edges else None, rect[index], LIMITS["edge_px"], "px")
    dom_header = as_tuple(state["searchHeader"])
    ref_header = measure_header(reference_image, dom_header[1] + dom_header[3] - 1)
    report.check("harness-reference", name, capture, "search-header", "rule y", ref_header["rule"],
                 dom_header[1] + dom_header[3] - 1, LIMITS["edge_px"], "px")
    dom_footer = as_tuple(state["footer"])
    ref_footer = measure_footer(reference_image, dom_footer[1])
    report.check("harness-reference", name, capture, "footer", "rule y", ref_footer["rule"], dom_footer[1],
                 LIMITS["edge_px"], "px")

    # ---- parity: the same component in the same state
    native_selected = sorted(r["title"] for r in declared["rows"] if r["selected"] and r.get("visible", True))
    ref_selected = sorted(r["title"] for r in state["rows"] if r["selected"])
    report.check("parity", name, capture, "selection", "selected rows", ", ".join(native_selected),
                 ", ".join(ref_selected), 0, "", "the pointer's movement selects the row it moves over, on both sides (#94)")
    native_hovered = sorted(r["title"] for r in declared["rows"]
                            if r["hovered"] and not r["selected"] and r.get("visible", True))
    ref_hover_only = sorted(r["title"] for r in state["rows"] if r["hovered"] and not r["selected"])
    report.check("parity", name, capture, "selection", "hover-only rows", ", ".join(native_hovered),
                 ", ".join(ref_hover_only), 0, "")
    for title, (row, measured) in native_rows.items():
        if title not in ref_rows:
            continue
        ref_row, ref_measured = ref_rows[title]
        subject = f"row:{title}"
        n_rect, r_rect = as_tuple(row["rect"]), as_tuple(ref_row["rect"])
        crops.append(("parity-row-" + title, n_rect, r_rect))
        report.check("parity", name, capture, subject, "wash alpha", measured.get("alpha"),
                     ref_measured.get("alpha"), LIMITS["flat_fill_levels"], "levels")
        n_edges, r_edges = measured.get("edges"), ref_measured.get("edges")
        # A hover-only wash is too faint to edge on the reference's glass,
        # whose blurred wallpaper shades across the row: its alpha is
        # compared, its edges only where the row is selected.
        if n_edges and r_edges and row["selected"]:
            for prop, index in (("left", 0), ("width", 2), ("height", 3)):
                report.check("parity", name, capture, subject, f"wash {prop}", n_edges[index], r_edges[index],
                             LIMITS["edge_px"], "px")
        report.check("parity", name, capture, subject, "top in client", n_rect[1], r_rect[1], LIMITS["edge_px"], "px",
                     "the reference lists the pinned strip and section labels above the rows (#101)")
        n_title, r_title = measured.get("title"), ref_measured.get("title")
        if n_title and r_title:
            report.check("parity", name, capture, subject, "title ink left in row", n_title["left"], r_title["left"],
                         LIMITS["edge_px"], "px")
            report.check("parity", name, capture, subject, "title ink top in row", n_title["top"], r_title["top"],
                         LIMITS["edge_px"], "px")
            report.check("parity", name, capture, subject, "title ink height", n_title["height"], r_title["height"],
                         LIMITS["edge_px"], "px")
            worst = max(abs(a - b) for a, b in zip(n_title["color"], r_title["color"]))
            report.check("parity", name, capture, subject, "title glyph core color (max channel)", worst, 0,
                         LIMITS["glyph_core_levels"], "levels",
                         f"native {fmt_color(n_title['color'])} vs reference {fmt_color(r_title['color'])}")
        if measured.get("subtitleGap") is not None and ref_measured.get("subtitleGap") is not None:
            report.check("parity", name, capture, subject, "title-to-subtitle ink gap", measured["subtitleGap"],
                         ref_measured["subtitleGap"], LIMITS["edge_px"], "px")
        compare_row_tile(report, name, capture, subject, row, ref_row, manifest, native, scale, reference_image,
                         measured, ref_measured, crops)
        compare_row_trailing(report, name, capture, subject, row, ref_row, manifest, native, scale,
                             reference_image, measured, ref_measured, crops)
        if state["query"]:
            report.check("parity", name, capture, subject, "title match highlighted",
                         "yes" if measured.get("accent") else "no", "yes" if ref_measured.get("accent") else "no", 0, "",
                         "the reference paints the matched part of a title in the accent")
    report.check("parity", name, capture, "search-header", "rule y", header["rule"], ref_header["rule"], LIMITS["edge_px"], "px")
    if header["text"] and ref_header["text"]:
        for prop in ("left", "top", "height"):
            report.check("parity", name, capture, "search-header", f"query/placeholder ink {prop}",
                         header["text"][prop], ref_header["text"][prop], LIMITS["edge_px"], "px",
                         None if state["query"] else "placeholder copy is adapted: 'Search apps and commands…' vs 'Search apps, commands, plugins…' (#92)")
    report.check("parity", name, capture, "footer", "rule y", footer["rule"], ref_footer["rule"], LIMITS["edge_px"], "px")
    if not covered:
        report.check("parity", name, capture, "footer", "tint alpha", footer["alpha"], ref_footer["alpha"],
                     LIMITS["flat_fill_levels"], "levels")
    if declared.get("actionButton") and state.get("footerPrimary"):
        button = as_tuple(declared["actionButton"])
        ref_button = as_tuple(state["footerPrimary"]["rect"])
        native_label = declared.get("action") or ""
        report.check("parity", name, capture, "footer-primary", "label", native_label,
                     state["footerPrimary"]["label"].rstrip("↵").strip(), 0, "")
        report.check("parity", name, capture, "footer-primary", "button height", button[3], ref_button[3],
                     LIMITS["edge_px"], "px")
        ref_cap = measure_cap(reference_image, ref_button)
        native_cap = measure_cap(native, button, scale)
        report.check("parity", name, capture, "footer-primary", "button fill alpha",
                     native_cap and native_cap["alpha"], ref_cap and ref_cap["alpha"], LIMITS["flat_fill_levels"], "levels",
                     "the reference's footer buttons are transparent at rest")
        crops.append(("parity-footer-primary", button, ref_button))
    ref_groups = state.get("keycaps", {})
    for group in manifest.get("keycaps", []):
        footer_group = group.get("group") in ("footer-primary", "footer-actions")
        static_board = len((state.get("footerParts") or {}).get("hint") or []) == 1
        compare_key_group(report, name, capture, group, manifest, native, scale,
                          ref_groups.get(group.get("group")), reference_image, crops,
                          chained=group.get("group") == "footer-primary", footer=footer_group,
                          static=STATIC_ENTER if static_board and group.get("group") == "footer-primary" else None)
    if not covered:
        compare_sections(report, name, capture, declared, state, native, scale, reference_image, crops)
    parity_footer(report, name, capture, declared, manifest, native, scale, state, reference_image, pal, crops)
    parity_actions(report, name, capture, declared, manifest, native, scale, state, reference_image, pal, crops)


# The launcher frame (#92). Windows rounds the launcher's window itself,
# through the Desktop Window Manager; a painted 18px radius would show the
# window's acrylic (which covers the whole window rectangle) as a plate
# behind the curve. PrintWindow reads the client before that clip, so the
# capture's corners are square.
CORNER_DISPOSITION = ("accepted (#92): the panel paints no radius on Windows - the acrylic backdrop covers the "
                      "whole window rectangle, so a painted 18px curve would show it as a plate - and the DWM's "
                      "corner preference (DWMWCP_ROUND, Microsoft's documented 8px) rounds the window instead. "
                      "This capture is PrintWindow's, taken before the DWM clip, so it shows the panel's square "
                      "corner, not the 8px the screen shows; the on-screen corner is documented, not measured here")


def overlay_alpha(fill, background, lighter):
    """A wash's alpha: a white overlay's when it lightens the panel, a black
    one's when it darkens it."""
    return white_alpha(fill, background) if lighter else black_alpha(fill, background)


def edge_alpha(image, edge, scale=1.0, lighter=True):
    """The panel's inset ring on one edge of the client, as an overlay alpha
    (levels): the outermost pixel line against the line two pixels in,
    median over a stretch of the edge clear of content - the list's side
    padding for the left and right edges, the header's top for the top,
    the footer's empty middle for the bottom."""
    width, height = image.width, image.height
    data = image.load()
    if edge in ("left", "right"):
        x_out = 0 if edge == "left" else width - 1
        x_in = 2 if edge == "left" else width - 3
        pairs = [(data[x_out, y], data[x_in, y]) for y in range(int(100 * scale), min(height, int(400 * scale)), 2)]
    else:
        y_out = 0 if edge == "top" else height - 1
        y_in = 2 if edge == "top" else height - 3
        lo, hi = ((100, 700) if edge == "top" else (320, 460))
        hi = min(hi, width / scale - 20)
        pairs = [(data[x, y_out], data[x, y_in]) for x in range(int(lo * scale), int(hi * scale), 3)]
    alphas = [a for a in (overlay_alpha(f, b, lighter) for f, b in pairs) if a is not None]
    return statistics.median(alphas) if alphas else None


def corner_inset(image, scale=1.0):
    """How far along the top-left corner's diagonal the panel begins, in
    logical px: 0 for a square corner; about r(1 - 1/sqrt 2) for a corner
    rounded at radius r (18px -> ~5.3). A diagonal pixel belongs to the
    panel once its luma is nearer the panel's (beside the left edge, below
    the curve) than the corner pixel's (outside any curve). A corner pixel
    as bright as the left edge's ring (or brighter: the ring under the top
    highlight) is the panel's own: a square corner."""
    data = image.load()
    inside = statistics.median(luma(data[x, y]) for x in range(int(3 * scale), int(8 * scale))
                               for y in range(int(30 * scale), int(34 * scale)))
    ring = statistics.median(luma(data[0, y]) for y in range(int(100 * scale), int(300 * scale)))
    outside = luma(data[0, 0])
    if abs(inside - outside) < 2 or outside >= ring - 3:
        return 0.0
    for t in range(0, int(20 * scale)):
        level = luma(data[t, t])
        if abs(level - inside) < abs(level - outside) or level > max(inside, outside):
            return t / scale
    return None


def compare_frame(report, name, capture, declared, manifest, native, scale, ref_capture, reference_image, crops):
    """The launcher frame: the panel's inset edges, its corner, and where
    the header, the list and the footer begin and end."""
    colors = manifest["declared"]["colors"]
    ring = hex_rgba(colors["hairline"])
    top = hex_rgba(colors.get("panelTopHighlight", "#00000000"))
    lighter = palette(manifest)["lighter"]
    for edge in ("left", "right"):
        report.check("harness-native", name, capture, "frame", f"{edge} edge ring alpha",
                     edge_alpha(native, edge, scale, lighter), ring[3], LIMITS["flat_fill_levels"], "levels",
                     "the panel's inset 1px ring")
    # The top edge is one overlay over another only where both lighten (the
    # light palette's ring darkens under a lightening highlight).
    if lighter and sum(top[:3]) > 384:
        combined = 255 * (1 - (1 - ring[3] / 255) * (1 - top[3] / 255))
        report.check("harness-native", name, capture, "frame", "top edge alpha", edge_alpha(native, "top", scale),
                     combined, LIMITS["flat_fill_levels"], "levels", "the ring under the top inset highlight")
    width, height = manifest["scenario"]["client"]
    search, list_rect, footer = (as_tuple(manifest[k]) for k in ("searchHeader", "list", "footer"))
    report.check("harness-native", name, capture, "frame", "header + list + footer height",
                 search[3] + list_rect[3] + footer[3], height, 0, "px",
                 f"{search[3]:g} + {list_rect[3]:g} + {footer[3]:g}: no border consumes the panel's layout")
    for label, rect in (("search header", search), ("list", list_rect), ("footer", footer)):
        report.check("harness-native", name, capture, "frame", f"{label} spans the width",
                     rect[2] if rect[0] == 0 else None, width, 0, "px")
    crops.append(("frame-corner", (0, 0, 40, 40), None))
    if ref_capture is None:
        return
    state = ref_capture["state"]
    for edge in ("left", "right", "top", "bottom"):
        report.check("parity", name, capture, "frame", f"{edge} edge alpha", edge_alpha(native, edge, scale),
                     edge_alpha(reference_image, edge), LIMITS["flat_fill_levels"], "levels",
                     "inset ring (and top highlight); the footer's wash covers the ring at the bottom")
    n_corner, r_corner = corner_inset(native, scale), corner_inset(reference_image)
    report.check("parity", name, capture, "frame", "top-left corner diagonal inset", n_corner, r_corner,
                 LIMITS["edge_px"], "px", "radius ~ inset x 3.41", accepted=CORNER_DISPOSITION)
    crops.append(("parity-frame-corner", (0, 0, 40, 40), (0, 0, 40, 40)))
    glass = as_tuple(state["glass"])
    report.check("parity", name, capture, "frame", "panel size", f"{width:g}x{height:g}", f"{glass[2]:g}x{glass[3]:g}", 0, "")
    for label, n_rect, key in (("search header", search, "searchHeader"), ("list", list_rect, "list"),
                               ("footer", footer, "footer")):
        r_rect = as_tuple(state[key])
        for prop, index in (("top", 1), ("height", 3), ("left", 0), ("width", 2)):
            report.check("parity", name, capture, "frame", f"{label} {prop}", n_rect[index], r_rect[index],
                         LIMITS["edge_px"], "px")
    # The footer's content ends with the Actions button's last key, on
    # both sides: its cap has a fill to edge (the `.fbtn` itself is
    # transparent at rest).
    native_keys = next((g for g in manifest.get("keycaps", []) if g.get("group") == "footer-actions"), None)
    ref_keys = (state.get("keycaps") or {}).get("footer-actions")
    if native_keys and ref_keys:
        mine = measure_key(native, as_tuple(native_keys["caps"][-1]["rect"]), False,
                           hex_rgba(manifest["declared"]["colors"]["keycapText"])[:3], scale, palette(manifest)["lighter"])
        theirs = measure_key(reference_image, as_tuple(ref_keys["caps"][-1]["rect"]), False,
                             hex_rgba(manifest["declared"]["colors"]["keycapText"])[:3])
        n_right = mine["edges"][0] + mine["edges"][2] if mine and mine["edges"] else None
        r_right = theirs["edges"][0] + theirs["edges"][2] if theirs and theirs["edges"] else None
        report.check("parity", name, capture, "frame", "footer content right edge", n_right, r_right,
                     LIMITS["edge_px"], "px", "the Actions button's last key: the footer's 8px right padding and the button's own")


def fmt_color(color):
    return "#" + "".join(f"{int(round(c)):02X}" for c in color[:3])


def check_fonts(report, name, manifest):
    """The embedded faces resolved as themselves: a face the text system
    could not find resolves to the same fallback face a family no system
    has resolves to at that weight (#93: a fallback cannot pass as Geist)."""
    faces = manifest.get("fontResolution") or []
    probes = {f["weight"]: f for f in faces if f["family"] == "Pane Missing Font Probe"}
    for face in faces:
        if face["family"] == "Pane Missing Font Probe":
            continue
        probe = probes.get(face["weight"])
        fallback = probe is not None and (face["fontId"] == probe["fontId"])
        report.check("harness-native", name, "manifest", "fonts", f"{face['family']} {face['weight']:g} resolved",
                     "the fallback face" if fallback or probe is None else "its own face", "its own face", 0, "",
                     f"probe widths: {face['probeWidth']:.2f} vs fallback {probe['probeWidth']:.2f}" if probe else
                     "no fallback probe at this weight")
    by_face = {(f["family"], f["weight"]): f["fontId"] for f in faces}
    for family in sorted({f["family"] for f in faces if f["family"] != "Pane Missing Font Probe"}):
        regular, medium = by_face.get((family, 400.0)), by_face.get((family, 500.0))
        if regular is not None and medium is not None:
            report.check("harness-native", name, "manifest", "fonts", f"{family} 500 is its own face",
                         "distinct" if regular != medium else "the 400 face", "distinct", 0, "",
                         "500 must resolve to the embedded Medium face, not a synthesized weight")


def box_edges(image, rect, background, fill, scale=1.0, margin=2):
    """The edges of a filled box (a keycap, a tile) near its declared rect:
    left and right along its middle rows, top (and bottom) down columns
    just inside its left edge, where no label or glyph reaches. Pixels count
    as the box's when they differ from the background, in the fill's
    direction, by half the fill's contrast. margin stays inside the 3px gap
    between the caps of a sequence. Returns (x, y, w, h) in logical px; the
    bottom is reliable only for a box whose bottom row keeps the fill's
    direction (an accent cap, a tile) - a regular cap's bottom line darkens
    it. A label or glyph lighter than the fill is part of the box; one
    darker (an accent cap's ink) is inside its run and changes no edge."""
    x, y, w, h = scaled(rect, scale)
    data = image.load()
    bg = luma(background)
    sign = 1 if luma(fill) >= bg else -1
    delta = max(3.0, abs(luma(fill) - bg) / 2)

    def lit(color):
        return sign * (luma(color) - bg) >= delta

    columns = range(max(0, int(x - margin * scale)), min(image.width, int(x + w + margin * scale)))
    lefts, rights = [], []
    for row in range(int(y + h * 0.35), int(y + h * 0.65) + 1):
        flags = [lit(data[c, row]) for c in columns]
        if any(flags):
            lefts.append(columns.start + flags.index(True))
            rights.append(columns.start + len(flags) - flags[::-1].index(True))
    rows = range(max(0, int(y - margin * scale)), min(image.height, int(y + h + margin * scale)))
    tops, bottoms = [], []
    # Down the middle columns, clear of the rounded corners: a label or
    # glyph there is inside the run, which only its ends decide.
    for column in range(int(x + w / 2 - scale), int(x + w / 2 + scale) + 1):
        flags = [lit(data[column, r]) for r in rows]
        if any(flags):
            tops.append(rows.start + flags.index(True))
            bottoms.append(rows.start + len(flags) - flags[::-1].index(True))
    if not (lefts and tops):
        return None
    left, right = statistics.median(lefts), statistics.median(rights)
    top, bottom = statistics.median(tops), statistics.median(bottoms)
    return tuple(v / scale for v in (left, top, right - left, bottom - top))


def bottom_line_alpha(image, rect, fill, scale=1.0):
    """The black line inset along a regular cap's bottom, as an alpha in
    levels, with the cap's 1px ring lying over it (the reference's order):
    the ring's alpha is read on the cap's top row, removed from the bottom
    row, and what is left is the line's darkening of the fill. Rows are
    read across the cap clear of its rounded corners (and of its label,
    which never reaches its top or bottom rows), each against the fill two
    rows inside it, since the panel's sheen shades the fill down the cap.
    Only a cap on whole pixels has a bottom row to read; None otherwise.
    (`fill` is unused: kept for the call's symmetry with the other
    measures.)"""
    x, y, w, h = scaled(rect, scale)
    if abs(y + h - round(y + h)) > 0.01 or abs(y - round(y)) > 0.01:
        return None
    data = image.load()
    across = range(int(x + 5 * scale), int(x + w - 5 * scale))

    def row_color(r):
        return tuple(statistics.median(data[c, r][i] for c in across) for i in range(3))

    top_row, bottom_row = int(round(y)), int(round(y + h)) - 1
    ring = (white_alpha(row_color(top_row), row_color(top_row + 2)) or 0) / 255
    under = tuple((b - 255 * ring) / (1 - ring) for b in row_color(bottom_row))
    return black_alpha(under, row_color(bottom_row - 2))


def measure_key(image, rect, accent, text_color, scale=1.0, lighter=True):
    """One keycap: its edges, its fill (as a white overlay's alpha, or its
    color for an accent cap), the line along its bottom, and its label's
    ink box relative to the cap."""
    x, y, w, h = scaled(rect, scale)
    above = median_color(image, (x + 2 * scale, y - 4 * scale, x + w - 2 * scale, y - 2 * scale))
    below = median_color(image, (x + 2 * scale, y + h + 2 * scale, x + w - 2 * scale, y + h + 4 * scale))
    fill = median_color(image, (x + 1.5 * scale, y + 2 * scale, x + 3 * scale, y + h - 3 * scale))
    if above is None or fill is None:
        return None
    # The panel under a cap shades down the panel (the sheen); the cap's
    # fill is read against the panel beside it on both sides.
    background = tuple((a + b) / 2 for a, b in zip(above, below or above))
    edges = box_edges(image, rect, background, fill, scale)
    label = ink(image, (x + 2 * scale, y + 2 * scale, x + w - 2 * scale, y + h - 2 * scale), fill, text_color,
                exclude_accent=False)
    return {
        "edges": edges,
        "fill": fill,
        "alpha": None if accent else overlay_alpha(fill, background, lighter),
        # The estimate assumes a lightening ring over the darkening line,
        # the dark palette's caps.
        "bottom": None if accent or not lighter else bottom_line_alpha(image, rect, fill, scale),
        # The label's place relative to the cap as drawn: its measured left
        # edge, where a cap's fractional declared left snapped.
        "label": None if label is None else {
            "left": label["box"][0] / scale - (edges[0] if edges else x / scale),
            "top": (label["box"][1] - y) / scale,
            "height": label["box"][3] / scale,
        },
    }


# Positions the fixture declares through a chain of shaped widths (the
# footer's hint, its rule, the primary action after Actions): layout rounds
# each part to whole pixels, so a declared position drifts by up to a pixel
# and a half; parity measures the real offset at 1px.
CHAIN_SLACK_PX = 1.5
CHAIN_NOTE = "declared through a chain of shaped widths, which layout rounds part by part"


def compare_key_group(report, name, capture, group, manifest, native, scale, ref_group, reference_image, crops,
                      harness=True, chained=False, footer=False, static=None):
    """One key sequence: each cap against its declaration, then - when the
    reference shows the same effective binding - against the reference's
    caps: labels, sizes, fills, the bottom line and the label's place."""
    colors = manifest["declared"]["colors"]
    accent = group["style"] == "accent"
    text = hex_rgba(colors["accentInk"] if accent else colors["keycapText"])[:3]
    subject = f"keys:{group['binding']}"
    declared_fill = hex_rgba(colors["accent"] if accent else colors["keycapBackground"])
    lighter = palette(manifest)["lighter"]
    native_caps = []
    for index, cap in enumerate(group["caps"]):
        rect = as_tuple(cap["rect"])
        measured = measure_key(native, rect, accent, text, scale, lighter)
        native_caps.append(measured)
        if not harness:
            continue
        edges = measured and measured["edges"]
        props = (("left", 0), ("top", 1), ("width", 2)) + ((("height", 3),) if accent else ())
        for prop, i in props:
            slack = chained and prop == "left"
            report.check("harness-native", name, capture, subject, f"cap {index} {prop}",
                         edges[i] if edges else None, rect[i], CHAIN_SLACK_PX if slack else LIMITS["edge_px"], "px",
                         CHAIN_NOTE if slack else None)
        if accent:
            report.check("harness-native", name, capture, subject, f"cap {index} accent fill (max channel)",
                         max(abs(a - b) for a, b in zip(measured["fill"], declared_fill[:3])) if measured else None,
                         0, LIMITS["flat_fill_levels"], "levels")
        else:
            report.check("harness-native", name, capture, subject, f"cap {index} fill alpha",
                         measured and measured["alpha"], declared_fill[3], LIMITS["flat_fill_levels"], "levels")
            if lighter and not footer:
                report.check("harness-native", name, capture, subject, f"cap {index} bottom line alpha",
                             measured and measured["bottom"], hex_rgba(colors["keycapBottom"])[3],
                             BOTTOM_LINE_LIMIT, "levels", BOTTOM_LINE_NOTE)
    crops.append((subject, as_tuple(group["rect"]), None))
    if not ref_group or reference_image is None:
        return
    ref_rect = as_tuple(ref_group["rect"])
    crops.append(("parity-" + subject, as_tuple(group["rect"]), ref_rect))
    ref_labels = [c["label"] for c in ref_group["caps"]]
    report.check("parity", name, capture, subject, "labels", " | ".join(c["label"] for c in group["caps"]),
                 " | ".join(ref_labels), 0, "", "one cap per key, the same effective binding on both sides")
    family = ref_group["font"]["family"].split(",")[0].strip().strip('"')
    report.check("parity", name, capture, subject, "label font", group["font"],
                 f"{ref_group['font']['size']} {family} {ref_group['font']['weight']}", 0, "")
    n_first, n_last = native_caps[0], native_caps[-1]
    if n_first and n_last and n_first["edges"] and n_last["edges"]:
        width = n_last["edges"][0] + n_last["edges"][2] - n_first["edges"][0]
        report.check("parity", name, capture, subject, "group width", width, ref_rect[2], LIMITS["edge_px"], "px")
    for index, (cap, ref_cap) in enumerate(zip(group["caps"], ref_group["caps"])):
        mine = native_caps[index]
        theirs = measure_key(reference_image, as_tuple(ref_cap["rect"]), accent, text)
        if not mine or not theirs:
            continue
        if mine["edges"] and theirs["edges"]:
            report.check("parity", name, capture, subject, f"cap {index} width", mine["edges"][2], theirs["edges"][2],
                         LIMITS["edge_px"], "px")
            if accent:
                report.check("parity", name, capture, subject, f"cap {index} height", mine["edges"][3],
                             theirs["edges"][3], LIMITS["edge_px"], "px")
        if accent:
            report.check("parity", name, capture, subject, f"cap {index} accent fill (max channel)",
                         max(abs(a - b) for a, b in zip(mine["fill"], theirs["fill"])), 0,
                         LIMITS["flat_fill_levels"], "levels",
                         f"native {fmt_color(mine['fill'])} vs reference {fmt_color(theirs['fill'])}",
                         accepted=static)
        else:
            report.check("parity", name, capture, subject, f"cap {index} fill alpha", mine["alpha"], theirs["alpha"],
                         LIMITS["flat_fill_levels"], "levels")
            if theirs["bottom"] is not None:
                report.check("parity", name, capture, subject, f"cap {index} bottom line alpha", mine["bottom"],
                             theirs["bottom"], BOTTOM_LINE_LIMIT, "levels", BOTTOM_LINE_NOTE)
        if mine["label"] and theirs["label"]:
            subset = SUBSET_DISPOSITION if not cap["label"].isascii() else None
            for prop in ("left", "top", "height"):
                report.check("parity", name, capture, subject, f"cap {index} label ink {prop}", mine["label"][prop],
                             theirs["label"][prop], LIMITS["edge_px"], "px", accepted=subset)


# The bottom-line estimate removes the ring and divides by the fill, so each
# level of the bottom row moves it by about 255 / fill (about 5 levels on
# the dark panel): an estimate's limit, not a flat fill's.
BOTTOM_LINE_LIMIT = 12
BOTTOM_LINE_NOTE = "the black 35% inset along the cap's bottom, under its ring; a two-overlay estimate"
# The reference embeds Geist and Geist Mono as 225-glyph subsets with no
# arrows and no return symbol, so Chrome draws those labels in a system
# fallback face; Pane draws Geist Mono's own glyphs.
SUBSET_DISPOSITION = ("accepted (#93): the reference's embedded Geist Mono is a 225-glyph subset without "
                      "this symbol, so the browser draws it in a system fallback face; Pane draws Geist Mono's own")


def compare_keycaps(report, name, capture, manifest, native, scale, ref_capture, reference_image, crops):
    groups = (ref_capture or {}).get("state", {}).get("keycaps", {}) if ref_capture else {}
    for group in manifest["keycaps"]:
        compare_key_group(report, name, capture, group, manifest, native, scale,
                          groups.get(group["group"]) if group["group"] else None, reference_image, crops)


def measure_tile(image, rect, app, glyph_color, background, scale=1.0):
    """An icon tile: its edges, its gradient's two ends (an app tile) or its
    fill's overlay alpha (a command tile), and its glyph's core ink - box
    relative to the tile and core pixel count (the stroke's weight)."""
    x, y, w, h = scaled(rect, scale)
    top = median_color(image, (x + 3 * scale, y + 3 * scale, x + 5 * scale, y + 5 * scale))
    bottom = median_color(image, (x + 3 * scale, y + h - 6 * scale, x + 5 * scale, y + h - 4 * scale))
    middle = median_color(image, (x + 3 * scale, y + h / 2 - 2 * scale, x + 5 * scale, y + h / 2 + 2 * scale))
    if top is None or middle is None:
        return None
    glyph = ink(image, (x + 2 * scale, y + 2 * scale, x + w - 2 * scale, y + h - 2 * scale), middle, glyph_color,
                exclude_accent=False)
    return {
        "edges": box_edges(image, rect, background, middle, scale),
        "top": top,
        "bottom": bottom,
        "alpha": None if app else white_alpha(middle, background),
        "glyph": None if glyph is None else {
            "left": (glyph["box"][0] - x) / scale,
            "top": (glyph["box"][1] - y) / scale,
            "width": glyph["box"][2] / scale,
            "height": glyph["box"][3] / scale,
            "count": glyph["count"] / (scale * scale),
            "color": glyph["color"],
        },
    }


# The reference's app tile glyphs are white on their gradients; a command
# tile's are #E9E9EC.
APP_GLYPH, COMMAND_GLYPH = (255, 255, 255), (233, 233, 236)
# How far a glyph's core pixel count may differ between the two
# rasterizers (resvg's in GPUI, Skia's in Chrome) for the same path and
# stroke, in percent: an engineering limit, not a reference value. The
# 2px application stroke carries about 25% more core than the 1.6px one,
# so a wrong stroke width still fails it.
STROKE_LIMIT = 25


def compare_row_tile(report, name, capture, subject, row, ref_row, manifest, native, scale, reference_image,
                     measured, ref_measured, crops):
    """A row's icon tile, native against reference: where it lies, its
    fills and its glyph."""
    geometry = manifest["declared"]["geometry"]
    x, y, w, h = as_tuple(row["rect"])
    side = geometry["tileSize"]
    n_rect = (x + geometry["rowPaddingX"], y + (h - side) / 2, side, side)
    r_rect = as_tuple(ref_row.get("tile")) if ref_row.get("tile") else None
    if r_rect is None or not measured or not ref_measured:
        return
    app = ref_row.get("tileApp", None)
    if app is None:
        app = row.get("tone", "command") != "command"
    color = APP_GLYPH if app else COMMAND_GLYPH
    n_bg = native_wash_fill(native, row, scale)
    r_bg = native_wash_fill(reference_image, ref_row, 1.0)
    mine = measure_tile(native, n_rect, app, color, n_bg, scale)
    theirs = measure_tile(reference_image, r_rect, app, color, r_bg)
    if not mine or not theirs:
        return
    sub = subject + "/tile"
    crops.append(("parity-tile-" + row["title"], n_rect, r_rect))
    if mine["edges"] and theirs["edges"]:
        for prop, i in (("left", 0), ("top", 1), ("width", 2), ("height", 3)):
            n_value = mine["edges"][i] - (x if i == 0 else y if i == 1 else 0)
            r_value = theirs["edges"][i] - (as_tuple(ref_row["rect"])[0] if i == 0 else
                                            as_tuple(ref_row["rect"])[1] if i == 1 else 0)
            report.check("parity", name, capture, sub, f"{prop}{' in row' if i < 2 else ''}", n_value, r_value,
                         LIMITS["edge_px"], "px")
    if app:
        # A gradient is not a flat fill: across a 28px tile it changes about
        # 1.5 levels a row, so half a pixel of placement moves a sample by
        # about that much. The glyph-core limit applies.
        for end in ("top", "bottom"):
            report.check("parity", name, capture, sub, f"gradient {end} (max channel)",
                         max(abs(a - b) for a, b in zip(mine[end], theirs[end])), 0, LIMITS["glyph_core_levels"],
                         "levels", f"native {fmt_color(mine[end])} vs reference {fmt_color(theirs[end])}")
    else:
        report.check("parity", name, capture, sub, "fill alpha", mine["alpha"], theirs["alpha"],
                     LIMITS["flat_fill_levels"], "levels")
    if mine["glyph"] and theirs["glyph"]:
        for prop in ("left", "top", "width", "height"):
            report.check("parity", name, capture, sub, f"glyph ink {prop}", mine["glyph"][prop],
                         theirs["glyph"][prop], LIMITS["edge_px"], "px")
        report.check("parity", name, capture, sub, "glyph core pixels (stroke weight)",
                     100 * (mine["glyph"]["count"] / max(1, theirs["glyph"]["count"]) - 1), 0, STROKE_LIMIT, "%",
                     f"{mine['glyph']['count']:.0f} vs {theirs['glyph']['count']:.0f} core pixels")


def kind_ink(image, rect, fill, scale=1.0, pal=DARK):
    """A row's kind text: its core ink box, read in the row's right part."""
    x, y, w, h = scaled(rect, scale)
    found = ink(image, (x, y + 4 * scale, x + w + 2 * scale, y + h - 4 * scale), fill, pal["muted"],
                exclude_accent=False)
    if not found:
        return None
    bx, by, bw, bh = found["box"]
    return {"right": (bx + bw) / scale, "top": by / scale, "height": bh / scale}


def measure_alias(image, rect, fill, scale=1.0, pal=DARK):
    """An alias chip: its ring's box (the chip's own edges) and its
    label's ink, near its declared place in the row."""
    x, y, w, h = scaled(rect, scale)
    middle = y + h / 2
    chip_rect = (x / scale, (middle - 9 * scale) / scale, w / scale, 18)
    background = median_color(image, (x - 6 * scale, middle - 2 * scale, x - 3 * scale, middle + 2 * scale)) or fill
    # The chip's ring is its only fill: the column near its declared left
    # that stands out most from the row is the ring's.
    columns = [median_color(image, (column, middle - 3 * scale, column + 1, middle + 3 * scale))
               for column in range(int(x - 3 * scale), int(x + 3 * scale) + 1)]
    columns = [color for color in columns if color]
    ring = max(columns, key=lambda color: abs(luma(color) - luma(background))) if columns else None
    edges = box_edges(image, chip_rect, background, ring, scale, margin=3) if ring else None
    label_box = (x + 2 * scale, middle - 8 * scale, x + w - 2 * scale, middle + 8 * scale)
    label = ink(image, label_box, background, pal["alias"], exclude_accent=False)
    # The label's top from ink_top: a thin ascender (the 'b' of "cb") lands
    # on a renderer's own subpixel phase, which ink()'s core alone misreads
    # as the label sitting lower.
    if label is None:
        return {"edges": edges, "label": None}
    lx, ly, _, lh = label["box"]
    top = min(ly, ink_top(image, label_box, background, pal["alias"]) or ly)
    return {
        "edges": edges,
        "label": {"left": lx / scale, "top": top / scale, "height": (ly + lh - top) / scale},
    }


def harness_row_trailing(report, name, capture, row, manifest, native, scale, crops):
    """A native row's trailing parts against their declaration: the kind's
    text ends at the row's right padding, the alias chip lies where it is
    declared, and the key sequence's caps (see compare_key_group)."""
    subject = f"row:{row['title']}"
    fill = native_wash_fill(native, row, scale)
    if fill is None:
        return
    if row.get("kindRect"):
        rect = as_tuple(row["kindRect"])
        found = kind_ink(native, rect, fill, scale, palette(manifest))
        # Text is right-aligned in its box: its ink ends within a pixel of
        # the box's right (a glyph's right side bearing).
        report.check("harness-native", name, capture, subject, "kind ink right",
                     found and found["right"], rect[0] + rect[2], LIMITS["edge_px"] * 1.5, "px",
                     "right-aligned at the row's padding; a glyph's side bearing allowed")
    if row.get("aliasRect"):
        rect = as_tuple(row["aliasRect"])
        measured = measure_alias(native, rect, fill, scale, palette(manifest))
        edges = measured["edges"]
        report.check("harness-native", name, capture, subject, "alias chip width",
                     edges[2] if edges else None, rect[2], LIMITS["edge_px"], "px")
        # Its place: the row's gap before what follows it - the key
        # sequence's first cap, measured, or the kind's declared box. (The
        # caps' widths round to whole pixels in layout, so declared
        # positions drift by fractions of a pixel per cap.)
        following = row["kindRect"]["x"] if row.get("kindRect") else None
        if row.get("keyGroup"):
            first = as_tuple(row["keyGroup"]["caps"][0]["rect"])
            x, y, w, h = scaled(first, scale)
            key_fill = median_color(native, (x + 1.5 * scale, y + 2 * scale, x + 3 * scale, y + h - 3 * scale))
            key_edges = box_edges(native, first, fill, key_fill, scale) if key_fill else None
            following = key_edges[0] if key_edges else None
        gap = manifest["declared"]["geometry"].get("rowGap", 12)
        report.check("harness-native", name, capture, subject, "alias chip gap to what follows",
                     following - (edges[0] + edges[2]) if edges and following is not None else None, gap,
                     LIMITS["edge_px"], "px")
        # What comes before it - a title or reason however long - truncates
        # or wraps short of the gap: no text ink in the gap before the chip.
        if edges:
            _, ry, _, rh = scaled(as_tuple(row["rect"]), scale)
            left = (edges[0] - gap + 1) * scale
            stray = ink(native, (left, ry, edges[0] * scale - 1, ry + rh), fill, palette(manifest)["muted"],
                        exclude_accent=False)
            report.check("harness-native", name, capture, subject, "text ink in the gap before the alias chip",
                         0 if stray is None else stray["count"], 0, 0, "px")
    if row.get("keyGroup"):
        compare_key_group(report, name, capture, row["keyGroup"], manifest, native, scale, None, None, crops)


def compare_row_trailing(report, name, capture, subject, row, ref_row, manifest, native, scale, reference_image,
                         measured, ref_measured, crops):
    """A row's trailing parts, native against reference: the kind's text,
    the alias chip and the key sequence."""
    n_fill = native_wash_fill(native, row, scale)
    r_fill = native_wash_fill(reference_image, ref_row, 1.0)
    if n_fill is None or r_fill is None:
        return
    n_row, r_row = as_tuple(row["rect"]), as_tuple(ref_row["rect"])
    if row.get("kindRect") and ref_row.get("kind"):
        # Each side's kind box across the row's own height: the reference's
        # element is only its text's height.
        kx, _, kw, _ = as_tuple(row["kindRect"])
        rx, _, rw, _ = as_tuple(ref_row["kind"]["rect"])
        mine = kind_ink(native, (kx, n_row[1], kw, n_row[3]), n_fill, scale, palette(manifest))
        theirs = kind_ink(reference_image, (rx, r_row[1], rw, r_row[3]), r_fill)
        report.check("parity", name, capture, subject, "kind", row["kind"], ref_row["kind"]["text"], 0, "")
        if mine and theirs:
            report.check("parity", name, capture, subject, "kind ink right in row", mine["right"] - n_row[0],
                         theirs["right"] - r_row[0], LIMITS["edge_px"], "px")
            report.check("parity", name, capture, subject, "kind ink top in row", mine["top"] - n_row[1],
                         theirs["top"] - r_row[1], LIMITS["edge_px"], "px")
            report.check("parity", name, capture, subject, "kind ink height", mine["height"], theirs["height"],
                         LIMITS["edge_px"], "px")
    if row.get("aliasRect") and ref_row.get("alias"):
        ax, _, aw, _ = as_tuple(row["aliasRect"])
        bx, _, bw, _ = as_tuple(ref_row["alias"]["rect"])
        mine = measure_alias(native, (ax, n_row[1], aw, n_row[3]), n_fill, scale, palette(manifest))
        theirs = measure_alias(reference_image, (bx, r_row[1], bw, r_row[3]), r_fill)
        report.check("parity", name, capture, subject, "alias", row["alias"], ref_row["alias"]["text"], 0, "")
        if mine["edges"] and theirs["edges"]:
            for prop, index, origin in (("left in row", 0, 0), ("top in row", 1, 1), ("width", 2, None),
                                        ("height", 3, None)):
                n_value = mine["edges"][index] - (n_row[origin] if origin is not None else 0)
                r_value = theirs["edges"][index] - (r_row[origin] if origin is not None else 0)
                report.check("parity", name, capture, subject, f"alias chip {prop}", n_value, r_value,
                             LIMITS["edge_px"], "px")
        if mine["label"] and theirs["label"]:
            report.check("parity", name, capture, subject, "alias label ink top in row",
                         mine["label"]["top"] - n_row[1], theirs["label"]["top"] - r_row[1], LIMITS["edge_px"], "px")
        crops.append(("parity-alias-" + row["title"], as_tuple(row["aliasRect"]), as_tuple(ref_row["alias"]["rect"])))
    if row.get("keyGroup") and ref_row.get("keyGroup"):
        compare_key_group(report, name, capture, row["keyGroup"], manifest, native, scale, ref_row["keyGroup"],
                          reference_image, crops, harness=False)


# The reference's blank-query list, labelled with its fixture's own story:
# a pinned strip (#101) and "Suggested · From your recent use" above its
# commands. Pane labels a blank query's list as what it is - its commands,
# in root search's order - and claims no recent use (#100).
SECTION_DISPOSITION = ("accepted (#94, #100): Pane labels a blank query's rows \"Commands\" and claims no recent "
                       "use, so the reference's \"Suggested · From your recent use\" is not shown; the reference's "
                       "pinned strip and its label are #101's")


def section_ink(image, rect, scale=1.0, pal=DARK):
    x, y, w, h = scaled(rect, scale)
    background = median_color(image, (x + w * 0.4, y + 2 * scale, x + w * 0.5, y + 5 * scale))
    if background is None:
        return None, None
    left = ink(image, (x, y + 6 * scale, x + w * 0.5, y + h), background, pal["muted"], exclude_accent=False)
    right = ink(image, (x + w * 0.5, y + 6 * scale, x + w + 2 * scale, y + h), background, pal["muted"],
                exclude_accent=False)
    return left, right


def compare_sections(report, name, capture, declared, state, native, scale, reference_image, crops):
    """The section labels: which labels show over the rows, and the
    typography of a label both sides show."""
    native_labels = declared.get("sections") or []
    ref_labels = [label for label in state.get("labels", []) if label.get("visible", True)]
    report.check("parity", name, capture, "sections", "labels",
                 " | ".join(label["label"] for label in native_labels),
                 " | ".join(label["text"] for label in ref_labels), 0, "",
                 accepted=SECTION_DISPOSITION if not state["query"] else None)
    by_text = {label["text"]: label for label in ref_labels}
    for label in native_labels:
        ref = by_text.get(label["label"])
        if not ref:
            continue
        subject = f"section:{label['label']}"
        n_rect, r_rect = as_tuple(label["rect"]), as_tuple(ref["rect"])
        crops.append(("parity-" + subject, n_rect, r_rect))
        n_left, n_right = section_ink(native, n_rect, scale)
        r_left, r_right = section_ink(reference_image, r_rect)
        report.check("parity", name, capture, subject, "note", label.get("note") or "", ref.get("note") or "", 0, "")
        report.check("parity", name, capture, subject, "label height", n_rect[3], r_rect[3], LIMITS["edge_px"], "px")
        if n_left and r_left:
            for prop, index in (("left", 0), ("top", 1)):
                report.check("parity", name, capture, subject, f"title ink {prop} in label",
                             n_left["box"][index] / scale - n_rect[index], r_left["box"][index] - r_rect[index],
                             LIMITS["edge_px"], "px")
            report.check("parity", name, capture, subject, "title ink height", n_left["box"][3] / scale,
                         r_left["box"][3], LIMITS["edge_px"], "px")
        if n_right and r_right and (label.get("note") or ""):
            report.check("parity", name, capture, subject, "note ink right in label",
                         (n_right["box"][0] + n_right["box"][2]) / scale - n_rect[0],
                         r_right["box"][0] + r_right["box"][2] - r_rect[0], LIMITS["edge_px"] * 1.5, "px",
                         "right-aligned; a glyph's side bearing allowed")


# ------------------------------------------------- the footer and Actions (#95)

# Content the reference authors that Pane deliberately does not show: its
# Actions lists operations Pane has no working contract for (#100), and its
# static Actions board's footer carries a tip instead of the hint.
ACTIONS_CONTENT = ("accepted (#95, #100): the reference lists Pin to Quick Slot (#101), Open New Window, Show in "
                   "File Manager, Quit and Hide from Results; Pane lists only the operations it can perform - the "
                   "primary action and an installed command's hotkey and alias")
STATIC_ENTER = ("accepted (#95): the static Actions board draws its footer's Enter as a plain cap; the root "
                "board, live, draws it in the accent, as Pane does (#93)")
STATIC_HINT = ("accepted (#95): the static Actions board's footer shows a tip ('Every action keeps its shortcut - no "
               "menu needed next time'); Pane's footer shows the hint the root board shows while Actions is open")

# The dimmed list's sample: the list's left padding near its bottom, where
# no row, wash or sheen reaches.
UNDIMMED = {}


def box_px(rect, scale=1.0, pad=0.0):
    """A logical (x, y, w, h) as an image box (left, top, right, bottom),
    grown by pad on every side."""
    x, y, w, h = rect
    return ((x - pad) * scale, (y - pad) * scale, (x + w + pad) * scale, (y + h + pad) * scale)


def ink_extent(image, rect, background, color, scale=1.0, pad=0.0):
    """The core ink of color inside rect (grown by pad): its box in logical
    px (left, top, right, bottom) and its color, or None."""
    found = ink(image, box_px(rect, scale, pad), background, color, exclude_accent=False)
    if not found:
        return None
    left, top, width, height = found["box"]
    return {"left": left / scale, "top": top / scale, "right": (left + width) / scale,
            "bottom": (top + height) / scale, "color": found["color"]}


def coverage_edges(image, rect, background, color, scale=1.0, enough=1.0):
    """Where text's ink begins inside rect, by coverage: each pixel counts
    how far it is from the background toward color (0 to 1), and the left
    edge is the first column, the top the first row, whose pixels add up
    to enough - a pixel's worth of ink. An anti-aliased stem tip or a thin
    diagonal that one renderer leaves just under a core threshold and the
    other just over moves this by a fraction, not a pixel. Logical px
    (left, top), or None."""
    left, top, right, bottom = clamp_box(image, box_px(rect, scale))
    bg, fg = luma(background), luma(color)
    if fg == bg:
        return None
    data = image.load()
    # Below a fifth of the way to the ink, a pixel is the background's own
    # variation (the glass's gradient, the wallpaper's shading), not ink.
    def coverage(value):
        amount = min(1.0, max(0.0, (luma(value) - bg) / (fg - bg)))
        return amount if amount >= 0.2 else 0.0

    cover = [[coverage(data[x, y]) for x in range(left, right)] for y in range(top, bottom)]
    columns = [sum(row[i] for row in cover) for i in range(right - left)]
    rows = [sum(row) for row in cover]
    first_column = next((i for i, total in enumerate(columns) if total >= enough), None)
    first_row = next((i for i, total in enumerate(rows) if total >= enough), None)
    if first_column is None or first_row is None:
        return None
    return {"left": (left + first_column) / scale, "top": (top + first_row) / scale}


def composite(fill_hex, background):
    """The color fill (#RRGGBBAA) paints over background."""
    r, g, b, a = hex_rgba(fill_hex)
    a /= 255
    return tuple(bg * (1 - a) + c * a for c, bg in zip((r, g, b), background))


def color_delta(a, b):
    return max(abs(x - y) for x, y in zip(a, b)) if a and b else None


def footer_strip_background(image, footer, scale=1.0):
    """The footer's own fill: its left padding, which no part reaches."""
    x, y, w, h = footer
    return median_color(image, box_px((x + 2, y + 6, 6, 8), scale))


def measure_footer_parts(image, footer, parts, scale=1.0, pal=DARK, mark_color=(218, 218, 220),
                         divider_color=None):
    """The footer's mark, hint parts, rule and Actions button, measured near
    where parts (the declaration, or the reference's DOM) puts them."""
    background = footer_strip_background(image, footer, scale)
    out = {"background": background}
    if parts.get("mark"):
        out["mark"] = ink_extent(image, as_tuple(parts["mark"]), background, mark_color, scale, pad=2)
    line_top = footer[1] + 1
    line = footer[3] - 1
    hint = []
    for part in parts.get("hint", []):
        x, _, w, _ = as_tuple(part["rect"])
        if part["keys"]:
            cap = (x, line_top + (line - 20) / 2, w, 20)
            measured = measure_cap(image, cap, scale, pal["lighter"])
            edges = measured and measured["edges"]
            hint.append({"text": part["text"], "keys": True, "left": edges[0] if edges else None,
                         "top": edges[1] if edges else None})
        else:
            found = coverage_edges(image, (x - 2, line_top, w + 4, line), background, pal["muted"], scale)
            # The top from the text's interior, clear of a cap's label beside it.
            inner = coverage_edges(image, (x + 3, line_top, max(1, w - 6), line), background, pal["muted"], scale)
            hint.append({"text": part["text"], "keys": False, "left": found and found["left"],
                         "top": inner and inner["top"]})
    out["hint"] = hint
    if parts.get("divider"):
        x, y, w, h = as_tuple(parts["divider"])
        # The column that stands out most from the strip, within 4px of
        # where the rule is declared.
        # The rule lights one column, or two when it falls between them:
        # the strongest neighbouring pair within 3px of its declared place
        # holds it, the nearer pair winning a tie (the open Actions
        # button's wash begins 5px to its right).
        # Against the strip just left of it (the gap before it holds no
        # button): the open panel's shadow darkens the strip unevenly.
        local = median_color(image, box_px((x - 5, y + 3, 2, h - 6), scale)) or background
        columns = []
        for column_x in range(int((x - 3) * scale), int((x + 4) * scale)):
            color = median_color(image, (column_x, (y + 3) * scale, column_x + 1, (y + h - 3) * scale))
            alpha = overlay_alpha(color, local, pal["lighter"]) if color and local else None
            columns.append((max(alpha or 0, 0), column_x / scale))
        pairs = [(a[0] + b[0], -abs(a[1] - x), a[1] if a[0] >= b[0] else b[1])
                 for a, b in zip(columns, columns[1:])]
        alpha, _, found = max(pairs) if pairs else (None, None, None)
        out["divider"] = {"alpha": alpha, "x": found}
    button = parts.get("actionsButton")
    if button:
        rect = as_tuple(button if "x" in button else button["rect"])
        # Against the button's own fill (its open wash, while open).
        inside = median_color(image, box_px((rect[0] + 2, rect[1] + 8, 3, rect[3] - 16), scale)) or background
        text = coverage_edges(image, (rect[0] + 3, rect[1], rect[2] * 0.6, rect[3]), inside,
                              (217, 218, 221), scale)
        measured = measure_cap(image, rect, scale, pal["lighter"])
        out["actionsButton"] = {"labelLeft": text and text["left"], "labelTop": text and text["top"],
                                "alpha": measured and measured["alpha"], "edges": measured and measured["edges"]}
    return out


def harness_footer(report, name, capture, declared, manifest, native, scale, pal, crops):
    """The native footer against its declaration: the mark where it is
    declared, each hint part starting where declared, the rule's alpha and
    the Actions button's label and wash."""
    parts = declared.get("footer")
    if not parts:
        return
    colors = manifest["declared"]["colors"]
    footer = as_tuple(manifest["footer"])
    measured = measure_footer_parts(native, footer, parts, scale, pal, hex_rgba(colors["footerMark"])[:3])
    mark = as_tuple(parts["mark"])
    found = measured.get("mark")
    # The filled square's top and right (y 3.5 and x 21 of the mark's 24
    # units: 2.6 and 15.75 of 18px), which the stroked square never
    # reaches: its thin stroke clears the core threshold on one renderer
    # and not on the other, so it decides no edge here.
    report.check("harness-native", name, capture, "footer-mark", "ink top", found and found["top"],
                 mark[1] + 3, LIMITS["edge_px"], "px")
    report.check("harness-native", name, capture, "footer-mark", "ink right", found and found["right"],
                 mark[0] + 15.75, LIMITS["edge_px"], "px")
    crops.append(("footer-mark", mark, None))
    for index, (part, found) in enumerate(zip(parts["hint"], measured["hint"])):
        if part.get("clipped"):
            continue
        subject = f"footer-hint:{index}:{part['text']}"
        x = as_tuple(part["rect"])[0]
        report.check("harness-native", name, capture, subject, "cap left" if part["keys"] else "ink left",
                     found["left"], x, CHAIN_SLACK_PX + (0 if part["keys"] else 0.5), "px",
                     CHAIN_NOTE + ("" if part["keys"] else "; a glyph's side bearing allowed"))
    report.check("harness-native", name, capture, "footer-divider", "x", measured.get("divider", {}).get("x"),
                 as_tuple(parts["divider"])[0], CHAIN_SLACK_PX, "px", CHAIN_NOTE)
    report.check("harness-native", name, capture, "footer-divider", "alpha",
                 measured.get("divider", {}).get("alpha"), hex_rgba(colors["footerDivider"])[3],
                 LIMITS["flat_fill_levels"], "levels")
    button = as_tuple(parts["actionsButton"])
    found = measured["actionsButton"]
    report.check("harness-native", name, capture, "footer-actions", "label ink left", found["labelLeft"],
                 button[0] + manifest["declared"]["geometry"].get("actionPaddingX", 8), LIMITS["edge_px"] * 1.5, "px",
                 "a glyph's side bearing allowed")
    expected = hex_rgba(colors["footerButtonOpen"])[3] if parts["actionsPressed"] else 0
    report.check("harness-native", name, capture, "footer-actions",
                 f"wash alpha ({'open' if parts['actionsPressed'] else 'rest'})", found["alpha"], expected,
                 LIMITS["flat_fill_levels"], "levels")
    crops.append(("footer-actions", button, None))


def parity_footer(report, name, capture, declared, manifest, native, scale, state, reference_image, pal, crops):
    """The footer's left side and Actions button, native against reference."""
    parts, ref_parts = declared.get("footer"), state.get("footerParts")
    if not parts or not ref_parts:
        return
    colors = manifest["declared"]["colors"]
    footer = as_tuple(manifest["footer"])
    ref_footer = as_tuple(state["footer"])
    mine = measure_footer_parts(native, footer, parts, scale, pal, hex_rgba(colors["footerMark"])[:3])
    theirs = measure_footer_parts(reference_image, ref_footer, ref_parts)
    if mine.get("mark") and theirs.get("mark"):
        for prop in ("top", "right"):
            report.check("parity", name, capture, "footer-mark", f"ink {prop}", mine["mark"][prop],
                         theirs["mark"][prop], LIMITS["edge_px"], "px")
        crops.append(("parity-footer-mark", as_tuple(parts["mark"]), as_tuple(ref_parts["mark"])))
    # The static Actions board's footer carries one tip where the hint is.
    static = len(ref_parts["hint"]) == 1
    native_text = " ".join(part["text"] for part in parts["hint"])
    ref_text = " ".join(part["text"] for part in ref_parts["hint"])
    report.check("parity", name, capture, "footer-hint", "text", native_text, ref_text, 0, "",
                 accepted=STATIC_HINT if static else None)
    if not static:
        for index, (a, b) in enumerate(zip(mine["hint"], theirs["hint"])):
            subject = f"footer-hint:{index}:{a['text']}"
            for prop in ("left", "top"):
                report.check("parity", name, capture, subject, f"{'cap' if a['keys'] else 'ink'} {prop}",
                             a.get(prop), b.get(prop), LIMITS["edge_px"], "px",
                             "the reference's subset has no ↵: its cap label comes from a fallback face (#93)"
                             if "↵" in a["text"] and prop == "top" else None)
    if ref_parts.get("divider") and parts.get("divider"):
        report.check("parity", name, capture, "footer-divider", "x", mine["divider"]["x"],
                     theirs["divider"]["x"], LIMITS["edge_px"], "px")
        report.check("parity", name, capture, "footer-divider", "alpha", mine["divider"]["alpha"],
                     theirs["divider"]["alpha"], LIMITS["flat_fill_levels"], "levels")
    ref_button = ref_parts.get("actionsButton")
    if ref_button:
        a, b = mine["actionsButton"], theirs["actionsButton"]
        for prop in ("labelLeft", "labelTop"):
            report.check("parity", name, capture, "footer-actions", f"label ink {prop[5:].lower()}", a[prop], b[prop],
                         LIMITS["edge_px"], "px")
        report.check("parity", name, capture, "footer-actions", "pressed", "yes" if parts["actionsPressed"] else "no",
                     "yes" if ref_button["pressed"] else "no", 0, "")
        report.check("parity", name, capture, "footer-actions", "wash alpha", a["alpha"], b["alpha"],
                     LIMITS["flat_fill_levels"], "levels")
        crops.append(("parity-footer-actions", as_tuple(parts["actionsButton"]), as_tuple(ref_button["rect"])))


def action_of(label, index):
    """The kind of an Actions entry, by what the reference names it: the
    first is the primary action."""
    if index == 0:
        return "invoke"
    if "Hotkey" in label:
        return "hotkey"
    if "Alias" in label:
        return "alias"
    return None


def measure_panel(image, panel, scale=1.0, pal=DARK, colors=None):
    """The open Actions panel near panel (the declaration, or the
    reference's DOM, in the same shape): its box, its fill, the header's
    tile and title, each entry's wash, glyph and label, the group labels,
    the rules, the search row's rule and text, and the empty note."""
    rect = as_tuple(panel["rect"])
    x, y, w, h = rect
    background = median_color(image, box_px((x - 8, y + h * 0.5 - 2, 5, 4), scale))
    fill = median_color(image, box_px((x + 1.5, y + h * 0.5 - 2, 2, 4), scale))
    out = {"background": background, "fill": fill}
    out["edges"] = box_edges(image, rect, background, fill, scale) if background and fill else None
    title_color = (142, 143, 148)
    text_color = (228, 228, 231)
    icon_color = (163, 164, 169)
    if colors:
        text_color = hex_rgba(colors["actionText"])[:3]
        icon_color = hex_rgba(colors["actionIcon"])[:3]
        title_color = hex_rgba(colors["textMuted"])[:3]
    header = panel.get("header")
    if header and header.get("tile"):
        tile = as_tuple(header["tile"])
        tile_fill = median_color(image, box_px((tile[0] + 1.5, tile[1] + 7, 2, 4), scale))
        tile_bg = median_color(image, box_px((tile[0] - 6, tile[1] + 7, 3, 4), scale))
        edges = box_edges(image, tile, tile_bg, tile_fill, scale) if tile_fill and tile_bg else None
        hrect = as_tuple(header["rect"])
        title = ink_extent(image, (tile[0] + tile[2] + 2, hrect[1], hrect[2] / 2, hrect[3]), tile_bg, title_color, scale)
        out["header"] = {"tile": edges, "title": title}
    rows = []
    for index, row in enumerate(panel.get("rows", [])):
        r = as_tuple(row["rect"])
        side = median_color(image, box_px((r[0] - 4, r[1] + r[3] * 0.3, 2, r[3] * 0.4), scale))
        wash = median_color(image, box_px((r[0] + 2, r[1] + r[3] * 0.3, 2, r[3] * 0.4), scale))
        alpha = overlay_alpha(wash, side, pal["lighter"]) if wash and side else None
        glyph_box = as_tuple(row["glyph"]) if "glyph" in row else (r[0] + 8, r[1] + 10, 16, 16)
        glyph = ink_extent(image, glyph_box, wash, icon_color, scale, pad=1)
        label = ink_extent(image, (glyph_box[0] + glyph_box[2] + 4, r[1], r[2] * 0.55, r[3]), wash, text_color, scale)
        rows.append({"label": row["label"], "action": row.get("action") or action_of(row["label"], index),
                     "rect": r, "alpha": alpha, "glyph": glyph, "text": label})
    out["rows"] = rows
    out["groups"] = [
        {"text": group["text"], "rect": as_tuple(group["rect"]),
         "ink": ink_extent(image, as_tuple(group["rect"]), fill, title_color, scale)}
        for group in panel.get("groups", [])
    ]
    rules = []
    for rule in panel.get("rules", []):
        rx, ry, rw, rh = as_tuple(rule)
        found = hline(image, (int((rx + 20) * scale), int((rx + rw - 20) * scale)),
                      (int((ry - 3) * scale), int((ry + 4) * scale)), fill, pal["lighter"])
        line = median_color(image, box_px((rx + 20, found / scale if found else ry, rw - 40, 1), scale)) if found else None
        # Against the panel just above it: the popover's sheen fades over
        # its top 40%, so its middle is darker than the rule's surround.
        above = median_color(image, box_px((rx + 20, (found / scale if found else ry) - 3, rw - 40, 1), scale))
        rules.append({"y": found / scale if found else None,
                      "alpha": overlay_alpha(line, above, pal["lighter"]) if line and above else None})
    out["rules"] = rules
    search = as_tuple(panel["search"])
    sx, sy, sw, sh = search
    found = hline(image, (int((sx + 60) * scale), int((sx + sw - 20) * scale)),
                  (int((sy - 3) * scale), int((sy + 4) * scale)), fill, pal["lighter"])
    below = median_color(image, box_px((sx + 60, sy + 4, sw - 80, 4), scale))
    # The field's text, the caret (the accent) apart.
    field_box = box_px((sx + 32, sy + 4, sw - 40, sh - 8), scale)
    field = ink(image, field_box, below, pal["placeholder"]) or ink(image, field_box, below, pal["query"])
    if field:
        left, top, width, height = field["box"]
        field = {"left": left / scale, "top": top / scale, "right": (left + width) / scale,
                 "bottom": (top + height) / scale, "color": field["color"]}
    glass = ink_extent(image, (sx + 8, sy + 8, 24, sh - 16), below, title_color, scale)
    out["search"] = {"rule": found / scale if found else None, "text": field, "glyph": glass}
    empty = panel.get("empty")
    if empty:
        out["empty"] = ink_extent(image, as_tuple(empty["rect"]), fill, title_color, scale)
    return out


def dimmer_sample(image, list_rect, scale=1.0):
    x, y, w, h = list_rect
    return median_color(image, box_px((x + 3, y + h - 12, 5, 6), scale))


def harness_actions(report, name, capture, declared, manifest, native, scale, pal, crops):
    """The native Actions panel against its declaration."""
    panel = declared.get("actions")
    list_rect = as_tuple(manifest["list"])
    sample = dimmer_sample(native, list_rect, scale)
    colors = manifest["declared"]["colors"]
    if not panel:
        if sample:
            UNDIMMED[("native", name)] = sample
        return
    measured = measure_panel(native, panel, scale, pal, colors)
    rect = as_tuple(panel["rect"])
    edges = measured["edges"]
    for prop, index in (("left", 0), ("top", 1), ("width", 2), ("height", 3)):
        report.check("harness-native", name, capture, "actions-panel", prop, edges[index] if edges else None,
                     rect[index], LIMITS["edge_px"], "px")
    crops.append(("actions-panel", rect, None))
    header = panel.get("header")
    if header:
        tile, edges = as_tuple(header["tile"]), measured["header"]["tile"]
        for prop, index in (("left", 0), ("top", 1), ("width", 2), ("height", 3)):
            report.check("harness-native", name, capture, "actions-header", f"tile {prop}",
                         edges[index] if edges else None, tile[index], LIMITS["edge_px"], "px")
        title = measured["header"]["title"]
        report.check("harness-native", name, capture, "actions-header", "title ink left", title and title["left"],
                     tile[0] + tile[2] + 8, LIMITS["edge_px"] * 1.5, "px", "a glyph's side bearing allowed")
    selected_alpha = hex_rgba(colors["actionSelected"])[3]
    for row, found in zip(panel["rows"], measured["rows"]):
        subject = f"action:{row['label']}"
        report.check("harness-native", name, capture, subject, f"wash alpha ({'selected' if row['selected'] else 'rest'})",
                     found["alpha"], selected_alpha if row["selected"] else 0, LIMITS["flat_fill_levels"], "levels")
        glyph = as_tuple(row["glyph"])
        ink_box = found["glyph"]
        report.check("harness-native", name, capture, subject, "glyph ink inside its box",
                     "yes" if ink_box and ink_box["left"] >= glyph[0] - 0.5 and ink_box["right"] <= glyph[0] + glyph[2] + 0.5
                     and ink_box["top"] >= glyph[1] - 0.5 and ink_box["bottom"] <= glyph[1] + glyph[3] + 0.5 else "no",
                     "yes", 0, "")
        report.check("harness-native", name, capture, subject, "label ink left", found["text"] and found["text"]["left"],
                     glyph[0] + glyph[2] + 10, LIMITS["edge_px"] * 1.5, "px", "a glyph's side bearing allowed")
    for group, found in zip(panel["groups"], measured["groups"]):
        rect = as_tuple(group["rect"])
        report.check("harness-native", name, capture, f"action-group:{group['text']}", "ink left",
                     found["ink"] and found["ink"]["left"], rect[0] + 8, LIMITS["edge_px"] * 1.5, "px",
                     "a glyph's side bearing allowed")
    rule_alpha = hex_rgba(colors["actionRule"])[3]
    for rule, found in zip(panel["rules"], measured["rules"]):
        report.check("harness-native", name, capture, "action-rule", "y", found["y"], as_tuple(rule)[1],
                     LIMITS["edge_px"], "px")
        report.check("harness-native", name, capture, "action-rule", "alpha", found["alpha"], rule_alpha,
                     LIMITS["flat_fill_levels"], "levels")
    search = as_tuple(panel["search"])
    report.check("harness-native", name, capture, "actions-search", "rule y", measured["search"]["rule"], search[1],
                 LIMITS["edge_px"], "px")
    text = measured["search"]["text"]
    report.check("harness-native", name, capture, "actions-search", "field ink left", text and text["left"],
                 search[0] + 14 + 15 + 10 + 2, LIMITS["edge_px"] * 2, "px",
                 "after the field's 2px inset; the first glyph's side bearing allowed")
    if panel.get("empty"):
        rect = as_tuple(panel["empty"]["rect"])
        found = measured.get("empty")
        report.check("harness-native", name, capture, "actions-empty", "ink left", found and found["left"],
                     rect[0] + 10, LIMITS["edge_px"] * 1.5, "px", "a glyph's side bearing allowed")
    # The dimmer: the list's padding against the same spot with the panel
    # closed (an earlier capture), or the panel's own color.
    undimmed = UNDIMMED.get(("native", name)) or hex_rgba(colors["panelSolid"])[:3]
    expected = composite(colors["actionsDimmer"], undimmed)
    report.check("harness-native", name, capture, "actions-dimmer", "dimmed list (max channel)",
                 color_delta(sample, expected), 0, LIMITS["flat_fill_levels"], "levels")


def parity_actions(report, name, capture, declared, manifest, native, scale, state, reference_image, pal, crops):
    """The Actions panel, native against reference: placed the same over
    the footer, and each part's anatomy the same. The reference lists more
    entries (see ACTIONS_CONTENT), so its panel is taller: its top and the
    entries are compared relative to the panel's top, entries by what they
    do, and its bottom and right edges where they are."""
    panel, ref_panel = declared.get("actions"), state.get("panel")
    list_rect = as_tuple(state["list"])
    ref_sample = dimmer_sample(reference_image, list_rect)
    if not ref_panel:
        if ref_sample:
            UNDIMMED[("reference", name)] = ref_sample
    report.check("parity", name, capture, "actions-panel", "open", "yes" if panel else "no",
                 "yes" if ref_panel else "no", 0, "")
    if not (panel and ref_panel):
        return
    colors = manifest["declared"]["colors"]
    mine = measure_panel(native, panel, scale, pal, colors)
    theirs = measure_panel(reference_image, ref_panel)
    crops.append(("parity-actions-panel", as_tuple(panel["rect"]), as_tuple(ref_panel["rect"])))
    a, b = mine["edges"], theirs["edges"]
    if a and b:
        report.check("parity", name, capture, "actions-panel", "left", a[0], b[0], LIMITS["edge_px"], "px")
        report.check("parity", name, capture, "actions-panel", "width", a[2], b[2], LIMITS["edge_px"], "px")
        report.check("parity", name, capture, "actions-panel", "bottom", a[1] + a[3], b[1] + b[3], LIMITS["edge_px"], "px")
    native_labels = [row["label"] for row in panel["rows"]]
    ref_labels = [row["label"] for row in ref_panel["rows"]]
    same = native_labels == ref_labels
    report.check("parity", name, capture, "actions-panel", "entries", " | ".join(native_labels),
                 " | ".join(ref_labels), 0, "", accepted=None if same else ACTIONS_CONTENT)
    top, ref_top = as_tuple(panel["rect"])[1], as_tuple(ref_panel["rect"])[1]
    if mine.get("header") and theirs.get("header"):
        ta, tb = mine["header"]["tile"], theirs["header"]["tile"]
        if ta and tb:
            for prop, index in (("left", 0), ("width", 2), ("height", 3)):
                report.check("parity", name, capture, "actions-header", f"tile {prop}", ta[index], tb[index],
                             LIMITS["edge_px"], "px")
            report.check("parity", name, capture, "actions-header", "tile top in panel", ta[1] - top, tb[1] - ref_top,
                         LIMITS["edge_px"], "px")
        na, nb = mine["header"]["title"], theirs["header"]["title"]
        report.check("parity", name, capture, "actions-header", "title", panel["header"]["title"],
                     ref_panel["header"]["title"], 0, "")
        if na and nb:
            report.check("parity", name, capture, "actions-header", "title ink left", na["left"], nb["left"],
                         LIMITS["edge_px"], "px")
            report.check("parity", name, capture, "actions-header", "title ink top in panel", na["top"] - top,
                         nb["top"] - ref_top, LIMITS["edge_px"], "px")
    ref_by_action = {}
    for row in theirs["rows"]:
        if row["action"] and row["action"] not in ref_by_action:
            ref_by_action[row["action"]] = row
    for row in mine["rows"]:
        ref_row = ref_by_action.get(row["action"])
        if not ref_row:
            continue
        subject = f"action:{row['action']}"
        report.check("parity", name, capture, subject, "height", row["rect"][3], ref_row["rect"][3], LIMITS["edge_px"], "px")
        report.check("parity", name, capture, subject, "left", row["rect"][0], ref_row["rect"][0], LIMITS["edge_px"], "px")
        report.check("parity", name, capture, subject, "wash alpha", row["alpha"], ref_row["alpha"],
                     LIMITS["flat_fill_levels"], "levels")
        if row["action"] == "invoke":
            report.check("parity", name, capture, subject, "top in panel", row["rect"][1] - top,
                         ref_row["rect"][1] - ref_top, LIMITS["edge_px"], "px")
        for part in ("glyph", "text"):
            pa, pb = row[part], ref_row[part]
            if pa and pb:
                report.check("parity", name, capture, subject, f"{part} ink left", pa["left"], pb["left"],
                             LIMITS["edge_px"], "px")
                report.check("parity", name, capture, subject, f"{part} ink top in row", pa["top"] - row["rect"][1],
                             pb["top"] - ref_row["rect"][1], LIMITS["edge_px"], "px")
        crops.append((f"parity-action-{row['action']}", row["rect"], ref_row["rect"]))
    for ga, gb in zip(mine["groups"], theirs["groups"]):
        report.check("parity", name, capture, "action-group", "text", ga["text"], gb["text"], 0, "")
        if ga["ink"] and gb["ink"]:
            report.check("parity", name, capture, "action-group", "ink left", ga["ink"]["left"], gb["ink"]["left"],
                         LIMITS["edge_px"], "px")
            report.check("parity", name, capture, "action-group", "ink top in panel", ga["ink"]["top"] - top,
                         gb["ink"]["top"] - ref_top, LIMITS["edge_px"], "px")
    if mine["rules"] and theirs["rules"]:
        ra, rb = mine["rules"][0], theirs["rules"][0]
        if ra["y"] is not None and rb["y"] is not None:
            report.check("parity", name, capture, "action-rule", "y in panel", ra["y"] - top, rb["y"] - ref_top,
                         LIMITS["edge_px"], "px")
        report.check("parity", name, capture, "action-rule", "alpha", ra["alpha"], rb["alpha"],
                     LIMITS["flat_fill_levels"], "levels")
    sa, sb = mine["search"], theirs["search"]
    bottom, ref_bottom = top + as_tuple(panel["rect"])[3], ref_top + as_tuple(ref_panel["rect"])[3]
    if sa["rule"] is not None and sb["rule"] is not None:
        report.check("parity", name, capture, "actions-search", "rule above the bottom", bottom - sa["rule"],
                     ref_bottom - sb["rule"], LIMITS["edge_px"], "px")
    for part in ("text", "glyph"):
        if sa[part] and sb[part]:
            report.check("parity", name, capture, "actions-search", f"{part} ink left", sa[part]["left"],
                         sb[part]["left"], LIMITS["edge_px"], "px")
            if sa["rule"] is not None and sb["rule"] is not None:
                report.check("parity", name, capture, "actions-search", f"{part} ink top below the rule",
                             sa[part]["top"] - sa["rule"], sb[part]["top"] - sb["rule"], LIMITS["edge_px"], "px")
    if mine.get("empty") or theirs.get("empty"):
        ea, eb = mine.get("empty"), theirs.get("empty")
        report.check("parity", name, capture, "actions-empty", "shown", "yes" if ea else "no", "yes" if eb else "no", 0, "")
        if ea and eb:
            report.check("parity", name, capture, "actions-empty", "ink left", ea["left"], eb["left"],
                         LIMITS["edge_px"], "px")
            report.check("parity", name, capture, "actions-empty", "ink top in panel", ea["top"] - top,
                         eb["top"] - ref_top, LIMITS["edge_px"], "px")
    # The dimmer, as each side darkens its own list.
    mine_sample = dimmer_sample(native, as_tuple(manifest["list"]), scale)
    native_undimmed, ref_undimmed = UNDIMMED.get(("native", name)), UNDIMMED.get(("reference", name))
    if native_undimmed and ref_undimmed and mine_sample and ref_sample:
        dimmer = manifest["declared"]["colors"]["actionsDimmer"]
        report.check("parity", name, capture, "actions-dimmer", "dimmed list off its own composite (max channel)",
                     color_delta(mine_sample, composite(dimmer, native_undimmed)),
                     color_delta(ref_sample, composite(dimmer, ref_undimmed)), LIMITS["flat_fill_levels"], "levels",
                     "each side's dimmed list against rgba(6,7,8,.34) over its own undimmed list")


def native_wash_fill(image, row, scale):
    """The fill a row's tile sits on: the row's own wash (or the panel),
    just right of the tile."""
    x, y, w, h = scaled(as_tuple(row["rect"]), scale)
    return median_color(image, (x + 2 * scale, y + 4 * scale, x + 8 * scale, y + 6 * scale))


def compare_tiles(report, name, capture, manifest, native, scale, crops):
    """The tile family at each size: each tile where it is declared, and its
    glyph centered in it."""
    data = native.load()
    for index, tile in enumerate(manifest["tiles"]):
        rect = as_tuple(tile["rect"])
        app = tile["tone"] == "app"
        x, y, w, h = scaled(rect, scale)
        background = median_color(native, (x - 6 * scale, y + h / 2 - 2 * scale, x - 3 * scale, y + h / 2 + 2 * scale))
        measured = measure_tile(native, rect, app, APP_GLYPH if app else COMMAND_GLYPH, background, scale)
        subject = f"tile:{tile['size']}-{tile['tone']}"
        edges = measured and measured["edges"]
        for prop, i in (("left", 0), ("top", 1), ("width", 2), ("height", 3)):
            report.check("harness-native", name, capture, subject, prop, edges[i] if edges else None, rect[i],
                         LIMITS["edge_px"], "px")
        glyph = measured and measured["glyph"]
        if glyph:
            center = (glyph["left"] + glyph["width"] / 2, glyph["top"] + glyph["height"] / 2)
            report.check("harness-native", name, capture, subject, "glyph centered (x)", center[0], rect[2] / 2,
                         LIMITS["edge_px"] * 1.5, "px", "the glyph's ink, not its viewBox, so allow 1.5px")
            report.check("harness-native", name, capture, subject, "glyph centered (y)", center[1], rect[3] / 2,
                         LIMITS["edge_px"] * 1.5, "px")
        crops.append((subject, rect, None))
    # The same glyph at the same size: the application's 2px stroke against
    # the command's 1.6px.
    by_size = {}
    for tile, cores in measured_cores(manifest, native, scale):
        by_size.setdefault(tile["size"], {})[tile["tone"]] = cores
    for size, tones in by_size.items():
        # At the Actions header's 11px the two strokes are 0.9 and 0.7px:
        # under a pixel, so their cores do not separate.
        if size == "mini":
            continue
        if "app" in tones and "command" in tones and tones["command"]:
            report.check("harness-native", name, capture, f"tile:{size}", "app stroke heavier than command (%)",
                         100 * (tones["app"] / tones["command"] - 1), 25, 15, "%",
                         "same pen glyph and size; 2px against 1.6px is +25% stroke")


def measured_cores(manifest, native, scale):
    for tile in manifest["tiles"]:
        rect = as_tuple(tile["rect"])
        app = tile["tone"] == "app"
        x, y, w, h = scaled(rect, scale)
        background = median_color(native, (x - 6 * scale, y + h / 2 - 2 * scale, x - 3 * scale, y + h / 2 + 2 * scale))
        measured = measure_tile(native, rect, app, APP_GLYPH if app else COMMAND_GLYPH, background, scale)
        yield tile, measured and measured["glyph"] and measured["glyph"]["count"]


# ------------------------------------------------------------- evidence


def write_images(out_dir, name, capture, native, reference_image, crops, scale):
    base = Path(out_dir, name)
    base.mkdir(parents=True, exist_ok=True)
    record = {"scenario": name, "capture": capture, "native": f"{name}/{capture}-native.png"}
    native.save(base / f"{capture}-native.png")
    gap = 8
    if reference_image is not None:
        record["reference"] = f"{name}/{capture}-reference.png"
        reference_image.save(base / f"{capture}-reference.png")
        side = Image.new("RGB", (native.width + gap + reference_image.width, max(native.height, reference_image.height)), "#FF00FF")
        side.paste(native, (0, 0))
        side.paste(reference_image, (native.width + gap, 0))
        side.save(base / f"{capture}-side-by-side.png")
        record["sideBySide"] = f"{name}/{capture}-side-by-side.png"
        if native.size == reference_image.size:
            diff = ImageChops.difference(native, reference_image).point(lambda v: min(255, v * 4))
            diff.save(base / f"{capture}-diff-x4.png")
            record["diff"] = f"{name}/{capture}-diff-x4.png"
    overlay = native.copy()
    draw = ImageDraw.Draw(overlay)
    for crop_name, native_rect, ref_rect in crops:
        if native_rect:
            x, y, w, h = scaled(native_rect, scale)
            draw.rectangle((x, y, x + w - 1, y + h - 1), outline="#00FF66")
        if ref_rect:
            x, y, w, h = ref_rect
            draw.rectangle((x, y, x + w - 1, y + h - 1), outline="#FF3399")
    overlay.save(base / f"{capture}-overlay.png")
    record["overlay"] = f"{name}/{capture}-overlay.png"
    record["crops"] = []
    for crop_name, native_rect, ref_rect in crops:
        pad = 8
        if not ref_rect or reference_image is None:
            if crop_name.startswith("parity-"):
                continue
            nx, ny, nw, nh = scaled(native_rect, scale)
            safe = "".join(c if c.isalnum() or c in "-_" else "-" for c in crop_name)[:60]
            native.crop(clamp_box(native, (nx - pad, ny - pad, nx + nw + pad, ny + nh + pad))).save(
                base / f"{capture}-native-crop-{safe}.png")
            record["crops"].append(f"{name}/{capture}-native-crop-{safe}.png")
            continue
        nx, ny, nw, nh = scaled(native_rect, scale)
        rx, ry, rw, rh = ref_rect
        n_crop = native.crop(clamp_box(native, (nx - pad, ny - pad, nx + nw + pad, ny + nh + pad)))
        r_crop = reference_image.crop(clamp_box(reference_image, (rx - pad, ry - pad, rx + rw + pad, ry + rh + pad)))
        pair = Image.new("RGB", (max(n_crop.width, r_crop.width), n_crop.height + gap + r_crop.height), "#FF00FF")
        pair.paste(n_crop, (0, 0))
        pair.paste(r_crop, (0, n_crop.height + gap))
        safe = "".join(c if c.isalnum() or c in "-_" else "-" for c in crop_name)[:60]
        pair.save(base / f"{capture}-crop-{safe}.png")
        record["crops"].append(f"{name}/{capture}-crop-{safe}.png")
    return record


def sensitivity(current, baseline):
    """Checks that passed in the baseline and fail now, with both values."""
    before = {check["id"]: check for check in baseline["checks"]}
    flips = []
    for check in current["checks"]:
        old = before.get(check["id"])
        if old and old["passed"] and not check["passed"]:
            value = "measured" if "measured" in check else "native"
            flips.append({
                "id": check["id"],
                "baseline": old.get(value),
                "now": check.get(value),
                "expected": check.get("expected", check.get("reference")),
                "delta": check["delta"],
                "limit": check["limit"],
                "unit": check["unit"],
            })
    return flips


def summary_markdown(result):
    lines = [f"# Visual workbench comparison: {result['meta']['label']}", ""]
    lines.append(f"Reference SHA-256 `{result['meta']['referenceSha256']}`, {result['meta']['chrome']}.")
    lines.append(f"Limits: {result['meta']['limits']}.")
    lines.append("")
    for scenario in result["meta"]["scenarios"]:
        lines.append(f"- `{scenario['name']}`: {scenario['dpi']} DPI, client {scenario['client']}, "
                     f"{scenario['material']}, perturbation {scenario['perturbation']}")
    lines.append("")
    lines.append("| Section | Passed | Failed | Accepted discrepancies |")
    lines.append("|---|---|---|---|")
    for section, totals in result["totals"].items():
        lines.append(f"| {section} | {totals['passed']} | {totals['failed']} | {totals.get('accepted', 0)} |")
    if "sensitivity" in result:
        lines += ["", "## Sensitivity: passed in the baseline, failing now", ""]
        lines.append("| Check | Baseline | Now | Expected | Delta | Limit |")
        lines.append("|---|---|---|---|---|---|")
        for flip in result["sensitivity"]:
            lines.append(f"| `{flip['id']}` | {flip['baseline']} | {flip['now']} | {flip['expected']} | "
                         f"{flip['delta']} | {flip['limit']} {flip['unit']} |")
    for section in ("harness-native", "harness-reference", "parity"):
        failed = [c for c in result["checks"] if c["section"] == section and not c["passed"] and not c.get("accepted")]
        lines += ["", f"## {section}: {len(failed)} failed", ""]
        if failed:
            lines.append("| Check | Native/measured | Reference/expected | Delta | Limit | Note |")
            lines.append("|---|---|---|---|---|---|")
            for check in failed:
                left = check.get("native", check.get("measured"))
                right = check.get("reference", check.get("expected"))
                lines.append(f"| `{check['scenario']}/{check['capture']}/{check['subject']}/{check['property']}` | {left} | "
                             f"{right} | {check['delta']} | {check['limit']} {check['unit']} | {check.get('note', '')} |")
    accepted = [c for c in result["checks"] if c.get("accepted")]
    if accepted:
        lines += ["", f"## Accepted discrepancies: {len(accepted)} (failing, with a recorded disposition)", ""]
        lines.append("| Check | Native/measured | Reference/expected | Delta | Limit | Disposition |")
        lines.append("|---|---|---|---|---|---|")
        for check in accepted:
            left = check.get("native", check.get("measured"))
            right = check.get("reference", check.get("expected"))
            lines.append(f"| `{check['scenario']}/{check['capture']}/{check['subject']}/{check['property']}` | {left} | "
                         f"{right} | {check['delta']} | {check['limit']} {check['unit']} | {check['accepted']} |")
    if result["nativeOnly"]:
        lines += ["", "## Native-only captures (no reference counterpart; not compared)", ""]
        for entry in result["nativeOnly"]:
            lines.append(f"- `{entry['scenario']}`: {', '.join(entry['captures'])}")
    if result["pending"]:
        lines += ["", "## Pending scenarios (reference saved; native fixture not registered yet)", ""]
        for entry in result["pending"]:
            lines.append(f"- `{entry['name']}` ({entry['board']} board, {entry['client'][0]:g}x{entry['client'][1]:g}): {entry['ticket']}")
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--native", required=True)
    parser.add_argument("--reference", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--baseline")
    parser.add_argument("--label", default="baseline")
    args = parser.parse_args()
    Path(args.out).mkdir(parents=True, exist_ok=True)
    result = compare(args.native, args.reference, args.out, args.label)
    if args.baseline:
        result["sensitivity"] = sensitivity(result, json.loads(Path(args.baseline).read_text(encoding="utf-8")))
    Path(args.out, "report.json").write_text(json.dumps(result, indent=2, ensure_ascii=False), encoding="utf-8")
    Path(args.out, "summary.md").write_text(summary_markdown(result), encoding="utf-8")
    for section, totals in result["totals"].items():
        print(f"{section}: {totals['passed']} passed, {totals['failed']} failed, {totals.get('accepted', 0)} accepted")
    if "sensitivity" in result:
        print(f"sensitivity: {len(result['sensitivity'])} checks flipped from passed to failed")


if __name__ == "__main__":
    main()
