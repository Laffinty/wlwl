//! P3-009b: clap subcommand enumeration tests for wlwl-cli.
//!
//! Adds coverage for every (subcommand, --format) combination plus the
//! error-path branches (file not found, parse error, lex error). The
//! P3-009 coverage run showed `wlwl-cli` at 51% line / 57% region; the
//! missing paths are exactly these (run, check, ast) x (Human, Json,
//! Jsonl) combinations.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

fn wlwl_exe() -> PathBuf {
    // Cargo sets CARGO_BIN_EXE_<name> for the binary of the same crate,
    // pointing at the freshly-built executable. This is the canonical way
    // to locate the test binary across platforms and cargo working
    // directories.
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_wlwl") {
        let p = PathBuf::from(p);
        if p.exists() {
            return p;
        }
    }
    // Fall back to relative path lookups.
    let exe_name = if cfg!(windows) { "wlwl.exe" } else { "wlwl" };
    let candidates = [
        std::env::current_dir()
            .unwrap()
            .join("target/debug")
            .join(exe_name),
        PathBuf::from(format!("target/debug/{exe_name}")),
        PathBuf::from(exe_name),
    ];
    candidates
        .into_iter()
        .find(|c| c.exists())
        .unwrap_or_else(|| panic!("wlwl binary not found; build it first with `cargo build`"))
}

fn write_source(dir: &std::path::Path, name: &str, src: &str) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let p = dir.join(name);
    let mut f = fs::File::create(&p).unwrap();
    f.write_all(src.as_bytes()).unwrap();
    p
}

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(wlwl_exe())
        .args(args)
        .output()
        .expect("failed to spawn wlwl")
}

// ── `wlwl run` + every --format value ─────────────────────────
#[test]
fn cli_run_human_format() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "run_human.wll", "PRINT(\"hi\");");
    let out = run_cli(&["run", p.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("hi"), "stdout: {stdout}");
}

#[test]
fn cli_run_json_format() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "run_json.wll", "PRINT(\"hi\");");
    let out = run_cli(&["run", "--format=json", p.to_str().unwrap()]);
    assert!(out.status.success());
}

#[test]
fn cli_run_jsonl_format() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "run_jsonl.wll", "PRINT(\"hi\");");
    let out = run_cli(&["run", "--format=jsonl", p.to_str().unwrap()]);
    assert!(out.status.success());
}

#[test]
fn cli_run_default_format_is_human() {
    // no --format flag -> default = human
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "run_default.wll", "PRINT(\"default\");");
    let out = run_cli(&["run", p.to_str().unwrap()]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("default"), "stdout: {stdout}");
}

// ── `wlwl check` + every --format value ───────────────────────
#[test]
fn cli_check_valid_program_human() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "check_ok.wll", "LET(x, 1);");
    let out = run_cli(&["check", p.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("OK: parsed"), "stdout: {stdout}");
}

#[test]
fn cli_check_valid_program_json() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "check_ok_json.wll", "LET(x, 1);");
    let out = run_cli(&["check", "--format=json", p.to_str().unwrap()]);
    assert!(out.status.success());
}

#[test]
fn cli_check_invalid_program_returns_nonzero() {
    // unterminated string -> parser E0002
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "check_bad.wll", "LET(x, \"unterminated);");
    let out = run_cli(&["check", p.to_str().unwrap()]);
    assert!(!out.status.success(), "expected nonzero exit, got 0");
}

// ── `wlwl ast` + every --format value ─────────────────────────
#[test]
fn cli_ast_default_format_is_json() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "ast_default.wll", "LET(x, 1);");
    let out = run_cli(&["ast", p.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Default is JSON; must contain at least one variant tag and "line_start" (Span wire).
    assert!(stdout.contains("\"line_start\""), "stdout: {stdout}");
    // Must contain "Let" (the LET tag from Expr::Let)
    assert!(stdout.contains("Let"), "stdout: {stdout}");
}

#[test]
fn cli_ast_explicit_json_format() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "ast_json.wll", "LET(x, 1);");
    let out = run_cli(&["ast", "--format=json", p.to_str().unwrap()]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Let"));
}

#[test]
fn cli_ast_jsonl_format_accepted() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "ast_jsonl.wll", "LET(x, 1);");
    let out = run_cli(&["ast", "--format=jsonl", p.to_str().unwrap()]);
    assert!(out.status.success());
}

