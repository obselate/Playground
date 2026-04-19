//! `censor` CLI entrypoint.
//!
//! See `censor --help` for the full synopsis. The CLI is a thin shell over
//! [`censor::Redactor`]; all the policy lives in the library.

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{ArgAction, Parser};

use censor::patterns::PatternSpec;
use censor::{BuiltinPatterns, Config, Redactor, RedactorOptions};

#[derive(Parser, Debug)]
#[command(
    name = "censor",
    about = "Stream-through redactor for secrets in logs and shell output.",
    version
)]
struct Cli {
    /// Input files. Pass `-` or omit entirely to read from stdin.
    files: Vec<PathBuf>,

    /// Write output to this path instead of stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// TOML config file with custom patterns, disables, and allowlist.
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Disable a built-in pattern by label. May be repeated.
    #[arg(long = "disable", value_name = "LABEL")]
    disable: Vec<String>,

    /// Add an allowlist string. May be repeated.
    #[arg(long = "allow", value_name = "STRING")]
    allow: Vec<String>,

    /// Turn off the entropy-based fallback heuristic.
    #[arg(long, action = ArgAction::SetTrue)]
    no_entropy: bool,

    /// Minimum length for entropy detection (default: 24).
    #[arg(long, value_name = "N")]
    entropy_min_length: Option<usize>,

    /// Minimum Shannon entropy in bits/char for entropy detection
    /// (default: 4.0).
    #[arg(long, value_name = "BITS")]
    entropy_min_bits: Option<f64>,

    /// Keep the last N characters of each redacted value as a hint in
    /// the replacement marker.
    #[arg(long, value_name = "N", default_value_t = 0)]
    keep_last: usize,

    /// Print a JSON redaction summary to stderr after the run.
    #[arg(long, action = ArgAction::SetTrue)]
    report: bool,

    /// Exit with a non-zero status if any redactions happened. Useful
    /// for pre-commit hooks that want to fail on potential leaks.
    #[arg(long, action = ArgAction::SetTrue)]
    strict: bool,

    /// Print the names of all built-in patterns and exit.
    #[arg(long, action = ArgAction::SetTrue)]
    list_patterns: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("censor: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();

    if cli.list_patterns {
        for name in BuiltinPatterns::names() {
            println!("{name}");
        }
        return Ok(ExitCode::SUCCESS);
    }

    let config = match &cli.config {
        Some(path) => Config::from_path(path)?,
        None => Config::default(),
    };

    let opts = build_options(&cli, config)?;
    let redactor = Redactor::new(opts);

    // Resolve output sink before we start streaming so we fail fast on
    // bad paths.
    let mut sink: Box<dyn Write> = match &cli.output {
        Some(path) => Box::new(BufWriter::new(
            File::create(path).with_context(|| format!("opening {}", path.display()))?,
        )),
        None => Box::new(BufWriter::new(io::stdout().lock())),
    };

    let mut total_stats = censor::RedactStats::default();

    // Sources: each file in turn, or stdin if none/`-`.
    if cli.files.is_empty() || cli.files.iter().any(|p| p.as_os_str() == "-") {
        let stdin = io::stdin();
        let handle = stdin.lock();
        let reader = BufReader::new(handle);
        let stats = redactor
            .run(reader, &mut sink)
            .context("streaming stdin through censor")?;
        merge_stats(&mut total_stats, stats);
    } else {
        for path in &cli.files {
            let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
            let reader = BufReader::new(file);
            let stats = redactor
                .run(reader, &mut sink)
                .with_context(|| format!("streaming {} through censor", path.display()))?;
            merge_stats(&mut total_stats, stats);
        }
    }

    sink.flush()?;
    drop(sink); // release stdout lock or close file

    if cli.report {
        emit_report(&total_stats)?;
    }

    if cli.strict && total_stats.total > 0 {
        return Ok(ExitCode::from(1));
    }
    Ok(ExitCode::SUCCESS)
}

fn build_options(cli: &Cli, mut config: Config) -> Result<RedactorOptions> {
    // Start with the built-in catalogue, minus any disables from either
    // the CLI or the config.
    let mut disable: std::collections::HashSet<String> = cli.disable.iter().cloned().collect();
    for d in config.disable.drain(..) {
        disable.insert(d);
    }

    let mut patterns: Vec<_> = BuiltinPatterns::all()
        .into_iter()
        .filter(|p| !disable.contains(&p.label))
        .collect();

    // Layer user patterns on top so they can shadow nothing (order-wise
    // later) but still participate.
    for spec in config.patterns {
        patterns.push(compile_spec(spec)?);
    }

    let mut allowlist = cli.allow.clone();
    allowlist.append(&mut config.allow);

    let entropy = if cli.no_entropy {
        None
    } else {
        let min_len = cli
            .entropy_min_length
            .or(config.entropy.min_length)
            .unwrap_or(24);
        let min_bits = cli
            .entropy_min_bits
            .or(config.entropy.min_bits)
            .unwrap_or(4.0);
        Some((min_len, min_bits))
    };

    Ok(RedactorOptions {
        patterns,
        allowlist,
        entropy,
        keep_last: cli.keep_last,
    })
}

fn compile_spec(spec: PatternSpec) -> Result<censor::Pattern> {
    spec.compile()
}

fn merge_stats(dst: &mut censor::RedactStats, src: censor::RedactStats) {
    dst.total += src.total;
    for (k, v) in src.by_label {
        *dst.by_label.entry(k).or_insert(0) += v;
    }
}

fn emit_report(stats: &censor::RedactStats) -> Result<()> {
    // Hand-written JSON to avoid a serde_json dependency for a tiny payload.
    // Sorted keys keep the output diff-stable across runs.
    let mut entries: Vec<(&String, &usize)> = stats.by_label.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));

    let mut out = String::new();
    out.push_str("{\"total\":");
    out.push_str(&stats.total.to_string());
    out.push_str(",\"by_label\":{");
    for (i, (label, count)) in entries.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(&escape_json(label));
        out.push_str("\":");
        out.push_str(&count.to_string());
    }
    out.push_str("}}");

    let mut stderr = io::stderr().lock();
    stderr.write_all(out.as_bytes())?;
    stderr.write_all(b"\n")?;
    Ok(())
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}
