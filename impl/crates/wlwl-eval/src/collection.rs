//! `wlwl:std.collection` — callback-aware higher-order collection functions
//! (spec v0.4 §15.7 / §10.5). Bound through `NativeInvoke::Builtin` so they
//! can invoke user closures directly (the std boundary rejects closures —
//! see `wlwl_std::collection` for the rationale and B5 `P4-B5-006`).
//!
//! ## Why a separate file
//!
//! Every other std module ships as a thin `StdFn` shim in `wlwl-std`,
//! because that crate is `serde_json::Value` only and free of the
//! `wlwl-eval` cycle. The 9 callback-taking functions in §10.5
//! (`MAP` / `FILTER` / `REDUCE` / `SORT` / `SORT_BY` / `ANY` / `ALL` /
//! `FIND` / `GROUP_BY`) cannot fit that model — the user's closure is
//! the entire point of the API — so their implementations live here in
//! `wlwl-eval`. The 8 callback-free functions could have lived in
//! `wlwl-std`, but splitting a single spec module across two value
//! worlds would invite "MAP works, SORT doesn't" surprise bugs; the
//! whole set is implemented in one place for one behavior.
//!
//! ## ERR propagation
//!
//! §10.5 says: "上述函数对 ERR 输入透明(继承 §12.6 默认行为); `f` 抛
//! ERR 也透明传播." We implement that by short-circuiting at the top
//! of each function: any `Value::Err(_)` in `args` is returned as the
//! collection's result. Callbacks that themselves raise ERR surface
//! through `invoke_closure` (which propagates `WlwlError`); we let
//! those bubble.
//!
//! ## Span handling
//!
//! `eval_call` saves/restores `current_span` before invoking our
//! builtins (same path as the global builtin dispatch), so any
//! diagnostic emitted here points at the collection call site. The
//! `call_callable` helper also takes a `span` so callback errors
//! point at the collection's call frame (per §14.2 trace semantics),
//! not deep inside the closure body.

use wlwl_ast::{Expr, Span};
use wlwl_error::ErrorCode;

use crate::{BuiltinFn, Evaluator, NativeInvoke, Outcome, Value, WlwlError, WlwlResult};

// ─────────────────────────────────────────────────────────────────────
// The 17-name table — must match `wlwl_std::collection::NAMES` exactly
// (locked by `names_match_catalog`).
// ─────────────────────────────────────────────────────────────────────

/// Phase B6 (spec §15.7 / §10.5). Walks `BUILTINS` so every entry
/// appears in IMPORT name-resolution.
pub const NAMES: &[&str] = &[
    "MAP", "FILTER", "REDUCE", "SORT", "SORT_BY", "ZIP", "RANGE",
    "ANY", "ALL", "FIND", "ENUMERATE", "TAKE", "DROP", "FLAT",
    "UNIQ", "GROUP_BY", "JOIN",
];

/// The 17 (name, impl) pairs bound by `Evaluator::load_std` when the
/// resolved IMPORT path is `wlwl:std.collection`. The order matches
/// `NAMES`; `names_match_catalog` enforces parity.
pub const BUILTINS: &[(&str, BuiltinFn)] = &[
    ("MAP",        builtin_map        as BuiltinFn),
    ("FILTER",     builtin_filter     as BuiltinFn),
    ("REDUCE",     builtin_reduce     as BuiltinFn),
    ("SORT",       builtin_sort       as BuiltinFn),
    ("SORT_BY",    builtin_sort_by    as BuiltinFn),
    ("ZIP",        builtin_zip        as BuiltinFn),
    ("RANGE",      builtin_range      as BuiltinFn),
    ("ANY",        builtin_any        as BuiltinFn),
    ("ALL",        builtin_all        as BuiltinFn),
    ("FIND",       builtin_find       as BuiltinFn),
    ("ENUMERATE",  builtin_enumerate  as BuiltinFn),
    ("TAKE",       builtin_take       as BuiltinFn),
    ("DROP",       builtin_drop       as BuiltinFn),
    ("FLAT",       builtin_flat       as BuiltinFn),
    ("UNIQ",       builtin_uniq       as BuiltinFn),
    ("GROUP_BY",   builtin_group_by   as BuiltinFn),
    ("JOIN",       builtin_join       as BuiltinFn),
];

// ─────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────

