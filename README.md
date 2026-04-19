# Playground

A little monorepo of small, original, open-source tools. Each subproject
ships independently and is documented in its own README.

| Project | Language | What it does |
| --- | --- | --- |
| [`regex-garden`](#regex-garden) (at the repo root under `src/`) | Python 3.10+ | Renders a regex as an ASCII plant whose shape mirrors its AST. |
| [`censor/`](censor/README.md) | Rust | Stream-through redactor for secrets in logs, diffs, and shell output. Runs on Linux, macOS, and Windows. |

---

# regex-garden

Grow ASCII plants from regular expressions.

`regex-garden` is a small CLI that reads a regex, parses it with Python's
own `re._parser`, and renders its abstract syntax tree as a plant. Literals
become leaves on a stem; character classes become flowers; alternation
forks the stem; quantifiers stack the same shape multiple times. Equivalent
regexes grow the same plant, which makes it easy to *see* structural
patterns you would normally have to read character-by-character.

```
$ regex-garden plant 'cat|dog|bird'

                d-|
                  |
  |-t     |-g     |-r
  |       |       |
a-|     o-|     i-|
  |       |       |
  |-c     |-d     |-b
   \      |      /
    \     |     /
     \    |    /
      \   |   /
       \  |  /
        \ | /
         \|/
          |
          |
~~~~~~~~~~~~~~~~~~~~~
```

It is Python 3.10+ only, zero third-party runtime dependencies, MIT
licensed, and fits in a handful of files.

## Install

From a checkout:

```
pip install -e .
```

Or just run it in-place without installing:

```
python -m regex_garden examples
```

## Usage

Three subcommands:

```
regex-garden plant PATTERN [--label NAME | --labels]
regex-garden garden FILE.garden
regex-garden examples
```

- `plant` renders a single pattern.
- `garden` reads a `.garden` file (see `examples/sampler.garden`) and
  renders every named pattern in order.
- `examples` prints a built-in sampler.

### A `.garden` file

The format is deliberately minimal so gardens are easy to write and diff:

```
# My garden
- name: email-ish
  pattern: [a-z]+@[a-z]+

- name: phone
  pattern: \d{3}-\d{4}
```

Comments start with `#`, blank lines are ignored, and patterns are taken
verbatim — backslashes stay backslashes.

## Glyph vocabulary

| Regex construct | Rendering |
| --- | --- |
| literal `a` | leaf `\|-a` (side alternates up the stem) |
| any char `.` | `(*)` pollen puff |
| `\d`, `\w`, `\s`, … | `(o)digit`, `(#)word`, `(~)space`, … — named flower |
| `[abc]`, `[a-z]` | `(*){abc}` / `(*){a-z}` — flower labelled with members |
| `[^abc]` | `(*){^abc}` — class with a thorn |
| alternation `a\|b\|c` | V-branching stems reconverging on a single trunk |
| group `(ab)` | sub-shoot captioned `(#n)` for group number `n` |
| quantifier `a+` | stem labelled `(x+)`, child shape stacked (capped at 3 copies) |
| anchors `^`, `$` | labelled banners at ground and crown |
| lookahead `(?=…)` | marker `(?=)` above the sub-plant, joined by a dotted stem |

## A known quirk: `a|b` vs `[ab]`

Python's own regex parser folds single-character alternations like
`a|b|c` into a character class `[abc]` before we ever see them. So
`regex-garden plant 'a|b'` renders the same flower as `regex-garden plant
'[ab]'`. This is semantically honest — the two patterns describe exactly
the same language — but it does mean small alternations do not fork the
stem. Use multi-character alternatives (`ab|cd`) if you want the branch.

## Development

```
python -m pytest
```

The test suite lives in `tests/` and covers the canvas primitives, the
regex-to-plant renderer, the garden parser, and the CLI. Tests assert on
structural properties (is there a `~` ground line? does the quantifier
label appear?) rather than full-string equality, so harmless cosmetic
tweaks don't break them.

### Repository layout

```
src/regex_garden/
    canvas.py    # text blocks with a stem-pivot column and composition ops
    plant.py     # regex AST -> Block renderer
    garden.py    # parser and formatter for .garden files
    cli.py       # argparse-based CLI
    __main__.py  # python -m regex_garden entry point
tests/           # pytest suite
examples/
    sampler.garden
```

## License

MIT. See [LICENSE](LICENSE).
