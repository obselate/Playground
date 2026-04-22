#  Tools that would make an AI coding agent more efficient

A brainstorm. Each idea below is scoped so it could live in this monorepo
as a sibling of `censor/`, `regex-garden/`, and `mile-marker/`: one
directory, one binary (or one `python -m` entry point), its own README,
its own CI job, MIT.

The guiding question: **what do I, as an AI, currently do by burning
context tokens or by making many small tool calls, that a ~200 LOC
binary could do faster and cheaper in one shot?**

Ideas are sorted roughly by expected value (token savings × how often it
would actually be used).

---

## Tier 1 — Highest leverage

### 1. `slice` — structural, line-numbered file slicer

**Language:** Rust.
**Why:** Every time I "read a file to find one function" I pull 200+
lines into context. A tool that returns *just the enclosing block* for a
symbol would cut that by 5–20×.

**Interface:**

```
slice <path> --symbol foo                # the whole definition of `foo`
slice <path> --around 142 --ctx 5        # 5 lines around line 142, with line numbers
slice <path> --block 142                 # smallest enclosing {...} / def / class
slice <path> --outline                   # one line per top-level symbol, with line ranges
slice <path> --imports                   # just the import/use block
slice <path> --signatures                # all fn/def signatures, no bodies
```

Output always prefixes `NNN│ ` so the AI can cite line numbers without
re-reading.

**Implementation:** tree-sitter grammars for the top ~12 languages
(rust, python, ts/js/tsx/jsx, go, java, c, c++, ruby, bash, json, toml).
Falls back to brace/indent heuristics for unknown languages. The
`--outline` mode alone is the killer feature: ~40 tokens to understand
a 2000-line file's shape.

**Token math:** typical `Read` of a 600-line file ≈ 4k tokens. A
`slice --outline` on the same file ≈ 200 tokens, and then a targeted
`--symbol` fetches just what's needed.

---

### 2. `ctx` — repo-aware context pack

**Language:** Rust.
**Why:** When I start a task I often do 6–10 exploratory `rg` / `glob` /
`read` calls just to answer "where does this live, who calls it, what
type is it." One command, one response.

**Interface:**

```
ctx symbol MyThing                       # def + all callers + all importers
ctx symbol MyThing --depth 2             # expand one level of callees too
ctx file path/to/foo.rs                  # outline + who-imports-it + tests touching it
ctx diff                                 # summary of currently-staged changes with surrounding signatures
ctx owners path/foo.rs                   # git blame condensed: top 3 authors + last 3 touches
```

Output is a single compact Markdown document with file:line anchors.
Pure read-only, no indexing daemon — it walks on demand and is fast
enough thanks to `ignore` + `tree-sitter`.

**Token math:** replaces ~8 round-trips with 1, and is denser than the
raw tool outputs.

---

### 3. `patchkit` — deterministic, idempotent patch applier

**Language:** Rust.
**Why:** I currently produce `str_replace` edits, which fail the moment
whitespace drifts or a match isn't unique. A smarter applier would cut
my retry rate dramatically and let me describe edits more compactly.

**Interface:**

```
patchkit apply <<'EOF'
@@ file: src/lib.rs
@@ anchor: fn parse(
- let n = s.parse::<u32>().unwrap();
+ let n: u32 = s.parse()?;
EOF
```

Features:

- Anchor-based: find the block by a symbol/regex, then apply the diff
  inside it. Whitespace-normalised matching with a reported confidence
  score.
- Dry-run by default; `--write` to commit.
- Emits a structured JSON reply: `{applied, file, line_range, hunks}`.
- Exits non-zero with a *minimal* diff suggestion when ambiguous, so the
  next tool call has everything it needs.
- Bundles an `undo` subcommand backed by a local `.patchkit/` journal.

**Token math:** avoids the classic "old_string was not unique" retry
loop, which often costs a full re-read.

---

### 4. `gitq` — git, but shaped for LLM consumption

**Language:** Rust (wraps `git` via `gitoxide` or shells out).
**Why:** `git log`, `git diff`, `git blame` dump way more than I need.
A purpose-built front-end can return the answer in a token budget.

**Interface:**

```
gitq why <path> <line>                   # last commit + message that touched that line
gitq hot --since 30d                     # top 10 most-churned files with blast radius
gitq diff --stat-only --budget 2000      # auto-truncate diff to stay under a token budget
gitq last <symbol>                       # last commit that touched the enclosing function
gitq conflicts                           # machine-readable list of unresolved conflict hunks
```

Everything emits a `--json` form too. The `--budget N` flag
(approximated as chars/4) is the key idea: the tool picks what to drop.

---

## Tier 2 — Frequently useful

### 5. `qjson` — streaming JSON/YAML/TOML projector

**Language:** Rust.
**Why:** `jq` is great but verbose, and I often need *structural* info
rather than a single value. `qjson` answers "what shape is this file?"
cheaply and gives `--max-depth` / `--sample` flags that bound output.

```
qjson shape config.yaml                  # tree of keys + leaf types, no values
qjson pick package.json .dependencies    # dot-path project, works across json/yaml/toml
qjson diff a.json b.json                 # structural diff, not textual
qjson redact config.yaml --keys 'secret,token,password'
```