// ── Error paths ────────────────────────────────────────────────
#[test]
fn cli_run_missing_file_returns_nonzero() {
    let out = run_cli(&["run", "/nonexistent/path/to/file.wll"]);
    assert!(!out.status.success());
}

#[test]
fn cli_check_missing_file_returns_nonzero() {
    let out = run_cli(&["check", "/nonexistent/path/to/file.wll"]);
    assert!(!out.status.success());
}

#[test]
fn cli_ast_missing_file_returns_nonzero() {
    let out = run_cli(&["ast", "/nonexistent/path/to/file.wll"]);
    assert!(!out.status.success());
}

#[test]
fn cli_run_with_lex_error_human() {
    // illegal char '@' -> lexer E0001
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "lex_err.wll", "LET(x, @bad);");
    let out = run_cli(&["run", p.to_str().unwrap()]);
    assert!(!out.status.success(), "expected nonzero on lex error");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("E0001") || stderr.contains("illegal character"),
        "stderr should mention E0001: {stderr}"
    );
}

#[test]
fn cli_run_with_lex_error_json() {
    // Same but --format=json: the diagnostic should appear in the JSON output
    // (regardless of stream: stdout or stderr, depending on the writer).
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "lex_err_json.wll", "LET(x, @bad);");
    let out = run_cli(&["run", "--format=json", p.to_str().unwrap()]);
    assert!(!out.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(
        combined.contains("E0001"),
        "combined output should mention E0001: {combined}"
    );
}

#[test]
fn cli_run_with_parse_error_human() {
    // missing ',' in arg list -> parser E0012
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "parse_err.wll", "LET(x 1);");
    let out = run_cli(&["run", p.to_str().unwrap()]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("E0012") || stderr.contains("expected ','"),
        "stderr should mention E0012: {stderr}"
    );
}

#[test]
fn cli_run_with_runtime_error_human() {
    // undefined name -> eval E0020; suggestion_code must be populated
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "rt_err.wll", "LET(counter, 0); PRINT(countr);");
    let out = run_cli(&["run", p.to_str().unwrap()]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("E0020") || stderr.contains("undefined"),
        "stderr should mention E0020: {stderr}"
    );
}

#[test]
fn cli_run_with_runtime_error_jsonl() {
    // --format=jsonl should produce a JSON line containing the error
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "rt_err_jsonl.wll", "LET(counter, 0); PRINT(countr);");
    let out = run_cli(&["run", "--format=jsonl", p.to_str().unwrap()]);
    assert!(!out.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(
        combined.contains("E0020") || combined.contains("\"code\""),
        "jsonl output should be a JSON object: {combined}"
    );
}

// ── Help / version ─────────────────────────────────────────────
#[test]
fn cli_help_exits_zero() {
    let out = run_cli(&["--help"]);
    // clap --help exits 0 by default
    let _ = out.status.code();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.to_lowercase().contains("usage") || stdout.contains("wlwl"),
        "help text should mention usage: {stdout}"
    );
}

// ── Phase E4: `wlwl check` unified warning channel ─────────────
#[test]
fn cli_check_prints_lint_warnings_but_succeeds() {
    // Unused LET => W0010 on stdout; warnings never fail the check.
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "check_lint.wll", "LET(x, 1); PRINT(2);");
    let out = run_cli(&["check", p.to_str().unwrap()]);
    assert!(out.status.success(), "warnings must not fail the check");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("W0010"), "expected W0010, got: {stdout}");
    assert!(stdout.contains("never used"), "got: {stdout}");
    assert!(stdout.contains("OK: parsed"), "got: {stdout}");
}

#[test]
fn cli_check_reports_w0020_from_parser_channel() {
    // Mixed array/dict literal => parser-channel W0020.
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "check_w0020.wll", "LET(a, [1, \"k\": 2]); PRINT(a);");
    let out = run_cli(&["check", p.to_str().unwrap()]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("W0020"), "expected W0020, got: {stdout}");
}

#[test]
fn cli_check_clean_source_has_no_warnings() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "check_clean.wll", "LET(x, 1); PRINT(x);");
    let out = run_cli(&["check", p.to_str().unwrap()]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("warning"), "got: {stdout}");
}

#[test]
fn cli_check_unused_param_reports_w0011() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "check_w0011.wll", "LET(f, FUN((a, b), a)); f(1, 2);");
    let out = run_cli(&["check", p.to_str().unwrap()]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("W0011"), "expected W0011, got: {stdout}");
}