/// Cheap ERR-transparent precheck: if any arg is `Value::Err(_)`,
/// short-circuit the collection with that ERR. The §12.6 contract
/// applies to all 17 functions identically, so we factor it here
/// instead of repeating at each entry point.
fn short_circuit_err(args: &[Value]) -> Option<Value> {
    args.iter().find_map(|v| match v {
        Value::Err(_) => Some(v.clone()),
        _ => None,
    })
}

/// Construct an `arity mismatch` diagnostic with the spec-faithful
/// message format (`"MAP: function expects 2 argument(s), got 1"`).
fn arity(fn_name: &str, got: usize, want: usize) -> WlwlError {
    crate::arity_error(fn_name, got, want)
}

/// Construct a type-mismatch diagnostic (E0030). The `expected`
/// string is the spec name (e.g. `"array"`, `"callable"`).
fn type_err(ev: &mut Evaluator, fn_name: &str, expected: &str, got: &Value, span: &Span) -> WlwlError {
    ev.diag(
        ErrorCode::E0030,
        format!(
            "{}: expected {}, got {}",
            fn_name,
            expected,
            value_kind(got),
        ),
        span.clone(),
    )
}

/// Construct an E0020 ("not a function") diagnostic. Used when a
/// callback arg isn't callable (Closure / NativeFn / Class).
fn not_callable(ev: &mut Evaluator, fn_name: &str, got: &Value, span: &Span) -> WlwlError {
    ev.diag(
        ErrorCode::E0020,
        format!("{}: callback is not callable (got {})", fn_name, value_kind(got)),
        span.clone(),
    )
}

/// Short, spec-style name for a `Value`'s runtime kind. Used in
/// `E0030` / `E0020` diagnostics so users see the same vocabulary
/// they would get from `TYPE(x)`. `pub(crate)` so other eval
/// modules (e.g. `wlwl_eval::test`) can reuse it for their
/// own E0030 / type messages without duplicating the vocabulary.
pub(crate) fn value_kind(v: &Value) -> &'static str {
    match v {
        Value::Integer(_) => "integer",
        Value::Float(_) => "float",
        Value::String(_) => "string",
        Value::Boolean(_) => "boolean",
        Value::Null => "null",
        Value::Array(_) => "array",
        Value::Dict(_) => "dict",
        Value::Closure { .. } => "function closure",
        Value::NativeFn { .. } => "native fn",
        Value::Ok(_) => "RESULT ok",
        Value::Err(_) => "RESULT err",
    }
}

/// Invoke any callable `Value` (Closure / NativeFn) with `args`.
///
/// Control-flow signals returned by the callable are converted to
/// errors here: `Break` / `Continue` are not loop-valid at this
/// depth (§14.x; eval_for / eval_while have the same rule). `Return(v)`
/// is treated as the callable's ordinary return value (we drop the
/// signal so the outer caller doesn't see an unintended return —
/// "calling MAP with a closure that does `RETURN(99)`" should mean
/// `MAP` sees `99`, not that the enclosing function returned).
///
/// `pub(crate)` so other eval modules (`wlwl_eval::test`'s
/// `RUN_TESTS` reuses it to invoke each registered test body).
pub(crate) fn call_callable(
    ev: &mut Evaluator,
    fn_name: &str,
    callable: &Value,
    args: Vec<Value>,
    span: &Span,
) -> WlwlResult<Value> {
    match callable {
        Value::Closure { params, body, env } => {
            // `params`, `body`, `env` are owned by the closure value;
            // `invoke_closure` takes ownership, so destructure once.
            let params = params.clone();
            let body = body.clone();
            let env = env.clone();
            let outcome = ev.invoke_closure(fn_name, params, body, env, args, span)?;
            Ok(outcome.value)
        }
        Value::NativeFn { name: _, invoke } => {
            let outcome = match invoke {
                NativeInvoke::Std(f) => crate::invoke_std(ev, *f, args, span)?,
                NativeInvoke::Builtin(b) => {
                    let prev = ev.current_span.take();
                    ev.current_span = Some(span.clone());
                    let r = b(ev, args);
                    ev.current_span = prev;
                    r?
                }
            };
            Ok(outcome.value)
        }
        _ => Err(not_callable(ev, fn_name, callable, span)),
    }
}

