"""Turn a regular expression into an ASCII plant.

The pipeline is:

1. Parse the pattern with the stdlib ``re._parser`` (fallback ``sre_parse``).
2. Walk the opcode tree and emit a :class:`canvas.Block` per node.
3. Stack siblings vertically so the pattern grows from ground to crown in
   left-to-right reading order: earlier tokens sit at the base, later tokens
   reach toward the sky.
4. Append ground and optional sky/root markers for anchors.

Each glyph is chosen so the plant is a faithful, if whimsical, mirror of the
regex structure: a ``+`` quantifier stacks the same shape multiple times, a
character class bundles its members into a single flower, and alternation
makes the stem fork.
"""

from __future__ import annotations

from typing import Any

from regex_garden.canvas import Block, branch, stack, stack_many

try:  # Python 3.11+ moved the parser; the old path is deprecated.
    from re import _parser as _sre_parse  # type: ignore[attr-defined]
    from re import _constants as _sre_const  # type: ignore[attr-defined]
except ImportError:  # pragma: no cover - very old Pythons only
    import sre_parse as _sre_parse  # type: ignore[no-redef]
    import sre_constants as _sre_const  # type: ignore[no-redef]


# Opcode constants we care about. Fetched by name so this module is robust
# against the stdlib shuffling between versions.
def _op(name: str) -> Any:
    return getattr(_sre_const, name)


LITERAL = _op("LITERAL")
NOT_LITERAL = _op("NOT_LITERAL")
ANY = _op("ANY")
IN = _op("IN")
BRANCH = _op("BRANCH")
MAX_REPEAT = _op("MAX_REPEAT")
MIN_REPEAT = _op("MIN_REPEAT")
SUBPATTERN = _op("SUBPATTERN")
AT = _op("AT")
CATEGORY = _op("CATEGORY")
RANGE = _op("RANGE")
NEGATE = _op("NEGATE")
ASSERT = _op("ASSERT")
ASSERT_NOT = _op("ASSERT_NOT")
GROUPREF = _op("GROUPREF")

# How many repetitions we actually draw. A ``{100}`` quantifier does not mean
# we paint 100 leaves; we cap at this many and label the rest.
REPEAT_RENDER_CAP = 3


GROUND_GLYPH = "~"
SKY_GLYPH = "-"


# --------------------------------------------------------------------------
# Leaf-level glyph selection
# --------------------------------------------------------------------------

def _literal_glyph(char: str) -> str:
    """Render a single literal character as a compact glyph.

    Printable characters render as themselves so the regex is still readable
    in the plant. Unprintable ones fall back to a placeholder.
    """
    if char.isprintable() and not char.isspace():
        return char
    return "?"


def _category_glyph(cat_name: str) -> str:
    """Map ``\\d``, ``\\w`` etc. to a recognisable flower glyph.

    The glyphs intentionally differ enough that you can eyeball which
    shorthand produced them.
    """
    return {
        "CATEGORY_DIGIT": "o",  # round like a numeric fruit
        "CATEGORY_NOT_DIGIT": "x",
        "CATEGORY_SPACE": "~",
        "CATEGORY_NOT_SPACE": "=",
        "CATEGORY_WORD": "#",  # dense like a word
        "CATEGORY_NOT_WORD": "%",
        "CATEGORY_LINEBREAK": "_",
        "CATEGORY_NOT_LINEBREAK": "^",
    }.get(cat_name, "?")


def _at_label(at_name: str) -> str:
    return {
        "AT_BEGINNING": "^",
        "AT_BEGINNING_STRING": "^",
        "AT_BEGINNING_LINE": "^",
        "AT_END": "$",
        "AT_END_STRING": "$",
        "AT_END_LINE": "$",
        "AT_BOUNDARY": "b",
        "AT_NON_BOUNDARY": "B",
    }.get(at_name, "?")


# --------------------------------------------------------------------------
# Node renderers. Each returns a Block with pivot on its vertical stem.
# --------------------------------------------------------------------------

def _leaf(char: str, side: str = "right") -> Block:
    """A single character attached to the central stem.

    ``side`` alternates between ``"left"`` and ``"right"`` as we stack
    leaves along a stem, which makes straight sequences look plant-like
    rather than stacked dominoes.
    """
    g = _literal_glyph(char)
    if side == "right":
        lines = [f"|-{g}"]
        pivot = 0
    else:
        lines = [f"{g}-|"]
        pivot = 2
    return Block(lines, pivot)


