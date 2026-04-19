//! censor: stream-through redactor for secrets.
//!
//! The library exposes three layers:
//!
//! * [`patterns`]: the built-in catalogue of secret-like regexes plus a
//!   helper for parsing user-provided rules.
//! * [`entropy`]: a Shannon-entropy helper used to catch high-entropy
//!   tokens that do not match any structured pattern.
//! * [`redactor`]: the streaming engine that consumes a [`BufRead`] and
//!   writes a redacted copy to a [`Write`], with a small state machine
//!   for multi-line secrets such as PEM-armoured private keys.
//!
//! [`BufRead`]: std::io::BufRead
//! [`Write`]: std::io::Write

pub mod config;
pub mod entropy;
pub mod patterns;
pub mod redactor;

pub use config::Config;
pub use patterns::{BuiltinPatterns, Pattern, PatternKind};
pub use redactor::{RedactStats, Redactor, RedactorOptions};
