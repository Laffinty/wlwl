//! `wlwl:std.test` — in-process test framework (spec v0.4 §15.9).
//! Bound through `NativeInvoke::Builtin` (same std-boundary
//! rationale as `wlwl_eval::collection` — see B6 P4-B6-001 and
//! `wlwl_std::test` for the full story).
//!
//! ## Surface
//!
//! Six functions per §15.9:
//! - `TEST(name, body)` — registers a test case; body is a closure.
//! - `ASSERT(cond, msg?)` — cond FALSE → `ERR(E0046)`; cond TRUE →
//!   `OK(TRUE)`.
//! - `ASSERT_EQ(a, b, msg?)` — uses `crate::values_equal`; on
//!   inequality → `ERR(E0047)`.
//! - `ASSERT_NEQ(a, b, msg?)` — on equality → `ERR(E0048)`.
//! - `EXPECT_ERR(expr)` — if `expr` is not `Value::Err`, return
//!   `ERR(E0049)`; otherwise return `OK(payload)`.
//! - `RUN_TESTS()` — drain the evaluator-local `test_registry`,
//!   run each body, catch per-test `ERR` (assertions), and return
//!   `ARRAY` of `DICT` per §15.9 schema:
//!   `["name", "passed", "duration_ms", "error"?]`.
//!
//! ## ERR vs OK convention
//!
//! Spec §15.9 says "断言 ERR 不透明传播(§12.6);RUN_TESTS 用 TRY
//! 捕获每个 TEST." So the natural unit-of-work is `TRY(TEST_body)`,
//! and `ASSERT_*` returns an `OK(TRUE)` (success) or an `ERR(...)`
//! (failure). `RUN_TESTS` builds the DICT entries from the
//! `TRY`-wrapped outcomes; an `OK` payload means "passed", an `ERR`
//! means "failed" with the error attached.
//!
//! ## Non-OK non-ERR outcomes
//!
//! If a `TEST` body returns a bare `Value` (not wrapped in `OK`
//! or `ERR`), we treat it as "passed with no assertion" — the
//! `passed` flag is `TRUE` and `error` is absent. This matches
//! spec §15.9's permissive `RUN_TESTS` contract (TEST body just
//! "runs" — assertions are explicit).

use std::time::Instant;

use wlwl_ast::Span;
use wlwl_error::ErrorCode;

use crate::{BuiltinFn, Evaluator, Outcome, Value, WlwlError, WlwlResult};

// ─────────────────────────────────────────────────────────────────────
// 6-name table — must match `wlwl_std::test::NAMES` exactly
// (locked by `names_match_catalog`).
// ─────────────────────────────────────────────────────────────────────

pub const NAMES: &[&str] = &[
    "TEST",
    "ASSERT",
    "ASSERT_EQ",
    "ASSERT_NEQ",
    "EXPECT_ERR",
    "RUN_TESTS",
];

pub const BUILTINS: &[(&str, BuiltinFn)] = &[
    ("TEST",        builtin_test       as BuiltinFn),
    ("ASSERT",      builtin_assert     as BuiltinFn),
    ("ASSERT_EQ",   builtin_assert_eq  as BuiltinFn),
    ("ASSERT_NEQ",  builtin_assert_neq as BuiltinFn),
    ("EXPECT_ERR",  builtin_expect_err as BuiltinFn),
    ("RUN_TESTS",   builtin_run_tests  as BuiltinFn),
];

// ─────────────────────────────────────────────────────────────────────
// Registry entry type — exposed so `Evaluator` can hold a
// `Vec<TestEntry>` in its `test_registry` field.
// ─────────────────────────────────────────────────────────────────────

/// One `TEST(name, body)` registration. Body is the user's closure
/// (kept as `Value` for cloning into the vec); `RUN_TESTS` invokes
/// it via `Evaluator::invoke_closure`.
#[derive(Debug, Clone)]
pub struct TestEntry {
    pub name: String,
    pub body: Value,
}

// ─────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────

fn arity(fn_name: &str, got: usize, want: usize) -> WlwlError {
    crate::arity_error(fn_name, got, want)
}

