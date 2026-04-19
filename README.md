# Playground

A small monorepo of original, open-source tools. Each project is
self-contained — it has its own README, its own build, its own tests,
and its own CI job — and can be built, installed, and released
independently.

## Projects

| Project | Language | Summary |
| --- | --- | --- |
| [`regex-garden/`](regex-garden/README.md) | Python 3.10+ | Grows each regex into a unique ASCII plant whose shape mirrors its AST. CLI + `.garden` file format. |
| [`censor/`](censor/README.md) | Rust (edition 2021) | Stream-through redactor for secrets in logs, diffs, and shell output. Zero OS-specific code; ships as a single binary on Linux, macOS, and Windows. |

## Layout

```
.
├── LICENSE               # MIT, covers everything in this repo
├── README.md             # you are here
├── .github/workflows/    # one workflow per project (scoped via path filters)
├── regex-garden/
│   ├── README.md
│   ├── pyproject.toml
│   ├── conftest.py
│   ├── src/regex_garden/
│   ├── tests/
│   └── examples/
└── censor/
    ├── README.md
    ├── Cargo.toml
    ├── Cargo.lock
    ├── src/
    └── tests/
```

Each subproject is the root of its own build:

- **regex-garden**: `cd regex-garden && python -m pytest` (tests), `pip install -e .` (install), `python -m regex_garden examples` (demo).
- **censor**: `cd censor && cargo test` (tests), `cargo install --path .` (install), `cargo run -- --list-patterns` (sanity check).

## CI

GitHub Actions runs one workflow per project, each scoped by a path
filter so a change to Python code doesn't rebuild Rust and vice versa.
See `.github/workflows/`.

## Adding a new project

1. Create a new top-level directory with its own README, build file, and
   tests.
2. Add a matching workflow under `.github/workflows/<project>.yml` whose
   `paths:` filter matches that directory.
3. Link it from the Projects table above.

## License

MIT — see [LICENSE](LICENSE).