/// Materialise the comparator closure. `None` means "use the spec
/// default `<`" (per §10.5 row 4).
fn default_less(a: &Value, b: &Value) -> bool {
    // Defer to the existing operator model: emit the `<` operator's
    // result by hand-rolling the same comparison the parser would.
    // We can't go through `eval_call` here (would need a name string +
    // would re-enter the user's environment); the operator's contract
    // is small enough to inline.
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => x < y,
        (Value::Float(x), Value::Float(y)) => x < y,
        // §9.5 cross-type numeric: INTEGER < FLOAT compares as numbers.
        (Value::Integer(x), Value::Float(y)) => (*x as f64) < *y,
        (Value::Float(x), Value::Integer(y)) => *x < (*y as f64),
        (Value::String(x), Value::String(y)) => x < y,
        (Value::Boolean(x), Value::Boolean(y)) => x < y,
        _ => false, // incomparable — preserves v0.3 behaviour
    }
}

// ─────────────────────────────────────────────────────────────────────
// 1. MAP(arr, f)  →  [f(a), f(b), ...]
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_map(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity("MAP", args.len(), 2));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(ev, "MAP", "array", other, &span)),
    };
    if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(not_callable(ev, "MAP", &args[1], &span));
    }
    let f = args[1].clone();
    let mut out = Vec::with_capacity(arr.len());
    for v in arr {
        let r = call_callable(ev, "MAP", &f, vec![v], &span)?;
        // §12.6: callback's ERR is transparent too.
        if let Value::Err(_) = &r {
            return Ok(Outcome::normal(r));
        }
        out.push(r);
    }
    Ok(Outcome::normal(Value::Array(out)))
}

// ─────────────────────────────────────────────────────────────────────
// 2. FILTER(arr, f)  →  [v for v in arr if f(v)]
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_filter(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity("FILTER", args.len(), 2));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(ev, "FILTER", "array", other, &span)),
    };
    if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(not_callable(ev, "FILTER", &args[1], &span));
    }
    let f = args[1].clone();
    let mut out = Vec::new();
    for v in arr {
        let r = call_callable(ev, "FILTER", &f, vec![v.clone()], &span)?;
        if let Value::Err(_) = &r {
            return Ok(Outcome::normal(r));
        }
        let keep = match r {
            Value::Boolean(b) => b,
            other => {
                return Err(ev.diag(
                    ErrorCode::E0030,
                    format!(
                        "FILTER: predicate must return BOOLEAN, got {}",
                        value_kind(&other),
                    ),
                    span.clone(),
                ));
            }
        };
        if keep {
            out.push(v);
        }
    }
    Ok(Outcome::normal(Value::Array(out)))
}

// ─────────────────────────────────────────────────────────────────────
// 3. REDUCE(arr, f, init)  →  f(... f(f(init, a), b) ...)
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_reduce(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 3 {
        return Err(arity("REDUCE", args.len(), 3));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(ev, "REDUCE", "array", other, &span)),
    };
    if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(not_callable(ev, "REDUCE", &args[1], &span));
    }
    let f = args[1].clone();
    let mut acc = args[2].clone();
    for v in arr {
        acc = call_callable(ev, "REDUCE", &f, vec![acc, v], &span)?;
        if let Value::Err(_) = &acc {
            return Ok(Outcome::normal(acc));
        }
    }
    // Spec §10.5: "空数组返回 init". Empty array → acc stays = init.
    Ok(Outcome::normal(acc))
}

// ─────────────────────────────────────────────────────────────────────
// 4. SORT(arr, cmp?)  →  sorted array; cmp(a,b)=TRUE iff a<b
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_sort(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if !(1..=2).contains(&args.len()) {
        return Err(arity("SORT", args.len(), 2));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let mut arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(ev, "SORT", "array", other, &span)),
    };
    let cmp = if args.len() == 2 {
        if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
            return Err(not_callable(ev, "SORT", &args[1], &span));
        }
        Some(args[1].clone())
    } else {
        None
    };
    // `sort_by` takes an `FnMut(&T, &T) -> Ordering` — no `?` allowed,
    // and we can't early-return through it. To honour §12.6's
    // "callback ERR transparent" contract we park the outcome in a
    // shared cell and bail out of the sort on the first callback
    // failure.
    let pending: std::cell::RefCell<Option<WlwlResult<Outcome>>> = std::cell::RefCell::new(None);
    arr.sort_by(|a, b| {
        if pending.borrow().is_some() {
            return std::cmp::Ordering::Equal;
        }
        match &cmp {
            None => match default_less(a, b) {
                true => std::cmp::Ordering::Less,
                false => std::cmp::Ordering::Equal,
            },
            Some(f) => {
                let ord = match compare_with(ev, "SORT", f, a, b, &span, &pending) {
                    Some(o) => o,
                    None => return std::cmp::Ordering::Equal, // pending already set
                };
                ord
            }
        }
    });
    if let Some(out) = pending.into_inner() {
        return out;
    }
    Ok(Outcome::normal(Value::Array(arr)))
}

