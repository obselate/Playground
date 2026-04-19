"""A plain-text "garden" file format: a list of named regexes.

The format is deliberately trivial so a garden is easy to write by hand and
diff-friendly. Empty lines and ``#`` comments are ignored. Every regex entry
begins with ``- name:`` followed by a ``pattern:`` line, e.g.::

    # My garden
    - name: email
      pattern: [A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}

    - name: zipcode
      pattern: \\d{5}(-\\d{4})?

The patterns themselves are passed through verbatim; we do NOT expand any
YAML-style escapes. This keeps the parser a single pass and means regex
backslashes survive untouched.
"""

from __future__ import annotations

from dataclasses import dataclass


@dataclass
class GardenEntry:
    name: str
    pattern: str


@dataclass
class Garden:
    entries: list[GardenEntry]

    def names(self) -> list[str]:
        return [e.name for e in self.entries]


class GardenParseError(ValueError):
    """Raised when a garden file cannot be parsed."""


def parse_garden(text: str) -> Garden:
    """Parse garden-file ``text`` into a :class:`Garden`.

    Parser rules:

    - Lines starting with ``#`` (after optional whitespace) are comments.
    - Blank lines separate entries but are otherwise ignored.
    - A new entry starts with ``- name: <name>``.
    - The matching ``pattern:`` line must follow before the next ``- name:``.
    - Any other non-blank, non-comment line is an error.
    """
    entries: list[GardenEntry] = []
    current_name: str | None = None
    current_pattern: str | None = None

    def flush(lineno: int) -> None:
        nonlocal current_name, current_pattern
        if current_name is None and current_pattern is None:
            return
        if current_name is None or current_pattern is None:
            raise GardenParseError(
                f"line {lineno}: entry is missing "
                f"{'name' if current_name is None else 'pattern'}"
            )
        entries.append(GardenEntry(current_name, current_pattern))
        current_name = None
        current_pattern = None

    for lineno, raw in enumerate(text.splitlines(), start=1):
        stripped = raw.strip()
        if not stripped or stripped.startswith("#"):
            continue

        if stripped.startswith("- name:"):
            flush(lineno)
            current_name = stripped[len("- name:"):].strip()
            if not current_name:
                raise GardenParseError(f"line {lineno}: empty name")
            continue

        if stripped.startswith("pattern:"):
            if current_name is None:
                raise GardenParseError(
                    f"line {lineno}: 'pattern:' before any '- name:'"
                )
            if current_pattern is not None:
                raise GardenParseError(
                    f"line {lineno}: duplicate 'pattern:' for entry '{current_name}'"
                )
            # Preserve exact characters after the colon (minus the single
            # separating space, if present). We read from the original line,
            # not the stripped one, so leading indent is ignored but a
            # trailing-space pattern survives.
            after = raw.split("pattern:", 1)[1]
            if after.startswith(" "):
                after = after[1:]
            current_pattern = after
            continue

        raise GardenParseError(
            f"line {lineno}: unrecognised directive: {stripped!r}"
        )

    flush(lineno=len(text.splitlines()) + 1)
    return Garden(entries)


def format_garden(garden: Garden) -> str:
    """Serialise a :class:`Garden` back to the canonical text form."""
    parts: list[str] = []
    for e in garden.entries:
        parts.append(f"- name: {e.name}")
        parts.append(f"  pattern: {e.pattern}")
        parts.append("")
    return "\n".join(parts).rstrip() + "\n"
