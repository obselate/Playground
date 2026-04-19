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
| [`mile-marker/`](mile-marker/README.md) | Rust (edition 2021) | Native GUI (egui) for "record-and-narrate-as-you-go" screen capture: auto-pause-for-caption mechanic, exports a slideshow GIF or a Markdown walkthrough. |

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
├── censor/
│   ├── README.md
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── src/
│   └── tests/
└── mile-marker/
    ├── README.md
    ├── Cargo.toml
    ├── Cargo.lock
    ├── assets/         # bundled font for caption overlays
    ├── src/
    └── tests/
```

Each subproject is the root of its own build:

- **regex-garden**: `cd regex-garden && python -m pytest` (tests), `pip install -e .` (install), `python -m regex_garden examples` (demo).
- **censor**: `cd censor && cargo test` (tests), `cargo install --path .` (install), `cargo run -- --list-patterns` (sanity check).
- **mile-marker**: `cd mile-marker && cargo test` (tests), `cargo install --path .` (install), `cargo run --release` (launch the GUI).

## CI

GitHub Actions runs one workflow per project, each scoped by a path
filter so a change to Python code doesn't rebuild Rust and vice versa.
See `.github/workflows/`.

## Releases

Each project releases independently on its own tag pattern. Pushing
the tag triggers a matching `release-*.yml` workflow that builds
distribution artifacts and attaches them to a GitHub Release.

| Project | Tag | Artifacts |
| --- | --- | --- |
| regex-garden | `regex-garden-v<semver>` | source tarball + pure-Python wheel |
| censor | `censor-v<semver>` | release binaries for Linux x86_64, macOS x86_64, macOS aarch64, Windows x86_64 |
| mile-marker | `mile-marker-v<semver>` | release binaries for the same four targets, plus bundled font notice |

Example:

```
git tag censor-v0.2.0
git push origin censor-v0.2.0
```

Release notes are auto-generated from commits since the previous tag.
Rust binaries are stripped, LTO-thin, codegen-units=1 (all set in each
project's `Cargo.toml` `[profile.release]`).

## Adding a new project

1. Create a new top-level directory with its own README, build file, and
   tests.
2. Add a matching workflow under `.github/workflows/<project>.yml` whose
   `paths:` filter matches that directory.
3. Link it from the Projects table above.

## License

MIT — see [LICENSE](LICENSE).