def _flower(members: list[str], negated: bool = False) -> Block:
    """A single flower head standing in for a whole character class.

    We keep the exact members in braces so the rendering is still legible as
    a regex. A leading ``^`` marks a negated class as a thorn.
    """
    label = "".join(members)
    prefix = "^" if negated else ""
    body = f"{{{prefix}{label}}}"
    top = f"(*){body}"
    return Block([top], pivot=1)


def _any_glyph() -> Block:
    """The ``.`` metachar: a puff of pollen."""
    return Block(["(*)"], pivot=1)


def _category_flower(cat_name: str) -> Block:
    g = _category_glyph(cat_name)
    name = cat_name.replace("CATEGORY_", "").lower()
    return Block([f"({g}){name}"], pivot=1)


def _groupref_glyph(group_number: int) -> Block:
    return Block([f"<-#{group_number}"], pivot=0)


def _assert_glyph(positive: bool, child: Block) -> Block:
    marker = "(?=)" if positive else "(?!)"
    head = Block([marker], pivot=1)
    # Stack the marker above the asserted sub-plant using a dashed stem to
    # hint that an assertion does not consume input.
    return stack(head, child, connector=":")


def _anchor_top(at_name: str) -> Block:
    """Sky/ground marker for ``^`` / ``\\A`` / ``\\b``.

    ``^`` anchors the start of the regex, which we render at the base of the
    plant, so it looks like a banner hanging just above the ground.
    """
    label = _at_label(at_name)
    return Block([f"--[{label}]--"], pivot=3)


def _anchor_bottom(at_name: str) -> Block:
    """Crown marker for ``$`` / ``\\Z``.

    ``$`` anchors the end of the regex, which sits at the crown of the plant.
    """
    label = _at_label(at_name)
    return Block([f"--[{label}]--"], pivot=3)


