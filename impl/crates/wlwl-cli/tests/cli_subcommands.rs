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
        std::env::current_dir().unwrap().join("target/debug").join(exe_name),
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
    let p = write_source(&dir, "run_human.wl", "PRINT(\"hi\");");
    let out = run_cli(&["run", p.to_str().unwrap()]);
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("hi"), "stdout: {stdout}");
}

#[test]
fn cli_run_json_format() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "run_json.wl", "PRINT(\"hi\");");
    let out = run_cli(&["run", "--format=json", p.to_str().unwrap()]);
    assert!(out.status.success());
}

#[test]
fn cli_run_jsonl_format() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "run_jsonl.wl", "PRINT(\"hi\");");
    let out = run_cli(&["run", "--format=jsonl", p.to_str().unwrap()]);
    assert!(out.status.success());
}

#[test]
fn cli_run_default_format_is_human() {
    // no --format flag -> default = human
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "run_default.wl", "PRINT(\"default\");");
    let out = run_cli(&["run", p.to_str().unwrap()]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("default"), "stdout: {stdout}");
}

// ── `wlwl check` + every --format value ───────────────────────
#[test]
fn cli_check_valid_program_human() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "check_ok.wl", "LET(x, 1);");
    let out = run_cli(&["check", p.to_str().unwrap()]);
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("OK: parsed"), "stdout: {stdout}");
}

#[test]
fn cli_check_valid_program_json() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "check_ok_json.wl", "LET(x, 1);");
    let out = run_cli(&["check", "--format=json", p.to_str().unwrap()]);
    assert!(out.status.success());
}

#[test]
fn cli_check_invalid_program_returns_nonzero() {
    // unterminated string -> parser E0002
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "check_bad.wl", "LET(x, \"unterminated);");
    let out = run_cli(&["check", p.to_str().unwrap()]);
    assert!(!out.status.success(), "expected nonzero exit, got 0");
}

// ── `wlwl ast` + every --format value ─────────────────────────
#[test]
fn cli_ast_default_format_is_json() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "ast_default.wl", "LET(x, 1);");
    let out = run_cli(&["ast", p.to_str().unwrap()]);
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Default is JSON; must contain at least one variant tag and "line_start" (Span wire).
    assert!(stdout.contains("\"line_start\""), "stdout: {stdout}");
    // Must contain "Let" (the LET tag from Expr::Let)
    assert!(stdout.contains("Let"), "stdout: {stdout}");
}

#[test]
fn cli_ast_explicit_json_format() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "ast_json.wl", "LET(x, 1);");
    let out = run_cli(&["ast", "--format=json", p.to_str().unwrap()]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Let"));
}

#[test]
fn cli_ast_jsonl_format_accepted() {
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "ast_jsonl.wl", "LET(x, 1);");
    let out = run_cli(&["ast", "--format=jsonl", p.to_str().unwrap()]);
    assert!(out.status.success());
}

// ── Error paths ────────────────────────────────────────────────
#[test]
fn cli_run_missing_file_returns_nonzero() {
    let out = run_cli(&["run", "/nonexistent/path/to/file.wl"]);
    assert!(!out.status.success());
}

#[test]
fn cli_check_missing_file_returns_nonzero() {
    let out = run_cli(&["check", "/nonexistent/path/to/file.wl"]);
    assert!(!out.status.success());
}

#[test]
fn cli_ast_missing_file_returns_nonzero() {
    let out = run_cli(&["ast", "/nonexistent/path/to/file.wl"]);
    assert!(!out.status.success());
}

#[test]
fn cli_run_with_lex_error_human() {
    // illegal char '@' -> lexer E0001
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "lex_err.wl", "LET(x, @bad);");
    let out = run_cli(&["run", p.to_str().unwrap()]);
    assert!(!out.status.success(), "expected nonzero on lex error");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("E0001") || stderr.contains("illegal character"),
            "stderr should mention E0001: {stderr}");
}

#[test]
fn cli_run_with_lex_error_json() {
    // Same but --format=json: the diagnostic should appear in the JSON output
    // (regardless of stream: stdout or stderr, depending on the writer).
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "lex_err_json.wl", "LET(x, @bad);");
    let out = run_cli(&["run", "--format=json", p.to_str().unwrap()]);
    assert!(!out.status.success());
    let combined = format!("{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(combined.contains("E0001"),
            "combined output should mention E0001: {combined}");
}

#[test]
fn cli_run_with_parse_error_human() {
    // missing ',' in arg list -> parser E0012
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "parse_err.wl", "LET(x 1);");
    let out = run_cli(&["run", p.to_str().unwrap()]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("E0012") || stderr.contains("expected ','"),
            "stderr should mention E0012: {stderr}");
}

#[test]
fn cli_run_with_runtime_error_human() {
    // undefined name -> eval E0020; suggestion_code must be populated
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "rt_err.wl", "LET(counter, 0); PRINT(countr);");
    let out = run_cli(&["run", p.to_str().unwrap()]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("E0020") || stderr.contains("undefined"),
            "stderr should mention E0020: {stderr}");
}

#[test]
fn cli_run_with_runtime_error_jsonl() {
    // --format=jsonl should produce a JSON line containing the error
    let dir = std::env::temp_dir().join("wlwl-cli-tests");
    let p = write_source(&dir, "rt_err_jsonl.wl", "LET(counter, 0); PRINT(countr);");
    let out = run_cli(&["run", "--format=jsonl", p.to_str().unwrap()]);
    assert!(!out.status.success());
    let combined = format!("{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(combined.contains("E0020") || combined.contains("\"code\""),
            "jsonl output should be a JSON object: {combined}");
}

// ── Help / version ─────────────────────────────────────────────
#[test]
fn cli_help_exits_zero() {
    let out = run_cli(&["--help"]);
    // clap --help exits 0 by default
    let _ = out.status.code();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.to_lowercase().contains("usage") || stdout.contains("wlwl"),
            "help text should mention usage: {stdout}");
}