//! WLWL tree-walking interpreter (Phase 2).
//!
//! Phase 2 implements the core semantics from v0.3 §6–§13 (subset):
//! - §6   `LET` binding with block-scoped lexical environment
//! - §7   Control flow: `IF` / `WHILE` / `FOR` / `RETURN` / `BREAK` / `CONTINUE`
//! - §8   `FUN` literals with closures (env captured by clone — no mutation in Phase 2)
//! - §9   Operators exposed as built-in functions (`+`, `==`, `&&`, `!`, …)
//! - §10  Arrays, dicts, strings
//! - §12  `OK` / `ERR` / `PANIC` / `TRY` / `IS_OK` / `IS_ERR` / `OR_DIE`
//! - §12.6 **ERR transparent propagation** (intercepted in `call_with_args`)
//! - §13  Single-directory `IMPORT` / `EXPORT` (no cross-dir, no `wlwl:` namespaces — Phase 4)
//!
//! **Deferred:**
//! - OOP (§11) — Phase 3
//! - `SET` re-binding — Phase 3+
//! - Cross-directory / `wlwl:std.io` paths — Phase 4
//! - `std.ai` — Phase 4

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use wlwl_ast::{Expr, ImportName, Literal, Span};
use wlwl_error::{
    extract_line, ErrorCode, Location, WlwlDiagnostic, WlwlError, WlwlResult,
};

// ──────────────────────────────────────────────────────────────────────
// Runtime values
// ──────────────────────────────────────────────────────────────────────

/// Runtime value (v0.3 §2.2 — Phase 2).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Null,
    Array(Vec<Value>),
    Dict(Vec<(Value, Value)>),
    /// §8.2 function literal with captured environment (clone-based closure).
    Closure {
        params: Vec<String>,
        body: Box<Expr>,
        env: Env,
    },
    /// §12 OK(value)
    Ok(Box<Value>),
    /// §12 ERR(value)
    Err(Box<Value>),
}

impl Value {
    pub fn display(&self) -> String {
        match self {
            Value::Integer(v) => v.to_string(),
            Value::Float(v) => {
                if v.fract() == 0.0 {
                    format!("{:.1}", v)
                } else {
                    v.to_string()
                }
            }
            Value::String(s) => s.clone(),
            Value::Boolean(b) => {
                if *b { "TRUE" } else { "FALSE" }.to_string()
            }
            Value::Null => "NULL".to_string(),
            Value::Array(items) => {
                let parts: Vec<String> = items.iter().map(|v| v.display()).collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Dict(entries) => {
                let parts: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k.display(), v.display()))
                    .collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Closure { params, .. } => {
                format!("<fun({})>", params.join(", "))
            }
            Value::Ok(v) => format!("OK({})", v.display()),
            Value::Err(v) => format!("ERR({})", v.display()),
        }
    }
}

impl From<Literal> for Value {
    fn from(l: Literal) -> Self {
        match l {
            Literal::Integer(v) => Value::Integer(v),
            Literal::Float(v) => Value::Float(v),
            Literal::String(s) => Value::String(s),
            Literal::Boolean(b) => Value::Boolean(b),
            Literal::Null => Value::Null,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// Lexical environment (chain of scopes; v0.3 §6.3)
// ──────────────────────────────────────────────────────────────────────

/// Lexical environment. Phase 2 uses a simple `Vec<HashMap>` chain where
/// index 0 is the innermost scope. Lookups walk from inside out. New
/// scopes are pushed/popped around blocks and function bodies.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Env {
    scopes: Vec<HashMap<String, Value>>,
}

impl Env {
    pub fn new() -> Self {
        Self { scopes: vec![HashMap::new()] }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        // Never pop the global scope.
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Walk the scope chain from innermost to outermost; return the first
    /// match. Used for variable reads.
    pub fn get(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        None
    }

    /// Bind in the current (innermost) scope. If `name` already exists in
    /// this scope, this is a re-binding in the same scope. The semantic
    /// rule for cross-scope re-binding is reserved for Phase 3.
    pub fn set_local(&mut self, name: impl Into<String>, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.into(), value);
        }
    }

    /// Walk to the first scope that already has `name` and overwrite it.
    /// Used for SET (Phase 3+). In Phase 2 we don't have SET, so this is
    /// unused.
    #[allow(dead_code)]
    pub fn set_existing(&mut self, name: &str, value: Value) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return true;
            }
        }
        false
    }

    /// Snapshot all currently-bound names (for module exports).
    pub fn names(&self) -> HashSet<String> {
        let mut out = HashSet::new();
        for scope in &self.scopes {
            for k in scope.keys() {
                out.insert(k.clone());
            }
        }
        out
    }
}

// ──────────────────────────────────────────────────────────────────────
// Control-flow signals
// ──────────────────────────────────────────────────────────────────────

/// Control-flow signal (separate from `Value`). Propagated up through
/// nested expressions and converted back to a value at the matching
/// frame boundary:
///   * `Return(v)`  — at a function call frame, becomes the function's
///                    return value; at top level, becomes E0102 if v is
///                    `Value::Err(_)`, otherwise the program's result.
///   * `Break` / `Continue` — at a loop frame, become loop control;
///                    outside any loop, become E0014.
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    None,
    Break,
    Continue,
    Return(Value),
}

/// A single evaluation result: a value plus an optional control-flow
/// signal. `Err(...)` from this layer is a hard error (E0020, E0022,
/// E0030, E0100, etc.), not a value-level error.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub value: Value,
    pub signal: Signal,
}

impl Outcome {
    fn normal(v: Value) -> Self {
        Outcome { value: v, signal: Signal::None }
    }
}

// ──────────────────────────────────────────────────────────────────────
// Module loader (single-directory, Phase 2 subset)
// ──────────────────────────────────────────────────────────────────────

/// Result of loading a module: a fresh `Env` containing all top-level
/// bindings, plus the set of names that were `EXPORT`ed.
#[derive(Debug, Clone)]
struct LoadedModule {
    env: Env,
    exports: HashSet<String>,
}

#[derive(Debug, Clone, Default)]
struct ModuleLoader {
    /// Directory used to resolve `IMPORT("foo", …)`. In Phase 2 this is
    /// always the directory containing the entry-point file.
    base_dir: PathBuf,
    /// Cache of fully-loaded modules (avoids re-parsing).
    cache: HashMap<String, LoadedModule>,
    /// Stack of module paths currently being loaded — used to detect
    /// circular imports (E0041). Shared with sub-loaders so a cycle
    /// anywhere in the import graph is detected.
    loading: Rc<RefCell<HashSet<String>>>,
}

