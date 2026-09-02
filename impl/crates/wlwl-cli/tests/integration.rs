//! End-to-end integration tests for the WLWL Phase 1 toolchain.
//!
//! These exercise the full lexer → parser → evaluator pipeline against
//! small .wl programs (read from disk via the `wlwl-cli` binary, or via
//! direct library calls to `parse` + `Evaluator::eval`).

use std::fs;
use std::io::Write;
use wlwl_eval::{Evaluator, Value};
use wlwl_error::ErrorCode;
use wlwl_parser::parse;

fn run_source(src: &str) -> Result<Value, ErrorCode> {
    let ast = match parse(src, "t.wl") {
        Ok(a) => a,
        Err(e) => return Err(e.diagnostic().code),
    };
    let mut ev = Evaluator::new();
    ev.eval(&ast).map_err(|e| e.diagnostic().code)
}

#[test]
fn int_1_hello_world() {
    // 1) Hello world — single PRINT
    assert_eq!(run_source("PRINT(\"hello, world!\");").unwrap(), Value::Null);
}

#[test]
fn int_2_let_and_var() {
    // 2) LET binding + variable lookup via PRINT
    //    (we use PRINT to avoid needing the '+' operator in Phase 1)
    assert_eq!(
        run_source("LET(name, \"WLWL\"); PRINT(\"hi\", name);").unwrap(),
        Value::Null
    );
}

#[test]
fn int_3_array_literal() {
    // 3) Array literal — value flows through evaluation
    assert_eq!(
        run_source("[1, 2, 3];").unwrap(),
        Value::Array(vec![
            Value::Integer(1),
            Value::Integer(2),
            Value::Integer(3)
        ])
    );
}

#[test]
fn int_4_dict_literal() {
    // 4) Dict literal — string keys, integer values
    assert_eq!(
        run_source("[\"a\": 1, \"b\": 2];").unwrap(),
        Value::Dict(vec![
            (Value::String("a".into()), Value::Integer(1)),
            (Value::String("b".into()), Value::Integer(2)),
        ])
    );
}

#[test]
fn int_5_block_returns_last_value() {
    // 5) Block expression returns the value of its last sub-expression.
    //    (Not printed; the program block's value is returned to the harness.)
    assert_eq!(
        run_source("LET(x, 1); LET(y, 2); y;").unwrap(),
        Value::Integer(2)
    );
}

#[test]
fn int_6_error_undefined_name() {
    // 6) Undefined name → E0020
    let err = run_source("PRINT(nonexistent);").unwrap_err();
    assert_eq!(err, ErrorCode::E0020);
}

#[test]
fn int_7_error_missing_semicolon() {
    // 7) Two top-level statements, the first missing its ';' → E0013
    //    (A single expression at top level is allowed without trailing ';' —
    //     this matches REPL-like behavior.)
    let err = run_source("LET(x, 1) LET(y, 2);").unwrap_err();
    assert_eq!(err, ErrorCode::E0013);
}

#[test]
fn int_8_error_lex_illegal_char() {
    // 8) Illegal character @ → E0001
    let err = run_source("@bad;").unwrap_err();
    assert_eq!(err, ErrorCode::E0001);
}

/// Helper: write a .wl file to a temp path, run it via the CLI binary, return exit code.
#[test]
fn int_9_cli_runs_hello() {
    use std::process::Command;
    let dir = std::env::temp_dir().join("wlwl-int-tests");
    fs::create_dir_all(&dir).unwrap();
    let p = dir.join("cli_hello.wl");
    let mut f = fs::File::create(&p).unwrap();
    f.write_all(b"LET(x, 1); PRINT(\"x =\", x);").unwrap();
    drop(f);

    // The CLI binary may not be on PATH; use cargo run as a fallback.
    // In a workspace, the bin is at `target/debug/wlwl.exe` after build.
    // We try the workspace's target dir first.
    let workspace_target = std::env::current_dir()
        .unwrap()
        .parent() // tests/ is inside impl/
        .map(|p| p.join("target").join("debug").join(if cfg!(windows) { "wlwl.exe" } else { "wlwl" }))
        .unwrap_or_else(|| PathBuf::from("target/debug/wlwl"));

    let candidates = [
        workspace_target,
        PathBuf::from("target/debug/wlwl"),
        PathBuf::from("wlwl"),
    ];
    let exe = candidates
        .into_iter()
        .find(|c| c.exists())
        .unwrap_or_else(|| PathBuf::from("wlwl"));

    let out = Command::new(&exe)
        .arg("run")
        .arg(&p)
        .output()
        .expect("failed to spawn wlwl binary");

    assert!(
        out.status.success(),
        "wlwl run failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("x = 1"), "stdout: {}", stdout);
}

use std::path::PathBuf;
