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


if __name__ == "__main__":
    unittest.main()
