#!/usr/bin/env python3
"""Compute, and optionally create, the next release tag of the manual.

The version is calendar-derived, and the git tag is the single source of truth. The
scheme is the one of the SnapshotPB app:

    <2-digit ISO week-year>.<ISO week 1-53>.<release count this week, >= 1>

The first release cut in ISO week 41 of 2026 is ``26.41.1``. The second release that
week is ``26.41.2``. The count is ``max(existing tags this ISO week) + 1``. The
repository stores no counter.

The ``release.yml`` workflow runs this compute core. The pure functions
(:func:`iso_week_year`, :func:`parse_tag`, :func:`next_count`, :func:`compute_version`)
take their inputs explicitly, so they are unit-testable without git. See
``tests/test_cut_version.py``.

Usage::

    cut_version.py --dry-run        # print the next version + tag, touch nothing
    cut_version.py --print          # print only the next version to stdout
    cut_version.py --tag [--ref R]  # create annotated tag v<version> at R (default HEAD)

``--tag`` creates only the local annotated tag, and it writes ``version=`` and ``tag=``
to ``$GITHUB_OUTPUT``. The workflow pushes the tag and dispatches the build, so that the
credentials and the permissions stay there.
"""

from __future__ import annotations

import argparse
import datetime
import os
import re
import subprocess
import sys

# A release tag: v<year>.<week>.<count>. Each component is a plain integer with no zero
# padding, the same as the emitted `year.week.count`. Each other form (a dev suffix, a
# malformed tag) gives None, so it never contributes to the count.
TAG_RE = re.compile(r"^v(\d+)\.(\d+)\.(\d+)$")

TAG_PREFIX = "v"


def iso_week_year(today: datetime.date) -> tuple[int, int]:
    """Return ``(2-digit ISO week-year, ISO week)`` for *today*.

    This uses the ISO-8601 week-year (``date.isocalendar()`` / ``date +%G``). The
    week-year goes with the ISO week, and it is not the calendar year in late December
    and early January. For example, 2025-12-29 belongs to ISO week-year 2026, week 1.
    The calendar year (``%Y``) there would give ``25.1`` after ``25.52``, a lower
    version after a higher one.

    The year is 2 digits (``2026`` -> ``26``), the same as the app.
    """
    iso = today.isocalendar()
    return iso[0] % 100, iso[1]


def parse_tag(tag: str) -> tuple[int, int, int] | None:
    """Parse ``v<year>.<week>.<count>`` into an int triple, or None if it does not match."""
    match = TAG_RE.match(tag.strip())
    if match is None:
        return None
    return int(match.group(1)), int(match.group(2)), int(match.group(3))


def next_count(tags: "list[str] | tuple[str, ...]", year: int, week: int) -> int:
    """The next release count for (*year*, *week*): ``max(existing) + 1``, else 1.

    A malformed tag and a tag from a different week or year are ignored.
    """
    counts = [
        count
        for parsed in (parse_tag(tag) for tag in tags)
        if parsed is not None
        for (tag_year, tag_week, count) in [parsed]
        if tag_year == year and tag_week == week
    ]
    return max(counts, default=0) + 1


def compute_version(tags: "list[str] | tuple[str, ...]", today: datetime.date) -> str:
    """The next version string, for example ``26.41.1``."""
    year, week = iso_week_year(today)
    count = next_count(tags, year, week)
    return f"{year}.{week}.{count}"


def tag_name(version: str) -> str:
    return f"{TAG_PREFIX}{version}"


# --------------------------------------------------------------------------------------
# git / CI glue (the unit tests do not run this part)
# --------------------------------------------------------------------------------------


def _run_git(args: "list[str]") -> str:
    """Run git, and show its stderr on a failure.

    ``capture_output`` hides the diagnostics of git. Without this function, a bad
    ``--ref`` would show only a bare CalledProcessError traceback in the Actions log.
    """
    try:
        result = subprocess.run(
            ["git", *args],
            check=True,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError:
        raise SystemExit("git is not installed or not on PATH")
    except subprocess.CalledProcessError as error:
        detail = (error.stderr or error.stdout or "").strip() or f"exit status {error.returncode}"
        raise SystemExit(f"git {' '.join(args)} failed: {detail}")
    return result.stdout.strip()


def existing_tags() -> "list[str]":
    """All release-shaped tags in the repository (``v*``)."""
    out = _run_git(["tag", "--list", f"{TAG_PREFIX}*"])
    return [line.strip() for line in out.splitlines() if line.strip()]


def create_tag(version: str, ref: str) -> str:
    """Create an annotated tag ``v<version>`` at *ref*. Return the tag name."""
    tag = tag_name(version)
    if tag in existing_tags():
        raise SystemExit(f"refusing to create {tag}: it already exists")
    _run_git(["tag", "-a", tag, "-m", f"Release {tag}", ref])
    return tag


def _emit_github_output(version: str, tag: str) -> None:
    output_path = os.environ.get("GITHUB_OUTPUT")
    if not output_path:
        return
    with open(output_path, "a", encoding="utf-8") as handle:
        handle.write(f"version={version}\n")
        handle.write(f"tag={tag}\n")


def main(argv: "list[str] | None" = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--dry-run", action="store_true", help="print the next version/tag and exit; touch nothing")
    mode.add_argument("--print", dest="print_only", action="store_true", help="print only the next version to stdout")
    mode.add_argument("--tag", action="store_true", help="create the annotated tag locally (does not push)")
    parser.add_argument("--ref", default="HEAD", help="commit-ish to tag with --tag (default: HEAD)")
    args = parser.parse_args(argv)

    tags = existing_tags()
    today = datetime.datetime.now(datetime.timezone.utc).date()
    version = compute_version(tags, today)
    tag = tag_name(version)

    if args.print_only:
        print(version)
        return 0

    if args.tag:
        created = create_tag(version, args.ref)
        _emit_github_output(version, created)
        print(f"Created tag {created} at {args.ref}")
        return 0

    # The default is the same as --dry-run, so that a bare invocation is always safe.
    print(f"Next version: {version}")
    print(f"Next tag:     {tag}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
