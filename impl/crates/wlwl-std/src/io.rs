//! `wlwl:std.io` — `PRINT`, `INPUT` (v0.3 §15.1).
//!
//! `PRINT` here is the std-module form. It co-exists with the
//! `resolve_builtin("PRINT")` fallback in `wlwl-eval` so that
//! programs that have NOT imported the std module still work; once
//! the user does `IMPORT("wlwl:std.io", ["PRINT"])`, the env-bound
//! `NativeFn` takes priority (per the dispatch rules in
//! `eval_call`).

use crate::{expect_string, json_type_name, StdCtx, StdError, StdFn, StdValue};
use crate::ModuleSpec;
use std::io::BufRead;
use wlwl_error::ErrorCode;

pub fn std_print(_ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    let parts: Vec<String> = args.iter().map(json_to_print_string).collect();
    println!("{}", parts.join(" "));
    Ok(StdValue::Null)
}

pub fn std_input(_ctx: &mut StdCtx, args: Vec<StdValue>) -> Result<StdValue, StdError> {
    if !args.is_empty() {
        return Err(crate::arity_error("INPUT", args.len(), 0));
    }
    let stdin = std::io::stdin();
    let mut locked = stdin.lock();
    read_input_line(&mut locked).map(StdValue::String)
}

/// Internal helper: read one line from any `BufRead` source. Split
/// out from `std_input` (P3-009c) so the EOF / IO-error / CRLF paths
/// can be tested without spawning a subprocess.
///
/// Contract:
///   * EOF before any byte → returns `Ok("")`.
///   * Reads to `\n` or EOF, then strips trailing `\n` / `\r`.
///   * Any other I/O error → `Err` with `E0060`.
pub(crate) fn read_input_line<R: BufRead>(r: &mut R) -> Result<String, StdError> {
    let mut line = String::new();
    let n = r.read_line(&mut line).map_err(|e| StdError {
        code: ErrorCode::E0060,
        message: format!("INPUT: failed to read stdin: {}", e),
    })?;
    if n == 0 {
        // EOF before any byte
        return Ok(String::new());
    }
    while line.ends_with('\n') || line.ends_with('\r') {
        line.pop();
    }
    Ok(line)
}

/// Convert a `serde_json::Value` to the form `PRINT` should emit. We
/// keep this consistent with the builtin `PRINT` in `wlwl-eval`:
/// strings print as-is, booleans as TRUE/FALSE, null as NULL, and
/// everything else uses `serde_json`'s default formatting.
fn json_to_print_string(v: &StdValue) -> String {
    match v {
        StdValue::String(s) => s.clone(),
        StdValue::Bool(b) => if *b { "TRUE".to_string() } else { "FALSE".to_string() },
        StdValue::Null => "NULL".to_string(),
        other => other.to_string(),
    }
}

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.io",
    functions: &[
        ("PRINT", std_print as StdFn),
        ("INPUT", std_input as StdFn),
    ],
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Error as IoError, ErrorKind, Read};

    #[test]
    fn print_emits_space_joined() {
        let mut ctx = StdCtx::default();
        let out = std_print(
            &mut ctx,
            vec![
                StdValue::String("hello".into()),
                StdValue::Number(serde_json::Number::from(42)),
            ],
        )
        .unwrap();
        assert_eq!(out, StdValue::Null);
    }

    #[test]
    fn print_formatting_matches_builtin() {
        // The std PRINT and the builtin PRINT must agree on
        // null/boolean/array/dict formatting so the user does not see
        // a behaviour shift after IMPORT.
        assert_eq!(json_to_print_string(&StdValue::Null), "NULL");
        assert_eq!(json_to_print_string(&StdValue::Bool(true)), "TRUE");
        assert_eq!(json_to_print_string(&StdValue::Bool(false)), "FALSE");
        let arr = StdValue::Array(vec![StdValue::from(1), StdValue::from(2)]);
        assert_eq!(json_to_print_string(&arr), "[1,2]");
    }

    #[test]
    fn print_formatting_numbers_and_dicts() {
        // Numbers and dicts fall through the `other` arm — verify the
        // serde_json default formatting so the user sees a stable
        // shape across PRINT and STRINGIFY.
        let n = StdValue::Number(serde_json::Number::from(42));
        assert_eq!(json_to_print_string(&n), "42");
        let d = StdValue::Object(serde_json::Map::from_iter([
            ("k".to_string(), StdValue::from(1)),
        ]));
        assert_eq!(json_to_print_string(&d), "{\"k\":1}");
    }

    #[test]
    fn input_arity_mismatch_is_e0022() {
        let mut ctx = StdCtx::default();
        let err = std_input(&mut ctx, vec![StdValue::Null]).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn spec_contains_print_and_input() {
        assert_eq!(SPEC.path, "wlwl:std.io");
        let names: Vec<&str> = SPEC.functions.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"PRINT"));
        assert!(names.contains(&"INPUT"));
    }

    // ---- read_input_line: P3-009c ----

    #[test]
    fn read_line_strips_lf() {
        let mut cur = Cursor::new(b"hello\n".to_vec());
        assert_eq!(read_input_line(&mut cur).unwrap(), "hello");
    }

    #[test]
    fn read_line_strips_crlf() {
        // Windows line ending — should strip both \r and \n.
        let mut cur = Cursor::new(b"hello\r\n".to_vec());
        assert_eq!(read_input_line(&mut cur).unwrap(), "hello");
    }

    #[test]
    fn read_line_strips_trailing_cr_only() {
        // Old Mac line ending — should strip the lone \r.
        let mut cur = Cursor::new(b"hello\r".to_vec());
        assert_eq!(read_input_line(&mut cur).unwrap(), "hello");
    }

    #[test]
    fn read_line_eof_returns_empty_string() {
        // EOF before any byte — return empty string per the
        // documented contract (do NOT raise E0060).
        let mut cur = Cursor::new(Vec::<u8>::new());
        assert_eq!(read_input_line(&mut cur).unwrap(), "");
    }

    #[test]
    fn read_line_eof_after_partial_line_returns_partial() {
        // EOF in the middle of a line (no terminator) is fine — return
        // what we got.
        let mut cur = Cursor::new(b"no-newline".to_vec());
        assert_eq!(read_input_line(&mut cur).unwrap(), "no-newline");
    }

    #[test]
    fn read_line_preserves_empty_line() {
        // A blank line (`\n` immediately) is a valid line of length
        // zero — should return `""`, not signal EOF.
        let mut cur = Cursor::new(b"\n".to_vec());
        assert_eq!(read_input_line(&mut cur).unwrap(), "");
    }

    #[test]
    fn read_line_io_error_is_e0060() {
        // A `Read` impl that always fails should produce E0060, not
        // E0001 / E0002.
        struct FailingRead;
        impl Read for FailingRead {
            fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
                Err(IoError::new(ErrorKind::Other, "synthetic"))
            }
        }
        let r = FailingRead;
        let err = read_input_line(&mut r.bufreader()).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0060);
        assert!(err.message.contains("INPUT: failed to read stdin"));
    }

    // Helper to convert a Read into a BufRead for the failing-read test.
    trait ReadExt: Read + Sized {
        fn bufreader(self) -> std::io::BufReader<Self> {
            std::io::BufReader::new(self)
        }
    }
    impl<T: Read> ReadExt for T {}
}