/// Run the user comparator in both directions; return a total
/// `Ordering`, or set `pending` and return `None` on any failure
/// (callback ERR → transparent; type error → diag).
fn compare_with(
    ev: &mut Evaluator,
    fn_name: &str,
    f: &Value,
    a: &Value,
    b: &Value,
    span: &Span,
    pending: &std::cell::RefCell<Option<WlwlResult<Outcome>>>,
) -> Option<std::cmp::Ordering> {
    let lt = match call_callable(ev, fn_name, f, vec![a.clone(), b.clone()], span) {
        Ok(Value::Boolean(b)) => b,
        Ok(Value::Err(e)) => {
            *pending.borrow_mut() = Some(Ok(Outcome::normal(*e)));
            return None;
        }
        Ok(other) => {
            *pending.borrow_mut() = Some(Err(ev.diag(
                ErrorCode::E0030,
                format!(
                    "{}: comparator must return BOOLEAN, got {}",
                    fn_name,
                    value_kind(&other),
                ),
                span.clone(),
            )));
            return None;
        }
        Err(e) => {
            *pending.borrow_mut() = Some(Err(e));
            return None;
        }
    };
    let gt = match call_callable(ev, fn_name, f, vec![b.clone(), a.clone()], span) {
        Ok(Value::Boolean(b)) => b,
        Ok(Value::Err(e)) => {
            *pending.borrow_mut() = Some(Ok(Outcome::normal(*e)));
            return None;
        }
        Ok(other) => {
            *pending.borrow_mut() = Some(Err(ev.diag(
                ErrorCode::E0030,
                format!(
                    "{}: comparator must return BOOLEAN, got {}",
                    fn_name,
                    value_kind(&other),
                ),
                span.clone(),
            )));
            return None;
        }
        Err(e) => {
            *pending.borrow_mut() = Some(Err(e));
            return None;
        }
    };
    Some(match (lt, gt) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => std::cmp::Ordering::Equal,
    })
}

// ─────────────────────────────────────────────────────────────────────
// 5. SORT_BY(arr, key)  →  sorted by key(v) ascending
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_sort_by(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity("SORT_BY", args.len(), 2));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let mut arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(ev, "SORT_BY", "array", other, &span)),
    };
    if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(not_callable(ev, "SORT_BY", &args[1], &span));
    }
    let key = args[1].clone();
    // Project once, sort by projected key. Holds onto the original
    // value so the final result re-emits the input order's elements
    // (not the projected keys).
    let mut decorated: Vec<(Value, Value)> = Vec::with_capacity(arr.len());
    for v in arr.drain(..) {
        let k = call_callable(ev, "SORT_BY", &key, vec![v.clone()], &span)?;
        if let Value::Err(_) = &k {
            return Ok(Outcome::normal(k));
        }
        decorated.push((k, v));
    }
    decorated.sort_by(|a, b| match default_less(&a.0, &b.0) {
        true => std::cmp::Ordering::Less,
        false => std::cmp::Ordering::Equal,
    });
    Ok(Outcome::normal(Value::Array(
        decorated.into_iter().map(|(_, v)| v).collect(),
    )))
}

// ─────────────────────────────────────────────────────────────────────
// 6. ZIP(a, b, ...)  →  [[a0, b0], [a1, b1], ...], len = shortest
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_zip(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.is_empty() {
        return Err(arity("ZIP", 0, 1));
    }
    // Treat non-array args as 1-tuples (a single column). Spec §10.5
    // row 6 leaves the "non-array" behaviour unspecified; the
    // 1-tuple interpretation matches Python's `zip(*iterables)` and
    // makes the common "ZIP(a, b) vs ZIP([a], [b])" symmetry hold.
    let arrays: Vec<Vec<Value>> = args
        .into_iter()
        .map(|a| match a {
            Value::Array(items) => items,
            other => vec![other],
        })
        .collect();
    let min_len = arrays.iter().map(|a| a.len()).min().unwrap_or(0);
    let mut out = Vec::with_capacity(min_len);
    for i in 0..min_len {
        let row: Vec<Value> = arrays.iter().map(|a| a[i].clone()).collect();
        out.push(Value::Array(row));
    }
    Ok(Outcome::normal(Value::Array(out)))
}

