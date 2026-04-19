//! The streaming redactor.
//!
//! [`Redactor::run`] reads from a [`BufRead`] line-by-line, applies every
//! configured pattern, and writes a redacted copy to a [`Write`]. A small
//! state machine handles multi-line block patterns so secrets like PEM
//! private keys collapse into a single placeholder instead of producing
//! one placeholder per line.
//!
//! Memory is bounded to a single line (plus whatever a block pattern is
//! currently consuming), so `censor` handles arbitrarily large streams.
//!
//! [`BufRead`]: std::io::BufRead
//! [`Write`]: std::io::Write

use std::collections::HashMap;
use std::io::{BufRead, Write};

use crate::entropy::EntropyRedactor;
use crate::patterns::{Pattern, PatternKind};

/// Per-run redactor configuration.
pub struct RedactorOptions {
    pub patterns: Vec<Pattern>,
    /// Substrings that should be left untouched even if they match.
    pub allowlist: Vec<String>,
    /// If `Some`, run the entropy heuristic with these thresholds.
    pub entropy: Option<(usize, f64)>,
    /// Number of trailing chars to preserve in the replacement marker.
    /// 0 disables the hint. Small values (2–4) help debugging without
    /// meaningfully re-identifying a secret.
    pub keep_last: usize,
}

impl Default for RedactorOptions {
    fn default() -> Self {
        Self {
            patterns: crate::patterns::BuiltinPatterns::all(),
            allowlist: Vec::new(),
            entropy: Some((24, 4.0)),
            keep_last: 0,
        }
    }
}

/// Statistics reported after a run.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RedactStats {
    /// Total number of redactions performed, across all labels.
    pub total: usize,
    /// Count per pattern label.
    pub by_label: HashMap<String, usize>,
}

impl RedactStats {
    fn bump(&mut self, label: &str, n: usize) {
        if n == 0 {
            return;
        }
        self.total += n;
        *self.by_label.entry(label.to_string()).or_insert(0) += n;
    }
}

/// The main engine.
pub struct Redactor {
    opts: RedactorOptions,
    entropy: Option<EntropyRedactor>,
}

impl Redactor {
    pub fn new(opts: RedactorOptions) -> Self {
        let entropy = opts
            .entropy
            .map(|(min_len, min_bits)| EntropyRedactor::new(min_len, min_bits));
        Self { opts, entropy }
    }

    /// Stream `reader` through the redactor into `writer`.
    ///
    /// Returns statistics describing what was redacted. IO errors propagate
    /// as `Err`; malformed UTF-8 in the input is replaced rather than
    /// rejected, so binary-ish input still produces something usable.
    pub fn run<R: BufRead, W: Write>(
        &self,
        reader: R,
        mut writer: W,
    ) -> std::io::Result<RedactStats> {
        let mut stats = RedactStats::default();
        let mut active_block: Option<usize> = None; // index into self.opts.patterns

        let mut buf = Vec::new();
        let mut reader = reader;

        loop {
            buf.clear();
            let read = reader.read_until(b'\n', &mut buf)?;
            if read == 0 {
                break;
            }

            // Detach the trailing newline (if any) so regex anchors behave
            // sensibly and we can reattach the exact line terminator below.
            let had_nl = buf.ends_with(b"\n");
            let had_crlf = had_nl && buf.ends_with(b"\r\n");
            let content_len = if had_crlf {
                buf.len() - 2
            } else if had_nl {
                buf.len() - 1
            } else {
                buf.len()
            };
            let line = String::from_utf8_lossy(&buf[..content_len]).into_owned();

            let terminator: &[u8] = if had_crlf {
                b"\r\n"
            } else if had_nl {
                b"\n"
            } else {
                b""
            };

            // --- Multi-line block handling ---
            if let Some(idx) = active_block {
                let pat = &self.opts.patterns[idx];
                if let PatternKind::Block { end, .. } = &pat.kind {
                    if end.is_match(&line) {
                        // End of the block; emit the placeholder for the
                        // whole region and clear the state.
                        writeln!(writer, "<REDACTED:{}>", pat.label)?;
                        stats.bump(&pat.label, 1);
                        active_block = None;
                    }
                    // Either way, the content of this line is swallowed.
                    continue;
                }
            }

            // Do any block patterns START on this line?
            let mut entered_block = None;
            for (i, pat) in self.opts.patterns.iter().enumerate() {
                if let PatternKind::Block { start, .. } = &pat.kind {
                    if start.is_match(&line) {
                        entered_block = Some(i);
                        break;
                    }
                }
            }
            if let Some(i) = entered_block {
                active_block = Some(i);
                // Do not emit the opening line; we'll emit a single
                // placeholder when the end regex matches.
                continue;
            }

            // --- Single-line pattern sweep ---
            let redacted = self.apply_line(&line, &mut stats);

            writer.write_all(redacted.as_bytes())?;
            writer.write_all(terminator)?;
        }

        // If the input ended mid-block, still emit a placeholder so the
        // output does not lie about having finished the block cleanly.
        if let Some(idx) = active_block {
            let pat = &self.opts.patterns[idx];
            writeln!(writer, "<REDACTED:{}:truncated>", pat.label)?;
            stats.bump(&pat.label, 1);
        }

        Ok(stats)
    }