def _ground(width: int = 7) -> Block:
    width = max(width, 5)
    return Block([GROUND_GLYPH * width], pivot=width // 2)


# --------------------------------------------------------------------------
# Repetition: duplicate the child block a small number of times, with a
# label for the quantifier bounds.
# --------------------------------------------------------------------------

def _repeat_label(lo: int, hi: int) -> str:
    maxrep = _sre_const.MAXREPEAT
    if lo == 0 and hi == maxrep:
        return "*"
    if lo == 1 and hi == maxrep:
        return "+"
    if lo == 0 and hi == 1:
        return "?"
    if lo == hi:
        return f"{{{lo}}}"
    if hi == maxrep:
        return f"{{{lo},}}"
    return f"{{{lo},{hi}}}"


def _render_repeat(lo: int, hi: int, child_tree: Any) -> Block:
    child_block = _render_sequence(child_tree)
    label = _repeat_label(lo, hi)

    # Decide how many copies we will actually draw.
    if hi == _sre_const.MAXREPEAT:
        copies = REPEAT_RENDER_CAP
    else:
        copies = min(max(hi, 1), REPEAT_RENDER_CAP)
    if lo == 0 and copies == 0:
        copies = 1  # always render at least one so the shape is visible

    stacked = stack_many([child_block for _ in range(copies)])

    # Attach a small label near the stem showing the quantifier.
    cap = Block([f"(x{label})"], pivot=1)
    return stack(cap, stacked)


# --------------------------------------------------------------------------
# Sequences and dispatch
# --------------------------------------------------------------------------

def _render_sequence(tree: Any) -> Block:
    """Render a flat sequence of tokens as a single vertical stem.

    The regex reads left-to-right; the rendered plant grows bottom-to-top,
    so we place the first token at the base and stack subsequent ones above
    it. Leaves alternate sides so a run of literals traces a zig-zag up
    the stem.
    """
    blocks: list[Block] = []
    literal_index = 0
    for node in tree:
        opcode, args = node
        blk = _render_node(opcode, args, literal_side_index=literal_index)
        blocks.append(blk)
        if opcode in (LITERAL, NOT_LITERAL):
            literal_index += 1

    if not blocks:
        return Block([" "], pivot=0)

    # Reverse so earliest token is at the bottom of the stack.
    return stack_many(list(reversed(blocks)))


def _render_node(opcode: Any, args: Any, literal_side_index: int = 0) -> Block:
    """Render a single opcode node."""
    if opcode is LITERAL:
        side = "right" if literal_side_index % 2 == 0 else "left"
        return _leaf(chr(args), side=side)

    if opcode is NOT_LITERAL:
        side = "right" if literal_side_index % 2 == 0 else "left"
        g = _literal_glyph(chr(args))
        # Negated literal draws a thorn next to the char.
        return Block([f"|-{g}'"] if side == "right" else [f"'{g}-|"],
                     pivot=0 if side == "right" else 3)

    if opcode is ANY:
        return _any_glyph()

    if opcode is IN:
        return _render_in(args)

    if opcode is BRANCH:
        _, alternatives = args
        sub_plants = [_render_sequence(alt) for alt in alternatives]
        return branch(sub_plants, gap=3)

    if opcode is MAX_REPEAT or opcode is MIN_REPEAT:
        lo, hi, sub_tree = args
        return _render_repeat(lo, hi, sub_tree)

    if opcode is SUBPATTERN:
        # args is (group_number, add_flags, del_flags, sub_tree) in modern
        # Pythons; older variants used (group_number, sub_tree).
        sub_tree = args[-1]
        group_number = args[0]
        child = _render_sequence(sub_tree)
        label = "()" if group_number is None else f"(#{group_number})"
        cap = Block([label], pivot=len(label) // 2)
        return stack(cap, child)

    if opcode is AT:
        # Anchors render as sky or root markers depending on direction.
        name = _sre_const.ATCODES[args] if isinstance(args, int) else args
        name_str = str(name)
        if "END" in name_str:
            return _anchor_bottom(name_str)
        return _anchor_top(name_str)

    if opcode is CATEGORY:
        name = str(args)
        return _category_flower(name)

    if opcode is GROUPREF:
        return _groupref_glyph(args)

    if opcode is ASSERT:
        direction, sub_tree = args
        child = _render_sequence(sub_tree)
        return _assert_glyph(True, child)

    if opcode is ASSERT_NOT:
        direction, sub_tree = args
        child = _render_sequence(sub_tree)
        return _assert_glyph(False, child)

    # Fallback: unknown opcode becomes an opaque marker. We prefer this over
    # raising so the tool stays useful on exotic patterns.
    return Block([f"({opcode!s})"], pivot=1)


def _render_in(members: list[Any]) -> Block:
    """Render a character class body.

    As a special case, a solitary category (e.g. bare ``\\d`` which the parser
    still wraps in an ``IN`` node) renders with the category's friendly name,
    which is more informative than a single-glyph flower.
    """
    # Special-case: IN containing exactly one CATEGORY, possibly with NEGATE.
    non_negate = [m for m in members if m[0] is not NEGATE]
    negated = any(m[0] is NEGATE for m in members)
    if len(non_negate) == 1 and non_negate[0][0] is CATEGORY:
        cat_block = _category_flower(str(non_negate[0][1]))
        if not negated:
            return cat_block
        # Prefix with a thorn marker so negated categories are visually distinct.
        thorn = Block(["(^)"], pivot=1)
        return stack(thorn, cat_block)

    glyphs: list[str] = []
    for node in members:
        op, arg = node
        if op is NEGATE:
            continue
        elif op is LITERAL:
            glyphs.append(_literal_glyph(chr(arg)))
        elif op is NOT_LITERAL:
            glyphs.append(_literal_glyph(chr(arg)) + "'")
        elif op is RANGE:
            lo, hi = arg
            glyphs.append(f"{_literal_glyph(chr(lo))}-{_literal_glyph(chr(hi))}")
        elif op is CATEGORY:
            glyphs.append(_category_glyph(str(arg)))
        else:
            glyphs.append("?")
    return _flower(glyphs, negated=negated)


# --------------------------------------------------------------------------
# Public entrypoint
# --------------------------------------------------------------------------

def plant(pattern: str) -> str:
    """Render a regex ``pattern`` as a multi-line ASCII plant.

    Raises ``re.error`` (a ``ValueError`` subclass in CPython) if the pattern
    fails to parse, mirroring stdlib regex behaviour.
    """
    tree = _sre_parse.parse(pattern)
    body = _render_sequence(tree)
    ground = _ground(width=max(body.width, 5))
    full = stack(body, ground, connector="|")
    return full.render()
