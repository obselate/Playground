"""Command-line entrypoint for ``regex-garden``.

Subcommands:

``plant PATTERN``
    Print a single plant for ``PATTERN``. Use ``--label NAME`` to prefix it
    with a caption, or ``--labels`` to print the pattern above the plant.

``garden FILE``
    Parse ``FILE`` as a garden (see :mod:`regex_garden.garden`) and print
    every plant in order, with headings.

``examples``
    Print a hand-curated garden of example patterns so users get a taste of
    the output without writing any input.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

from regex_garden import __version__
from regex_garden.garden import Garden, GardenEntry, parse_garden
from regex_garden.plant import plant as render_plant


EXAMPLES = Garden(
    entries=[
        GardenEntry("hello", "hello"),
        GardenEntry("quantified", "a+b"),
        GardenEntry("alternation", "cat|dog|bird"),
        GardenEntry("char-class", "[A-Za-z_]"),
        GardenEntry("digits", r"\d{3}-\d{4}"),
        GardenEntry("group-repeat", "(ab)+"),
        GardenEntry("anchored", r"^hello$"),
    ]
)


def _print_plant(entry: GardenEntry, *, show_label: bool, out) -> None:
    if show_label:
        title = f"{entry.name}:  {entry.pattern}"
        print(title, file=out)
        print("-" * len(title), file=out)
    print(render_plant(entry.pattern), file=out)
    print(file=out)


def _cmd_plant(args: argparse.Namespace) -> int:
    name = args.label if args.label is not None else "plant"
    entry = GardenEntry(name, args.pattern)
    try:
        _print_plant(entry, show_label=args.labels or args.label is not None, out=sys.stdout)
    except re.error as e:
        print(f"regex-garden: invalid regex: {e}", file=sys.stderr)
        return 2
    return 0


def _cmd_garden(args: argparse.Namespace) -> int:
    path = Path(args.file)
    if not path.exists():
        print(f"regex-garden: no such file: {path}", file=sys.stderr)
        return 2
    text = path.read_text(encoding="utf-8")
    try:
        garden: Garden = parse_garden(text)
    except ValueError as e:
        print(f"regex-garden: {e}", file=sys.stderr)
        return 2

    rc = 0
    for entry in garden.entries:
        try:
            _print_plant(entry, show_label=True, out=sys.stdout)
        except re.error as e:
            print(
                f"regex-garden: entry {entry.name!r}: invalid regex: {e}",
                file=sys.stderr,
            )
            rc = 1
    return rc


def _cmd_examples(args: argparse.Namespace) -> int:
    for entry in EXAMPLES.entries:
        _print_plant(entry, show_label=True, out=sys.stdout)
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="regex-garden",
        description="Grow ASCII plants from regular expressions.",
    )
    parser.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    sub = parser.add_subparsers(dest="command", required=True)

    p_plant = sub.add_parser("plant", help="Render a single regex as a plant.")
    p_plant.add_argument("pattern", help="The regex pattern to render.")
    p_plant.add_argument(
        "--label",
        help="Optional caption (implies --labels).",
        default=None,
    )
    p_plant.add_argument(
        "--labels",
        action="store_true",
        help="Print the pattern as a caption above the plant.",
    )
    p_plant.set_defaults(func=_cmd_plant)

    p_garden = sub.add_parser("garden", help="Render every entry in a garden file.")
    p_garden.add_argument("file", help="Path to a .garden file.")
    p_garden.set_defaults(func=_cmd_garden)

    p_examples = sub.add_parser(
        "examples", help="Print a built-in sampler of example plants."
    )
    p_examples.set_defaults(func=_cmd_examples)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
