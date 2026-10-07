//! WLWL 值层(标准库底座 v0.11 / ADR-0022)。
//!
//! 单源承载语言规范 §2.1 的十三类运行时值与全部宿主侧数据结构
//! (`Env` / `ClassEntry` / `TaskHandle` / `ChannelHandle` / 会话协议
//! 光标等),以及 std 原生函数的调用契约([`StdFn`] / [`StdHost`] /
//! [`StdCtx`])。内容自 `wlwl-eval` **纯搬移**,不改任何语义。
//!
//! 依赖方向单向:`wlwl-eval → wlwl-value ← wlwl-std`;本 crate 只依赖
//! `wlwl-ast`(闭包体 AST)与 `wlwl-error`(诊断)。
//!
//! 函数调用行为(如何求值闭包)留在 eval 侧,经 [`StdHost::call`]
//! 注入 —— wlwl-value 只承载值与句柄,不承载求值器。

#![allow(clippy::doc_overindented_list_items)]

use std::cell::{Ref, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use wlwl_ast::{Expr, FunParam, Literal};
use wlwl_error::{ErrorCode, Location, WlwlDiagnostic, WlwlError};

// ──────────────────────────────────────────────────────────────────────
// 运行时值与宿主侧数据(自 wlwl-eval 纯搬移)
// ──────────────────────────────────────────────────────────────────────

// ──────────────────────────────────────────────────────────────────────
// Runtime values
// ──────────────────────────────────────────────────────────────────────

/// Runtime value (v0.3 §2.2 — Phase 2).
///
/// [v0.9 Step 9a-1 / plan §4.3 / ADR-0019 §4.3] PartialEq is hand-
/// implemented (rather than `#[derive]`d) starting this commit:
/// the new `Class` / `Instance` variants wrap `Rc<RefCell<_>>`,
/// which doesn't auto-derive `PartialEq`. Class / Instance equality
/// is by **identity** (`Rc::ptr_eq`), consistent with how the
/// existing handle variants (`TaskHandle`, `ChannelHandle`) compare
/// by their `id` + `generation` fields rather than by deep value.
#[derive(Debug, Clone)]
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
        params: Vec<FunParam>,
        body: Box<Expr>,
        env: Env,
    },
    /// §15 std library function (Phase 4): body is a native Rust impl,
    /// dispatchable like a closure but without a parseable `Expr`. Bound
    /// in the env by `IMPORT("wlwl:std.X", …)` so that the user-supplied
    /// IMPORT takes priority over the `resolve_builtin` fallback.
    NativeFn {
        name: String,
        invoke: StdFn,
    },
    /// §12 OK(value)
    /// §12 OK(value)
    Ok(Box<Value>),
    /// §12 ERR(value)
    Err(Box<Value>),
    /// [v0.7 Phase C2] Handle returned to user code by `SPAWN(fn)`.
    /// AWAIT (C3) takes this value and dereferences it to read
    /// the spawned task's terminal value or error. At C2 the
    /// referenced task has always already finished (synchronous
    /// execution); B5b will let back-pointers capture still-running
    /// tasks and AWAIT will actually wait on them.
    TaskHandle(TaskHandle),
    /// [v0.7 Phase D-A] Handle returned to user code by
    /// `CHANNEL_NEW(buf)`. SEND / RECV / CLOSE / TRY_SEND / TRY_RECV
    /// / LEN / CAP all take this value. Generation-tracked: a slot
    /// recycled by the D-D leak detector bumps the generation so a
    /// stale handle fails E0053-style validation rather than
    /// operating on a different channel than the one the user
    /// originally opened.
    ChannelHandle(ChannelHandle),
    /// [v0.9 Step 9a-1 / plan §4.3 / ADR-0019 §4.3] Class value
    /// produced by `CLASS(name?, parent, members)`. Wrapped in
    /// `Rc<RefCell<_>>` so closure capture (`Env::clone`) can share
    /// the same class entry between the defining scope and any
    /// downstream `NEW` call — parallel to the cell-sharing rule
    /// for `LET MUT` (v0.6 §3.4). Step 9a-2 wires `builtin_class`
    /// to populate the entry; Step 9a-3 will add `init` tracking
    /// (currently a placeholder `Option<Value>`).
    Class(std::rc::Rc<std::cell::RefCell<ClassEntry>>),
    /// [v0.9 Step 9a-1 / plan §4.3 / ADR-0019 §4.3] Instance value
    /// produced by `NEW(cls, args...)`. The `class` field is shared
    /// with the `Value::Class` that created it (same `Rc`); `fields`
    /// is the per-instance `(name → value)` table that `GET_PROP` /
    /// `SET_PROP` read / write (Step 9a-5 wires these). `this_token`
    /// is the placeholder for the linear `THIS` capability (Step
    /// 9a-3 fleshes it out to a runtime-checked `Rc<RefCell<_>>`).
    ///
    /// [v0.9 Step 9a-5] `fields` is wrapped in `Rc<RefCell<_>>`
    /// (rather than the plain `Vec` that 9a-1 used) so that
    /// `SET_PROP` mutations propagate to every reference that
    /// shares the instance. The plain-`Vec` shape meant the
    /// caller had to rebind `LET inst = SET_PROP(inst, "k", v)`
    /// after every mutation — a footgun the §13 ergonomics
    /// call out as a deviation. With the Rc-wrapped shape,
    /// any future `inst.foo` reference observes the new
    /// field automatically, mirroring how closures already
    /// share cells via `Env::clone`.
    Instance {
        class: std::rc::Rc<std::cell::RefCell<ClassEntry>>,
        fields: std::rc::Rc<std::cell::RefCell<Vec<(Value, Value)>>>,
        this_token: std::rc::Rc<std::cell::RefCell<ThisToken>>,
        /// [v0.9 Step 9b] Per-instance session-protocol cursor.
        protocol_state: std::rc::Rc<std::cell::RefCell<ProtocolCursor>>,
    },
}
/// [v0.9 Step 9a-1 / plan §4.3 / ADR-0019 §4.3] Class entry — the
/// shared backing store for `Value::Class` and `Value::Instance`.
/// All fields are public + `Clone`-able so the `Rc<RefCell<_>>`
/// wrapper can be cheaply cloned through `Env::clone` (closure
/// capture). Mutation goes through `RefCell::borrow_mut`; cycle
/// detection in the parent chain lives in `builtin_class` /
/// `builtin_new` (Step 9a-2) — see `E0050` ("class inheritance
/// chain error") for the canonical error path.
///
/// `init` is the optional constructor closure (first parameter
/// named `self` by spec §13 convention); Step 9a-2 populates it
/// when the user passes a closure inside the members array.
#[derive(Debug, Clone)]
pub struct ClassEntry {
    /// Optional class name (user-supplied via the first arg of
    /// `CLASS`). `None` means an anonymous class — useful for
    /// one-shot closures but otherwise discouraged by §13.
    pub name: Option<String>,
    /// Optional parent class (second arg of `CLASS`). Stored via
    /// `Rc<RefCell<_>>` so a class with a parent can outlive its
    /// parent's scope (e.g. when both are top-level lets in the
    /// same module).
    pub parent: Option<std::rc::Rc<std::cell::RefCell<ClassEntry>>>,
    /// Member table: `(name → Value::Closure)` for methods, or
    /// `(name → any)` for static fields. The spec §13 convention
    /// is "first parameter named `self` = instance method; no
    /// `self` parameter = static". Step 9a-4 / 9a-5 walk this
    /// table for method / property dispatch.
    pub members: Vec<(Value, Value)>,
    /// Optional constructor closure. `builtin_new` (Step 9a-2)
    /// reads the first parameter (must be named `self` per §13
    /// convention) and binds it to the freshly-allocated instance.
    pub init: Option<Value>,
    /// [v0.9 Step 9b / plan §4.4.3 / ADR-0019 §14] Session-type
    /// protocol constraining method-call order. `None` = unrestricted.
    pub protocol: Option<Proto>,
}

