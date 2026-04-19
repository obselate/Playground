//! End-to-end tests for the `censor` binary.
//!
//! We invoke the compiled binary that Cargo produces via
//! `env!("CARGO_BIN_EXE_censor")`. All paths go through `tempfile`, so the
//! tests work unchanged on Linux, macOS, and Windows.

use std::io::Write;
use std::process::{Command, Stdio};

use tempfile::TempDir;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_censor")
}

/// Invoke the binary, feeding `stdin_text` and returning
/// `(stdout, stderr, status)`.
fn run(args: &[&str], stdin_text: &str) -> (String, String, i32) {
    let mut cmd = Command::new(bin());
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn censor");
    {
        let stdin = child.stdin.as_mut().expect("child stdin");
        stdin.write_all(stdin_text.as_bytes()).expect("write stdin");
    }
    let output = child.wait_with_output().expect("wait censor");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn redacts_github_token_from_stdin() {
    let (out, _, code) = run(
        &[],
        "export GITHUB_TOKEN=ghp_abcdefghijklmnopqrstuvwxyz0123456789\n",
    );
    assert_eq!(code, 0);
    assert!(
        out.contains("<REDACTED:github-pat>"),
        "expected github-pat marker, got {out:?}"
    );
    // The original token should not survive in the output.
    assert!(
        !out.contains("ghp_abcdefghijklmnopqrstuvwxyz0123456789"),
        "raw token leaked: {out:?}"
    );
}

#[test]
fn strict_mode_nonzero_exit_on_redaction() {
    let (_, _, code) = run(&["--strict"], "AKIAIOSFODNN7EXAMPLE\n");
    assert_eq!(
        code, 1,
        "--strict should exit 1 when something was redacted"
    );
}

#[test]
fn strict_mode_zero_exit_on_clean_input() {
    let (_, _, code) = run(&["--strict"], "the quick brown fox\n");
    assert_eq!(code, 0);
}

#[test]
fn report_flag_emits_json_to_stderr() {
    let (_, err, code) = run(&["--report"], "AKIAIOSFODNN7EXAMPLE\n");
    assert_eq!(code, 0);
    assert!(err.contains("\"total\":1"), "stderr was {err:?}");
    assert!(err.contains("\"aws-access-key\":1"), "stderr was {err:?}");
}

#[test]
fn list_patterns_prints_known_labels() {
    let (out, _, code) = run(&["--list-patterns"], "");
    assert_eq!(code, 0);
    for expected in ["aws-access-key", "github-pat", "private-key"] {
        assert!(
            out.contains(expected),
            "expected {expected} in pattern list, got {out:?}"
        );
    }
}

#[test]
fn file_input_is_supported() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("sample.log");
    std::fs::write(&path, "key=AKIAIOSFODNN7EXAMPLE\n").expect("write sample");

    let (out, _, code) = run(
        &[path.to_str().expect("utf-8 path")],
        "", // no stdin
    );
    assert_eq!(code, 0);
    assert!(out.contains("<REDACTED:aws-access-key>"), "got {out:?}");
}

#[test]
fn output_flag_writes_to_file_and_not_stdout() {
    let dir = TempDir::new().expect("tempdir");
    let out_path = dir.path().join("clean.log");
    let (stdout, _, code) = run(
        &["--output", out_path.to_str().expect("utf-8 path")],
        "AKIAIOSFODNN7EXAMPLE\n",
    );
    assert_eq!(code, 0);
    assert!(stdout.is_empty(), "stdout should be empty, got {stdout:?}");
    let body = std::fs::read_to_string(&out_path).expect("read output");
    assert!(
        body.contains("<REDACTED:aws-access-key>"),
        "output file: {body:?}"
    );
}

#[test]
fn config_file_adds_custom_patterns_and_allowlist() {
    let dir = TempDir::new().expect("tempdir");
    let cfg_path = dir.path().join("censor.toml");
    std::fs::write(
        &cfg_path,
        r#"
allow = ["alice@example.com"]

[[patterns]]
label = "tenant-id"
regex = 'tenant-[A-Z0-9]{8}'
"#,
    )
    .expect("write config");

    let (out, _, code) = run(
        &["--config", cfg_path.to_str().expect("utf-8 path")],
        "tenant-ABCD1234 mail alice@example.com bob@example.com\n",
    );
    assert_eq!(code, 0);
    assert!(out.contains("<REDACTED:tenant-id>"), "got {out:?}");
    // allowlisted email survives, un-allowlisted one is scrubbed.
    assert!(out.contains("alice@example.com"), "got {out:?}");
    assert!(!out.contains("bob@example.com"), "got {out:?}");
}

#[test]
fn disable_flag_removes_builtin_pattern() {
    // Use a long-ish email to also exceed the entropy threshold; then
    // turn entropy off so the only surviving detector would be `email`.
    let (out, _, code) = run(
        &["--disable", "email", "--no-entropy"],
        "contact: alice@example.com\n",
    );
    assert_eq!(code, 0);
    assert!(out.contains("alice@example.com"), "got {out:?}");
}

#[test]
fn crlf_input_produces_crlf_output() {
    let (out, _, _) = run(&[], "plain\r\nAKIAIOSFODNN7EXAMPLE\r\n");
    assert!(out.contains("\r\n"), "CRLF missing: {out:?}");
}