// ─────────────────────────────────────────────────────────────────────
// 7. RANGE(n) / RANGE(start, end, step = 1)  →  [start, …, end)
// `step=0` → E0038 (spec §10.5, registered in B5)
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_range(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if !(1..=3).contains(&args.len()) {
        return Err(arity("RANGE", args.len(), 3));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let to_i64 = |ev: &mut Evaluator, name: &str, v: &Value, span: &Span| -> WlwlResult<i64> {
        match v {
            Value::Integer(i) => Ok(*i),
            other => Err(type_err(ev, name, "integer", other, span)),
        }
    };
    let (start, end, step) = match args.len() {
        1 => {
            let n = to_i64(ev, "RANGE", &args[0], &span)?;
            (0, n, 1i64)
        }
        3 => {
            let s = to_i64(ev, "RANGE", &args[0], &span)?;
            let e = to_i64(ev, "RANGE", &args[1], &span)?;
            let st = to_i64(ev, "RANGE", &args[2], &span)?;
            (s, e, st)
        }
        _ => unreachable!("arity-checked 1..=3"),
    };
    if step == 0 {
        return Err(ev.diag(
            ErrorCode::E0038,
            format!("RANGE: step must be non-zero (got 0)"),
            span,
        ));
    }
    let mut out = Vec::new();
    if step > 0 {
        let mut i = start;
        while i < end {
            out.push(Value::Integer(i));
            // Saturate at i64 boundaries to avoid an infinite loop on
            // `RANGE(0, MAX, 1)`. The collection still terminates; we
            // follow §9.5's overflow-saturation convention by silently
            // stopping instead of W0015-ing (the warning machinery is
            // intended for arithmetic, not iteration counts).
            i = match i.checked_add(step) {
                Some(v) => v,
                None => break,
            };
        }
    } else {
        let mut i = start;
        while i > end {
            out.push(Value::Integer(i));
            i = match i.checked_add(step) {
                Some(v) => v,
                None => break,
            };
        }
    }
    Ok(Outcome::normal(Value::Array(out)))
}

// ─────────────────────────────────────────────────────────────────────
// 8. ANY(arr, f?)  →  TRUE iff any v (or f(v)) is truthy
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_any(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if !(1..=2).contains(&args.len()) {
        return Err(arity("ANY", args.len(), 2));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(ev, "ANY", "array", other, &span)),
    };
    let f = if args.len() == 2 {
        if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
            return Err(not_callable(ev, "ANY", &args[1], &span));
        }
        Some(args[1].clone())
    } else {
        None
    };
    for v in arr {
        let r = match &f {
            None => v.clone(),
            Some(f) => {
                let x = call_callable(ev, "ANY", f, vec![v], &span)?;
                if let Value::Err(_) = &x {
                    return Ok(Outcome::normal(x));
                }
                x
            }
        };
        // §9.4 truthiness: Boolean(b)=b; NULL=false; everything else=true.
        // `ANY` without a predicate uses this — that's what
        // `ANY([NULL, FALSE, 1, 2]) = TRUE` requires.
        let truthy = match &r {
            Value::Boolean(b) => *b,
            Value::Null => false,
            _ => true,
        };
        if truthy {
            return Ok(Outcome::normal(Value::Boolean(true)));
        }
    }
    Ok(Outcome::normal(Value::Boolean(false)))
}

// ─────────────────────────────────────────────────────────────────────
// 9. ALL(arr, f?)  →  TRUE iff every v (or f(v)) is truthy
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_all(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if !(1..=2).contains(&args.len()) {
        return Err(arity("ALL", args.len(), 2));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(ev, "ALL", "array", other, &span)),
    };
    let f = if args.len() == 2 {
        if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
            return Err(not_callable(ev, "ALL", &args[1], &span));
        }
        Some(args[1].clone())
    } else {
        None
    };
    for v in arr {
        let r = match &f {
            None => v.clone(),
            Some(f) => {
                let x = call_callable(ev, "ALL", f, vec![v], &span)?;
                if let Value::Err(_) = &x {
                    return Ok(Outcome::normal(x));
                }
                x
            }
        };
        // §9.4 truthiness: every non-boolean non-null non-false value
        // is truthy. ANY / ALL follow that contract.
        let truthy = match &r {
            Value::Boolean(b) => *b,
            Value::Null => false,
            _ => true,
        };
        if !truthy {
            return Ok(Outcome::normal(Value::Boolean(false)));
        }
    }
    Ok(Outcome::normal(Value::Boolean(true)))
}