/// [v0.9 Step 9a-3 / plan §4.3 / ADR-0019 §4.3] Linear `THIS`
/// capability (runtime check).
///
/// `ThisToken` is the per-instance bookkeeping for the spec §15
/// "linear capability" rule: each method body has exactly ONE
/// opportunity to call `THIS`. The first call flips `moved`
/// from `false` to `true`; the second call observes `moved ==
/// true` and raises E0095 ("linear value used after move").
///
/// This is the v0.9.0 minimal "linear" model — stronger than
/// "freely aliasable" but weaker than Rust's compile-time
/// linear typing. The runtime cost is one bool flip per
/// method invocation; spec §15 leaves stronger enforcement
/// (move / discard tracking) as a v0.9.1+ extension.
#[derive(Debug, Clone)]
pub struct ThisToken {
    /// `false` until the first `THIS()` read inside the current
    /// method body, then `true` for the rest of that body's
    /// lifetime. Reset to `false` when the body returns (the
    /// owning instance outlives any single method call, so a
    /// later method invocation on the same instance starts
    /// fresh).
    pub moved: bool,
}

impl ThisToken {
    /// [v0.9 Step 9a-3] Consume the capability: succeed (and
    /// flip `moved = true`) on the first call, fail on any
    /// subsequent call within the same method body. Used by
    /// `builtin_this` to gate the `moved` flag atomically
    /// under the `RefCell::borrow_mut`.
    ///
    /// Returns `Ok(())` for the first call (caller may now
    /// bind / forward the linear reference), `Err(AlreadyMoved)`
    /// for any subsequent call (caller raises E0095 with the
    /// `AlreadyMoved` variant as the source of truth — the
    /// `Result<(), ()>` shape would otherwise trip clippy's
    /// `result_unit_err` lint, and using a named enum keeps
    /// the future direction open if 9a-4+ adds more failure
    /// modes to the linear capability contract).
    pub fn try_consume(&mut self) -> Result<(), ThisTokenError> {
        if self.moved {
            Err(ThisTokenError::AlreadyMoved)
        } else {
            self.moved = true;
            Ok(())
        }
    }
}

/// [v0.9 Step 9a-3] Failure modes for `ThisToken::try_consume`.
/// Currently a single variant (`AlreadyMoved`); the enum
/// exists so the `try_consume` return type isn't
/// `Result<(), ()>` (which clippy flags) and to leave room for
/// additional linear-capability violations in 9a-4 / 9a-5
/// (e.g. `ReadAfterDiscard` for E0096, which is reserved but
/// not yet emitted by the runtime).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThisTokenError {
    /// First `THIS()` call within the method body already
    /// consumed the capability; subsequent reads raise E0095.
    AlreadyMoved,
}

/// [v0.9 P1-M2] Shared fields table of one `Value::Instance`.
pub type LinearThis = std::rc::Rc<std::cell::RefCell<Vec<(Value, Value)>>>;

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
            Value::Boolean(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
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
                format!(
                    "<fun({})>",
                    params
                        .iter()
                        .map(|p| p.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Value::NativeFn { name, .. } => {
                format!("<native fun {}>", name)
            }
            Value::Ok(v) => format!("OK({})", v.display()),
            Value::Err(v) => format!("ERR({})", v.display()),
            // [v0.7 Phase C2] AWAIT (C3) dereferences a TaskHandle;
            // until then, displaying one shows the underlying id
            // + generation so users can recognise stale handles
            // (the v0.6 §3.x trait 'use-after-cancel' analogue).
            Value::TaskHandle(h) => format!("<task handle id={} gen={}>", h.id.0, h.generation),
            // [v0.7 Phase D-A] Channel handles display the same way
            // as task handles for symmetry; the runtime can
            // distinguish via type_name at type-check time.
            Value::ChannelHandle(h) => {
                format!("<channel handle id={} gen={}>", h.id.0, h.generation)
            }
            // [v0.9 Step 9a-1 / plan §4.3] Class display uses the
            // optional user-supplied name when present, otherwise
            // `<class>` for anonymous classes. The address-style
            // disambiguation (`@0x...`) is intentionally omitted
            // because class identity is `Rc::ptr_eq`-based and
            // raw pointer addresses aren't stable across
            // pretty-print passes; the spec §13 prose leaves the
            // exact format open ("the runtime chooses a self-
            // describing label").
            Value::Class(entry) => {
                let b = entry.borrow();
                match &b.name {
                    Some(n) => format!("<class {}>", n),
                    None => "<class>".to_string(),
                }
            }
            // [v0.9 Step 9a-1 / plan §4.3] Instance display shows
            // the class name (when present) plus the field count
            // so two NEW() calls of the same class print distinctly
            // — the field count isn't a stable identity (mutating
            // via SET_PROP doesn't change the display label) but
            // it gives the user a hint at the instance's shape.
            Value::Instance {
                class,
                fields,
                this_token: _,
                protocol_state: _,
            } => {
                let b = class.borrow();
                let label = match &b.name {
                    Some(n) => format!("<{} instance>", n),
                    None => "<instance>".to_string(),
                };
                format!("{}[{} fields]", label, fields.borrow().len())
            }
        }
    }
}