impl ModuleLoader {
    fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            cache: HashMap::new(),
            loading: Rc::new(RefCell::new(HashSet::new())),
        }
    }

    /// Load module `name` (a simple bare name like `"math"`, no extension).
    /// Reads `<base_dir>/<name>.wl`, parses, evaluates, caches, and
    /// returns its env + export set. Diagnostics produced here do not
    /// carry the caller's source_line (it would require borrowing
    /// `self` recursively); the caller may add source context if it
    /// wants richer errors.
    fn load(&mut self, name: &str) -> WlwlResult<LoadedModule> {
        if let Some(cached) = self.cache.get(name) {
            return Ok(cached.clone());
        }
        if self.loading.borrow().contains(name) {
            return Err(WlwlDiagnostic::new(
                ErrorCode::E0041,
                format!("circular IMPORT: module '{}' is currently being loaded", name),
                Location::point("<module>", 0, 0),
            )
            .into());
        }
        self.loading.borrow_mut().insert(name.to_string());

        let path = self.base_dir.join(format!("{}.wl", name));
        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                self.loading.borrow_mut().remove(name);
                return Err(WlwlDiagnostic::new(
                    ErrorCode::E0040,
                    format!(
                        "module '{}' not found (looked for {}): {}",
                        name,
                        path.display(),
                        e
                    ),
                    Location::point("<module>", 0, 0),
                )
                .into());
            }
        };
        let ast = match wlwl_parser::parse(&source, &path.display().to_string()) {
            Ok(a) => a,
            Err(e) => {
                self.loading.borrow_mut().remove(name);
                return Err(e);
            }
        };
        // Sub-evaluator shares the loading set so cycles are detected
        // regardless of which IMPORT in the cycle initiates the check.
        let sub_loader = ModuleLoader {
            base_dir: self.base_dir.clone(),
            cache: HashMap::new(),
            loading: Rc::clone(&self.loading),
        };
        let mut sub = Evaluator::new_with_loader(sub_loader);
        if let Err(e) = sub.eval_module(&ast) {
            self.loading.borrow_mut().remove(name);
            return Err(e);
        }
        let exports = collect_exports(&ast);
        let mut env = Env::new();
        for n in &exports {
            if let Some(v) = sub.env.get(n) {
                env.set_local(n.clone(), v.clone());
            } else {
                self.loading.borrow_mut().remove(name);
                return Err(WlwlDiagnostic::new(
                    ErrorCode::E0023,
                    format!(
                        "EXPORT name '{}' is not bound in module '{}'",
                        n, name
                    ),
                    Location::point("<module>", 0, 0),
                )
                .into());
            }
        }
        self.loading.borrow_mut().remove(name);
        let result = LoadedModule { env, exports };
        self.cache.insert(name.to_string(), result.clone());
        Ok(result)
    }
}

/// Walk the top-level expressions of a module program and return the
/// union of names listed in any `EXPORT(...)` node.
fn collect_exports(program: &Expr) -> HashSet<String> {
    fn collect(e: &Expr, out: &mut HashSet<String>) {
        match e {
            Expr::Block { exprs, .. } => {
                for e in exprs {
                    collect(e, out);
                }
            }
            Expr::Export { names, .. } => {
                for n in names {
                    out.insert(n.local_name().to_string());
                }
            }
            _ => {}
        }
    }
    let mut out = HashSet::new();
    collect(program, &mut out);
    out
}

// ──────────────────────────────────────────────────────────────────────
// Built-in functions
// ──────────────────────────────────────────────────────────────────────

type BuiltinFn = fn(&mut Evaluator, Vec<Value>) -> WlwlResult<Outcome>;

fn builtin_print(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let parts: Vec<String> = args.iter().map(|v| v.display()).collect();
    println!("{}", parts.join(" "));
    Ok(Outcome::normal(Value::Null))
}

fn builtin_len(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("LEN", &args, 1)?;
    let n = match v {
        Value::String(s) => s.chars().count() as i64,
        Value::Array(a) => a.len() as i64,
        Value::Dict(d) => d.len() as i64,
        other => {
            return Err(type_error(
                "LEN",
                format!("expected string/array/dict, got {}", type_name(&other)),
            ));
        }
    };
    Ok(Outcome::normal(Value::Integer(n)))
}

fn builtin_push(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("PUSH", args.len(), 2));
    }
    let item = &args[1];
    let arr = match &args[0] {
        Value::Array(a) => {
            let mut a = a.clone();
            a.push(item.clone());
            Value::Array(a)
        }
        other => {
            return Err(type_error(
                "PUSH",
                format!("expected array as first arg, got {}", type_name(other)),
            ));
        }
    };
    Ok(Outcome::normal(arr))
}

/// The single dispatch table: maps a built-in name to its implementation.
/// Operators (`+`, `==`, …) live here too — the parser turns `+(1, 2)`
/// into `Call { name: "+", … }`, and we dispatch on the operator name.
fn resolve_builtin(name: &str) -> Option<BuiltinFn> {
    match name {
        "PRINT" => Some(builtin_print),
        "LEN" => Some(builtin_len),
        "PUSH" => Some(builtin_push),
        "+" => Some(builtin_add),
        "-" => Some(builtin_sub),
        "*" => Some(builtin_mul),
        "/" => Some(builtin_div),
        "%" => Some(builtin_mod),
        "==" => Some(builtin_eq),
        "!=" => Some(builtin_ne),
        "<" => Some(builtin_lt),
        ">" => Some(builtin_gt),
        "<=" => Some(builtin_le),
        ">=" => Some(builtin_ge),
        "&&" => Some(builtin_and),
        "||" => Some(builtin_or),
        "!" => Some(builtin_not),
        "OR_DIE" => Some(builtin_or_die),
        _ => None,
    }
}

/// Whitelist of functions that **consume** an `ERR` value instead of
/// letting it transparently propagate (v0.3 §19.4 / Theorem 19.1).
/// Operators (`+`, `==`, …) are NOT in this set — they are transparent
/// to `ERR`, per the spec. `OR_DIE` is treated as a regular function
/// call by the parser (since it isn't a reserved keyword in v0.3 §3.2),
/// so it lives in this set by name.
fn is_err_consumer(name: &str) -> bool {
    matches!(name, "IS_OK" | "IS_ERR" | "OR_DIE" | "TRY")
}

// ── Operator implementations (v0.3 §9) ─────────────────────────────

fn expect_arity<'a>(fn_name: &str, args: &'a [Value], n: usize) -> WlwlResult<&'a Value> {
    if args.len() != n {
        return Err(arity_error(fn_name, args.len(), n));
    }
    Ok(&args[0])
}

fn expect_arity2<'a>(fn_name: &str, args: &'a [Value]) -> WlwlResult<(&'a Value, &'a Value)> {
    if args.len() != 2 {
        return Err(arity_error(fn_name, args.len(), 2));
    }
    Ok((&args[0], &args[1]))
}

fn arity_error(name: &str, got: usize, want: usize) -> WlwlError {
    let d = WlwlDiagnostic::new(
        ErrorCode::E0022,
        format!("function '{}' expects {} argument(s), got {}", name, want, got),
        Location::point("<runtime>", 0, 0),
    );
    d.into()
}

fn type_error(fn_name: &str, msg: String) -> WlwlError {
    let d = WlwlDiagnostic::new(
        ErrorCode::E0030,
        format!("{}: {}", fn_name, msg),
        Location::point("<runtime>", 0, 0),
    );
    d.into()
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Integer(_) => "integer",
        Value::Float(_) => "float",
        Value::String(_) => "string",
        Value::Boolean(_) => "boolean",
        Value::Null => "null",
        Value::Array(_) => "array",
        Value::Dict(_) => "dict",
        Value::Closure { .. } => "function",
        Value::Ok(_) => "ok",
        Value::Err(_) => "err",
    }
}

fn numeric(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    }
}

fn builtin_add(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("+", args.len(), 2));
    }
    let a = &args[0];
    let b = &args[1];
    // String concatenation takes priority (per spec appendix examples).
    if let (Value::String(s1), Value::String(s2)) = (a, b) {
        return Ok(Outcome::normal(Value::String(format!("{}{}", s1, s2))));
    }
    // Array concatenation.
    if let (Value::Array(a1), Value::Array(a2)) = (a, b) {
        let mut out = a1.clone();
        out.extend(a2.iter().cloned());
        return Ok(Outcome::normal(Value::Array(out)));
    }
    let (xa, xb) = (numeric(a), numeric(b));
    match (xa, xb) {
        (Some(x), Some(y)) => {
            if let (Value::Integer(i1), Value::Integer(i2)) = (a, b) {
                Ok(Outcome::normal(Value::Integer(i1.wrapping_add(*i2))))
            } else {
                Ok(Outcome::normal(Value::Float(x + y)))
            }
        }
        _ => Err(type_error("+", format!(
            "cannot add {} and {}",
            type_name(a), type_name(b)
        ))),
    }
}