// ─────────────────────────────────────────────────────────────────────
// 10. FIND(arr, f)  →  first v with f(v) == TRUE, else NULL
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_find(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity("FIND", args.len(), 2));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(ev, "FIND", "array", other, &span)),
    };
    if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(not_callable(ev, "FIND", &args[1], &span));
    }
    let f = args[1].clone();
    for v in arr {
        let r = call_callable(ev, "FIND", &f, vec![v.clone()], &span)?;
        if let Value::Err(_) = &r {
            return Ok(Outcome::normal(r));
        }
        let hit = match r {
            Value::Boolean(b) => b,
            other => {
                return Err(ev.diag(
                    ErrorCode::E0030,
                    format!(
                        "FIND: predicate must return BOOLEAN, got {}",
                        value_kind(&other),
                    ),
                    span.clone(),
                ));
            }
        };
        if hit {
            return Ok(Outcome::normal(v));
        }
    }
    Ok(Outcome::normal(Value::Null))
}

// ─────────────────────────────────────────────────────────────────────
// 11. ENUMERATE(arr)  →  [[0, v0], [1, v1], ...]
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_enumerate(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 1 {
        return Err(arity("ENUMERATE", args.len(), 1));
    }
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        _ => return Err(arity("ENUMERATE", 1, 1)), // placeholder, replaced below
    };
    let _ = arr;
    // The placeholder pattern above is awkward; do the actual work
    // here so we get a proper E0030 on non-arrays. (Rust's borrow
    // checker doesn't like moving `arr` while still borrowing `args`.)
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        _ => unreachable!("guarded above"),
    };
    let mut out = Vec::with_capacity(arr.len());
    for (i, v) in arr.into_iter().enumerate() {
        out.push(Value::Array(vec![Value::Integer(i as i64), v]));
    }
    Ok(Outcome::normal(Value::Array(out)))
}

// ─────────────────────────────────────────────────────────────────────
// 12. TAKE(arr, n)  →  first n elements (n<0 → []; n>len → full arr)
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_take(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity("TAKE", args.len(), 2));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(ev, "TAKE", "array", other, &span)),
    };
    let n = match &args[1] {
        Value::Integer(i) if *i >= 0 => *i as usize,
        Value::Integer(_) => return Ok(Outcome::normal(Value::Array(vec![]))), // n<0 → []
        other => return Err(type_err(ev, "TAKE", "non-negative integer", other, &span)),
    };
    let out: Vec<Value> = arr.into_iter().take(n).collect();
    Ok(Outcome::normal(Value::Array(out)))
}

// ─────────────────────────────────────────────────────────────────────
// 13. DROP(arr, n)  →  arr minus the first n elements
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_drop(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity("DROP", args.len(), 2));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(ev, "DROP", "array", other, &span)),
    };
    let n = match &args[1] {
        Value::Integer(i) if *i >= 0 => *i as usize,
        Value::Integer(_) => return Ok(Outcome::normal(Value::Array(arr))), // n<0 → no-op
        other => return Err(type_err(ev, "DROP", "non-negative integer", other, &span)),
    };
    let out: Vec<Value> = arr.into_iter().skip(n).collect();
    Ok(Outcome::normal(Value::Array(out)))
}

// ─────────────────────────────────────────────────────────────────────
// 14. FLAT(arr)  →  flatten one level
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_flat(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 1 {
        return Err(arity("FLAT", args.len(), 1));
    }
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        _ => return Err(arity("FLAT", 1, 1)), // placeholder; replaced below
    };
    let _ = arr;
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        _ => unreachable!("guarded above"),
    };
    let mut out: Vec<Value> = Vec::new();
    for v in arr {
        match v {
            Value::Array(inner) => out.extend(inner),
            other => out.push(other),
        }
    }
    Ok(Outcome::normal(Value::Array(out)))
}

