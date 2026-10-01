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
use crate::{ModuleSpec, StdFn};
use wlwl_error::{ErrorCode, WlwlError, WlwlResult};

use wlwl_value::{values_equal, Outcome, StdHost, Value};

// ─────────────────────────────────────────────────────────────────────
// The 17-name table — must match `wlwl_std::collection::NAMES` exactly
// (locked by `names_match_catalog`).
// ─────────────────────────────────────────────────────────────────────

/// Phase B6 (spec §15.7 / §10.5). Walks `BUILTINS` so every entry
/// appears in IMPORT name-resolution.
pub const NAMES: &[&str] = &[
    "MAP",
    "FILTER",
    "REDUCE",
    "SORT",
    "SORT_BY",
    "ZIP",
    "RANGE",
    "ANY",
    "ALL",
    "FIND",
    "ENUMERATE",
    "TAKE",
    "DROP",
    "FLAT",
    "UNIQ",
    "GROUP_BY",
    "JOIN",
];

// ─────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────

/// Cheap ERR-transparent precheck: if any arg is `Value::Err(_)`,
/// short-circuit the collection with that ERR. The §12.6 contract
/// applies to all 17 functions identically, so we factor it here
/// instead of repeating at each entry point.
pub fn short_circuit_err(args: &[Value]) -> Option<Value> {
    args.iter().find_map(|v| match v {
        Value::Err(_) => Some(v.clone()),
        _ => None,
    })
}

/// Construct an `arity mismatch` diagnostic with the spec-faithful
/// message format (`"MAP: function expects 2 argument(s), got 1"`).
fn arity(host: &mut dyn StdHost, fn_name: &str, got: usize, want: usize) -> WlwlError {
    host.diag(
        ErrorCode::E0022,
        format!("{fn_name}: function expects {want} argument(s), got {got}"),
    )
}

/// Construct a type-mismatch diagnostic (E0030). The `expected`
/// string is the spec name (e.g. `"array"`, `"callable"`).
fn type_err(host: &mut dyn StdHost, fn_name: &str, expected: &str, got: &Value) -> WlwlError {
    host.diag(
        ErrorCode::E0030,
        format!(
            "{}: expected {}, got {}",
            fn_name,
            expected,
            value_kind(got),
        ),
    )
}

/// Construct an E0020 ("not a function") diagnostic. Used when a
/// callback arg isn't callable (Closure / NativeFn / Class).
fn not_callable(host: &mut dyn StdHost, fn_name: &str, got: &Value) -> WlwlError {
    host.diag(
        ErrorCode::E0020,
        format!(
            "{}: callback is not callable (got {})",
            fn_name,
            value_kind(got)
        ),
    )
}

/// Short, spec-style name for a `Value`'s runtime kind. Used in
/// `E0030` / `E0020` diagnostics so users see the same vocabulary
/// they would get from `TYPE(x)`. `pub(crate)` so other eval
/// modules (e.g. `wlwl_eval::test`) can reuse it for their
/// own E0030 / type messages without duplicating the vocabulary.
pub fn value_kind(v: &Value) -> &'static str {
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
        // [v0.7 Phase C2] TaskHandle shows up when E0030 / E0020
        // diagnostics mention a value that the user produced via
        // SPAWN. Use the spec-v0.7 vocabulary ("task handle") so
        // the message reads naturally next to `SPAWN(...)` and
        // `AWAIT(...)` rather than "RESULT ok" or similar.
        Value::TaskHandle(_) => "task handle",
        // [v0.7 Phase D-A] same shape for channel handles; the
        // vocabulary ("channel handle") matches the user-facing
        // names CHANNEL_NEW / CHANNEL_SEND / CHANNEL_RECV etc.
        Value::ChannelHandle(_) => "channel handle",
        // [v0.9 Step 9a-1 / plan §4.3] OOP values for the
        // E0030-vocabulary (kept distinct from "function closure"
        // / "native fn" so misuse diagnostics name the actual
        // shape the user gave).
        Value::Class(_) => "class",
        Value::Instance { .. } => "instance",
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
    host: &mut dyn StdHost,
    fn_name: &str,
    callable: &Value,
    args: Vec<Value>,
) -> WlwlResult<Value> {
    // [v0.11 M2 / ADR-0022] 挂起/诊断语义由宿主原样上抛;与旧实现
    // 一致,这里只取 outcome.value(signal 由更上层消费)。
    let outcome = host.call(callable, args, fn_name)?;
    Ok(outcome.value)
}

