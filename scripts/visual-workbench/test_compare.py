"""Offline tests for the visual workbench's measurement math; never launch
anything. They check the measuring itself on synthetic images with known
answers - not token values, which the captures check against the real
renders.

Run: python -m unittest discover -s scripts/visual-workbench -p test_compare.py
"""
import unittest

from PIL import Image, ImageDraw

import compare


def over_white(background, alpha):
    """`background` under a white overlay of `alpha` (0-1), as a browser or
    GPUI composites it."""
    return tuple(round(b + (255 - b) * alpha) for b in background)


class OverlayAlpha(unittest.TestCase):
    def test_white_alpha_recovers_the_overlay_over_any_background(self):
        for background in [(22, 23, 26), (40, 44, 50), (90, 90, 90)]:
            fill = over_white(background, 0.085)
            self.assertAlmostEqual(compare.white_alpha(fill, background), 0.085 * 255, delta=1.5)

    def test_black_alpha_recovers_a_darkening_overlay(self):
        background = (40, 41, 45)
        fill = tuple(round(b * (1 - 0.14)) for b in background)
        self.assertAlmostEqual(compare.black_alpha(fill, background), 0.14 * 255, delta=2.0)


class Runs(unittest.TestCase):
    def test_a_single_dark_sample_does_not_split_a_run(self):
        lit = [False, True, True, False, True, True, False, False, True]
        self.assertEqual(compare.lit_run(lit, 4), (1, 6))

    def test_a_two_sample_gap_separates_neighbors(self):
        # Two rows' washes with the list's 2px gap between them.
        lit = [True] * 5 + [False] * 2 + [True] * 5
        self.assertEqual(compare.lit_run(lit, 9), (7, 12))


class WashEdges(unittest.TestCase):
    def panel(self):
        image = Image.new("RGB", (200, 120), (22, 23, 26))
        draw = ImageDraw.Draw(image)
        # A rule above, a neighbor's wash 2px above, and the wash itself
        # with a dark tile shadow and bright text inside it.
        draw.line((0, 10, 199, 10), fill=(40, 41, 44))
        draw.rectangle((11, 14, 188, 37), fill=over_white((22, 23, 26), 0.085))
        draw.rounded_rectangle((11, 40, 188, 83), radius=10, fill=over_white((22, 23, 26), 0.085))
        draw.rectangle((19, 48, 23, 75), fill=(5, 5, 6))      # a tile's drop shadow
        draw.rectangle((24, 48, 51, 75), fill=(200, 90, 40))  # the tile
        draw.text((60, 55), "Title", fill=(237, 237, 239))
        return image

    def test_edges_ignore_the_neighbor_the_rule_and_the_content(self):
        image = self.panel()
        edges = compare.wash_edges(image, (11, 40, 178, 44), (22, 23, 26), 9)
        self.assertEqual(edges, (11, 40, 178, 44))

    def test_a_shifted_wash_is_measured_where_it_is(self):
        image = Image.new("RGB", (200, 80), (22, 23, 26))
        ImageDraw.Draw(image).rounded_rectangle((15, 20, 184, 63), radius=10, fill=(41, 42, 45))
        # Looked for at its declared place, 4px to the left.
        edges = compare.wash_edges(image, (11, 20, 178, 44), (22, 23, 26), 9)
        self.assertEqual(edges[0], 15)
        self.assertEqual(edges[2], 170)