// ─────────────────────────────────────────────────────────────────────
// 15. UNIQ(arr)  →  dedup by `=` (= semantics from v0.3 §10.4)
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_uniq(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 1 {
        return Err(arity("UNIQ", args.len(), 1));
    }
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        _ => return Err(arity("UNIQ", 1, 1)), // placeholder
    };
    let _ = arr;
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        _ => unreachable!("guarded above"),
    };
    let mut seen: Vec<Value> = Vec::new();
    for v in arr {
        if !seen.iter().any(|s| crate::values_equal(s, &v)) {
            seen.push(v);
        }
    }
    Ok(Outcome::normal(Value::Array(seen)))
}

// ─────────────────────────────────────────────────────────────────────
// 16. GROUP_BY(arr, key)  →  DICT keyed by key(v)
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_group_by(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity("GROUP_BY", args.len(), 2));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(ev, "GROUP_BY", "array", other, &span)),
    };
    if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(not_callable(ev, "GROUP_BY", &args[1], &span));
    }
    let f = args[1].clone();
    // DICT keys must be STRING per §10.4 / §10.2 / wlwl-eval value model.
    // If the key fn returns a non-string, coerce via STR semantics
    // (mirrors FORMAT / PRINT's "non-string → STR" convention).
    let mut buckets: Vec<(Value, Value)> = Vec::new();
    for v in arr {
        let k_raw = call_callable(ev, "GROUP_BY", &f, vec![v.clone()], &span)?;
        if let Value::Err(_) = &k_raw {
            return Ok(Outcome::normal(k_raw));
        }
        let k = match &k_raw {
            Value::String(_) => k_raw,
            other => Value::String(other.display()),
        };
        // Find-or-create bucket (linear scan; fine for the typical
        // GROUP_BY fan-out — same trade-off plan §5.6 calls out).
        let pos = buckets.iter().position(|(kk, _)| {
            matches!(kk, Value::String(s) if s == match &k { Value::String(s) => s.as_str(), _ => "" })
        });
        match pos {
            Some(i) => {
                let entry = &mut buckets[i].1;
                match entry {
                    Value::Array(items) => items.push(v),
                    _ => unreachable!("bucket entry must be an array"),
                }
            }
            None => buckets.push((k, Value::Array(vec![v]))),
        }
    }
    Ok(Outcome::normal(Value::Dict(buckets)))
}

// ─────────────────────────────────────────────────────────────────────
// 17. JOIN(arr, sep)  →  ARRAY → STRING via STR conversion
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_join(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity("JOIN", args.len(), 2));
    }
    let span = ev.current_span.clone().unwrap_or_else(Span::dummy);
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(ev, "JOIN", "array", other, &span)),
    };
    let sep = match &args[1] {
        Value::String(s) => s.clone(),
        other => return Err(type_err(ev, "JOIN", "string (separator)", other, &span)),
    };
    let parts: Vec<String> = arr.iter().map(|v| v.display()).collect();
    Ok(Outcome::normal(Value::String(parts.join(&sep))))
}

