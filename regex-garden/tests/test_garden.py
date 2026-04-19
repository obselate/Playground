"""Tests for the garden file parser and formatter."""

from __future__ import annotations

import pytest

from regex_garden.garden import (
    Garden,
    GardenEntry,
    GardenParseError,
    format_garden,
    parse_garden,
)


def test_parse_empty_text_gives_empty_garden():
    assert parse_garden("") == Garden(entries=[])


def test_parse_ignores_comments_and_blank_lines():
    text = """
    # This is a comment
    # Another comment

    - name: foo
      pattern: abc
    """
    g = parse_garden(text)
    assert g.names() == ["foo"]
    assert g.entries[0].pattern == "abc"


def test_parse_preserves_regex_backslashes_and_specials():
    text = r"""
    - name: digits
      pattern: \d{3}-\d{4}

    - name: weird
      pattern: a|b|[^xyz]+
    """
    g = parse_garden(text)
    patterns = {e.name: e.pattern for e in g.entries}
    assert patterns["digits"] == r"\d{3}-\d{4}"
    assert patterns["weird"] == "a|b|[^xyz]+"


def test_parse_rejects_pattern_without_name():
    with pytest.raises(GardenParseError):
        parse_garden("pattern: abc\n")


def test_parse_rejects_duplicate_pattern():
    text = """
    - name: foo
      pattern: abc
      pattern: def
    """
    with pytest.raises(GardenParseError):
        parse_garden(text)


def test_parse_rejects_unknown_directive():
    text = """
    - name: foo
      something: else
    """
    with pytest.raises(GardenParseError):
        parse_garden(text)


def test_parse_rejects_empty_name():
    with pytest.raises(GardenParseError):
        parse_garden("- name:\n  pattern: abc\n")


def test_format_roundtrip():
    g = Garden(
        entries=[
            GardenEntry("a", "abc"),
            GardenEntry("b", r"\d+"),
        ]
    )
    text = format_garden(g)
    reparsed = parse_garden(text)
    assert reparsed == g


def test_entry_missing_pattern_is_error():
    with pytest.raises(GardenParseError):
        parse_garden("- name: foo\n")