Pairs nicely with `censor` (same repo) for scrubbing before paste.

---

### 6. `tokscope` — estimate & cap token cost of a file or command

**Language:** Rust. Embeds a BPE tokenizer (e.g. `tiktoken-rs`) + a
`cl100k`-compatible vocab.
**Why:** Before I `Read` a big generated file (lockfiles, fixtures,
minified JS) I want to know it'll be 80k tokens, not spend 80k tokens
finding out. Also useful as a wrapper.

```
tokscope file Cargo.lock                  # -> 14,203 tokens
tokscope cmd -- cargo test -- --nocapture # streams stdout, aborts if it exceeds budget
tokscope fit path/to/huge.log --budget 4000   # head/tail/sampled slice that fits
```

`fit` is the killer mode: given a budget, it returns a sensible
summarised view (head + tail + uniform samples + collapsed runs) so the
AI can still reason about the file.

---

### 7. `structgrep` — AST-aware grep

**Language:** Rust on tree-sitter.
**Why:** Plain `rg` over-matches on identifiers. "Find all call sites
of `parse` where the first argument is a literal" is cheap with AST
queries and painful with regex.

```
structgrep --lang rust '(call_expression function: (identifier) @f (#eq? @f "parse"))'
structgrep --preset rust-fn-callers parse
structgrep --preset unused-imports .
```

Ships with a `--preset` library for the 10 most common questions per
language so the AI doesn't have to write tree-sitter queries from
scratch.

---

### 8. `dryrun` — cached deterministic command runner

**Language:** Rust.
**Why:** I re-run `cargo check`, `pytest --collect-only`, `tsc
--noEmit`, etc., dozens of times. Many runs hash-identical if the
inputs haven't changed.

```
dryrun cargo check                       # memoises by (cmd, env, file hashes)
dryrun --invalidate src/ cargo test
dryrun --stdout-budget 4000 pytest -q    # truncate noisy green output
```

Benefits: fast re-runs, auto-truncation, single canonical error
extractor (`--errors-only` filters output to the lines a compiler marks
as errors/warnings).

---

### 9. `testfocus` — minimal test runner invocation planner

**Language:** Python (works with pytest, jest, go test, cargo test).
**Why:** Running the full suite wastes tokens on green output. Given a
list of changed files, emit the narrowest test selection that still
covers the blast radius.

```
testfocus --changed $(git diff --name-only main) --runner pytest
# -> pytest tests/test_parser.py::TestThing -q
```

Reads pytest collection, cargo test `--list`, etc. Emits the exact
command to run; pairs with `dryrun` for the execution.

---

## Tier 3 — Nice to have

### 10. `scaffold` — minimal-example extractor

Given a symbol, emit the smallest compilable/runnable file that
exercises it, using existing usages from tests as a template. Handy for
"can you show me how X is used" without pulling entire test files.

### 11. `lockreader` — purpose-built lockfile summariser

`Cargo.lock`, `package-lock.json`, `uv.lock`, `poetry.lock`. Answers
"what version of X is pinned, who pulls it in, any duplicate majors?"
in under 500 tokens on any size lockfile.

### 12. `errorlens` — compiler/runtime error condenser

Pipe `cargo build` / `tsc` / `pyright` / `eslint` output in; get out a
deduplicated, file-grouped list with the minimum context needed to fix
each one. Strips progress bars, colour codes, and repeated "note:"
chains.

### 13. `shapeof` — infer a JSON schema / TypeScript type / Rust struct from a sample payload

Preset targets: `--as ts`, `--as rust`, `--as pydantic`, `--as
json-schema`. Makes it trivial to bootstrap types from an API response.

### 14. `manpager` — local doc grepper for installed libraries

Walks `~/.cargo/registry/src`, `node_modules`, `site-packages`, etc.,
and serves a `manpager query "axum::Router::route"` that returns the
exact doc comment — no web fetch needed. Saves roundtrips that would
otherwise hit docs.rs / npm.

### 15. `envsniff` — safe summary of the local dev environment

One compact report: OS, shells, installed toolchains + versions, git
config, whether common binaries exist (`rg`, `fd`, `jq`, `uv`, …).
Pre-flight for "can I run this command?" without 8 separate `which`
calls.

---

## Picking the first one to build

If I had to ship one, it would be **`slice`** (#1). Rationale:

1. The token savings are immediate and measurable on every task.
2. It's small enough to fit the repo's pattern (~300–600 LOC Rust,
   zero-to-few deps beyond tree-sitter).
3. It composes well with the other tools on this list: `ctx`, `gitq`,
   `structgrep`, and `errorlens` can all call `slice` internally for
   their "show me the relevant code" fragments.
4. It's language-agnostic *enough* — the outline mode is useful even
   for languages we don't have a grammar for, via indentation fallback.

Candidate sibling directory layout, matching the rest of this repo:

```
slice/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs
│   ├── outline.rs
│   ├── languages/      # one file per tree-sitter grammar wrapper
│   └── fallback.rs     # indent/brace heuristic
└── tests/
    └── fixtures/       # sample files in each supported language
```

Ship target: `slice-v0.1.0` tag → `release-slice.yml` builds the same
4-way matrix (Linux x86_64, macOS x86_64, macOS aarch64, Windows
x86_64) the other binaries already use.
