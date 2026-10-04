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
DARK = {"lighter": True, "muted": MUTED, "query": (243, 243, 245), "placeholder": (134, 135, 140)}


def palette(manifest):
    """The native capture's palette, from its manifest's declared colors."""
    colors = manifest["declared"]["colors"]
    return {
        "lighter": manifest.get("appearance", "dark") == "dark",
        "muted": hex_rgba(colors["textMuted"])[:3],
        "query": hex_rgba(colors.get("textQuery", "#F3F3F5FF"))[:3],
        "placeholder": hex_rgba(colors["textPlaceholder"])[:3],
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
    }


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
    native_rows = {}
    for row in declared["rows"]:
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
    report.check("harness-native", name, capture, "footer", "tint alpha", footer["alpha"],
                 hex_rgba(manifest["declared"]["colors"]["footerTint"])[3], LIMITS["flat_fill_levels"], "levels")
    if declared.get("actionButton"):
        button = as_tuple(declared["actionButton"])
        measured = measure_cap(native, button, scale, pal["lighter"])
        edges = measured and measured["edges"]
        for prop, index in (("left", 0), ("top", 1), ("width", 2), ("height", 3)):
            report.check("harness-native", name, capture, "footer-primary", f"button {prop}",
                         edges[index] if edges else None, button[index], LIMITS["edge_px"], "px")
        crops.append(("footer-primary", button, None))
    crops.append(("search-header", search, None))
    if manifest["scenario"].get("frame"):
        compare_frame(report, name, capture, declared, manifest, native, scale, ref_capture, reference_image, crops)

    if ref_capture is None:
        return
    # ---- the reference side against its DOM
    state = ref_capture["state"]
    ref_rows = {}
    for row in state["rows"]:
        if not row["visible"]:
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
    native_selected = sorted(t for t, (r, _) in native_rows.items() if r["selected"])
    ref_selected = sorted(r["title"] for r in state["rows"] if r["selected"])
    report.check("parity", name, capture, "selection", "selected rows", ", ".join(native_selected),
                 ", ".join(ref_selected), 0, "", "the reference selects the row the pointer moves over; production only washes it")
    native_hovered = sorted(t for t, (r, _) in native_rows.items() if r["hovered"] and not r["selected"])
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
        if n_edges and r_edges:
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
    buttons = state.get("footerButtons") or []
    if declared.get("actionButton") and buttons:
        n_button = as_tuple(declared["actionButton"])
        measured = measure_cap(native, n_button, scale)
        n_right = measured["edges"][0] + measured["edges"][2] if measured and measured["edges"] else None
        r_last = as_tuple(buttons[-1]["rect"])
        report.check("parity", name, capture, "frame", "footer content right edge", n_right, r_last[0] + r_last[2],
                     LIMITS["edge_px"], "px", "the footer's 8px right padding: native's rightmost button against the reference's")


def fmt_color(color):
    return "#" + "".join(f"{int(round(c)):02X}" for c in color[:3])


def compare_keycaps(report, name, capture, manifest, native, scale, ref_capture, reference_image, crops):
    groups = (ref_capture or {}).get("state", {}).get("keycaps", {}) if ref_capture else {}
    for cap in manifest["keycaps"]:
        rect = as_tuple(cap["rect"])
        measured = measure_cap(native, rect, scale)
        edges = measured and measured["edges"]
        subject = f"keycap:{cap['binding']}"
        for prop, index in (("left", 0), ("top", 1), ("width", 2), ("height", 3)):
            report.check("harness-native", name, capture, subject, prop, edges[index] if edges else None, rect[index],
                         LIMITS["edge_px"], "px")
        group = groups.get(cap["group"])
        if not group:
            crops.append((subject, rect, None))
            continue
        ref_rect = as_tuple(group["rect"])
        crops.append(("parity-" + subject, rect, ref_rect))
        ref_labels = [c["label"] for c in group["caps"]]
        report.check("parity", name, capture, subject, "labels", " | ".join([cap["label"]]), " | ".join(ref_labels), 0, "",
                     "production shows one cap with the chord's text; the reference one cap per key")
        report.check("parity", name, capture, subject, "cap height", edges[3] if edges else None,
                     group["caps"][0]["rect"]["height"], LIMITS["edge_px"], "px")
        report.check("parity", name, capture, subject, "group width", edges[2] if edges else None, ref_rect[2],
                     LIMITS["edge_px"], "px", "label widths are controlled: same effective binding on both sides")
        ref_first = as_tuple(group["caps"][0]["rect"])
        ref_measured = measure_cap(reference_image, ref_first)
        report.check("parity", name, capture, subject, "cap fill alpha", measured and measured["alpha"],
                     ref_measured and ref_measured["alpha"], LIMITS["flat_fill_levels"], "levels",
                     "the reference's Enter cap is the lime accent" if cap["group"] == "footer-primary" else None)
        report.check("parity", name, capture, subject, "label font",
                     f"{manifest['declared']['geometry']['rowTitleSize']:g}px Geist 500",
                     f"{group['font']['size']} {group['font']['family'].split(',')[0].strip(chr(34))} {group['font']['weight']}",
                     0, "", "the reference's keycaps set Geist Mono 11/500")


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