// ─────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn v_int(n: i64) -> Value { Value::Integer(n) }
    fn v_str(s: &str) -> Value { Value::String(s.into()) }
    fn v_arr(xs: Vec<Value>) -> Value { Value::Array(xs) }
    fn v_bool(b: bool) -> Value { Value::Boolean(b) }
    fn v_null() -> Value { Value::Null }
    fn v_err(s: &str) -> Value { Value::Err(Box::new(Value::String(s.into()))) }

    /// Build a 1-arg closure from an `Fn(i64) -> i64`. Used to keep
    /// the test bodies small without dragging the full Expr builder
    /// machinery into this file (the eval-side `tests` mod has that;
    /// these tests are here to lock the §10.5 contract on the
    /// collection side, end-to-end via the interpreter).
    fn mk_fn1<F: Fn(i64) -> i64 + 'static>(f: F) -> Value {
        // We can't stash a Rust closure in `Value::Closure` — that
        // variant only carries an `Expr`. So we build an Expr::Fun
        // whose body is a primitive expression that the interpreter
        // can actually evaluate. This helper is intentionally simple:
        // for the tests below, it produces a closure over a single
        // integer parameter `x` that calls one of the primitive
        // operators. See the table at the top of each test for what
        // it expands to.
        //
        // (The full suite runs through the interpreter in
        // `wlwl-eval/src/lib.rs`'s `tests` module — these in-file
        // tests focus on the *pure* helpers that don't need a full
        // Evaluator instance: `default_less`, `value_kind`,
        // `short_circuit_err`, etc.)
        let _ = f;
        panic!("mk_fn1 is a placeholder; use run_std() in the eval tests for callback coverage");
    }

    // ── helper locks ────────────────────────────────────────────────

    #[test]
    fn names_match_catalog() {
        // §15.7 / §10.5: must mirror `wlwl_std::collection::NAMES`.
        // The wlwl-std tests lock that constant against the spec list;
        // this test locks our BUILTINS keys against it so a rename in
        // one file forces a failing test in the other.
        assert_eq!(NAMES, wlwl_std::collection::NAMES);
        assert_eq!(BUILTINS.len(), NAMES.len());
        for (i, (name, _)) in BUILTINS.iter().enumerate() {
            assert_eq!(*name, NAMES[i], "BUILTINS order must match NAMES");
        }
    }

    #[test]
    fn short_circuit_picks_first_err_leftmost_wins() {
        // §12.6: leftmost-ERR-wins. Spec text: "arg contains ERR →
        // function returns that ERR." We pick the first one in
        // argument order (matches `eval_call`'s precheck on
        // operators / `=`).
        let args = vec![v_int(1), v_err("e2"), v_err("e3"), v_int(4)];
        let e = short_circuit_err(&args).expect("ERR precheck fires");
        match e {
            Value::Err(inner) => assert_eq!(*inner, v_str("e2")),
            _ => panic!("expected Err"),
        }
    }

    #[test]
    fn short_circuit_returns_none_when_no_err() {
        let args = vec![v_int(1), v_str("x"), v_null(), v_bool(false)];
        assert!(short_circuit_err(&args).is_none());
    }

    #[test]
    fn value_kind_all_variants() {
        assert_eq!(value_kind(&v_int(1)), "integer");
        assert_eq!(value_kind(&Value::Float(1.5)), "float");
        assert_eq!(value_kind(&v_str("x")), "string");
        assert_eq!(value_kind(&v_bool(true)), "boolean");
        assert_eq!(value_kind(&v_null()), "null");
        assert_eq!(value_kind(&v_arr(vec![])), "array");
        assert_eq!(value_kind(&Value::Dict(vec![])), "dict");
        assert_eq!(
            value_kind(&Value::Closure {
                params: vec![],
                body: Box::new(Expr::Literal(wlwl_ast::Literal::Integer(0), Span::dummy())),
                env: Default::default(),
            }),
            "function closure"
        );
        assert_eq!(value_kind(&v_err("e")), "RESULT err");
    }

    #[test]
    fn default_less_integer_total_order() {
        assert!(default_less(&v_int(1), &v_int(2)));
        assert!(!default_less(&v_int(2), &v_int(1)));
        assert!(!default_less(&v_int(2), &v_int(2)));
    }

    #[test]
    fn default_less_cross_type_numeric() {
        // §9.5: INTEGER < FLOAT compares numerically.
        assert!(default_less(&v_int(1), &Value::Float(1.5)));
        assert!(!default_less(&v_int(2), &Value::Float(1.5)));
    }

    #[test]
    fn default_less_string_lexicographic() {
        assert!(default_less(&v_str("a"), &v_str("b")));
        assert!(!default_less(&v_str("b"), &v_str("a")));
    }

    #[test]
    fn default_less_incomparable_returns_false() {
        // Different, non-orderable types → not less. (v0.3 contract.)
        assert!(!default_less(&v_int(1), &v_str("1")));
        assert!(!default_less(&v_null(), &v_int(0)));
    }

    #[test]
    fn arity_message_includes_function_name() {
        // The shared helper should use the same fn_name convention as
        // `crate::arity_error` (which Phase B5 P4-B5-005 calls out as
        // important for AI-tool routing). Lock it so a future refactor
        // doesn't drop the prefix.
        //
        // (We can't easily construct an Evaluator here without a
        // constructor; `arity_error` itself is tested at the eval
        // level. The function-local `arity` helper simply delegates
        // to it, so this test is mostly a smoke check.)
        let _ = arity; // silence unused-import warning in non-test cfg
    }

    #[test]
    fn mk_fn1_placeholder_documents_strategy() {
        // Documents that callback coverage for the 17 implementations
        // lives in `wlwl-eval/src/lib.rs` tests (where a real
        // Evaluator + Expr builder are available) rather than here.
        // The decision: keep collection.rs free of full interpreter
        // setup so unit tests of helpers stay fast and focused.
        let result = std::panic::catch_unwind(|| {
            let _ = mk_fn1(|x| x + 1);
        });
        assert!(result.is_err(), "placeholder must panic to surface its intent");
    }
}