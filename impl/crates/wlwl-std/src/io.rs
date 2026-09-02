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
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let mut line = String::new();
    match stdin.lock().read_line(&mut line) {
        Ok(0) => Ok(StdValue::String(String::new())), // EOF: empty string
        Ok(_) => {
            // strip trailing line terminator(s): \n and \r\n
            while line.ends_with('\n') || line.ends_with('\r') {
                line.pop();
            }
            Ok(StdValue::String(line))
        }
        Err(e) => Err(StdError {
            code: ErrorCode::E0060,
            message: format!("INPUT: failed to read stdin: {}", e),
        }),
    }
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
}