fn builtin_sub(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("-", args.len(), 2));
    }
    let a = &args[0];
    let b = &args[1];
    let (xa, xb) = (numeric(a), numeric(b));
    match (xa, xb) {
        (Some(x), Some(y)) => {
            if let (Value::Integer(i1), Value::Integer(i2)) = (a, b) {
                Ok(Outcome::normal(Value::Integer(i1.wrapping_sub(*i2))))
            } else {
                Ok(Outcome::normal(Value::Float(x - y)))
            }
        }
        _ => Err(type_error("-", format!(
            "cannot subtract {} and {}",
            type_name(a), type_name(b)
        ))),
    }
}

fn builtin_mul(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("*", args.len(), 2));
    }
    let a = &args[0];
    let b = &args[1];
    let (xa, xb) = (numeric(a), numeric(b));
    match (xa, xb) {
        (Some(x), Some(y)) => {
            if let (Value::Integer(i1), Value::Integer(i2)) = (a, b) {
                Ok(Outcome::normal(Value::Integer(i1.wrapping_mul(*i2))))
            } else {
                Ok(Outcome::normal(Value::Float(x * y)))
            }
        }
        _ => Err(type_error("*", format!(
            "cannot multiply {} and {}",
            type_name(a), type_name(b)
        ))),
    }
}

fn builtin_div(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("/", args.len(), 2));
    }
    let a = &args[0];
    let b = &args[1];
    let (xa, xb) = (numeric(a), numeric(b));
    match (xa, xb) {
        (Some(x), Some(y)) => {
            if y == 0.0 {
                Err(type_error("/", "division by zero".into()))
            } else if let (Value::Integer(i1), Value::Integer(i2)) = (a, b) {
                if *i2 == 0 {
                    return Err(type_error("/", "division by zero".into()));
                }
                Ok(Outcome::normal(Value::Integer(i1 / i2)))
            } else {
                Ok(Outcome::normal(Value::Float(x / y)))
            }
        }
        _ => Err(type_error("/", format!(
            "cannot divide {} and {}",
            type_name(a), type_name(b)
        ))),
    }
}

fn builtin_mod(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("%", args.len(), 2));
    }
    if let (Value::Integer(i1), Value::Integer(i2)) = (&args[0], &args[1]) {
        if *i2 == 0 {
            return Err(type_error("%", "modulo by zero".into()));
        }
        Ok(Outcome::normal(Value::Integer(i1 % i2)))
    } else {
        Err(type_error("%", format!(
            "expected two integers, got {} and {}",
            type_name(&args[0]), type_name(&args[1])
        )))
    }
}

fn builtin_eq(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (a, b) = expect_arity2("==", &args)?;
    Ok(Outcome::normal(Value::Boolean(values_equal(a, b))))
}

fn builtin_ne(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (a, b) = expect_arity2("!=", &args)?;
    Ok(Outcome::normal(Value::Boolean(!values_equal(a, b))))
}

fn builtin_lt(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (a, b) = expect_arity2("<", &args)?;
    cmp_op(a, b, "less than", |o| o == std::cmp::Ordering::Less)
}

fn builtin_gt(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (a, b) = expect_arity2(">", &args)?;
    cmp_op(a, b, "greater than", |o| o == std::cmp::Ordering::Greater)
}

fn builtin_le(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (a, b) = expect_arity2("<=", &args)?;
    cmp_op(a, b, "less or equal", |o| o != std::cmp::Ordering::Greater)
}

fn builtin_ge(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (a, b) = expect_arity2(">=", &args)?;
    cmp_op(a, b, "greater or equal", |o| o != std::cmp::Ordering::Less)
}

fn cmp_op<F>(a: &Value, b: &Value, label: &str, pred: F) -> WlwlResult<Outcome>
where F: Fn(std::cmp::Ordering) -> bool,
{
    // Numeric comparison.
    if let (Some(x), Some(y)) = (numeric(a), numeric(b)) {
        let ord = x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal);
        return Ok(Outcome::normal(Value::Boolean(pred(ord))));
    }
    // String comparison.
    if let (Value::String(s1), Value::String(s2)) = (a, b) {
        return Ok(Outcome::normal(Value::Boolean(pred(s1.cmp(s2)))));
    }
    Err(type_error(
        "<cmp>",
        format!("cannot compute {} for {} and {}", label, type_name(a), type_name(b)),
    ))
}

fn builtin_and(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (a, b) = expect_arity2("&&", &args)?;
    Ok(Outcome::normal(Value::Boolean(is_truthy(a) && is_truthy(b))))
}

fn builtin_or(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (a, b) = expect_arity2("||", &args)?;
    Ok(Outcome::normal(Value::Boolean(is_truthy(a) || is_truthy(b))))
}

fn builtin_not(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("!", &args, 1)?;
    Ok(Outcome::normal(Value::Boolean(!is_truthy(v))))
}

/// `OR_DIE(value, default)`: §12. If `value` is OK(v) → v. If ERR(_) →
/// `default` (evaluated lazily, but in this implementation it has
/// already been evaluated by the call site). This is a §12.6
/// ERR-consumer, so it gets a special entry in `is_err_consumer`.
fn builtin_or_die(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("OR_DIE", args.len(), 2));
    }
    match &args[0] {
        Value::Ok(v) => Ok(Outcome::normal((**v).clone())),
        Value::Err(_) => Ok(Outcome::normal(args[1].clone())),
        other => Err(type_error(
            "OR_DIE",
            format!("expected OK/ERR, got {}", type_name(other)),
        )),
    }
}

fn is_truthy(v: &Value) -> bool {
    !matches!(v, Value::Boolean(false) | Value::Null)
}

/// Structural equality (v0.3 §10.4). Dict key ordering does not matter.
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Integer(x), Value::Integer(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Integer(x), Value::Float(y)) => (*x as f64) == *y,
        (Value::Float(x), Value::Integer(y)) => *x == (*y as f64),
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Boolean(x), Value::Boolean(y)) => x == y,
        (Value::Null, Value::Null) => true,
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| values_equal(a, b))
        }
        (Value::Dict(x), Value::Dict(y)) => {
            if x.len() != y.len() {
                return false;
            }
            x.iter().all(|(xk, xv)| {
                y.iter().any(|(yk, yv)| values_equal(xk, yk) && values_equal(xv, yv))
            })
        }
        (Value::Ok(x), Value::Ok(y)) => values_equal(x, y),
        (Value::Err(x), Value::Err(y)) => values_equal(x, y),
        _ => false,
    }
}

// ──────────────────────────────────────────────────────────────────────
// Evaluator
// ──────────────────────────────────────────────────────────────────────

