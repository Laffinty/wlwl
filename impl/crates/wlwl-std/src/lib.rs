//! WLWL standard library (v0.3 §15) — Phase 4 first batch.
//!
//! Modules exposed:
//!   - `wlwl:std.io`    — `PRINT`, `INPUT` (§15.1)
//!   - `wlwl:std.fs`    — `READ_FILE`, `WRITE_FILE`, `EXISTS` (§15.3)
//!   - `wlwl:std.json`  — `PARSE`, `STRINGIFY` (§15.3 + E0070/E0071)
//!
//! ## Design boundary
//!
//! `wlwl-std` does **not** depend on `wlwl-eval` (would be a cycle, since
//! `wlwl-eval` calls into us for `IMPORT("wlwl:std.X", …)`). Instead,
//! every standard function operates on `serde_json::Value` — the eval
//! side converts `Value` ↔ `serde_json::Value` at the call boundary.
//!
//! This keeps `wlwl-std` pure-Rust, fast to test in isolation, and
//! trivially reusable from non-eval entry points (e.g. a future
//! `wlwl-repl`).

pub mod io;
pub mod fs;
pub mod json;
pub mod ai;

use std::collections::HashMap;
use wlwl_error::ErrorCode;

/// Common value type used at the std / eval boundary.
///
/// Alias for `serde_json::Value` so we can keep `wlwl-std` free of the
/// `wlwl-eval` dependency (which would create a cycle: `wlwl-eval`
/// imports this crate for `IMPORT("wlwl:std.X", …)`).
pub type StdValue = serde_json::Value;

/// Per-call context passed to every std function. Holds process-level
/// state that doesn't belong to any one call (argv, env vars). Phase 4
/// only carries the basics; Phase 4-batch-3 (std.ai) will add
/// HTTP-client configuration.
#[derive(Debug, Clone, Default)]
pub struct StdCtx {
    pub argv: Vec<String>,
    pub env: HashMap<String, String>,
}

impl StdCtx {
    pub fn from_process() -> Self {
        Self {
            argv: std::env::args().collect(),
            env: std::env::vars().collect(),
        }
    }
}

/// Function signature every std function conforms to. Errors are
/// reported via `StdError` and translated into `WlwlError` on the
/// eval side.
pub type StdFn = fn(&mut StdCtx, Vec<StdValue>) -> Result<StdValue, StdError>;

/// A std module: a stable path + a static list of (name, function)
/// pairs. The list is the contract with the eval side: the IMPORT
/// `names` field is checked against this list, and each requested
/// name is bound as a `Value::NativeFn` wrapping the `StdFn`.
pub struct ModuleSpec {
    pub path: &'static str,
    pub functions: &'static [(&'static str, StdFn)],
}

/// Resolve a `wlwl:std.X` path to its module spec. Returns `None` for
/// anything that doesn't match — the eval side will then surface
/// `E0040 module 'X' not found` (treating the namespace path as a
/// module name).
pub fn resolve(path: &str) -> Option<&'static ModuleSpec> {
    match path {
        "wlwl:std.io" => Some(&io::SPEC),
        "wlwl:std.fs" => Some(&fs::SPEC),
        "wlwl:std.json" => Some(&json::SPEC),
        "wlwl:std.ai" => Some(&ai::SPEC),
        _ => None,
    }
}

/// Error type used at the std / eval boundary. Carries the spec's
/// stable error code + a human message; the eval side wraps this into
/// a `WlwlDiagnostic` with appropriate location info.
#[derive(Debug, Clone)]
pub struct StdError {
    pub code: ErrorCode,
    pub message: String,
}

impl std::fmt::Display for StdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for StdError {}

/// Helper used by every std function: build a `StdError` for an
/// arity mismatch with the standard message format.
pub(crate) fn arity_error(fn_name: &str, got: usize, want: usize) -> StdError {
    StdError {
        code: ErrorCode::E0022,
        message: format!(
            "function expects {} argument(s), got {}",
            want, got
        ),
    }
}

/// Helper for type-mismatch errors (E0030).
pub(crate) fn type_error(fn_name: &str, expected: &str, got: &StdValue) -> StdError {
    StdError {
        code: ErrorCode::E0030,
        message: format!(
            "{}: expected {}, got {}",
            fn_name,
            expected,
            json_type_name(got)
        ),
    }
}

pub(crate) fn json_type_name(v: &StdValue) -> &'static str {
    match v {
        StdValue::Null => "null",
        StdValue::Bool(_) => "boolean",
        StdValue::Number(_) => "number",
        StdValue::String(_) => "string",
        StdValue::Array(_) => "array",
        StdValue::Object(_) => "dict",
    }
}