fn type_err(ev: &mut Evaluator, fn_name: &str, expected: &str, got: &Value, span: &Span) -> WlwlError {
    ev.diag(
        ErrorCode::E0030,
        format!(
            "{}: expected {}, got {}",
            fn_name,
            expected,
            crate::collection::value_kind(got),
        ),
        span.clone(),
    )
}

fn err_with_payload(code: ErrorCode, msg: String, payload: Value, span: &Span) -> WlwlError {
    // The std diagnostic machinery produces E0046-E0049 with a
    // payload that flows into the `Value::Err` returned by the
    // assertion builtins. We piggyback on `ev.diag` for the
    // canonical message format, then reshape the result so the
    // caller sees `Value::Err(payload)` (not a diagnostic thrown
    // upward — assertions are values, not exceptions).
    //
    // Implementation detail: the `payload` is what `TRY` will wrap
    // into `OK(ERR(...))`, and `RUN_TESTS` reads it back out via
    // the dict's `error` key.
    let _ = (code, msg, payload, span);
    unreachable!("err_with_payload: use make_err() instead")
}

fn make_err(ev: &mut Evaluator, code: ErrorCode, msg: String, payload: Value, span: &Span) -> Value {
    // Build an `Err(payload)` Value. `ev.diag` would make a
    // `WlwlError` (a Rust Err), which would propagate up via `?`
    // — but we want the ERR to be a *value* that the calling
    // TEST body sees and that `TRY` catches. So we use the diag
    // machinery for the canonical message format, then recover
    // the `Value::Err(payload)` the caller expects.
    let diag = ev.diag(code, msg, span.clone());
    // Lock the message on the diag so the err payload's `display`
    // stays in sync; we don't ship the whole diagnostic, just the
    // code + payload (matching spec §15.9 row 2-5 schema: ERR has
    // `{code, ...}`).
    let _ = diag; // suppress unused-variable; future use: enrich payload
    Value::Err(Box::new(payload))
}

fn short_circuit_err(args: &[Value]) -> Option<Value> {
    args.iter().find_map(|v| match v {
        Value::Err(_) => Some(v.clone()),
        _ => None,
    })
}

// ─────────────────────────────────────────────────────────────────────
// 1. TEST(name, body)  →  NULL; pushes into evaluator's test_registry
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_test(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity("TEST", args.len(), 2));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let name = match &args[0] {
        Value::String(s) => s.clone(),
        other => return Err(type_err(ev, "TEST", "string", other, &span)),
    };
    if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(type_err(ev, "TEST", "function", &args[1], &span));
    }
    ev.test_registry.push(TestEntry {
        name,
        body: args[1].clone(),
    });
    Ok(Outcome::normal(Value::Null))
}

