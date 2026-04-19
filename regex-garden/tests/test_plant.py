"""End-to-end tests for the regex-to-plant renderer.

We intentionally assert on shape properties (presence of specific glyphs,
line counts) rather than on whole-string equality, so that tiny cosmetic
changes don't break the suite but regressions in structure still do.
"""

from __future__ import annotations

import re

import pytest

from regex_garden import plant


def lines(pattern: str) -> list[str]:
    return plant(pattern).splitlines()


def test_single_literal_has_leaf_and_ground():
    out = lines("a")
    assert any("a" in line for line in out)
    assert out[-1].strip(" ").startswith("~")


def test_sequence_stacks_characters_bottom_up():
    out = plant("abc")
    # Earliest token is at the base; latest token is near the top.
    a_row = next(i for i, l in enumerate(out.splitlines()) if "a" in l)
    c_row = next(i for i, l in enumerate(out.splitlines()) if "c" in l)
    assert c_row < a_row


def test_branch_renders_convergence():
    out = plant("ab|cd")
    # Both alternatives appear on the same row near the top.
    first_lines = out.splitlines()[:3]
    joined = "\n".join(first_lines)
    assert "a" in joined and "c" in joined
    # Diagonal convergence characters appear somewhere in the render.
    assert "/" in out and "\\" in out


def test_repeat_label_present_for_plus():
    assert "(x+)" in plant("a+")


def test_repeat_label_present_for_bounded():
    assert "(x{3})" in plant("a{3}")


def test_repeat_label_present_for_star():
    assert "(x*)" in plant("a*")


def test_char_class_is_flower_with_members():
    out = plant("[a-z]")
    assert "{a-z}" in out
    assert "(*)" in out


def test_negated_class_marked_with_caret():
    out = plant("[^abc]")
    assert "{^abc}" in out


def test_digit_category_renders_fruit():
    out = plant(r"\d")
    assert "digit" in out


def test_anchors_render_labels():
    out = plant(r"^hello$")
    assert "[^]" in out
    assert "[$]" in out


def test_any_char_renders_pollen():
    out = plant(".")
    assert "(*)" in out


def test_invalid_regex_raises():
    with pytest.raises(re.error):
        plant("(")


def test_plant_is_idempotent_and_deterministic():
    assert plant("abc") == plant("abc")
    assert plant("a+b") == plant("a+b")


def test_group_shows_reference_marker():
    out = plant("(ab)")
    assert "#1" in out
