//! Pattern catalogue for censor.
//!
//! Each [`Pattern`] pairs a compiled regex with a short label. The label is
//! what ends up inside the replacement marker (e.g. `<REDACTED:aws-key>`),
//! so keep labels short, lowercase, and kebab-cased.
//!
//! The catalogue is split into two shapes:
//!
//! * Single-line [`PatternKind::Line`] patterns — the common case. The
//!   regex is run against each line independently.
//! * Block patterns [`PatternKind::Block`] — used for multi-line secrets
//!   such as PEM-armoured private keys. These have a `start` regex and an
//!   `end` regex; every line between them (inclusive) is collapsed into a
//!   single placeholder.
//!
//! Users can layer additional patterns via a config file (see
//! [`crate::config`]).

use regex::Regex;
use serde::Deserialize;

/// A redaction pattern.
#[derive(Debug, Clone)]
pub struct Pattern {
    pub label: String,
    pub kind: PatternKind,
}

#[derive(Debug, Clone)]
pub enum PatternKind {
    /// Match and replace substrings within a single line.
    Line { regex: Regex },
    /// Match a multi-line region delimited by two regexes.
    Block { start: Regex, end: Regex },
}

/// Raw pattern definition loaded from config, before regex compilation.
#[derive(Debug, Deserialize)]
pub struct PatternSpec {
    pub label: String,
    #[serde(default)]
    pub regex: Option<String>,
    #[serde(default)]
    pub start: Option<String>,
    #[serde(default)]
    pub end: Option<String>,
}

impl PatternSpec {
    /// Compile the spec into a runtime [`Pattern`].
    ///
    /// Either `regex` (line pattern) or both `start` and `end` (block
    /// pattern) must be present. Mixing the two is an error so a typo
    /// doesn't silently fall back to line mode.
    pub fn compile(self) -> Result<Pattern, anyhow::Error> {
        match (self.regex, self.start, self.end) {
            (Some(rx), None, None) => Ok(Pattern {
                label: self.label,
                kind: PatternKind::Line {
                    regex: Regex::new(&rx)?,
                },
            }),
            (None, Some(start), Some(end)) => Ok(Pattern {
                label: self.label,
                kind: PatternKind::Block {
                    start: Regex::new(&start)?,
                    end: Regex::new(&end)?,
                },
            }),
            (Some(_), Some(_), _) | (Some(_), _, Some(_)) => Err(anyhow::anyhow!(
                "pattern {:?} mixes `regex` with `start`/`end`; pick one shape",
                self.label
            )),
            (None, None, _) | (None, _, None) => Err(anyhow::anyhow!(
                "pattern {:?} needs `regex` or both `start` and `end`",
                self.label
            )),
        }
    }
}

/// The built-in catalogue. Deliberately conservative: we prefer structured,
/// high-precision patterns over greedy ones so a plain paragraph of prose
/// doesn't get shredded into redaction markers.
pub struct BuiltinPatterns;

impl BuiltinPatterns {
    /// Return every built-in pattern, already compiled.
    ///
    /// This allocates a fresh `Vec` so callers can mutate the set freely
    /// (e.g. to disable categories via `--disable`).
    pub fn all() -> Vec<Pattern> {
        let specs: &[(&str, &str, &str)] = &[
            // --- Cloud keys ---
            ("aws-access-key", "line", r"AKIA[0-9A-Z]{16}"),
            // AWS secret access keys are 40 base64-ish chars; only match
            // when clearly labelled so we don't shred random git hashes.
            (
                "aws-secret",
                "line",
                r#"(?i)aws[_\-]?(?:secret|sk)[^=:]{0,20}[=:]\s*['"]?[A-Za-z0-9/+]{40}['"]?"#,
            ),
            // GCP / Firebase API keys.
            ("google-api-key", "line", r"AIza[0-9A-Za-z_\-]{35}"),
            // --- Source-forge / CI tokens ---
            ("github-pat", "line", r"gh[pousr]_[A-Za-z0-9]{36}"),
            (
                "github-fine-grained",
                "line",
                r"github_pat_[A-Za-z0-9_]{22,}",
            ),
            ("slack-token", "line", r"xox[baprs]-[0-9A-Za-z-]{10,}"),
            // --- Payment processors ---
            (
                "stripe-secret",
                "line",
                r"sk_(?:live|test)_[0-9a-zA-Z]{24,}",
            ),
            // --- Generic shapes ---
            // JWTs: header.payload.signature with base64url segments.
            (
                "jwt",
                "line",
                r"eyJ[A-Za-z0-9_\-]{5,}\.eyJ[A-Za-z0-9_\-]{5,}\.[A-Za-z0-9_\-]{5,}",
            ),
            // Authorization headers.
            (
                "bearer-token",
                "line",
                r"(?i)(?:authorization:\s*)?bearer\s+[A-Za-z0-9\-_\.~+/=]{12,}",
            ),
            (
                "basic-auth",
                "line",
                r"(?i)(?:authorization:\s*)?basic\s+[A-Za-z0-9+/=]{10,}",
            ),
            // URL with inline credentials: proto://user:pass@host
            (
                "url-credentials",
                "line",
                r"[a-zA-Z][a-zA-Z0-9+\-.]*://[^\s:/@]+:[^\s/@]+@[^\s]+",
            ),
            // Credit-card-shaped digit runs (13-19 digits with optional
            // spaces or dashes). Prone to false positives on long numeric
            // identifiers; refinements are up to the caller.
            ("credit-card", "line", r"\b(?:\d[ -]?){13,19}\b"),
            // Emails. Not secrets per se, but typically PII worth scrubbing
            // before pasting publicly.
            (
                "email",
                "line",
                r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b",
            ),
            // RFC1918 private IPv4 ranges.
            (
                "private-ipv4",
                "line",
                r"\b(?:10(?:\.\d{1,3}){3}|192\.168(?:\.\d{1,3}){2}|172\.(?:1[6-9]|2\d|3[01])(?:\.\d{1,3}){2})\b",
            ),
            // UUIDs.
            (
                "uuid",
                "line",
                r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b",
            ),
            // Generic `key = "value"` shapes that look like secrets.
            //
            // The character class explicitly excludes the outer quote so the
            // match stops at the closing quote rather than greedily eating
            // whatever follows on the line.
            (
                "generic-assignment",
                "line",
                r#"(?i)\b(?:password|passwd|pwd|secret|token|api[_\-]?key|access[_\-]?key|client[_\-]?secret|private[_\-]?key)\b\s*[=:]\s*['"][^'"\r\n]{4,}['"]"#,
            ),
        ];

        let mut out: Vec<Pattern> = specs
            .iter()
            .map(|(label, kind, rx)| {
                assert_eq!(
                    *kind, "line",
                    "only line kind supported in the static table"
                );
                Pattern {
                    label: (*label).into(),
                    kind: PatternKind::Line {
                        regex: Regex::new(rx).expect("builtin regex failed to compile"),
                    },
                }
            })
            .collect();

        // Multi-line block patterns.
        out.push(Pattern {
            label: "private-key".into(),
            kind: PatternKind::Block {
                start: Regex::new(r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----").unwrap(),
                end: Regex::new(r"-----END [A-Z0-9 ]*PRIVATE KEY-----").unwrap(),
            },
        });

        out
    }

    /// Names of all built-in patterns. Handy for `--list-patterns` and
    /// for validating `--disable` inputs.
    pub fn names() -> Vec<String> {
        Self::all().into_iter().map(|p| p.label).collect()
    }
}
