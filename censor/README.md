# censor

Stream-through redactor for secrets in logs, diffs, and shell output.

`censor` reads anything on stdin (or file arguments), finds things that
look like secrets — API keys, tokens, private keys, emails, private IPs,
JWTs, credit cards, PEM-armoured keys, and high-entropy `key=value`
values — and writes a version with those substrings replaced by a
compact `<REDACTED:label>` marker. Line shape is preserved so stack
traces and diffs stay readable.

Built as a single zero-dependency binary with Rust and [`regex`].

```
$ cat server.log | censor
```

```
$ censor server.log other.log --output safe.log --report 2>redactions.json
```

## Install

```
cargo install --path censor
```

Or grab a binary from the repo's releases page.

Cross-platform: Linux, macOS, and Windows. The code is pure Rust with no
OS-specific syscalls, so PowerShell, cmd, and bash all work:

```
PS> Get-Content error.log | censor
```

```
> type error.log | censor
```

```
$ cat error.log | censor
```

## What it catches

Run `censor --list-patterns` for the live list. Built-ins:

| Label | Catches |
| --- | --- |
| `aws-access-key` | `AKIA…` 20-char access key IDs |
| `aws-secret` | `aws_secret…=…` 40-char values |
| `google-api-key` | `AIza…` 39-char keys |
| `github-pat` | `ghp_`/`gho_`/`ghs_`/`ghu_`/`ghr_` 40-char tokens |
| `github-fine-grained` | `github_pat_…` tokens |
| `slack-token` | `xoxb-`/`xoxp-`/`xoxa-`/`xoxr-`/`xoxs-` |
| `stripe-secret` | `sk_live_…`/`sk_test_…` |
| `jwt` | standard 3-segment JWT |
| `bearer-token` | `Authorization: Bearer …` |
| `basic-auth` | `Authorization: Basic …` |
| `url-credentials` | `proto://user:pass@host/…` |
| `credit-card` | 13–19 digit runs (naive; no Luhn) |
| `email` | `user@host.tld` |
| `private-ipv4` | RFC1918 private ranges |
| `uuid` | standard 8-4-4-4-12 UUIDs |
| `generic-assignment` | `password="…"` / `token="…"` / etc. |
| `private-key` | PEM-armoured `-----BEGIN … PRIVATE KEY-----` blocks |

On top of the structured patterns there's an **entropy fallback**: any
value after a credential-looking key (`token`, `secret`, `api_key`, etc.)
that's at least 24 characters and exceeds ≈4 bits of Shannon entropy per
character gets redacted too. This picks up custom tokens the structured
patterns don't know about. Disable with `--no-entropy` or tune with
`--entropy-min-length` and `--entropy-min-bits`.

## Examples

### Scrub an error before pasting to GitHub

```
$ ./flaky-thing 2>&1 | censor
```

### Fail a commit that would leak something

```
# in a pre-commit hook
git diff --cached | censor --strict --no-entropy >/dev/null || {
  echo "refusing to commit: possible secret in diff" >&2
  exit 1
}
```

### Preview, don't hide: keep last 4 chars for debugging

```
$ echo 'key=AKIAIOSFODNN7EXAMPLE' | censor --keep-last 4
key=<REDACTED:aws-access-key:…MPLE>
```

### Emit a JSON summary to stderr

```
$ censor server.log --report >safe.log 2>summary.json
$ cat summary.json
{"total":7,"by_label":{"aws-access-key":2,"email":3,"github-pat":2}}
```

### Config file

`censor.toml`:

```toml
# Strings that should never be redacted.
allow = ["127.0.0.1", "user@example.com"]

# Drop built-ins you don't want.
disable = ["uuid", "credit-card"]

# Add custom patterns.
[[patterns]]
label = "tenant-id"
regex = 'tenant-[A-Z0-9]{12}'

[[patterns]]
label = "corp-key"
start = '-----BEGIN CORP KEY-----'
end = '-----END CORP KEY-----'

# Entropy tuning.
[entropy]
min_length = 32
min_bits = 4.5
```

Use it with `censor --config censor.toml`.

## CLI

```
censor [OPTIONS] [FILES...]

Read from FILES (or stdin if none / `-`), redact, write to stdout or
--output.

Options:
  -o, --output <PATH>             Write output to PATH instead of stdout
  -c, --config <PATH>             TOML config file
      --disable <LABEL>           Turn off a built-in pattern (repeatable)
      --allow <STRING>            Allowlist a literal string (repeatable)
      --no-entropy                Disable entropy-based fallback
      --entropy-min-length <N>    Minimum token length for entropy (default 24)
      --entropy-min-bits <BITS>   Minimum entropy bits/char (default 4.0)
      --keep-last <N>             Keep last N chars of each secret as a hint
      --report                    Print JSON redaction summary to stderr
      --strict                    Exit 1 if anything was redacted
      --list-patterns             List built-in pattern labels and exit
  -h, --help                      Show help
  -V, --version                   Show version
```

## How it works

- Line-buffered streaming: memory is bounded by one line at a time
  regardless of input size.
- Each line is run through every built-in pattern in catalogue order;
  earlier matches win.
- The entropy heuristic runs last so structured labels take precedence
  over the generic `entropy` marker.
- PEM-armoured private keys use a tiny state machine: the entire
  `-----BEGIN … PRIVATE KEY-----` / `-----END … PRIVATE KEY-----` range
  collapses into one placeholder rather than scrambling each line.

## Caveats

- The `credit-card` pattern is naive (no Luhn check). False positives
  on long numeric identifiers are possible; disable it if it's noisy
  for your workload.
- `email` catches anything that looks like an email, including public
  addresses. Use `--allow` or `disable = ["email"]` if that's too
  aggressive for your logs.
- This is defence-in-depth, not a guarantee. If your logs contain
  novel secret formats, add a pattern to `censor.toml`.

## Development

```
cargo build
cargo test
cargo run -- --list-patterns
```

## License

MIT.