/// Helper: extract a string argument at position `i`, or return an
/// `E0022` / `E0030` error with the right framing.
pub(crate) fn expect_string<'a>(
    fn_name: &str,
    args: &'a [StdValue],
    i: usize,
    want_arity: usize,
) -> Result<&'a str, StdError> {
    if args.len() != want_arity {
        return Err(arity_error(fn_name, args.len(), want_arity));
    }
    match &args[i] {
        StdValue::String(s) => Ok(s.as_str()),
        other => Err(type_error(fn_name, "string", other)),
    }
}

#[cfg(test)]
mod tests {
    //! P3-009c: surface tests for the public helpers in `wlwl-std`
    //! that were never directly exercised (only reached indirectly
    //! through the per-module SPEC functions). The `resolve` /
    //! `expect_string` / `json_type_name` / `arity_error` /
    //! `type_error` / `StdError` `Display` paths now have explicit
    //! coverage so coverage instrumentation can report on them
    //! without depending on a particular std module's tests.

    use super::*;

    #[test]
    fn std_ctx_default_is_empty() {
        let ctx = StdCtx::default();
        assert!(ctx.argv.is_empty());
        assert!(ctx.env.is_empty());
    }

    #[test]
    fn std_ctx_from_process_sees_argv() {
        // `cargo test` always passes at least the program path as
        // argv[0]. The env snapshot is not asserted because it
        // varies by host.
        let ctx = StdCtx::from_process();
        assert!(!ctx.argv.is_empty());
    }

    // ---- resolve ----

    #[test]
    fn resolve_io() {
        let s = resolve("wlwl:std.io").expect("io resolves");
        assert_eq!(s.path, "wlwl:std.io");
    }
    #[test]
    fn resolve_fs() {
        let s = resolve("wlwl:std.fs").expect("fs resolves");
        assert_eq!(s.path, "wlwl:std.fs");
    }
    #[test]
    fn resolve_json() {
        let s = resolve("wlwl:std.json").expect("json resolves");
        assert_eq!(s.path, "wlwl:std.json");
    }
    #[test]
    fn resolve_ai() {
        let s = resolve("wlwl:std.ai").expect("ai resolves");
        assert_eq!(s.path, "wlwl:std.ai");
    }
    #[test]
    fn resolve_unknown_returns_none() {
        assert!(resolve("wlwl:std.unknown").is_none());
        assert!(resolve("wlwl:std.ioo").is_none());
        assert!(resolve("").is_none());
        assert!(resolve("std.io").is_none()); // missing namespace
    }

    // ---- StdError Display ----

    #[test]
    fn std_error_display_format() {
        let e = StdError {
            code: ErrorCode::E0022,
            message: "function expects 1 argument(s), got 2".into(),
        };
        assert_eq!(e.to_string(), "E0022: function expects 1 argument(s), got 2");
    }

    #[test]
    fn std_error_is_std_error_trait() {
        // Compile-time check that StdError implements std::error::Error.
        fn assert_error<E: std::error::Error>(_: &E) {}
        let e = StdError { code: ErrorCode::E0060, message: "x".into() };
        assert_error(&e);
    }

    // ---- arity_error ----

    #[test]
    fn arity_error_uses_e0022() {
        let e = arity_error("F", 3, 1);
        assert_eq!(e.code, ErrorCode::E0022);
        assert_eq!(e.message, "function expects 1 argument(s), got 3");
    }

    // ---- type_error ----

    #[test]
    fn type_error_uses_e0030() {
        let got = StdValue::Number(serde_json::Number::from(1));
        let e = type_error("F", "string", &got);
        assert_eq!(e.code, ErrorCode::E0030);
        assert_eq!(e.message, "F: expected string, got number");
    }

    // ---- json_type_name ----

    #[test]
    fn json_type_name_all_variants() {
        assert_eq!(json_type_name(&StdValue::Null), "null");
        assert_eq!(json_type_name(&StdValue::Bool(true)), "boolean");
        assert_eq!(json_type_name(&StdValue::Number(serde_json::Number::from(1))), "number");
        assert_eq!(json_type_name(&StdValue::String("s".into())), "string");
        assert_eq!(json_type_name(&StdValue::Array(vec![])), "array");
        let mut m = serde_json::Map::new();
        m.insert("k".into(), StdValue::from(1));
        assert_eq!(json_type_name(&StdValue::Object(m)), "dict");
    }

    // ---- expect_string ----

    #[test]
    fn expect_string_happy_path() {
        let args = vec![StdValue::String("hi".into())];
        assert_eq!(expect_string("F", &args, 0, 1).unwrap(), "hi");
    }

    #[test]
    fn expect_string_arity_mismatch_is_e0022() {
        let args = vec![StdValue::String("hi".into()), StdValue::Null];
        let err = expect_string("F", &args, 0, 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0022);
    }

    #[test]
    fn expect_string_type_mismatch_is_e0030() {
        let args = vec![StdValue::Number(serde_json::Number::from(1))];
        let err = expect_string("F", &args, 0, 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::E0030);
    }
}