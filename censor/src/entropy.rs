//! Shannon-entropy helpers for catching high-entropy tokens that slip
//! past the structured patterns.
//!
//! The heuristic is intentionally simple: split each line on common
//! delimiters, and redact any sufficiently long, sufficiently random token
//! that looks key-shaped. We only run this on tokens *following* a
//! key-like prefix (e.g. `token=`, `secret:`, `Authorization:`) to keep
//! the false-positive rate manageable on natural text.

use regex::Regex;

/// Compute Shannon entropy in bits per character.
///
/// An empty string returns 0.0.
pub fn shannon_bits_per_char(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let len = s.chars().count() as f64;
    let mut counts: std::collections::HashMap<char, u32> = std::collections::HashMap::new();
    for ch in s.chars() {
        *counts.entry(ch).or_insert(0) += 1;
    }
    let mut total = 0.0;
    for &c in counts.values() {
        let p = c as f64 / len;
        total -= p * p.log2();
    }
    total
}

/// A context-aware entropy redactor.
///
/// Looks for `key=value` / `key: value` shapes where the key name suggests
/// a credential and the value is long and high-entropy, then redacts the
/// value. This catches custom tokens that the built-in catalogue doesn't
/// know about (e.g. internal service keys).
pub struct EntropyRedactor {
    assignment_regex: Regex,
    min_length: usize,
    min_entropy_bits: f64,
}

impl EntropyRedactor {
    pub fn new(min_length: usize, min_entropy_bits: f64) -> Self {
        // Match `key=value` and `key: value` where the key is one of the
        // usual suspect nouns. We capture (prefix, value) so the caller
        // can swap just the value portion.
        let rx = Regex::new(
            r#"(?i)(?P<prefix>\b(?:password|passwd|pwd|secret|token|api[_\-]?key|access[_\-]?key|client[_\-]?secret|private[_\-]?key|auth(?:orization)?|session[_\-]?id|bearer)\b\s*[=:]\s*)(?P<value>[A-Za-z0-9+/=\-_\.]{8,})"#,
        )
        .expect("entropy regex failed to compile");
        Self {
            assignment_regex: rx,
            min_length,
            min_entropy_bits,
        }
    }

    /// Return `true` if the value looks key-like and should be redacted.
    pub fn should_redact(&self, value: &str) -> bool {
        if value.chars().count() < self.min_length {
            return false;
        }
        shannon_bits_per_char(value) >= self.min_entropy_bits
    }

    /// Run the entropy check over `line` and return a new string with
    /// matching values replaced by `redacted`. The caller supplies the
    /// redaction token so this module stays string-representation agnostic.
    pub fn apply<'a>(
        &'a self,
        line: &'a str,
        mut redacted_for: impl FnMut(&str) -> String,
    ) -> (String, usize) {
        let mut count = 0;
        let out = self
            .assignment_regex
            .replace_all(line, |caps: &regex::Captures| {
                let prefix = &caps["prefix"];
                let value = &caps["value"];
                if self.should_redact(value) {
                    count += 1;
                    format!("{prefix}{}", redacted_for(value))
                } else {
                    caps.get(0).unwrap().as_str().to_string()
                }
            })
            .into_owned();
        (out, count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_string_has_zero_entropy() {
        assert_eq!(shannon_bits_per_char("aaaaaa"), 0.0);
    }

    #[test]
    fn diverse_string_has_nonzero_entropy() {
        let bits = shannon_bits_per_char("abcdef1234");
        assert!(bits > 3.0, "expected > 3 bits, got {bits}");
    }

    #[test]
    fn entropy_redactor_flags_random_token() {
        let r = EntropyRedactor::new(10, 3.0);
        let (out, count) = r.apply("api_key=Zx9qRt2LmNpVbAa7KcYwQe", |_| {
            "<REDACTED:entropy>".into()
        });
        assert_eq!(count, 1);
        assert!(out.contains("<REDACTED:entropy>"), "got {out}");
    }

    #[test]
    fn entropy_redactor_leaves_low_entropy_value() {
        let r = EntropyRedactor::new(10, 4.5);
        let (out, count) = r.apply("password=aaaaaaaaaa", |_| "<REDACTED>".into());
        assert_eq!(count, 0);
        assert_eq!(out, "password=aaaaaaaaaa");
    }
}
