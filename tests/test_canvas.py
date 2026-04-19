"""Tests for the block/canvas primitives."""

from __future__ import annotations

from regex_garden.canvas import Block, branch, stack, stack_many


def test_block_normalises_to_uniform_width():
    b = Block(["ab", "c", "defg"], pivot=1)
    assert b.width == 4
    assert all(len(line) == 4 for line in b.lines)
    assert b.render().splitlines() == ["ab", "c", "defg"]  # render rstrips


def test_pad_shifts_pivot_and_keeps_rectangle():
    b = Block(["x"], pivot=0)
    padded = b.pad(left=2, right=3, top=1, bottom=1)
    assert padded.pivot == 2
    assert padded.width == 6
    assert padded.height == 3
    # the top and bottom padding rows are pure spaces
    assert padded.lines[0] == " " * 6
    assert padded.lines[-1] == " " * 6
    # the original character sits at the new pivot column
    assert padded.lines[1][2] == "x"


def test_stack_aligns_pivots_and_inserts_connector():
    top = Block(["T"], pivot=0)
    bot = Block(["BOT"], pivot=2)
    s = stack(top, bot, connector="|")
    # After alignment the combined pivot is max(0, 2) == 2.
    assert s.pivot == 2
    assert s.height == 3
    # Middle row is the connector placed at the pivot column.
    assert s.lines[1][s.pivot] == "|"


def test_stack_many_preserves_order():
    blocks = [Block([str(i)], pivot=0) for i in range(3)]
    s = stack_many(blocks, connector="|")
    rendered = s.render().splitlines()
    # The first block in the list ends up on top.
    assert rendered[0].strip() == "0"
    assert rendered[-1].strip() == "2"


def test_branch_converges_onto_single_pivot():
    tops = [Block(["L"], pivot=0), Block(["R"], pivot=0)]
    b = branch(tops, gap=3)
    # Target pivot is the midpoint of the two sub-pivots; each leaf sits at
    # its original column, then slashes bring them together.
    last = b.lines[-1]
    assert last[b.pivot] == "|"
    # Exactly one leaf on each side of the branch in the top row.
    top_row = b.lines[0]
    assert top_row.count("L") == 1
    assert top_row.count("R") == 1


def test_branch_single_child_is_identity():
    only = Block(["solo"], pivot=2)
    b = branch([only])
    assert b.lines == only.lines
    assert b.pivot == only.pivot