pub struct Evaluator {
    env: Env,
    /// Optional original source (for `source_line` in runtime diagnostics).
    source: Option<String>,
    /// Current source file name (for diagnostics).
    file: Option<String>,
    /// Module loader (shared with sub-evaluators for cross-module IMPORTs).
    /// `RefCell` for interior mutability so we can mutate the cache and
    /// the loading-set without conflicting with `&mut self` on the
    /// evaluator.
    loader: Rc<RefCell<ModuleLoader>>,
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluator {
    pub fn new() -> Self {
        Self {
            env: Env::new(),
            source: None,
            file: None,
            loader: Rc::new(RefCell::new(ModuleLoader::new(PathBuf::from(".")))),
        }
    }

    /// Set the base directory used to resolve `IMPORT` paths. Must be
    /// called before `eval` when the program uses `IMPORT`.
    pub fn with_base_dir(mut self, dir: PathBuf) -> Self {
        // Rebuild the loader with the new base_dir.
        self.loader = Rc::new(RefCell::new(ModuleLoader::new(dir)));
        self
    }

    fn new_with_loader(loader: ModuleLoader) -> Self {
        Self {
            env: Env::new(),
            source: None,
            file: None,
            loader: Rc::new(RefCell::new(loader)),
        }
    }

    /// Attach the original source text and file name so that runtime
    /// diagnostics (e.g. `E0020` undefined name) include a `source_line`.
    pub fn with_source(mut self, source: impl Into<String>, file: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self.file = Some(file.into());
        self
    }

    // ── Top-level entry points ─────────────────────────────────────

    /// Evaluate a program (the result of `parse`). Used for both
    /// entry-point files and modules.
    pub fn eval(&mut self, expr: &Expr) -> WlwlResult<Value> {
        let outcome = self.eval_top_level(expr)?;
        // §19.6 Corollary 19.1: if the top-level program finishes with
        // an ERR value, either as a Return(Err) signal (from a TRY
        // inside a function) or as the final value, that means an ERR
        // escaped without being consumed — E0102.
        if let Signal::Return(Value::Err(payload)) = &outcome.signal {
            return Err(self.diag(
                ErrorCode::E0102,
                format!("unhandled ERR escaped to top level: {}", payload.display()),
                expr.span().clone(),
            ));
        }
        if matches!(outcome.signal, Signal::None) {
            if let Value::Err(payload) = &outcome.value {
                return Err(self.diag(
                    ErrorCode::E0102,
                    format!("unhandled ERR escaped to top level: {}", payload.display()),
                    expr.span().clone(),
                ));
            }
        }
        // Other signals at top level are a bug; treat as E0014.
        match outcome.signal {
            Signal::None => Ok(outcome.value),
            Signal::Break | Signal::Continue => Err(self.diag(
                ErrorCode::E0014,
                "BREAK or CONTINUE used outside a loop".to_string(),
                expr.span().clone(),
            )),
            Signal::Return(v) => Ok(v),
        }
    }

    /// Evaluate a program at the top level. If the program is a Block
    /// (which is what the parser always produces for a `.wl` file),
    /// evaluate it as a *top-level* block — the block does not get its
    /// own scope, so top-level `LET` bindings and `EXPORT` declarations
    /// persist after evaluation.
    fn eval_top_level(&mut self, expr: &Expr) -> WlwlResult<Outcome> {
        if let Expr::Block { exprs, .. } = expr {
            self.eval_block(exprs, true)
        } else {
            self.eval_expr(expr)
        }
    }

    /// Like `eval`, but suppresses the top-level `Return(Err(_)) → E0102`
    /// promotion. Used when evaluating a module file, where returning an
    /// ERR at the top is fine — the caller (an `IMPORT` site) will
    /// see the ERR and §12.6 will propagate it transparently.
    fn eval_module(&mut self, expr: &Expr) -> WlwlResult<()> {
        let outcome = self.eval_top_level(expr)?;
        match outcome.signal {
            Signal::None | Signal::Return(_) => Ok(()),
            Signal::Break | Signal::Continue => Err(self.diag(
                ErrorCode::E0014,
                "BREAK or CONTINUE used outside a loop".to_string(),
                expr.span().clone(),
            )),
        }
    }

    // ── Expression evaluation ──────────────────────────────────────

    fn eval_expr(&mut self, expr: &Expr) -> WlwlResult<Outcome> {
        match expr {
            Expr::Literal(lit, _) => Ok(Outcome::normal(Value::from(lit.clone()))),
            Expr::Var(name, span) => match self.env.get(name) {
                Some(v) => Ok(Outcome::normal(v.clone())),
                None => Err(self.undefined_name(name, span)),
            },
            Expr::Call { name, args, span } => self.eval_call(name, args, span),
            Expr::Let { name, value, .. } => {
                let v = self.eval_expr(value)?;
                if v.signal != Signal::None {
                    return Ok(v);
                }
                // Phase 2 fix: if `name` already exists in any enclosing
                // scope, update that binding (so LET inside a loop body
                // can accumulate). Otherwise bind in the current scope.
                if !self.env.set_existing(&name, v.value.clone()) {
                    self.env.set_local(name.clone(), v.value.clone());
                }
                Ok(Outcome::normal(Value::Null))
            }
            Expr::Block { exprs, .. } => self.eval_block(exprs, false),
            Expr::Array { items, .. } => {
                let mut vs = Vec::with_capacity(items.len());
                for it in items {
                    let o = self.eval_expr(it)?;
                    if o.signal != Signal::None {
                        return Ok(o);
                    }
                    vs.push(o.value);
                }
                Ok(Outcome::normal(Value::Array(vs)))
            }
            Expr::Dict { entries, .. } => {
                let mut vs = Vec::with_capacity(entries.len());
                for (k, v) in entries {
                    let ko = self.eval_expr(k)?;
                    if ko.signal != Signal::None {
                        return Ok(ko);
                    }
                    let vo = self.eval_expr(v)?;
                    if vo.signal != Signal::None {
                        return Ok(vo);
                    }
                    vs.push((ko.value, vo.value));
                }
                Ok(Outcome::normal(Value::Dict(vs)))
            }
            Expr::If { cond, then_branch, else_branch, .. } => {
                self.eval_if(cond, then_branch, else_branch.as_deref())
            }
            Expr::While { cond, body, .. } => self.eval_while(cond, body),
            Expr::For { var, iter, body, .. } => self.eval_for(var, iter, body),
            Expr::Return { value, .. } => {
                let v = if let Some(e) = value {
                    let o = self.eval_expr(e)?;
                    if o.signal != Signal::None {
                        return Ok(o);
                    }
                    o.value
                } else {
                    Value::Null
                };
                Ok(Outcome { value: Value::Null, signal: Signal::Return(v) })
            }
            Expr::Break { .. } => {
                Ok(Outcome { value: Value::Null, signal: Signal::Break })
            }
            Expr::Continue { .. } => {
                Ok(Outcome { value: Value::Null, signal: Signal::Continue })
            }
            Expr::Fun { params, body, .. } => {
                // Capture the *current* env by clone. The closure can be
                // called later, and at that point we push a new scope on
                // top of the captured env. Cloning Env is cheap for
                // small scopes; for very large programs this is a
                // candidate for Rc<RefCell> in Phase 4+ performance work.
                Ok(Outcome::normal(Value::Closure {
                    params: params.clone(),
                    body: body.clone(),
                    env: self.env.clone(),
                }))
            }
            Expr::Ok { value, .. } => {
                let o = self.eval_expr(value)?;
                if o.signal != Signal::None {
                    return Ok(o);
                }
                Ok(Outcome::normal(Value::Ok(Box::new(o.value))))
            }
            Expr::Err { value, .. } => {
                let o = self.eval_expr(value)?;
                if o.signal != Signal::None {
                    return Ok(o);
                }
                Ok(Outcome::normal(Value::Err(Box::new(o.value))))
            }
            Expr::Panic { value, .. } => {
                let o = self.eval_expr(value)?;
                if o.signal != Signal::None {
                    return Ok(o);
                }
                // PANIC: an unrecoverable error. Emit E0100 with the panic
                // message as the diagnostic message.
                let msg = o.value.display();
                Err(self.diag(
                    ErrorCode::E0100,
                    format!("PANIC: {}", msg),
                    expr.span().clone(),
                ))
            }
            Expr::Try { value, .. } => {
                let o = self.eval_expr(value)?;
                match o.value {
                    Value::Ok(v) => Ok(Outcome::normal(*v)),
                    Value::Err(v) => {
                        // §19.4 E-TryErr: emit Return(Err(v)).
                        // This propagates up to the enclosing function
                        // frame; at the top level it becomes E0102.
                        Ok(Outcome { value: Value::Null, signal: Signal::Return(Value::Err(v)) })
                    }
                    other => Err(self.diag(
                        ErrorCode::E0030,
                        format!("TRY expects OK/ERR, got {}", type_name(&other)),
                        expr.span().clone(),
                    )),
                }
            }
            Expr::IsOk { value, .. } => {
                let o = self.eval_expr(value)?;
                if o.signal != Signal::None {
                    return Ok(o);
                }
                let r = matches!(o.value, Value::Ok(_));
                Ok(Outcome::normal(Value::Boolean(r)))
            }
            Expr::IsErr { value, .. } => {
                let o = self.eval_expr(value)?;
                if o.signal != Signal::None {
                    return Ok(o);
                }
                let r = matches!(o.value, Value::Err(_));
                Ok(Outcome::normal(Value::Boolean(r)))
            }
            Expr::OrDie { value, default, .. } => {
                let o = self.eval_expr(value)?;
                if o.signal != Signal::None {
                    return Ok(o);
                }
                match o.value {
                    Value::Ok(v) => Ok(Outcome::normal(*v)),
                    Value::Err(_) => {
                        let d = self.eval_expr(default)?;
                        if d.signal != Signal::None {
                            return Ok(d);
                        }
                        Ok(Outcome::normal(d.value))
                    }
                    other => Err(self.diag(
                        ErrorCode::E0030,
                        format!("OR_DIE expects OK/ERR, got {}", type_name(&other)),
                        expr.span().clone(),
                    )),
                }
            }
            Expr::Import { path, names, .. } => self.eval_import(path, names),
            Expr::Export { names, .. } => self.eval_export(names, expr.span()),
        }
    }

    /// Evaluate a block expression. `top_level` controls whether the
    /// block introduces a new scope (false, normal block) or runs in
    /// the current scope (true, used for module top-level so that
    /// `LET` bindings and `EXPORT` declarations persist after the
    /// block is done).
    fn eval_block(&mut self, exprs: &[Expr], top_level: bool) -> WlwlResult<Outcome> {
        if !top_level {
            // Blocks create a new lexical scope — LET inside the block
            // does not leak out.
            self.env.push_scope();
        }
        let mut last = Outcome::normal(Value::Null);
        for e in exprs {
            last = self.eval_expr(e)?;
            // Control-flow signal short-circuits the block.
            if last.signal != Signal::None {
                break;
            }
        }
        if !top_level {
            self.env.pop_scope();
        }
        Ok(last)
    }

    fn eval_if(
        &mut self,
        cond: &Expr,
        then_branch: &Expr,
        else_branch: Option<&Expr>,
    ) -> WlwlResult<Outcome> {
        let c = self.eval_expr(cond)?;
        if c.signal != Signal::None {
            return Ok(c);
        }
        if is_truthy(&c.value) {
            self.eval_expr(then_branch)
        } else if let Some(e) = else_branch {
            self.eval_expr(e)
        } else {
            Ok(Outcome::normal(Value::Null))
        }
    }

    fn eval_while(&mut self, cond: &Expr, body: &Expr) -> WlwlResult<Outcome> {
        loop {
            let c = self.eval_expr(cond)?;
            if c.signal != Signal::None {
                return Ok(c);
            }
            if !is_truthy(&c.value) {
                break;
            }
            // Push a fresh scope around the loop body so LETs inside
            // the body don't leak across iterations.
            self.env.push_scope();
            let o = self.eval_expr(body)?;
            self.env.pop_scope();
            match o.signal {
                Signal::None => continue,
                Signal::Continue => continue,
                Signal::Break => {
                    // Break terminates the loop; consume the signal so
                    // it does not escape to the enclosing frame.
                    break;
                }
                Signal::Return(_) => return Ok(o),
            }
        }
        Ok(Outcome::normal(Value::Null))
    }

    fn eval_for(&mut self, var: &str, iter_expr: &Expr, body: &Expr) -> WlwlResult<Outcome> {
        let iter_val = self.eval_expr(iter_expr)?;
        if iter_val.signal != Signal::None {
            return Ok(iter_val);
        }
        // Bind the loop variable in a fresh scope.
        self.env.push_scope();
        let outcome = match iter_val.value {
            Value::Array(items) => {
                for item in items {
                    self.env.set_local(var, item);
                    let o = self.eval_expr(body)?;
                    match o.signal {
                        Signal::None | Signal::Continue => {}
                        Signal::Break => {
                            self.env.pop_scope();
                            // Break terminates the loop; consume the
                            // signal so it does not escape to the
                            // enclosing frame.
                            return Ok(Outcome::normal(Value::Null));
                        }
                        Signal::Return(_) => {
                            self.env.pop_scope();
                            return Ok(o);
                        }
                    }
                }
                Outcome::normal(Value::Null)
            }
            Value::Dict(entries) => {
                for (k, _v) in entries {
                    self.env.set_local(var, k);
                    let o = self.eval_expr(body)?;
                    match o.signal {
                        Signal::None | Signal::Continue => {}
                        Signal::Break => {
                            self.env.pop_scope();
                            return Ok(Outcome::normal(Value::Null));
                        }
                        Signal::Return(_) => {
                            self.env.pop_scope();
                            return Ok(o);
                        }
                    }
                }
                Outcome::normal(Value::Null)
            }
            Value::String(s) => {
                for ch in s.chars() {
                    self.env.set_local(var, Value::String(ch.to_string()));
                    let o = self.eval_expr(body)?;
                    match o.signal {
                        Signal::None | Signal::Continue => {}
                        Signal::Break => {
                            self.env.pop_scope();
                            return Ok(Outcome::normal(Value::Null));
                        }
                        Signal::Return(_) => {
                            self.env.pop_scope();
                            return Ok(o);
                        }
                    }
                }
                Outcome::normal(Value::Null)
            }
            other => {
                self.env.pop_scope();
                return Err(self.diag(
                    ErrorCode::E0030,
                    format!(
                        "FOR expects an iterable (array/dict/string), got {}",
                        type_name(&other)
                    ),
                    iter_expr.span().clone(),
                ));
            }
        };
        self.env.pop_scope();
        Ok(outcome)
    }

    // ── Calls (the heart of §12.6 ERR transparent propagation) ─────

    fn eval_call(&mut self, name: &str, args: &[Expr], span: &Span) -> WlwlResult<Outcome> {
        // Look up the callee (user function takes priority over built-in
        // with the same name; in Phase 2 we keep them in disjoint
        // namespaces by convention — there is no name conflict in the
        // std yet).
        let user_fn = self.env.get(name).cloned();
        let whitelisted = is_err_consumer(name);

        // Evaluate arguments left-to-right. §12.6 short-circuit: if
        // the callee is not in the ERR whitelist and we have already
        // seen an ERR, return the leftmost ERR without evaluating the
        // remaining args.
        let mut arg_values = Vec::with_capacity(args.len());
        let mut pending_err: Option<Value> = None;
        for a in args {
            let o = self.eval_expr(a)?;
            if o.signal != Signal::None {
                return Ok(o);
            }
            if !whitelisted {
                if let Value::Err(_) = &o.value {
                    pending_err.get_or_insert_with(|| o.value.clone());
                    // Stop evaluating further args — the ERR will
                    // short-circuit the call.
                    break;
                }
            }
            arg_values.push(o.value);
        }

        // §12.6 transparent propagation: if the function is NOT in the
        // ERR-consumer whitelist, the first ERR encountered is the
        // result of the call.
        if !whitelisted {
            if let Some(err) = pending_err {
                return Ok(Outcome::normal(err));
            }
            for v in &arg_values {
                if let Value::Err(_) = v {
                    return Ok(Outcome::normal(v.clone()));
                }
            }
        }

        // Dispatch.
        if let Some(v) = user_fn {
            if let Value::Closure { params, body, env } = v {
                return self.invoke_closure(params, body, env, arg_values, span);
            }
            // If the name resolves to a non-Closure value, treat as
            // E0020 (the user is trying to call a non-callable).
            return Err(self.diag(
                ErrorCode::E0020,
                format!("'{}' is not a function", name),
                span.clone(),
            ));
        }
        if let Some(b) = resolve_builtin(name) {
            return b(self, arg_values);
        }
        Err(self.undefined_name(name, span))
    }

    fn invoke_closure(
        &mut self,
        params: Vec<String>,
        body: Box<Expr>,
        captured_env: Env,
        arg_values: Vec<Value>,
        span: &Span,
    ) -> WlwlResult<Outcome> {
        if params.len() != arg_values.len() {
            return Err(self.diag(
                ErrorCode::E0022,
                format!(
                    "function expects {} argument(s), got {}",
                    params.len(),
                    arg_values.len()
                ),
                span.clone(),
            ));
        }
        // Install the function's lexical frame on top of the caller's
        // env. This makes the function's lexical captures visible (the
        // closure-captured variables) while still allowing lookups to
        // fall through to the caller's env (which includes the global
        // scope). The latter is what makes self-recursion work — at
        // function definition time, the function's own name may not yet
        // be in the captured env, but the global scope (where it is
        // eventually bound) is reachable through the caller.
        let caller_scopes = std::mem::take(&mut self.env.scopes);
        let captured_scopes = captured_env.scopes;
        // New scope stack: [captured lexical scopes, caller scopes, fresh param scope]
        let mut new_scopes = captured_scopes;
        new_scopes.extend(caller_scopes.iter().cloned());
        new_scopes.push(HashMap::new()); // fresh scope for params
        self.env.scopes = new_scopes;
        for (p, v) in params.iter().zip(arg_values) {
            self.env.set_local(p, v);
        }
        let outcome = self.eval_expr(&body)?;
        // Restore the caller's env EXACTLY (by swapping back, so any
        // mutations during the call are discarded).
        self.env.scopes = caller_scopes;

        // Convert a Return signal back to a normal value; treat
        // Break/Continue as E0014 inside a function body.
        match outcome.signal {
            Signal::None => Ok(outcome),
            Signal::Return(v) => Ok(Outcome::normal(v)),
            Signal::Break | Signal::Continue => Err(self.diag(
                ErrorCode::E0014,
                "BREAK or CONTINUE used outside a loop".to_string(),
                span.clone(),
            )),
        }
    }

    // ── Module evaluation ──────────────────────────────────────────

    fn eval_import(&mut self, path: &str, names: &[ImportName]) -> WlwlResult<Outcome> {
        // Phase 2: `path` is a simple bare module name (no ./, ../, or
        // `wlwl:` — the parser already rejected those).
        let module_name = path.to_string();
        // The loader is shared via Rc<RefCell<ModuleLoader>> so we can
        // mutate the cache + loading-set through interior mutability
        // without conflicting with the &mut self borrow held by
        // eval_import.
        let module = {
            let mut loader = self.loader.borrow_mut();
            loader.load(&module_name)?
        };
        // Bind each requested name in the current scope, with rename.
        for imp in names {
            // Reject duplicate imports of the same local name in this
            // scope (E0021, v0.3 §13.3).
            let local = imp.local_name().to_string();
            if self.env.scopes.last().map(|s| s.contains_key(&local)).unwrap_or(false) {
                return Err(self.diag(
                    ErrorCode::E0021,
                    format!(
                        "name '{}' already bound by previous IMPORT in this scope",
                        local
                    ),
                    imp.span.clone(),
                ));
            }
            // Verify the *original* (un-renamed) name was actually
            // exported by the module. The alias is purely a local
            // binding concern.
            if !module.exports.contains(&imp.name) {
                return Err(self.diag(
                    ErrorCode::E0023,
                    format!(
                        "'{}' is not exported by module '{}'",
                        imp.name, module_name
                    ),
                    imp.span.clone(),
                ));
            }
            let v = module.env.get(&imp.name).cloned().ok_or_else(|| {
                self.diag(
                    ErrorCode::E0023,
                    format!(
                        "'{}' is exported by '{}' but missing at runtime (internal bug)",
                        imp.name, module_name
                    ),
                    imp.span.clone(),
                )
            })?;
            self.env.set_local(local, v);
        }
        Ok(Outcome::normal(Value::Null))
    }

    fn eval_export(&mut self, names: &[ImportName], span: &Span) -> WlwlResult<Outcome> {
        // EXPORT is a no-op at runtime in Phase 2 — its effect is
        // captured by `collect_exports` when the module finishes
        // loading. We just verify each name is actually bound.
        for imp in names {
            let local = imp.local_name();
            if self.env.get(local).is_none() {
                return Err(self.diag(
                    ErrorCode::E0020,
                    format!(
                        "EXPORT: name '{}' is not bound in this module",
                        local
                    ),
                    imp.span.clone(),
                ));
            }
        }
        let _ = span; // silence unused
        Ok(Outcome::normal(Value::Null))
    }

    // ── Diagnostic helpers ─────────────────────────────────────────

    fn diag(&self, code: ErrorCode, message: impl Into<String>, span: Span) -> WlwlError {
        let loc = Location {
            file: span.file.clone(),
            line: span.line_start,
            col: span.col_start,
            line_end: span.line_end,
            col_end: span.col_end,
        };
        let mut d = WlwlDiagnostic::new(code, message, loc);
        if let Some(src) = &self.source {
            if let Some(line_text) = extract_line(src, span.line_start) {
                d = d.with_source_line(line_text);
            }
        }
        d.into()
    }

    fn undefined_name(&self, name: &str, span: &Span) -> WlwlError {
        let mut d = WlwlDiagnostic::new(
            ErrorCode::E0020,
            format!("undefined name '{}'", name),
            Location {
                file: span.file.clone(),
                line: span.line_start,
                col: span.col_start,
                line_end: span.line_end,
                col_end: span.col_end,
            },
        );
        d = d.with_hint(format!(
            "no binding for '{}'; either add a LET before this use, or import it from a module",
            name
        ));
        if let Some(src) = &self.source {
            if let Some(line_text) = extract_line(src, span.line_start) {
                d = d.with_source_line(line_text);
            }
        }
        d.into()
    }
}

// ──────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use wlwl_parser::parse;