/// Materialise the comparator closure. `None` means "use the spec
/// default `<`" (per §10.5 row 4).
pub fn default_less(a: &Value, b: &Value) -> bool {
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

pub fn builtin_map(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity(host, "MAP", args.len(), 2));
    }
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(host, "MAP", "array", other)),
    };
    if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(not_callable(host, "MAP", &args[1]));
    }
    let f = args[1].clone();
    let mut out = Vec::with_capacity(arr.len());
    for v in arr {
        let r = call_callable(host, "MAP", &f, vec![v])?;
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

pub fn builtin_filter(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity(host, "FILTER", args.len(), 2));
    }
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(host, "FILTER", "array", other)),
    };
    if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(not_callable(host, "FILTER", &args[1]));
    }
    let f = args[1].clone();
    let mut out = Vec::new();
    for v in arr {
        let r = call_callable(host, "FILTER", &f, vec![v.clone()])?;
        if let Value::Err(_) = &r {
            return Ok(Outcome::normal(r));
        }
        let keep = match r {
            Value::Boolean(b) => b,
            other => {
                return Err(host.diag(
                    ErrorCode::E0030,
                    format!(
                        "FILTER: predicate must return BOOLEAN, got {}",
                        value_kind(&other),
                    ),
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

pub fn builtin_reduce(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 3 {
        return Err(arity(host, "REDUCE", args.len(), 3));
    }
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(host, "REDUCE", "array", other)),
    };
    if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(not_callable(host, "REDUCE", &args[1]));
    }
    let f = args[1].clone();
    let mut acc = args[2].clone();
    for v in arr {
        acc = call_callable(host, "REDUCE", &f, vec![acc, v])?;
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

pub fn builtin_sort(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if !(1..=2).contains(&args.len()) {
        return Err(arity(host, "SORT", args.len(), 2));
    }
    let mut arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(host, "SORT", "array", other)),
    };
    let cmp = if args.len() == 2 {
        if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
            return Err(not_callable(host, "SORT", &args[1]));
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
            Some(f) => match compare_with(host, "SORT", f, a, b, &pending) {
                Some(o) => o,
                None => std::cmp::Ordering::Equal, // pending already set
            },
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
    host: &mut dyn StdHost,
    fn_name: &str,
    f: &Value,
    a: &Value,
    b: &Value,

    pending: &std::cell::RefCell<Option<WlwlResult<Outcome>>>,
) -> Option<std::cmp::Ordering> {
    let lt = match call_callable(host, fn_name, f, vec![a.clone(), b.clone()]) {
        Ok(Value::Boolean(b)) => b,
        Ok(Value::Err(e)) => {
            *pending.borrow_mut() = Some(Ok(Outcome::normal(*e)));
            return None;
        }
        Ok(other) => {
            *pending.borrow_mut() = Some(Err(host.diag(
                ErrorCode::E0030,
                format!(
                    "{}: comparator must return BOOLEAN, got {}",
                    fn_name,
                    value_kind(&other),
                ),
            )));
            return None;
        }
        Err(e) => {
            *pending.borrow_mut() = Some(Err(e));
            return None;
        }
    };
    let gt = match call_callable(host, fn_name, f, vec![b.clone(), a.clone()]) {
        Ok(Value::Boolean(b)) => b,
        Ok(Value::Err(e)) => {
            *pending.borrow_mut() = Some(Ok(Outcome::normal(*e)));
            return None;
        }
        Ok(other) => {
            *pending.borrow_mut() = Some(Err(host.diag(
                ErrorCode::E0030,
                format!(
                    "{}: comparator must return BOOLEAN, got {}",
                    fn_name,
                    value_kind(&other),
                ),
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

pub fn builtin_sort_by(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity(host, "SORT_BY", args.len(), 2));
    }
    let mut arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(host, "SORT_BY", "array", other)),
    };
    if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(not_callable(host, "SORT_BY", &args[1]));
    }
    let key = args[1].clone();
    // Project once, sort by projected key. Holds onto the original
    // value so the final result re-emits the input order's elements
    // (not the projected keys).
    let mut decorated: Vec<(Value, Value)> = Vec::with_capacity(arr.len());
    for v in arr.drain(..) {
        let k = call_callable(host, "SORT_BY", &key, vec![v.clone()])?;
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

pub fn builtin_zip(_host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.is_empty() {
        return Err(arity(_host, "ZIP", 0, 1));
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

pub fn builtin_range(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if !(1..=3).contains(&args.len()) {
        return Err(arity(host, "RANGE", args.len(), 3));
    }
    let to_i64 = |host: &mut dyn StdHost, name: &str, v: &Value| -> WlwlResult<i64> {
        match v {
            Value::Integer(i) => Ok(*i),
            other => Err(type_err(host, name, "integer", other)),
        }
    };
    let (start, end, step) = match args.len() {
        1 => {
            let n = to_i64(host, "RANGE", &args[0])?;
            (0, n, 1i64)
        }
        // Phase I1 (spec §10.5): `RANGE(start, end)` — step defaults to
        // 1. This arm was previously missing, so the 2-arg form fell
        // into `unreachable!()` and panicked (exit 101).
        2 => {
            let s = to_i64(host, "RANGE", &args[0])?;
            let e = to_i64(host, "RANGE", &args[1])?;
            (s, e, 1i64)
        }
        3 => {
            let s = to_i64(host, "RANGE", &args[0])?;
            let e = to_i64(host, "RANGE", &args[1])?;
            let st = to_i64(host, "RANGE", &args[2])?;
            (s, e, st)
        }
        _ => unreachable!("arity-checked 1..=3"),
    };
    if step == 0 {
        return Err(host.diag(
            ErrorCode::E0038,
            "RANGE: step must be non-zero (got 0)".to_string(),
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

pub fn builtin_any(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if !(1..=2).contains(&args.len()) {
        return Err(arity(host, "ANY", args.len(), 2));
    }
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(host, "ANY", "array", other)),
    };
    let f = if args.len() == 2 {
        if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
            return Err(not_callable(host, "ANY", &args[1]));
        }
        Some(args[1].clone())
    } else {
        None
    };
    for v in arr {
        let r = match &f {
            None => v.clone(),
            Some(f) => {
                let x = call_callable(host, "ANY", f, vec![v])?;
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

pub fn builtin_all(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if !(1..=2).contains(&args.len()) {
        return Err(arity(host, "ALL", args.len(), 2));
    }
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(host, "ALL", "array", other)),
    };
    let f = if args.len() == 2 {
        if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
            return Err(not_callable(host, "ALL", &args[1]));
        }
        Some(args[1].clone())
    } else {
        None
    };
    for v in arr {
        let r = match &f {
            None => v.clone(),
            Some(f) => {
                let x = call_callable(host, "ALL", f, vec![v])?;
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

pub fn builtin_find(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity(host, "FIND", args.len(), 2));
    }
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(host, "FIND", "array", other)),
    };
    if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(not_callable(host, "FIND", &args[1]));
    }
    let f = args[1].clone();
    for v in arr {
        let r = call_callable(host, "FIND", &f, vec![v.clone()])?;
        if let Value::Err(_) = &r {
            return Ok(Outcome::normal(r));
        }
        let hit = match r {
            Value::Boolean(b) => b,
            other => {
                return Err(host.diag(
                    ErrorCode::E0030,
                    format!(
                        "FIND: predicate must return BOOLEAN, got {}",
                        value_kind(&other),
                    ),
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

pub fn builtin_enumerate(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 1 {
        return Err(arity(host, "ENUMERATE", args.len(), 1));
    }
    // [v0.10.3] 与同文件的 MAP / FILTER 等统一:type_err 要 span,
    // 所以这里取一次(eval_call 在派发前设过 current_span)。
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(host, "ENUMERATE", "array", other)),
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

pub fn builtin_take(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity(host, "TAKE", args.len(), 2));
    }
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(host, "TAKE", "array", other)),
    };
    let n = match &args[1] {
        Value::Integer(i) if *i >= 0 => *i as usize,
        Value::Integer(_) => return Ok(Outcome::normal(Value::Array(vec![]))), // n<0 → []
        other => return Err(type_err(host, "TAKE", "non-negative integer", other)),
    };
    let out: Vec<Value> = arr.into_iter().take(n).collect();
    Ok(Outcome::normal(Value::Array(out)))
}

// ─────────────────────────────────────────────────────────────────────
// 13. DROP(arr, n)  →  arr minus the first n elements
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_drop(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity(host, "DROP", args.len(), 2));
    }
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(host, "DROP", "array", other)),
    };
    let n = match &args[1] {
        Value::Integer(i) if *i >= 0 => *i as usize,
        Value::Integer(_) => return Ok(Outcome::normal(Value::Array(arr))), // n<0 → no-op
        other => return Err(type_err(host, "DROP", "non-negative integer", other)),
    };
    let out: Vec<Value> = arr.into_iter().skip(n).collect();
    Ok(Outcome::normal(Value::Array(out)))
}

// ─────────────────────────────────────────────────────────────────────
// 14. FLAT(arr)  →  flatten one level
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_flat(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 1 {
        return Err(arity(host, "FLAT", args.len(), 1));
    }
    // [v0.10.3] 与同文件的 MAP / FILTER 等统一:type_err 要 span,
    // 所以这里取一次(eval_call 在派发前设过 current_span)。
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(host, "FLAT", "array", other)),
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

pub fn builtin_uniq(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 1 {
        return Err(arity(host, "UNIQ", args.len(), 1));
    }
    // [v0.10.3] 与同文件的 MAP / FILTER 等统一:type_err 要 span,
    // 所以这里取一次(eval_call 在派发前设过 current_span)。
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(host, "UNIQ", "array", other)),
    };
    let mut seen: Vec<Value> = Vec::new();
    for v in arr {
        if !seen.iter().any(|s| values_equal(s, &v)) {
            seen.push(v);
        }
    }
    Ok(Outcome::normal(Value::Array(seen)))
}

// ─────────────────────────────────────────────────────────────────────
// 16. GROUP_BY(arr, key)  →  DICT keyed by key(v)
// ─────────────────────────────────────────────────────────────────────

pub fn builtin_group_by(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity(host, "GROUP_BY", args.len(), 2));
    }
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(host, "GROUP_BY", "array", other)),
    };
    if !matches!(args[1], Value::Closure { .. } | Value::NativeFn { .. }) {
        return Err(not_callable(host, "GROUP_BY", &args[1]));
    }
    let f = args[1].clone();
    // DICT keys must be STRING per §10.4 / §10.2 / wlwl-eval value model.
    // If the key fn returns a non-string, coerce via STR semantics
    // (mirrors FORMAT / PRINT's "non-string → STR" convention).
    let mut buckets: Vec<(Value, Value)> = Vec::new();
    for v in arr {
        let k_raw = call_callable(host, "GROUP_BY", &f, vec![v.clone()])?;
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

pub fn builtin_join(host: &mut dyn StdHost, args: Vec<Value>) -> WlwlResult<Outcome> {
    if let Some(e) = short_circuit_err(&args) {
        return Ok(Outcome::normal(e));
    }
    if args.len() != 2 {
        return Err(arity(host, "JOIN", args.len(), 2));
    }
    let arr = match &args[0] {
        Value::Array(items) => items.clone(),
        other => return Err(type_err(host, "JOIN", "array", other)),
    };
    let sep = match &args[1] {
        Value::String(s) => s.clone(),
        other => return Err(type_err(host, "JOIN", "string (separator)", other)),
    };
    let parts: Vec<String> = arr.iter().map(|v| v.display()).collect();
    Ok(Outcome::normal(Value::String(parts.join(&sep))))
}

// ─────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────

pub static SPEC: ModuleSpec = ModuleSpec {
    path: "wlwl:std.collection",
    functions: &[
        ("MAP", builtin_map as StdFn),
        ("FILTER", builtin_filter as StdFn),
        ("REDUCE", builtin_reduce as StdFn),
        ("SORT", builtin_sort as StdFn),
        ("SORT_BY", builtin_sort_by as StdFn),
        ("ZIP", builtin_zip as StdFn),
        ("RANGE", builtin_range as StdFn),
        ("ANY", builtin_any as StdFn),
        ("ALL", builtin_all as StdFn),
        ("FIND", builtin_find as StdFn),
        ("ENUMERATE", builtin_enumerate as StdFn),
        ("TAKE", builtin_take as StdFn),
        ("DROP", builtin_drop as StdFn),
        ("FLAT", builtin_flat as StdFn),
        ("UNIQ", builtin_uniq as StdFn),
        ("GROUP_BY", builtin_group_by as StdFn),
        ("JOIN", builtin_join as StdFn),
    ],
};
