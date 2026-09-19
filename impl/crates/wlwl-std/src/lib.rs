//! WLWL standard library (v0.4 §15) — Phase 4.
//!
//! Modules exposed:
//!   - `wlwl:std.io`     — `PRINT`, `INPUT` (§15.1)
//!   - `wlwl:std.fs`     — `READ_FILE`, `WRITE_FILE`, `EXISTS` (§15.3)
//!   - `wlwl:std.json`   — `PARSE`, `STRINGIFY` (§15.3 + E0070/E0071)
//!   - `wlwl:std.ai`     — ASK / ASK_STREAM stubs (§15.13, Phase 4 batch 3)
//!   - `wlwl:std.format` — `FORMAT` + the shared template grammar (§15.8 / §10.6, Phase B5)
//!   - `wlwl:std.collection` — **name catalog only** for the 17 higher-order
//!     collection functions (§15.7 / §10.5, Phase B6). The real callback-aware
//!     implementations live in `wlwl-eval::collection` because the std
//!     boundary rejects `Value::Closure` (existing contract — see B5 P4-B5-006).
//!   - `wlwl:std.test`   — **name catalog only** for the in-process test
//!     framework (§15.9, Phase B7). Real impls live in
//!     `wlwl-eval::test`; same std-boundary rationale as collection.
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

pub mod agent;
pub mod ai;
pub mod collection;
pub mod format;
pub mod fs;
pub mod io;
pub mod json;
pub mod test;

use std::collections::HashMap;
use wlwl_error::ErrorCode;

/// Common value type used at the std / eval boundary.
///
/// Alias for `serde_json::Value` so we can keep `wlwl-std` free of the
/// `wlwl-eval` dependency (which would create a cycle: `wlwl-eval`
/// imports this crate for `IMPORT("wlwl:std.X", …)`).
pub type StdValue = serde_json::Value;

/// Per-call context passed to every std function. Holds process-level
/// state that doesn't belong to any one call (argv, env vars) plus
/// the Phase D additions: a warnings sink and an optional HTTP
/// client for `wlwl:std.ai` real-mode.
///
/// ## Warnings
///
/// Std functions that produce *warnings* (not errors) push
/// `(ErrorCode, message)` tuples into `warnings`. The eval side
/// drains the sink after the call and emits each entry through the
/// standard `WlwlDiagnostic` channel with `severity = Warning`. Phase
/// D5 uses this to emit `W0052` when an LLM model name lacks the
/// `provider/` prefix.
///
/// ## HTTP client
///
/// The `http_client` field is set up lazily by `ai::ensure_http_client`
/// when `real-ai` is enabled and `WLWL_AI_ENDPOINT` is in env. The
/// default (offline / mock) build leaves it `None`; the mock path in
/// `ai.rs` checks `is_none()` and returns a deterministic payload
/// without touching the network.
#[derive(Debug, Clone, Default)]
pub struct StdCtx {
    pub argv: Vec<String>,
    pub env: HashMap<String, String>,
    /// Phase D5: warnings emitted by std functions (e.g. W0052 for
    /// bare model names). The eval side drains after each call.
    pub warnings: Vec<(wlwl_error::ErrorCode, String)>,
    /// Phase D1: lazily-initialized reqwest blocking client when
    /// `real-ai` feature is enabled. `None` for offline/mock builds.
    #[cfg(feature = "real-ai")]
    pub http_client: Option<std::sync::Arc<reqwest::blocking::Client>>,
}

impl StdCtx {
    pub fn from_process() -> Self {
        Self {
            argv: std::env::args().collect(),
            env: std::env::vars().collect(),
            warnings: Vec::new(),
            #[cfg(feature = "real-ai")]
            http_client: None,
        }
    }

    /// Push a warning. Std functions call this when they want to
    /// produce a non-fatal diagnostic (e.g. W0052).
    pub fn warn(&mut self, code: wlwl_error::ErrorCode, message: impl Into<String>) {
        self.warnings.push((code, message.into()));
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
        "wlwl:std.agent" => Some(&agent::SPEC),
        "wlwl:std.format" => Some(&format::SPEC),
        "wlwl:std.collection" => Some(&collection::SPEC),
        "wlwl:std.test" => Some(&test::SPEC),
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
/// arity mismatch with the standard message format. P3-012:
/// include `fn_name` in the message so callers (and tests) can
/// attribute the error to a specific function — the old format
/// `function expects N argument(s), got M` dropped this info.
pub(crate) fn arity_error(fn_name: &str, got: usize, want: usize) -> StdError {
    StdError {
        code: ErrorCode::E0022,
        message: format!(
            "{fn_name}: function expects {} argument(s), got {}",
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
    fn resolve_agent() {
        let s = resolve("wlwl:std.agent").expect("agent resolves");
        assert_eq!(s.path, "wlwl:std.agent");
        let names: Vec<&str> = s.functions.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["TASK", "TOOL", "CALL_TOOL", "MODEL", "CONTEXT"]);
    }

    #[test]
    fn resolve_format() {
        // Phase B5 (spec v0.4 §15.8): wlwl:std.format exposes FORMAT.
        let s = resolve("wlwl:std.format").expect("format resolves");
        assert_eq!(s.path, "wlwl:std.format");
        let names: Vec<&str> = s.functions.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["FORMAT"]);
    }
    #[test]
    fn resolve_collection() {
        // Phase B6 (spec v0.4 §15.7): wlwl:std.collection is a name catalog
        // — it advertises the 17 higher-order function names (so IMPORT
        // passes the path check) but its `functions` slice is empty,
        // because the real callback-aware implementations live in
        // `wlwl-eval::collection` and are bound to the imported env by
        // `Evaluator::load_std` (which detects this path).
        let s = resolve("wlwl:std.collection").expect("collection resolves");
        assert_eq!(s.path, "wlwl:std.collection");
        assert!(
            s.functions.is_empty(),
            "collection SPEC must be a name catalog (functions empty); \
             actual: {:?}",
            s.functions.iter().map(|(n, _)| *n).collect::<Vec<_>>()
        );
    }
    #[test]
    fn resolve_test() {
        // Phase B7 (spec v0.4 §15.9): wlwl:std.test is also a name
        // catalog — same std-boundary rationale as collection. Real
        // callback-aware impls in `wlwl-eval::test`.
        let s = resolve("wlwl:std.test").expect("test resolves");
        assert_eq!(s.path, "wlwl:std.test");
        assert!(
            s.functions.is_empty(),
            "test SPEC must be a name catalog (functions empty); \
             actual: {:?}",
            s.functions.iter().map(|(n, _)| *n).collect::<Vec<_>>()
        );
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
        assert_eq!(
            e.to_string(),
            "E0022: function expects 1 argument(s), got 2"
        );
    }

    #[test]
    fn std_error_is_std_error_trait() {
        // Compile-time check that StdError implements std::error::Error.
        fn assert_error<E: std::error::Error>(_: &E) {}
        let e = StdError {
            code: ErrorCode::E0060,
            message: "x".into(),
        };
        assert_error(&e);
    }

    // ---- arity_error ----

    #[test]
    fn arity_error_uses_e0022() {
        // P3-012: arity_error now includes the function name so
        // callers can attribute the failure (mirrors type_error's
        // `"FN: expected X, got Y"` format).
        let e = arity_error("F", 3, 1);
        assert_eq!(e.code, ErrorCode::E0022);
        assert_eq!(e.message, "F: function expects 1 argument(s), got 3");
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
        assert_eq!(
            json_type_name(&StdValue::Number(serde_json::Number::from(1))),
            "number"
        );
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