    fn apply_line(&self, line: &str, stats: &mut RedactStats) -> String {
        let mut current = line.to_string();

        // Line patterns first. We iterate in catalogue order; earlier
        // patterns "win" because they've already consumed their matches.
        for pat in &self.opts.patterns {
            if let PatternKind::Line { regex } = &pat.kind {
                let label = &pat.label;
                let mut count = 0;
                let replaced = regex.replace_all(&current, |caps: &regex::Captures| {
                    let matched = caps.get(0).unwrap().as_str();
                    if self.is_allowlisted(matched) {
                        matched.to_string()
                    } else {
                        count += 1;
                        self.placeholder(label, matched)
                    }
                });
                stats.bump(label, count);
                current = replaced.into_owned();
            }
        }

        // Entropy heuristic last, so structured patterns take precedence
        // and we don't double-redact.
        if let Some(entropy) = &self.entropy {
            let allowlist = self.opts.allowlist.clone();
            let keep_last = self.opts.keep_last;
            let (out, count) = entropy.apply(&current, move |value| {
                if allowlist.iter().any(|a| a == value) {
                    return value.to_string();
                }
                format_placeholder("entropy", value, keep_last)
            });
            stats.bump("entropy", count);
            current = out;
        }

        current
    }

    fn is_allowlisted(&self, matched: &str) -> bool {
        self.opts
            .allowlist
            .iter()
            .any(|a| a == matched || matched.contains(a.as_str()))
    }

    fn placeholder(&self, label: &str, matched: &str) -> String {
        format_placeholder(label, matched, self.opts.keep_last)
    }
}

fn format_placeholder(label: &str, matched: &str, keep_last: usize) -> String {
    if keep_last == 0 {
        return format!("<REDACTED:{label}>");
    }
    let chars: Vec<char> = matched.chars().collect();
    if chars.len() <= keep_last {
        // Too short to usefully preview; just redact.
        return format!("<REDACTED:{label}>");
    }
    let hint: String = chars[chars.len() - keep_last..].iter().collect();
    format!("<REDACTED:{label}:…{hint}>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn run_default(input: &str) -> (String, RedactStats) {
        let r = Redactor::new(RedactorOptions::default());
        let mut out = Vec::new();
        let stats = r.run(Cursor::new(input.as_bytes()), &mut out).unwrap();
        (String::from_utf8(out).unwrap(), stats)
    }

    #[test]
    fn aws_access_key_is_redacted() {
        let (out, stats) = run_default("key=AKIAIOSFODNN7EXAMPLE\n");
        assert!(out.contains("<REDACTED:aws-access-key>"), "got {out}");
        assert_eq!(stats.total, 1);
    }

    #[test]
    fn github_pat_is_redacted() {
        let (out, _) = run_default("export TOKEN=ghp_abcdefghijklmnopqrstuvwxyz0123456789\n");
        assert!(out.contains("<REDACTED:github-pat>"), "got {out}");
    }

    #[test]
    fn private_key_block_collapses_to_single_line() {
        let input = "\
before\n\
-----BEGIN RSA PRIVATE KEY-----\n\
MIIEpAIBAAKCAQEA...\n\
xxxxxxxxxxxxxxxx\n\
-----END RSA PRIVATE KEY-----\n\
after\n";
        let (out, stats) = run_default(input);
        let lines: Vec<_> = out.lines().collect();
        assert_eq!(lines.len(), 3, "got {lines:?}");
        assert_eq!(lines[0], "before");
        assert_eq!(lines[1], "<REDACTED:private-key>");
        assert_eq!(lines[2], "after");
        assert_eq!(stats.by_label.get("private-key"), Some(&1));
    }

    #[test]
    fn crlf_line_endings_are_preserved() {
        let input = "plain\r\nkey=AKIAIOSFODNN7EXAMPLE\r\n";
        let (out, _) = run_default(input);
        assert!(out.contains("\r\n"), "CRLF missing in output: {out:?}");
    }

    #[test]
    fn allowlist_prevents_redaction() {
        let mut opts = RedactorOptions::default();
        opts.allowlist.push("user@example.com".into());
        let r = Redactor::new(opts);
        let mut out = Vec::new();
        r.run(
            Cursor::new("contact: user@example.com\n".as_bytes()),
            &mut out,
        )
        .unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("user@example.com"), "got {s}");
    }

    #[test]
    fn keep_last_appends_hint() {
        let opts = RedactorOptions {
            keep_last: 4,
            entropy: None,
            ..RedactorOptions::default()
        };
        let r = Redactor::new(opts);
        let mut out = Vec::new();
        r.run(
            Cursor::new("key=AKIAIOSFODNN7EXAMPLE\n".as_bytes()),
            &mut out,
        )
        .unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("MPLE>"), "expected MPLE hint in {s:?}");
    }

    #[test]
    fn plain_prose_is_not_shredded() {
        let input = "The quick brown fox jumps over the lazy dog.\n";
        let (out, stats) = run_default(input);
        assert_eq!(out, input);
        assert_eq!(stats.total, 0);
    }

    #[test]
    fn stats_count_by_label() {
        let input = "a: AKIAIOSFODNN7EXAMPLE\nb: AKIAABCDEFGHIJKLMNOP\n";
        let (_, stats) = run_default(input);
        assert_eq!(stats.by_label.get("aws-access-key"), Some(&2));
    }

    #[test]
    fn truncated_block_is_flagged() {
        let input = "-----BEGIN RSA PRIVATE KEY-----\nabc\n";
        let (out, _) = run_default(input);
        assert!(
            out.contains("<REDACTED:private-key:truncated>"),
            "got {out}"
        );
    }
}