// [v0.9 Step 9a-1 / plan §4.3 / ADR-0019 §4.3] Manual `PartialEq`
// for `Value`. The `#[derive(PartialEq)]` is intentionally NOT used
// (see top-level enum doc) because the new `Class` / `Instance`
// variants wrap `Rc<RefCell<_>>` which doesn't auto-derive
// `PartialEq`. Existing variants keep their original semantics:
// integers / floats / strings / bools by value; arrays / dicts
// element-wise; closures / native-fns by field-wise closure
// equality (params + body + env); result wrappers by inner value;
// handles by their `id` + `generation` fields.
//
// New variants are compared by **identity**:
//   * `Value::Class(a) == Value::Class(b)` iff the two `Rc` point
//     at the same allocation (`Rc::ptr_eq`). Two CLASS() calls in
//     the same scope that produce different classes are NOT equal,
//     even when their members line up — class identity is the
//     user-meaningful comparison (`==` on classes is rare; spec
//     §13 leaves it open).
//   * `Value::Instance` identity is the per-instance state cell
//     (`fields` `Rc::ptr_eq`). Two `NEW()` calls from the same
//     class are distinct objects even when field values line up;
//     `Value::clone` shares the same `fields` allocation and so
//     compares equal (alias of the same object).

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Dict(a), Value::Dict(b)) => a == b,
            (
                Value::Closure {
                    params: pa,
                    body: ba,
                    env: ea,
                },
                Value::Closure {
                    params: pb,
                    body: bb,
                    env: eb,
                },
            ) => pa == pb && ba == bb && ea == eb,
            (Value::NativeFn { name: na, .. }, Value::NativeFn { name: nb, .. }) => na == nb,
            (Value::Ok(a), Value::Ok(b)) => a == b,
            (Value::Err(a), Value::Err(b)) => a == b,
            (Value::TaskHandle(a), Value::TaskHandle(b)) => a == b,
            (Value::ChannelHandle(a), Value::ChannelHandle(b)) => a == b,
            (Value::Class(a), Value::Class(b)) => std::rc::Rc::ptr_eq(a, b),
            (
                Value::Instance {
                    class: _ca,
                    fields: fa,
                    this_token: _ta,
                    protocol_state: _pa,
                },
                Value::Instance {
                    class: _cb,
                    fields: fb,
                    this_token: _tb,
                    protocol_state: _pb,
                },
            ) => std::rc::Rc::ptr_eq(fa, fb),
            _ => false,
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
            // v0.6 §1.8: a literal interpolation cannot appear in a
            // pure `Literal → Value` conversion (interpolation requires
            // evaluating inner expressions, which lives outside
            // `From`). The parser keeps interpolation in a dedicated
            // `Expr::Literal(Interpolated)` AST node that the
            // evaluator handles separately. Map it to a temporary
            // sentinel so the eval layer doesn't accidentally render
            // it as an empty string; full evaluation is implemented
            // in batch 2.
            Literal::Interpolated(_) => Value::String(String::new()),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// Lexical environment (chain of scopes; v0.3 §6.3)
// ──────────────────────────────────────────────────────────────────────

/// Cell payload: a single binding's value plus its mutability flag.
///
/// `mutable` is `false` (IMMUTABLE) when the cell is first created by a
/// `LET` (v0.4 spec 搂6.4 cell model). It is upgraded to `true` when
/// the binding is captured by a closure, per the formal rule
/// `E-CloCap` in 附录 E.4.6:
///
///   "闭包捕获的 cell 全部升级为 MUTABLE"
///
/// `SET` checks the flag and raises E0024 if the cell is still
/// IMMUTABLE.
#[derive(Debug, Clone)]

pub struct Binding {
    pub value: Value,
    pub mutable: bool,
}

/// A heap-allocated cell. Cloning the `Rc` is cheap and is what
/// `Env::clone` does when capturing into a closure's environment --
/// this is exactly the spec's "闭包捕获 cell 引用" rule (single layer,
/// no chain; multiple closures sharing the same lexical `LET` see the
/// same cell).
pub type Cell = Rc<RefCell<Binding>>;

/// Make a new immutable cell wrapping `value`.
pub fn new_cell(value: Value) -> Cell {
    Rc::new(RefCell::new(Binding {
        value,
        mutable: false,
    }))
}

/// Lexical environment. v0.4 搂6.4: stores `HashMap<String, Cell>` so
/// that closures can share the same cell with the lexical scope that
/// defined the binding. Scope chain layout is unchanged from v0.3:
/// index 0 is the innermost scope; lookups walk from inside out.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub scopes: Vec<HashMap<String, Cell>>,
}

