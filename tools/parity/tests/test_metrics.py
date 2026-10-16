import sys
from pathlib import Path
import unittest
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from pdf_metrics import ssim, match_lines, parse_lines, compare_pages


class Metrics(unittest.TestCase):
    def test_ssim_identity_shift_and_deleted_picture(self):
        a = np.full((64, 96), 255, dtype=np.uint8)
        a[12:40, 20:52] = 0
        self.assertAlmostEqual(ssim(a, a), 1)
        self.assertLess(ssim(a, np.roll(a, 2, axis=1)), 1)
        self.assertLess(ssim(a, np.full_like(a, 255)), 1)
        with self.assertRaises(ValueError):
            ssim(a, a[:, :-1])

    def test_line_matching_preserves_order_and_limits_work(self):
        self.assertEqual(
            match_lines(
                ["intro", "one two", "three", "end"],
                ["intro", "one", "two three", "end"],
            ),
            [(0, 0), (3, 3)],
        )
        with self.assertRaisesRegex(ValueError, "limit"):
            match_lines(["a"] * 100, ["b"] * 100, max_cells=10)

    def test_positions_and_same_words_different_line_breaks(self):
        xml = '<doc><page><flow><block><line><word xMin="10" yMin="20">One</word><word xMin="30" yMin="20">&amp;</word><word xMin="40" yMin="20">two</word></line></block></flow></page></doc>'
        lines = parse_lines(xml)
        self.assertEqual(
            lines, [{"page": 0, "x": 10.0, "y": 20.0, "text": "One & two"}]
        )
        white = np.full((64, 96), 255, dtype=np.uint8)
        pages = [{"gray": white, "rgb": np.repeat(white[:, :, None], 3, axis=2)}]
        moved = [dict(lines[0], x=30.0, y=50.0)]
        result = compare_pages(pages, pages, lines, moved)
        self.assertEqual(result["summary"]["same_break_pct"], 100)
        self.assertEqual(result["pages"][0]["dx"], 20)
        self.assertEqual(result["pages"][0]["dy"], 30)
        broken = [dict(lines[0], text="One &"), dict(lines[0], text="two", y=30.0)]
        result = compare_pages(pages, pages, lines, broken)
        self.assertEqual(result["summary"]["word_recall"], 1)
        self.assertLess(result["summary"]["same_break_pct"], 100)

    def test_page_count_disagreement_is_not_a_partial_area_score(self):
        with self.assertRaisesRegex(ValueError, "page"):
            compare_pages([], [{"gray": np.zeros((8, 8))}], [], [])
