"""Rectangular text blocks with a stem-pivot column, plus composition ops.

A ``Block`` is an axis-aligned rectangle of characters together with a single
"pivot" column. The pivot marks where the plant's stem passes through the
block so that stacked blocks line up cleanly and branches can be attached at
deterministic positions. All composition keeps the invariant that every row
has the same width as the block.
"""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class Block:
    """A rectangular text block with a stem pivot column."""

    lines: list[str] = field(default_factory=list)
    pivot: int = 0

    def __post_init__(self) -> None:
        if not self.lines:
            self.lines = [""]
        width = max(len(line) for line in self.lines)
        self.lines = [line.ljust(width) for line in self.lines]

    @property
    def width(self) -> int:
        return len(self.lines[0])

    @property
    def height(self) -> int:
        return len(self.lines)

    def render(self) -> str:
        return "\n".join(line.rstrip() for line in self.lines)

    def pad(self, left: int = 0, right: int = 0, top: int = 0, bottom: int = 0) -> "Block":
        new_width = self.width + left + right
        padded_rows = [(" " * left) + line + (" " * right) for line in self.lines]
        top_rows = [" " * new_width for _ in range(top)]
        bot_rows = [" " * new_width for _ in range(bottom)]
        return Block(top_rows + padded_rows + bot_rows, self.pivot + left)


def _align_pivots(blocks: list[Block]) -> tuple[list[Block], int]:
    """Pad blocks so all share the same pivot column and width."""
    if not blocks:
        return [], 0
    target_pivot = max(b.pivot for b in blocks)
    max_right = max(b.width - b.pivot for b in blocks)
    aligned = []
    for b in blocks:
        left = target_pivot - b.pivot
        right = max_right - (b.width - b.pivot)
        aligned.append(b.pad(left=left, right=right))
    return aligned, target_pivot


def stack(top: Block, bottom: Block, connector: str = "|") -> Block:
    """Place ``bottom`` beneath ``top`` with a single-char connector stem.

    The resulting block's pivot matches both children after alignment.
    """
    aligned, pivot = _align_pivots([top, bottom])
    top_a, bot_a = aligned
    width = top_a.width
    connector_row = " " * pivot + connector + " " * (width - pivot - 1)
    return Block(top_a.lines + [connector_row] + bot_a.lines, pivot)


def stack_many(blocks: list[Block], connector: str = "|") -> Block:
    """Stack blocks top-to-bottom in order, inserting connector rows between."""
    if not blocks:
        return Block()
    result = blocks[0]
    for nxt in blocks[1:]:
        result = stack(result, nxt, connector=connector)
    return result


def branch(tops: list[Block], gap: int = 3) -> Block:
    """Render multiple sub-plants as alternation branches converging down.

    Each sub-plant keeps its internal pivot. We space them horizontally with
    ``gap`` columns between neighbours, then draw ``\\``/``|``/``/`` diagonals
    beneath them so they converge onto a single stem at the bottom.
    """
    if len(tops) == 1:
        return tops[0]

    heights = [b.height for b in tops]
    max_height = max(heights)
    tops = [b.pad(top=max_height - b.height) for b in tops]

    pivots: list[int] = []
    pieces: list[list[str]] = []
    running_width = 0
    for i, b in enumerate(tops):
        if i > 0:
            running_width += gap
        pivots.append(running_width + b.pivot)
        pieces.append(b.lines)
        running_width += b.width

    total_width = running_width
    top_rows = ["".join(slice_row(pieces, row, gap)) for row in range(max_height)]

    target_pivot = (pivots[0] + pivots[-1]) // 2
    max_delta = max(abs(p - target_pivot) for p in pivots)

    diag_rows: list[str] = []
    for step in range(1, max_delta + 1):
        row = [" "] * total_width
        for p in pivots:
            if p == target_pivot:
                col = target_pivot
                glyph = "|"
            elif p < target_pivot:
                col = min(p + step, target_pivot)
                glyph = "|" if col == target_pivot else "\\"
            else:
                col = max(p - step, target_pivot)
                glyph = "|" if col == target_pivot else "/"
            if 0 <= col < total_width:
                row[col] = glyph
        diag_rows.append("".join(row))

    all_lines = top_rows + diag_rows
    return Block(all_lines, target_pivot)


def slice_row(pieces: list[list[str]], row: int, gap: int) -> list[str]:
    """Concatenate the same row index from each piece with gap spaces between."""
    out: list[str] = []
    for i, piece in enumerate(pieces):
        if i > 0:
            out.append(" " * gap)
        out.append(piece[row])
    return out