// Manual `PartialEq` because `Rc<RefCell<_>>` doesn't derive it well;
// we compare the *value* snapshot for tests that need it.
impl PartialEq for Env {
    fn eq(&self, other: &Self) -> bool {
        if self.scopes.len() != other.scopes.len() {
            return false;
        }
        for (a, b) in self.scopes.iter().zip(other.scopes.iter()) {
            if a.len() != b.len() {
                return false;
            }
            for (k, va) in a {
                match b.get(k) {
                    Some(vb) => {
                        // Compare Rc pointer + value snapshot. We borrow
                        // immutably and compare the inner `Binding` via
                        // `borrow()` snapshots. If either is borrowed
                        // mutably elsewhere this would panic; tests
                        // only call this on quiescent envs.
                        let ba = va.borrow();
                        let bb = vb.borrow();
                        if ba.value != bb.value || ba.mutable != bb.mutable {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
        }
        true
    }
}

impl Env {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
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

    /// Take the entire scope stack out of the Env, leaving it in an
    /// empty-but-valid state (a single empty scope so subsequent
    /// `get`/`set_local` calls don't panic). Used by the segmented
    /// task runner (Phase B5a-3 Path B) to install / save per-segment
    /// env state around mid-body suspension.
    pub fn take_scopes(&mut self) -> Vec<HashMap<String, Cell>> {
        std::mem::take(&mut self.scopes)
    }

    /// Replace the scope stack wholesale. The caller must pass at
    /// least one scope; if `new_scopes` is empty we re-seed with a
    /// single empty scope so the invariant holds.
    pub fn replace_scopes(&mut self, mut new_scopes: Vec<HashMap<String, Cell>>) {
        if new_scopes.is_empty() {
            new_scopes.push(HashMap::new());
        }
        self.scopes = new_scopes;
    }

    /// Walk the scope chain from innermost to outermost; return the
    /// first match as a borrow guard on the inner value. Used for
    /// variable reads. The returned `Ref` is tied to `&self`'s
    /// lifetime; callers typically `.clone()` the inner value or use
    /// the ref briefly within a single expression.
    ///
    /// v0.4 (搂6.4 cell model): the value lives behind an
    /// `Rc<RefCell<Binding>>`; we use `Ref::map` to project a
    /// `Ref<Value>` out of the cell borrow.
    pub fn get(&self, name: &str) -> Option<Ref<'_, Value>> {
        for scope in self.scopes.iter().rev() {
            if let Some(cell) = scope.get(name) {
                return Some(Ref::map(cell.borrow(), |b| &b.value));
            }
        }
        None
    }

    /// Look up the cell (not just the value) for `name`. Used by `SET`
    /// to mutate the cell in place, and by the cell-upgrade path.
    pub fn get_cell(&self, name: &str) -> Option<Cell> {
        for scope in self.scopes.iter().rev() {
            if let Some(cell) = scope.get(name) {
                return Some(cell.clone());
            }
        }
        None
    }

    /// Bind in the current (innermost) scope. The value is wrapped in a
    /// fresh IMMUTABLE cell (per v0.6 §3.1: `LET(name, value)` creates
    /// an immutable binding; use `LET MUT(name, value)` for a mutable
    /// one, via `set_local_mut`).
    pub fn set_local(&mut self, name: impl Into<String>, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.into(), new_cell(value));
        }
    }

    /// v0.6 §3.1: `LET MUT(name, value)` binds a mutable cell. The
    /// mutability flag is set at binding creation time and **never**
    /// changes — there is no closure-capture upgrade (removed from
    /// v0.6; the v0.4/v0.5 "first closure call upgrades cell" rule
    /// was deemed too magical and removed).
    pub fn set_local_mut(&mut self, name: impl Into<String>, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(
                name.into(),
                Rc::new(RefCell::new(Binding {
                    value,
                    mutable: true,
                })),
            );
        }
    }

    /// Set the cell value if the cell is `mutable` (per spec 搂6.4
    /// mutability rule). Returns:
    ///   * `Ok(true)`  -- cell found and updated
    ///   * `Ok(false)` -- cell found but IMMUTABLE (caller raises E0024)
    ///   * `Err(())`   -- cell not found at all (caller raises E0020)
    #[allow(clippy::result_unit_err)] // bool tri-state (Ok/Err-not-found) — see Phase A2 cell semantics
    pub fn set_cell_value(&self, name: &str, value: Value) -> Result<bool, ()> {
        for scope in self.scopes.iter().rev() {
            if let Some(cell) = scope.get(name) {
                let mut b = cell.borrow_mut();
                if !b.mutable {
                    return Ok(false);
                }
                b.value = value;
                return Ok(true);
            }
        }
        Err(())
    }

