#!/usr/bin/env python3
"""Unit tests for the compute core of scripts/cut_version.py.

Run with: python3 -m unittest discover -s scripts/tests

The tests use the pure functions only. They need no git and no network, so they run
anywhere.
"""

import datetime
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from cut_version import (  # noqa: E402
    compute_version,
    iso_week_year,
    next_count,
    parse_tag,
    tag_name,
)


class ParseTagTests(unittest.TestCase):
    def test_valid(self):
        self.assertEqual(parse_tag("v26.29.1"), (26, 29, 1))
        self.assertEqual(parse_tag("v26.1.1"), (26, 1, 1))
        self.assertEqual(parse_tag("v26.53.12"), (26, 53, 12))

    def test_strips_whitespace(self):
        self.assertEqual(parse_tag("  v26.29.1\n"), (26, 29, 1))

    def test_invalid(self):
        self.assertIsNone(parse_tag("26.29.1"))  # no v prefix
        self.assertIsNone(parse_tag("v26.29"))  # too few components
        self.assertIsNone(parse_tag("v26.29.1.4"))  # too many components
        self.assertIsNone(parse_tag("v26.29.1-dev+3-g"))  # dev suffix
        self.assertIsNone(parse_tag("vfoo"))
        self.assertIsNone(parse_tag(""))
        self.assertIsNone(parse_tag("garbage"))


class IsoWeekYearTests(unittest.TestCase):
    def test_regular_date(self):
        # 2026-07-23 is a Thursday in ISO week 30 of week-year 2026.
        self.assertEqual(iso_week_year(datetime.date(2026, 7, 23)), (26, 30))

    def test_first_week_of_year(self):
        # 2026-01-01 is a Thursday, so it is in ISO week 1 of 2026.
        self.assertEqual(iso_week_year(datetime.date(2026, 1, 1)), (26, 1))

    def test_late_december_in_next_iso_year(self):
        # 2025-12-29 is a Monday. Its Thursday is 2026-01-01, so it is in ISO week 1 of
        # 2026. The calendar year would give 25.
        self.assertEqual(iso_week_year(datetime.date(2025, 12, 29)), (26, 1))

    def test_early_january_in_previous_iso_year(self):
        # 2027-01-01 is a Friday. Its Thursday is 2026-12-31, so it is in ISO week 53 of
        # 2026. The calendar year would give 27.
        self.assertEqual(iso_week_year(datetime.date(2027, 1, 1)), (26, 53))


class NextCountTests(unittest.TestCase):
    def test_no_tags(self):
        self.assertEqual(next_count([], 26, 29), 1)

    def test_other_weeks_ignored(self):
        self.assertEqual(next_count(["v26.28.3", "v26.30.1"], 26, 29), 1)

    def test_other_years_ignored(self):
        self.assertEqual(next_count(["v25.29.4"], 26, 29), 1)

    def test_increments_max(self):
        self.assertEqual(next_count(["v26.29.1", "v26.29.3", "v26.29.2"], 26, 29), 4)

    def test_malformed_ignored(self):
        self.assertEqual(next_count(["v26.29.1", "v26.29.2-dev", "nightly", ""], 26, 29), 2)


class ComputeVersionTests(unittest.TestCase):
    def test_first_of_week(self):
        self.assertEqual(compute_version([], datetime.date(2026, 7, 23)), "26.30.1")

    def test_second_of_week(self):
        self.assertEqual(compute_version(["v26.30.1"], datetime.date(2026, 7, 23)), "26.30.2")

    def test_tag_name(self):
        self.assertEqual(tag_name("26.30.1"), "v26.30.1")


if __name__ == "__main__":
    unittest.main()
