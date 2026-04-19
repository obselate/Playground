//! Config file support.
//!
//! A config file is an optional TOML document that layers user preferences
//! on top of the built-in defaults. Everything is optional, so an empty
//! file is a valid config.
//!
//! ```toml
//! # censor.toml
//!
//! # Append custom patterns on top of the built-ins.
//! [[patterns]]
//! label = "internal-tenant-id"
//! regex = 'tenant-[A-Z0-9]{12}'
//!
//! # Disable built-in patterns by label.
//! disable = ["email", "private-ipv4"]
//!
//! # Strings that should never be redacted, even if they match a pattern.
//! allow = ["user@example.com", "127.0.0.1"]
//!
//! # Entropy thresholds for the fallback heuristic.
//! [entropy]
//! min_length = 20
//! min_bits = 4.0
//! ```

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::patterns::PatternSpec;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub patterns: Vec<PatternSpec>,
    #[serde(default)]
    pub disable: Vec<String>,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub entropy: EntropyConfig,
}

#[derive(Debug, Deserialize, Default)]
pub struct EntropyConfig {
    pub min_length: Option<usize>,
    pub min_bits: Option<f64>,
}

impl Config {
    pub fn from_path(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading config file {}", path.display()))?;
        let cfg: Config = toml::from_str(&text)
            .with_context(|| format!("parsing config file {}", path.display()))?;
        Ok(cfg)
    }
}