    /// Parse + eval a one-shot expression. The temporary directory used
    /// as the module base is irrelevant for tests that don't IMPORT.
    fn run(src: &str) -> WlwlResult<Value> {
        let e = parse(src, "t.wl")?;
        let mut ev = Evaluator::new();
        ev.eval(&e)
    }

    fn run_in(dir: &Path, src: &str) -> WlwlResult<Value> {
        let e = parse(src, "t.wl")?;
        let mut ev = Evaluator::new().with_base_dir(dir.to_path_buf());
        ev.eval(&e)
    }

    // ── Phase 1 sanity (unchanged) ─────────────────────────────────

    #[test]
    fn eval_integer() {
        assert_eq!(run("42;").unwrap(), Value::Integer(42));
    }

    #[test]
    fn eval_let_var_lookup() {
        let v = run("LET(x, 1); LET(y, x); PRINT(y);").unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn eval_array() {
        assert_eq!(
            run("[1, 2, 3];").unwrap(),
            Value::Array(vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)])
        );
    }

    #[test]
    fn eval_dict() {
        assert_eq!(
            run("[\"a\": 1, \"b\": 2];").unwrap(),
            Value::Dict(vec![
                (Value::String("a".into()), Value::Integer(1)),
                (Value::String("b".into()), Value::Integer(2)),
            ])
        );
    }

    #[test]
    fn eval_undefined_name() {
        let err = run("PRINT(x);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0020);
    }

    #[test]
    fn eval_call_unknown_builtin() {
        let err = run("NOSUCH();").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0020);
    }

    #[test]
    fn eval_block_returns_last() {
        assert_eq!(run("LET(x, 1); LET(y, 2); y;").unwrap(), Value::Integer(2));
    }

    // ── §9 Operators ───────────────────────────────────────────────

    #[test]
    fn op_add_ints() {
        assert_eq!(run("+(1, 2);").unwrap(), Value::Integer(3));
    }

    #[test]
    fn op_add_strings() {
        assert_eq!(
            run(r#"+("hello, ", "world");"#).unwrap(),
            Value::String("hello, world".into())
        );
    }

    #[test]
    fn op_arith() {
        assert_eq!(run("-(10, 3);").unwrap(), Value::Integer(7));
        assert_eq!(run("*(4, 5);").unwrap(), Value::Integer(20));
        assert_eq!(run("/(10, 3);").unwrap(), Value::Integer(3));
        assert_eq!(run("%(10, 3);").unwrap(), Value::Integer(1));
    }

    #[test]
    fn op_div_by_zero() {
        let err = run("/(1, 0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn op_eq_ne() {
        assert_eq!(run("==(1, 1);").unwrap(), Value::Boolean(true));
        assert_eq!(run("==(1, 2);").unwrap(), Value::Boolean(false));
        assert_eq!(run("!=(1, 2);").unwrap(), Value::Boolean(true));
    }

    #[test]
    fn op_ordering() {
        assert_eq!(run("<(1, 2);").unwrap(), Value::Boolean(true));
        assert_eq!(run(">(1, 2);").unwrap(), Value::Boolean(false));
        assert_eq!(run("<=(1, 1);").unwrap(), Value::Boolean(true));
        assert_eq!(run(">=(2, 1);").unwrap(), Value::Boolean(true));
    }

    #[test]
    fn op_logical() {
        assert_eq!(run("&&(TRUE, FALSE);").unwrap(), Value::Boolean(false));
        assert_eq!(run("||(FALSE, TRUE);").unwrap(), Value::Boolean(true));
        assert_eq!(run("!(TRUE);").unwrap(), Value::Boolean(false));
        assert_eq!(run("!(FALSE);").unwrap(), Value::Boolean(true));
        // Truthiness rules: null and false are falsy; everything else truthy.
        assert_eq!(run("!(NULL);").unwrap(), Value::Boolean(true));
        assert_eq!(run("&&(1, NULL);").unwrap(), Value::Boolean(false));
    }

    // ── §7 Control flow ────────────────────────────────────────────

    #[test]
    fn control_if_then() {
        assert_eq!(run("IF(TRUE, 1, 2);").unwrap(), Value::Integer(1));
        assert_eq!(run("IF(FALSE, 1, 2);").unwrap(), Value::Integer(2));
    }

    #[test]
    fn control_if_no_else() {
        // No else branch: returns NULL when condition is false.
        assert_eq!(run("IF(FALSE, 1);").unwrap(), Value::Null);
        assert_eq!(run("IF(TRUE, 1);").unwrap(), Value::Integer(1));
    }

    #[test]
    fn control_while_sum() {
        // Sum 1..10 with a while loop.
        let src = r#"
            LET(total, 0);
            LET(i, 1);
            WHILE(<(i, 11),
                LET(total, +(total, i));
                LET(i, +(i, 1))
            );
            total;
        "#;
        assert_eq!(run(src).unwrap(), Value::Integer(55));
    }

    #[test]
    fn control_for_array() {
        // Sum 1..5 using FOR.
        let src = r#"
            LET(total, 0);
            FOR(i, [1, 2, 3, 4, 5],
                LET(total, +(total, i))
            );
            total;
        "#;
        assert_eq!(run(src).unwrap(), Value::Integer(15));
    }

    #[test]
    fn control_for_break() {
        // Break out of a FOR loop early.
        let src = r#"
            LET(total, 0);
            FOR(i, [1, 2, 3, 4, 5],
                IF(==(i, 3),
                    BREAK()
                );
                LET(total, +(total, i))
            );
            total;
        "#;
        assert_eq!(run(src).unwrap(), Value::Integer(1 + 2));
    }

    #[test]
    fn control_continue() {
        // CONTINUE skips the rest of the body.
        let src = r#"
            LET(total, 0);
            FOR(i, [1, 2, 3, 4, 5],
                IF(==(i, 3),
                    CONTINUE()
                );
                LET(total, +(total, i))
            );
            total;
        "#;
        // 1+2+4+5 = 12
        assert_eq!(run(src).unwrap(), Value::Integer(12));
    }

    #[test]
    fn control_break_continue_outside_loop_is_e0014() {
        let err = run("BREAK();").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0014);
        let err = run("CONTINUE();").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0014);
    }

    // ── §8 Functions and closures ──────────────────────────────────

    #[test]
    fn fun_call_basic() {
        // Define a function, bind it, call it.
        let src = r#"
            LET(double, FUN((x), *(x, 2)));
            double(5);
        "#;
        assert_eq!(run(src).unwrap(), Value::Integer(10));
    }

    #[test]
    fn fun_two_params() {
        let src = r#"
            LET(add, FUN((a, b), +(a, b)));
            add(3, 4);
        "#;
        assert_eq!(run(src).unwrap(), Value::Integer(7));
    }

    #[test]
    fn fun_zero_params() {
        let src = r#"
            LET(answer, FUN((), 42));
            answer();
        "#;
        assert_eq!(run(src).unwrap(), Value::Integer(42));
    }

    #[test]
    fn fun_arity_mismatch_e0022() {
        // Too few args.
        let err = run(r#"
            LET(f, FUN((a, b), +(a, b)));
            f(1);
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
        // Too many args.
        let err = run(r#"
            LET(f, FUN((a, b), +(a, b)));
            f(1, 2, 3);
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn fun_recursion_factorial() {
        // Classic recursion test.
        let src = r#"
            LET(fact, FUN((n),
                IF(<=(n, 1),
                    1,
                    *(n, fact(-(n, 1)))
                )
            ));
            fact(5);
        "#;
        assert_eq!(run(src).unwrap(), Value::Integer(120));
    }

    #[test]
    fn fun_closure_captures_var() {
        // The inner function captures `x` from the enclosing scope.
        let src = r#"
            LET(x, 10);
            LET(get_x, FUN((), x));
            get_x();
        "#;
        assert_eq!(run(src).unwrap(), Value::Integer(10));
    }

    #[test]
    fn fun_closure_independent() {
        // Two closures capture their own env at definition time.
        let src = r#"
            LET(mk, FUN((v), FUN((), v)));
            LET(a, mk(1));
            LET(b, mk(2));
            +(a(), b());
        "#;
        assert_eq!(run(src).unwrap(), Value::Integer(3));
    }

    #[test]
    fn fun_return_void() {
        // RETURN() with no value → returns NULL from the function.
        let src = r#"
            LET(f, FUN((), RETURN()));
            f();
        "#;
        assert_eq!(run(src).unwrap(), Value::Null);
    }

    #[test]
    fn fun_return_early() {
        // RETURN short-circuits the rest of the body.
        let src = r#"
            LET(f, FUN((x),
                IF(<(x, 0),
                    RETURN(0)
                );
                x
            ));
            -(f(5), f(-3));
        "#;
        // f(5) = 5, f(-3) = 0, 5 - 0 = 5.
        assert_eq!(run(src).unwrap(), Value::Integer(5));
    }

    // ── §12 Error handling and §12.6 transparent propagation ──────

    #[test]
    fn err_is_ok_is_err() {
        assert_eq!(run("IS_OK(OK(1));").unwrap(), Value::Boolean(true));
        assert_eq!(run("IS_OK(ERR(1));").unwrap(), Value::Boolean(false));
        assert_eq!(run("IS_ERR(OK(1));").unwrap(), Value::Boolean(false));
        assert_eq!(run("IS_ERR(ERR(1));").unwrap(), Value::Boolean(true));
    }

    #[test]
    fn err_or_die_ok() {
        // OK → unwrap.
        assert_eq!(run(r#"OR_DIE(OK(42), 0);"#).unwrap(), Value::Integer(42));
    }

    #[test]
    fn err_or_die_err_uses_default() {
        // ERR → fall back to the default. Default is only evaluated on
        // ERR (lazy).
        assert_eq!(run(r#"OR_DIE(ERR("bad"), 99);"#).unwrap(), Value::Integer(99));
    }

    #[test]
    fn err_try_ok_propagates_value() {
        // TRY(OK(v)) returns v; the function returns 7.
        let src = r#"
            LET(f, FUN((),
                LET(x, TRY(OK(7)));
                x
            ));
            f();
        "#;
        assert_eq!(run(src).unwrap(), Value::Integer(7));
    }

    #[test]
    fn err_try_err_returns_from_enclosing_function() {
        // TRY(ERR(...)) makes the enclosing function return that ERR.
        let src = r#"
            LET(f, FUN((),
                TRY(ERR("inner"));
                PRINT("after try")
            ));
            f();
        "#;
        // The function returns Value::Err("inner"); top-level eval
        // promotes unhandled Return(Err(_)) at the top to E0102.
        let err = run(src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    // ── §12.6 ERR transparent propagation (the big one) ──────────

    #[test]
    fn transparent_propagation_builtin() {
        // + with one ERR arg: returns ERR without entering +.
        // Wrap with OR_DIE so the top-level value is not the raw ERR
        // (which would trigger E0102 per §19.6 Corollary 19.1).
        let r = run(r#"OR_DIE(+(1, ERR("bad")), "ok");"#).unwrap();
        assert_eq!(r, Value::String("ok".into()));
        // + with ERR as the *first* arg also propagates.
        let r = run(r#"OR_DIE(+(ERR("e1"), 2), "ok");"#).unwrap();
        assert_eq!(r, Value::String("ok".into()));
    }

    #[test]
    fn transparent_propagation_user_function() {
        // A user function `f` called with an ERR arg never runs f.
        let src = r#"
            LET(f, FUN((x), PRINT("ran")));  // would print if executed
            f(ERR("nope"));
        "#;
        // The call short-circuits to ERR("nope"), which then escapes to
        // the top level → E0102.
        let err = run(src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn transparent_propagation_through_nested_calls() {
        // ERR passes through f → g → h, and h is never entered.
        // The chain returns the ERR (which we OR_DIE to handle).
        let src = r#"
            LET(h, FUN((x), PRINT("h ran")));
            LET(g, FUN((x), h(x)));
            LET(f, FUN((x), g(x)));
            OR_DIE(f(ERR("boom")), -1);
        "#;
        assert_eq!(run(src).unwrap(), Value::Integer(-1));
    }

    #[test]
    fn transparent_propagation_is_ok_consumes() {
        // IS_OK is whitelisted — it consumes the ERR.
        let src = r#"
            IS_OK(ERR("e"));
        "#;
        // IS_OK(ERR) → FALSE (not an ERR escape).
        assert_eq!(run(src).unwrap(), Value::Boolean(false));
    }

    #[test]
    fn transparent_propagation_try_consumes() {
        // TRY is whitelisted — it consumes the ERR and emits a Return
        // signal up to the enclosing function.
        let src = r#"
            LET(f, FUN((),
                TRY(ERR("e"));
                100
            ));
            OR_DIE(f(), -1);
        "#;
        // f() returns ERR("e") because of TRY → escapes OR_DIE → OR_DIE
        // returns -1.
        assert_eq!(run(src).unwrap(), Value::Integer(-1));
    }

    #[test]
    fn transparent_propagation_or_die_consumes() {
        // OR_DIE is whitelisted — it consumes the ERR.
        assert_eq!(
            run(r#"OR_DIE(ERR("e"), 42);"#).unwrap(),
            Value::Integer(42)
        );
    }

    #[test]
    fn transparent_propagation_leftmost_err_wins() {
        // Multiple ERR args: we return the leftmost (deterministic).
        // Wrap with OR_DIE so the raw ERR doesn't trigger E0102 at top.
        let r = run(r#"OR_DIE(+(ERR("a"), ERR("b")), "ok");"#).unwrap();
        assert_eq!(r, Value::String("ok".into()));
    }

    #[test]
    fn transparent_propagation_short_circuits_eval_order() {
        // If the first arg to + is an OK whose evaluation triggers an
        // error, the second arg (which would PANIC) is not evaluated.
        // We use a LET to test this: the second arg of + is bound to a
        // value first, so we can't directly test short-circuit in arg
        // evaluation order. Instead, test: a function that PANICs is
        // never called when its containing call has an earlier ERR.
        let src = r#"
            LET(boom, FUN((), PANIC("should not run")));
            OR_DIE(+(ERR("e"), boom()), -1);
        "#;
        // boom() is never called because + returns ERR("e") and OR_DIE
        // consumes it.
        assert_eq!(run(src).unwrap(), Value::Integer(-1));
    }

    #[test]
    fn unhandled_err_at_top_level_is_e0102() {
        // ERR that bubbles all the way up becomes E0102 (Corollary
        // 19.1, §19.6).
        let err = run(r#"ERR("top");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn err_panic_is_e0100() {
        let err = run(r#"PANIC("oops");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0100);
    }

    // ── §13 Modules (single-directory, Phase 2 subset) ─────────────

    use std::path::Path;

    /// Make a unique subdirectory inside the system temp dir. We can't
    /// use the `tempfile` crate because its transitive deps aren't in
    /// our offline cargo cache; a manual `std::env::temp_dir` + a
    /// per-test subdirectory gives us the same isolation.
    fn unique_test_dir(name: &str) -> PathBuf {
        // Use nanosecond precision + a per-test name; if a previous run
        // left files behind, we wipe the subdir first.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("wlwl_test_{}_{}", name, nanos));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn module_basic() {
        // Write a sibling module to a temp dir, then IMPORT from a
        // program in the same dir.
        let dir = unique_test_dir("basic");
        let module_path = dir.join("math.wl");
        std::fs::write(
            &module_path,
            r#"
                LET(answer, 42);
                EXPORT(["answer"]);
            "#,
        )
        .unwrap();
        let src = r#"
            IMPORT("math", ["answer"]);
            PRINT(answer);
        "#;
        let v = run_in(&dir, src).unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn module_with_rename() {
        let dir = unique_test_dir("rename");
        std::fs::write(
            dir.join("math.wl"),
            r#"
                LET(pi, 314);
                EXPORT(["pi"]);
            "#,
        )
        .unwrap();
        let src = r#"
            IMPORT("math", ["pi": "MATH_PI"]);
            MATH_PI;
        "#;
        assert_eq!(run_in(&dir, src).unwrap(), Value::Integer(314));
    }

    #[test]
    fn module_unexported_name_is_e0023() {
        let dir = unique_test_dir("unexported");
        std::fs::write(
            dir.join("m.wl"),
            r#"
                LET(visible, 1);
                LET(hidden, 2);
                EXPORT(["visible"]);
            "#,
        )
        .unwrap();
        let src = r#"
            IMPORT("m", ["hidden"]);
            hidden;
        "#;
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0023);
    }

    #[test]
    fn module_duplicate_import_is_e0021() {
        let dir = unique_test_dir("dup");
        std::fs::write(
            dir.join("m.wl"),
            r#"
                LET(x, 1);
                LET(y, 2);
                EXPORT(["x", "y"]);
            "#,
        )
        .unwrap();
        // Two IMPORTs of the same name in the same scope → E0021.
        let src = r#"
            IMPORT("m", ["x"]);
            IMPORT("m", ["y": "x"]);
        "#;
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0021);
    }

    #[test]
    fn module_not_found_is_e0040() {
        let dir = unique_test_dir("notfound");
        let src = r#"
            IMPORT("doesnotexist", ["x"]);
        "#;
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0040);
    }

    #[test]
    fn module_circular_import_is_e0041() {
        let dir = unique_test_dir("cycle");
        // a.wl imports b.wl, b.wl imports a.wl.
        std::fs::write(
            dir.join("a.wl"),
            r#"
                IMPORT("b", ["y"]);
                LET(x, 1);
                EXPORT(["x"]);
            "#,
        )
        .unwrap();
        std::fs::write(
            dir.join("b.wl"),
            r#"
                IMPORT("a", ["x"]);
                LET(y, 2);
                EXPORT(["y"]);
            "#,
        )
        .unwrap();
        let src = r#"
            IMPORT("a", ["x"]);
            x;
        "#;
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0041);
    }
}