    /// [v0.9 P1-M2] True when any binding in any scope matches `pred`.
    pub fn any_binding_matches(&self, pred: impl Fn(&Value) -> bool) -> bool {
        for scope in &self.scopes {
            for cell in scope.values() {
                if pred(&cell.borrow().value) {
                    return true;
                }
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
///   * `Yield(reason)` — [v0.7 Phase B5a-3 slice 1] user-defined yield
///                    point. Propagates up the eval stack exactly like
///                    `Break` / `Continue` / `Return`; the
///                    state-machine entry point `Evaluator::step_once`
///                    translates it into
///                    `crate::runtime::StepResult::Yield`(现于 wlwl-eval), while the
///                    run-to-completion `Evaluator::eval` rejects it
///                    with E0014 (yield is only valid inside a
///                    scheduler step loop, not at a bare top-level
///                    call). No v0.6 builtin produces this variant, so
///                    v0.6 programs are observably unaffected.
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    None,
    Break,
    Continue,
    Return(Value),
    Yield(YieldReason),
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
    pub fn normal(v: Value) -> Self {
        Outcome {
            value: v,
            signal: Signal::None,
        }
    }

    /// 带一个**控制流信号**的返回值(如 `Signal::Return`)。
    ///
    /// ⚠️ 跨层调用方**必须**自己决定要不要把信号往上再传 —— `wlwl-std` 的
    /// 成员不得消费它(结构化并发,spec §17):`TIMEOUT` 就是这么用的,它
    /// **接收** body 逃上来的 `Signal::Return` 并把它**换成自己的返回值**。
    pub fn with_signal(v: Value, signal: Signal) -> Self {
        Outcome { value: v, signal }
    }
}

// ──────────────────────────────────────────────────────────────────────
// Module loader (v0.3 §13 — Phase 4 batch 1 + batch 2)
// ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub usize);

/// Generation-tracked task handle returned to user code by `SPAWN`.
///
/// Detecting `use-after-cancel` / `use-after-recycle`: a slot can be
/// reused after a task finishes, but its generation bumps. A handle
/// that doesn't match the current generation is stale (E0053).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskHandle {
    pub id: TaskId,
    pub generation: u64,
}

/// [v0.7 Phase C2] 用户 `SPAWN`/`AWAIT`/通道阻塞时任务挂起的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YieldReason {
    /// User-invoked `YIELD()`. No wait condition; the task is
    /// immediately re-eligible (added back to `run_queue`).
    Explicit,
    /// `AWAIT(child)` and `child` has not finished yet. The task is
    /// parked until `child` transitions to `Done` or `Cancelled`,
    /// at which point `Scheduler::wake_dependents` re-enqueues it.
    AwaitingChild(TaskId),
    /// `CHANNEL_RECV(ch)` and `buf` is empty. The task is parked
    /// on `ch`'s receiver wait list until a sender wakes it.
    ReceivingOn(RcHandle),
    /// `CHANNEL_SEND(ch)` and `buf` is full. The task is parked on
    /// `ch`'s sender wait list until a receiver drains a slot.
    SendingOn(RcHandle),
}

impl YieldReason {
    /// v0.9 Step 10: derive the WasmFX-style [`Tag`] for this reason.
    /// Single source of truth used by `TaskState::Suspended` to
    /// keep `Suspended { tag, reason }` consistent.
    ///
    /// `TaskState` lives in `wlwl-eval::runtime`, i.e. **outside this
    /// crate** — which is why the mention above is plain code and not an
    /// intra-doc link, and why the variant is spelled `Suspended` (an
    /// earlier draft said `suspended`, a name that no longer exists).
    /// [v0.11 M2] `YieldReason` was moved here from `wlwl-eval`; the
    /// reference crossed the crate boundary with it and CI never ran on
    /// the wip branch, so `cargo doc -D warnings` stayed broken until
    /// the branch was added to the trigger list.
    ///
    /// Mapping (plan §3.1 + ADR-0017 §3.1 + ADR-0019 §4.4.1):
    /// - `Explicit` / `AwaitingChild` — the effect is `perform Yield`;
    /// - `ReceivingOn` / `SendingOn` — the effect is
    ///   `perform ChannelOp` (synchronous channel SEND/RECV).
    pub fn tag(self) -> Tag {
        match self {
            YieldReason::Explicit | YieldReason::AwaitingChild(_) => Tag::Yield,
            YieldReason::ReceivingOn(_) | YieldReason::SendingOn(_) => Tag::ChannelOp,
        }
    }
}

/// The algebraic-effect **tag** of a control-flow event: which handler
/// family routes it. The **payload** lives beside the tag, in the
/// `{ tag, payload }` pair the runtime builds (WasmFX Phase 3 /
/// Pretnar & Bauer 2015 shape; the rationale and the tag list live in
/// `wlwl-eval::runtime`). Written as prose, not an intra-doc link: the
/// payload type is declared in `wlwl-eval`, and this crate must not depend
/// on it (dependency direction is `eval → wlwl-value ← std`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tag {
    /// `perform Yield` — collaborative yield point. Equivalent to
    /// `Signal::Yield(YieldReason::Explicit)` in the legacy plumbing.
    Yield,
    /// `perform ChannelOp` — synchronous SEND / RECV blocking on a
    /// channel's buf or wait list. Maps to
    /// `Signal::Yield(YieldReason::ReceivingOn | SendingOn)` in the
    /// legacy plumbing; the runtime routes to the channel's
    /// sender_waiters / receiver_waiters list.
    ChannelOp,
    /// `raise Cancelled` — task was cancelled (with optional reason
    /// payload per ADR-0019 §4.4.2). Maps to `TaskState::Cancelled`
    /// transition; no legacy `Signal` equivalent (cancellation was
    /// previously advisory-only via `cancel_requested` flag).
    Cancelled,
    /// `perform MethodCall` — OOP `CALL_METHOD` control-flow event
    /// (ADR-0019 §4.4.3 / spec §16.4).
    ///
    /// **保留 tag,本版不产生**(D-4 裁决 = 规范明文化;见 spec §17.4 末段)。
    /// 方法调用同步直落,协议违规当场报 `E0050` / `E0051`。保留本变体是为
    /// 了让代数效果后端迁移时 tag 面已就位;实现不得依赖它产生任何可观察行为。
    MethodCall,
    /// `raise ProtocolViolation` — session-type state-machine miss
    /// (保留 tag,本版不产生 —— D-4;见 `MethodCall` 的说明)
    /// (ADR-0019 §4.4.3 / spec §16.4). Reserved naming; surfaced
    /// today as `E0050` / `E0051` diagnostics rather than a handled
    /// effect.
    ProtocolViolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelId(pub usize);

/// Session-type protocol AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Proto {
    End,
    /// Call `method` (optional payload type name), then continue.
    Then {
        method: String,
        payload: Option<String>,
        next: Box<Proto>,
    },
    /// Internal choice `⊕`: caller picks exactly one offered method.
    Choice(Vec<(String, Box<Proto>)>),
    /// `μX. body`.
    Mu(String, Box<Proto>),
    /// Recursion variable `X`.
    Var(String),
}

/// Cursor over a live protocol for one instance.
#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolCursor {
    /// No protocol declared — every method is unrestricted.
    Unrestricted,
    /// Mid-protocol. `steps` bounds μ unfoldings (ADR: μ exhausted → E0051).
    Live {
        proto: Proto,
        /// Unfolding budget for `Mu` (prevents infinite μ expansion).
        fuel: u32,
    },
    /// Protocol completed (`end` reached). Further `CALL_METHOD` →
    /// `ProtocolError::Exhaused` → **E0050**(终态算状态机不匹配,与
    /// step-order slip 的 E0051 分开;spec §14.3.2 优先级,见
    /// `builtin_call_method` 里的映射)。
    Done,
}

/// Default μ-unfolding budget per instance.
pub const DEFAULT_FUEL: u32 = 64;

/// Why a `CALL_METHOD` was rejected by the state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolError {
    /// Method not offered in the current state (sequence order / ⊕ choice).
    NotOffered {
        method: String,
        expected: Vec<String>,
    },
    /// Protocol already at `end`.
    Exhaused,
    /// μ recursion fuel exhausted.
    RecursionExhausted { method: String },
    /// Malformed protocol expression in `CLASS` second arg.
    Syntax(String),
}

impl ProtocolCursor {
    pub fn unrestricted() -> Self {
        ProtocolCursor::Unrestricted
    }

    pub fn live(proto: Proto) -> Self {
        ProtocolCursor::Live {
            proto,
            fuel: DEFAULT_FUEL,
        }
    }

