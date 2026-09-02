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

use std::collections::HashMap;

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