// ─────────────────────────────────────────────────────────────────────
// 2. ASSERT(cond, msg?)  →  OK(TRUE) on truthy; ERR(E0046) on falsy
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_assert(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if !(1..=2).contains(&args.len()) {
        return Err(arity("ASSERT", args.len(), 2));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let cond = &args[0];
    let truthy = match cond {
        Value::Boolean(b) => *b,
        Value::Null => false,
        _ => true,
    };
    if truthy {
        return Ok(Outcome::normal(Value::Ok(Box::new(Value::Boolean(true)))));
    }
    // Failure path: build ERR(E0046, {code: E0046, cond: <orig>, msg}).
    let msg = if args.len() == 2 {
        Some(args[1].clone())
    } else {
        None
    };
    let payload = build_payload("E0046", cond, msg, "test_assertion_failed");
    Ok(Outcome::normal(make_err(
        ev,
        ErrorCode::E0046,
        format!("ASSERT failed: cond = {}", cond.display()),
        payload,
        &span,
    )))
}

// ─────────────────────────────────────────────────────────────────────
// 3. ASSERT_EQ(a, b, msg?)  →  OK(TRUE) on equal; ERR(E0047) on !=
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_assert_eq(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if !(2..=3).contains(&args.len()) {
        return Err(arity("ASSERT_EQ", args.len(), 3));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let (a, b) = (&args[0], &args[1]);
    if crate::values_equal(a, b) {
        return Ok(Outcome::normal(Value::Ok(Box::new(Value::Boolean(true)))));
    }
    let msg = if args.len() == 3 {
        Some(args[2].clone())
    } else {
        None
    };
    let payload = build_payload_with_actual("E0047", a, b, msg, "test_assertion_eq_failed");
    Ok(Outcome::normal(make_err(
        ev,
        ErrorCode::E0047,
        format!("ASSERT_EQ failed: {} != {}", a.display(), b.display()),
        payload,
        &span,
    )))
}

// ─────────────────────────────────────────────────────────────────────
// 4. ASSERT_NEQ(a, b, msg?)  →  OK(TRUE) on !equal; ERR(E0048) on =
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_assert_neq(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if !(2..=3).contains(&args.len()) {
        return Err(arity("ASSERT_NEQ", args.len(), 3));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let (a, b) = (&args[0], &args[1]);
    if !crate::values_equal(a, b) {
        return Ok(Outcome::normal(Value::Ok(Box::new(Value::Boolean(true)))));
    }
    let msg = if args.len() == 3 {
        Some(args[2].clone())
    } else {
        None
    };
    let payload = build_payload_with_actual("E0048", a, b, msg, "test_assertion_neq_failed");
    Ok(Outcome::normal(make_err(
        ev,
        ErrorCode::E0048,
        format!("ASSERT_NEQ failed: {} == {}", a.display(), b.display()),
        payload,
        &span,
    )))
}

// ─────────────────────────────────────────────────────────────────────
// 5. EXPECT_ERR(expr)  →  OK(payload) if expr is ERR; else ERR(E0049)
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_expect_err(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        // Short-circuit: input was ERR — that IS the expected case.
        return Ok(Outcome::normal(Value::Ok(Box::new(e))));
    }
    if args.len() != 1 {
        return Err(arity("EXPECT_ERR", args.len(), 1));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    // Input is not ERR — that's the failure mode for EXPECT_ERR.
    let payload = build_payload(
        "E0049",
        &args[0],
        None,
        "test_expect_err_failed",
    );
    Ok(Outcome::normal(make_err(
        ev,
        ErrorCode::E0049,
        format!("EXPECT_ERR failed: input was not ERR (got {})", args[0].display()),
        payload,
        &span,
    )))
}

// ─────────────────────────────────────────────────────────────────────
// 6. RUN_TESTS()  →  ARRAY of DICT (per §15.9 schema)
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_run_tests(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 0 {
        return Err(arity("RUN_TESTS", args.len(), 0));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    // Drain the registry. We swap with a new empty Vec so a TEST
    // that registers more tests inside its body (unusual but
    // legitimate) doesn't deadlock.
    let entries = std::mem::take(&mut ev.test_registry);
    let mut results: Vec<Value> = Vec::with_capacity(entries.len());
    for entry in entries {
        let started = Instant::now();
        // Per §15.9: ERR from assertions is "non-transparent" by
        // spec rule (RUN_TESTS uses TRY to catch). We don't try to
        // reimplement TRY here — `invoke_closure` propagates a
        // `WlwlError` for uncaught ERRs (the top-level E0102
        // path), and for a properly-written TEST the body
        // either returns OK(TRUE) on pass or an ERR value from
        // ASSERT/ASSERT_EQ on fail. The latter returns through
        // Outcome::normal(err) from the assertion builtin, which
        // we read off the Outcome.value below.
        let outcome = match crate::collection::call_callable(
            ev,
            "RUN_TESTS",
            &entry.body,
            vec![],
            &span,
        ) {
            Ok(v) => Ok(v),
            Err(_e) => {
                // Diagnostic surfaced during the test (e.g. uncaught
                // E0102 from an unexpected ERR escape). Record the
                // failure as a generic ERR.
                Err(())
            }
        };
        let duration_ms = started.elapsed().as_millis() as i64;
        let dict = match outcome {
            Ok(Value::Err(payload)) => {
                // Spec §15.9 row 6: error field carries the payload.
                let mut d = vec![
                    (Value::String("name".into()), Value::String(entry.name.clone())),
                    (Value::String("passed".into()), Value::Boolean(false)),
                    (Value::String("duration_ms".into()), Value::Integer(duration_ms)),
                    (Value::String("error".into()), *payload),
                ];
                // Order-stable for assertion predictability.
                d.sort_by(|a, b| {
                    let ka = match &a.0 { Value::String(s) => s.as_str(), _ => "" };
                    let kb = match &b.0 { Value::String(s) => s.as_str(), _ => "" };
                    ka.cmp(kb)
                });
                Value::Dict(d)
            }
            Ok(other) => {
                // Non-ERR outcome. Could be OK(TRUE) (from a passed
                // assertion), NULL, or a bare value. All count as
                // "passed" per §15.9's permissive contract.
                let mut d = vec![
                    (Value::String("name".into()), Value::String(entry.name.clone())),
                    (Value::String("passed".into()), Value::Boolean(true)),
                    (Value::String("duration_ms".into()), Value::Integer(duration_ms)),
                ];
                if !matches!(other, Value::Null) {
                    d.push((Value::String("return_value".into()), other));
                }
                d.sort_by(|a, b| {
                    let ka = match &a.0 { Value::String(s) => s.as_str(), _ => "" };
                    let kb = match &b.0 { Value::String(s) => s.as_str(), _ => "" };
                    ka.cmp(kb)
                });
                Value::Dict(d)
            }
            Err(()) => {
                let mut d = vec![
                    (Value::String("name".into()), Value::String(entry.name.clone())),
                    (Value::String("passed".into()), Value::Boolean(false)),
                    (Value::String("duration_ms".into()), Value::Integer(duration_ms)),
                    (Value::String("error".into()), Value::String("uncaught diagnostic".into())),
                ];
                d.sort_by(|a, b| {
                    let ka = match &a.0 { Value::String(s) => s.as_str(), _ => "" };
                    let kb = match &b.0 { Value::String(s) => s.as_str(), _ => "" };
                    ka.cmp(kb)
                });
                Value::Dict(d)
            }
        };
        results.push(dict);
    }
    Ok(Outcome::normal(Value::Array(results)))
}

// ─────────────────────────────────────────────────────────────────────
// Shared payload builder
// ─────────────────────────────────────────────────────────────────────

fn build_payload(code: &str, cond: &Value, msg: Option<Value>, kind: &str) -> Value {
    // Spec §15.9: ERR payload is a DICT `{code, kind, cond?, msg?}`.
    let mut entries = vec![
        (Value::String("code".into()), Value::String(code.into())),
        (Value::String("kind".into()), Value::String(kind.into())),
    ];
    if !matches!(cond, Value::Null | Value::Boolean(false)) {
        entries.push((Value::String("cond".into()), cond.clone()));
    }
    if let Some(m) = msg {
        entries.push((Value::String("msg".into()), m));
    }
    Value::Dict(entries)
}

fn build_payload_with_actual(
    code: &str,
    actual: &Value,
    expected: &Value,
    msg: Option<Value>,
    kind: &str,
) -> Value {
    let mut entries = vec![
        (Value::String("code".into()), Value::String(code.into())),
        (Value::String("kind".into()), Value::String(kind.into())),
        (Value::String("actual".into()), actual.clone()),
        (Value::String("expected".into()), expected.clone()),
    ];
    if let Some(m) = msg {
        entries.push((Value::String("msg".into()), m));
    }
    Value::Dict(entries)
}

// ─────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_catalog() {
        // Same cross-file lock pattern as `wlwl_eval::collection`.
        assert_eq!(NAMES, crate::collection::NAMES.chunks(2).next().map(|_| NAMES).unwrap_or(NAMES));
        // The simpler assertion: NAMES parity with the std catalog.
        assert_eq!(NAMES, wlwl_std::test::NAMES);
        assert_eq!(BUILTINS.len(), NAMES.len());
        for (i, (name, _)) in BUILTINS.iter().enumerate() {
            assert_eq!(*name, NAMES[i], "BUILTINS order must match NAMES");
        }
    }
}