    /// Method names currently offered (for diagnostics).
    pub fn offered(&self) -> Vec<String> {
        match self {
            ProtocolCursor::Unrestricted => vec![],
            ProtocolCursor::Done => vec![],
            ProtocolCursor::Live { proto, .. } => offered_of(proto),
        }
    }

    /// Attempt to perform `method`. On success the cursor advances.
    pub fn step(&mut self, method: &str) -> Result<(), ProtocolError> {
        match self {
            ProtocolCursor::Unrestricted => Ok(()),
            ProtocolCursor::Done => Err(ProtocolError::Exhaused),
            ProtocolCursor::Live { proto, fuel } => {
                let (next, used_fuel) = step_proto(proto, method, *fuel)?;
                *fuel = used_fuel;
                match next {
                    Some(p) => {
                        *proto = p;
                        Ok(())
                    }
                    None => {
                        *self = ProtocolCursor::Done;
                        Ok(())
                    }
                }
            }
        }
    }
}

pub fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Integer(_) => "integer",
        Value::Float(_) => "float",
        Value::String(_) => "string",
        Value::Boolean(_) => "boolean",
        Value::Null => "null",
        Value::Array(_) => "array",
        Value::Dict(_) => "dict",
        Value::Closure { .. } => "function",
        Value::NativeFn { .. } => "native-function",
        Value::Ok(_) => "ok",
        Value::Err(_) => "err",
        // [v0.7 Phase C2] user-facing type name for SPAWN handles;
        // AWAIT (C3) is the only consumer at C2 (where the value
        // never escapes from SPAWN's call site without being
        // dereferenced), so this name is rarely seen. Kept distinct
        // from `function` so a misuse like `+`(handle, 1) gives a
        // readable diagnostic.
        Value::TaskHandle(_) => "task-handle",
        // [v0.7 Phase D-A] user-facing type name for channel handles.
        Value::ChannelHandle(_) => "channel-handle",
        // [v0.9 Step 9a-1 / plan §4.3] OOP type names. Class and
        // Instance are distinct user-visible types so a misuse
        // like `+(class, 1)` produces a readable diagnostic that
        // distinguishes "you gave me a class, expected an
        // instance" from the reverse.
        Value::Class(_) => "class",
        Value::Instance { .. } => "instance",
    }
}

pub fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Integer(_) => "INTEGER",
        Value::Float(_) => "FLOAT",
        Value::String(_) => "STRING",
        Value::Boolean(_) => "BOOLEAN",
        Value::Null => "NULL",
        Value::Array(_) => "ARRAY",
        Value::Dict(_) => "DICT",
        Value::Closure { .. } | Value::NativeFn { .. } => "FUNCTION",
        Value::Ok(_) | Value::Err(_) => "RESULT",
        // [v0.7 Phase C2] distinct user-visible type for SPAWN
        // handles (parallel to function / result).
        Value::TaskHandle(_) => "TASK",
        // [v0.7 Phase D-A] distinct user-visible type for channel
        // handles, parallel to TASK. Spec doesn't fix the name; we
        // use CHANNEL so a `TYPE(ch)` prints a self-describing
        // label without colliding with TASK.
        Value::ChannelHandle(_) => "CHANNEL",
        // [v0.9 Step 9a-1 / plan §4.3] OOP types exposed by TYPE().
        // CLASS / INSTANCE mirror the spec §2.2.1 conventions
        // (parallel to FUNCTION / TASK / CHANNEL — one entry per
        // runtime value kind).
        Value::Class(_) => "CLASS",
        Value::Instance { .. } => "INSTANCE",
    }
}

pub fn values_equal(a: &Value, b: &Value) -> bool {
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
                y.iter()
                    .any(|(yk, yv)| values_equal(xk, yk) && values_equal(xv, yv))
            })
        }
        (Value::Ok(x), Value::Ok(y)) => values_equal(x, y),
        (Value::Err(x), Value::Err(y)) => values_equal(x, y),
        // v0.7 additive identity for generation-tracked handles
        // (same shape as v0.6 §2.4 function instance identity).
        // Without this arm `==(h, h)` fell through to `false`.
        (Value::TaskHandle(x), Value::TaskHandle(y)) => x == y,
        (Value::ChannelHandle(x), Value::ChannelHandle(y)) => x == y,
        // Native std functions compare by bound name (unique per import).
        (Value::NativeFn { name: x, .. }, Value::NativeFn { name: y, .. }) => x == y,
        // Closures: v0.6 §2.4 requires instance reference equality.
        // `Value::Closure` is cloned by value (no Rc), so we cannot
        // recover pointer identity after `LET(g, f)`. Two closures are
        // equal only when every structural field matches (body, params,
        // and Env's PartialEq value-snapshot). This is weaker than §2.4
        // "引用相等" — pre-existing gap, not introduced by v0.7.
        (
            Value::Closure {
                params: p1,
                body: b1,
                env: e1,
            },
            Value::Closure {
                params: p2,
                body: b2,
                env: e2,
            },
        ) => p1 == p2 && b1 == b2 && e1 == e2,
        // [v0.10.1 / R10-012] OOP 身份(spec §2.4,v0.9 起规范性):
        // 「`CLASS`/`INSTANCE` 按**对象身份**恒等 …… 同一实例与其别名相等」。
        //
        // v0.9–v0.10 这两种值落到 `_ => false`,于是连自反律都不成立:
        // `==(a, a)` 与 `==(C, C)` 都返回 FALSE。身份比较的机制本来就有
        // (TaskHandle / ChannelHandle / Closure 三条臂都在),只是这两个类型
        // 漏接了。
        //
        // 身份载体就是各自的 `Rc`:
        // - `Class` 的 `Rc<RefCell<ClassEntry>>` 每次 `CLASS(...)` 新建一个,
        //   别名共享同一个;
        // - `Instance` 的 `fields: Rc<RefCell<Vec<..>>>` 是**每实例**新建的,
        //   `LET(b, a)` 与所有下游引用共享同一个(这正是 v0.9 Step 9a-5 把
        //   `fields` 包进 Rc 的目的 —— `SET_PROP` 要让所有引用都看得到)。
        //
        // 所以 `Rc::ptr_eq` 就是规范说的「对象身份」:同实例相等(含别名),
        // 两次 `NEW` 不等,两次 `CLASS(...)` 不等。
        (Value::Class(x), Value::Class(y)) => std::rc::Rc::ptr_eq(x, y),
        (Value::Instance { fields: fx, .. }, Value::Instance { fields: fy, .. }) => {
            std::rc::Rc::ptr_eq(fx, fy)
        }
        _ => false,
    }
}