class RowWash(unittest.TestCase):
    def test_the_alpha_holds_over_a_backdrop_that_varies_across_and_down(self):
        image = Image.new("RGB", (300, 120))
        data = image.load()
        for y in range(120):
            for x in range(300):
                data[x, y] = (20 + x // 15, 21 + x // 15, 25 + x // 15 - y // 30)
        row = (10, 30, 280, 44)
        for y in range(30, 74):
            for x in range(10, 290):
                data[x, y] = over_white(data[x, y], 0.085)
        wash = compare.row_wash(image, row)
        self.assertAlmostEqual(wash["alpha"], 0.085 * 255, delta=1.0)

    def test_an_unwashed_row_measures_no_alpha(self):
        image = Image.new("RGB", (300, 120), (22, 23, 26))
        self.assertAlmostEqual(compare.row_wash(image, (10, 30, 280, 44))["alpha"], 0, delta=0.5)


class Lines(unittest.TestCase):
    def test_a_rule_is_found_beside_a_wash_edge(self):
        image = Image.new("RGB", (300, 100), (22, 23, 26))
        draw = ImageDraw.Draw(image)
        draw.line((0, 64, 299, 64), fill=(37, 38, 41))                    # the rule
        draw.rectangle((10, 68, 289, 99), fill=over_white((22, 23, 26), 0.085))  # a wash below
        self.assertEqual(compare.hline(image, (0, 300), (60, 69)), 64)


class Ink(unittest.TestCase):
    def test_core_pixels_exclude_antialiased_edges_and_the_caret(self):
        image = Image.new("RGB", (100, 40), (22, 23, 26))
        draw = ImageDraw.Draw(image)
        draw.rectangle((20, 10, 29, 25), fill=(237, 237, 239))  # a glyph's core
        draw.line((19, 10, 19, 25), fill=(100, 100, 102))       # its antialiased edge
        draw.line((40, 8, 40, 30), fill=(201, 238, 106))        # the lime caret
        found = compare.ink(image, (0, 0, 100, 40), (22, 23, 26), (237, 237, 239))
        self.assertEqual(found["box"], (20, 10, 10, 16))
        self.assertEqual(found["color"], (237, 237, 239))

    def test_a_stem_split_across_two_columns_still_marks_the_ink_top(self):
        # A 'b': its ascender's stem falls half in each of two columns, so
        # neither pixel is core; its bowl below is.
        background, text = (22, 23, 26), (185, 186, 190)
        half = tuple(round((b + t) / 2) for b, t in zip(background, text))
        image = Image.new("RGB", (40, 30), background)
        draw = ImageDraw.Draw(image)
        draw.rectangle((10, 5, 11, 6), fill=half)   # the split stem
        draw.rectangle((10, 7, 16, 12), fill=text)  # the bowl
        self.assertEqual(compare.ink(image, (0, 0, 40, 30), background, text)["box"][1], 7)
        self.assertEqual(compare.ink_top(image, (0, 0, 40, 30), background, text), 5)

    def test_a_faint_ring_is_not_ink(self):
        background, text = (22, 23, 26), (185, 186, 190)
        image = Image.new("RGB", (40, 30), background)
        draw = ImageDraw.Draw(image)
        draw.rectangle((4, 2, 30, 20), outline=over_white(background, 0.14))  # the chip's ring
        draw.rectangle((10, 8, 16, 12), fill=text)
        self.assertEqual(compare.ink_top(image, (0, 0, 40, 30), background, text), 8)
        self.assertIsNone(compare.ink_top(image, (0, 0, 40, 6), background, text))


class Frame(unittest.TestCase):
    def panel(self, radius=0):
        """A 760x518 dark panel with a 1px inset ring at 7.5%, a top
        highlight at 10%, optionally rounded over a darker backdrop."""
        backdrop, base = (8, 9, 10), (22, 23, 26)
        image = Image.new("RGB", (760, 518), backdrop if radius else base)
        draw = ImageDraw.Draw(image)
        ring = over_white(base, 0.075)
        draw.rounded_rectangle((0, 0, 759, 517), radius=radius, fill=ring)
        draw.rounded_rectangle((1, 1, 758, 516), radius=max(0, radius - 1), fill=base)
        top = over_white(ring, 0.10)
        draw.line((radius, 0, 759 - radius, 0), fill=top)
        return image

    def test_the_inset_ring_measures_its_alpha_on_each_side(self):
        image = self.panel()
        for edge in ("left", "right", "bottom"):
            self.assertAlmostEqual(compare.edge_alpha(image, edge), 0.075 * 255, delta=1.5, msg=edge)
        combined = 255 * (1 - 0.925 * 0.9)
        self.assertAlmostEqual(compare.edge_alpha(image, "top"), combined, delta=1.5)

    def test_a_missing_ring_measures_none(self):
        image = Image.new("RGB", (760, 518), (22, 23, 26))
        self.assertAlmostEqual(compare.edge_alpha(image, "left"), 0, delta=0.5)

    def test_a_square_corner_has_no_inset_and_a_rounded_one_its_radius(self):
        self.assertEqual(compare.corner_inset(self.panel()), 0)
        inset = compare.corner_inset(self.panel(radius=18))
        # r(1 - 1/sqrt 2) = 5.3 for 18px, within the diagonal's pixel steps.
        self.assertTrue(4 <= inset <= 6, inset)


def over_black(background, alpha):
    return tuple(round(b * (1 - alpha)) for b in background)


class Keycaps(unittest.TestCase):
    def cap(self, x=20, y=20, w=36, h=20, label=True):
        """A reference .kbd on a dark panel: a white 7% fill, the black 35%
        bottom line under a white 8% ring, a bright label in the middle."""
        panel = (22, 23, 26)
        image = Image.new("RGB", (120, 70), panel)
        draw = ImageDraw.Draw(image)
        fill = over_white(panel, 0.07)
        draw.rounded_rectangle((x, y, x + w - 1, y + h - 1), radius=5, fill=over_white(fill, 0.08))
        draw.rounded_rectangle((x + 1, y + 1, x + w - 2, y + h - 2), radius=4, fill=fill)
        bottom = over_white(over_black(fill, 0.35), 0.08)
        draw.line((x + 5, y + h - 1, x + w - 6, y + h - 1), fill=bottom)
        if label:
            draw.rectangle((x + 8, y + 6, x + w - 9, y + 13), fill=(201, 202, 206))
        return image

    def test_edges_of_a_rounded_cap_with_a_label(self):
        image = self.cap()
        measured = compare.measure_key(image, (20, 20, 36, 20), False, (201, 202, 206))
        left, top, width, _ = measured["edges"]
        self.assertEqual((left, top, width), (20, 20, 36))
        self.assertAlmostEqual(measured["alpha"], 0.07 * 255, delta=1.5)

    def test_the_bottom_line_is_recovered_from_under_the_ring(self):
        measured = compare.measure_key(self.cap(), (20, 20, 36, 20), False, (201, 202, 206))
        self.assertAlmostEqual(measured["bottom"], 0.35 * 255, delta=4)

    def test_a_cap_off_whole_pixels_reports_no_bottom_line(self):
        self.assertIsNone(compare.bottom_line_alpha(self.cap(), (20, 20.5, 36, 20), (40, 41, 44)))


class ResultBoards(unittest.TestCase):
    """The no-results notice's disc and the answer card (#96)."""

    PANEL = (22, 23, 26)
    ACCENT = (201, 238, 106)

    def disc(self, x=20, y=20, size=44, ring=True, glyph=True):
        """The notice's disc: white 6% under a white 8% ring, a lighter
        glyph in its middle."""
        image = Image.new("RGB", (120, 90), self.PANEL)
        draw = ImageDraw.Draw(image)
        fill = over_white(self.PANEL, 0.06)
        box = (x, y, x + size - 1, y + size - 1)
        draw.ellipse(box, fill=over_white(fill, 0.08) if ring else fill)
        draw.ellipse((x + 1, y + 1, x + size - 2, y + size - 2), fill=fill)
        if glyph:
            draw.ellipse((x + 14, y + 14, x + 28, y + 28), outline=(163, 164, 169), width=2)
        return image

    def card(self, ring):
        """A 300x120 card: white 6%, a 1px accent ring when selected, a
        white value in the middle of its left half."""
        image = Image.new("RGB", (340, 160), self.PANEL)
        draw = ImageDraw.Draw(image)
        fill = over_white(self.PANEL, 0.06)
        draw.rounded_rectangle((20, 20, 319, 139), radius=14, fill=self.ACCENT if ring else fill)
        draw.rounded_rectangle((21, 21, 318, 138), radius=13, fill=fill)
        draw.rectangle((70, 60, 109, 89), fill=(255, 255, 255))  # the value's core
        return image

    def test_a_disc_with_a_glyph_measures_its_box_and_fill(self):
        measured = compare.measure_box(self.disc(), (20, 20, 44, 44))
        left, top, width, height = measured["edges"]
        # The circle's chord across its middle rows is its diameter, or a
        # pixel short of it on the rows either side.
        self.assertTrue(19 <= left <= 21 and 43 <= width <= 45, measured["edges"])
        self.assertEqual((top, height), (20, 44))
        self.assertAlmostEqual(measured["alpha"], 0.06 * 255, delta=1.5)

    def test_a_shifted_disc_is_measured_where_it_is(self):
        measured = compare.measure_box(self.disc(x=24), (20, 20, 44, 44))
        self.assertTrue(23 <= measured["edges"][0] <= 25, measured["edges"])

    def test_the_accent_ring_marks_only_the_selected_card(self):
        self.assertTrue(compare.accent_ring(self.card(ring=True), (20, 20, 300, 120)))
        self.assertFalse(compare.accent_ring(self.card(ring=False), (20, 20, 300, 120)))
        # Measured a row off, it is found in the row below.
        self.assertTrue(compare.accent_ring(self.card(ring=True), (20, 19, 300, 120)))

    def test_a_cards_edges_and_fill_hold_under_its_ring(self):
        for ring in (True, False):
            measured = compare.measure_box(self.card(ring), (20, 20, 300, 120))
            self.assertEqual(measured["edges"], (20, 20, 300, 120), ring)
            self.assertAlmostEqual(measured["alpha"], 0.06 * 255, delta=1.5)

    def test_a_values_ink_is_found_inside_its_widened_line_box(self):
        image = self.card(ring=True)
        fill = over_white(self.PANEL, 0.06)
        # Declared 4px off its ink, as shaping may leave it.
        found = compare.measure_text(image, (74, 56, 40, 40), fill, (255, 255, 255))
        self.assertEqual((found["left"], found["top"], found["right"]), (70, 60, 110))
        self.assertEqual((found["height"], found["center"]), (30, 90))
        self.assertIsNone(compare.measure_text(image, (74, 56, 40, 40), None, (255, 255, 255)))


class Accepted(unittest.TestCase):
    def test_a_failing_check_with_a_disposition_is_accepted_not_failed(self):
        report = compare.Report()
        report.check("parity", "s", "c", "frame", "corner", 0, 5, 1, "px", accepted="platform limit")
        report.check("parity", "s", "c", "frame", "edge", 0, 5, 1, "px")
        report.check("parity", "s", "c", "frame", "ok", 5, 5, 1, "px", accepted="unused")
        self.assertEqual([c.get("accepted") for c in report.checks], ["platform limit", None, None])
        self.assertEqual([c["passed"] for c in report.checks], [False, False, True])


class Sensitivity(unittest.TestCase):
    def test_only_checks_that_passed_before_and_fail_now_flip(self):
        def check(identifier, passed, value):
            return {"id": identifier, "passed": passed, "measured": value, "expected": 11, "delta": value - 11,
                    "limit": 1, "unit": "px"}

        baseline = {"checks": [check("a", True, 11), check("b", False, 20), check("c", True, 11)]}
        current = {"checks": [check("a", False, 15), check("b", False, 20), check("c", True, 11.5)]}
        flips = compare.sensitivity(current, baseline)
        self.assertEqual([flip["id"] for flip in flips], ["a"])
        self.assertEqual((flips[0]["baseline"], flips[0]["now"], flips[0]["delta"]), (11, 15, 4))


class SettingsShell(unittest.TestCase):
    """The Settings shell's measures (#97)."""

    def sidebar(self):
        # The sidebar's black 10% fill left of x 100, its 1px white rule at
        # x 99, and the page to the right.
        image = Image.new("RGB", (200, 120), (25, 26, 29))
        draw = ImageDraw.Draw(image)
        draw.rectangle((0, 0, 98, 119), fill=(22, 23, 26))
        draw.line((99, 0, 99, 119), fill=over_white((22, 23, 26), 0.06))
        return image

    def test_vline_finds_the_rule_not_the_step_between_two_fills(self):
        image = self.sidebar()
        self.assertEqual(compare.vline(image, (90, 110), (10, 110)), 99)

    def test_vline_finds_a_darker_rule_in_the_light_palette(self):
        image = Image.new("RGB", (200, 120), (240, 240, 242))
        ImageDraw.Draw(image).line((60, 0, 60, 119), fill=(225, 225, 227))
        self.assertEqual(compare.vline(image, (50, 70), (10, 110), lighter=False), 60)

    def test_a_ringed_box_is_its_fill_and_its_ring(self):
        # The search well: black 24% under a white 6% ring at 10,60 211x34,
        # with its magnifier and placeholder lighter than the fill inside.
        background = (22, 23, 26)
        fill = tuple(round(c * 0.76) for c in background)
        image = Image.new("RGB", (240, 120), background)
        draw = ImageDraw.Draw(image)
        draw.rectangle((10, 60, 220, 93), fill=over_white(fill, 0.06))
        draw.rectangle((11, 61, 219, 92), fill=fill)
        draw.rectangle((20, 70, 33, 83), fill=(142, 143, 148))   # the magnifier
        draw.rectangle((42, 70, 140, 84), fill=(134, 135, 140))  # the placeholder
        self.assertEqual(compare.ringed_box(image, (10, 60, 211, 34), background, fill), (10, 60, 211, 34))

    def test_a_ringed_box_is_measured_where_it_is(self):
        background = (22, 23, 26)
        fill = tuple(round(c * 0.76) for c in background)
        image = Image.new("RGB", (240, 120), background)
        draw = ImageDraw.Draw(image)
        draw.rectangle((12, 61, 222, 94), fill=over_white(fill, 0.06))
        draw.rectangle((13, 62, 221, 93), fill=fill)
        # Looked for at its declared place, 2px left of and 1px above it.
        edges = compare.ringed_box(image, (10, 60, 211, 34), background, fill)
        self.assertEqual(edges, (12, 61, 211, 34))


if __name__ == "__main__":
    unittest.main()