// ──────────────────────────────────────────────────────────────────────
// std 调用契约(标准库底座 v0.11 / ADR-0022)
// ──────────────────────────────────────────────────────────────────────

/// std 原生函数签名:直接收发真 [`Value`]`(含闭包),回调经 [`StdHost`]
/// 注入;`Outcome.signal == Signal::Yield(_)` 表示回调挂起,std 层必须
/// 原样穿透,不得消费(结构化并发,spec §17)。
pub type StdFn = fn(&mut dyn StdHost, Vec<Value>) -> Result<Outcome, WlwlError>;

/// std 原生函数看到的宿主。由 wlwl-eval 为其求值器实现;依赖方向
/// `eval → wlwl-value ← wlwl-std` 单向成立(ADR-0022)。
pub trait StdHost {
    /// 每调用级上下文(argv/env/警告槽/测试注册表/入口文件名)。
    fn ctx(&mut self) -> &mut StdCtx;
    /// 调用函数值(闭包 / NativeFn;name 用于诊断定位)。诊断带真实 span 原样上抛。
    fn call(&mut self, f: &Value, args: Vec<Value>, name: &str) -> Result<Outcome, WlwlError>;
    /// 以当前调用点 span 构造诊断。
    fn diag(&mut self, code: ErrorCode, message: String) -> WlwlError;

    // ── 时钟接缝(v0.11.3 M6 / addendum-01 B 片)────────────────────────
    //
    // 形态来自 `ADR-0024` §6.3-2 **候选甲**:`StdHost` **加方法**。
    //
    // ⚠️ **为什么是「加方法」而不是往 `StdCtx` 加字段** —— `addendum-01` §3.10
    // 否掉的 `StdCtx.task_depth` 恰好是**同一个候选的镜像**:那次要问的是
    // 「我在不在任务内」,那是**逐次调用的上下文事实**(与 `argv` / `env` 同类,
    // 于是放 `StdCtx`);这次要问的是「当前处于哪个气泡、虚拟钟走到哪」,那是
    // **调度器状态**,塞进 `StdCtx` 会让「谁拥有时钟」在类型上说不清。
    // **一个判据,两次相反的落点。**
    //
    // **默认值是「不在气泡内 + 真实时钟」**,所以每个手写 `StdHost`(测试里的
    // `NullHost`、基准里的对照宿主)都**不用改**,而它们的存在本身就是在断言
    // 「`std.rand` / 纯计算成员不碰宿主」。

    /// 当前是否处于假时钟气泡内(`TEST_BUBBLE` 之内)。
    fn in_clock_bubble(&self) -> bool {
        false
    }

    /// 当前可见的**墙钟**毫秒(Unix 纪元)。气泡内 = 虚拟钟,否则 = 真实系统时钟。
    fn clock_wall_ms(&self) -> i64 {
        real_wall_ms()
    }

    /// 当前可见的**单调钟**毫秒(进程启动起)。气泡内 = 虚拟钟,否则 = 真实 `Instant`。
    ///
    /// ⚠️ 绝对值无意义、跨进程不可比,只有差值有效(规范 §15.1)。
    fn clock_mono_ms(&self) -> i64 {
        real_mono_ms()
    }

    /// 进入一个假时钟气泡。**已在气泡内 → `E0030`**(气泡不再嵌套,
    /// `ADR-0024` §5-1 业主 2026-10-07 裁决)。成功时虚拟钟**重置到固定起点**。
    ///
    /// 必须与 [`StdHost::exit_clock_bubble`] 成对调用。
    fn enter_clock_bubble(&mut self) -> Result<(), WlwlError> {
        let _ = self;
        Err(self.diag(
            ErrorCode::E0030,
            "clock bubble: this host does not support bubbles".into(),
        ))
    }

    /// 退出当前气泡(与 [`StdHost::enter_clock_bubble`] 成对)。不在气泡内 = 空操作。
    fn exit_clock_bubble(&mut self) {
        let _ = self;
    }

    /// 把虚拟钟拨快 `ms` 毫秒,返回「**最紧的**未放弃 `TIMEOUT` deadline
    /// 是否被越过」。**不在气泡内 → `E0030`**。
    ///
    /// ⚠️ 返回那个 `bool` 是因为 `SLEEP` 靠它决定要不要把 body 从最近的函数
    /// 边界弹回去(`Signal::Return`);让成员自己去查一次「有没有被越过」会
    /// 多一次虚方法调用,而且把「谁判定」这件事在两处各写一遍。
    fn advance_clock(&mut self, ms: i64) -> Result<bool, WlwlError> {
        let _ = ms;
        Err(self.diag(ErrorCode::E0030, "clock bubble: not inside a bubble".into()))
    }

    /// 压入一个 `TIMEOUT` 的 deadline(`at_ms`,虚拟单调域),返回它在栈里的
    /// **深度**。**不在气泡内 → `E0030`**。
    fn push_clock_deadline(&mut self, at_ms: i64) -> Result<usize, WlwlError> {
        let _ = at_ms;
        Err(self.diag(ErrorCode::E0030, "clock bubble: not inside a bubble".into()))
    }

    /// 弹出深度 `depth` 的 deadline,返回**它是否已被放弃过**。
    fn pop_clock_deadline(&mut self, depth: usize) -> bool {
        let _ = depth;
        false
    }
}

/// 真实墙钟:Unix 纪元起的毫秒。**默认实现** —— 只有气泡内的虚拟钟需要宿主接管。
pub fn real_wall_ms() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_millis()).unwrap_or(i64::MAX),
        // 系统时钟早于 1970 是病态情形(改了系统时间 / 容器里 epoch 未初始化)。
        // 不用 `unwrap()` 崩掉一个只读的时钟成员:负毫秒是**可表达**的
        // (减法仍然精确),而崩溃不是。
        Err(e) => -i64::try_from(e.duration().as_millis()).unwrap_or(i64::MAX),
    }
}

/// 真实单调钟:**进程启动起**的毫秒。基准用 `OnceLock` 固定住 —— 否则每次调用都
/// 从一个新基准算起,结果不是单调的,而是「永远等于上一次到现在」。
pub fn real_mono_ms() -> i64 {
    static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let origin = ORIGIN.get_or_init(std::time::Instant::now);
    i64::try_from(origin.elapsed().as_millis()).unwrap_or(i64::MAX)
}

/// 注册进 [`StdCtx::tests`] 的单条测试(name + 零参测试体)。
#[derive(Debug, Clone)]
pub struct TestEntry {
    pub name: String,
    pub body: Value,
}

/// std 原生函数的每调用级上下文(自 wlwl-std 迁入,ADR-0022)。
#[derive(Debug, Clone)]
pub struct StdCtx {
    pub argv: Vec<String>,
    pub env: HashMap<String, String>,
    /// Soft warnings produced by std functions (e.g. `W0052`).
    ///
    /// [D11-019] The comment here used to claim "eval drains it after each
    /// call" — **nothing drains it**. The only readers in the tree are
    /// `wlwl-std`'s own unit tests, so a `W0052` raised by `std.ai` /
    /// `std.agent` is written and never surfaces to the user. Fixing that
    /// means making `invoke_std` drain and re-emit, which is a new
    /// observable behavior (warnings that never appeared would start
    /// appearing) and therefore its own decision, not a doc fix; recorded
    /// in the ledger as D11-022. Until then this field is a write-only sink.
    pub warnings: Vec<(ErrorCode, String)>,
    /// 入口源文件名(诊断定位用),由 eval 在调用前写入。
    pub source_file: String,
    /// `std.test` 注册表:TEST 推入,RUN_TESTS 排空调用。
    pub tests: Vec<TestEntry>,
}

impl Default for StdCtx {
    fn default() -> Self {
        Self {
            argv: Vec::new(),
            env: HashMap::new(),
            warnings: Vec::new(),
            source_file: "<runtime>".into(),
            tests: Vec::new(),
        }
    }
}

impl StdCtx {
    pub fn from_process() -> Self {
        Self {
            argv: std::env::args().collect(),
            env: std::env::vars().collect(),
            warnings: Vec::new(),
            source_file: "<runtime>".into(),
            tests: Vec::new(),
        }
    }

    /// 推送软警告(不改变程序语义)。
    pub fn warn(&mut self, code: ErrorCode, message: impl Into<String>) {
        self.warnings.push((code, message.into()));
    }

    /// 以入口文件定位构造 std 侧诊断。
    pub fn err(&self, code: ErrorCode, message: impl Into<String>) -> WlwlError {
        WlwlDiagnostic::new(
            code,
            message.into(),
            Location::point(&self.source_file, 0, 0),
        )
        .into()
    }
}

/// 通道句柄在调度器语境下的别名(自 runtime.rs 迁入)。
pub type RcHandle = ChannelId;

/// [v0.7 Phase D-A] 通道句柄:`CHANNEL_NEW(buf)` 返回给用户代码;
/// generation 跟踪使回收后的旧句柄按 E0053 判失效。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelHandle {
    pub id: ChannelId,
    pub generation: u64,
}

pub fn offered_of(p: &Proto) -> Vec<String> {
    match p {
        Proto::End => vec![],
        Proto::Var(_) => vec![],
        Proto::Mu(_, body) => offered_of(body),
        Proto::Then { method, next, .. } => {
            if method.is_empty() {
                offered_of(next)
            } else {
                vec![method.clone()]
            }
        }
        Proto::Choice(arms) => arms.iter().map(|(m, _)| m.clone()).collect(),
    }
}

pub(crate) fn step_proto(
    p: &Proto,
    method: &str,
    fuel: u32,
) -> Result<(Option<Proto>, u32), ProtocolError> {
    match p {
        Proto::End => Err(ProtocolError::Exhaused),
        Proto::Var(_) => Err(ProtocolError::NotOffered {
            method: method.to_string(),
            expected: vec![],
        }),
        Proto::Mu(name, body) => {
            if fuel == 0 {
                return Err(ProtocolError::RecursionExhausted {
                    method: method.to_string(),
                });
            }
            let unfolded = subst(body, name, &Proto::Mu(name.clone(), body.clone()));
            step_proto(&unfolded, method, fuel - 1)
        }
        Proto::Then {
            method: expected,
            next,
            ..
        } => {
            if expected.is_empty() {
                // Anonymous payload node — skip.
                return step_proto(next, method, fuel);
            }
            if expected == method {
                let cont = (**next).clone();
                if matches!(cont, Proto::End) {
                    Ok((None, fuel))
                } else {
                    Ok((Some(cont), fuel))
                }
            } else {
                Err(ProtocolError::NotOffered {
                    method: method.to_string(),
                    expected: vec![expected.clone()],
                })
            }
        }
        Proto::Choice(arms) => {
            for (m, cont) in arms {
                if m == method {
                    let cont = (**cont).clone();
                    if matches!(cont, Proto::End) {
                        return Ok((None, fuel));
                    }
                    return Ok((Some(cont), fuel));
                }
            }
            Err(ProtocolError::NotOffered {
                method: method.to_string(),
                expected: arms.iter().map(|(m, _)| m.clone()).collect(),
            })
        }
    }
}

pub fn subst(p: &Proto, name: &str, repl: &Proto) -> Proto {
    match p {
        Proto::Var(v) if v == name => repl.clone(),
        Proto::Var(v) => Proto::Var(v.clone()),
        Proto::End => Proto::End,
        Proto::Then {
            method,
            payload,
            next,
        } => Proto::Then {
            method: method.clone(),
            payload: payload.clone(),
            next: Box::new(subst(next, name, repl)),
        },
        Proto::Choice(arms) => Proto::Choice(
            arms.iter()
                .map(|(m, c)| (m.clone(), Box::new(subst(c, name, repl))))
                .collect(),
        ),
        // Nested μ rebinds the same name → stop (skeleton; no shadowing tests).
        Proto::Mu(v, body) if v == name => Proto::Mu(v.clone(), body.clone()),
        Proto::Mu(v, body) => Proto::Mu(v.clone(), Box::new(subst(body, name, repl))),
    }
}
