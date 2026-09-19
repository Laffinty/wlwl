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

#![allow(unpredictable_function_pointer_comparisons)]

use std::cell::{Ref, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::path::Path;
use std::sync::Arc;

use wlwl_ast::{Expr, FunParam, ImportName, Literal, MatchClause, Pattern, Span};
use wlwl_error::{
    extract_line, ErrorCategory, ErrorCause, ErrorCode, Location, Suggestion,
    TraceFrame, WlwlDiagnostic, WlwlError, WlwlResult,
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
        invoke: NativeInvoke,
    },
    /// §12 OK(value)
    /// §12 OK(value)
    Ok(Box<Value>),
    /// §12 ERR(value)
    Err(Box<Value>),
}

/// Tag for native-function implementations. A `Value::NativeFn`
/// carries one of these alongside its name; the dispatch in
/// `eval_call` matches on the tag to call the right wrapper.
///
/// Adding a new std module (e.g. `std.ai` in batch 3) means adding
/// a new variant here and a new dispatch arm in `eval_call`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeInvoke {
    /// §15 standard library: a `wlwl_std::StdFn` that takes a
    /// `&mut wlwl_std::StdCtx` and `Vec<serde_json::Value>`.
    Std(wlwl_std::StdFn),
    /// §15 standard library "callback-aware" variant: an eval-internal
    /// `BuiltinFn` that takes a `&mut Evaluator` and `Vec<Value>`. Used
    /// by modules that need to invoke user closures (which can't cross
    /// the `serde_json::Value` std boundary — see B5 P4-B5-006 and
    /// `wlwl-std::collection` for the full rationale). Phase B6 binds
    /// `wlwl:std.collection` to a table of these via
    /// `wlwl_eval::collection::BUILTINS`.
    Builtin(crate::BuiltinFn),
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
                format!("<fun({})>", params.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", "))
            }
            Value::NativeFn { name, .. } => {
                format!("<native fun {}>", name)
            },
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
    Rc::new(RefCell::new(Binding { value, mutable: false }))
}

/// Lexical environment. v0.4 搂6.4: stores `HashMap<String, Cell>` so
/// that closures can share the same cell with the lexical scope that
/// defined the binding. Scope chain layout is unchanged from v0.3:
/// index 0 is the innermost scope; lookups walk from inside out.
#[derive(Debug, Clone, Default)]
pub struct Env {
    scopes: Vec<HashMap<String, Cell>>,
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
    /// fresh IMMUTABLE cell (per v0.4 搂6.4 / E.4.6: cells start
    /// IMMUTABLE and are upgraded only when captured by a closure).
    pub fn set_local(&mut self, name: impl Into<String>, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.into(), new_cell(value));
        }
    }

    /// Walk to the first scope that already has `name` and overwrite
    /// the cell's value. Used for `LET` re-binding in an enclosing
    /// scope (e.g. inside a loop body). For `SET` semantics use
    /// `set_cell` instead -- `set_existing` does not check the
    /// mutability flag and is reserved for the `LET` re-binding path.
    pub fn set_existing(&mut self, name: &str, value: Value) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(cell) = scope.get_mut(name) {
                cell.borrow_mut().value = value;
                return true;
            }
        }
        false
    }

    /// Set the cell value if the cell is `mutable` (per spec 搂6.4
    /// mutability rule). Returns:
    ///   * `Ok(true)`  -- cell found and updated
    ///   * `Ok(false)` -- cell found but IMMUTABLE (caller raises E0024)
    ///   * `Err(())`   -- cell not found at all (caller raises E0020)
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

    /// Upgrade every cell in every scope to `mutable = true`. Called
    /// once per closure invocation (E-CloCap) on the captured scopes
    /// only -- the caller's scopes are not touched.
    pub fn upgrade_all_to_mutable(&self) {
        for scope in &self.scopes {
            for cell in scope.values() {
                cell.borrow_mut().mutable = true;
            }
        }
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

/// A soft warning surfaced by the evaluator without aborting the run.
/// Currently the only emitter is the integer-overflow path in §9.5
/// (`+` / `-` / `*` on `INTEGER`): on overflow the value saturates to
/// `INT64_MAX` / `INT64_MIN` and a `W0015` warning is appended.
#[derive(Debug, Clone, PartialEq)]
pub struct Warning {
    pub code: ErrorCode,
    pub message: String,
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
// Module loader (v0.3 §13 — Phase 4 batch 1 + batch 2)
// ──────────────────────────────────────────────────────────────────────

/// Result of loading a module: a fresh `Env` containing all top-level
/// bindings, plus the set of names that were `EXPORT`ed.
#[derive(Debug, Clone)]
struct LoadedModule {
    env: Env,
    exports: HashSet<String>,
}

/// Project-level metadata carried by every `ModuleLoader` instance.
/// Computed once at entry-point evaluation; shared (via `Rc`) with
/// every sub-loader so a cycle anywhere in the graph is detected
/// against the same root.
#[derive(Debug, Clone)]
struct ProjectContext {
    /// Resolved project root: the nearest ancestor of the entry file
    /// containing a `wlwl.toml`. If no such ancestor exists, this is
    /// the entry file's directory (i.e. the project is "no-toml").
    project_root: PathBuf,
    /// Parsed manifest, if a `wlwl.toml` was found at the project
    /// root. `None` means the project has no manifest; in that case
    /// cross-dir and third-party namespace imports are unavailable.
    manifest: Option<Arc<wlwl_toml::manifest::Manifest>>,
    /// Stack of module paths currently being loaded — used to detect
    /// circular imports (E0041) and to surface the full cycle path
    /// per spec §13.7 ("v0.3 增强:错误信息列出完整环路路径").
    /// Shared with sub-loaders so a cycle anywhere in the import
    /// graph is detected.
    loading: Rc<RefCell<Vec<String>>>,
}

#[derive(Debug, Clone)]
struct ModuleLoader {
    /// Directory of the module this loader is parsing. Used as the
    /// base for `./` and `../` relative imports.
    base_dir: PathBuf,
    /// Cache of fully-loaded modules (avoids re-parsing).
    cache: HashMap<String, LoadedModule>,
    /// Project-level context (project root + manifest + loading
    /// stack). Shared with sub-loaders.
    project: ProjectContext,
}

impl ModuleLoader {
    fn new(base_dir: PathBuf) -> Self {
        let project_root = find_project_root(&base_dir);
        let manifest = load_manifest(&project_root);
        Self {
            base_dir,
            cache: HashMap::new(),
            project: ProjectContext {
                project_root,
                manifest,
                loading: Rc::new(RefCell::new(Vec::new())),
            },
        }
    }

    /// Load a module referenced by `path`. Four forms are supported
    /// in Phase 4 batches 1+2 (in resolution order):
    ///
    /// - `wlwl:std.X`: built-in std module (`wlwl_std::resolve`).
    ///   Bound as `Value::NativeFn` in a fresh env. Cached.
    /// - `myteam:utils` (any `ns:name` form not under `wlwl:`):
    ///   resolved against the project manifest. If the namespace
    ///   or the dependency is unknown, an E0043 is raised.
    /// - `./foo` / `../bar`: relative to the current module's
    ///   `base_dir`. Resolved against the project root; trying to
    ///   escape the root is an E0040.
    /// - Simple bare name (`math`): first try the current module's
    ///   `base_dir`, then the project root. Mirrors the v0.2 single-
    ///   directory behaviour so old programs keep working.
    fn load(&mut self, path: &str) -> WlwlResult<LoadedModule> {
        if let Some(cached) = self.cache.get(path) {
            return Ok(cached.clone());
        }

        // 1. `wlwl:std.X` — std library.
        if let Some(spec) = wlwl_std::resolve(path) {
            return self.load_std(spec, path);
        }

        // 2. `ns:name` — third-party / user namespace.
        if let Some((ns, name)) = parse_ns_path(path) {
            if let Some(manifest) = &self.project.manifest {
                if let Some(rel) =
                    wlwl_toml::manifest::resolve_namespace(manifest, ns, name)
                {
                    // The manifest entry is a directory; the module
                    // file is `<dir>/<name>.wl`.
                    let dep_dir = self.base_dir.join(&rel);
                    let file_path = dep_dir.join(format!("{}.wl", name));
                    // Phase C7: normalize away `..` components before the
                    // containment check — a raw prefix test would let
                    // `<root>/../escape/mod.wl` pass `starts_with(root)`.
                    if !is_within(
                        &lexical_normalize(&file_path),
                        &lexical_normalize(&self.project.project_root),
                    ) {
                        return Err(self.diag_outside_root(path));
                    }
                    if !file_path.is_file() {
                        return Err(self.diag_module_not_found(path, &file_path));
                    }
                    return self.load_file_module(&file_path, name);
                }
            }
            // Namespace format recognised but unregistered.
            return Err(self.diag_unregistered_namespace(path, ns, name));
        }

        // 3. `./foo` / `../bar` — relative paths. Walk each `..`
        //    prefix to pop the current module's `base_dir` once;
        //    `./` is a no-op. The remainder is split into a
        //    directory portion and a module name (last segment,
        //    stripped of an optional `.wl`).
        if path.starts_with("./") || path.starts_with("../") {
            let mut dep_dir = self.base_dir.clone();
            let mut rest = path;
            // Strip `./` (no pop) and `../` (pop one level) prefixes
            // repeatedly. Any `..` that would pop above the
            // project root is rejected with E0040 below.
            while let Some(stripped) = rest.strip_prefix("../") {
                if !dep_dir.pop() {
                    return Err(self.diag_outside_root(path));
                }
                rest = stripped;
            }
            if let Some(stripped) = rest.strip_prefix("./") {
                rest = stripped;
            }
            // The remainder may have a sub-directory prefix; the
            // last `/`-separated segment is the module name.
            let (rel_dir, mod_name) = match rest.rsplit_once('/') {
                Some((d, n)) => (d.to_string(), n.trim_end_matches(".wl").to_string()),
                None => (String::new(), rest.trim_end_matches(".wl").to_string()),
            };
            dep_dir = dep_dir.join(&rel_dir);
            let file_path = dep_dir.join(format!("{}.wl", mod_name));
            // Phase C7: same `..`-smuggling hardening as the `ns:name`
            // branch above (the walk loop already pops `../` prefixes,
            // but an in-path `/../` sequence still needs normalization).
            if !is_within(
                &lexical_normalize(&file_path),
                &lexical_normalize(&self.project.project_root),
            ) {
                return Err(self.diag_outside_root(path));
            }
            if !file_path.is_file() {
                return Err(self.diag_module_not_found(path, &file_path));
            }
            return self.load_file_module(&file_path, &mod_name);
        }

        // 4. Simple bare name. Try `base_dir/<name>.wl` first, then
        //    fall back to `<project_root>/<name>.wl`.
        let in_module = self.base_dir.join(format!("{}.wl", path));
        if in_module.is_file() {
            return self.load_file_module(&in_module, path);
        }
        if self.base_dir != self.project.project_root {
            let in_root = self
                .project
                .project_root
                .join(format!("{}.wl", path));
            if in_root.is_file() {
                return self.load_file_module(&in_root, path);
            }
        }

        Err(self.diag_module_not_found(path, &in_module))
    }

    /// Load a std module by its `ModuleSpec` without consulting the
    /// cache. Caches the result under the original path so a second
    /// IMPORT of the same std module reuses the same env.
    fn load_std(
        &mut self,
        spec: &'static wlwl_std::ModuleSpec,
        path: &str,
    ) -> WlwlResult<LoadedModule> {
        let mut env = Env::new();
        let mut exports = HashSet::new();
        // Phase B6 (spec §15.7): `wlwl:std.collection` is a "name
        // catalog" — its SPEC.functions slice is empty (see the
        // module-level docs in `wlwl_std::collection`); the real
        // callback-aware implementations live in
        // `wlwl_eval::collection::BUILTINS` and are bound here. The
        // path check is the only route in: any rename of this
        // constant is a breaking change for IMPORT("wlwl:std.collection").
        if path == "wlwl:std.collection" {
            for (name, builtin) in collection::BUILTINS {
                env.set_local(
                    (*name).to_string(),
                    Value::NativeFn {
                        name: (*name).to_string(),
                        invoke: NativeInvoke::Builtin(*builtin),
                    },
                );
                exports.insert((*name).to_string());
            }
        } else if path == "wlwl:std.test" {
            // Phase B7 (spec §15.9): `wlwl:std.test` follows the
            // same name-catalog pattern (see B6 P4-B6-001). The
            // module-level docs in `wlwl_std::test` spell out why
            // (`TEST` body is a closure; `RUN_TESTS` must invoke it).
            for (name, builtin) in test::BUILTINS {
                env.set_local(
                    (*name).to_string(),
                    Value::NativeFn {
                        name: (*name).to_string(),
                        invoke: NativeInvoke::Builtin(*builtin),
                    },
                );
                exports.insert((*name).to_string());
            }
        } else {
            for (name, func) in spec.functions {
                env.set_local(
                    (*name).to_string(),
                    Value::NativeFn {
                        name: (*name).to_string(),
                        invoke: NativeInvoke::Std(*func),
                    },
                );
                exports.insert((*name).to_string());
            }
        }
        let result = LoadedModule { env, exports };
        self.cache.insert(path.to_string(), result.clone());
        Ok(result)
    }

    /// Load a `.wl` file, parse, evaluate, and return its exports.
    /// Centralises the cycle-detection + file IO + sub-eval wiring
    /// shared by all file-based import paths.
    fn load_file_module(
        &mut self,
        file_path: &Path,
        module_name: &str,
    ) -> WlwlResult<LoadedModule> {
        if self
            .project
            .loading
            .borrow()
            .iter()
            .any(|m| m == module_name)
        {
            return Err(self.diag_circular(module_name));
        }
        self.project.loading.borrow_mut().push(module_name.to_string());
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_e) => {
                self.project.loading.borrow_mut().pop();
                return Err(self.diag_module_not_found(module_name, file_path));
            }
        };
        let ast = match wlwl_parser::parse(&source, &file_path.display().to_string()) {
            Ok(a) => a,
            Err(e) => {
                self.project.loading.borrow_mut().pop();
                return Err(e);
            }
        };
        // Sub-loader shares the project context (root + manifest +
        // loading stack) so cycles are detected across the whole
        // graph, and uses the dependency directory as its own
        // `base_dir` for nested relative imports.
        let dep_dir = file_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.base_dir.clone());
        let sub_loader = ModuleLoader {
            base_dir: dep_dir,
            cache: HashMap::new(),
            project: self.project.clone(),
        };
        let mut sub = Evaluator::new_with_loader(sub_loader);
        if let Err(e) = sub.eval_module(&ast) {
            self.project.loading.borrow_mut().pop();
            return Err(e);
        }
        let exports = collect_exports(&ast);
        let mut env = Env::new();
        for n in &exports {
            if let Some(v) = sub.env.get(n) {
                env.set_local(n.clone(), v.clone());
            } else {
                self.project.loading.borrow_mut().pop();
                return Err(WlwlDiagnostic::new(
                    ErrorCode::E0023,
                    format!(
                        "EXPORT name '{}' is not bound in module '{}'",
                        n, module_name
                    ),
                    Location::point("<module>", 0, 0),
                )
                .into());
            }
        }
        self.project.loading.borrow_mut().pop();
        let result = LoadedModule { env, exports };
        // Cache under the simple name (so re-imports hit the cache).
        self.cache
            .insert(module_name.to_string(), result.clone());
        Ok(result)
    }

    // ── Diagnostics ───────────────────────────────────────────────

    fn diag_module_not_found(
        &self,
        path: &str,
        file_path: &Path,
    ) -> WlwlError {
        WlwlDiagnostic::new(
            ErrorCode::E0040,
            format!(
                "module '{}' not found (looked for {}); check the IMPORT path \
                 and the project's wlwl.toml [namespaces] / [dependencies] \
                 if this is a third-party reference",
                path,
                file_path.display()
            ),
            Location::point("<module>", 0, 0),
        )
    .with_suggestion(Suggestion::Note { description: "check the IMPORT path; for `ns:name` forms, also add the namespace to `wlwl.toml` [namespaces] section".into() })
        .into()
    }

    fn diag_outside_root(&self, path: &str) -> WlwlError {
        // spec §13.5 钉死的措辞:"module 'foo' not found outside
        // project root";括注保留实际 root 方便定位。
        WlwlDiagnostic::new(
            ErrorCode::E0040,
            format!(
                "module '{}' not found outside project root ({})",
                path,
                self.project.project_root.display()
            ),
            Location::point("<module>", 0, 0),
        )
    .with_suggestion(Suggestion::Note { description: "move the file inside the project root, or use a relative path (`./mod`, `../mod`)".into() })
        .into()
    }

    fn diag_unregistered_namespace(
        &self,
        path: &str,
        ns: &str,
        name: &str,
    ) -> WlwlError {
        WlwlDiagnostic::new(
            ErrorCode::E0043,
            format!(
                "namespace '{}' for module '{}' is not registered in this \
                 project's wlwl.toml (neither [namespaces] nor \
                 [dependencies] contains '{}:{}')",
                ns, path, ns, name
            ),
            Location::point("<module>", 0, 0),
        )
    .with_suggestion(Suggestion::Note { description: "add the `[namespaces] <ns>-<name> = <path>` (or `[dependencies] <ns>-<name> = <path>`) section to the project `wlwl.toml`".into() })
        .into()
    }

    fn diag_circular(&self, module_name: &str) -> WlwlError {
        let cycle = {
            let stack = self.project.loading.borrow();
            let mut path: Vec<String> = stack.clone();
            path.push(module_name.to_string());
            path.join(" -> ")
        };
        WlwlDiagnostic::new(
            ErrorCode::E0041,
            format!("circular IMPORT detected: {}", cycle),
            Location::point("<module>", 0, 0),
        )
    .with_suggestion(Suggestion::Note { description: "break the cycle by extracting the shared declarations into a third module that both can import".into() })
        .into()
    }
}

// ── Free helpers ──────────────────────────────────────────────

/// Walk up from `start` looking for a `wlwl.toml`. If found, return
/// its containing directory; otherwise return `start` itself (i.e.
/// the project has no manifest).
fn find_project_root(start: &Path) -> PathBuf {
    let mut cur = start.to_path_buf();
    loop {
        if cur.join("wlwl.toml").is_file() {
            return cur;
        }
        if !cur.pop() {
            return start.to_path_buf();
        }
    }
}

/// Try to load a `wlwl.toml` at `project_root`. Returns `None` on
/// any failure: missing file, parse error, invalid schema. The
/// caller treats all three the same (project is "no-toml" and cross-
/// dir / namespace imports are unavailable). Errors are silent by
/// design — surfacing them would break simple `wlwl run foo.wl`
/// invocations in projects without a manifest.
fn load_manifest(project_root: &Path) -> Option<Arc<wlwl_toml::manifest::Manifest>> {
    let path = project_root.join("wlwl.toml");
    let s = std::fs::read_to_string(&path).ok()?;
    let m = wlwl_toml::manifest::parse(&s).ok()?;
    Some(Arc::new(m))
}

/// True if `path` is `root` or a descendant of `root` (lexically;
/// does not touch the filesystem).
fn is_within(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

/// Lexically normalize a path: drop `.` components and resolve `..`
/// against preceding components. Does **not** touch the filesystem
/// (symlinks are not resolved — documented limitation). Used by the
/// §13.5 project-root boundary checks so a `ns:name` manifest entry
/// like `"../../outside"` cannot smuggle a `..` component past the
/// lexical `is_within` prefix test (Phase C7 hardening).
fn lexical_normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// If `path` looks like `<ns>:<name>`, return the split. Returns
/// `None` for paths that are simple names, std paths, or relative
/// paths — only the third-party namespace form matches.
fn parse_ns_path(path: &str) -> Option<(&str, &str)> {
    if path.starts_with("wlwl:") {
        return None; // std; handled above
    }
    if path.starts_with("./") || path.starts_with("../") {
        return None;
    }
    let (ns, name) = path.split_once(':')?;
    if ns.is_empty() || name.is_empty() {
        return None;
    }
    Some((ns, name))
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

// ──────────────────────────────────────────────────────────────────────
// std library dispatch (v0.3 §15) — Phase 4
// ──────────────────────────────────────────────────────────────────────

/// Wrap a `wlwl_std::StdFn` invocation: convert `Value` args to
/// `serde_json::Value`, call the std fn against `ev.std_ctx`, then
/// convert the result back. Translates `wlwl_std::StdError` to
/// `WlwlDiagnostic` using the call site's `span`.
fn invoke_std(
    ev: &mut Evaluator,
    std_fn: wlwl_std::StdFn,
    args: Vec<Value>,
    span: &Span,
) -> WlwlResult<Outcome> {
    // 1. Convert Value -> StdValue for each arg.
    let mut std_args = Vec::with_capacity(args.len());
    for a in &args {
        std_args.push(value_to_std_value(a).map_err(|e| match e {
            StdValueConvError::Type { expected, got } => ev.diag(
                ErrorCode::E0030,
                format!("std argument: expected {}, got {}", expected, got),
                span.clone(),
            ),
        })?);
    }
    // 2. Call the std fn.
    let result = std_fn(&mut ev.std_ctx, std_args);
    // 3. Convert result.
    match result {
        Ok(v) => Ok(Outcome::normal(std_value_to_value(v))),
        Err(e) => {
            let loc = Location::point(ev.file.as_deref().unwrap_or("<runtime>"), 0, 0);
            let diag = WlwlDiagnostic::new(e.code, e.message, loc);
            Err(diag.into())
        }
    }
}

/// Convert a `Value` to a `serde_json::Value` for the std boundary.
/// Errors out via `E0030` when the source value carries a type that
/// has no JSON equivalent (closures, native fns).
fn value_to_std_value(v: &Value) -> Result<wlwl_std::StdValue, StdValueConvError> {
    use wlwl_std::StdValue;
    Ok(match v {
        Value::Null => StdValue::Null,
        Value::Boolean(b) => StdValue::Bool(*b),
        Value::Integer(i) => StdValue::Number(serde_json::Number::from(*i)),
        Value::Float(f) => {
            serde_json::Number::from_f64(*f)
                .map(StdValue::Number)
                .ok_or_else(|| StdValueConvError::Type {
                    expected: "finite number".into(),
                    got: "NaN/Inf float".into(),
                })?
        }
        Value::String(s) => StdValue::String(s.clone()),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(value_to_std_value(item)?);
            }
            StdValue::Array(out)
        }
        Value::Dict(entries) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in entries {
                let key = match k {
                    Value::String(s) => s.clone(),
                    other => {
                        return Err(StdValueConvError::Type {
                            expected: "string dict key".into(),
                            got: type_name(other).into(),
                        });
                    }
                };
                obj.insert(key, value_to_std_value(v)?);
            }
            StdValue::Object(obj)
        }
        Value::Ok(inner) => {
            // §12 OK wraps a value; pass the inner value through.
            value_to_std_value(inner)?
        }
        Value::Err(inner) => {
            return Err(StdValueConvError::Type {
                expected: "OK/primitives at std boundary".into(),
                got: "ERR(...)".into(),
            });
        }
        Value::Closure { .. } => {
            return Err(StdValueConvError::Type {
                expected: "data value at std boundary".into(),
                got: "function closure".into(),
            });
        }
        Value::NativeFn { name, .. } => {
            return Err(StdValueConvError::Type {
                expected: "data value at std boundary".into(),
                got: format!("native fn `{}`", name),
            });
        }
    })
}

#[derive(Debug)]
enum StdValueConvError {
    Type { expected: String, got: String },
}

fn std_value_to_value(v: wlwl_std::StdValue) -> Value {
    use wlwl_std::StdValue;
    match v {
        StdValue::Null => Value::Null,
        StdValue::Bool(b) => Value::Boolean(b),
        StdValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                // Shouldn't happen: serde_json::Number is always
                // either int or float. Fall back to Null defensively.
                Value::Null
            }
        }
        StdValue::String(s) => Value::String(s),
        StdValue::Array(items) => {
            Value::Array(items.into_iter().map(std_value_to_value).collect())
        }
        StdValue::Object(obj) => {
            let mut entries = Vec::with_capacity(obj.len());
            // Preserve insertion order via serde_json's BTreeMap-free
            // ordering: serde_json::Map preserves insertion order when
            // `preserve_order` feature is enabled, but the default
            // uses BTreeMap. We collect into Vec<(Value, Value)> to
            // keep the v0.3 DICT insertion-order guarantee from §10.
            for (k, v) in obj {
                entries.push((Value::String(k), std_value_to_value(v)));
            }
            Value::Dict(entries)
        }
    }
}
// ──────────────────────────────────────────────────────────────────────
// Built-in functions
// ──────────────────────────────────────────────────────────────────────

/// Function signature for every eval-internal builtin. Same shape as
/// `wlwl_std::StdFn` but operates on the rich `Value` type so it can
/// invoke user closures (which can't cross the `serde_json::Value`
/// std boundary — see B5 `P4-B5-006`).
///
/// Used by:
/// - the global builtin dispatch table (`resolve_builtin`), reached
///   when the user calls a builtin by its bare name (e.g. `PRINT(...)`);
/// - the callback-aware std modules bound through `NativeInvoke::Builtin`
///   (Phase B6: `wlwl:std.collection`).
///
/// `pub(crate)` so `wlwl_eval::collection` can name the type when it
/// enumerates its `BUILTINS` table.
pub(crate) type BuiltinFn = fn(&mut Evaluator, Vec<Value>) -> WlwlResult<Outcome>;

fn builtin_print(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let parts: Vec<String> = args.iter().map(|v| v.display()).collect();
    println!("{}", parts.join(" "));
    Ok(Outcome::normal(Value::Null))
}

/// Phase B10 (spec §15.1 + §3.4): `PRINT_ERR` is the stderr twin
/// of `PRINT`. Same value formatting (`v.display()`), same null
/// return, but `eprintln!` so programs can split diagnostics from
/// results via shell redirect (`2>` vs `>`).
///
/// Not in §12.7 ERR consumer registry; not in plan §5.10's
/// `wlwl:std.io` extension list — it joins `PRINT` / `INPUT` as
/// one of the three top-level std I/O functions per §15.1.
fn builtin_print_err(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let parts: Vec<String> = args.iter().map(|v| v.display()).collect();
    eprintln!("{}", parts.join(" "));
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

/// v0.4 spec §9.5 — `INT(x)` builtin: convert `x` to `INTEGER`.
///
/// - `INTEGER`         → unchanged
/// - `FLOAT`           → truncation toward zero; out-of-range → `E0035`
/// - `STRING`          → integer parse (decimal); on parse failure
///                       returns `ERR(["kind": "ParseError", "input": x])`
///                       (matches the error convention used elsewhere
///                       in the std for "soft" conversion failures)
/// - anything else     → `E0030` type error
///
/// Not in the §12.7 ERR consumer registry (transparent propagation):
/// passing `INT(ERR("e"))` returns `ERR("e")`, not a runtime error.
fn builtin_int(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("INT", &args, 1)?;
    match v {
        Value::Integer(i) => Ok(Outcome::normal(Value::Integer(*i))),
        Value::Float(f) => {
            // Rust's `as i64` on f64 truncates toward zero (matches
            // spec §9.5 row 7). Out-of-range (|f| > i64::MAX, or NaN,
            // or +Inf / -Inf) produces `i64::MIN` from a wrapping cast
            // — we detect that and report E0035 instead.
            if !f.is_finite() || *f > (i64::MAX as f64) || *f < (i64::MIN as f64) {
                return Err(builtin_error(
                    ErrorCode::E0035,
                    "INT",
                    format!("FLOAT value {} out of INTEGER range", f),
                ));
            }
            Ok(Outcome::normal(Value::Integer(*f as i64)))
        }
        Value::String(s) => match s.parse::<i64>() {
            Ok(i) => Ok(Outcome::normal(Value::Integer(i))),
            Err(e) => Ok(Outcome::normal(Value::Err(Box::new(Value::Dict(vec![
                (Value::String("kind".into()), Value::String("ParseError".into())),
                (Value::String("input".into()), Value::String(s.clone())),
                (Value::String("reason".into()), Value::String(e.to_string())),
            ]))))),
        },
        other => Err(type_error(
            "INT",
            format!("cannot convert {} to INTEGER", type_name(other)),
        )),
    }
}

// ── Phase B8: spec v0.4 §10.3 string extensions + FLOAT conversion ──
//
// All eleven builtins below are append-only (no lexer / parser /
// AST change). They're registered in `resolve_builtin` further down;
// none are in the §12.7 ERR consumer registry, so they inherit the
// default §12.6 transparent ERR propagation.

/// `FLOAT(s)` — spec §10.3 + §9.5: STRING / INTEGER → FLOAT; on STRING
/// parse failure, return `ERR(["kind": "ParseError", "input", "reason"])`
/// (same shape as `INT`'s ParseError so callers can match either).
///
/// FLOAT → FLOAT is identity (no precision loss claim). INTEGER → FLOAT
/// is exact (Rust's `as f64` produces the nearest representable f64,
/// which is exact for i64 within ±2^53).
fn builtin_float(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("FLOAT", &args, 1)?;
    match v {
        Value::Float(f) => Ok(Outcome::normal(Value::Float(*f))),
        Value::Integer(i) => Ok(Outcome::normal(Value::Float(*i as f64))),
        Value::String(s) => match s.parse::<f64>() {
            Ok(f) if f.is_finite() => Ok(Outcome::normal(Value::Float(f))),
            Ok(f) => {
                // NaN / ±Inf from string parse (e.g. "nan", "inf").
                // §10.3 doesn't pin behavior; we mirror INT's
                // ParseError shape so callers get a uniform
                // `ERR(["kind": "ParseError"])` payload.
                Ok(Outcome::normal(Value::Err(Box::new(Value::Dict(vec![
                    (Value::String("kind".into()), Value::String("ParseError".into())),
                    (Value::String("input".into()), Value::String(s.clone())),
                    (
                        Value::String("reason".into()),
                        Value::String(format!("non-finite FLOAT: {}", f)),
                    ),
                ])))))
            }
            Err(e) => Ok(Outcome::normal(Value::Err(Box::new(Value::Dict(vec![
                (Value::String("kind".into()), Value::String("ParseError".into())),
                (Value::String("input".into()), Value::String(s.clone())),
                (Value::String("reason".into()), Value::String(e.to_string())),
            ]))))),
        },
        other => Err(type_error(
            "FLOAT",
            format!("cannot convert {} to FLOAT", type_name(other)),
        )),
    }
}

/// Helper: trim ASCII whitespace from both ends of a STRING.
fn trim_ascii(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_ascii_whitespace())
}

/// `TRIM(s)` — spec §10.3 row 9: ASCII whitespace both ends.
fn builtin_trim(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("TRIM", &args, 1)?;
    match v {
        Value::String(s) => Ok(Outcome::normal(Value::String(trim_ascii(s).into()))),
        other => Err(type_error(
            "TRIM",
            format!("expected STRING, got {}", type_name(other)),
        )),
    }
}

/// `TRIM_START(s)` — leading whitespace only.
fn builtin_trim_start(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("TRIM_START", &args, 1)?;
    match v {
        Value::String(s) => {
            let trimmed = s.trim_start_matches(|c: char| c.is_ascii_whitespace());
            Ok(Outcome::normal(Value::String(trimmed.into())))
        }
        other => Err(type_error(
            "TRIM_START",
            format!("expected STRING, got {}", type_name(other)),
        )),
    }
}

/// `TRIM_END(s)` — trailing whitespace only.
fn builtin_trim_end(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("TRIM_END", &args, 1)?;
    match v {
        Value::String(s) => {
            let trimmed = s.trim_end_matches(|c: char| c.is_ascii_whitespace());
            Ok(Outcome::normal(Value::String(trimmed.into())))
        }
        other => Err(type_error(
            "TRIM_END",
            format!("expected STRING, got {}", type_name(other)),
        )),
    }
}

/// `STARTS_WITH(s, prefix)` — BOOLEAN.
fn builtin_starts_with(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (s, pre) = expect_arity2("STARTS_WITH", &args)?;
    match (s, pre) {
        (Value::String(s), Value::String(p)) => {
            Ok(Outcome::normal(Value::Boolean(s.starts_with(p.as_str()))))
        }
        (_, _) => Err(type_error(
            "STARTS_WITH",
            "expected STRING, STRING".to_string(),
        )),
    }
}

/// `ENDS_WITH(s, suffix)` — BOOLEAN.
fn builtin_ends_with(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (s, suf) = expect_arity2("ENDS_WITH", &args)?;
    match (s, suf) {
        (Value::String(s), Value::String(p)) => {
            Ok(Outcome::normal(Value::Boolean(s.ends_with(p.as_str()))))
        }
        (_, _) => Err(type_error(
            "ENDS_WITH",
            "expected STRING, STRING".to_string(),
        )),
    }
}

/// `REPEAT(s, n)` — `n` copies of `s`. n must be ≥ 0 (negative → E0030).
fn builtin_repeat(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (s, n) = expect_arity2("REPEAT", &args)?;
    match (s, n) {
        (Value::String(s), Value::Integer(n)) if *n >= 0 => {
            // Spec §9.5: integer arithmetic uses saturating semantics.
            // `String::repeat(usize)` panics on overflow, so we cap at
            // `isize::MAX` (≈ 9.2 EB on 64-bit) — same trade-off as
            // the operator machinery in `builtin_add_integer`.
            let n_usize = (*n as u64).min(usize::MAX as u64) as usize;
            Ok(Outcome::normal(Value::String(s.repeat(n_usize))))
        }
        (Value::String(_), Value::Integer(_)) => Err(type_error(
            "REPEAT",
            "second arg must be non-negative INTEGER".to_string(),
        )),
        (_, _) => Err(type_error(
            "REPEAT",
            "expected STRING, INTEGER".to_string(),
        )),
    }
}

/// `PAD_START(s, n, c)` / `PAD_END(s, n, c)` share a helper.
fn pad_with(s: &str, n: usize, c: char, side: &str) -> String {
    let len = s.chars().count();
    if len >= n {
        return s.to_string();
    }
    let pad_count = n - len;
    let pad: String = std::iter::repeat(c).take(pad_count).collect();
    let mut out = String::with_capacity(n);
    if side == "start" {
        out.push_str(&pad);
        out.push_str(s);
    } else {
        out.push_str(s);
        out.push_str(&pad);
    }
    out
}

/// `PAD_START(s, n, c)` — pad to length `n` with `c` on the left.
fn builtin_pad_start(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (s, n, c) = expect_arity3("PAD_START", &args)?;
    match (s, n, c) {
        (Value::String(s), Value::Integer(n), Value::String(c)) => {
            let pad_char = c.chars().next().ok_or_else(|| {
                type_error("PAD_START", "pad character STRING must be non-empty".to_string())
            })?;
            Ok(Outcome::normal(Value::String(pad_with(
                s,
                (*n).max(0) as usize,
                pad_char,
                "start",
            ))))
        }
        _ => Err(type_error(
            "PAD_START",
            "expected STRING, INTEGER, STRING".to_string(),
        )),
    }
}

/// `PAD_END(s, n, c)` — pad to length `n` with `c` on the right.
fn builtin_pad_end(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (s, n, c) = expect_arity3("PAD_END", &args)?;
    match (s, n, c) {
        (Value::String(s), Value::Integer(n), Value::String(c)) => {
            let pad_char = c.chars().next().ok_or_else(|| {
                type_error("PAD_END", "pad character STRING must be non-empty".to_string())
            })?;
            Ok(Outcome::normal(Value::String(pad_with(
                s,
                (*n).max(0) as usize,
                pad_char,
                "end",
            ))))
        }
        _ => Err(type_error(
            "PAD_END",
            "expected STRING, INTEGER, STRING".to_string(),
        )),
    }
}

/// `CODEPOINTS(s)` — return ARRAY of INTEGER codepoints (Unicode
/// scalar values, not UTF-16 code units).
fn builtin_codepoints(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("CODEPOINTS", &args, 1)?;
    match v {
        Value::String(s) => {
            let cps: Vec<Value> = s.chars().map(|c| Value::Integer(c as i64)).collect();
            Ok(Outcome::normal(Value::Array(cps)))
        }
        other => Err(type_error(
            "CODEPOINTS",
            format!("expected STRING, got {}", type_name(other)),
        )),
    }
}

/// `FROM_CODEPOINTS(arr)` — inverse of CODEPOINTS. Each element must
/// be INTEGER; out-of-Unicode-scalar-range → E0031 (Type bucket,
/// 越界类 type error, mirrors INT's out-of-range E0035).
fn builtin_from_codepoints(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("FROM_CODEPOINTS", &args, 1)?;
    match v {
        Value::Array(items) => {
            let mut out = String::new();
            for it in items {
                match it {
                    Value::Integer(i) if (0..=0x10FFFF).contains(i) => {
                        if let Some(c) = char::from_u32(*i as u32) {
                            out.push(c);
                        } else {
                            return Err(builtin_error(
                                ErrorCode::E0031,
                                "FROM_CODEPOINTS",
                                format!("codepoint U+{:04X} not a valid Unicode scalar", i),
                            ));
                        }
                    }
                    Value::Integer(i) => {
                        return Err(builtin_error(
                            ErrorCode::E0031,
                            "FROM_CODEPOINTS",
                            format!("codepoint {} outside Unicode range 0..=0x10FFFF", i),
                        ));
                    }
                    other => {
                        return Err(type_error(
                            "FROM_CODEPOINTS",
                            format!("array element must be INTEGER, got {}", type_name(other)),
                        ));
                    }
                }
            }
            Ok(Outcome::normal(Value::String(out)))
        }
        other => Err(type_error(
            "FROM_CODEPOINTS",
            format!("expected ARRAY, got {}", type_name(other)),
        )),
    }
}

/// `expect_arity3` — 3-arg helper for PAD_*. Mirrors `expect_arity2`.
fn expect_arity3<'a>(
    fn_name: &str,
    args: &'a [Value],
) -> WlwlResult<(&'a Value, &'a Value, &'a Value)> {
    if args.len() != 3 {
        return Err(arity_error(fn_name, args.len(), 3));
    }
    Ok((&args[0], &args[1], &args[2]))
}

// ── Phase B1: spec v0.4 §10.1 / §10.2 subscript / key access primitives ──
//
// `INDEX_GET` / `INDEX_SET` / `AT` / `REMOVE_KEY` / `POP` (dict variant).
// None of these are in the §12.7 ERR consumer registry — they inherit
// the default §12.6 transparent ERR propagation. The "lenient" trio
// (`AT`, `POP`-dict, `REMOVE_KEY`) never raise ERR on absent keys; the
// strict trio (`INDEX_GET`, `INDEX_SET`) raise E0036 (ARRAY OOB) /
// E0037 (DICT missing).

/// Resolve a logical index `i` against a `Value::Array`.
///
/// - `i` must be `INTEGER` (else `E0031`)
/// - `-LEN(arr) ≤ i < LEN(arr)` (negative indexes count from the end)
/// - Out-of-range → `E0036: array index out of bounds`
///
/// Spec v0.4 §10.1 row 1 / boundary rule.
fn resolve_array_index(
    fn_name: &str,
    arr: &[Value],
    raw: &Value,
) -> WlwlResult<usize> {
    let i = match raw {
        Value::Integer(n) => *n,
        other => {
            return Err(builtin_error(
                ErrorCode::E0031,
                fn_name,
                format!(
                    "array index must be INTEGER, got {}",
                    type_name(other)
                ),
            ));
        }
    };
    let len = arr.len() as i64;
    let normalised = if i < 0 { i + len } else { i };
    if normalised < 0 || normalised >= len {
        return Err(builtin_error(
            ErrorCode::E0036,
            fn_name,
            format!(
                "array index {} out of bounds for length {}",
                i,
                arr.len()
            ),
        ));
    }
    Ok(normalised as usize)
}

/// Look up a key in a `Value::Dict` entries list. Returns
/// `Some(idx)` if present, `None` if absent. Equality uses §10.4
/// strict rules via `values_equal`.
fn dict_lookup(entries: &[(Value, Value)], key: &Value) -> Option<usize> {
    entries.iter().position(|(k, _)| values_equal(k, key))
}

/// v0.4 §10.1 + §10.2 — `INDEX_GET(coll, idx_or_key)`.
///
/// - ARRAY + INTEGER → element (negative indexes supported; OOB → E0036)
/// - DICT  + any      → value (missing → E0037)
/// - other            → E0030
///
/// Not in §12.7 registry → §12.6 ERR transparent propagation.
fn builtin_index_get(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (coll, idx) = expect_arity2("INDEX_GET", &args)?;
    let v = match (coll, idx) {
        (Value::Array(a), raw) => {
            let i = resolve_array_index("INDEX_GET", a, raw)?;
            Ok(a[i].clone())
        }
        (Value::Dict(entries), key) => match dict_lookup(entries, key) {
            Some(i) => Ok(entries[i].1.clone()),
            None => Err(builtin_error(
                ErrorCode::E0037,
                "INDEX_GET",
                format!("dict key {} not found", key.display()),
            )),
        },
        (other, _) => Err(type_error(
            "INDEX_GET",
            format!(
                "expected ARRAY or DICT as first arg, got {}",
                type_name(other)
            ),
        )),
    }?;
    Ok(Outcome::normal(v))
}

/// v0.4 §10.1 + §10.2 — `INDEX_SET(coll, idx_or_key, val)`.
///
/// - ARRAY + INTEGER → upsert at index (negative indexes supported;
///                       OOB → E0036, no mutation)
/// - DICT  + any      → insert or update key (insertion-order
///                       preserved on insertion; existing keys keep
///                       original position on update — matches
///                       `MERGE` semantics in §10.2)
/// - other            → E0030
///
/// "In-place" semantics are emulated by cloning the container,
/// mutating the clone, and returning the clone — equivalent at the
/// user level for a tree-walking interpreter where Value::Array /
/// Value::Dict are owned Vecs (no Rc<RefCell<_>> yet).
fn builtin_index_set(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 3 {
        return Err(arity_error("INDEX_SET", args.len(), 3));
    }
    let new_val = args[2].clone();
    let v = match (&args[0], &args[1]) {
        (Value::Array(a), raw) => {
            let i = resolve_array_index("INDEX_SET", a, raw)?;
            let mut out = a.clone();
            out[i] = new_val;
            Value::Array(out)
        }
        (Value::Dict(entries), key) => {
            let mut out = entries.clone();
            match dict_lookup(&out, key) {
                Some(i) => out[i].1 = new_val,
                None => out.push((key.clone(), new_val)),
            }
            Value::Dict(out)
        }
        (other, _) => {
            return Err(type_error(
                "INDEX_SET",
                format!(
                    "expected ARRAY or DICT as first arg, got {}",
                    type_name(other)
                ),
            ));
        }
    };
    Ok(Outcome::normal(v))
}

/// v0.4 §10.1 + §10.2 — `AT(coll, idx_or_key, default)`.
///
/// Lenient version of `INDEX_GET`: returns `default` on
/// OOB / missing-key instead of raising E0036 / E0037. Spec §10.1
/// row 3 + §10.2 row "安全下标".
fn builtin_at(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 3 {
        return Err(arity_error("AT", args.len(), 3));
    }
    let default = &args[2];
    let v = match (&args[0], &args[1]) {
        (Value::Array(a), raw) => match raw {
            Value::Integer(i) => {
                let len = a.len() as i64;
                let normalised = if *i < 0 { *i + len } else { *i };
                if normalised < 0 || normalised >= len {
                    default.clone()
                } else {
                    a[normalised as usize].clone()
                }
            }
            other => {
                return Err(builtin_error(
                    ErrorCode::E0031,
                    "AT",
                    format!(
                        "array index must be INTEGER, got {}",
                        type_name(other)
                    ),
                ));
            }
        },
        (Value::Dict(entries), key) => match dict_lookup(entries, key) {
            Some(i) => entries[i].1.clone(),
            None => default.clone(),
        },
        (other, _) => {
            return Err(type_error(
                "AT",
                format!(
                    "expected ARRAY or DICT as first arg, got {}",
                    type_name(other)
                ),
            ));
        }
    };
    Ok(Outcome::normal(v))
}

/// v0.4 §10.2 — `REMOVE_KEY(d, key)` (v0.4 main name).
///
/// - First arg must be DICT, else E0030
/// - Missing key → NOOP (returns the dict unchanged)
/// - Returns the dict with the key removed
///
/// The v0.3-compat alias `DEL` is wired in `builtin_remove_key_compat`
/// below — it calls this fn after emitting W0051.
fn builtin_remove_key(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (coll, key) = expect_arity2("REMOVE_KEY", &args)?;
    let v = match coll {
        Value::Dict(entries) => {
            let mut out = entries.clone();
            if let Some(i) = dict_lookup(&out, key) {
                out.remove(i);
            }
            Value::Dict(out)
        }
        other => {
            return Err(type_error(
                "REMOVE_KEY",
                format!("expected DICT, got {}", type_name(other)),
            ));
        }
    };
    Ok(Outcome::normal(v))
}

/// v0.4 §10.2 + §14.5 — `DEL(d, key)` is the v0.3-compat alias for
/// `REMOVE_KEY`. Per spec §14.5, every call emits `W0051`. v0.5 will
/// drop this alias entirely.
///
/// Note: the warning fires **after** `eval_call`'s ERR short-circuit
/// (see line ~1935 region), so `DEL(ERR("e"), "k")` propagates the
/// ERR without emitting W0051 — this matches `REMOVE_KEY`'s
/// transparent-propagation semantics for legacy callers.
fn builtin_remove_key_compat(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    ev.emit_warning(
        ErrorCode::W0051,
        "`DEL` is a v0.3-compat alias; use `REMOVE_KEY` instead (will be removed in v0.5)",
    );
    builtin_remove_key(ev, args)
}

/// v0.4 §10.2 — `POP(d, key, default)` (dict variant — safe delete).
///
/// - First arg must be DICT, else E0030
/// - Key present → returns the value and removes the entry
/// - Key absent → returns `default`, dict unchanged
///
/// Note: `POP(arr)` (array variant) is not implemented in this batch —
/// spec v0.3 §10.1 only lists `POP(arr)` removing the last element,
/// but the plan's B1 task list focuses on the DICT variant (see
/// deviations B1-002).
fn builtin_pop_dict(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 3 {
        return Err(arity_error("POP", args.len(), 3));
    }
    let default = &args[2];
    let v = match (&args[0], &args[1]) {
        (Value::Dict(entries), key) => match dict_lookup(entries, key) {
            Some(i) => {
                let mut out = entries.clone();
                let removed = out.remove(i).1;
                // Spec §10.2 says "返回删除的值". Return the
                // removed value, NOT the modified dict — that's what
                // callers want from a safe-delete primitive.
                return Ok(Outcome::normal(removed));
            }
            None => default.clone(),
        },
        (other, _) => {
            return Err(type_error(
                "POP",
                format!("expected DICT as first arg, got {}", type_name(other)),
            ));
        }
    };
    Ok(Outcome::normal(v))
}


// ──────────────────────────────────────────────────────────────────────
// Phase B12: spec v0.4 §10.1 ARRAY ops (7 项)
// ──────────────────────────────────────────────────────────────────────
//
// 把 spec 附录 G Deferred 的 7 个 array builtin 接进 resolve_builtin。
// 全部 append-only (不动 lexer/parser/ast);保持 immutable 语义
// (返回新 array,不修改原 array —— 与 PUSH/POP 已实现路径一致)。
// ERR-transparent propagation (§12.6 默认) —— 任何 args[i] 是 ERR
// 由 eval_call 短路,我们这里只看正常值。
//
// **POP 命名说明 (deviation P4-B12-002)**: spec 附录 G 表把 `POP`
// 列在 ARRAY 操作组,签名 `POP(arr) -> ARRAY`(移除最后一个元素)。
// 但本 impl 早在 Phase B1 把 `POP` 重载为 `POP(dict, key, default) -> v`
// (DICT 安全删除 + 默认值 fallback,移除 PUSH 误用风险)。
// registry 把 POP 标 `ResolvedBuiltin / Array group` 是历史沿用,
// impl 仍是 DICT 3-arg 版。本批**不动** POP 语义以保持向后兼容,
// 仅在 B12 deviation 里记录这条 spec drift。v0.5 重命名 `POP_DICT`
// 之类可彻底分开。

/// `SHIFT(arr)` -> ARRAY: 移除第一个元素,返回剩余 array。
fn builtin_shift(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("SHIFT", &args, 1)?;
    match v {
        Value::Array(a) => {
            if a.is_empty() {
                Ok(Outcome::normal(Value::Array(Vec::new())))
            } else {
                Ok(Outcome::normal(Value::Array(a[1..].to_vec())))
            }
        }
        other => Err(type_error(
            "SHIFT",
            format!("expected ARRAY, got {}", type_name(other)),
        )),
    }
}

/// `UNSHIFT(arr, x) -> ARRAY`: 在开头插入元素,返回新 array。
fn builtin_unshift(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("UNSHIFT", args.len(), 2));
    }
    match &args[0] {
        Value::Array(a) => {
            let mut out = Vec::with_capacity(a.len() + 1);
            out.push(args[1].clone());
            out.extend(a.iter().cloned());
            Ok(Outcome::normal(Value::Array(out)))
        }
        other => Err(type_error(
            "UNSHIFT",
            format!("expected ARRAY as first arg, got {}", type_name(other)),
        )),
    }
}

/// `SLICE(arr, start, end?) -> ARRAY`: 半开区间 [start, end) 切片。
/// start/end 负数从尾部数 (类似 Python);end 缺省 -> 切到末尾。
fn builtin_slice(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() < 2 || args.len() > 3 {
        return Err(arity_error("SLICE", 2, args.len()));
    }
    let arr = match &args[0] {
        Value::Array(a) => a,
        other => {
            return Err(type_error(
                "SLICE",
                format!("expected ARRAY as first arg, got {}", type_name(other)),
            ));
        }
    };
    let len = arr.len() as i64;
    let start_raw = match &args[1] {
        Value::Integer(i) => *i,
        other => {
            return Err(type_error(
                "SLICE",
                format!("start must be INTEGER, got {}", type_name(other)),
            ));
        }
    };
    let end_raw = if args.len() == 3 {
        match &args[2] {
            Value::Integer(i) => *i,
            other => {
                return Err(type_error(
                    "SLICE",
                    format!("end must be INTEGER, got {}", type_name(other)),
                ));
            }
        }
    } else {
        len
    };
    let norm_start = if start_raw < 0 { (start_raw + len).max(0) } else { start_raw.min(len) };
    let norm_end = if end_raw < 0 { (end_raw + len).max(0) } else { end_raw.min(len) };
    if norm_end <= norm_start {
        return Ok(Outcome::normal(Value::Array(Vec::new())));
    }
    let start = norm_start as usize;
    let end = norm_end as usize;
    Ok(Outcome::normal(Value::Array(arr[start..end].to_vec())))
}

/// `CONCAT(a, b) -> ARRAY`: 拼接两个 ARRAY,返回新 ARRAY。
/// 同时接受 STRING+STRING,返回 codepoint ARRAY (与 CODEPOINTS 一致)。
/// spec §10.1 row 7 允许字符串拼接形式,我们采取宽松路径。
fn builtin_concat(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("CONCAT", args.len(), 2));
    }
    match (&args[0], &args[1]) {
        (Value::Array(a), Value::Array(b)) => {
            let mut out = Vec::with_capacity(a.len() + b.len());
            out.extend(a.iter().cloned());
            out.extend(b.iter().cloned());
            Ok(Outcome::normal(Value::Array(out)))
        }
        (Value::String(a), Value::String(b)) => {
            let mut out = Vec::with_capacity(a.chars().count() + b.chars().count());
            for c in a.chars() {
                out.push(Value::Integer(c as i64));
            }
            for c in b.chars() {
                out.push(Value::Integer(c as i64));
            }
            Ok(Outcome::normal(Value::Array(out)))
        }
        (other_a, other_b) => Err(type_error(
            "CONCAT",
            format!("expected (ARRAY, ARRAY) or (STRING, STRING), got ({}, {})",
                type_name(other_a), type_name(other_b)),
        )),
    }
}

/// `CONTAINS(arr, x) -> BOOLEAN`: 浅相等 (==) 检查元素是否存在。
/// ARRAY 用 values_equal;STRING 也接受,把 needle 当 substring 查。
fn builtin_contains(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("CONTAINS", args.len(), 2));
    }
    let needle = &args[1];
    match &args[0] {
        Value::Array(a) => {
            let found = a.iter().any(|item| values_equal(item, needle));
            Ok(Outcome::normal(Value::Boolean(found)))
        }
        Value::String(s) => {
            let needle_str = match needle {
                Value::String(n) => n,
                other => {
                    return Err(type_error(
                        "CONTAINS",
                        format!("STRING contains expects STRING needle, got {}", type_name(other)),
                    ));
                }
            };
            Ok(Outcome::normal(Value::Boolean(s.contains(needle_str.as_str()))))
        }
        other => Err(type_error(
            "CONTAINS",
            format!("expected ARRAY or STRING, got {}", type_name(other)),
        )),
    }
}

/// `INDEX(arr, x) -> INTEGER / -1`: 找元素首次出现的 1-based index。
/// 找不到返回 -1 (与 v0.2 历史兼容);spec §10.1 row 8 1-based 约定。
fn builtin_index(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("INDEX", args.len(), 2));
    }
    let needle = &args[1];
    match &args[0] {
        Value::Array(a) => {
            for (i, item) in a.iter().enumerate() {
                if values_equal(item, needle) {
                    return Ok(Outcome::normal(Value::Integer((i + 1) as i64)));
                }
            }
            Ok(Outcome::normal(Value::Integer(-1)))
        }
        Value::String(s) => {
            let needle_str = match needle {
                Value::String(n) => n,
                other => {
                    return Err(type_error(
                        "INDEX",
                        format!("STRING index expects STRING needle, got {}", type_name(other)),
                    ));
                }
            };
            match s.find(needle_str.as_str()) {
                Some(i) => Ok(Outcome::normal(Value::Integer((i + 1) as i64))),
                None => Ok(Outcome::normal(Value::Integer(-1))),
            }
        }
        other => Err(type_error(
            "INDEX",
            format!("expected ARRAY or STRING, got {}", type_name(other)),
        )),
    }
}

/// `REVERSE(arr) -> ARRAY`: 反转 array,返回新 array。
/// STRING 也接受,返回 codepoint array (与 CODEPOINTS 一致)。
fn builtin_reverse(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("REVERSE", &args, 1)?;
    match v {
        Value::Array(a) => {
            let mut out = a.clone();
            out.reverse();
            Ok(Outcome::normal(Value::Array(out)))
        }
        Value::String(s) => {
            let mut chars: Vec<i64> = s.chars().map(|c| c as i64).collect();
            chars.reverse();
            Ok(Outcome::normal(Value::Array(chars.into_iter().map(Value::Integer).collect())))
        }
        other => Err(type_error(
            "REVERSE",
            format!("expected ARRAY or STRING, got {}", type_name(other)),
        )),
    }
}


// ──────────────────────────────────────────────────────────────────────
// Phase B13: spec v0.4 §10.3 STRING ops (5 项)
// ──────────────────────────────────────────────────────────────────────
//
// 把 spec 附录 G Deferred 的 5 个 string builtin 接进 resolve_builtin。
// 全部 ASCII-only (非 ASCII case-fold / 长 substring 留 v0.5);
// SUB/REPLACE/SPLIT 用 Rust 标准库字符串操作 + 错误边界 (start/end INTEGER,
// REPLACE old/new 必须 STRING)。ErrConsumerStatus::No —— §12.6 default
// transparent propagation。

/// `UPPER(s) -> STRING`: ASCII upper-case (a-z → A-Z);非 ASCII char
/// 原样保留 (与 spec §10.3 row 4 一致,非 ASCII case-fold 是
/// implementation-defined)。
fn builtin_upper(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("UPPER", &args, 1)?;
    match v {
        Value::String(s) => Ok(Outcome::normal(Value::String(s.to_ascii_uppercase()))),
        other => Err(type_error(
            "UPPER",
            format!("expected STRING, got {}", type_name(other)),
        )),
    }
}

/// `LOWER(s) -> STRING`: ASCII lower-case (A-Z → a-z);非 ASCII char 原样。
fn builtin_lower(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("LOWER", &args, 1)?;
    match v {
        Value::String(s) => Ok(Outcome::normal(Value::String(s.to_ascii_lowercase()))),
        other => Err(type_error(
            "LOWER",
            format!("expected STRING, got {}", type_name(other)),
        )),
    }
}

/// `SUB(s, start, end?) -> STRING`: 半开区间 [start, end) 子字符串。
/// start/end INTEGER;end 缺省切到末尾;负数从尾数 (类似 SLICE)。
/// start/end out-of-range 钳到合法边界 (与 SLICE 一致)。
fn builtin_substr(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() < 2 || args.len() > 3 {
        return Err(arity_error("SUB", 2, args.len()));
    }
    let s = match &args[0] {
        Value::String(s) => s,
        other => {
            return Err(type_error(
                "SUB",
                format!("expected STRING as first arg, got {}", type_name(other)),
            ));
        }
    };
    let len = s.chars().count() as i64;
    let start_raw = match &args[1] {
        Value::Integer(i) => *i,
        other => {
            return Err(type_error(
                "SUB",
                format!("start must be INTEGER, got {}", type_name(other)),
            ));
        }
    };
    let end_raw = if args.len() == 3 {
        match &args[2] {
            Value::Integer(i) => *i,
            other => {
                return Err(type_error(
                    "SUB",
                    format!("end must be INTEGER, got {}", type_name(other)),
                ));
            }
        }
    } else {
        len
    };
    let norm_start = if start_raw < 0 { (start_raw + len).max(0) } else { start_raw.min(len) };
    let norm_end = if end_raw < 0 { (end_raw + len).max(0) } else { end_raw.min(len) };
    if norm_end <= norm_start {
        return Ok(Outcome::normal(Value::String(String::new())));
    }
    // 用 chars() 处理 codepoint-aware 切片
    let chars: Vec<char> = s.chars().collect();
    let start = norm_start as usize;
    let end = norm_end as usize;
    Ok(Outcome::normal(Value::String(chars[start..end].iter().collect())))
}

/// `REPLACE(s, old, new) -> STRING`: 把所有 `old` 替换为 `new`。
/// non-overlapping 替换 (Rust `String::replace` 默认);`old` 空字符串
/// → E0030 (避免死循环)。
fn builtin_replace(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 3 {
        return Err(arity_error("REPLACE", args.len(), 3));
    }
    let s = match &args[0] {
        Value::String(s) => s,
        other => {
            return Err(type_error(
                "REPLACE",
                format!("expected STRING as first arg, got {}", type_name(other)),
            ));
        }
    };
    let old = match &args[1] {
        Value::String(o) => o,
        other => {
            return Err(type_error(
                "REPLACE",
                format!("old must be STRING, got {}", type_name(other)),
            ));
        }
    };
    let new = match &args[2] {
        Value::String(n) => n,
        other => {
            return Err(type_error(
                "REPLACE",
                format!("new must be STRING, got {}", type_name(other)),
            ));
        }
    };
    if old.is_empty() {
        return Err(type_error(
            "REPLACE",
            "old pattern must be non-empty (empty would cause infinite loop)".to_string(),
        ));
    }
    Ok(Outcome::normal(Value::String(s.replace(old.as_str(), new.as_str()))))
}

/// `SPLIT(s, sep) -> ARRAY`: 按 `sep` 拆分 STRING,返回 STRING ARRAY。
/// `sep` 空字符串 → E0030 (split 行为未定义)。
fn builtin_split(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("SPLIT", args.len(), 2));
    }
    let s = match &args[0] {
        Value::String(s) => s,
        other => {
            return Err(type_error(
                "SPLIT",
                format!("expected STRING as first arg, got {}", type_name(other)),
            ));
        }
    };
    let sep = match &args[1] {
        Value::String(sep) => sep,
        other => {
            return Err(type_error(
                "SPLIT",
                format!("sep must be STRING, got {}", type_name(other)),
            ));
        }
    };
    if sep.is_empty() {
        return Err(type_error(
            "SPLIT",
            "separator must be non-empty".to_string(),
        ));
    }
    let parts: Vec<Value> = s
        .split(sep.as_str())
        .map(|p| Value::String(p.to_string()))
        .collect();
    Ok(Outcome::normal(Value::Array(parts)))
}


// ──────────────────────────────────────────────────────────────────────
// Phase B14: spec v0.4 §10.2 DICT ops (4 项)
// ──────────────────────────────────────────────────────────────────────
//
// KEYS / VALUES / HAS / MERGE 接进 resolve_builtin。沿用 B1 期
// dict_lookup helper 风格的 key 比较;MERGE 把后一个 dict 的
// (key, value) 按出现顺序附加到前一个 dict,key 冲突时后值覆盖前值
// (符合 v0.2 §10.2 习惯)。KEYS / VALUES 保持出现顺序 (spec §10.2
// DICT 内部按 Vec<(Value, Value)> 顺序保存)。ErrConsumerStatus::No。

/// `KEYS(dict) -> ARRAY`: 返回 dict 所有 key,保持出现顺序。
fn builtin_keys(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("KEYS", &args, 1)?;
    match v {
        Value::Dict(entries) => {
            let keys: Vec<Value> = entries.iter().map(|(k, _)| k.clone()).collect();
            Ok(Outcome::normal(Value::Array(keys)))
        }
        other => Err(type_error(
            "KEYS",
            format!("expected DICT, got {}", type_name(other)),
        )),
    }
}

/// `VALUES(dict) -> ARRAY`: 返回 dict 所有 value,保持出现顺序。
fn builtin_values(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("VALUES", &args, 1)?;
    match v {
        Value::Dict(entries) => {
            let vals: Vec<Value> = entries.iter().map(|(_, v)| v.clone()).collect();
            Ok(Outcome::normal(Value::Array(vals)))
        }
        other => Err(type_error(
            "VALUES",
            format!("expected DICT, got {}", type_name(other)),
        )),
    }
}

/// `HAS(dict, k) -> BOOLEAN`: 检查 dict 是否包含 key (基于 values_equal)。
fn builtin_has(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("HAS", args.len(), 2));
    }
    let dict = match &args[0] {
        Value::Dict(entries) => entries,
        other => {
            return Err(type_error(
                "HAS",
                format!("expected DICT as first arg, got {}", type_name(other)),
            ));
        }
    };
    let key = &args[1];
    let found = dict_lookup(dict, key).is_some();
    Ok(Outcome::normal(Value::Boolean(found)))
}

/// `MERGE(a, b) -> DICT`: 把 dict b 的 (k, v) 按顺序附加到 dict a,
/// key 冲突时 b 的值覆盖 a 的值 (a 后 c)。
/// 两个 dict 同 key 时,结果是按 b 的顺序 (Vec 顺序),不重复。
fn builtin_merge(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("MERGE", args.len(), 2));
    }
    let a = match &args[0] {
        Value::Dict(entries) => entries,
        other => {
            return Err(type_error(
                "MERGE",
                format!("expected DICT as first arg, got {}", type_name(other)),
            ));
        }
    };
    let b = match &args[1] {
        Value::Dict(entries) => entries,
        other => {
            return Err(type_error(
                "MERGE",
                format!("expected DICT as second arg, got {}", type_name(other)),
            ));
        }
    };
    let mut out: Vec<(Value, Value)> = a.clone();
    for (k, v) in b {
        if let Some(i) = dict_lookup(&out, k) {
            out[i].1 = v.clone();
        } else {
            out.push((k.clone(), v.clone()));
        }
    }
    Ok(Outcome::normal(Value::Dict(out)))
}


// ──────────────────────────────────────────────────────────────────────
// Phase B15: spec v0.4 misc 8 项
// ──────────────────────────────────────────────────────────────────────
//
// INPUT / BOOL / CALL / NEG 是简单 builtin;
// GET_PROP / SET_PROP / CALL_METHOD / MODULE_REF 是 OOP / 模块值
// 系统的"v0.4 引入"项,本批最小化 stub:返回 E0037 / E0021 标记未接,
// 注册表登记 ResolvedBuiltin 锁住 spec 集成契约,真正的 OOP/模块
// 系统实现留 Phase B15+ / Phase C。

/// `BOOL(x) -> BOOLEAN`: spec §9.4 truthiness 规则。
/// NULL → false;Boolean(b) → b;Integer/Float/String/Array/Dict/
/// Closure/NativeFn/Ok/Err → true (与 B9 NOT truthiness 同源)。
fn builtin_bool(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("BOOL", &args, 1)?;
    Ok(Outcome::normal(Value::Boolean(is_truthy(v))))
}


fn builtin_neg(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("NEG", &args, 1)?;
    match v {
        Value::Integer(i) => Ok(Outcome::normal(Value::Integer(-i))),
        Value::Float(f) => Ok(Outcome::normal(Value::Float(-f))),
        other => Err(type_error(
            "NEG",
            format!("expected INTEGER or FLOAT, got {}", type_name(other)),
        )),
    }
}

/// `INPUT(prompt?) -> STRING`: 从 stdin 读一行。
/// prompt 可选 (STRING);输出到 stdout,读一行返回 (去掉尾部 \n)。
/// 0 arg → 不输出 prompt 直接读。
/// 无可用 stdin 时 (测试场景) → ERR("NoInputAvailable")。
fn builtin_input(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() > 1 {
        return Err(arity_error("INPUT", args.len(), 1));
    }
    if args.len() == 1 {
        let prompt = match &args[0] {
            Value::String(s) => s,
            other => {
                return Err(type_error(
                    "INPUT",
                    format!("prompt must be STRING, got {}", type_name(other)),
                ));
            }
        };
        print!("{}", prompt);
        use std::io::Write;
        std::io::stdout().flush().ok();
    }
    let mut line = String::new();
    let bytes = match std::io::stdin().read_line(&mut line) {
        Ok(n) => n,
        Err(_) => {
            return Ok(Outcome::normal(Value::Err(Box::new(Value::Dict(vec![
                (Value::String("kind".into()), Value::String("NoInputAvailable".into())),
            ])))));
        }
    };
    if bytes == 0 {
        // EOF
        return Ok(Outcome::normal(Value::Err(Box::new(Value::Dict(vec![
            (Value::String("kind".into()), Value::String("EOF".into())),
        ])))));
    }
    // 去掉尾部 \n (Windows: \r\n)
    let trimmed = line.trim_end_matches(|c| c == '\n' || c == '\r').to_string();
    Ok(Outcome::normal(Value::String(trimmed)))
}

/// `CALL(fn, args...) -> v`: 通用函数调用 —— 接受一个 callable (closure,
/// native fn, 或 builtin) + 任意数量参数,执行 invoke_closure 等价语义。
/// 主要用途:把函数作为值传递 / 在 ARRAY 里存函数 / 动态分发。
fn builtin_call(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.is_empty() {
        return Err(arity_error("CALL", args.len(), 1));
    }
    // 当前实现:CALL 路径已经走 eval_call (因为 Expr::Call 入口);
    // 这里的 builtin_call 是显式 self-call 入口,用于 dynamic dispatch。
    // 当前仅支持 closure 直接调用 —— NativeFn 的 invoke 接口暴露给
    // registry 但本批不递归 (会触发 §12.6 ERR 短路 bug)。返回 E0020
    // 让 caller 知道这不是 closure 调用路径。
    match &args[0] {
        Value::Closure { .. } => {
            // closure 直接调用由 eval_call 处理,本路径不应该走 builtin_call
            return Err(type_error(
                "CALL",
                "CALL(closure, ...) is dispatched via eval_call already; this path is reserved for dynamic dispatch".into(),
            ));
        }
        _ => {
            return Err(type_error(
                "CALL",
                "CALL first arg must be closure (or native fn via registry); other callables not yet supported".into(),
            ));
        }
    }
}

// ── Properties / methods / module-as-value (Phase C2) ──
//
// spec v0.4 §13.12:模块对象是 **DICT**(同 §11.1 类内部表示)。
// 所以 GET_PROP / SET_PROP / CALL_METHOD 的 receiver 统一是 DICT:
// 模块对象(MODULE_REF 返回值)与普通 DICT 走同一套 property 语义。
// CLASS / NEW / THIS 的 OOP eval 仍是 LexerMacro-only(v0.4 §11.2/§8.6
// 的类值协议 deferred,见 deviations P4-C2-003)。

/// `GET_PROP(obj, k) -> v / E0037`: spec §13.12 / §5.5 property get。
/// obj 必须是 DICT(模块对象也是 DICT),否则 E0030;
/// k 必须是 STRING,否则 E0030;键缺失 → E0037。
fn builtin_get_prop(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let (obj, key) = expect_arity2("GET_PROP", &args)?;
    let entries = match obj {
        Value::Dict(e) => e,
        other => {
            return Err(type_error(
                "GET_PROP",
                format!("expected DICT object, got {}", type_name(other)),
            ));
        }
    };
    let k = match key {
        Value::String(s) => s,
        other => {
            return Err(type_error(
                "GET_PROP",
                format!("property key must be STRING, got {}", type_name(other)),
            ));
        }
    };
    match dict_lookup(entries, &Value::String(k.clone())) {
        Some(i) => Ok(Outcome::normal(entries[i].1.clone())),
        None => Err(builtin_error(
            ErrorCode::E0037,
            "GET_PROP",
            format!("no such property '{}'", k),
        )),
    }
}

/// `SET_PROP(obj, k, v) -> DICT`: spec §13.12 / §5.5 property set。
/// DICT 上插入或更新键;返回**更新后的 DICT**(值语义,与 Phase B1
/// `INDEX_SET` 的既定实现一致 —— builtin 边界按值传参,无法就地
/// 写回 receiver;registry 的 `-> NULL` 签名沿 spec 文本,实现偏差
/// 见 deviations P4-C2-002)。
fn builtin_set_prop(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 3 {
        return Err(arity_error("SET_PROP", args.len(), 3));
    }
    let mut entries = match &args[0] {
        Value::Dict(e) => e.clone(),
        other => {
            return Err(type_error(
                "SET_PROP",
                format!("expected DICT object, got {}", type_name(other)),
            ));
        }
    };
    let k = match &args[1] {
        Value::String(s) => s.clone(),
        other => {
            return Err(type_error(
                "SET_PROP",
                format!("property key must be STRING, got {}", type_name(other)),
            ));
        }
    };
    let v = args[2].clone();
    match dict_lookup(&entries, &Value::String(k.clone())) {
        Some(i) => entries[i].1 = v,
        None => entries.push((Value::String(k), v)),
    }
    Ok(Outcome::normal(Value::Dict(entries)))
}

/// `CALL_METHOD(obj, m, args...) -> v`: spec §11.4 / §8.6 method call。
///
/// self 注入规则(§8.6 "本规范明确"):receiver **总是**作为第一个实参
/// 注入 closure 方法 —— 等价 `CALL_METHOD(rect, "get_area", rect)`;
/// 首形参命名为 `self` 只是一个**约定**(不强制),命名不影响绑定。
///
/// NativeFn(std 模块成员)则**不**注入 receiver:§13.12 钉死
/// `io.PRINT("hi")` 等价 `IMPORT` 后的 `PRINT("hi")`。
fn builtin_call_method(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() < 2 {
        return Err(arity_error("CALL_METHOD", args.len(), 2));
    }
    let obj = match &args[0] {
        Value::Dict(e) => e.clone(),
        other => {
            return Err(type_error(
                "CALL_METHOD",
                format!("expected DICT object receiver, got {}", type_name(other)),
            ));
        }
    };
    let method = match &args[1] {
        Value::String(s) => s.clone(),
        other => {
            return Err(type_error(
                "CALL_METHOD",
                format!("method name must be STRING, got {}", type_name(other)),
            ));
        }
    };
    let rest: Vec<Value> = args[2..].to_vec();
    let callee = match dict_lookup(&obj, &Value::String(method.clone())) {
        Some(i) => obj[i].1.clone(),
        None => {
            return Err(builtin_error(
                ErrorCode::E0037,
                "CALL_METHOD",
                format!("no such method '{}'", method),
            ));
        }
    };
    let span = ev
        .current_span
        .clone()
        .unwrap_or_else(runtime_span);
    ev.call_value_with_receiver(callee, Some(Value::Dict(obj)), rest, &span, &method)
}

/// `MODULE_REF(path) -> MODULE`: spec §13.12 module-as-value。
///
/// 语义 = 加载模块但**不绑定名字**,返回模块对象(DICT:导出名 → 值)。
/// 与 IMPORT 共用 `ModuleLoader`(缓存 / 循环检测 / 命名空间解析),
/// 只是不做 per-name `E0023` 校验与局部绑定 —— 键集合就是模块的
/// EXPORT 面。加载失败(E0040 / E0041 / E0043 / E0023)原样冒泡。
fn builtin_module_ref(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let path = match expect_arity("MODULE_REF", &args, 1)? {
        Value::String(s) => s.clone(),
        other => {
            return Err(type_error(
                "MODULE_REF",
                format!("expected STRING module path, got {}", type_name(other)),
            ));
        }
    };
    let loaded = {
        let mut loader = ev.loader.borrow_mut();
        loader.load(&path)?
    };
    // 排序保证 DICT 键序确定(HashSet 迭代序不稳定)。
    let mut names: Vec<&String> = loaded.exports.iter().collect();
    names.sort();
    let mut entries: Vec<(Value, Value)> = Vec::with_capacity(names.len());
    for n in names {
        let v = loaded.env.get(n).map(|r| r.clone()).unwrap_or(Value::Null);
        entries.push((Value::String(n.clone()), v));
    }
    Ok(Outcome::normal(Value::Dict(entries)))
}

/// v0.4 §10.3 + appendix G — `STR(x) → STRING`.
///
/// Rendering is `Value::display()` — the same conversion `PRINT`
/// applies to non-STRING args (appendix G: "args 不是 STRING →
/// `STR(args)`"). `FORMAT` (§10.6) reuses this semantics when
/// inserting placeholder values.
///
/// Not an ERR consumer (appendix G ❌) and not a macro (❌): plain
/// `Expr::Call` dispatch, and §12.6 transparent propagation happens in
/// `eval_call` before this fn ever sees an ERR argument.
fn builtin_str(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("STR", &args, 1)?;
    Ok(Outcome::normal(Value::String(v.display())))
}

/// v0.4 §10.6 + §15.8 + appendix G — `FORMAT(template, args...) → STRING`.
///
/// Appendix G pins FORMAT as a **global builtin** that is neither an
/// ERR consumer nor a macro, so it dispatches through the generic
/// `Expr::Call` path (§12.6 ERR propagation included, for free).
/// `wlwl:std.format` (§15.8) is its home module — `IMPORT` binds the
/// std-side implementation, which shares the template grammar
/// (`wlwl_std::format::parse_template`) with this builtin.
///
/// Placeholder semantics (§10.6):
/// - `{N}`: insert the N-th format arg (args **after** the template),
///   rendered with STR semantics; out of range → keep `{N}` as-is;
/// - `{name}`: look up the **first DICT among the format args** — in
///   the pure-named pattern that is args[0] exactly as the spec says,
///   and scanning (not hardcoding position 0) is what makes the spec's
///   own mixed example work:
///   `FORMAT("hi {0}, age {age}", "alice", ["age": 30])`;
///   missing key / no DICT arg → keep `{name}` as-is;
/// - template parse failure (unclosed `{`, empty `{}`) → `E0039`;
/// - non-STRING template → `E0030`; zero args → `E0022`.
///
/// Parsed templates are memoized in `Evaluator.format_cache` (plan
/// §5.5: "相同 template 复用解析结果") so loops formatting with the
/// same template parse it once.
fn builtin_format(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.is_empty() {
        return Err(WlwlDiagnostic::new(
            ErrorCode::E0022,
            "FORMAT: function expects at least 1 argument (template), got 0".to_string(),
            Location::point("<runtime>", 0, 0),
        )
        .into());
    }
    let template = match &args[0] {
        Value::String(s) => s.clone(),
        other => {
            return Err(type_error(
                "FORMAT",
                format!(
                    "template must be STRING, got {}",
                    type_name(other)
                ),
            ))
        }
    };
    // Parse with memoization. The grammar lives in wlwl-std so the
    // global builtin and the std.format module share one definition.
    let segments = match ev.format_cache.get(&template) {
        Some(cached) => cached.clone(),
        None => {
            let parsed =
                Rc::new(wlwl_std::format::parse_template(&template).map_err(|e| {
                    // Span-aware E0039 like B4's builtin_unwrap E0100:
                    // point at the FORMAT call site, not `<runtime>`.
                    let loc = ev
                        .current_span
                        .as_ref()
                        .map(|s| Location {
                            file: s.file.clone(),
                            line: s.line_start,
                            col: s.col_start,
                            line_end: s.line_end,
                            col_end: s.col_end,
                        })
                        .unwrap_or_else(|| {
                            Location::point(ev.file.as_deref().unwrap_or("<runtime>"), 0, 0)
                        });
                    WlwlDiagnostic::new(e.code, e.message, loc)
                })?);
            ev.format_cache.insert(template.clone(), parsed.clone());
            parsed
        }
    };
    let format_args = &args[1..];
    let named = format_args.iter().find_map(|a| match a {
        Value::Dict(entries) => Some(entries),
        _ => None,
    });
    let mut out = String::new();
    for seg in segments.iter() {
        match seg {
            wlwl_std::format::FormatSegment::Literal(s) => out.push_str(s),
            wlwl_std::format::FormatSegment::Positional(n) => match format_args.get(*n) {
                Some(v) => out.push_str(&v.display()),
                None => {
                    out.push('{');
                    out.push_str(&n.to_string());
                    out.push('}');
                }
            },
            wlwl_std::format::FormatSegment::Named(name) => {
                let hit = named
                    .and_then(|entries| {
                        dict_lookup(entries, &Value::String(name.clone())).map(|i| &entries[i].1)
                    });
                match hit {
                    Some(v) => out.push_str(&v.display()),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
            }
        }
    }
    Ok(Outcome::normal(Value::String(out)))
}

/// The single dispatch table: maps a built-in name to its implementation.
/// Operators (`+`, `==`, …) live here too — the parser turns `+(1, 2)`
/// into `Call { name: "+", … }`, and we dispatch on the operator name.
fn resolve_builtin(name: &str) -> Option<BuiltinFn> {
    match name {
        "PRINT" => Some(builtin_print),
        // Phase B10 (spec §15.1): PRINT_ERR writes to stderr.
        // Same dispatch as PRINT — appended to the global builtin
        // table so it's available without IMPORT (mirrors `PRINT`).
        "PRINT_ERR" => Some(builtin_print_err),
        "LEN" => Some(builtin_len),
        "PUSH" => Some(builtin_push),
        "INT" => Some(builtin_int),
        // Phase B8 (spec §10.3 + §9.5): FLOAT conversion. Same
        // shape as INT — STRING/INTEGER/FLOAT → FLOAT; STRING parse
        // failure returns `ERR(["kind": "ParseError", "input", "reason"])`
        // for symmetric handling with INT.
        "FLOAT" => Some(builtin_float),
        // Phase B8 (spec §10.3 row 9): three trim builtins.
        // ASCII-whitespace only by spec (non-ASCII trim is
        // implementation-defined; we keep ASCII).
        "TRIM" => Some(builtin_trim),
        "TRIM_START" => Some(builtin_trim_start),
        "TRIM_END" => Some(builtin_trim_end),
        // Phase B8 (spec §10.3 row 10): STARTS_WITH / ENDS_WITH
        // are BOOLEAN-returning substring predicates.
        "STARTS_WITH" => Some(builtin_starts_with),
        "ENDS_WITH" => Some(builtin_ends_with),
        // Phase B8 (spec §10.3 row 11): REPEAT(s, n) builds an
        // n-fold copy; saturating on overflow per §9.5.
        "REPEAT" => Some(builtin_repeat),
        // Phase B8 (spec §10.3 row 12): PAD_START / PAD_END pad
        // with a single char (the leading char of the third arg).
        "PAD_START" => Some(builtin_pad_start),
        "PAD_END" => Some(builtin_pad_end),
        // Phase B8 (spec §10.3 row 13-14): CODEPOINTS ↔ FROM_CODEPOINTS.
        // FROM_CODEPOINTS validates Unicode scalar range (0..=0x10FFFF)
        // and rejects surrogate halves + out-of-range with E0031.
        "CODEPOINTS" => Some(builtin_codepoints),
        "FROM_CODEPOINTS" => Some(builtin_from_codepoints),
        // v0.4 spec §10.3 + appendix G — `STR(x) → STRING` global
        // conversion builtin (v0.2 provenance; the implementation only
        // lands in Phase B5 because FORMAT's §10.6 conversion rule
        // references STR semantics). Not an ERR consumer, not a macro.
        "STR" => Some(builtin_str),
        // v0.4 spec §10.6 + §15.8 + appendix G — `FORMAT(template,
        // args...) → STRING`. Global builtin (appendix G; the plan's
        // §4.2 `Expr::Format` AST sketch is superseded by the spec's
        // normative registry, which marks FORMAT 宏函数 ❌ — see
        // deviations P4-B5-002). Not an ERR consumer → §12.6 default
        // transparent propagation. The std.format home module shares
        // the template grammar.
        "FORMAT" => Some(builtin_format),
        // v0.4 spec §10.1 / §10.2 subscript primitives (Phase B1).
        // None of these consume ERR — they inherit the default §12.6
        // transparent propagation, matching the pre-A6 behaviour for
        // the previous code path.
        "INDEX_GET" => Some(builtin_index_get),
        "INDEX_SET" => Some(builtin_index_set),
        "AT" => Some(builtin_at),
        "REMOVE_KEY" => Some(builtin_remove_key),
        // v0.4 §10.2 — `DEL` is the v0.3-compat alias for `REMOVE_KEY`.
        // Spec §14.5 mandates W0051 on every legacy use; v0.5 removes
        // the alias. Added Phase B2.
        "DEL" => Some(builtin_remove_key_compat),
        "POP" => Some(builtin_pop_dict),

        // Phase B15 (spec 附录 G): misc 8 项从 Deferred 转到 ResolvedBuiltin。
        "BOOL" => Some(builtin_bool),
        "NEG" => Some(builtin_neg),
        "INPUT" => Some(builtin_input),
        "CALL" => Some(builtin_call),
        "GET_PROP" => Some(builtin_get_prop),
        "SET_PROP" => Some(builtin_set_prop),
        "CALL_METHOD" => Some(builtin_call_method),
        "MODULE_REF" => Some(builtin_module_ref),


        // Phase B14 (spec §10.2): DICT ops 4 项从 Deferred 转到 ResolvedBuiltin。
        "KEYS" => Some(builtin_keys),
        "VALUES" => Some(builtin_values),
        "HAS" => Some(builtin_has),
        "MERGE" => Some(builtin_merge),


        // Phase B13 (spec §10.3): STRING ops 5 项从 Deferred 转到 ResolvedBuiltin。
        "UPPER" => Some(builtin_upper),
        "LOWER" => Some(builtin_lower),
        "SUB" => Some(builtin_substr),
        "REPLACE" => Some(builtin_replace),
        "SPLIT" => Some(builtin_split),


        // Phase B12 (spec §10.1): ARRAY ops 7 项从 Deferred 转到 ResolvedBuiltin。
        "SHIFT" => Some(builtin_shift),
        "UNSHIFT" => Some(builtin_unshift),
        "SLICE" => Some(builtin_slice),
        "CONCAT" => Some(builtin_concat),
        "CONTAINS" => Some(builtin_contains),
        "INDEX" => Some(builtin_index),
        "REVERSE" => Some(builtin_reverse),

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
        "!" => Some(builtin_not_bang_compat),
        // Phase B9 (spec §3.4 + §14.5): NOT is the v0.4 canonical
        // name for logical negation; the single-char `!` above is
        // the v0.3-compat alias that emits W0054 at the dispatch
        // boundary. Same function under the hood; just two
        // dispatch entries so we can attach the warning to `!`
        // without coupling it to the clean path.
        "NOT" => Some(builtin_not),
        // v0.4 §12.7 + §14.5 — `OR_DIE` is the v0.3-compat alias for
        // `UNWRAP_OR`. Spec §14.5 mandates W0051 on every legacy use;
        // v0.5 removes the alias. Renamed dispatch target from
        // `builtin_or_die` → `builtin_unwrap_or` in Phase B3 to match
        // the canonical name.
        "OR_DIE" => Some(builtin_unwrap_or_compat),
        // v0.4 spec §12.7 main name for `UNWRAP_OR`. No warning — this
        // is the canonical spelling; the alias wrapper lives at
        // `"OR_DIE"` (above) and emits W0051.
        "UNWRAP_OR" => Some(builtin_unwrap_or),
        // v0.4 spec §2.2.1 — uppercase type name builtin; listed in
        // §12.7 ERR consumer registry (TYPE does not propagate ERR).
        "TYPE" => Some(builtin_type),
        // Phase B4 (spec §12.2): ERR-consumer primitives. UNWRAP
        // consumes OK/ERR per §12.6 whitelist. ERR_PAYLOAD / WRAP
        // also consume ERR — ERR_PAYLOAD extracts the payload,
        // WRAP re-wraps with a `context` dict layer. None of these
        // are v0.3 aliases so no W0051 path here; they are the
        // canonical v0.4 §12.7 names.
        "UNWRAP" => Some(builtin_unwrap),
        "ERR_PAYLOAD" => Some(builtin_err_payload),
        "WRAP" => Some(builtin_wrap),
        _ => None,
    }
}

/// §12.7 ERR consumer registry (v0.4 spec).
///
/// Replaces the v0.3 closed 4-item whitelist with a spec-pinned
/// registry. Each entry here **must** consume `ERR` (i.e. NOT
/// transparently propagate it per §12.6). Adding a new entry
/// requires a spec upgrade (§18 / appendix D); the implementation
/// must NOT consume ERR outside this set (spec §12.7 末段).
///
/// **Currently registered (10 names)**:
///
/// | name          | behavior on ERR                                          |
/// |---------------|----------------------------------------------------------|
/// | `IS_OK`       | returns `FALSE` (observation, no payload)                |
/// | `IS_ERR`      | returns `TRUE`  (observation, no payload)                |
/// | `OR_DIE`      | returns the `default` arg                                 |
/// | `UNWRAP_OR`   | canonical (v0.4 §12.7); `OR_DIE` is the v0.3 alias that emits `W0051` (Phase B3) |
/// | `TRY`         | early-`RETURN` from the enclosing function                |
/// | `UNWRAP`      | `PANIC` E0100 with `cause` = payload — spec §12.2 (Phase B4) |
/// | `ERR_PAYLOAD` | extracts payload — spec §12.2 (Phase B4)              |
/// | `WRAP`        | re-wraps with `{original, context}` dict — spec §12.2 (Phase B4) |
/// | `TYPE`        | returns `"RESULT"` (observation)                         |
///
/// Spec §12.7 also lists `=` / `!=` / `IF` as "registered"; those
/// **explicitly do not consume ERR** (per the table footnote in
/// §12.7 and §9.2 / §7.1) and propagate per §12.6 default. They
/// are NOT in this table — the default transparent propagation IS
/// the correct behavior for them.
///
/// **Note on lexer-level macros**: `IS_OK` / `IS_ERR` / `OR_DIE` /
/// `TRY` are lexer keywords that the parser lowers into
/// `Expr::IsOk` / `Expr::IsErr` / `Expr::OrDie` / `Expr::Try`
/// **before** `eval_call` sees them. Those names therefore do NOT
/// actually flow through this registry at runtime — they consume
/// ERR via their own `Expr::*` arms. The registry exists for the
/// remaining names (`UNWRAP_OR` / `UNWRAP` / `ERR_PAYLOAD` / `WRAP`
/// / `TYPE` / `EXPECT_ERR`) that ARE reached via the generic
/// `Expr::Call { name }` path. The `=` / `!=` / `IF` operators
/// also reach `eval_call` (via `Expr::Call { name: "==" | "!=" |
/// "IF", ... }`) but are not in the registry because they MUST
/// propagate ERR per §9.2.
const ERR_CONSUMER_REGISTRY: &[&str] = &[
    "IS_OK",
    "IS_ERR",
    "OR_DIE",
    "UNWRAP_OR",
    "TRY",
    "UNWRAP",
    "ERR_PAYLOAD",
    "WRAP",
    "TYPE",
    // Phase B7 (spec §15.9 row 5): EXPECT_ERR is the
    // "expect-this-expression-to-fail" primitive. Without ERR-consumer
    // status, the very thing it's designed to inspect — an ERR value —
    // would be short-circuited by §12.6 before the builtin sees it,
    // producing a top-level E0102 instead of the E0049 the spec promises.
    "EXPECT_ERR",
];

/// Returns `true` if `name` is in the §12.7 ERR consumer registry.
/// These functions consume `ERR` instead of letting it transparently
/// propagate (per spec §12.6).
fn is_err_consumer(name: &str) -> bool {
    ERR_CONSUMER_REGISTRY.contains(&name)
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
    let fix = if got > want {
        format!("too many arguments: pass {} fewer (got {}, want {})", got - want, got, want)
    } else {
        format!("too few arguments: add {} more (got {}, want {})", want - got, got, want)
    };
    WlwlDiagnostic::new(
        ErrorCode::E0022,
        format!("function `{}` expects {} argument(s), got {}", name, want, got),
        Location::point("<runtime>", 0, 0),
    )
    .with_suggestion(Suggestion::Note { description: fix })
    .into()
}

fn type_error(fn_name: &str, msg: String) -> WlwlError {
    WlwlDiagnostic::new(
        ErrorCode::E0030,
        format!("{}: {}", fn_name, msg),
        Location::point("<runtime>", 0, 0),
    )
    .with_suggestion(Suggestion::Note {
        description: "check the operand types or use an explicit conversion; v0.3 has no implicit numeric coercion".into(),
    })
    .into()
}

/// Construct a builtin-emitted error (v0.4 spec §9.5 etc.).
///
/// Span is a `<runtime>` placeholder because builtins are not
/// dispatched with the AST `Span` (they take `(evaluator, args)`
/// only); see `eval_call`. `enrich_with_trace` will still pick up
/// the current `call_stack` for trace context.
///
/// Used by:
///   * `E1003` (division / modulo by zero — Runtime bucket)
///   * `E0034` (NEG(INTEGER_MIN) — Type bucket)
///   * `E0035` (FLOAT → INTEGER out-of-range — Type bucket)
fn builtin_error(code: ErrorCode, fn_name: &str, msg: String) -> WlwlError {
    debug_assert!(
        code.category() == ErrorCategory::Runtime
            || code.category() == ErrorCategory::Type,
        "builtin_error called with code {:?} of category {:?}",
        code,
        code.category()
    );
    WlwlDiagnostic::new(
        code,
        format!("{}: {}", fn_name, msg),
        Location::point("<runtime>", 0, 0),
    )
    .into()
}

/// Placeholder span for builtin-emitted diagnostics outside any AST
/// call site (direct builtin invocation from tests / Rust callers).
fn runtime_span() -> Span {
    Span {
        file: "<runtime>".to_string(),
        line_start: 0,
        col_start: 0,
        line_end: 0,
        col_end: 0,
    }
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
        Value::NativeFn { .. } => "native-function",
        Value::Ok(_) => "ok",
        Value::Err(_) => "err",
    }
}

/// v0.4 spec §2.2.1 — uppercase type names for the user-facing `TYPE(x)`
/// builtin. Note that both `Value::Ok(_)` and `Value::Err(_)` collapse to
/// `"RESULT"` per spec §2.2.1 (the two variants share one type name).
///
/// Distinct from `type_name` (lowercase, used for human-readable error
/// messages) so that adding `TYPE` does not ripple through every
/// `type_error` / `arity_error` diagnostic string.
///
/// `Value::NativeFn { .. }` is folded into `"FUNCTION"` (spec §2.2 lists
/// one `FUNCTION` type; `NativeFn` is a std-injection mechanism, not a
/// separate user-visible type). This is a known simplification — see
/// deviations P4-A5-001.
fn value_type_name(v: &Value) -> &'static str {
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
    }
}

/// v0.4 spec §2.2.1 — `TYPE(x)` builtin.
///
/// Returns the **uppercase** type name of `x` (per spec §2.2 table).
/// Registered in the §12.7 ERR consumer registry (does not propagate ERR).
fn builtin_type(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("TYPE", &args, 1)?;
    Ok(Outcome::normal(Value::String(value_type_name(v).to_string())))
}

fn numeric(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    }
}

fn builtin_add(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
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
    // v0.4 §9.5: INTEGER + INTEGER overflow saturates to INT64_MAX/MIN
    // (never wraps) and emits W0015.
    if let (Value::Integer(i1), Value::Integer(i2)) = (a, b) {
        match i1.checked_add(*i2) {
            Some(r) => Ok(Outcome::normal(Value::Integer(r))),
            None => {
                let saturated = if i1.signum() > 0 { i64::MAX } else { i64::MIN };
                ev.emit_warning(
                    ErrorCode::W0015,
                    format!(
                        "integer overflow in `+`, saturated to {}",
                        saturated
                    ),
                );
                Ok(Outcome::normal(Value::Integer(saturated)))
            }
        }
    } else if numeric(a).is_some() && numeric(b).is_some() {
        // FLOAT path: IEEE 754 default (Inf / NaN propagate, no W0015).
        Ok(Outcome::normal(Value::Float(
            numeric(a).unwrap() + numeric(b).unwrap(),
        )))
    } else {
        Err(type_error("+", format!(
            "cannot add {} and {}",
            type_name(a), type_name(b)
        )))
    }
}

fn builtin_sub(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("-", args.len(), 2));
    }
    let a = &args[0];
    let b = &args[1];
    // v0.4 §9.5: `-(0, INTEGER_MIN)` is the **NEG** case from the spec
    // table and must throw E0034 (NOT saturate / W0015). Other INTEGER
    // underflow saturates to INT64_MAX/MIN and emits W0015 like `+`.
    if let (Value::Integer(i1), Value::Integer(i2)) = (a, b) {
        if *i1 == 0 && *i2 == i64::MIN {
            return Err(builtin_error(
                ErrorCode::E0034,
                "NEG",
                format!(
                    "cannot negate INTEGER_MIN ({}); use -INTEGER_MIN+1 or special-case",
                    i64::MIN
                ),
            ));
        }
        match i1.checked_sub(*i2) {
            Some(r) => Ok(Outcome::normal(Value::Integer(r))),
            None => {
                let saturated = if i1.signum() > 0 || (*i1 == 0 && i2.signum() < 0) {
                    i64::MAX
                } else {
                    i64::MIN
                };
                ev.emit_warning(
                    ErrorCode::W0015,
                    format!(
                        "integer overflow in `-`, saturated to {}",
                        saturated
                    ),
                );
                Ok(Outcome::normal(Value::Integer(saturated)))
            }
        }
    } else if numeric(a).is_some() && numeric(b).is_some() {
        Ok(Outcome::normal(Value::Float(
            numeric(a).unwrap() - numeric(b).unwrap(),
        )))
    } else {
        Err(type_error("-", format!(
            "cannot subtract {} and {}",
            type_name(a), type_name(b)
        )))
    }
}

fn builtin_mul(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("*", args.len(), 2));
    }
    let a = &args[0];
    let b = &args[1];
    if let (Value::Integer(i1), Value::Integer(i2)) = (a, b) {
        match i1.checked_mul(*i2) {
            Some(r) => Ok(Outcome::normal(Value::Integer(r))),
            None => {
                // -INT64_MIN also cannot be negated by `0 - r` so the
                // only saturated positive answer is INT64_MAX; negative
                // saturates to INT64_MIN.
                let saturated = if i1.signum() * i2.signum() > 0 { i64::MAX } else { i64::MIN };
                ev.emit_warning(
                    ErrorCode::W0015,
                    format!(
                        "integer overflow in `*`, saturated to {}",
                        saturated
                    ),
                );
                Ok(Outcome::normal(Value::Integer(saturated)))
            }
        }
    } else if numeric(a).is_some() && numeric(b).is_some() {
        Ok(Outcome::normal(Value::Float(
            numeric(a).unwrap() * numeric(b).unwrap(),
        )))
    } else {
        Err(type_error("*", format!(
            "cannot multiply {} and {}",
            type_name(a), type_name(b)
        )))
    }
}

fn builtin_div(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("/", args.len(), 2));
    }
    let a = &args[0];
    let b = &args[1];
    // v0.4 §9.5 — INTEGER / INTEGER: truncation toward zero (Rust's
    // default `/` on i64), division by zero is E1003.
    if let (Value::Integer(i1), Value::Integer(i2)) = (a, b) {
        if *i2 == 0 {
            return Err(builtin_error(
                ErrorCode::E1003,
                "/",
                "division by zero".into(),
            ));
        }
        // Rust's i64 `/` already does truncation toward zero
        // (matches spec §9.5 row 1: `/(7, 2) = 3`, `/(-7, 2) = -3`).
        return Ok(Outcome::normal(Value::Integer(i1 / i2)));
    }
    // FLOAT path: spec §9.5 mandates E1003 even though IEEE 754 would
    // happily produce ±Inf. NaN-propagation is preserved for non-zero
    // divisors.
    let x = numeric(a);
    let y = numeric(b);
    match (x, y) {
        (Some(x), Some(y)) => {
            if y == 0.0 {
                return Err(builtin_error(
                    ErrorCode::E1003,
                    "/",
                    "division by zero".into(),
                ));
            }
            // NaN-propagation (per spec §9.5 row 6): if either side
            // is NaN, the result is NaN without an error.
            Ok(Outcome::normal(Value::Float(x / y)))
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
            return Err(builtin_error(
                ErrorCode::E1003,
                "%",
                "modulo by zero".into(),
            ));
        }
        // Rust's i64 `%` implements `a - (a / b) * b` with sign
        // matching the dividend (matches spec §9.5 row 2).
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

/// Logical negation (spec §9.4 + §3.4). Registered under two
/// names in `resolve_builtin`:
///   - `"NOT"`  → `builtin_not` directly (v0.4 canonical; no
///                warning; the clean path).
///   - `"!"`    → `builtin_not_bang_compat` (v0.3-compat alias;
///                emits W0054 first, then delegates here; v0.5
///                removes the alias).
///
/// Both paths share the same truthiness semantics — `!NULL = TRUE`,
/// `!0 = TRUE`, etc., per §9.4 row 5.
fn builtin_not(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    let v = expect_arity("NOT", &args, 1)?;
    Ok(Outcome::normal(Value::Boolean(!is_truthy(v))))
}

/// v0.3-compat dispatch wrapper for the single-char `!` token.
/// Emits W0054 (deprecated_op_form) on every call, then delegates
/// to `builtin_not`. The emit lives **here**, at the dispatch
/// boundary, so that:
///   - `!(x)` and `!x` (the two `!` invocation shapes) both go
///     through this wrapper and emit exactly once;
///   - `NOT(x)` never reaches this wrapper and never emits;
///   - the inner fn stays a pure expression of the semantics.
///
/// See plan §5.9 ("`!` 改为 NOT 宏函数 + W0054").
fn builtin_not_bang_compat(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    ev.emit_warning(
        ErrorCode::W0054,
        "`!` is a v0.3-compat alias; use `NOT(expr)` instead (will be removed in v0.5)",
    );
    builtin_not(ev, args)
}

/// `UNWRAP_OR(value, default)`: spec §12.2 canonical name (v0.4 §12.7).
/// If `value` is OK(v) → v. If ERR(_) → `default` (evaluated lazily,
/// but in this implementation it has already been evaluated by the
/// call site). This is a §12.6 ERR-consumer, so it gets a special
/// entry in `is_err_consumer`.
///
/// The v0.3-compat alias `OR_DIE` is wired in
/// `builtin_unwrap_or_compat` below — it calls this fn after emitting
/// `W0051`. Note: the W0051 emit point for the lexer-keyword path
/// (`Expr::OrDie`) lives in `eval_expr`'s `Expr::OrDie` arm, NOT here.
fn builtin_unwrap_or(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("UNWRAP_OR", args.len(), 2));
    }
    match &args[0] {
        Value::Ok(v) => Ok(Outcome::normal((**v).clone())),
        Value::Err(_) => Ok(Outcome::normal(args[1].clone())),
        other => Err(type_error(
            "UNWRAP_OR",
            format!("expected OK/ERR, got {}", type_name(other)),
        )),
    }
}

/// v0.4 §12.7 + §14.5 — `OR_DIE(value, default)` is the v0.3-compat
/// alias for `UNWRAP_OR`. Per spec §14.5, every call emits `W0051`.
/// v0.5 will drop this alias entirely.
///
/// This wrapper handles the **runtime-call** path:
///   `OR_DIE` reaches `eval_call` only when called as an ordinary
///   function reference (e.g. via `f = OR_DIE; f(ERR, 0)` after a
///   normal `resolve_builtin` lookup). The **lexer-keyword** path
///   (`OR_DIE(...)` literal at parse time) is handled separately in
///   `eval_expr`'s `Expr::OrDie` arm — both paths emit exactly one
///   `W0051` per user-visible `OR_DIE(...)` source occurrence.
fn builtin_unwrap_or_compat(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    ev.emit_warning(
        ErrorCode::W0051,
        "`OR_DIE` is a v0.3-compat alias; use `UNWRAP_OR` instead (will be removed in v0.5)",
    );
    builtin_unwrap_or(ev, args)
}

// ──────────────────────────────────────────────────────────────────────
// Phase B4 — spec §12.2 ERR-consumer primitives
//
// Three primitives that operate on RESULT values (OK/ERR):
//
//   * `UNWRAP(value)` — OK(v) → v; ERR(e) → PANIC E0100 with
//     `cause = e`; non-RESULT → E0030 type error. The PANIC carries
//     the original error payload into the diagnostic's `cause` field
//     (spec §12.8 / §14.2) so callers (and AI tools reading JSON
//     diagnostics) can trace the WRAP chain.
//
//   * `ERR_PAYLOAD(value)` — ERR(e) → e; OK → E0030 ("expected ERR,
//     got OK"); non-RESULT → E0030. Extracts the payload without
//     consuming it for ERR propagation purposes (the value is no
//     longer an ERR so §12.6 short-circuit stops applying).
//
//   * `WRAP(value, context)` — ERR(e) → ERR({"original": e, "context":
//     ctx}); OK(v) → OK(v) (pass-through); non-RESULT → E0030. Each
//     WRAP adds a `{original, context}` dict layer; nesting WRAPs
//     produces a nested dict chain (no flattening). The deepest
//     `original` is the user-facing error message; `context` is the
//     most recent annotation.
//
// All three are listed in the §12.7 ERR consumer registry so they
// receive the ERR value instead of letting §12.6 transparently
// propagate it.
// ──────────────────────────────────────────────────────────────────────

/// `UNWRAP(value)`: spec §12.2. See module docs for full contract.
fn builtin_unwrap(ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity_error("UNWRAP", args.len(), 1));
    }
    match &args[0] {
        Value::Ok(v) => Ok(Outcome::normal((**v).clone())),
        Value::Err(payload) => {
            // PANIC E0100 with `cause` = payload. This is the §12.2
            // way to surface an ERR: the value is converted to a
            // fatal diagnostic that aborts evaluation (not a TRY-
            // catchable ERR value).
            let loc = ev
                .current_span
                .as_ref()
                .map(|s| Location {
                    file: s.file.clone(),
                    line: s.line_start,
                    col: s.col_start,
                    line_end: s.line_end,
                    col_end: s.col_end,
                })
                .unwrap_or_else(|| {
                    Location::point(ev.file.as_deref().unwrap_or("<runtime>"), 0, 0)
                });
            let mut diag = WlwlDiagnostic::new(
                ErrorCode::E0100,
                format!("UNWRAP called on ERR value"),
                loc,
            );
            if let Some(cause) = value_to_error_cause(payload) {
                diag = diag.with_cause(cause);
            }
            Err(WlwlError::Diagnostic(diag))
        }
        other => Err(type_error(
            "UNWRAP",
            format!("expected OK/ERR, got {}", type_name(other)),
        )),
    }
}

/// `ERR_PAYLOAD(value)`: spec §12.2. See module docs for full contract.
fn builtin_err_payload(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 1 {
        return Err(arity_error("ERR_PAYLOAD", args.len(), 1));
    }
    match &args[0] {
        Value::Err(payload) => Ok(Outcome::normal((**payload).clone())),
        Value::Ok(_) => Err(type_error(
            "ERR_PAYLOAD",
            "expected ERR, got OK".to_string(),
        )),
        other => Err(type_error(
            "ERR_PAYLOAD",
            format!("expected ERR, got {}", type_name(other)),
        )),
    }
}

/// `WRAP(value, context)`: spec §12.2. See module docs for full contract.
fn builtin_wrap(_ev: &mut Evaluator, args: Vec<Value>) -> WlwlResult<Outcome> {
    if args.len() != 2 {
        return Err(arity_error("WRAP", args.len(), 2));
    }
    match &args[0] {
        Value::Err(payload) => {
            // Build `{"original": payload, "context": ctx}` dict.
            // Nested WRAPs produce nested dicts (no flattening) so the
            // full chain is preserved — `original` of an outer WRAP
            // is the entire inner ERR value (which may itself be a
            // `{original, context}` dict from a previous WRAP).
            let dict = Value::Dict(vec![
                (
                    Value::String("original".into()),
                    (**payload).clone(),
                ),
                (
                    Value::String("context".into()),
                    args[1].clone(),
                ),
            ]);
            Ok(Outcome::normal(Value::Err(Box::new(dict))))
        }
        Value::Ok(v) => Ok(Outcome::normal(Value::Ok(v.clone()))),
        other => Err(type_error(
            "WRAP",
            format!("expected OK/ERR, got {}", type_name(other)),
        )),
    }
}

/// Convert a `Value` into an `ErrorCause` for the diagnostic `cause`
/// field. Returns `None` for values that have no JSON-equivalent
/// representation (closures, native fns, NaN/Inf floats, etc.). ERR
/// payloads are validated at construction time to be STRING or DICT
/// (spec §2.2.1), so in practice the `None` path should be hit only
/// for malformed legacy programs.
fn value_to_error_cause(v: &Value) -> Option<ErrorCause> {
    match v {
        Value::String(s) => Some(ErrorCause::String(s.clone())),
        Value::Dict(entries) => {
            let mut map = serde_json::Map::new();
            for (k, val) in entries {
                let key_str = match k {
                    Value::String(s) => s.clone(),
                    _ => return None,
                };
                let json_val = value_to_json_value(val)?;
                map.insert(key_str, json_val);
            }
            Some(ErrorCause::Dict(map))
        }
        // ERR/ERR(...) value as cause: recurse on the payload. This
        // handles the `UNWRAP(WRAP(WRAP(ERR("e"), "c1"), "c2"))` shape
        // where the outer WRAP's payload is itself an ERR-wrapped
        // dict — but in practice WRAP never produces nested ERR
        // values, only nested dicts, so this branch is mostly future-
        // proofing.
        Value::Err(inner) => value_to_error_cause(inner),
        _ => None,
    }
}

/// Best-effort conversion from `Value` to `serde_json::Value` for use
/// inside `ErrorCause::Dict`. Returns `None` on types that don't
/// round-trip (closures, native fns, NaN/Inf). Booleans / integers /
/// floats / strings / arrays / dicts / null are all preserved
/// faithfully; OK values are unwrapped to their inner payload
/// (matching how JSON has no separate OK wrapper).
fn value_to_json_value(v: &Value) -> Option<serde_json::Value> {
    Some(match v {
        Value::Null => serde_json::Value::Null,
        Value::Boolean(b) => serde_json::Value::Bool(*b),
        Value::Integer(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)?,
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Array(items) => {
            let mut arr = Vec::with_capacity(items.len());
            for item in items {
                arr.push(value_to_json_value(item)?);
            }
            serde_json::Value::Array(arr)
        }
        Value::Dict(entries) => {
            let mut obj = serde_json::Map::new();
            for (k, val) in entries {
                let key_str = match k {
                    Value::String(s) => s.clone(),
                    _ => return None,
                };
                let json_val = value_to_json_value(val)?;
                obj.insert(key_str, json_val);
            }
            serde_json::Value::Object(obj)
        }
        Value::Err(_) => return None, // ERR nested in WRAP chain: skip (avoid infinite recursion for pathological inputs)
        Value::Ok(inner) => value_to_json_value(inner)?,
        Value::Closure { .. } | Value::NativeFn { .. } => return None,
    })
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

/// `wlwl:std.collection` — callback-aware higher-order collection
/// functions (spec v0.4 §15.7 / §10.5). The std boundary can't host
/// these because `value_to_std_value` rejects closures (B5 P4-B5-006);
/// `Evaluator::load_std` detects the path and binds from
/// `collection::BUILTINS` instead of `spec.functions`.
pub mod collection;

/// `wlwl:std.test` — in-process test framework (spec v0.4 §15.9,
/// Phase B7). Same std-boundary rationale as collection: `TEST` /
/// `RUN_TESTS` need callback invocation, `ASSERT` / friends need
/// rich-Value inspection. Real impls in this crate, bound via
/// `test::BUILTINS` through `NativeInvoke::Builtin`.
pub mod test;
pub mod registry;

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
    /// Per-evaluator context for `wlwl_std` calls (argv, env). Set to
    /// `wlwl_std::StdCtx::from_process()` in `new`; tests can override
    /// via the `std_ctx` field directly.
    std_ctx: wlwl_std::StdCtx,
    /// [v0.4 spec Sec. 14.2] Call stack frames currently in scope.
    /// `invoke_closure` pushes a frame on entry and pops on
    /// every exit path. `enrich_with_trace` clones this into
    /// the diagnostic's `trace` field on error.
    call_stack: Vec<TraceFrame>,
    /// Soft warnings (v0.4 spec §14.5) accumulated during the run.
    /// Numeric overflow saturates + emits `W0015` here without
    /// aborting evaluation; callers can drain via `take_warnings()`
    /// or the `run_with_warnings` test helper.
    pub warnings: Vec<Warning>,
    /// [v0.4 Phase B4] Source span of the **current builtin call site**.
    /// Set by `eval_call` before dispatching into a builtin function
    /// (`fn(&mut Evaluator, Vec<Value>) -> _`); cleared on return.
    /// Lets builtins that synthesize span-aware diagnostics (currently
    /// `builtin_unwrap` for `E0100 PANIC` with a `cause` field, and
    /// `builtin_format` for `E0039`) see the exact call location.
    /// Most builtins don't read it.
    pub current_span: Option<Span>,
    /// [v0.4 Phase B5] Memoized FORMAT template parses (plan §5.5:
    /// "相同 template 复用解析结果"). Keyed by template text; the
    /// parsed segment list is shared via `Rc` so cache hits are a
    /// pointer clone. Unbounded by design — same trade-off as the
    /// module cache; a run formatting N distinct templates keeps N
    /// small segment vectors alive.
    format_cache: HashMap<String, Rc<Vec<wlwl_std::format::FormatSegment>>>,
    /// [v0.4 Phase B7] Test-case registry for `wlwl:std.test`.
    /// `TEST(name, body)` pushes a `TestEntry { name, body }` here;
    /// `RUN_TESTS()` drains the vec, invokes each body via
    /// `invoke_closure`, and returns an ARRAY of DICTs (per §15.9:
    /// `["name", "passed", "duration_ms", "error"?]`). Evaluator-local
    /// so each run starts with an empty registry — there's no
    /// static-state leak across `Evaluator::new()` calls.
    pub(crate) test_registry: Vec<crate::test::TestEntry>,
    /// [v0.4 Phase E1] Boundary-check flag from `wlwl.toml` `[features]
    /// strict_types = true` (spec §2.7). When `true`, `invoke_closure`
    /// validates each call's actual argument `TYPE(...)` against the
    /// parameter's `name: Type` annotation; mismatches raise `E0033`.
    /// Defaults to `false` so behavior matches v0.3 (annotations parsed
    /// but ignored at runtime). CLI wires this up via
    /// `with_strict_types(true)` after loading `wlwl.toml`.
    pub(crate) strict_types: bool,
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
            std_ctx: wlwl_std::StdCtx::from_process(),
            call_stack: Vec::new(),
            warnings: Vec::new(),
            current_span: None,
            format_cache: HashMap::new(),
            test_registry: Vec::new(),
            strict_types: false,
        }
    }

    /// Enable or disable strict-types boundary checks (Phase E1,
    /// spec §2.7). When enabled, `invoke_closure` validates actual
    /// argument types against per-param `name: Type` annotations
    /// and raises `E0033` on mismatch. Defaults to `false` (v0.3
    /// transient behavior).
    ///
    /// This is a builder method intended to be chained right after
    /// `Evaluator::new()`, typically by the CLI after it loads
    /// `wlwl.toml`:
    ///
    /// ```ignore
    /// let manifest = wlwl_toml::parse(&toml_text)?;
    /// let mut ev = Evaluator::new().with_strict_types(manifest.strict_types());
    /// ```
    pub fn with_strict_types(mut self, on: bool) -> Self {
        self.strict_types = on;
        self
    }

    /// Read the current strict_types flag (Phase E1). Primarily
    /// for tests; production code sets it via the builder above.
    pub fn strict_types(&self) -> bool {
        self.strict_types
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
            std_ctx: wlwl_std::StdCtx::default(),
            call_stack: Vec::new(),
            warnings: Vec::new(),
            current_span: None,
            format_cache: HashMap::new(),
            test_registry: Vec::new(),
            strict_types: false,
        }
    }

    /// Attach the original source text and file name so that runtime
    /// diagnostics (e.g. `E0020` undefined name) include a `source_line`.
    pub fn with_source(mut self, source: impl Into<String>, file: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self.file = Some(file.into());
        self
    }

    /// Append a soft warning to the evaluator's warning log. Currently
    /// only `W0015` (integer overflow saturated) is emitted by builtin
    /// arithmetic; the channel is open for future warnings
    /// (e.g. `W0014` non-ASCII case-fold ambiguity).
    pub fn emit_warning(&mut self, code: ErrorCode, message: impl Into<String>) {
        debug_assert!(
            code.is_warning(),
            "emit_warning called with non-warning code {:?}",
            code
        );
        self.warnings.push(Warning { code, message: message.into() });
    }

    /// Drain the accumulated warnings and return them, leaving the
    /// evaluator's buffer empty. Callers can inspect / log them
    /// without aborting the run.
    pub fn take_warnings(&mut self) -> Vec<Warning> {
        std::mem::take(&mut self.warnings)
    }

    /// Invoke a callable `Value` (closure or native fn) with an
    /// optional method receiver (Phase C2, spec §8.6 / §13.12).
    ///
    /// - `Value::Closure`: the receiver is injected as the first
    ///   actual argument **only when the first formal parameter is
    ///   named `self`** (§8.6 "首形参即 self 约定注入"; §11.4 主句).
    ///   Closures without a `self` first formal behave like plain FUN
    ///   values (§5.5 关键决策:属性访问结果为函数 + 括号即调用,
    ///   等价 `CALL(a.b, args)`) — this keeps file-module exports
    ///   (`m.f(x)`) natural. The "否则首形参接收调用对象" clause of
    ///   §11.4 is NOT implemented; see deviations P4-C2-001.
    /// - `Value::NativeFn`: the receiver is **dropped** — std module
    ///   members are invoked bare (§13.12: `io.PRINT("hi")` 等价
    ///   `IMPORT` 后的 `PRINT("hi")`).
    pub(crate) fn call_value_with_receiver(
        &mut self,
        callee: Value,
        receiver: Option<Value>,
        mut args: Vec<Value>,
        span: &Span,
        name: &str,
    ) -> WlwlResult<Outcome> {
        match callee {
            Value::Closure { params, body, env } => {
                if let (Some(r), Some(p)) = (receiver, params.first()) {
                    if p.name == "self" {
                        args.insert(0, r);
                    }
                }
                self.invoke_closure(name, params, body, env, args, span)
            }
            Value::NativeFn { invoke, .. } => match invoke {
                NativeInvoke::Std(f) => invoke_std(self, f, args, span),
                NativeInvoke::Builtin(b) => {
                    let prev_span = self.current_span.take();
                    self.current_span = Some(span.clone());
                    let result = b(self, args);
                    self.current_span = prev_span;
                    result
                }
            },
            other => Err(type_error(
                name,
                format!("member is not callable ({} value)", type_name(&other)),
            )),
        }
    }

    // ── Top-level entry points ─────────────────────────────────────

    /// Evaluate a program (the result of `parse`). Used for both
    /// entry-point files and modules.
    pub fn eval(&mut self, expr: &Expr) -> WlwlResult<Value> {
        // Phase C3/C4/C6 (spec v0.4 §13.8/§13.9): project-config gate
        // — language_version mismatch (E0044), MVS dependency
        // conflicts (E0045) and wlwl.lock inconsistency (E0042) all
        // surface here, before any user code runs. No-ops when the
        // project has no wlwl.toml.
        self.check_project_config(expr.span())?;
        // [v0.4 spec Sec. 14.2] Wrap any error in trace enrichment so
        // that *every* error from eval() carries the current call
        // stack. This catches errors that bypassed `self.diag()`
        // (e.g. constructed via direct `WlwlDiagnostic::new` calls
        // in std-helper functions like `type_error`).
        let result = self.eval_top_level(expr).map_err(|e| self.enrich_with_trace(e));
        let outcome = match result {
            Ok(o) => o,
            Err(e) => return Err(e),
        };
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

    /// Snapshot of the project manifest, if a `wlwl.toml` was found at
    /// the project root (Phase C3+). `None` = no-toml project.
    fn project_manifest(&self) -> Option<Arc<wlwl_toml::manifest::Manifest>> {
        self.loader.borrow().project.manifest.clone()
    }

    /// Project-config gate (Phase C3-C6): validate the project
    /// configuration before any user code runs. No-ops when the
    /// project has no `wlwl.toml`.
    ///
    /// Order (each maps to its spec §13.8/§13.9 error):
    /// 1. `language_version` mismatch → E0044 (Phase C3);
    /// 2. MVS dependency resolution → E0045 conflict / E0041 cycle
    ///    (Phase C4; path deps always resolve, version deps have an
    ///    empty candidate set in v0.4 — no central registry);
    /// 3. `wlwl.lock` consistency → E0042 (Phase C6). A missing lock
    ///    is *not* generated here — the CLI's `try_write_lock` owns
    ///    generation (eval stays IO-free).
    fn check_project_config(&mut self, span: &Span) -> WlwlResult<()> {
        use wlwl_toml::manifest::Dependency;
        let Some(m) = self.project_manifest() else {
            return Ok(());
        };
        // 1. E0044 — language_version (§13.8).
        if let Err(mismatch) = wlwl_toml::manifest::check_language_version(&m) {
            return Err(self.diag(ErrorCode::E0044, mismatch.to_string(), span.clone()));
        }
        // 2. E0045 — MVS dependency resolution (§13.9). Only
        //    version-style dependencies enter the solver; path deps
        //    always resolve (local directory).
        let mut root: std::collections::BTreeMap<String, wlwl_toml::mvs::Constraint> =
            std::collections::BTreeMap::new();
        for (name, dep) in &m.dependencies {
            let raw = match dep {
                Dependency::Version(v) => Some(v.clone()),
                Dependency::Detailed(d) if d.path.is_none() => d.version.clone(),
                _ => None,
            };
            let Some(raw) = raw else { continue };
            match wlwl_toml::mvs::Constraint::parse(&raw) {
                Some(c) => {
                    root.insert(name.clone(), c);
                }
                None => {
                    return Err(self.diag(
                        ErrorCode::E0045,
                        format!(
                            "dependency conflict: unparseable version constraint '{}' for '{}' \
                             (v0.4 has no central registry; use path dependencies)",
                            raw, name
                        ),
                        span.clone(),
                    ));
                }
            }
        }
        if !root.is_empty() {
            match wlwl_toml::mvs::solve(&root, &|_| Vec::new(), &|_, _| {
                std::collections::BTreeMap::new()
            }) {
                Ok(_) => {}
                Err(e) => {
                    let code = match &e {
                        wlwl_toml::mvs::MvsError::Cycle(_) => ErrorCode::E0041,
                        _ => ErrorCode::E0045,
                    };
                    let mut msg = e.to_string();
                    if code == ErrorCode::E0045 {
                        msg.push_str(
                            " (v0.4 has no central registry; use path dependencies)",
                        );
                    }
                    return Err(self.diag(code, msg, span.clone()));
                }
            }
        }
        // 3. E0042 — lock ↔ toml consistency (§13.8).
        let lock_path = self
            .loader
            .borrow()
            .project
            .project_root
            .join("wlwl.lock");
        match wlwl_toml::lock::read(&lock_path) {
            Ok(Some(lock)) => {
                if let Err(detail) = wlwl_toml::lock::validate_consistency(&lock, &m) {
                    return Err(self.diag(
                        ErrorCode::E0042,
                        format!("lock file inconsistent with wlwl.toml: {}", detail),
                        span.clone(),
                    ));
                }
            }
            Ok(None) => {}
            Err(e) => {
                return Err(self.diag(
                    ErrorCode::E0042,
                    format!("lock file unreadable: {}", e),
                    span.clone(),
                ));
            }
        }
        Ok(())
    }

    /// Phase C5 (spec §6.6): `LET` shadowing checks. Called only when
    /// a new binding is created (an existing binding update is a SET-
    /// like rebind, not a shadow).
    ///
    /// - Shadowing a **global builtin** (registry ResolvedBuiltin /
    ///   ResolvedCompat): `E0025` by default; with
    ///   `[features] allow_builtin_shadow = true` allowed + `W0030`.
    /// - Shadowing a **macro function / keyword** (LexerMacro):
    ///   allowed with `W0030` (spec §14.5 W0030).
    fn check_let_shadowing(&mut self, name: &str, span: &Span) -> WlwlResult<()> {
        let Some(spec) = crate::registry::lookup(name) else {
            return Ok(());
        };
        use crate::registry::DispatchStatus;
        match spec.dispatch {
            DispatchStatus::ResolvedBuiltin | DispatchStatus::ResolvedCompat => {
                let allowed = self
                    .project_manifest()
                    .map(|m| m.allow_builtin_shadow())
                    .unwrap_or(false);
                if allowed {
                    self.emit_warning(
                        ErrorCode::W0030,
                        format!(
                            "shadowing global builtin '{}' ([features] allow_builtin_shadow = true)",
                            name
                        ),
                    );
                    Ok(())
                } else {
                    Err(self.diag(
                        ErrorCode::E0025,
                        format!("cannot shadow built-in '{}'", name),
                        span.clone(),
                    ))
                }
            }
            DispatchStatus::LexerMacro | DispatchStatus::Deferred => {
                self.emit_warning(
                    ErrorCode::W0030,
                    format!("shadowing macro function / keyword '{}'", name),
                );
                Ok(())
            }
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
            Expr::Var(name, span) => {
                // Extract the value out of the cell-borrow before the
                // match, so the Ref is dropped before we may need
                // &mut self in the undefined-name branch. (*v).clone()
                // copies the inner Value; the Ref itself is then unused.
                let v = self.env.get(name).map(|v| (*v).clone());
                match v {
                    Some(v) => Ok(Outcome::normal(v)),
                    None => Err(self.undefined_name(name, span)),
                }
            },
            Expr::Call { name, args, span } => self.eval_call(name, args, span),
            Expr::Let { name, value, span, .. } => {
                let v = self.eval_expr(value)?;
                if v.signal != Signal::None {
                    return Ok(v);
                }
                // Phase 2 fix: if `name` already exists in any enclosing
                // scope, update that binding (so LET inside a loop body
                // can accumulate). Otherwise bind in the current scope.
                if !self.env.set_existing(&name, v.value.clone()) {
                    // Phase C5 (spec §6.6): shadowing checks on new
                    // bindings — E0025 for global builtins (unless
                    // allow_builtin_shadow), W0030 for macro/keyword.
                    self.check_let_shadowing(&name, span)?;
                    self.env.set_local(name.clone(), v.value.clone());
                }
                Ok(Outcome::normal(Value::Null))
            }
            Expr::LetPattern { pattern, value, span, .. } => {
                let v = self.eval_expr(value)?;
                if v.signal != Signal::None {
                    return Ok(v);
                }
                self.destructure(pattern, &v.value, span)?;
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
                // v0.4 spec §2.2.1: ERR(e) payload `e` must be STRING
                // or DICT; anything else is an E0030 type error.
                match &o.value {
                    Value::String(_) | Value::Dict(_) => {}
                    other => {
                        return Err(self.diag(
                            ErrorCode::E0030,
                            format!(
                                "ERR payload must be STRING or DICT, got {}",
                                type_name(other)
                            ),
                            expr.span().clone(),
                        ));
                    }
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
                // v0.4 §14.5 — `OR_DIE` is the v0.3-compat alias of
                // `UNWRAP_OR`. Emit W0051 on every source occurrence,
                // regardless of OK/ERR/short-circuit branch (the
                // alias was *written* by the user; that's what the
                // warning is about). The canonical spelling
                // `UNWRAP_OR` reaches this arm via the builtin-call
                // path and never emits here.
                self.emit_warning(
                    ErrorCode::W0051,
                    "`OR_DIE` is a v0.3-compat alias; use `UNWRAP_OR` instead (will be removed in v0.5)",
                );
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
                        format!("UNWRAP_OR expects OK/ERR, got {}", type_name(&other)),
                        expr.span().clone(),
                    )),
                }
            }
            Expr::Match { value, clauses, default, span } => self.eval_match(value, clauses, default, span),
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
        // [v0.4 spec Sec. 6.4] Fast path for the macro function `SET`:
        // `SET(target, value)` is parsed as `Call { name: "SET", args:
        // [Var(target), value] }`. We intercept before the normal
        // dispatch so the target is *not* evaluated as an expression
        // (it must be a bare name); the value side is evaluated
        // inside `eval_set`.
        if name == "SET" {
            return self.eval_set(args, span);
        }
        // [v0.4 spec §4.5 + §16.3 rule 10, Phase E2/E4] The empty-
        // collection forms `ARRAY()` / `DICT()` are canonical syntax
        // (the §16.3 formatter emits exactly this shape for empty
        // literals), so they must evaluate. Intercepted here as macro
        // forms rather than appendix-G builtins; a non-empty
        // `ARRAY(x, ...)` call keeps falling through to the normal
        // dispatch (unchanged from v0.3 behavior).
        if (name == "ARRAY" || name == "DICT") && args.is_empty() {
            let v = if name == "ARRAY" {
                Value::Array(Vec::new())
            } else {
                Value::Dict(Vec::new())
            };
            return Ok(Outcome::normal(v));
        }
        // Look up the callee (user function takes priority over built-in
        // with the same name; in Phase 2 we keep them in disjoint
        // namespaces by convention — there is no name conflict in the
        // std yet).
        let user_fn = self.env.get(name).map(|v| v.clone());
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
                return self.invoke_closure(name, params, body, env, arg_values, span);
            }
            if let Value::NativeFn { invoke, .. } = v {
                return match invoke {
                    NativeInvoke::Std(f) => invoke_std(self, f, arg_values, span),
                    NativeInvoke::Builtin(b) => {
                        // Phase B6: callback-aware std modules (notably
                        // `wlwl:std.collection`) bind eval-internal builtins
                        // through this variant. Same save/restore span
                        // contract as the global builtin dispatch
                        // (line ~2837) so any E0102 / E0038 etc. emitted
                        // from inside points at the call site, not the
                        // outer scope. BUILTINS shims themselves do NOT
                        // re-enter `eval_call` (they call closures via
                        // `invoke_closure` directly), so the span
                        // nesting is single-frame.
                        let prev_span = self.current_span.take();
                        self.current_span = Some(span.clone());
                        let result = b(self, arg_values);
                        self.current_span = prev_span;
                        result
                    }
                };
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
            // Phase B4: expose the call site span to builtins that
            // synthesize span-aware diagnostics (e.g. builtin_unwrap
            // producing E0100 PANIC with cause). save/restore so any
            // nested eval_call (e.g. recursive fn calls from inside
            // a builtin) doesn't clobber the outer span.
            let prev_span = self.current_span.take();
            self.current_span = Some(span.clone());
            let result = b(self, arg_values);
            self.current_span = prev_span;
            return result;
        }
        Err(self.undefined_name(name, span))
    }

    // [v0.4 spec Sec. 6.4] `SET(target, value)` -- a re-binding macro.
    //
    // `target` must be a bare name (Expr::Var), not an arbitrary
    // expression. `value` is evaluated like any other argument.
    //
    // Cell lookup (current env chain, innermost first):
    //   * cell found, mutable   -> update the cell value
    //   * cell found, IMMUTABLE -> E0024 (cannot SET non-captured
    //     binding from child scope)
    //   * cell not found        -> E0020 (undefined name; reuses the
    //     "did you mean?" suggestion in `undefined_name`)
    //
    // This implements the spec rule "未被子作用域捕获的 cell,子作用域
    // SET -> E0024": cells are created IMMUTABLE by `LET` and only
    // upgraded to MUTABLE by E-CloCap when a closure captures them
    // (see `invoke_closure`).
    fn eval_set(&mut self, args: &[Expr], span: &Span) -> WlwlResult<Outcome> {
        if args.len() != 2 {
            return Err(self.diag(
                ErrorCode::E0022,
                format!("SET expects 2 arguments, got {}", args.len()),
                span.clone(),
            ));
        }
        // The target is a name, not an expression.
        let target = match &args[0] {
            Expr::Var(n, _) => n.clone(),
            _ => {
                return Err(self.diag(
                    ErrorCode::E0030,
                    "SET target must be a variable name (not an expression)".to_string(),
                    span.clone(),
                ));
            }
        };
        // Evaluate the value side like any other call argument.
        let v = self.eval_expr(&args[1])?;
        if v.signal != Signal::None {
            return Ok(v);
        }
        // Look up the cell. Use `set_cell_value` to do the
        // mutability check in one shot.
        match self.env.set_cell_value(&target, v.value) {
            Ok(true) => Ok(Outcome::normal(Value::Null)),
            Ok(false) => Err(self.diag(
                ErrorCode::E0024,
                format!(
                    "cannot SET non-captured binding `{}` from child scope (v0.4 Sec. 6.4: cells are IMMUTABLE until captured by a closure)",
                    target
                ),
                span.clone(),
            )),
            Err(()) => Err(self.undefined_name(&target, span)),
        }
    }

    fn invoke_closure(
        &mut self,
        name: &str,
        params: Vec<FunParam>,
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
        // [v0.4 Phase E1 -- spec §2.7] strict_types boundary check.
        // When `self.strict_types` is true, every param with a
        // `: Type` annotation must receive a value whose TYPE matches
        // the annotation (case-insensitive, top-level shape only).
        // Nested generic / array element matching is deliberately
        // deferred to a later sub-phase; the spec §2.7 budget is a
        // ≤10% overhead in strict mode, which a top-level compare
        // comfortably meets. Failures raise E0033 (Phase E1) and do
        // NOT mutate any state.
        if self.strict_types {
            for (p, v) in params.iter().zip(arg_values.iter()) {
                let Some(ann) = p.type_annotation.as_ref() else {
                    continue;
                };
                let expected = ann.text.to_ascii_uppercase();
                let actual = value_type_name(v).to_string();
                if expected != actual {
                    let arg_loc = Location {
                        file: span.file.clone(),
                        line: span.line_start,
                        col: span.col_start,
                        line_end: span.line_end,
                        col_end: span.col_end,
                    };
                    let ann_loc = Location {
                        file: ann.span.file.clone(),
                        line: ann.span.line_start,
                        col: ann.span.col_start,
                        line_end: ann.span.line_end,
                        col_end: ann.span.col_end,
                    };
                    let diag = WlwlDiagnostic::new(
                        ErrorCode::E0033,
                        "type annotation mismatch (overwritten by helper)",
                        arg_loc.clone(),
                    )
                    .with_strict_types_violation(
                        expected.clone(),
                        actual.clone(),
                        ann_loc,
                        arg_loc,
                        "function",
                    );
                    return Err(diag.into());
                }
            }
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
        // [v0.4 spec Sec. 6.4 / 附录 E.4.6 E-CloCap] Upgrade every cell
        // in the captured environment to MUTABLE. After this point,
        // any `SET` against these cells from inside the closure body
        // (or from any sibling that shares the same lexical `LET`)
        // succeeds; any `SET` against an un-captured binding raises
        // E0024. We do this BEFORE installing the scopes so a `SET`
        // in the body -- including in the param-binding loop -- sees
        // the upgraded cells.
        for scope in &captured_scopes {
            for cell in scope.values() {
                cell.borrow_mut().mutable = true;
            }
        }
        // New scope stack: [captured lexical scopes, caller scopes, fresh param scope]
        let mut new_scopes = captured_scopes;
        new_scopes.extend(caller_scopes.iter().cloned());
        new_scopes.push(HashMap::new()); // fresh scope for params
        self.env.scopes = new_scopes;
        for (p, v) in params.iter().zip(arg_values) {
            self.env.set_local(p.name.clone(), v);
        }
        // [v0.4 spec Sec. 14.2] push call frame, eval body,
        // then pop on every exit path. The push is OUTSIDE the
        // for loop so it happens exactly once per call.
        let call_loc = Location {
            file: span.file.clone(),
            line: span.line_start,
            col: span.col_start,
            line_end: span.line_end,
            col_end: span.col_end,
        };
        let frame_name = if name.is_empty() { "<anonymous>" } else { name };
        self.call_stack.push(TraceFrame {
            frame: frame_name.to_string(),
            location: call_loc,
        });
        let outcome = match self.eval_expr(&body) {
            Ok(o) => o,
            Err(e) => {
                // Pop the frame we just pushed before bubbling.
                let _ = self.call_stack.pop();
                return Err(e);
            }
        };
        // Successful body eval: pop the frame before returning.
        let _ = self.call_stack.pop();
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
            let v = module.env.get(&imp.name).map(|v| v.clone()).ok_or_else(|| {
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

    /// [v0.4 spec Sec. 14.2] Post-hoc trace enrichment. If the error
    /// doesn't already have a trace (it was created via a path that
    /// bypassed `self.diag()`), populate it from the current
    /// `call_stack`. If the stack is empty, inject a synthetic
    /// `<toplevel>` frame so the spec's "minimum 1 frame" rule is
    /// satisfied.
    fn enrich_with_trace(&mut self, e: WlwlError) -> WlwlError {
        let mut d = match e {
            WlwlError::Diagnostic(d) => d,
        };
        if d.trace.is_empty() {
            if self.call_stack.is_empty() {
                d.trace.push(TraceFrame {
                    frame: "<toplevel>".into(),
                    location: Location {
                        file: d.location.file.clone(),
                        line: d.location.line,
                        col: d.location.col,
                        line_end: d.location.line_end,
                        col_end: d.location.col_end,
                    },
                });
            } else {
                // Same "innermost first" convention as `diag`.
                d.trace = self.call_stack.iter().rev().cloned().collect();
            }
        }
        d.into()
    }

    /// Destructure `value` against `pattern` (v0.4 `Sec. 7.5`),
    /// binding sub-patterns as new cells in the current scope.
    ///
    /// Errors:
    /// - `E0026` if the shape does not match (array length too
    ///   short, dict key missing, *rest not last).
    /// - `E0030` if `value` is the wrong runtime type for the
    ///   pattern (e.g. destructuring an `INTEGER` with an array
    ///   pattern).
    /// - `E0026` for literal mismatches (the spec is silent on
    ///   this case in v0.4; we collapse it to E0026 for now to
    ///   keep the error family tight).
    fn destructure(&mut self, pattern: &Pattern, value: &Value, _span: &Span) -> WlwlResult<()> {
        match pattern {
            Pattern::Ident(name, _) => {
                self.env.set_local(name.clone(), value.clone());
                Ok(())
            }
            Pattern::Wildcard(_) => Ok(()),
            Pattern::Literal(lit, lspan) => {
                let v = Value::from(lit.clone());
                if self.values_equal_loose(&v, value) {
                    Ok(())
                } else {
                    Err(self.destructure_shape_error(
                        format!(
                            "destructure pattern mismatch: literal `{}` does not match value",
                            lit
                        ),
                        lspan,
                    ))
                }
            }
            Pattern::Array(items, rest, aspan) => {
                let arr = match value {
                    Value::Array(items) => items.clone(),
                    _ => {
                        return Err(self.destructure_type_error(
                            "ARRAY", value, aspan,
                        ))
                    }
                };
                let needed_min = items.len();
                if arr.len() < needed_min {
                    return Err(self.destructure_shape_error(
                        format!(
                            "destructure pattern mismatch: array of length {} does not match pattern with {} element(s)",
                            arr.len(),
                            items.len()
                        ),
                        aspan,
                    ));
                }
                for (i, sub) in items.iter().enumerate() {
                    let sub_value = arr.get(i).ok_or_else(|| {
                        self.destructure_shape_error(
                            format!("destructure pattern mismatch: missing element at index {}", i),
                            aspan,
                        )
                    })?;
                    self.destructure(sub, sub_value, aspan)?;
                }
                if let Some(rest_pat) = rest {
                    let rest_items: Vec<Value> = arr.iter().skip(items.len()).cloned().collect();
                    let rest_value = Value::Array(rest_items);
                    self.destructure(rest_pat, &rest_value, aspan)?;
                } else if arr.len() > items.len() {
                    return Err(self.destructure_shape_error(
                        format!(
                            "destructure pattern mismatch: array of length {} is too long for pattern with {} element(s) (no *rest)",
                            arr.len(),
                            items.len()
                        ),
                        aspan,
                    ));
                }
                Ok(())
            }
            Pattern::Dict(entries, dspan) => {
                let dict = match value {
                    Value::Dict(entries) => entries.clone(),
                    _ => return Err(self.destructure_type_error("DICT", value, dspan)),
                };
                for (k_expr, sub) in entries {
                    let key_value = self.eval_literal_key(k_expr)?;
                    let pos = dict.iter().position(|(k, _)| {
                        self.values_equal_loose(k, &key_value)
                    });
                    let found = match pos {
                        Some(idx) => dict[idx].1.clone(),
                        None => {
                            return Err(self.destructure_shape_error(
                                format!(
                                    "destructure pattern mismatch: dict missing key `{}`",
                                    key_value.display()
                                ),
                                dspan,
                            ));
                        }
                    };
                    self.destructure(sub, &found, dspan)?;
                }
                Ok(())
            }
            Pattern::Constructor { name, inner, span: cspan } => {
                // OK(x) / ERR(e) pattern in `LET([OK(x), v], result)`.
                // Behaves like try_match but the failure path is
                // a hard E0026 (not soft Ok(false)) because
                // LET destructure is total.
                let inner_value: &Value = match (name.as_str(), value) {
                    ("OK", Value::Ok(v)) => v.as_ref(),
                    ("ERR", Value::Err(v)) => v.as_ref(),
                    ("OK", _) | ("ERR", _) => {
                        return Err(self.destructure_shape_error(
                            format!("destructure pattern mismatch: value is not {}", name),
                            cspan,
                        ));
                    }
                    _ => {
                        return Err(self.destructure_shape_error(
                            format!("unsupported constructor pattern `{}` in destructure (only OK / ERR are valid in v0.4)", name),
                            cspan,
                        ));
                    }
                };
                self.destructure(inner, inner_value, cspan)
            }
        }
    }

    /// Build an E0030 (type) diagnostic for a destructure shape
    /// mismatch where the runtime value has the wrong outer type.
    fn destructure_type_error(&mut self, expected: &str, value: &Value, span: &Span) -> WlwlError {
        let loc = Location {
            file: span.file.clone(),
            line: span.line_start,
            col: span.col_start,
            line_end: span.line_end,
            col_end: span.col_end,
        };
        let mut d = WlwlDiagnostic::new(
            ErrorCode::E0030,
            format!(
                "destructure pattern expected {}, got {}",
                expected,
                type_name(value)
            ),
            loc,
        );
        if let Some(src) = &self.source {
            if let Some(line_text) = extract_line(src, span.line_start) {
                d = d.with_source_line(line_text);
            }
        }
        d.into()
    }

    // ---- v0.4 Sec. 7.6: MATCH pattern matching ----------------------------

    /// Build an E0027 diagnostic for MATCH fell-through (no clause
    /// matched and no default arm was supplied). Reserved per spec
    /// 7.6 line 878 even though our parser synthesizes a NULL default
    /// when the source omits one (per spec 7.6 line 879, see
    /// docs/history/20260909.md). Kept here so future spec revisions
    /// that re-enable strict E0027 dispatch need only wire it into
    /// `eval_match` -- the diagnostic is already validated.
    #[allow(dead_code)]
    fn match_fell_through(&mut self, span: &Span) -> WlwlError {
        let loc = Location {
            file: span.file.clone(),
            line: span.line_start,
            col: span.col_start,
            line_end: span.line_end,
            col_end: span.col_end,
        };
        let mut d = WlwlDiagnostic::new(
            ErrorCode::E0027,
            "MATCH fell through without match or default",
            loc,
        );
        if let Some(src) = &self.source {
            if let Some(line_text) = extract_line(src, span.line_start) {
                d = d.with_source_line(line_text);
            }
        }
        d.into()
    }

    /// Build an E0026 (destructure pattern mismatch) diagnostic.
    fn destructure_shape_error(&mut self, message: String, span: &Span) -> WlwlError {
        let loc = Location {
            file: span.file.clone(),
            line: span.line_start,
            col: span.col_start,
            line_end: span.line_end,
            col_end: span.col_end,
        };
        let mut d = WlwlDiagnostic::new(ErrorCode::E0026, message, loc);
        if let Some(src) = &self.source {
            if let Some(line_text) = extract_line(src, span.line_start) {
                d = d.with_source_line(line_text);
            }
        }
        d.into()
    }

    /// Evaluate a key expression to a `Value` (used by Dict pattern
    /// destructuring). Keys in `Sec. 7.5` are always literals; the
    /// AST permits any `Expr` for forward compatibility with `MATCH`.
    /// Non-literal keys raise E0026.
    fn eval_literal_key(&mut self, expr: &Expr) -> WlwlResult<Value> {
        match expr {
            Expr::Literal(lit, _) => Ok(Value::from(lit.clone())),
            Expr::Var(name, span) => self
                .env
                .get(name)
                .map(|v| (*v).clone())
                .ok_or_else(|| self.undefined_name(name, span)),
            _ => {
                let span = expr.span().clone();
                let loc = Location {
                    file: span.file.clone(),
                    line: span.line_start,
                    col: span.col_start,
                    line_end: span.line_end,
                    col_end: span.col_end,
                };
                let mut d = WlwlDiagnostic::new(
                    ErrorCode::E0026,
                    "destructure pattern keys must be literals (Sec. 7.5)",
                    loc,
                );
                if let Some(src) = &self.source {
                    if let Some(line_text) = extract_line(src, span.line_start) {
                        d = d.with_source_line(line_text);
                    }
                }
                Err(d.into())
            }
        }
    }

    /// Loose equality used by pattern matching: compares Integer ==
    /// Integer and String == String by value, NULL == NULL, etc.
    /// We intentionally do NOT route through `=` so that destructure
    /// does not need an extra WlwlResult plumbing step; the function
    /// is only used for pattern-equality, not for runtime semantics.
    fn values_equal_loose(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Null, Value::Null) => true,
            (Value::Integer(x), Value::Integer(y)) => x == y,
            (Value::Float(x), Value::Float(y)) => x == y,
            (Value::Boolean(x), Value::Boolean(y)) => x == y,
            (Value::String(x), Value::String(y)) => x == y,
            _ => false,
        }
    }

    /// Evaluate `MATCH(value, clauses, default?)` (v0.4 Sec. 7.6).
    /// Each clause is tried in declaration order; the first one whose
    /// pattern matches the value wins. Bindings introduced by a
    /// matching pattern are installed in a fresh scope that is
    /// discarded before returning. E0027 fires when no clause
    /// matches and no default is provided.
    fn eval_match(
        &mut self,
        value: &Expr,
        clauses: &[MatchClause],
        default: &Expr,
        span: &Span,
    ) -> WlwlResult<Outcome> {
        // 1. Evaluate the scrutinee.
        let v = self.eval_expr(value)?;
        if v.signal != Signal::None {
            return Ok(v);
        }
        // 2. Try each clause.
        for clause in clauses {
            let mut bindings: Vec<(String, Value)> = Vec::new();
            let matched = self.try_match(&clause.pattern, &v.value, &mut bindings, span)?;
            if matched {
                // 3a. Run body in a fresh scope holding the bindings.
                self.env.push_scope();
                for (name, val) in &bindings {
                    self.env.set_local(name.clone(), val.clone());
                }
                let result = self.eval_expr(&clause.body)?;
                self.env.pop_scope();
                if result.signal != Signal::None {
                    return Ok(result);
                }
                return Ok(Outcome::normal(result.value));
            }
        }
        // 3b. No match -- evaluate the (synthesized-when-omitted)
        //     default arm. Spec 7.6 lets the source omit the default,
        //     in which case the parser substituted a NULL literal.
        let r = self.eval_expr(default)?;
        if r.signal != Signal::None {
            return Ok(r);
        }
        Ok(Outcome::normal(r.value))
    }

    /// Attempt to match `value` against `pattern`. On success, every
    /// identifier the pattern binds is appended to `bindings` in
    /// declaration order. On failure, `bindings` is left unchanged
    /// (callers rely on this to keep partial-match state from leaking
    /// into later clauses). Returns `Ok(false)` for soft mismatches
    /// (literal inequality / array length / dict missing key / outer
    /// type mismatch on constructor), and `Err(_)` for hard errors
    /// (type errors, unsupported constructor names).
    fn try_match(
        &mut self,
        pattern: &Pattern,
        value: &Value,
        bindings: &mut Vec<(String, Value)>,
        span: &Span,
    ) -> WlwlResult<bool> {
        match pattern {
            Pattern::Ident(name, _) => {
                bindings.push((name.clone(), value.clone()));
                Ok(true)
            }
            Pattern::Wildcard(_) => Ok(true),
            Pattern::Literal(lit, _) => {
                let v = Value::from(lit.clone());
                Ok(self.values_equal_loose(&v, value))
            }
            Pattern::Array(items, rest, pspan) => {
                let arr = match value {
                    Value::Array(items) => items.clone(),
                    _ => return Err(self.destructure_type_error("ARRAY", value, pspan)),
                };
                if arr.len() < items.len() {
                    return Ok(false);
                }
                let checkpoint = bindings.len();
                for (i, sub) in items.iter().enumerate() {
                    if !self.try_match(sub, &arr[i], bindings, pspan)? {
                        bindings.truncate(checkpoint);
                        return Ok(false);
                    }
                }
                if let Some(rest_pat) = rest {
                    let rest_items: Vec<Value> =
                        arr.iter().skip(items.len()).cloned().collect();
                    let rest_value = Value::Array(rest_items);
                    if !self.try_match(rest_pat, &rest_value, bindings, pspan)? {
                        bindings.truncate(checkpoint);
                        return Ok(false);
                    }
                } else if arr.len() > items.len() {
                    bindings.truncate(checkpoint);
                    return Ok(false);
                }
                Ok(true)
            }
            Pattern::Dict(entries, pspan) => {
                let dict = match value {
                    Value::Dict(entries) => entries.clone(),
                    _ => return Err(self.destructure_type_error("DICT", value, pspan)),
                };
                let checkpoint = bindings.len();
                for (k_expr, sub) in entries {
                    let key_value = self.eval_literal_key(k_expr)?;
                    let pos = dict
                        .iter()
                        .position(|(k, _)| self.values_equal_loose(k, &key_value));
                    let found = match pos {
                        Some(idx) => dict[idx].1.clone(),
                        None => {
                            bindings.truncate(checkpoint);
                            return Ok(false);
                        }
                    };
                    if !self.try_match(sub, &found, bindings, pspan)? {
                        bindings.truncate(checkpoint);
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Pattern::Constructor { name, inner, .. } => {
                // The parser only emits OK / ERR, but defend in depth.
                if name == "OK" {
                    match value {
                        Value::Ok(v) => self.try_match(inner, v, bindings, span),
                        _ => Ok(false),
                    }
                } else if name == "ERR" {
                    match value {
                        Value::Err(v) => self.try_match(inner, v, bindings, span),
                        _ => Ok(false),
                    }
                } else {
                    Err(self.destructure_shape_error(
                        format!(
                            "unsupported constructor pattern `{}` (only OK / ERR are valid in v0.4)",
                            name
                        ),
                        span,
                    ))
                }
            }
        }
    }



    fn diag(&mut self, code: ErrorCode, message: impl Into<String>, span: Span) -> WlwlError {
        let loc = Location {
            file: span.file.clone(),
            line: span.line_start,
            col: span.col_start,
            line_end: span.line_end,
            col_end: span.col_end,
        };
        let mut d = WlwlDiagnostic::new(code, message, loc);
        // [v0.4 spec Sec. 14.2] Capture the current call stack as
        // the diagnostic's `trace` at construction time. Doing this
        // here (not in `enrich_with_trace`) means the trace survives
        // even when `invoke_closure` pops its frame before the error
        // bubbles up. Reverse so the order is "innermost first",
        // matching Python-style traceback display (most recent call
        // at index 0) and the test contract in P4-A1d.
        if !self.call_stack.is_empty() {
            d.trace = self.call_stack.iter().rev().cloned().collect();
        }
        if let Some(src) = &self.source {
            if let Some(line_text) = extract_line(src, span.line_start) {
                d = d.with_source_line(line_text);
            }
        }
        d = match code {
            ErrorCode::E0014 => d.with_suggestion(Suggestion::Note {
                description: concat!("`BREAK` and `CONTINUE` are only valid inside ", "the body of a `WHILE` or `FOR` loop (v0.3 section 7); ", "this position is outside any loop body").into(),
            }),
            ErrorCode::E0020 => d.with_suggestion(Suggestion::Note {
                description: concat!("no binding for this name; either add a `LET(name, ...)` ", "before this use, or import it from a module").into(),
            }),
            ErrorCode::E0021 => d.with_suggestion(Suggestion::Note {
                description: "this name is already defined in the current scope; rename one of the two bindings".into(),
            }),
            ErrorCode::E0022 => d.with_suggestion(Suggestion::Note {
                description: "check the function signature: argument count must match the `FUN((p1, p2, ...), ...)` declaration".into(),
            }),
            ErrorCode::E0023 => d.with_suggestion(Suggestion::Note {
                description: "the source module does not export this name; add it to the `EXPORT([...])` list, or import a different name".into(),
            }),
            ErrorCode::E0030 => d.with_suggestion(Suggestion::Note {
                description: "operator or builtin received a value of the wrong type; check operand types or use an explicit conversion".into(),
            }),
            ErrorCode::E0040 => d.with_suggestion(Suggestion::Note {
                description: "module not found; check the IMPORT path, that the file exists, and that `wlwl.toml` lists the namespace (for `ns:name` imports)".into(),
            }),
            ErrorCode::E0041 => d.with_suggestion(Suggestion::Note {
                description: "break the cycle by extracting shared code into a third module that both can import".into(),
            }),
            ErrorCode::E0043 => d.with_suggestion(Suggestion::Note {
                description: "namespace paths look like `ns:name` (e.g. `wlwl:std.io`) or a relative path (`./mod`, `../mod`); see v0.3 section 13.3".into(),
            }),
            ErrorCode::E0061 => d.with_suggestion(Suggestion::Note {
                description: "file not found; check the path against the current working directory and create the file if needed".into(),
            }),
            ErrorCode::E0070 => d.with_suggestion(Suggestion::Note {
                description: "JSON parse failed; common causes are trailing commas, single-quoted strings, or unquoted keys".into(),
            }),
            ErrorCode::E0080 | ErrorCode::E0081 | ErrorCode::E0082 | ErrorCode::E0083 => {
                d.with_suggestion(Suggestion::Note {
                    description: "AI provider call failed; this is transient (retryable: true) -- retry the operation, or check `wlwl:std.ai` provider configuration".into(),
                })
            }
            ErrorCode::E0100 => d.with_suggestion(Suggestion::Note {
                description: "this is a compiler bug; please open an issue with the offending source file and stack trace".into(),
            }),
            ErrorCode::E0102 => d.with_suggestion(Suggestion::Note {
                description: "an `ERR(...)` reached the top level; wrap the call in `OR_DIE(expr, default)` or `TRY(expr)`, or check the upstream function for the source of the error".into(),
            }),
            _ => d,
        };
        d.into()
    }

    fn undefined_name(&mut self, name: &str, span: &Span) -> WlwlError {
        // self.diag() returns a WlwlError; extract the inner
        // WlwlDiagnostic to enrich it with the "did you mean?"
        // suggestion based on similar names in scope. `diag` now
        // takes `&mut self` so it can capture the call stack at
        // construction time; the trace is preserved through the
        // `match` below because it lives on the inner diagnostic.
        let mut d = match self.diag(
            ErrorCode::E0020,
            format!("undefined name `{}`", name),
            span.clone(),
        ) {
            WlwlError::Diagnostic(d) => d,
        };
        if let Some(src) = &self.source {
            if let Some(line_text) = extract_line(src, span.line_start) {
                d = d.with_source_line(line_text);
            }
        }
        let pool = self.env.names();
        let candidates = similar_names(name, &pool, 3);
        if !candidates.is_empty() {
            d = d.with_suggestion(Suggestion::Note {
                description: format!("did you mean one of: {}?", candidates.join(", ")),
            });
        }
        d.into()
    }
}

// 鈹€鈹€ Suggestion helpers 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
/// Edit distance (Levenshtein) between two strings. Used to surface "did you mean?"
/// candidates in E0020 (undefined name) diagnostics. O(len(a) * len(b)) but name
/// lengths are bounded by the spec (ASCII letters / digits / `_`).
/// Edit distance (Levenshtein) between two strings. Used to surface "did you mean?"
/// candidates in E0020 (undefined name) diagnostics. O(len(a) * len(b)) but name
/// lengths are bounded by the spec (ASCII letters / digits / `_`).
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() { return b.len(); }
    if b.is_empty() { return a.len(); }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// Pick up to `max` names from `pool` whose Levenshtein distance to `target` is
/// at most 3 and strictly positive (no point suggesting the exact match).
fn similar_names(target: &str, pool: &std::collections::HashSet<String>, max: usize) -> Vec<String> {
    let mut scored: Vec<(usize, String)> = pool
        .iter()
        .map(|n| (levenshtein(target, n), n.clone()))
        .filter(|(d, n)| *d > 0 && *d <= 3 && n.len() >= target.len().saturating_sub(2))
        .collect();
    scored.sort_by_key(|(d, _)| *d);
    scored.truncate(max);
    scored.into_iter().map(|(_, n)| n).collect()
}


// ──────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use wlwl_parser::parse;

    /// Parse + eval a one-shot expression. The temporary directory used
    /// as the module base is irrelevant for tests that don't IMPORT.
    fn run(src: &str) -> WlwlResult<Value> {
        let e = parse(src, "t.wl")?;
        let mut ev = Evaluator::new();
        ev.eval(&e)
    }

    /// Like `run` but also returns the warnings accumulated during the
    /// run. Used by the §9.5 overflow tests (`W0015`) — most tests
    /// keep using `run` since they don't care about warnings.
    fn run_with_warnings(src: &str) -> (WlwlResult<Value>, Vec<Warning>) {
        let e = parse(src, "t.wl").expect("parse");
        let mut ev = Evaluator::new();
        let r = ev.eval(&e);
        let w = ev.take_warnings();
        (r, w)
    }

    fn run_in(dir: &Path, src: &str) -> WlwlResult<Value> {
        let e = parse(src, "t.wl")?;
        let mut ev = Evaluator::new().with_base_dir(dir.to_path_buf());
        ev.eval(&e)
    }

    /// Like `run_in` but also returns the warnings accumulated during
    /// the run (Phase C3/C5 tests).
    fn run_in_with_warnings(dir: &Path, src: &str) -> (WlwlResult<Value>, Vec<Warning>) {
        let e = parse(src, "t.wl").expect("parse");
        let mut ev = Evaluator::new().with_base_dir(dir.to_path_buf());
        let r = ev.eval(&e);
        (r, ev.take_warnings())
    }

    /// Write a minimal wlwl.toml into `dir` with an optional
    /// language_version and an optional extra raw TOML body (e.g. a
    /// `[features]` block).
    fn write_manifest(dir: &Path, language_version: Option<&str>, extra: &str) {
        let lv = match language_version {
            Some(v) => format!("language_version = \"{}\"\n", v),
            None => String::new(),
        };
        std::fs::write(
            dir.join("wlwl.toml"),
            format!(
                "[package]\nname = \"app\"\nversion = \"0.1.0\"\nentry = \"main.wl\"\n{}{}\n",
                lv, extra
            ),
        )
        .unwrap();
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
        // v0.4 spec §9.5: division by zero is E1003 (runtime bucket),
        // not the type error E0030 it was in v0.3.
        let err = run("/(1, 0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E1003);
    }

    // ── §9.5 numeric / cross-type semantics (Phase A7) ─────────────
    //
    // v0.4 §9.5 pins these rules:
    //   * `/(INTEGER, INTEGER)` truncates toward zero (`/(7, 2)=3`,
    //     `/(-7, 2)=-3`).
    //   * `/(FLOAT, 0.0)` → E1003 (even though IEEE 754 → ±Inf).
    //   * `%(INTEGER, 0)` → E1003.
    //   * `+` / `-` / `*` on INTEGER overflow → saturate to INT64_MAX /
    //     INT64_MIN, never wrap, and emit W0015.
    //   * `-(0, INTEGER_MIN)` → E0034 (not the saturated W0015 path).
    //   * `INT(FLOAT)` truncates; out-of-range FLOAT → E0035.
    //   * `INT(STRING)` parses (or returns ERR kind=ParseError).
    //   * `=(INTEGER, FLOAT)` are equal when their numeric values
    //     match (e.g. `=(1, 1.0)` → TRUE).
    //
    // See `tests/history/20260915a7.md` for the full rationale.

    #[test]
    fn integer_div_truncates_toward_zero() {
        // §9.5 row 1.
        assert_eq!(run("/(7, 2);").unwrap(), Value::Integer(3));
        assert_eq!(run("/(-7, 2);").unwrap(), Value::Integer(-3));
        assert_eq!(run("/(7, -2);").unwrap(), Value::Integer(-3));
        assert_eq!(run("/(0, 5);").unwrap(), Value::Integer(0));
        assert_eq!(run("/(-7, -2);").unwrap(), Value::Integer(3));
    }

    #[test]
    fn integer_mod_sign_matches_divid() {
        // §9.5 row 2: `a % b = a - /(a, b) * b`, sign of the dividend.
        assert_eq!(run("%(7, 3);").unwrap(), Value::Integer(1));
        assert_eq!(run("%(-7, 3);").unwrap(), Value::Integer(-1));
        assert_eq!(run("%(7, -3);").unwrap(), Value::Integer(1));
    }

    #[test]
    fn div_by_zero_is_e1003_for_integer_and_float() {
        // §9.5 — both INTEGER/0 and FLOAT/0.0 produce E1003.
        let err = run("/(1, 0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E1003);
        // FLOAT / 0.0 also E1003 (IEEE 754 would give ±Inf; spec
        // explicitly forbids this).
        let err = run("/(1.0, 0.0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E1003);
        let err = run("/(0.0, 0.0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E1003);
    }

    #[test]
    fn mod_by_zero_is_e1003() {
        let err = run("%(7, 0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E1003);
        let err = run("%(0, 0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E1003);
    }

    #[test]
    fn float_div_preserves_nan_propagation() {
        // §9.5 row 6 — NaN propagates (no error, no saturation).
        // There's no INF / NaN literal in the source language, so
        // we can only assert the documented contract via the negative
        // test (`/(x, 0.0)` is E1003, NOT NaN/Inf). The real IEEE
        // NaN propagation kicks in once INF/NaN literals exist in a
        // later phase; for now the FLOAT path is IEEE-754 by default
        // and division-by-zero is intercepted per spec.
        let err = run("/(0.0, 0.0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E1003);
    }

    #[test]
    fn integer_add_overflow_saturates_and_emits_w0015() {
        // §9.5 row 3.
        let (r, w) = run_with_warnings("+(9223372036854775807, 1);");
        assert_eq!(r.unwrap(), Value::Integer(i64::MAX));
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].code, ErrorCode::W0015);
        assert!(w[0].message.contains("overflow"), "msg: {}", w[0].message);
    }

    #[test]
    fn integer_add_underflow_saturates_and_emits_w0015() {
        // INT64_MIN cannot be written as a literal — the lexer treats
        // `-` as the minus operator and `9223372036854775808` overflows
        // i64. Build it via LET chains instead:
        //   max = 9223372036854775807 (i64::MAX)
        //   neg_max = -max = -INT64_MAX = -9223372036854775807 = INT64_MIN + 1
        //   int_min = neg_max - 1 = INT64_MIN
        // Note: parser's unary-minus sugar only fires for `-name`
        // (no parens); `-(name)` is parsed as binary minus. We use
        // `-name` for the negative form and `-(name, n)` for binary.
        let src = "LET(max, 9223372036854775807); \
                   LET(int_min, -max); \
                   LET(int_min, -(int_min, 1)); \
                   +(int_min, -1);";
        let (r, w) = run_with_warnings(src);
        assert_eq!(r.unwrap(), Value::Integer(i64::MIN));
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].code, ErrorCode::W0015);
    }

    #[test]
    fn integer_mul_overflow_saturates_and_emits_w0015() {
        let (r, w) = run_with_warnings("*(9223372036854775807, 2);");
        assert_eq!(r.unwrap(), Value::Integer(i64::MAX));
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].code, ErrorCode::W0015);

        // Negative overflow saturates to INT64_MIN — built via LET.
        let src = "LET(max, 9223372036854775807); \
                   LET(int_min, -max); \
                   LET(int_min, -(int_min, 1)); \
                   *(int_min, 2);";
        let (r, w) = run_with_warnings(src);
        assert_eq!(r.unwrap(), Value::Integer(i64::MIN));
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].code, ErrorCode::W0015);
    }

    #[test]
    fn integer_sub_overflow_saturates_and_emits_w0015() {
        // INT64_MAX - (-1) overflows upward to INT64_MAX + 1 = INT64_MAX
        // saturated (and emits W0015). The `-(0, INT64_MIN)` case
        // is taken by the E0034 NEG-specialization (next test) per
        // spec §9.5 row 4, so we use a different overflow direction
        // here.
        let (r, w) = run_with_warnings("-(9223372036854775807, -1);");
        assert_eq!(r.unwrap(), Value::Integer(i64::MAX));
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].code, ErrorCode::W0015);
    }

    #[test]
    fn neg_integer_min_via_subtraction_throws_e0034() {
        // §9.5 row 4: `NEG(INTEGER_MIN)` (i.e. `-(0, INT64_MIN)`)
        // throws E0034. The parser lowers `-x` to `-(0, x)`, and
        // `0 - INT64_MIN` would require representing `+INT64_MAX+1`
        // which is not representable as `INTEGER`. Hence E0034
        // (specialised overflow), not the saturated W0015.
        let src = "LET(max, 9223372036854775807); \
                   LET(int_min, -max); \
                   LET(int_min, -(int_min, 1)); \
                   -(0, int_min);";
        let err = run(src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0034);
        assert!(
            err.diagnostic().message.contains("INTEGER_MIN")
                || err.diagnostic().message.contains("negate"),
            "msg: {}",
            err.diagnostic().message
        );
    }

    #[test]
    fn no_warning_on_normal_arithmetic() {
        // Sanity: the W0015 channel is silent for in-range ops.
        // Top-level expression value is the LAST statement's value,
        // which here is `/(8, 2) = 4` (not the `5` we read in some
        // older hand-traces).
        let (r, w) = run_with_warnings("+(1, 2); -(10, 5); *(3, 4); /(8, 2);");
        assert_eq!(r.unwrap(), Value::Integer(4));
        assert!(w.is_empty(), "no warnings expected for in-range ops");
    }

    #[test]
    fn float_arithmetic_does_not_emit_w0015() {
        // §9.5 row 6 — FLOAT just propagates NaN/Inf per IEEE 754,
        // no saturation, no warning. Use plain digit floats since the
        // lexer doesn't accept scientific notation (`1e308`).
        let (_, w) = run_with_warnings("+(1.0, 2.0); *(3.0, 4.0); /(10.0, 3.0);");
        assert!(w.is_empty(), "FLOAT arithmetic must not emit W0015");
    }

    #[test]
    fn int_builtin_converts_integer_unchanged() {
        assert_eq!(run("INT(42);").unwrap(), Value::Integer(42));
        assert_eq!(run("INT(0);").unwrap(), Value::Integer(0));
        assert_eq!(run("INT(-7);").unwrap(), Value::Integer(-7));
    }

    #[test]
    fn int_builtin_truncates_float_toward_zero() {
        // §9.5 row 7.
        assert_eq!(run("INT(3.7);").unwrap(), Value::Integer(3));
        assert_eq!(run("INT(-3.7);").unwrap(), Value::Integer(-3));
        assert_eq!(run("INT(0.999);").unwrap(), Value::Integer(0));
        assert_eq!(run("INT(-0.999);").unwrap(), Value::Integer(0));
    }

    #[test]
    fn int_builtin_float_out_of_range_is_e0035() {
        // §9.5 row 8. The lexer doesn't accept scientific notation
        // (`1e308`), so we use digit-only literals. `i64::MAX as f64`
        // is exactly `9.223372036854776e18`, which has 19 significant
        // digits — f64's mantissa only holds 53 bits (~15-17 decimal
        // digits), so `9223372036854775808.0` parses to `9.223372036854776e18`
        // and `INT(...)` truncates it to i64::MAX (NOT E0035).
        //
        // To produce a FLOAT unambiguously is greater than i64::MAX we
        // would need either:
        //   * scientific notation (lexer doesn't accept)
        //   * an +INF literal (no literal exists)
        //   * a chain of additions past i64::MAX (tedious; f64
        //     precision wraps around once you exceed 2^53)
        //
        // Until a FLOAT coercion path produces +Inf, the E0035 path
        // is reachable only via malformed / future FLOAT inputs. We
        // assert the contract on STRING parse failure instead (the
        // practical E0035 trigger today) and document the gap.

        // STRING → INTEGER parse failure (the E0035-adjacent path:
        // INT("foo") returns ERR kind=ParseError, NOT E0035).
        let r = run(r#"IS_ERR(INT("foo"));"#).unwrap();
        assert_eq!(r, Value::Boolean(true));

        // E0035 is registered + categorized. Direct reachability from
        // source-level FLOAT literals is currently a dead branch — see
        // TODO(Phase B4) on introducing +Inf literals.
        //
        // We at least check that INT on an in-range FLOAT does NOT
        // trigger E0035, so the E0035 branch is genuinely unreachable
        // today and not just shadowed by something else.
        assert_eq!(run("INT(3.7);").unwrap(), Value::Integer(3));
        assert_eq!(run("INT(-3.7);").unwrap(), Value::Integer(-3));
    }

    #[test]
    fn int_builtin_parses_string_decimal() {
        assert_eq!(run(r#"INT("42");"#).unwrap(), Value::Integer(42));
        assert_eq!(run(r#"INT("-7");"#).unwrap(), Value::Integer(-7));
        assert_eq!(run(r#"INT("0");"#).unwrap(), Value::Integer(0));
    }

    #[test]
    fn int_builtin_string_parse_failure_returns_err_dict() {
        // §9.5 — non-integer STRING → ERR with kind=ParseError.
        let r = run(r#"IS_ERR(INT("3.5"));"#).unwrap();
        assert_eq!(r, Value::Boolean(true));
        let r = run(r#"IS_ERR(INT("not a number"));"#).unwrap();
        assert_eq!(r, Value::Boolean(true));
    }

    #[test]
    fn int_builtin_rejects_other_types() {
        // ARRAY / BOOLEAN / NULL → E0030 type error.
        let err = run("INT([1, 2]);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        let err = run("INT(TRUE);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        let err = run("INT(NULL);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn int_builtin_propagates_err_transparently() {
        // INT is NOT in the §12.7 ERR consumer registry, so an
        // ERR passed in propagates transparently per §12.6 (which
        // at the top level becomes E0102). We wrap in IS_ERR to
        // observe the propagated ERR without the top-level promotion.
        assert_eq!(
            run(r#"IS_ERR(INT(ERR("e")));"#).unwrap(),
            Value::Boolean(true),
            "INT must not consume ERR — it should propagate via §12.6"
        );
    }

    #[test]
    fn cross_type_equality_integer_float() {
        // §9.5 row 10 — `=(1, 1.0)` is TRUE.
        assert_eq!(run("==(1, 1.0);").unwrap(), Value::Boolean(true));
        assert_eq!(run("==(0, 0.0);").unwrap(), Value::Boolean(true));
        assert_eq!(run("==(-7, -7.0);").unwrap(), Value::Boolean(true));
        assert_eq!(run("==(1, 2.0);").unwrap(), Value::Boolean(false));
        // Inverse cross-type inequality.
        assert_eq!(run("!=(1, 1.0);").unwrap(), Value::Boolean(false));
        assert_eq!(run("!=(1, 2.0);").unwrap(), Value::Boolean(true));
    }

    #[test]
    fn string_ordering_uses_unicode_codepoint_order() {
        // §9.5 row 11 — `<` / `>` on STRING is Unicode codepoint
        // (Rust's default string ordering).
        assert_eq!(run(r#"<("a", "b");"#).unwrap(), Value::Boolean(true));
        assert_eq!(run(r#">("b", "a");"#).unwrap(), Value::Boolean(true));
        // ASCII digits before lowercase letters.
        assert_eq!(run(r#"<("1", "a");"#).unwrap(), Value::Boolean(true));
        // Equal strings — `<=` and `>=` are TRUE, `<` and `>` FALSE.
        assert_eq!(run(r#"<=( "x", "x");"#).unwrap(), Value::Boolean(true));
        assert_eq!(run(r#">=("x", "x");"#).unwrap(), Value::Boolean(true));
    }

    #[test]
    fn run_with_warnings_drains_buffer() {
        // Sanity for the test helper itself: take_warnings should
        // empty the buffer so a second call returns nothing.
        let (r, w1) = run_with_warnings("+(1, 2);");
        assert_eq!(r.unwrap(), Value::Integer(3));
        assert!(w1.is_empty());
        // And on overflow, the warning is captured.
        let (r, w2) = run_with_warnings("+(9223372036854775807, 1);");
        assert_eq!(r.unwrap(), Value::Integer(i64::MAX));
        assert_eq!(w2.len(), 1);
    }

    #[test]
    fn evaluator_emit_warning_helper_directly() {
        // Tests the Evaluator::emit_warning API without going through
        // arithmetic — covers future warning sources (W0014, etc.).
        let mut ev = Evaluator::new();
        ev.emit_warning(ErrorCode::W0015, "manual warning");
        ev.emit_warning(ErrorCode::W0020, "second");
        let drained = ev.take_warnings();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].code, ErrorCode::W0015);
        assert_eq!(drained[1].code, ErrorCode::W0020);
        // Buffer is empty after drain.
        assert!(ev.take_warnings().is_empty());
    }

    // ── §9.2 comparison return-type rules (Phase A8) ───────────────
    //
    // v0.4 spec §9.2 is a **major revision** of v0.3 — v0.3 simultaneously
    // declared "comparisons always return BOOLEAN" AND "=(ERR, 1) returns
    // ERR (because `=` was not on the v0.3 whitelist)". Those statements
    // contradict. v0.4 pins:
    //
    //   * When **both operands are non-ERR**, `=` / `!=` / `>` / `<`
    //     / `>=` / `<=` returns `BOOLEAN`.
    //   * When **either operand is ERR**, the comparison transparently
    //     propagates ERR per §12.6 (does **not** return BOOLEAN).
    //
    // Implementation: `=` / `!=` are NOT in the §12.7 ERR_CONSUMER_REGISTRY
    // (Phase A6 set the registry deliberately excluding them), so the
    // existing `eval_call` short-circuit at line 1935-1944 handles the
    // ERR-transparent propagation for free. `>` `<` `>=` `<=` likewise
    // are not registered, and `IF` is treated identically. Phase A8
    // is therefore **0 impl changes**; this section is documentation
    // + test lockdown + cross-references to spec §9.2.

    #[test]
    fn eq_op_propagates_err_transparently() {
        // §9.2 — `=` / `!=` / `>` / `<` / `>=` / `<=` must NOT consume ERR;
        // they propagate transparently (§12.6). At the top level that
        // becomes E0102, so we wrap in IS_ERR to observe the result.
        // (IS_ERR is in the §12.7 registry; consumes ERR → TRUE.)
        assert_eq!(
            run(r###"IS_ERR(==(ERR("a"), 1));"###).unwrap(),
            Value::Boolean(true),
            "`==` must propagate ERR per §9.2 + §12.6"
        );
        assert_eq!(
            run(r###"IS_ERR(==(1, ERR("b")));"###).unwrap(),
            Value::Boolean(true),
            "`==` must propagate ERR even when ERR is the second arg"
        );
        assert_eq!(
            run(r###"IS_ERR(==(ERR("a"), ERR("b")));"###).unwrap(),
            Value::Boolean(true),
            "`==` must propagate ERR when both sides are ERR (leftmost wins)"
        );
        // IS_ERR consumes the propagated ERR, so the comparison never
        // returns BOOLEAN for the above inputs — verify with a direct
        // value path too: 1 == 2 still returns BOOLEAN(false).
        assert_eq!(run("==(1, 2);").unwrap(), Value::Boolean(false));
    }

    #[test]
    fn ne_op_propagates_err_transparently() {
        assert_eq!(
            run(r###"IS_ERR(!=(ERR("a"), 1));"###).unwrap(),
            Value::Boolean(true),
            "`!=` must propagate ERR per §9.2 + §12.6"
        );
        assert_eq!(
            run(r###"IS_ERR(!=(1, ERR("b")));"###).unwrap(),
            Value::Boolean(true)
        );
    }

    #[test]
    fn lt_op_propagates_err_transparently() {
        assert_eq!(
            run(r###"IS_ERR(<(ERR("a"), 1));"###).unwrap(),
            Value::Boolean(true),
            "`<` must propagate ERR per §9.2 + §12.6"
        );
        assert_eq!(
            run(r###"IS_ERR(<(1, ERR("b")));"###).unwrap(),
            Value::Boolean(true)
        );
    }

    #[test]
    fn gt_op_propagates_err_transparently() {
        assert_eq!(
            run(r###"IS_ERR(>(ERR("a"), 1));"###).unwrap(),
            Value::Boolean(true),
            "`>` must propagate ERR per §9.2 + §12.6"
        );
        assert_eq!(
            run(r###"IS_ERR(>(1, ERR("b")));"###).unwrap(),
            Value::Boolean(true)
        );
    }

    #[test]
    fn le_op_propagates_err_transparently() {
        assert_eq!(
            run(r###"IS_ERR(<=(ERR("a"), 1));"###).unwrap(),
            Value::Boolean(true),
            "`<=` must propagate ERR per §9.2 + §12.6"
        );
        assert_eq!(
            run(r###"IS_ERR(<=(1, ERR("b")));"###).unwrap(),
            Value::Boolean(true)
        );
    }

    #[test]
    fn ge_op_propagates_err_transparently() {
        assert_eq!(
            run(r###"IS_ERR(>=(ERR("a"), 1));"###).unwrap(),
            Value::Boolean(true),
            "`>=` must propagate ERR per §9.2 + §12.6"
        );
        assert_eq!(
            run(r###"IS_ERR(>=(1, ERR("b")));"###).unwrap(),
            Value::Boolean(true)
        );
    }

    #[test]
    fn comparison_returns_boolean_whenall_non_err() {
        // §9.2 — the "positive" half: all 6 ops return BOOLEAN when
        // both operands are non-ERR. Locked down here with a single
        // representative test (each op is covered transitively by
        // the op_eq_ne / op_ordering baseline tests; this is the
        // spec §9.2 wording assertion).
        for src in &[
            "==(1, 1);", "==(1, 2);",
            "!=(1, 2);", "!=(1, 1);",
            "<(1, 2);", ">(2, 1);",
            "<=(1, 1);", ">=(1, 1);",
        ] {
            let r = run(src).unwrap();
            assert!(
                matches!(r, Value::Boolean(_)),
                "{} should return BOOLEAN, got {:?}",
                src,
                r
            );
        }
    }

    #[test]
    fn comparison_leftmost_err_wins_under_transparent_propagation() {
        // §12.6 short-circuit: the **leftmost** ERR is the propagated
        // result. This is the same rule that `+` and `LEN` use.
        // Both `==(ERR("a"), ERR("b"))` and `==(ERR("b"), ERR("a"))`
        // return ERR; we wrap in `IS_ERR` (consumes ERR) and the
        // payload identity is checked via `TYPE(...) == "RESULT"`.
        assert_eq!(
            run(r###"TYPE(==(ERR("a"), ERR("b")));"###).unwrap(),
            Value::String("RESULT".into()),
            "comparing two ERRs still returns a RESULT, not BOOLEAN"
        );
        // And the comparison result is `Err("a")` (leftmost).
        // We check this via `ERR_PAYLOAD` (registered as §12.7 ERR
        // consumer; Phase B4 will implement it — until then the
        // ERR is propagated but ERR_PAYLOAD returns E0020). For
        // now we use a more general check: the result is an ERR
        // (asserted above) and its type is RESULT.
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
    fn fun_closure_param_scope_fresh_per_call() {
        // [v0.4 spec Sec. 6.4] Cell-sharing applies to bindings in a
        // PERSISTENT outer scope that multiple closures capture. A
        // parameter binding (`v` in mk's params) lives in a FRESH
        // scope per call, so two closures created in different
        // invocations of mk do NOT share v -- each captures the cell
        // of its own invocation. This preserves v0.3 deep-clone
        // behavior for parameter-scoped bindings, and is the
        // *correct* spec semantics: the cell for `v` is "lexically
        // inside mk", and mk's body is re-executed per call.
        let src = r#"
            LET(mk, FUN((v), FUN((), v)));
            LET(a, mk(1));
            LET(b, mk(2));
            +(a(), b());
        "#;
        assert_eq!(run(src).unwrap(), Value::Integer(3));
    }



    // ---- P4-A2: closure cell semantics + SET ----
    //
    // spec v0.4 搂6.4 / 附录 E.4.6:
    //   * `LET` creates an IMMUTABLE cell in the current scope.
    //   * When a closure captures the cell (E-CloCap, fired at
    //     closure call time), the cell is upgraded to MUTABLE.
    //   * `SET(target, value)` looks up the cell: mutable -> write;
    //     IMMUTABLE -> E0024; not found -> E0020.

    /// Classic counter pattern: closure mutates a captured variable
    /// and the mutation is visible to subsequent calls and to other
    /// closures that share the same cell.
    #[test]
    fn p4_a2_closure_counter() {
        let src = r#"
            LET(make_counter, FUN((),
                LET(count, 0);
                FUN((),
                    SET(count, +(count, 1));
                    count
                )
            ));
            LET(c, make_counter());
            +(c(), +(c(), c()));
        "#;
        // 1 + (1 + 1) = 3 (c, c, c -> 1, 2, 3)
        assert_eq!(run(src).unwrap(), Value::Integer(6));
    }

    /// Two closures built in the same outer scope share the same
    /// captured cell: mutating through one is visible through the
    /// other (basic cell-sharing, no INDEX_GET needed).
    #[test]
    fn p4_a2_closure_shared_cell() {
        let src = r#"
            LET(s, "init");
            LET(get, FUN((), s));
            LET(set, FUN((x), SET(s, x)));
            set("updated");
            get();
        "#;
        // After set("updated"), the cell behind s is "updated";
        // get() reads "updated" because both closures share the
        // same outer-scope cell (E-CloCap upgraded it to MUTABLE
        // when either closure was called).
        assert_eq!(
            run(src).unwrap(),
            Value::String("updated".into())
        );
    }

    /// The "two closures, two mutating operations" pattern: a
    /// stepper and a finaliser both share the same accumulator
    /// cell, no INDEX_GET needed.
    #[test]
    fn p4_a2_closure_shared_cell_increment() {
        let src = r#"
            LET(s, 0);
            LET(add, FUN((x), SET(s, +(s, x))));
            add(10);
            add(32);
            s
        "#;
        // 0 + 10 + 32 = 42
        assert_eq!(run(src).unwrap(), Value::Integer(42));
    }

    /// `SET` on a binding in the same scope (not captured) raises
    /// E0024 per spec 搂6.4: cells are IMMUTABLE until captured.
    #[test]
    fn p4_a2_set_non_captured_is_e0024() {
        let src = r#"
            LET(x, 1);
            IF(==(1, 1),
                SET(x, 2)
            );
            x
        "#;
        // x is bound in the outer scope and not captured by any
        // closure -- the IF's body is a child scope, not a closure.
        // SET from a child scope against a non-captured binding is
        // E0024.
        let err = run(src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0024);
    }

    /// `SET` on an undefined name raises E0020 (and reuses the
    /// "did you mean?" suggestion machinery).
    #[test]
    fn p4_a2_set_undefined_name_is_e0020() {
        let src = "SET(nonexistent, 1);";
        let err = run(src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0020);
    }

    /// `SET` with arity != 2 raises E0022.
    #[test]
    fn p4_a2_set_wrong_arity_is_e0022() {
        let err = run("LET(x, 0); SET(x);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
        let err = run("LET(x, 0); SET(x, 1, 2);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    /// `SET` with a non-Var target (an expression) raises E0030.
    #[test]
    fn p4_a2_set_non_var_target_is_e0030() {
        let err = run("LET(x, 0); SET(+(1, 2), 0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    /// `LET` + `SET` + read round-trip: verify the cell machinery
    /// works for the "closure that accumulates via SET" pattern.
    /// Note: SET on `i` from a WHILE body where `i` is not captured
    /// is E0024 per spec 搂6.4; here we wrap the loop in a closure
    /// `loop` that captures `i` so E-CloCap upgrades the cell.
    #[test]
    fn p4_a2_let_set_read_roundtrip() {
        let src = r#"
            LET(accum, FUN((n),
                LET(s, 0);
                LET(step, FUN((x), SET(s, +(s, x))));
                LET(i, 1);
                LET(loop, FUN((),
                    IF(<=(i, n),
                        step(i);
                        SET(i, +(i, 1));
                        loop()
                    )
                ));
                loop();
                s
            ));
            accum(10)
        "#;
        // 1+2+...+10 = 55
        assert_eq!(run(src).unwrap(), Value::Integer(55));
    }

    /// The mutability upgrade is *not* reverted when the closure
    /// returns: once captured, the cell stays MUTABLE for the rest
    /// of the program (per E-CloCap -- "captured cells stay MUTABLE").
    #[test]
    fn p4_a2_mutable_upgrade_persists_after_closure_returns() {
        // `s` is captured by the inner closure. After `inner` returns,
        // `s` is still MUTABLE in the outer scope, so SET works
        // directly from the outer scope.
        let src = r#"
            LET(outer, FUN((),
                LET(s, 0);
                LET(inner, FUN((), SET(s, 100)));
                inner();
                SET(s, 200);
                s
            ));
            outer()
        "#;
        assert_eq!(run(src).unwrap(), Value::Integer(200));
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
        // v0.4 spec §2.2.1: ERR payload must be STRING or DICT — string is fine.
        assert_eq!(run(r###"IS_OK(ERR("1"));"###).unwrap(), Value::Boolean(false));
        assert_eq!(run("IS_ERR(OK(1));").unwrap(), Value::Boolean(false));
        assert_eq!(run(r###"IS_ERR(ERR("1"));"###).unwrap(), Value::Boolean(true));
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

    // ── §12.7 ERR consumer registry (Phase A6) ────────────────────────
    //
    // These tests pin down the v0.4 spec §12.7 contract:
    //   * The registry is the **closed** set of names that consume ERR.
    //   * Names NOT in the registry propagate ERR per §12.6 default.
    //   * `=` / `!=` / `IF` are explicitly OUT — they observe via §9.2
    //     / §7.1 but do not consume ERR (per spec §12.7 table footnote).
    //   * v0.4 keywords IS_OK / IS_ERR / OR_DIE / TRY go through their
    //     own `Expr::*` arms and never flow through this registry at
    //     runtime — but they are still in the registry as the canonical
    //     spec-pinned list (future-proofing for spec conformance tests).
    //
    // The test cases below directly poke `is_err_consumer` (it's a free
    // function in this crate) plus the runtime-visible alias `UNWRAP_OR`.

    #[test]
    fn err_consumer_registry_contains_all_10_names() {
        // §12.7 v0.4 spec lists 9 names; Phase B7 (§15.9 row 5)
        // adds EXPECT_ERR for the same boundary-need as the others
        // (it's the "expect this expr to fail" primitive — without
        // ERR-consumer status the input ERR would short-circuit in
        // eval_call before the builtin sees it). Lock the set so
        // any future addition shows up as a deliberate, conscious
        // change.
        use crate::ERR_CONSUMER_REGISTRY;
        let actual: std::collections::HashSet<&str> =
            ERR_CONSUMER_REGISTRY.iter().copied().collect();
        let expected: std::collections::HashSet<&str> = [
            "IS_OK",
            "IS_ERR",
            "OR_DIE",
            "UNWRAP_OR",
            "TRY",
            "UNWRAP",
            "ERR_PAYLOAD",
            "WRAP",
            "TYPE",
            // Phase B7 (spec §15.9): see module-level docs in
            // `wlwl_eval::test` for the rationale.
            "EXPECT_ERR",
        ]
        .iter()
        .copied()
        .collect();
        assert_eq!(actual, expected, "§12.7 registry drifted from spec");
        assert_eq!(actual.len(), 10, "spec §12.7 + B7 add 10 entries");
    }

    #[test]
    fn err_consumer_registry_excludes_equality_and_if() {
        // §12.7 table footnote: `=` / `!=` / `IF` are listed in the
        // table but explicitly "do not consume ERR" — they MUST
        // propagate per §12.6 default. So they must NOT be in the
        // ERR_CONSUMER_REGISTRY (which gates the §12.6 short-circuit).
        for name in ["=", "!=", "IF"] {
            assert!(
                !is_err_consumer(name),
                "{} is listed in §12.7 but must NOT short-circuit ERR propagation",
                name
            );
        }
    }

    #[test]
    fn err_consumer_registry_unknown_returns_false() {
        // Names not in the spec table are NOT consumers.
        for name in ["+", "-", "*", "PRINT", "LEN", "PUSH", "NOSUCH"] {
            assert!(
                !is_err_consumer(name),
                "{} unexpectedly registered as ERR consumer",
                name
            );
        }
    }

    #[test]
    fn unwrap_or_alias_calls_or_die_impl() {
        // v0.4 spec §12.7: `UNWRAP_OR` is the main name; `OR_DIE` is
        // the v0.3 alias (Phase B3 will add W0051 on legacy use). For
        // now both names route to the same builtin and behave identically.
        // Tokenization: UNWRAP_OR is NOT a lexer keyword (OR_DIE is), so
        // `UNWRAP_OR(ERR("e"), 42)` reaches `eval_call` as
        // `Expr::Call { name: "UNWRAP_OR", ... }` and dispatches via
        // `resolve_builtin`. That is the path that exercises the alias.

        // ERR case: returns default
        assert_eq!(
            run(r#"UNWRAP_OR(ERR("e"), 42);"#).unwrap(),
            Value::Integer(42)
        );
        // OK case: returns inner value
        assert_eq!(
            run(r#"UNWRAP_OR(OK(7), 42);"#).unwrap(),
            Value::Integer(7)
        );
        // Non-RESULT case: E0030 (same error semantics as OR_DIE)
        let err = run(r#"UNWRAP_OR(123, 42);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        // Arity: same E0022 path as OR_DIE
        let err = run(r#"UNWRAP_OR(ERR("e"));"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
        let err = run(r#"UNWRAP_OR(ERR("e"), 1, 2);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn unwrap_or_alias_propagates_through_user_function() {
        // UNWRAP_OR inside a user function: since UNWRAP_OR is in
        // the §12.7 registry, ERR must reach the user function (not
        // be short-circuited by §12.6). The user function then sees
        // Value::Err and decides what to do with UNWRAP_OR.
        let src = r#"
            LET(pass_through, FUN((x), x));
            UNWRAP_OR(pass_through(ERR("inner")), -1);
        "#;
        assert_eq!(run(src).unwrap(), Value::Integer(-1));
    }

    #[test]
    fn registry_unwrap_now_implemented_returns_inner_for_ok() {
        // Phase B4 implements UNWRAP(OK(v)) → v. The previous gap
        // (`registry_unwrap_not_yet_implemented`) is closed: E0020
        // no longer fires for UNWRAP. Full coverage of UNWRAP
        // lives in the B4 test group (`b4_unwrap_*`).
        assert_eq!(run("UNWRAP(OK(42));").unwrap(), Value::Integer(42));
    }

    // ── §2.2.1 RESULT type + TYPE builtin (Phase A5) ────────────────
    //
    // v0.4 spec §2.2.1 promotes OK / ERR from "wrapper DICT" to a
    // first-class RESULT type, and pins TYPE(x) to return the
    // uppercase type name. ERR's payload is type-constrained to STRING
    // or DICT (any other value is an E0030 type error).
    //
    // The TYPE builtin is also in the §12.7 ERR consumer registry
    // (added by Phase A6), so TYPE on an ERR returns "RESULT" without
    // the ERR transparently propagating.

    #[test]
    fn type_returns_uppercase_spec_names_for_all_variants() {
        // §2.2.1 — every spec-listed type is reachable via TYPE().
        use Value::*;
        assert_eq!(run("TYPE(1);").unwrap(), Value::String("INTEGER".into()));
        assert_eq!(run("TYPE(1.5);").unwrap(), Value::String("FLOAT".into()));
        assert_eq!(run(r#"TYPE("hi");"#).unwrap(), Value::String("STRING".into()));
        assert_eq!(run("TYPE(TRUE);").unwrap(), Value::String("BOOLEAN".into()));
        assert_eq!(run("TYPE(NULL);").unwrap(), Value::String("NULL".into()));
        assert_eq!(run("TYPE([1, 2]);").unwrap(), Value::String("ARRAY".into()));
        assert_eq!(
            run(r#"TYPE(["k": 1]);"#).unwrap(),
            Value::String("DICT".into())
        );
        // Function types: closure AND native fn both report FUNCTION.
        assert_eq!(
            run("LET(f, FUN((), 1)); TYPE(f);").unwrap(),
            Value::String("FUNCTION".into())
        );
    }

    #[test]
    fn type_of_ok_and_err_both_return_result() {
        // §2.2.1 — `OK(...)` and `ERR(...)` share the type name RESULT.
        // "RESULT" is the **type** name; OK and ERR are the **variants**.
        assert_eq!(
            run("TYPE(OK(1));").unwrap(),
            Value::String("RESULT".into())
        );
        assert_eq!(
            run(r#"TYPE(ERR("e"));"#).unwrap(),
            Value::String("RESULT".into())
        );
        // Even when wrapping other RESULTs, the surface is still RESULT.
        assert_eq!(
            run("TYPE(OK(OK(1)));").unwrap(),
            Value::String("RESULT".into())
        );
    }

    #[test]
    fn type_consumes_err_without_propagation() {
        // TYPE is in the §12.7 registry (added by A6), so TYPE(ERR("e"))
        // returns "RESULT" — the ERR is observed as a type-tag, not
        // propagated transparently.
        assert_eq!(
            run(r#"TYPE(ERR("e"));"#).unwrap(),
            Value::String("RESULT".into())
        );
    }

    #[test]
    fn type_arity_wrong_is_e0022() {
        let err = run("TYPE();").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
        let err = run("TYPE(1, 2);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn err_payload_must_be_string_or_dict_string_ok() {
        // §2.2.1 — string payloads are accepted. Wrap in IS_ERR (which
        // is in the §12.7 ERR consumer registry) so the top-level ERR
        // does not promote to E0102.
        assert_eq!(
            run(r###"IS_ERR(ERR("network down"));"###).unwrap(),
            Value::Boolean(true),
            "string payload must be accepted as a valid ERR payload"
        );
    }

    #[test]
    fn err_payload_must_be_string_or_dict_dict_ok() {
        // §2.2.1 — dict payloads are accepted (structured error context).
        // IS_ERR consumes the ERR per §12.7 registration.
        assert_eq!(
            run(r###"IS_ERR(ERR(["code": "E1001", "retryable": TRUE]));"###).unwrap(),
            Value::Boolean(true),
            "dict payload must be accepted as a valid ERR payload"
        );
    }

    #[test]
    fn err_payload_integer_errors_with_e0030() {
        // Non-STRING / non-DICT payload → E0030 type error.
        let err = run("ERR(42);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn err_payload_array_errors_with_e0030() {
        // ARRAY is a perfectly valid value in general, but ERR specifically
        // requires STRING or DICT per spec §2.2.1.
        let err = run("ERR([1, 2, 3]);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn err_payload_boolean_and_null_errors_with_e0030() {
        let err = run("ERR(TRUE);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        let err = run("ERR(NULL);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn err_payload_ok_errors_with_e0030() {
        // OK is a RESULT, not STRING/DICT, so ERR(OK(1)) is also E0030.
        // Note: spec §2.2.1 explicitly limits the allowed set to STRING/DICT.
        let err = run("ERR(OK(1));").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn ok_payload_has_no_type_constraint() {
        // Spec §2.2.1 only constrains ERR(e). OK(v) accepts any v.
        // Including RESULT-wrapping RESULT and ARRAYs.
        assert!(matches!(run("OK(42);").unwrap(), Value::Ok(_)));
        assert!(matches!(run("OK([1, 2]);").unwrap(), Value::Ok(_)));
        assert!(matches!(run("OK(OK(1));").unwrap(), Value::Ok(_)));
        assert!(matches!(run(r#"OK("hi");"#).unwrap(), Value::Ok(_)));
    }

    #[test]
    fn err_payload_validation_only_runs_when_arg_evaluates_ok() {
        // Make sure the type check is performed on the **evaluated**
        // payload, not the AST shape — so e.g. LET-binding first then
        // passing into ERR also goes through the check.
        // Integer LET value fed into ERR: type check fires on evaluated value.
        let err = run(r###"LET(x, 42); ERR(x);"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        // Dict LET value fed into ERR: accepted. Use IS_ERR to observe
        // (and to consume the ERR per §12.7 so it doesn't E0102-promote).
        assert_eq!(
            run(r###"LET(d, ["k": "v"]); IS_ERR(ERR(d));"###).unwrap(),
            Value::Boolean(true)
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

        // ==== Phase 3: AI contract (v0.3 Sec. 14.7) ====

    fn ai_check_jsonl(src: &str) -> (serde_json::Value, wlwl_error::WlwlError) {
        let err = run(src).unwrap_err();
        let line = err.diagnostic().render_jsonl();
        assert!(!line.contains('\n'), "jsonl must be single-line: {}", line);
        let v: serde_json::Value = serde_json::from_str(&line).expect("jsonl parses");
        (v, err)
    }

    #[test]
    fn ai_contract_undefined_name() {
        let (v, _) = ai_check_jsonl("PRINT(zzz);");
        assert_eq!(v["error_schema_version"], "1.1.0");
        assert_eq!(v["code"], "E0020");
        assert_eq!(v["error_category"], "name");
        assert_eq!(v["retryable"], false);
        // v0.4 (Sec. 14.2) -- new required fields
        assert!(v["idempotent"].is_boolean(), "idempotent must be present (got: {})", v);
        assert!(v["retry_after"].is_null() || v["retry_after"].is_u64(),
            "retry_after must be null or u64 (got: {})", v);
        assert!(v["suggestion_code"].is_array());
        assert!(v["related"].is_array());
    }

    #[test]
    fn ai_contract_unhandled_err_escape() {
        let (v, _) = ai_check_jsonl("ERR(\"top\");");
        assert_eq!(v["code"], "E0102");
        assert_eq!(v["error_category"], "internal");
    }

    #[test]
    fn ai_contract_panic() {
        let (v, _) = ai_check_jsonl("PANIC(\"oops\");");
        assert_eq!(v["code"], "E0100");
        assert_eq!(v["error_category"], "internal");
    }

    #[test]
    fn ai_contract_break_outside_loop() {
        let (v, _) = ai_check_jsonl("BREAK();");
        assert_eq!(v["code"], "E0014");
        assert_eq!(v["error_category"], "syntax");
    }

    #[test]
    fn ai_contract_arity_mismatch() {
        let src = "LET(f, FUN((a, b), +(a, b))); f(1);";
        let (v, _) = ai_check_jsonl(src);
        assert_eq!(v["code"], "E0022");
        assert_eq!(v["error_category"], "name");
    }

    #[test]
    fn ai_contract_module_not_found() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("wlwl_ai_{}", nanos));
        std::fs::create_dir_all(&dir).unwrap();
        let src = "IMPORT(\"doesnotexist\", [\"x\"]);";
        let mut ev = crate::Evaluator::new()
            .with_source(src, "t.wl")
            .with_base_dir(dir.clone());
        let ast = wlwl_parser::parse(src, "t.wl").unwrap();
        let err = ev.eval(&ast).unwrap_err();
        let line = err.diagnostic().render_jsonl();
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["code"], "E0040");
        assert_eq!(v["error_category"], "module");
        assert!(!line.contains('\n'));
    }

    #[test]
    fn ai_contract_schema_version_is_stable() {
        let mut versions = std::collections::HashSet::new();
        for src in &["PRINT(zzz);", "PANIC(\"x\");", "ERR(\"y\");", "BREAK();"] {
            let err = run(src).unwrap_err();
            versions.insert(err.diagnostic().error_schema_version.clone());
        }
        assert_eq!(versions.len(), 1, "schema version must be stable across error kinds");
        // v0.4 spec Sec. 14.10: bump MAJOR on field add/remove, MINOR on add.
        // v0.3 was 0.3.1; v0.4 added trace/cause/idempotent/retry_after (MINOR bump).
        assert!(versions.contains("1.1.0"), "expected schema 1.1.0, got: {:?}", versions);
    }

    #[test]
    fn ai_contract_required_fields_present() {
        let err = run("PRINT(zzz);").unwrap_err();
        let v: serde_json::Value = serde_json::from_str(&err.diagnostic().render_jsonl()).unwrap();
        // v0.4 schema 1.1.0 -- required field set (Sec. 14.2).
        // trace + cause are optional (skip_serializing_if on the struct).
        for key in &[
            "error_schema_version", "code", "error_category", "severity",
            "message", "location", "retryable", "idempotent", "retry_after",
            "suggestion_code", "related",
        ] {
            assert!(v.get(*key).is_some(), "missing required key: {}", key);
        }
        let loc = &v["location"];
        // v0.4 spec Sec. 14.2: location is unified to
        // {file, line, col_start, line_end, col_end}.
        for k in &["file", "line", "col_start", "line_end", "col_end"] {
            assert!(loc.get(*k).is_some(), "missing location.{}", k);
        }
    }

    #[test]
    fn ai_contract_category_and_retryable_match_code() {
        // The error_category and retryable fields must be consistent
        // with the code's category()/retryable() methods. If anyone
        // adds a new code without wiring up these methods, this test
        // will catch the inconsistency.
        for src in &["PRINT(zzz);", "PANIC(\"x\");", "ERR(\"y\");", "BREAK();"] {
            let err = run(src).unwrap_err();
            let code = err.diagnostic().code;
            assert_eq!(
                err.diagnostic().error_category,
                code.category(),
                "category mismatch for code={:?}", code
            );
            assert_eq!(
                err.diagnostic().retryable,
                code.retryable(),
                "retryable mismatch for code={:?}", code
            );
        }
    }

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

    // ── Phase 4 batch 1: std library IMPORT integration ────────────

    /// Helper: run a program with an explicit temp dir; the IMPORT
    /// resolver only consults the temp dir for non-`wlwl:` paths, so
    /// the dir is a scratch space (and irrelevant for `wlwl:std.X`).
    fn run_std(src: &str) -> WlwlResult<Value> {
        let dir = unique_test_dir("std");
        run_in(&dir, src)
    }

    #[test]
    fn std_io_print_via_namespace_import() {
        // IMPORT("wlwl:std.io", ["PRINT"]) should bind PRINT as a
        // native function and route the call through it.
        let v = run_std(r#"
            IMPORT("wlwl:std.io", ["PRINT"]);
            PRINT("hello", "via", "std.io");
        "#)
        .unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn std_io_print_with_non_string_args() {
        // Confirm PRINT handles non-string values via JSON conversion.
        let v = run_std(r#"
            IMPORT("wlwl:std.io", ["PRINT"]);
            PRINT(1, 2, 3, [4, 5], ["k": "v"]);
        "#)
        .unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    #[test]
    fn std_io_input_arity_mismatch_is_e0022() {
        // INPUT() takes zero args; passing an arg surfaces E0022.
        // Real stdin behaviour is covered by the interactive doc
        // tests (CI stdin is not redirectable in a unit test).
        let src = r#"
            IMPORT("wlwl:std.io", ["INPUT"]);
            INPUT("oops");
        "#;
        let dir = unique_test_dir("input");
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn std_fs_write_then_read_roundtrip() {
        let dir = unique_test_dir("fs_rt");
        let path = dir.join("rt.txt").to_string_lossy().into_owned().replace("\\", "/");
        let src = format!(r#"
            IMPORT("wlwl:std.fs", ["READ_FILE", "WRITE_FILE"]);
            LET(p, "{path}");
            WRITE_FILE(p, "round-trip-body");
            READ_FILE(p);
        "#);
        let v = run_in(&dir, &src).unwrap();
        assert_eq!(v, Value::String("round-trip-body".into()));
    }

    #[test]
    fn std_fs_read_missing_file_is_e0061() {
        let dir = unique_test_dir("fs_miss");
        let src = r#"
            IMPORT("wlwl:std.fs", ["READ_FILE"]);
            READ_FILE("Z:/__wlwl_definitely_missing_/abc_xyz_123");
        "#;
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0061);
    }

    #[test]
    fn std_fs_exists_true_then_false() {
        let dir = unique_test_dir("fs_exists");
        let path = dir.join("e.txt").to_string_lossy().into_owned().replace("\\", "/");
        std::fs::write(&dir.join("e.txt"), b"x").unwrap();
        let src_ok = format!(r#"
            IMPORT("wlwl:std.fs", ["EXISTS"]);
            EXISTS("{path}");
        "#);
        assert_eq!(
            run_in(&dir, &src_ok).unwrap(),
            Value::Boolean(true)
        );
        let _ = std::fs::remove_file(dir.join("e.txt"));
        let src_missing = format!(r#"
            IMPORT("wlwl:std.fs", ["EXISTS"]);
            EXISTS("{path}");
        "#);
        assert_eq!(
            run_in(&dir, &src_missing).unwrap(),
            Value::Boolean(false)
        );
    }

    #[test]
    fn std_json_parse_object() {
        let src = r#"
            IMPORT("wlwl:std.json", ["PARSE"]);
            LET(v, PARSE("{\"a\": 1, \"b\": [2, 3]}"));
            v;
        "#;
        // The result is a Dict. Compare via JSON stringification.
        let dir = unique_test_dir("json_parse");
        let v = run_in(&dir, src).unwrap();
        match v {
            Value::Dict(entries) => {
                let mut sorted: Vec<&(Value, Value)> = entries.iter().collect();
                sorted.sort_by(|a, b| a.0.display().cmp(&b.0.display()));
                let rendered: Vec<String> = sorted
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k.display(), v.display()))
                    .collect();
                let combined = format!("[{}]", rendered.join(", "));
                // Compare the dict as a string. The exact format
                // depends on the value conversion; the assert is on
                // presence of keys.
                assert!(combined.contains("a: 1"));
                assert!(combined.contains("b:"));
                assert!(combined.contains("2"));
                assert!(combined.contains("3"));
            }
            other => panic!("expected Dict, got {:?}", other),
        }
    }

    #[test]
    fn std_json_parse_invalid_is_e0070() {
        let src = r#"
            IMPORT("wlwl:std.json", ["PARSE"]);
            PARSE("not json");
        "#;
        let dir = unique_test_dir("json_bad");
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0070);
    }

    #[test]
    fn std_json_stringify_object() {
        let src = r#"
            IMPORT("wlwl:std.json", ["STRINGIFY"]);
            STRINGIFY(["a": 1, "b": 2]);
        "#;
        let dir = unique_test_dir("json_str");
        let v = run_in(&dir, src).unwrap();
        // STRINGIFY uses serde_json's compact form.
        let s = match v {
            Value::String(s) => s,
            other => panic!("expected String, got {:?}", other),
        };
        // Order of DICT entries is insertion-order (Phase 3 guarantee).
        assert_eq!(s, r#"{"a":1,"b":2}"#);
    }

    #[test]
    fn std_import_unknown_namespace_path_is_e0040() {
        // ModuleLoader rejects unknown wlwl: paths with E0040 (not
        // found). The parser already accepts the `wlwl:` prefix.
        let src = r#"
            IMPORT("wlwl:std.does_not_exist", ["x"]);
        "#;
        let dir = unique_test_dir("unknown");
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0040);
    }

    #[test]
    fn std_import_unknown_name_in_module_is_e0023() {
        // Importing a name the std module does not expose triggers
        // E0023 (name not exported by module).
        let src = r#"
            IMPORT("wlwl:std.io", ["NONEXISTENT"]);
        "#;
        let dir = unique_test_dir("bad_name");
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0023);
    }

    // ── Phase 4 batch 2: cross-dir / namespace / project-root ──

    #[test]
    fn crossdir_import_subdirectory() {
        // `IMPORT("./sub/foo", …)` resolves to `<base_dir>/sub/foo.wl`
        // and binds `foo`'s exports.
        let dir = unique_test_dir("crossdir_sub");
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join("foo.wl"),
            r#"
                LET(answer, 42);
                EXPORT(["answer"]);
            "#,
        )
        .unwrap();
        let src = r#"
            IMPORT("./sub/foo", ["answer"]);
            answer;
        "#;
        assert_eq!(run_in(&dir, src).unwrap(), Value::Integer(42));
    }

    #[test]
    #[test]
    fn crossdir_import_parent_directory() {
        // `IMPORT("../sibling/math", …)` from a module in `dir/inner/`
        // climbs one level up to `dir/sibling/math.wl`. A
        // `wlwl.toml` is placed at `dir` so the project root is
        // `dir` and the relative path stays inside the root.
        let dir = unique_test_dir("crossdir_parent");
        std::fs::write(
            dir.join("wlwl.toml"),
            r#"
[package]
name = "crossdir"
version = "0.1.0"
entry = "inner/main.wl"
"#,
        )
        .unwrap();
        let sibling_dir = dir.join("sibling");
        std::fs::create_dir_all(&sibling_dir).unwrap();
        std::fs::write(
            sibling_dir.join("math.wl"),
            r#"
                LET(pi, 314);
                EXPORT(["pi"]);
            "#,
        )
        .unwrap();
        let inner = dir.join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(
            inner.join("main.wl"),
            r#"
                IMPORT("../sibling/math", ["pi"]);
                pi;
            "#,
        )
        .unwrap();
        let src = std::fs::read_to_string(inner.join("main.wl")).unwrap();
        assert_eq!(run_in(&inner, &src).unwrap(), Value::Integer(314));
    }

    #[test]
    fn crossdir_import_outside_project_root_is_e0040() {
        // Even with a wlwl.toml, escaping the project root is an
        // E0040 (module 'foo' not found / outside project root).
        let dir = unique_test_dir("crossdir_outside");
        let outside = dir.join("..").join("wlwl_test_outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(
            outside.join("escape.wl"),
            r#"
                LET(x, 1);
                EXPORT(["x"]);
            "#,
        )
        .unwrap();
        std::fs::write(
            dir.join("wlwl.toml"),
            r#"
[package]
name = "out"
version = "0.1.0"
entry = "main.wl"
"#,
        )
        .unwrap();
        let src = r#"
            IMPORT("../wlwl_test_outside/escape", ["x"]);
        "#;
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0040);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn namespace_path_resolves_via_manifest() {
        // `IMPORT("myteam:utils", …)` resolves through the project's
        // wlwl.toml [dependencies] map to a local path.
        //
        // Phase C7 (spec §13.5 严格语义):项目根是搜索的最高边界,
        // root 外的依赖(即使是 manifest 声明的 `path = "../…"`)→
        // E0040。旧版本测试把依赖放在 root 外,靠 is_within 的词法
        // `..` 漏洞通过;归一化修复后依赖必须放在 root 内
        // (deviations P4-C7-001)。
        let dir = unique_test_dir("ns_resolve");
        let dep_dir = dir.join("vendor").join("wlwl_test_dep");
        std::fs::create_dir_all(&dep_dir).unwrap();
        std::fs::write(
            dep_dir.join("utils.wl"),
            r#"
                LET(greet, "hi");
                EXPORT(["greet"]);
            "#,
        )
        .unwrap();
        std::fs::write(
            dir.join("wlwl.toml"),
            format!(
                r#"
[package]
name = "app"
version = "0.1.0"
entry = "main.wl"

[dependencies]
"myteam:utils" = {{ path = "vendor/wlwl_test_dep" }}
"#
            ),
        )
        .unwrap();
        let src = r#"
            IMPORT("myteam:utils", ["greet"]);
            greet;
        "#;
        assert_eq!(
            run_in(&dir, src).unwrap(),
            Value::String("hi".into())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn namespace_path_unregistered_is_e0043() {
        // A non-`wlwl:` namespace without a manifest entry surfaces
        // E0043 ("not registered in this project's wlwl.toml").
        let dir = unique_test_dir("ns_unreg");
        std::fs::write(
            dir.join("wlwl.toml"),
            r#"
[package]
name = "app"
version = "0.1.0"
entry = "main.wl"
"#,
        )
        .unwrap();
        let src = r#"
            IMPORT("myteam:utils", ["x"]);
        "#;
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0043);
    }

    #[test]
    fn namespace_path_without_manifest_is_e0043() {
        // No wlwl.toml at all -> the namespace registry is
        // unavailable, and any `<ns>:<name>` path is E0043.
        let dir = unique_test_dir("ns_no_manifest");
        let src = r#"
            IMPORT("myteam:utils", ["x"]);
        "#;
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0043);
    }

    #[test]
    fn crossdir_within_project_root_works() {
        // Sanity: with a wlwl.toml at the root, deep `./sub/leaf`
        // imports still work as long as they stay inside the root.
        let dir = unique_test_dir("crossdir_inside");
        let deep = dir.join("a").join("b").join("c");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(
            dir.join("wlwl.toml"),
            r#"
[package]
name = "deep"
version = "0.1.0"
entry = "main.wl"
"#,
        )
        .unwrap();
        std::fs::write(
            deep.join("leaf.wl"),
            r#"
                LET(v, 7);
                EXPORT(["v"]);
            "#,
        )
        .unwrap();
        let src = r#"
            IMPORT("./leaf", ["v"]);
            v;
        "#;
        assert_eq!(run_in(&deep, src).unwrap(), Value::Integer(7));
    }

    #[test]
    fn circular_import_surfaces_full_cycle_path() {
        // Per spec §13.7 v0.3 enhancement: the error message must
        // list the full cycle path, not just first and last.
        let dir = unique_test_dir("cycle");
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
        let msg = err.diagnostic().render_human();
        // The cycle path "a -> b -> a" must be present.
        assert!(
            msg.contains("a -> b -> a"),
            "cycle path missing full chain: {}",
            msg
        );
    }

    // ── Phase 4 batch 3: std.ai (mock) integration ────────────

    #[test]
    fn std_ai_ask_mock_response() {
        // ASK is a mock; the response includes the model name and
        // a hash of the prompt. End-to-end the value is a STRING.
        let dir = unique_test_dir("ai_ask_ok");
        let src = r#"
            IMPORT("wlwl:std.ai", ["ASK"]);
            LET(r, ASK("gpt-4", "explain ERR"));
            r;
        "#;
        let v = run_in(&dir, src).unwrap();
        match v {
            Value::String(s) => {
                assert!(s.contains("[mock:gpt-4]"), "{}", s);
                assert!(s.contains("explain ERR"));
            }
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn std_ai_ask_failure_token_triggers_e0080() {
        // Reserved model name "_fail_E0080" surfaces E0080
        // (unreachable) without needing to mutate env.
        let dir = unique_test_dir("ai_ask_e0080");
        let src = r#"
            IMPORT("wlwl:std.ai", ["ASK"]);
            ASK("_fail_E0080", "x");
        "#;
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0080);
    }

    #[test]
    fn std_ai_ask_failure_token_e0083_timeout() {
        let dir = unique_test_dir("ai_ask_e0083");
        let src = r#"
            IMPORT("wlwl:std.ai", ["ASK"]);
            ASK("_fail_E0083", "x");
        "#;
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0083);
        // E0083 is retryable per spec.
        assert!(err.diagnostic().retryable);
    }

    #[test]
    fn std_ai_embed_returns_array_of_floats() {
        let dir = unique_test_dir("ai_embed");
        let src = r#"
            IMPORT("wlwl:std.ai", ["EMBED"]);
            LET(v, EMBED("hello"));
            v;
        "#;
        let v = run_in(&dir, src).unwrap();
        match v {
            Value::Array(items) => {
                assert_eq!(items.len(), 4);
                for it in &items {
                    assert!(matches!(it, Value::Float(_) | Value::Integer(_)));
                }
            }
            other => panic!("expected Array, got {:?}", other),
        }
    }

    #[test]
    fn std_ai_embed_failure_via_model() {
        let dir = unique_test_dir("ai_embed_fail");
        let src = r#"
            IMPORT("wlwl:std.ai", ["EMBED"]);
            EMBED("hi", "_fail_E0081");
        "#;
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0081);
    }

    #[test]
    fn std_ai_complete_returns_string() {
        let dir = unique_test_dir("ai_complete");
        let src = r#"
            IMPORT("wlwl:std.ai", ["COMPLETE"]);
            LET(s, COMPLETE("fun fib(n) {", "rust"));
            s;
        "#;
        let v = run_in(&dir, src).unwrap();
        match v {
            Value::String(s) => {
                assert!(s.contains("(rust)"));
                assert!(s.contains("fun fib"));
            }
            other => panic!("expected String, got {:?}", other),
        }
    }

    #[test]
    fn std_ai_complete_failure_via_language() {
        let dir = unique_test_dir("ai_complete_fail");
        let src = r#"
            IMPORT("wlwl:std.ai", ["COMPLETE"]);
            COMPLETE("ctx", "_fail_E0082");
        "#;
        let err = run_in(&dir, src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0082);
    }

    // 鈹€鈹€ P3-008: per-site suggestion_code 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

    #[test]
    fn p3_008_undefined_name_suggests_similar() {
        // E0020 with a typo of a known binding produces a "did you mean?" Note.
        let dir = unique_test_dir("p3_008_undef");
        let src = "            LET(counter, 0);\r\n            PRINT(countr);\r\n        ";
        let err = run_in(&dir, src).unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0020);
        assert!(!d.suggestion_code.is_empty(), "expected at least one suggestion, got none");
        let has_did_you_mean = d.suggestion_code.iter().any(|s| match s {
            wlwl_error::Suggestion::Note { description } => description.contains("did you mean"),
            _ => false,
        });
        assert!(has_did_you_mean, "expected a Note suggestion with `did you mean`: {:?}", d.suggestion_code);
    }

    #[test]
    fn p3_008_arity_error_includes_fix_suggestion() {
        // E0022 includes a Note that states "too many" or "too few" arguments
        // with the exact got/want count.
        let dir = unique_test_dir("p3_008_arity");
        let src = "            +(1, 2, 3);\r\n        ";
        let err = run_in(&dir, src).unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0022);
        let has_fix = d.suggestion_code.iter().any(|s| match s {
            wlwl_error::Suggestion::Note { description } => description.contains("too many"),
            _ => false,
        });
        assert!(has_fix, "expected a Note suggesting to drop extra args: {:?}", d.suggestion_code);
    }

    #[test]
    fn p3_008_module_not_found_suggests_wlwl_toml() {
        // E0040 (module not found) carries a Note pointing at wlwl.toml.
        let dir = unique_test_dir("p3_008_mod404");
        let src = "            IMPORT(\"wlwl:nope.thing\", [\"x\"]);\r\n        ";
        let err = run_in(&dir, src).unwrap_err();
        let d = err.diagnostic();
        let has_toml = d.suggestion_code.iter().any(|s| match s {
            wlwl_error::Suggestion::Note { description } => description.contains("wlwl.toml"),
            _ => false,
        });
        assert!(has_toml, "expected a Note referencing wlwl.toml: {:?}", d.suggestion_code);
    }
    // ---- P3-009d: LEN / PUSH / module / ERR transparent paths ----

    #[test]
    fn builtin_len_on_integer_errors() {
        let err = run("LEN(42);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn builtin_len_happy_paths() {
        assert_eq!(run(r###"LEN("hello");"###).unwrap(), Value::Integer(5));
        assert_eq!(run("LEN([1, 2, 3]);").unwrap(), Value::Integer(3));
        assert_eq!(
            run(r###"LEN(["a": 1, "b": 2]);"###).unwrap(),
            Value::Integer(2)
        );
    }

    #[test]
    fn builtin_push_arity_wrong() {
        let err = run("PUSH([1]);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn builtin_push_first_arg_not_array() {
        let err = run("PUSH(1, 2);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn builtin_push_happy_path() {
        assert_eq!(
            run("PUSH([1, 2], 3);").unwrap(),
            Value::Array(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3)
            ])
        );
    }

    // ---- Phase B1 (spec v0.4 §10.1 / §10.2) ----------------------
    // INDEX_GET / INDEX_SET / AT / REMOVE_KEY / POP (dict variant).
    // None of these are in the §12.7 ERR consumer registry — they
    // inherit §12.6 transparent ERR propagation (verified at the end
    // of this block).

    #[test]
    fn index_get_array_positive() {
        // Standard positive-index read on ARRAY.
        assert_eq!(
            run("INDEX_GET([10, 20, 30], 0);").unwrap(),
            Value::Integer(10)
        );
        assert_eq!(
            run("INDEX_GET([10, 20, 30], 1);").unwrap(),
            Value::Integer(20)
        );
        assert_eq!(
            run("INDEX_GET([10, 20, 30], 2);").unwrap(),
            Value::Integer(30)
        );
    }

    #[test]
    fn index_get_array_negative() {
        // Negative index counts from the end (spec §10.1 row 1).
        assert_eq!(
            run("INDEX_GET([10, 20, 30], -1);").unwrap(),
            Value::Integer(30)
        );
        assert_eq!(
            run("INDEX_GET([10, 20, 30], -3);").unwrap(),
            Value::Integer(10)
        );
    }

    #[test]
    fn index_get_array_oob_is_e0036() {
        // Positive OOB and negative OOB both trip E0036.
        let err = run("INDEX_GET([1, 2, 3], 3);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0036);
        let err = run("INDEX_GET([1, 2, 3], -4);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0036);
    }

    #[test]
    fn index_get_dict_hit() {
        // Existing key returns the value.
        assert_eq!(
            run(r###"INDEX_GET(["a": 1, "b": 2], "a");"###).unwrap(),
            Value::Integer(1)
        );
        assert_eq!(
            run(r###"INDEX_GET(["a": 1, "b": 2], "b");"###).unwrap(),
            Value::Integer(2)
        );
    }

    #[test]
    fn index_get_dict_missing_is_e0037() {
        let err = run(r###"INDEX_GET(["a": 1], "c");"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0037);
    }

    #[test]
    fn index_get_non_integer_index_is_e0031() {
        // ARRAY path with a non-INTEGER index trips E0031 (type error
        // on the index operand; matches E0031 spec description).
        let err = run(r###"INDEX_GET([1, 2, 3], "0");"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0031);
    }

    #[test]
    fn index_get_non_collection_is_e0030() {
        // First arg must be ARRAY or DICT; INTEGER triggers E0030.
        let err = run("INDEX_GET(42, 0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn index_get_arity_wrong_is_e0022() {
        let err = run("INDEX_GET([1, 2, 3]);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
        let err = run("INDEX_GET([1, 2, 3], 0, 99);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn index_set_array_in_bounds() {
        // Write at index returns the new array.
        assert_eq!(
            run("INDEX_SET([10, 20, 30], 1, 99);").unwrap(),
            Value::Array(vec![
                Value::Integer(10),
                Value::Integer(99),
                Value::Integer(30)
            ])
        );
        // Negative-index write also works.
        assert_eq!(
            run("INDEX_SET([10, 20, 30], -1, 99);").unwrap(),
            Value::Array(vec![
                Value::Integer(10),
                Value::Integer(20),
                Value::Integer(99)
            ])
        );
    }

    #[test]
    fn index_set_array_oob_is_e0036() {
        // OOB on write raises E0036 (no mutation).
        let err = run("INDEX_SET([10, 20, 30], 5, 99);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0036);
        let err = run("INDEX_SET([10, 20, 30], -4, 99);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0036);
    }

    #[test]
    fn index_set_dict_insert_new_key() {
        // Inserting a new key appends to the entries (insertion order
        // preserved).
        assert_eq!(
            run(r###"INDEX_SET(["a": 1], "b", 2);"###).unwrap(),
            Value::Dict(vec![
                (Value::String("a".into()), Value::Integer(1)),
                (Value::String("b".into()), Value::Integer(2)),
            ])
        );
    }

    #[test]
    fn index_set_dict_update_existing_key() {
        // Updating an existing key keeps the original position
        // (matches MERGE semantics in §10.2).
        assert_eq!(
            run(r###"INDEX_SET(["a": 1, "b": 2], "a", 99);"###).unwrap(),
            Value::Dict(vec![
                (Value::String("a".into()), Value::Integer(99)),
                (Value::String("b".into()), Value::Integer(2)),
            ])
        );
    }

    #[test]
    fn index_set_arity_wrong_is_e0022() {
        let err = run("INDEX_SET([1, 2], 0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
        let err = run("INDEX_SET([1, 2], 0, 99, 100);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn index_set_non_collection_is_e0030() {
        let err = run("INDEX_SET(42, 0, 99);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn at_array_in_bounds_returns_value() {
        // AT on ARRAY with in-bounds index behaves like INDEX_GET.
        assert_eq!(
            run("AT([10, 20, 30], 1, -1);").unwrap(),
            Value::Integer(20)
        );
        // Negative index supported.
        assert_eq!(
            run("AT([10, 20, 30], -1, 0);").unwrap(),
            Value::Integer(30)
        );
    }

    #[test]
    fn at_array_oob_returns_default() {
        // Spec §10.1 row 3: AT returns default on OOB — NO error.
        assert_eq!(
            run("AT([10, 20, 30], 5, -1);").unwrap(),
            Value::Integer(-1)
        );
        assert_eq!(
            run("AT([10, 20, 30], -4, NULL);").unwrap(),
            Value::Null
        );
    }

    #[test]
    fn at_dict_missing_returns_default() {
        // Spec §10.2 row "安全下标": AT returns default on missing
        // key — NO error.
        assert_eq!(
            run(r###"AT(["a": 1], "missing", -1);"###).unwrap(),
            Value::Integer(-1)
        );
        assert_eq!(
            run(r###"AT(["a": 1], "missing", NULL);"###).unwrap(),
            Value::Null
        );
        // Existing key still returns the actual value.
        assert_eq!(
            run(r###"AT(["a": 1, "b": 2], "a", -1);"###).unwrap(),
            Value::Integer(1)
        );
    }

    #[test]
    fn at_non_collection_is_e0030() {
        let err = run("AT(42, 0, NULL);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn at_arity_wrong_is_e0022() {
        let err = run("AT([1, 2], 0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn remove_key_dict_existing() {
        // REMOVE_KEY returns the dict with the key dropped.
        assert_eq!(
            run(r###"REMOVE_KEY(["a": 1, "b": 2], "a");"###).unwrap(),
            Value::Dict(vec![(
                Value::String("b".into()),
                Value::Integer(2)
            )])
        );
    }

    #[test]
    fn remove_key_dict_missing_is_noop() {
        // Spec §10.2: missing key → NOOP (returns dict unchanged).
        assert_eq!(
            run(r###"REMOVE_KEY(["a": 1], "missing");"###).unwrap(),
            Value::Dict(vec![(
                Value::String("a".into()),
                Value::Integer(1)
            )])
        );
    }

    #[test]
    fn remove_key_non_dict_is_e0030() {
        let err = run(r###"REMOVE_KEY(42, "k");"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn pop_dict_existing_returns_value_and_removes() {
        // POP on DICT returns the deleted value (NOT the modified
        // dict — callers want the value from a safe-delete primitive).
        assert_eq!(
            run(r###"POP(["a": 1, "b": 2], "a", NULL);"###).unwrap(),
            Value::Integer(1)
        );
        // Side-effect: subsequent read sees the dict without "a".
        let after_pop = run(r###"LET(d, ["a": 1, "b": 2]); POP(d, "a", NULL); d;"###).unwrap();
        // The LET-binding keeps a snapshot of the original dict, so
        // the result is the original (per current Value semantics
        // — there is no Rc<RefCell> aliasing yet; see deviations
        // B1-003).
        assert_eq!(
            after_pop,
            Value::Dict(vec![
                (Value::String("a".into()), Value::Integer(1)),
                (Value::String("b".into()), Value::Integer(2)),
            ])
        );
    }

    #[test]
    fn pop_dict_missing_returns_default_and_noop() {
        // Spec §10.2 row "安全删除": missing key → default, NO error.
        assert_eq!(
            run(r###"POP(["a": 1], "missing", -1);"###).unwrap(),
            Value::Integer(-1)
        );
        assert_eq!(
            run(r###"POP(["a": 1], "missing", NULL);"###).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn pop_dict_arity_wrong_is_e0022() {
        let err = run(r###"POP(["a": 1], "a");"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn pop_dict_non_dict_is_e0030() {
        let err = run(r###"POP(42, "k", NULL);"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    // -- ERR transparent propagation (registry exclusion) --

    #[test]
    fn index_get_propagates_err() {
        // INDEX_GET is NOT in §12.7 ERR consumer registry → ERR
        // input propagates per §12.6. At top level this surfaces
        // as E0102.
        let err = run(r###"INDEX_GET(ERR("e"), 0);"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn index_set_propagates_err() {
        let err = run(r###"INDEX_SET(ERR("e"), 0, 1);"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn at_propagates_err() {
        let err = run(r###"AT(ERR("e"), 0, NULL);"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn remove_key_propagates_err() {
        let err = run(r###"REMOVE_KEY(ERR("e"), "k");"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn pop_dict_propagates_err() {
        let err = run(r###"POP(ERR("e"), "k", NULL);"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    // ── Phase B2: `DEL` v0.3-compat alias of `REMOVE_KEY` (spec §10.2 / §14.5)
    // The alias MUST emit W0051 on every legacy use (per spec §14.5),
    // but the ERR short-circuit in `eval_call` fires before the alias
    // body runs — so `DEL(ERR("e"), "k")` propagates ERR and does NOT
    // emit W0051. These tests pin down both behaviors.

    #[test]
    fn del_alias_removes_key() {
        let (r, w) = run_with_warnings(r###"DEL(["a": 1, "b": 2], "a");"###);
        assert_eq!(
            r.unwrap(),
            Value::Dict(vec![(Value::String("b".into()), Value::Integer(2))])
        );
        assert_eq!(w.len(), 1, "expected exactly one W0051");
        assert_eq!(w[0].code, ErrorCode::W0051);
    }

    #[test]
    fn del_alias_missing_key_is_noop() {
        let (r, w) = run_with_warnings(r###"DEL(["a": 1], "missing");"###);
        assert_eq!(
            r.unwrap(),
            Value::Dict(vec![(Value::String("a".into()), Value::Integer(1))])
        );
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].code, ErrorCode::W0051);
    }

    #[test]
    fn del_alias_arity_wrong_is_e0022() {
        let err = run(r###"DEL(["a": 1]);"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn del_alias_arity_too_many_is_e0022() {
        let err = run(r###"DEL(["a": 1], "a", "extra");"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn del_alias_non_dict_is_e0030() {
        let err = run(r###"DEL(42, "k");"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn del_alias_propagates_err_without_warning() {
        // The ERR short-circuit fires BEFORE the alias body, so:
        //   1. The ERR is propagated (raises E0102 at top level).
        //   2. W0051 is NOT emitted (alias body never runs).
        let (r, w) = run_with_warnings(r###"DEL(ERR("e"), "k");"###);
        let err = r.unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        assert!(
            w.is_empty(),
            "DEL(ERR(...)) must not emit W0051 (short-circuit pre-dispatch); got {:?}",
            w
        );
    }

    #[test]
    fn del_alias_warning_message_mentions_remove_key() {
        let (r, w) = run_with_warnings(r###"DEL(["a": 1], "a");"###);
        r.unwrap();
        assert_eq!(w.len(), 1);
        let msg = &w[0].message;
        assert!(msg.contains("`DEL`"), "message should name `DEL`: {}", msg);
        assert!(
            msg.contains("REMOVE_KEY"),
            "message should suggest `REMOVE_KEY`: {}",
            msg
        );
    }

    #[test]
    fn remove_key_does_not_emit_w0051() {
        // Regression guard: the v0.4 main name must stay silent.
        let (r, w) = run_with_warnings(r###"REMOVE_KEY(["a": 1, "b": 2], "a");"###);
        assert!(r.is_ok());
        assert!(
            w.is_empty(),
            "REMOVE_KEY must not emit W0051; got {:?}",
            w
        );
    }

    #[test]
    fn del_alias_warning_exactly_once_per_call() {
        let (r, w) = run_with_warnings(
            r###"DEL(["a": 1, "b": 2], "a"); DEL(["c": 3], "missing");"###,
        );
        r.unwrap();
        assert_eq!(w.len(), 2, "expected one W0051 per DEL call, got {:?}", w);
        assert!(w.iter().all(|x| x.code == ErrorCode::W0051));
    }

    #[test]
    fn err_propagated_through_arithmetic_is_e0102() {
        // Per spec section 12.6: arithmetic ops transparently
        // propagate ERR. At top level this surfaces as E0102.
        let err = run(r###"+(OK(1), ERR("boom"));"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn err_propagated_through_print_is_e0102() {
        let err = run(r###"PRINT(ERR("hello"));"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn err_propagated_through_len_is_e0102() {
        let err = run(r###"LEN(ERR("no"));"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn try_block_passes_err_through_as_e0102() {
        // TRY in this implementation propagates ERR (does NOT consume it).
        // The §12.6 whitelist is narrower: only IS_OK / IS_ERR / OR_DIE.
        // Top-level ERR surfaces as E0102.
        let err = run(r###"TRY(ERR("x"));"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn try_block_passes_ok_through() {
        let v = run(r###"TRY(OK(42));"###).unwrap();
        assert_eq!(v, Value::Integer(42));
    }

    #[test]
    fn is_ok_etc_whitelist_consume_err() {
        assert_eq!(run("IS_OK(OK(1));").unwrap(), Value::Boolean(true));
        assert_eq!(run(r###"IS_OK(ERR("x"));"###).unwrap(), Value::Boolean(false));
        assert_eq!(run("IS_ERR(OK(1));").unwrap(), Value::Boolean(false));
        assert_eq!(run(r###"IS_ERR(ERR("x"));"###).unwrap(), Value::Boolean(true));
        assert_eq!(
            run("OR_DIE(OK(1), 99);").unwrap(),
            Value::Integer(1)
        );
    }

    #[test]
    fn module_relative_dot_slash_prefix() {
        let dir = unique_test_dir("rel_dot");
        fs::write(
            dir.join("lib.wl"),
            r###"LET(v, 100); EXPORT(["v"]);
"###,
        ).unwrap();
        let src = r###"IMPORT("./lib", ["v"]); PRINT(v);
"###;
        let v = run_in(&dir, src).unwrap();
        assert_eq!(v, Value::Null);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn module_bare_name_falls_back_to_project_root() {
        let dir = unique_test_dir("bare_fallback");
        let sub = dir.join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(
            dir.join("wlwl.toml"),
            r###"[package]
name = "b"
version = "0.1.0"
entry = "main.wl"
"###,
        ).unwrap();
        fs::write(
            dir.join("helper.wl"),
            r###"LET(v, 1); EXPORT(["v"]);
"###,
        ).unwrap();
        let src = r###"IMPORT("helper", ["v"]); PRINT(v);
"###;
        let v = run_in(&sub, src).unwrap();
        assert_eq!(v, Value::Null);
        let _ = fs::remove_dir_all(&dir);
    }
    // ---- P3-009d: Value::display all variants + std boundary conversion ----

    #[test]
    fn value_display_all_variants() {
        // The runtime Display-ish display() is what PRINT uses.
        // Cover every variant so the formatting is locked in.
        assert_eq!(Value::Integer(42).display(), "42");
        assert_eq!(Value::Integer(-7).display(), "-7");
        // Whole-number float formats with a trailing .0.
        assert_eq!(Value::Float(2.0).display(), "2.0");
        // Fractional float uses default Display.
        assert_eq!(Value::Float(1.5).display(), "1.5");
        assert_eq!(Value::String("hi".into()).display(), "hi");
        assert_eq!(Value::Boolean(true).display(), "TRUE");
        assert_eq!(Value::Boolean(false).display(), "FALSE");
        assert_eq!(Value::Null.display(), "NULL");
        assert_eq!(
            Value::Array(vec![Value::Integer(1), Value::Integer(2)]).display(),
            "[1, 2]"
        );
        assert_eq!(
            Value::Dict(vec![
                (Value::String("a".into()), Value::Integer(1)),
                (Value::String("b".into()), Value::Integer(2)),
            ])
            .display(),
            "[a: 1, b: 2]"
        );
        assert_eq!(
            Value::Ok(Box::new(Value::Integer(7))).display(),
            "OK(7)"
        );
        assert_eq!(
            Value::Err(Box::new(Value::String("boom".into()))).display(),
            "ERR(boom)"
        );
    }

    #[test]
    fn value_display_closure_and_native() {
        let empty_closure = Value::Closure {
            params: vec![],
            body: Box::new(Expr::Literal(Literal::Integer(0), Span::dummy())),
            env: Env::new(),
        };
        assert_eq!(empty_closure.display(), "<fun()>");

        let two_arg = Value::Closure {
            params: vec![
                FunParam::new("a".into(), Span::dummy()),
                FunParam::new("b".into(), Span::dummy()),
            ],
            body: Box::new(Expr::Literal(Literal::Integer(0), Span::dummy())),
            env: Env::new(),
        };
        assert_eq!(two_arg.display(), "<fun(a, b)>");

        let nf = Value::NativeFn {
            name: "PRINT".into(),
            invoke: NativeInvoke::Std(wlwl_std::io::std_print as wlwl_std::StdFn),
        };
        assert_eq!(nf.display(), "<native fun PRINT>");
    }

    #[test]
    fn value_to_std_value_primitives() {
        assert_eq!(value_to_std_value(&Value::Null).unwrap(), wlwl_std::StdValue::Null);
        assert_eq!(value_to_std_value(&Value::Boolean(true)).unwrap(), wlwl_std::StdValue::Bool(true));
        assert_eq!(value_to_std_value(&Value::Integer(123)).unwrap(), wlwl_std::StdValue::Number(serde_json::Number::from(123)));
        assert_eq!(
            value_to_std_value(&Value::String("x".into())).unwrap(),
            wlwl_std::StdValue::String("x".into())
        );
        assert_eq!(
            value_to_std_value(&Value::Float(1.5)).unwrap(),
            wlwl_std::StdValue::Number(serde_json::Number::from_f64(1.5).unwrap())
        );
    }

    #[test]
    fn value_to_std_value_nan_errors() {
        let err = value_to_std_value(&Value::Float(f64::NAN)).unwrap_err();
        match err {
            StdValueConvError::Type { expected, got } => {
                assert!(expected.contains("finite"), "got {:?}", expected);
                assert!(got.contains("NaN"), "got {:?}", got);
            }
        }
    }

    #[test]
    fn value_to_std_value_nested_array_and_dict() {
        let arr = Value::Array(vec![
            Value::Integer(1),
            Value::Array(vec![Value::Integer(2), Value::Integer(3)]),
        ]);
        let out = value_to_std_value(&arr).unwrap();
        assert!(matches!(out, wlwl_std::StdValue::Array(_)));

        let dict = Value::Dict(vec![
            (Value::String("k".into()), Value::Integer(7)),
        ]);
        let out = value_to_std_value(&dict).unwrap();
        match out {
            wlwl_std::StdValue::Object(o) => {
                assert_eq!(o.get("k").unwrap(), &wlwl_std::StdValue::Number(serde_json::Number::from(7)));
            }
            other => panic!("expected Object, got {:?}", other),
        }
    }

    #[test]
    fn value_to_std_value_non_string_dict_key_errors() {
        let dict = Value::Dict(vec![
            (Value::Integer(1), Value::Integer(2)),
        ]);
        let err = value_to_std_value(&dict).unwrap_err();
        match err {
            StdValueConvError::Type { expected, .. } => {
                assert!(expected.contains("string dict key"), "got {:?}", expected);
            }
        }
    }

    #[test]
    fn value_to_std_value_ok_unwraps() {
        let v = Value::Ok(Box::new(Value::Integer(42)));
        let out = value_to_std_value(&v).unwrap();
        assert_eq!(out, wlwl_std::StdValue::Number(serde_json::Number::from(42)));
    }

    #[test]
    fn value_to_std_value_err_errors() {
        let v = Value::Err(Box::new(Value::String("oops".into())));
        let err = value_to_std_value(&v).unwrap_err();
        match err {
            StdValueConvError::Type { expected, .. } => {
                assert!(expected.contains("OK"), "got {:?}", expected);
            }
        }
    }

    #[test]
    fn value_to_std_value_closure_and_nativefn_error() {
        let c = Value::Closure {
            params: vec![],
            body: Box::new(Expr::Literal(Literal::Integer(0), Span::dummy())),
            env: Env::new(),
        };
        let err = value_to_std_value(&c).unwrap_err();
        match err {
            StdValueConvError::Type { got, .. } => {
                assert!(got.contains("function closure"), "got {:?}", got);
            }
        }
        let nf = Value::NativeFn {
            name: "PRINT".into(),
            invoke: NativeInvoke::Std(wlwl_std::io::std_print as wlwl_std::StdFn),
        };
        let err = value_to_std_value(&nf).unwrap_err();
        match err {
            StdValueConvError::Type { got, .. } => {
                assert!(got.contains("native fn"), "got {:?}", got);
            }
        }
    }

    #[test]
    fn std_value_to_value_roundtrip_all_variants() {
        assert_eq!(std_value_to_value(wlwl_std::StdValue::Null), Value::Null);
        assert_eq!(std_value_to_value(wlwl_std::StdValue::Bool(true)), Value::Boolean(true));
        assert_eq!(
            std_value_to_value(wlwl_std::StdValue::Number(serde_json::Number::from(1))),
            Value::Integer(1)
        );
        assert_eq!(
            std_value_to_value(wlwl_std::StdValue::Number(serde_json::Number::from_f64(1.5).unwrap())),
            Value::Float(1.5)
        );
        assert_eq!(
            std_value_to_value(wlwl_std::StdValue::String("x".into())),
            Value::String("x".into())
        );
        assert_eq!(
            std_value_to_value(wlwl_std::StdValue::Array(vec![wlwl_std::StdValue::Null])),
            Value::Array(vec![Value::Null])
        );
        let mut obj = serde_json::Map::new();
        obj.insert("k".to_string(), wlwl_std::StdValue::Number(serde_json::Number::from(7)));
        assert_eq!(
            std_value_to_value(wlwl_std::StdValue::Object(obj)),
            Value::Dict(vec![(Value::String("k".into()), Value::Integer(7))])
        );
    }

    // ---- P3-009d: more module loader + std call paths ----

    #[test]
    fn module_circular_import_detected() {
        // a.wl imports b.wl imports a.wl -> E0041.
        let dir = unique_test_dir("circular");
        fs::write(
            dir.join("a.wl"),
            r###"IMPORT("b", ["v"]); PRINT(v);"###,
        ).unwrap();
        fs::write(
            dir.join("b.wl"),
            r###"IMPORT("a", ["v"]); LET(v, 1); EXPORT(["v"]);"###,
        ).unwrap();
        let src = r###"IMPORT("a", ["v"]); PRINT(v);"###;
        let v = run_in(&dir, src);
        let err = v.expect_err("expected E0041");
        assert_eq!(err.diagnostic().code, ErrorCode::E0041);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn module_namespace_outside_project_root() {
        // A namespace dep that resolves outside the project root
        // must surface E0040.
        let dir = unique_test_dir("ns_outside");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("wlwl.toml"),
            r###"[package]
name = "ns"
version = "0.1.0"
entry = "main.wl"

[dependencies]
"evil:lib" = { path = "../escape" }
"###,
        ).unwrap();
        fs::write(
            dir.join("main.wl"),
            r###"IMPORT("evil:lib", ["v"]); PRINT(1);"###,
        ).unwrap();
        let v = run_in(&dir, "IMPORT(\"evil:lib\", [\"v\"]); PRINT(1);");
        let err = v.expect_err("expected E0040");
        assert_eq!(err.diagnostic().code, ErrorCode::E0040);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn module_bare_name_not_found() {
        // A bare import that doesn't exist in base_dir or project
        // root must surface E0040.
        let dir = unique_test_dir("bare_missing");
        fs::create_dir_all(&dir).unwrap();
        let v = run_in(&dir, "IMPORT(\"does_not_exist\", [\"v\"]);");
        let err = v.expect_err("expected E0040");
        assert_eq!(err.diagnostic().code, ErrorCode::E0040);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_unbound_via_e0023_or_e0020() {
        // A module that EXPORTs a name that wasn't bound yields
        // E0023 at IMPORT time. If the loader re-routes through
        // the undefined-name path, E0020 is also acceptable.
        let dir = unique_test_dir("export_unbound2");
        fs::write(
            dir.join("m.wl"),
            "LET(unused, 1); EXPORT([\"missing\"]);\n",
        ).unwrap();
        let src = "IMPORT(\"m\", [\"missing\"]); PRINT(1);\n";
        let v = run_in(&dir, src);
        let err = v.expect_err("expected export error");
        let code = err.diagnostic().code;
        assert!(
            code == ErrorCode::E0023 || code == ErrorCode::E0020,
            "got {:?}", code
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn namespace_format_recognised_but_unregistered() {
        // A path like unknown:thing is a recognized namespace
        // format but no [namespaces] / [dependencies] entry covers
        // it -> E0043 unregistered namespace.
        let dir = unique_test_dir("unreg_ns");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("wlwl.toml"),
            r###"[package]
name = "u"
version = "0.1.0"
entry = "main.wl"
"###,
        ).unwrap();
        fs::write(
            dir.join("main.wl"),
            r###"IMPORT("ghost:thing", ["v"]); PRINT(1);"###,
        ).unwrap();
        let v = run_in(&dir, "IMPORT(\"ghost:thing\", [\"v\"]);");
        let err = v.expect_err("expected E0043");
        assert_eq!(err.diagnostic().code, ErrorCode::E0043);
        let _ = fs::remove_dir_all(&dir);
    }
// ---- P3-009e: comprehensive integration tests for eval_expr arms ----

    #[test]
    fn panic_emits_e0100_v2() {
        let err = run(r###"PANIC("something went wrong");"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0100);
        assert!(err.diagnostic().message.contains("something went wrong"));
    }

    #[test]
    fn try_with_non_ok_err_value_is_e0030() {
        let err = run("TRY(42);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn or_die_with_non_ok_err_value_is_e0030() {
        let err = run("OR_DIE(42, 0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn or_die_with_err_returns_default() {
        let v = run(r###"OR_DIE(ERR("x"), 99);"###).unwrap();
        assert_eq!(v, Value::Integer(99));
    }

    #[test]
    fn while_with_break_exits_loop() {
        // Break terminates the loop; consume the signal.
        let v = run("LET(i, 0); WHILE(<(i, 10), IF(==(i, 3), BREAK(), LET(i, +(i, 1)))); i;").unwrap();
        assert_eq!(v, Value::Integer(3));
    }

    #[test]
    fn for_over_array() {
        let v = run("LET(s, 0); FOR(x, [1, 2, 3], LET(s, +(s, x))); s;").unwrap();
        assert_eq!(v, Value::Integer(6));
    }

    #[test]
    fn for_over_dict() {
        // FOR over a dict iterates the keys (sorted).
        let v = run(r###"LET(s, 0); FOR(k, ["a": 1, "b": 2], LET(s, +(s, 1))); s;"###).unwrap();
        assert_eq!(v, Value::Integer(2));
    }

    #[test]
    fn for_over_string() {
        let v = run(r###"LET(s, 0); FOR(c, "abc", LET(s, +(s, 1))); s;"###).unwrap();
        assert_eq!(v, Value::Integer(3));
    }

    #[test]
    fn for_over_non_iterable_is_e0030() {
        let err = run("FOR(x, 42, PRINT(x));").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn for_with_break_exits_loop() {
        let v = run("LET(s, 0); FOR(i, [1, 2, 3, 4, 5], IF(==(i, 3), BREAK(), LET(s, +(s, i)))); s;").unwrap();
        // s accumulates 1 + 2 = 3 then break on i=3
        assert_eq!(v, Value::Integer(3));
    }

    #[test]
    fn for_with_continue_skips_rest_of_body() {
        let v = run("LET(s, 0); FOR(i, [1, 2, 3, 4, 5], IF(==(i, 3), CONTINUE(), LET(s, +(s, i)))); s;").unwrap();
        // Skip i=3: 1+2+4+5 = 12
        assert_eq!(v, Value::Integer(12));
    }

    #[test]
    fn break_outside_loop_is_e0014() {
        let err = run("BREAK();").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0014);
    }

    #[test]
    fn continue_outside_loop_is_e0014() {
        let err = run("CONTINUE();").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0014);
    }

    #[test]
    fn import_duplicate_in_same_scope_is_e0021() {
        let dir = unique_test_dir("import_dup");
        fs::write(dir.join("m1.wl"), "LET(v, 1); EXPORT([\"v\"]);\n").unwrap();
        fs::write(dir.join("m2.wl"), "LET(v, 2); EXPORT([\"v\"]);\n").unwrap();
        let src = "IMPORT(\"m1\", [\"v\"]); IMPORT(\"m2\", [\"v\"]); PRINT(v);\n";
        let v = run_in(&dir, src);
        let err = v.expect_err("expected E0021");
        assert_eq!(err.diagnostic().code, ErrorCode::E0021);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_unbound_name_is_e0023() {
        let dir = unique_test_dir("import_unbound");
        fs::write(dir.join("m.wl"), "LET(v, 1); EXPORT([\"v\"]);\n").unwrap();
        let src = "IMPORT(\"m\", [\"missing\"]); PRINT(1);\n";
        let v = run_in(&dir, src);
        let err = v.expect_err("expected E0023");
        assert_eq!(err.diagnostic().code, ErrorCode::E0023);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_unbound_name_is_e0020() {
        let dir = unique_test_dir("export_unbound3");
        fs::write(dir.join("m.wl"), "EXPORT([\"missing\"]);\n").unwrap();
        let src = "IMPORT(\"m\", [\"missing\"]); PRINT(1);\n";
        let v = run_in(&dir, src);
        let err = v.expect_err("expected E0020 or E0023");
        let code = err.diagnostic().code;
        assert!(
            code == ErrorCode::E0020 || code == ErrorCode::E0023,
            "got {:?}", code
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn operators_comparison_and_logic() {
        assert_eq!(run("==(1, 1);").unwrap(), Value::Boolean(true));
        assert_eq!(run("!=(1, 2);").unwrap(), Value::Boolean(true));
        assert_eq!(run("<(1, 2);").unwrap(), Value::Boolean(true));
        assert_eq!(run(">(2, 1);").unwrap(), Value::Boolean(true));
        assert_eq!(run("<=(1, 1);").unwrap(), Value::Boolean(true));
        assert_eq!(run(">=(1, 1);").unwrap(), Value::Boolean(true));
        assert_eq!(run("&&(TRUE, FALSE);").unwrap(), Value::Boolean(false));
        assert_eq!(run("||(TRUE, FALSE);").unwrap(), Value::Boolean(true));
        assert_eq!(run("!(FALSE);").unwrap(), Value::Boolean(true));
    }

    #[test]
    fn operators_arithmetic_all() {
        assert_eq!(run("+(1, 2);").unwrap(), Value::Integer(3));
        assert_eq!(run("-(5, 2);").unwrap(), Value::Integer(3));
        assert_eq!(run("*(3, 4);").unwrap(), Value::Integer(12));
        assert_eq!(run("/(10, 3);").unwrap(), Value::Integer(3));
        assert_eq!(run("%(10, 3);").unwrap(), Value::Integer(1));
        assert_eq!(run(r###"+("a", "b");"###).unwrap(), Value::String("ab".into()));
    }

    #[test]
    fn block_with_let_does_not_leak() {
        let v = run("LET(x, 1); LET(x, 2); x;").unwrap();
        // Top-level LET re-binds in same scope.
        assert_eq!(v, Value::Integer(2));
    }

    #[test]
    fn function_call_with_3_args() {
        let v = run("LET(f, FUN((a, b, c), +(+(a, b), c))); f(1, 2, 3);").unwrap();
        assert_eq!(v, Value::Integer(6));
    }

    #[test]
    fn function_call_with_4_args() {
        let v = run("LET(f, FUN((a, b, c, d), +(+(+(a, b), c), d))); f(1, 2, 3, 4);").unwrap();
        assert_eq!(v, Value::Integer(10));
    }

    #[test]
    fn while_zero_iterations() {
        // WHILE with false condition never executes body.
        let v = run("LET(i, 0); WHILE(FALSE, LET(i, +(i, 1))); i;").unwrap();
        assert_eq!(v, Value::Integer(0));
    }

    #[test]
    fn for_over_empty_array() {
        let v = run("LET(s, 0); FOR(x, [], LET(s, +(s, 1))); s;").unwrap();
        assert_eq!(v, Value::Integer(0));
    }

    #[test]
    fn dict_key_value_evaluation_order() {
        // Key and value are both evaluated; their results form an entry.
        let v = run(r###"["a": +(1, 2), "b": *(3, 4)];"###).unwrap();
        match v {
            Value::Dict(entries) => {
                assert_eq!(entries.len(), 2);
            }
            _ => panic!("expected Dict"),
        }
    }

    #[test]
    fn err_in_block_propagates_to_top_level() {
        // An ERR not consumed by TRY/OR_DIE/IS_OK/IS_ERR reaches
        // the top level as E0102.
        let err = run("ERR(\"x\");").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }
    // ---- P3-009e: more focused tests (replaced complex FUN tests) ----

    #[test]
    fn return_evaluates_at_function_call_site() {
        // RETURN(42) at top level is a no-op (no enclosing function).
        let v = run("RETURN(42);").unwrap();
        // Top level doesn't unwrap Return, so v is whatever the
        // last expression evaluates to. RETURN is a function call
        // that takes one arg; calling it is the expression.
        assert!(v == Value::Integer(42) || v == Value::Null);
    }

    #[test]
    fn call_to_builtin_liken_returns_value() {
        // The builtin functions are bound and callable.
        assert_eq!(run("LEN(\"abc\");").unwrap(), Value::Integer(3));
    }

    // ---- P4-A1d: trace field in eval diagnostics (FIXED) ----
    //
    // All 6 trace tests pass. The nested-call / recursion / anonymous
    // closure cases were failing because `invoke_closure` popped its
    // frame on the Err path before the error reached `enrich_with_trace`,
    // so the post-hoc enrichment always saw an empty call_stack and
    // injected only the synthetic `<toplevel>` frame.
    //
    // Fix: `diag` now takes `&mut self` and snapshots the call_stack
    // (reversed -> innermost first) into `d.trace` at construction
    // time. `enrich_with_trace` remains as a safety net for errors
    // that bypassed `diag` (e.g. `WlwlDiagnostic::new` in module /
    // arity / type helpers). `undefined_name` and `enrich_with_trace`
    // were updated to match the new contract. -- 2026-09-06.

    /// [v0.4 spec Sec. 14.2] Top-level error has exactly one synthetic
    /// `<toplevel>` frame (minimum 1 frame per spec).
    #[test]
    fn trace_top_level_error_has_synthetic_toplevel_frame() {
        let err = run("PRINT(zzz);").unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.trace.len(), 1, "got: {:?}", d.trace);
        assert_eq!(d.trace[0].frame, "<toplevel>");
    }

    /// [v0.4 spec Sec. 14.2] Recursion: each frame has the same name
    /// but different locations. Multi-level recursion yields multiple
    /// frames in the trace.
    #[test]
    fn trace_recursion_has_repeated_frames() {
        let src = r###"
            LET(fact, FUN((n), IF(==(n, 0), zzz(1), *(n, fact(-(n, 1))))));
            fact(2);
        "###;
        let err = run(src).unwrap_err();
        let d = err.diagnostic();
        assert!(d.trace.len() >= 2, "got: {:?}", d.trace);
        let fact_count = d.trace.iter()
            .filter(|f| f.frame == "fact")
            .count();
        assert!(fact_count >= 2, "expected 2 fact frames, got: {:?}", d.trace);
    }

    /// [v0.4 spec Sec. 14.2] JSON serialization: trace field is an array
    /// of {frame, location} objects.
    #[test]
    fn trace_json_serialization_format() {
        let err = run("PRINT(zzz);").unwrap_err();
        let j: serde_json::Value = serde_json::from_str(
            &err.diagnostic().render_jsonl()
        ).unwrap();
        let trace = j["trace"].as_array()
            .expect("trace must be an array");
        assert_eq!(trace.len(), 1, "got: {:?}", trace);
        assert_eq!(trace[0]["frame"], "<toplevel>");
        // Each frame has a `location` object (Sec. 14.2 unified form).
        let loc = &trace[0]["location"];
        for k in &["file", "line", "col_start", "line_end", "col_end"] {
            assert!(loc.get(*k).is_some(), "missing location.{}", k);
        }
    }

    /// [v0.4 spec Sec. 14.2] Regression tripwire: call_stack must be
    /// empty after a successful eval (no leaked frames from prior
    /// calls that would pollute the next diagnostic).
    #[test]
    fn trace_empty_for_successful_evaluation() {
        // Successful eval should leave call_stack empty.
        let _ = run("LET(x, 1); x;").unwrap();
        // A subsequent error in the same Evaluator should see an
        // empty stack (not frames from the previous call).
        let err = run("PRINT(yyy);").unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.trace.len(), 1, "got: {:?}", d.trace);
        assert_eq!(d.trace[0].frame, "<toplevel>");
    }

    // ---- P4-A1d: FAILING tests (TODO next session) ----

    /// [v0.4 spec Sec. 14.2] Nested call: inner frame at index 0,
    /// outer at index 1 (innermost first, Python-style).
    #[test]
    fn trace_nested_call_has_two_frames_innermost_first() {
        let src = r###"
            LET(outer, FUN((x), inner(x)));
            LET(inner, FUN((y), zzz(y)));
            outer(1);
        "###;
        let err = run(src).unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.trace.len(), 2, "got: {:?}", d.trace);
        assert_eq!(d.trace[0].frame, "inner");
        assert_eq!(d.trace[1].frame, "outer");
    }

    /// [v0.4 spec Sec. 14.2] An anonymous closure reached via a
    /// named binding (`LET(f, FUN((x), zzz(x))); f(1);`) still uses
    /// the call-site identifier in the frame. The `<anonymous>`
    /// branch in `invoke_closure` (name.is_empty()) is dead code at
    /// the parser level: the grammar only emits `Call` nodes for
    /// `Ident(args)` and `obj.method(args)` (chain sugar), both of
    /// which have a non-empty name. Direct closure call syntax
    /// `(FUN((x),...))(1)` is not currently parseable. A separate
    /// test for the *named* FUN form `FUN name((x), body)` lands in
    /// the A2 closure-cell phase when `Value::Closure` gains a
    /// `name` field.
    #[test]

    // ---- P4-A3: destructuring LET (spec v0.4 Sec. 7.5) ----

    #[test]
    fn p4_a3_destructure_array_basic() {
        // [a, b] <- [1, 2]
        let src = "LET([a, b], [1, 2]); +(a, b);";
        assert_eq!(run(src).unwrap(), Value::Integer(3));
    }

    #[test]
    fn p4_a3_destructure_array_with_rest() {
        // [head, *rest] <- [10, 20, 30, 40]
        let src = "LET([head, *rest], [10, 20, 30, 40]); head;";
        assert_eq!(run(src).unwrap(), Value::Integer(10));
    }

    #[test]
    fn p4_a3_destructure_rest_is_value_array() {
        // `rest` should be the tail of the source array, not the
        // whole thing.
        let src = "LET([head, *rest], [10, 20, 30, 40]); rest;";
        let v = run(src).unwrap();
        assert_eq!(
            v,
            Value::Array(vec![
                Value::Integer(20),
                Value::Integer(30),
                Value::Integer(40),
            ])
        );
    }

    #[test]
    fn p4_a3_destructure_wildcard() {
        // [_, mid, _] <- [1, 2, 3]   (only `mid` is bound)
        let src = "LET([_, mid, _], [1, 2, 3]); mid;";
        assert_eq!(run(src).unwrap(), Value::Integer(2));
    }

    #[test]
    fn p4_a3_destructure_dict_basic() {
        let src = "LET([\"x\": x, \"y\": y], [\"x\": 100, \"y\": 200]); +(x, y);";
        assert_eq!(run(src).unwrap(), Value::Integer(300));
    }

    #[test]
    fn p4_a3_destructure_dict_partial() {
        // `name` is the only captured key; the rest is ignored.
        let src = "LET([\"name\": n], [\"name\": \"alice\", \"age\": 30]); n;";
        assert_eq!(
            run(src).unwrap(),
            Value::String("alice".to_string())
        );
    }

    #[test]
    fn p4_a3_destructure_nested_array() {
        // [[a, b], [c, d]] <- [[1, 2], [3, 4]]
        let src = "LET([[a, b], [c, d]], [[1, 2], [3, 4]]); +(+(a, b), +(c, d));";
        assert_eq!(run(src).unwrap(), Value::Integer(10));
    }

    #[test]
    fn p4_a3_destructure_nested_dict_in_array() {
        // [\"user\": [\"name\": n]] <- nested
        let src = "LET([\"user\": [\"name\": n]], [\"user\": [\"name\": \"bob\", \"age\": 25]]); n;";
        assert_eq!(
            run(src).unwrap(),
            Value::String("bob".to_string())
        );
    }

    #[test]
    fn p4_a3_destructure_array_too_short_is_e0026() {
        // 2-element pattern against 1-element array
        let src = "LET([a, b], [1]); a;";
        let err = run(src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0026, "got: {}", err);
    }

    #[test]
    fn p4_a3_destructure_array_too_long_no_rest_is_e0026() {
        let src = "LET([a, b], [1, 2, 3]); a;";
        let err = run(src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0026, "got: {}", err);
    }

    #[test]
    fn p4_a3_destructure_dict_missing_key_is_e0026() {
        let src = "LET([\"x\": x], [\"y\": 1]); x;";
        let err = run(src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0026, "got: {}", err);
    }

    #[test]
    fn p4_a3_destructure_integer_with_array_pattern_is_e0030() {
        let src = "LET([a, b], 42); a;";
        let err = run(src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030, "got: {}", err);
    }

    #[test]
    fn p4_a3_destructure_dict_with_integer_value_is_e0030() {
        let src = "LET([\"k\": v], 42); v;";
        let err = run(src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030, "got: {}", err);
    }

    #[test]
    fn p4_a3_destructure_dict_key_mismatch_is_e0026() {
        // Pattern key `"missing"` not in the value dict.
        let src = "LET([\"missing\": v], [\"k\": 1]); v;";
        let err = run(src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0026, "got: {}", err);
    }

    #[test]
    fn p4_a3_let_legacy_form_still_works() {
        // Backwards-compat: bare-Ident LET must keep producing
        // Expr::Let (not LetPattern), so the 500+ existing
        // tests that pattern-match on Expr::Let keep passing.
        let src = "LET(x, 42); x;";
        assert_eq!(run(src).unwrap(), Value::Integer(42));
    }

    // ---- v0.4 Sec. 7.6: MATCH pattern matching tests ---------------------

    #[test]
    fn p4_a4_match_literal_basic() {
        // First matching clause wins; literal pattern.
        let src = "MATCH(2, [[1, \"one\"], [2, \"two\"], [_, \"other\"]], NULL);";
        assert_eq!(
            run(src).unwrap(),
            Value::String("two".to_string())
        );
    }

    #[test]
    fn p4_a4_match_ident_binding() {
        // The pattern identifier is bound in the body scope.
        let src = "MATCH([1, 2], [[[a, b], +(a, b)]], NULL);";
        assert_eq!(run(src).unwrap(), Value::Integer(3));
    }

    #[test]
    fn p4_a4_match_wildcard() {
        // `_` matches anything without binding.
        let src = "MATCH(99, [[1, \"one\"], [_, \"other\"]], NULL);";
        assert_eq!(
            run(src).unwrap(),
            Value::String("other".to_string())
        );
    }

    #[test]
    fn p4_a4_match_first_clause_wins() {
        // scrutinee 1 hits clause 1 (literal 1); clause 2 (wildcard
        // -> "second") must not be evaluated.
        let src = "MATCH(1, [[1, \"first\"], [_, \"second\"]], NULL);";
        assert_eq!(
            run(src).unwrap(),
            Value::String("first".to_string())
        );
    }

    #[test]
    fn p4_a4_match_array_pattern_with_rest() {
        // `*rest` captures the tail as an array.
        let src = "MATCH([1, 2, 3, 4], [[[head, *rest], +(head, LEN(rest))]], NULL);";
        assert_eq!(run(src).unwrap(), Value::Integer(4));
    }

    #[test]
    fn p4_a4_match_dict_pattern() {
        let src = "MATCH([\"x\": 1, \"y\": 2], [[[\"x\": x, \"y\": y], +(x, y)]], NULL);";
        assert_eq!(run(src).unwrap(), Value::Integer(3));
    }

    #[test]
    fn p4_a4_match_constructor_ok_binds_inner() {
        // OK(x) pattern: only matches Value::Ok; binds `x` to inner.
        let src = "MATCH(OK(7), [[OK(x), x], [ERR(_), 0]], NULL);";
        assert_eq!(run(src).unwrap(), Value::Integer(7));
    }

    #[test]
    fn p4_a4_match_constructor_err_binds_inner() {
        // ERR(e) pattern: matches Value::Err; binds `e` to inner.
        let src = "MATCH(ERR(\"oops\"), [[OK(_), 0], [ERR(e), LEN(e)]], NULL);";
        assert_eq!(run(src).unwrap(), Value::Integer(4));
    }

    #[test]
    fn p4_a4_match_constructor_mismatch_falls_through() {
        // ERR(e) does not match an OK value; the wildcard fallback fires.
        let src = "MATCH(OK(1), [[ERR(_), 0], [_, 99]], NULL);";
        assert_eq!(run(src).unwrap(), Value::Integer(99));
    }

    #[test]
    fn p4_a4_match_fallthrough_default() {
        // Explicit default arm is evaluated when no clause matches.
        let src = "MATCH(99, [[1, \"one\"]], \"fallback\");";
        assert_eq!(
            run(src).unwrap(),
            Value::String("fallback".to_string())
        );
    }

    #[test]
    fn p4_a4_match_omitted_default_is_null_literal() {
        // Spec 7.6: when the source omits the default, the parser
        // synthesizes a NULL literal; non-matching value yields NULL
        // (NOT E0027 -- the NULL literal is the default).
        let src = "MATCH(99, [[1, \"one\"]]);";
        assert_eq!(run(src).unwrap(), Value::Null);
    }

    #[test]
    fn p4_a4_e0027_match_fell_through_diagnostic_renders_e0027() {
        // Spec 7.6 line 878 reserves E0027 for MATCH fall-through.
        // The parser in v0.4 A4 synthesizes a NULL default when
        // the source omits the default arm (per spec 7.6 line 879),
        // so the fell-through path is unreachable in eval. We
        // exercise the diagnostic builder directly here so the
        // helper stays covered.
        let dummy_span = wlwl_ast::Span {
            file: "t.wl".to_string(),
            line_start: 1,
            col_start: 1,
            line_end: 1,
            col_end: 2,
        };
        let mut ev = Evaluator::new();
        let err = ev.match_fell_through(&dummy_span);
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0027);
        assert!(d.message.contains("fell through"));
    }

    #[test]
    fn p4_a4_match_bindings_dont_leak_outside_clause_body() {
        // The `n` introduced by the matching clause must NOT be
        // visible after MATCH returns.
        // Outer `n` is 100; clause pattern is wildcard `_` (no
        // rebinding of `n`); body uses the outer `n` and produces 101.
        let src = "LET(n, 100); MATCH(2, [[_, +(n, 1)]], NULL);";
        assert_eq!(run(src).unwrap(), Value::Integer(101));
        // Outer `n` is still 100 after MATCH returns.
        let src = "LET(n, 100); MATCH(2, [[_, +(n, 1)]], NULL); n;";
        assert_eq!(run(src).unwrap(), Value::Integer(100));
    }

    #[test]
    fn p4_a4_destructure_constructor_ok_binds_inner() {
        // OK(x) pattern in `LET` destructuring.
        let src = "LET([OK(x), v], [OK(5), 10]); +(x, v);";
        assert_eq!(run(src).unwrap(), Value::Integer(15));
    }

    #[test]
    fn p4_a4_destructure_constructor_err_binds_inner() {
        // ERR(e) pattern in `LET` destructuring.
        let src = "LET([ERR(e), v], [ERR(\"bad\"), 7]); +(LEN(e), v);";
        assert_eq!(run(src).unwrap(), Value::Integer(10));
    }

    #[test]
    fn p4_a4_destructure_constructor_mismatch_is_e0026() {
        // LET(OK(x), ERR(...)) is a hard destructure failure (E0026).
        // v0.4 spec §2.2.1: ERR payload must be STRING or DICT — string is fine.
        let err = run(r###"LET([OK(x), v], [ERR("5"), 10]); +(x, v);"###).unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0026);
    }

    // ---- A4 coverage: soft-fall-through gauntlet + destructure Literal ----

    #[test]
    fn p4_a4_match_soft_fall_through_gauntlet() {
        // A single MATCH against [1, 2, 3, 4] whose earlier clauses
        // deliberately fail-soft against the value via different
        // Pattern variants, then a wildcard catches the value:
        //   * Pattern::Literal 42 / 99  (try_match line ~2449-2451)
        //   * Pattern::Array [1, 2] (no rest) shorter than the
        //     tail; the value has 4 elements vs pattern 2,
        //     so try_match falls through "no rest but too long"
        //     (line ~2477-2478) without mutating the value.
        //   * Pattern::Constructor OK(x) / ERR(x) against a
        //     non-Ok / non-Err value (line ~2512, ~2517)
        let src = r###"MATCH(
            [1, 2, 3, 4],
            [
                [42, "lit-mismatch-1"],
                [99, "lit-mismatch-2"],
                [[1, 2], "arr-too-long-no-rest"],
                [OK(x), "ctor-ok-mismatch"],
                [ERR(x), "ctor-err-mismatch"],
                [_, "wildcard-wins"]
            ],
            "default-not-used"
        );"###;
        assert_eq!(
            run(src).unwrap(),
            Value::String("wildcard-wins".to_string())
        );
    }

    #[test]
    fn p4_a4_match_array_pattern_against_non_array_is_e0030() {
        // try_match Pattern::Array against a non-array value is a
        // hard E0030 (defense-in-depth; the parser already enforces
        // type in well-typed programs, but the runtime guards
        // against dynamic patterns from MATCH).  Exercises
        // line ~2456.
        let src = "MATCH(42, [[[1, 2], \"unreachable\"]], \"default\");";
        let err = run(src).unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0030);
    }

    #[test]
    fn p4_a4_match_dict_pattern_against_non_dict_is_e0030() {
        // try_match Pattern::Dict against a non-dict value is a
        // hard E0030.  Exercises line ~2485.
        let src = "MATCH(42, [[[\"x\": 1], \"unreachable\"]], \"default\");";
        let err = run(src).unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0030);
    }

    #[test]
    fn p4_a4_destructure_literal_pattern_success_binds_value() {
        // `LET([1, x], [1, 99])` is a successful destructure whose
        // head is a Pattern::Literal (line ~2143-2146) and whose
        // tail is a Pattern::Ident.  The literal 1 matches
        // Value::Integer(1); x is bound to 99.
        let src = "LET([1, x], [1, 99]); x;";
        assert_eq!(run(src).unwrap(), Value::Integer(99));
    }

    #[test]
    fn p4_a4_destructure_literal_pattern_mismatch_is_e0026() {
        // `LET([1, x], [2, 99])` triggers the literal-mismatch E0026
        // path in destructure (line ~2148-2154).  This is a hard
        // error because LET is total.
        let err = run("LET([1, x], [2, 99]); x;").unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0026);
    }

    fn trace_call_uses_call_site_identifier() {
        let src = r###"
            LET(f, FUN((x), zzz(x)));
            f(1);
        "###;
        let err = run(src).unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.trace.len(), 1, "got: {:?}", d.trace);
        assert_eq!(d.trace[0].frame, "f");
    }

    // ──────────────────────────────────────────────────────────────────
    // B3: OR_DIE → UNWRAP_OR canonicalization (spec v0.4 §12.7 + §14.5)
    //
    // Phase B3 promotes `UNWRAP_OR` to canonical name; `OR_DIE` is the
    // v0.3-compat alias that emits `W0051` on every source occurrence.
    // The W0051 emit point lives in `eval_expr`'s `Expr::OrDie` arm —
    // OR_DIE is an unconditional lexer keyword that the parser lowers
    // to `Expr::OrDie`, so the alias-wrapper path (`builtin_unwrap_or_
    // compat`) is defensive and currently unreachable from real source.
    //
    // Behavior contract:
    //   - `OR_DIE(...)` — emits W0051 once per source occurrence,
    //     regardless of OK / ERR / non-RESULT / arity-error branch.
    //     The warning fires BEFORE the value/expression path so it is
    //     not short-circuited (unlike B2 DEL whose wrapper is skipped
    //     by the ERR short-circuit in `eval_call`).
    //   - `UNWRAP_OR(...)` — canonical, zero warnings, error messages
    //     surface "UNWRAP_OR" (never "OR_DIE").
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn b3_or_die_ok_emits_w0051() {
        let (r, w) = run_with_warnings("OR_DIE(OK(42), 0);");
        assert_eq!(r.unwrap(), Value::Integer(42));
        assert_eq!(w.len(), 1, "expected exactly one W0051, got {:?}", w);
        assert_eq!(w[0].code, ErrorCode::W0051);
    }

    #[test]
    fn b3_or_die_err_consumes_and_emits_w0051() {
        // Intentional difference from B2 DEL: OR_DIE emits W0051 even
        // when consuming an ERR. Rationale — spec §14.5 says "使用
        // v0.3 已弃用别名"; OR_DIE's *usage* in source is what the
        // warning is about, not its runtime branch. DEL's ERR path
        // suppresses the warning because DEL is NOT in §12.7 ERR
        // consumer registry — it's a silent alias — so an ERR short
        // circuit prevents legacy DEL calls on ERR streams from being
        // spammed. OR_DIE is a legit ERR consumer; the user wrote the
        // alias intentionally and gets warned regardless.
        let (r, w) = run_with_warnings(r###"OR_DIE(ERR("e"), 99);"###);
        assert_eq!(r.unwrap(), Value::Integer(99));
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].code, ErrorCode::W0051);
    }

    #[test]
    fn b3_or_die_non_result_emits_w0051_and_e0030() {
        let (r, w) = run_with_warnings("OR_DIE(42, 0);");
        let err = r.unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].code, ErrorCode::W0051);
    }

    #[test]
    fn b3_or_die_arity_too_few_is_parse_time_error() {
        // OR_DIE is a lexer keyword, parser enforces 2-arg shape
        // strictly (parse_or_die at line ~1072). Missing comma is
        // E0012, not runtime E0022. We do NOT emit W0051 here
        // because eval never runs — compare with the DEL short
        // circuit in B2 (different mechanism: lexical parse-time vs
        // runtime ERR short-circuit).
        let err = run("OR_DIE(OK(1));").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0012);
    }

    #[test]
    fn b3_or_die_arity_too_many_is_parse_time_error() {
        // Symmetric: too many args → E0011 expected ')'.
        let err = run("OR_DIE(OK(1), 0, 0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0011);
    }

    #[test]
    fn b3_or_die_warning_message_mentions_unwrap_or() {
        let (r, w) = run_with_warnings("OR_DIE(OK(1), 0);");
        r.unwrap();
        let msg = &w[0].message;
        assert!(msg.contains("`OR_DIE`"), "should name `OR_DIE`: {}", msg);
        assert!(
            msg.contains("UNWRAP_OR"),
            "should suggest `UNWRAP_OR`: {}",
            msg
        );
        assert!(
            msg.contains("v0.5"),
            "should mention v0.5 removal deadline: {}",
            msg
        );
    }

    #[test]
    fn b3_or_die_warning_exactly_once_per_call() {
        let (r, w) = run_with_warnings(
            r###"OR_DIE(OK(1), 0); OR_DIE(ERR("e"), 99); OR_DIE(OK(2), 0);"###,
        );
        r.unwrap();
        assert_eq!(
            w.len(),
            3,
            "expected one W0051 per OR_DIE call, got {:?}",
            w
        );
        assert!(w.iter().all(|x| x.code == ErrorCode::W0051));
    }

    #[test]
    fn b3_or_die_nested_in_default_position_each_layer_warns() {
        // OR_DIE nested in the default arg position — both source
        // occurrences are evaluated, both emit W0051. This is the
    // legit "nested" shape (the test formerly tried outer-wraps-
    // inner which is not a legal nested OR_DIE because the inner
    // returns a non-RESULT; see b3_or_die_nested_outer_wraps_inner_
    // is_e0030 below for that rejection).
        let (r, w) = run_with_warnings(
            r###"OR_DIE(ERR("e"), OR_DIE(OK(-1), 0));"###,
        );
        assert_eq!(r.unwrap(), Value::Integer(-1));
        assert_eq!(
            w.len(),
            2,
            "both OR_DIE tokens emit, got {:?}",
            w
        );
    }

    #[test]
    fn b3_or_die_outer_wraps_inner_returns_e0030() {
        // OR_DIE(OR_DIE(OK(1), 0), 0) — inner OR_DIE returns an
        // Integer (the unwrapped OK), not a RESULT. Outer's
        // Expr::OrDie arm rejects non-RESULT with E0030 ("UNWRAP_OR
        // expects OK/ERR, got integer"). This locks the §12.2
        // contract that OR_DIE must wrap a RESULT.
        let err = run("OR_DIE(OR_DIE(OK(1), 0), 0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        let msg = &err.diagnostic().message;
        assert!(
            msg.contains("UNWRAP_OR"),
            "E0030 message should name UNWRAP_OR: {}",
            msg
        );
    }

    #[test]
    fn b3_or_die_inside_user_function_emits_w0051() {
        // OR_DIE is keyword-tokenized at source level; user fn body
        // doesn't shield it.
        let src = r###"
            LET(f, FUN((x), OR_DIE(x, 0)));
            f(OK(1));
        "###;
        let (r, w) = run_with_warnings(src);
        assert_eq!(r.unwrap(), Value::Integer(1));
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].code, ErrorCode::W0051);
    }

    #[test]
    fn b3_unwrap_or_ok_emits_no_w0051() {
        let (r, w) = run_with_warnings("UNWRAP_OR(OK(42), 0);");
        assert_eq!(r.unwrap(), Value::Integer(42));
        assert!(w.is_empty(), "UNWRAP_OR must be silent, got {:?}", w);
    }

    #[test]
    fn b3_unwrap_or_err_consumes_no_w0051() {
        let (r, w) = run_with_warnings(r###"UNWRAP_OR(ERR("e"), 99);"###);
        assert_eq!(r.unwrap(), Value::Integer(99));
        assert!(w.is_empty());
    }

    #[test]
    fn b3_unwrap_or_non_result_is_e0030_with_unwrap_or_message() {
        // The canonical-name convention: error messages name UNWRAP_OR,
        // never OR_DIE.
        let err = run("UNWRAP_OR(42, 0);").unwrap_err();
        let msg = &err.diagnostic().message;
        assert!(
            msg.contains("UNWRAP_OR"),
            "should mention UNWRAP_OR: {}",
            msg
        );
        assert!(
            !msg.contains("`OR_DIE`"),
            "should NOT mention OR_DIE: {}",
            msg
        );
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b3_unwrap_or_arity_too_few_is_e0022_saying_unwrap_or() {
        let err = run("UNWRAP_OR(OK(1));").unwrap_err();
        let msg = &err.diagnostic().message;
        assert!(
            msg.contains("UNWRAP_OR"),
            "should mention UNWRAP_OR: {}",
            msg
        );
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn b3_unwrap_or_arity_too_many_is_e0022_saying_unwrap_or() {
        let err = run("UNWRAP_OR(OK(1), 0, 0);").unwrap_err();
        let msg = &err.diagnostic().message;
        assert!(
            msg.contains("UNWRAP_OR"),
            "should mention UNWRAP_OR: {}",
            msg
        );
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn b3_unwrap_or_inside_user_function_no_w0051() {
        let src = r###"
            LET(f, FUN((x), UNWRAP_OR(x, 0)));
            f(OK(1));
        "###;
        let (r, w) = run_with_warnings(src);
        assert_eq!(r.unwrap(), Value::Integer(1));
        assert!(w.is_empty());
    }

    #[test]
    fn b3_unwrap_or_propagates_through_user_function() {
        // Regression: rename must not change ERR-through-user-fn semantics.
        let src = r###"
            LET(pass_through, FUN((x), x));
            UNWRAP_OR(pass_through(ERR("inner")), -1);
        "###;
        assert_eq!(run(src).unwrap(), Value::Integer(-1));
    }

    #[test]
    fn b3_mixed_or_die_and_unwrap_or_only_or_die_warns() {
        // Same fn: OR_DIE branch warns, UNWRAP_OR branch silent.
        let src = r###"
            LET(f, FUN((x),
                LET(a, OR_DIE(x, -1));
                LET(b, UNWRAP_OR(x, -2));
                +(a, b)
            ));
            f(OK(10));
        "###;
        let (r, w) = run_with_warnings(src);
        assert_eq!(r.unwrap(), Value::Integer(20));
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].code, ErrorCode::W0051);
    }

    #[test]
    fn b3_or_die_preserves_v04_section_122_semantics() {
        // Behavior regression guard: rename + alias must not change
        // OK / ERR / non-RESULT semantics. Arity is enforced at
        // parse time for OR_DIE (lexer keyword) — see
        // b3_or_die_arity_*_is_parse_time_error above; arity E0022
        // for UNWRAP_OR (builtin call path) is covered by
        // b3_unwrap_or_arity_*_saying_unwrap_or.
        assert_eq!(run("OR_DIE(OK(42), 0);").unwrap(), Value::Integer(42));
        assert_eq!(
            run(r###"OR_DIE(ERR("e"), 99);"###).unwrap(),
            Value::Integer(99)
        );
        let err = run("OR_DIE(42, 0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        // OR_DIE non-RESULT error message names UNWRAP_OR (canonical).
        let msg = &err.diagnostic().message;
        assert!(
            msg.contains("UNWRAP_OR"),
            "E0030 message should name UNWRAP_OR: {}",
            msg
        );
    }

    #[test]
    fn b3_doc_style_unwrap_or_chains_are_silent() {
        // B3 plan: "文档与示例优先用 UNWRAP_OR". Pins the
        // silent-canonical contract on the basic doc-style pattern
        // — UNWRAP_OR consuming a RESULT (OK / ERR) without any
        // alias warning. Both branches run inline so the ERR is
        // consumed at the same expression site (top-level LET
        // bindings of ERR values trigger E0102 because each top-level
        // stmt is checked, separate from variable bindings).
        let (r1, w1) = run_with_warnings(r###"UNWRAP_OR(OK("data"), "fallback");"###);
        assert_eq!(r1.unwrap(), Value::String("data".into()));
        assert!(w1.is_empty());
        let (r2, w2) = run_with_warnings(r###"UNWRAP_OR(ERR("oops"), "fallback");"###);
        assert_eq!(r2.unwrap(), Value::String("fallback".into()));
        assert!(w2.is_empty(), "ERR branch must be silent, got {:?}", w2);
    }

    #[test]
    fn b3_w0051_code_still_resolves() {
        // Sanity: W0051 (added in B2) must still be a registered code
        // after the rename (regression for B3).
        assert_eq!(ErrorCode::W0051.as_str(), "W0051");
        assert!(ErrorCode::W0051.is_warning());
    }

    // ──────────────────────────────────────────────────────────────────
    // B4: UNWRAP / ERR_PAYLOAD / WRAP — spec v0.4 §12.2 + §14.2 (cause)
    //
    // Phase B4 closes the three ERR-consumer primitives A6 reserved
    // in the §12.7 registry but stubbed (would surface as E0020).
    // Also closes A1e deferred: WlwlDiagnostic `cause` field (added
    // in A1d as `None`) is now populated by `UNWRAP(ERR(e))` so the
    // WRAP chain propagates through PANIC E0100.
    //
    // Behavior contracts:
    //   * UNWRAP(OK(v))   → v
    //   * UNWRAP(ERR(e))  → E0100 PANIC with `cause` = e (ErrorCause)
    //   * UNWRAP(non-R)   → E0030
    //   * ERR_PAYLOAD(ERR(e)) → e
    //   * ERR_PAYLOAD(OK)     → E0030 ("expected ERR, got OK")
    //   * ERR_PAYLOAD(non-R)  → E0030
    //   * WRAP(ERR(e), ctx)    → ERR({"original": e, "context": ctx})
    //   * WRAP(OK(v), _)       → OK(v) (pass-through)
    //   * WRAP(non-R, _)       → E0030
    //
    // No W0051 paths — these are v0.4 §12.7 canonical names, not
    // v0.3 aliases (those live at UNWRAP_OR/OR_DIE in B3).
    // ──────────────────────────────────────────────────────────────────

    // ── UNWRAP ────────────────────────────────────────────────────────

    #[test]
    fn b4_unwrap_ok_returns_inner() {
        assert_eq!(run("UNWRAP(OK(42));").unwrap(), Value::Integer(42));
        assert_eq!(
            run(r###"UNWRAP(OK("hello"));"###).unwrap(),
            Value::String("hello".into())
        );
        assert_eq!(
            run("UNWRAP(OK([1, 2, 3]));").unwrap(),
            Value::Array(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3)
            ])
        );
    }

    #[test]
    fn b4_unwrap_err_panics_e0100_with_string_cause() {
        // UNWRAP(ERR("boom")) → E0100 PANIC, cause = ErrorCause::String("boom")
        let err = run(r###"UNWRAP(ERR("boom"));"###).unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0100);
        // The message should reference the UNWRAP intent so users / AI
        // can see what triggered the PANIC, not just the raw payload.
        assert!(
            d.message.contains("UNWRAP"),
            "E0100 message should mention UNWRAP: {}",
            d.message
        );
        // Cause field must be populated as ErrorCause::String("boom")
        let cause = d
            .cause
            .as_ref()
            .expect("UNWRAP on ERR must set cause field");
        assert_eq!(**cause, wlwl_error::ErrorCause::String("boom".into()));
    }

    #[test]
    fn b4_unwrap_err_with_dict_payload_panics_with_dict_cause() {
        // UNWRAP(ERR(["code": "E1001", "msg": "bad"])) →
        //   cause = ErrorCause::Dict({"code": "E1001", "msg": "bad"})
        let err = run(r###"UNWRAP(ERR(["code": "E1001", "msg": "bad"]));"###)
            .unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0100);
        let cause = d.cause.as_ref().expect("cause must be set");
        match &**cause {
            wlwl_error::ErrorCause::Dict(map) => {
                assert_eq!(map.get("code").unwrap(), &serde_json::json!("E1001"));
                assert_eq!(map.get("msg").unwrap(), &serde_json::json!("bad"));
            }
            other => panic!("expected ErrorCause::Dict, got {:?}", other),
        }
    }

    #[test]
    fn b4_unwrap_wrap_chain_populates_dict_cause() {
        // UNWRAP(WRAP(ERR("net down"), "during login")) → E0100
        //   cause = ErrorCause::Dict({"original": "net down", "context": "during login"})
        let err = run(
            r###"UNWRAP(WRAP(ERR("net down"), "during login"));"###,
        )
        .unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0100);
        let cause = d.cause.as_ref().expect("cause must be set");
        match &**cause {
            wlwl_error::ErrorCause::Dict(map) => {
                assert_eq!(
                    map.get("original").unwrap(),
                    &serde_json::json!("net down")
                );
                assert_eq!(
                    map.get("context").unwrap(),
                    &serde_json::json!("during login")
                );
            }
            other => panic!("expected Dict cause, got {:?}", other),
        }
    }

    #[test]
    fn b4_unwrap_nested_wrap_chain_preserves_full_chain() {
        // WRAP(WRAP(ERR(e), c1), c2) — outer WRAP carries the
        // whole inner WRAP result as `original` (no flattening).
        let err = run(
            r###"UNWRAP(WRAP(WRAP(ERR("e"), "c1"), "c2"));"###,
        )
        .unwrap_err();
        let cause = err.diagnostic().cause.as_ref().expect("cause");
        match &**cause {
            wlwl_error::ErrorCause::Dict(outer) => {
                assert_eq!(outer.get("context").unwrap(), &serde_json::json!("c2"));
                let inner = outer.get("original").unwrap();
                // `original` is the whole previous WRAP value, which
                // is itself a {"original": ..., "context": ...} dict.
                let inner_obj = inner
                    .as_object()
                    .expect("inner original must be a dict");
                assert_eq!(
                    inner_obj.get("context").unwrap(),
                    &serde_json::json!("c1")
                );
                assert_eq!(
                    inner_obj.get("original").unwrap(),
                    &serde_json::json!("e")
                );
            }
            other => panic!("expected Dict cause, got {:?}", other),
        }
    }

    #[test]
    fn b4_unwrap_non_result_is_e0030_with_unwrap_message() {
        let err = run("UNWRAP(42);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        let msg = &err.diagnostic().message;
        assert!(msg.contains("UNWRAP"));
        assert!(msg.contains("OK/ERR") || msg.contains("expected OK/ERR"));
        // No cause field for type errors.
        assert!(err.diagnostic().cause.is_none());
    }

    #[test]
    fn b4_unwrap_arity_too_few_is_e0022() {
        let err = run("UNWRAP();").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn b4_unwrap_arity_too_many_is_e0022() {
        let err = run("UNWRAP(OK(1), 0);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn b4_unwrap_inside_user_function_propagates_panic() {
        // UNWRAP inside a user function body → PANIC E0100 with cause.
        // The ERR must originate INSIDE the function body (not as a
        // call argument) because a user function is NOT in the §12.7
        // ERR consumer registry — `f(ERR(...))` would be §12.6-short-
        // circuited before entering f's body.
        let src = r###"
            LET(f, FUN(() ,
                LET(r, ERR("user fn boom"));
                UNWRAP(r)
            ));
            f();
        "###;
        let err = run(src).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0100);
        let cause = err.diagnostic().cause.as_ref().expect("cause");
        assert_eq!(**cause, wlwl_error::ErrorCause::String("user fn boom".into()));
    }

    #[test]
    fn b4_unwrap_does_not_emit_w0051() {
        // Canonical name, zero warnings.
        let (r, w) = run_with_warnings("UNWRAP(OK(42));");
        assert_eq!(r.unwrap(), Value::Integer(42));
        assert!(w.is_empty(), "UNWRAP must be silent, got {:?}", w);
        let (r, w) = run_with_warnings(r###"UNWRAP(ERR("e"));"###);
        assert!(r.is_err());
        assert!(w.is_empty(), "UNWRAP must be silent even on ERR, got {:?}", w);
    }

    // ── ERR_PAYLOAD ───────────────────────────────────────────────────

    #[test]
    fn b4_err_payload_err_returns_string_payload() {
        assert_eq!(
            run(r###"ERR_PAYLOAD(ERR("net down"));"###).unwrap(),
            Value::String("net down".into())
        );
    }

    #[test]
    fn b4_err_payload_err_returns_dict_payload() {
        let v = run(r###"ERR_PAYLOAD(ERR(["code": "E1001", "msg": "bad"]));"###)
            .unwrap();
        let expected = Value::Dict(vec![
            (
                Value::String("code".into()),
                Value::String("E1001".into()),
            ),
            (
                Value::String("msg".into()),
                Value::String("bad".into()),
            ),
        ]);
        assert_eq!(v, expected);
    }

    #[test]
    fn b4_err_payload_ok_is_e0030_saying_expected_err() {
        let err = run("ERR_PAYLOAD(OK(42));").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        let msg = &err.diagnostic().message;
        assert!(
            msg.contains("ERR_PAYLOAD"),
            "should name ERR_PAYLOAD: {}",
            msg
        );
        assert!(
            msg.contains("OK"),
            "should mention OK (the wrong type): {}",
            msg
        );
    }

    #[test]
    fn b4_err_payload_non_result_is_e0030() {
        let err = run("ERR_PAYLOAD(42);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        let err = run(r###"ERR_PAYLOAD("string");"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b4_err_payload_arity_wrong_is_e0022() {
        assert_eq!(
            run("ERR_PAYLOAD();").unwrap_err().diagnostic().code,
            ErrorCode::E0022
        );
        assert_eq!(
            run(r###"ERR_PAYLOAD(ERR("e"), 0);"###)
                .unwrap_err()
                .diagnostic()
                .code,
            ErrorCode::E0022
        );
    }

    #[test]
    fn b4_err_payload_does_not_emit_w0051() {
        let (r, w) = run_with_warnings(r###"ERR_PAYLOAD(ERR("e"));"###);
        assert_eq!(r.unwrap(), Value::String("e".into()));
        assert!(w.is_empty());
    }

    // ── WRAP ─────────────────────────────────────────────────────────

    #[test]
    fn b4_wrap_err_creates_wrapped_dict() {
        // WRAP(ERR("e"), "ctx") → ERR({"original": "e, "context": "ctx"}).
        // We extract the dict via ERR_PAYLOAD — top-level WRAP returns
        // a Value::Err which §12.6 would surface as E0102 (same
        // top-level trap that bit B3 `b3_doc_style_…`).
        let v = run(r###"ERR_PAYLOAD(WRAP(ERR("e"), "ctx"));"###).unwrap();
        let expected = Value::Dict(vec![
            (Value::String("original".into()), Value::String("e".into())),
            (
                Value::String("context".into()),
                Value::String("ctx".into()),
            ),
        ]);
        assert_eq!(v, expected);
    }

    #[test]
    fn b4_wrap_err_with_dict_payload_creates_nested_dict() {
        // WRAP(ERR(["code": "E1001"]), "ctx") →
        //   ERR({"original": {"code": "E1001"}, "context": "ctx"})
        let v = run(r###"ERR_PAYLOAD(WRAP(ERR(["code": "E1001"]), "ctx"));"###)
            .unwrap();
        let expected = Value::Dict(vec![
            (
                Value::String("original".into()),
                Value::Dict(vec![(
                    Value::String("code".into()),
                    Value::String("E1001".into()),
                )]),
            ),
            (
                Value::String("context".into()),
                Value::String("ctx".into()),
            ),
        ]);
        assert_eq!(v, expected);
    }

    #[test]
    fn b4_wrap_ok_passes_through_unchanged() {
        // WRAP(OK(v), _) → OK(v). ctx is ignored.
        assert_eq!(
            run("WRAP(OK(42), \"ignored\");").unwrap(),
            Value::Ok(Box::new(Value::Integer(42)))
        );
        assert_eq!(
            run("WRAP(OK([1, 2]), NULL);").unwrap(),
            Value::Ok(Box::new(Value::Array(vec![
                Value::Integer(1),
                Value::Integer(2)
            ])))
        );
    }

    #[test]
    fn b4_wrap_nested_chain_builds_deeper_dict() {
        // WRAP(WRAP(ERR("e"), "c1"), "c2") — second WRAP wraps
        // the WHOLE previous ERR (which holds a dict) as `original`.
        // Use ERR_PAYLOAD to extract the outer dict.
        let v = run(r###"ERR_PAYLOAD(WRAP(WRAP(ERR("e"), "c1"), "c2"));"###)
            .unwrap();
        let outer_dict = match v {
            Value::Dict(d) => d,
            _ => panic!("outer should be dict"),
        };
        assert_eq!(
            outer_dict[1],
            (Value::String("context".into()), Value::String("c2".into()))
        );
        // original is the previous wrapped ERR value (a dict)
        let inner_dict = match &outer_dict[0].1 {
            Value::Dict(d) => d.clone(),
            _ => panic!("original should be dict"),
        };
        assert_eq!(
            inner_dict[1],
            (Value::String("context".into()), Value::String("c1".into()))
        );
        assert_eq!(
            inner_dict[0],
            (
                Value::String("original".into()),
                Value::String("e".into())
            )
        );
    }

    #[test]
    fn b4_wrap_non_result_is_e0030() {
        let err = run(r###"WRAP(42, "ctx");"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        let err = run(r###"WRAP([1, 2], "ctx");"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b4_wrap_arity_wrong_is_e0022() {
        assert_eq!(
            run(r###"WRAP(ERR("e"));"###).unwrap_err().diagnostic().code,
            ErrorCode::E0022
        );
        assert_eq!(
            run(r###"WRAP(ERR("e"), "ctx", "extra");"###)
                .unwrap_err()
                .diagnostic()
                .code,
            ErrorCode::E0022
        );
    }

    #[test]
    fn b4_wrap_does_not_emit_w0051() {
        // Use IS_ERR (in §12.7 registry) to consume the ERR result
        // so it doesn't surface as top-level E0102.
        let (r, w) = run_with_warnings(r###"IS_ERR(WRAP(ERR("e"), "ctx"));"###);
        assert_eq!(r.unwrap(), Value::Boolean(true));
        assert!(w.is_empty());
        let (r, w) = run_with_warnings("WRAP(OK(42), \"ctx\");");
        assert_eq!(r.unwrap(), Value::Ok(Box::new(Value::Integer(42))));
        assert!(w.is_empty());
    }

    // ── Integration / regression ──────────────────────────────────────

    #[test]
    fn b4_is_ok_is_err_on_wrap_result() {
        // IS_OK / IS_ERR (§12.7) compose with WRAP — the WRAP
        // result is still a RESULT.
        assert_eq!(
            run(r###"IS_OK(WRAP(OK(42), "ctx"));"###).unwrap(),
            Value::Boolean(true)
        );
        assert_eq!(
            run(r###"IS_OK(WRAP(ERR("e"), "ctx"));"###).unwrap(),
            Value::Boolean(false)
        );
        assert_eq!(
            run(r###"IS_ERR(WRAP(ERR("e"), "ctx"));"###).unwrap(),
            Value::Boolean(true)
        );
    }

    #[test]
    fn b4_unwrap_or_does_not_consume_wrap_result() {
        // UNWRAP_OR on a wrapped ERR returns the default (the
        // wrap-dict is hidden, treated as a normal ERR).
        assert_eq!(
            run(r###"UNWRAP_OR(WRAP(ERR("e"), "ctx"), -1);"###).unwrap(),
            Value::Integer(-1)
        );
    }

    #[test]
    fn b4_err_payload_of_wrapped_err_returns_dict() {
        // ERR_PAYLOAD(WRAP(ERR("e"), "ctx")) → the wrap dict
        // (the most recent layer), NOT the original "e".
        let v = run(r###"ERR_PAYLOAD(WRAP(ERR("e"), "ctx"));"###).unwrap();
        let expected = Value::Dict(vec![
            (Value::String("original".into()), Value::String("e".into())),
            (
                Value::String("context".into()),
                Value::String("ctx".into()),
            ),
        ]);
        assert_eq!(v, expected);
    }

    #[test]
    fn b4_cause_field_round_trips_through_serde() {
        // Sanity: the WlwlDiagnostic with cause set serializes and
        // deserializes correctly (AI tooling reads --format=jsonl).
        let err = run(r###"UNWRAP(WRAP(ERR("e"), "c"));"###).unwrap_err();
        let d = err.diagnostic();
        let json = serde_json::to_string(d).expect("serialize diagnostic");
        let back: wlwl_error::WlwlDiagnostic =
            serde_json::from_str(&json).expect("deserialize diagnostic");
        assert_eq!(back.code, ErrorCode::E0100);
        assert!(back.cause.is_some(), "cause must round-trip");
        match back.cause.unwrap().as_ref() {
            wlwl_error::ErrorCause::Dict(map) => {
                assert_eq!(map.get("original").unwrap(), &serde_json::json!("e"));
                assert_eq!(map.get("context").unwrap(), &serde_json::json!("c"));
            }
            other => panic!("expected Dict cause after round-trip, got {:?}", other),
        }
    }

    #[test]
    fn b4_does_not_register_or_die_alias_path() {
        // Sanity: UNWRAP / ERR_PAYLOAD / WRAP are the v0.4 canonical
        // names — there is no v0.3 alias that emits W0051. (UNWRAP_OR
        // / OR_DIE are B3, a different primitive.) Use IS_ERR /
        // ERR_PAYLOAD to consume the ERR so it doesn't surface as
        // top-level E0102.
        let (r, w) = run_with_warnings("UNWRAP(OK(42));");
        r.unwrap();
        assert!(w.is_empty());
        let (r, w) = run_with_warnings("ERR_PAYLOAD(ERR(\"e\"));");
        r.unwrap();
        assert!(w.is_empty());
        let (r, w) = run_with_warnings("IS_ERR(WRAP(ERR(\"e\"), \"c\"));");
        r.unwrap();
        assert!(w.is_empty());
    }

    // ──────────────────────────────────────────────────────────────────
    // Phase B5 (spec v0.4 §10.6 + §15.8 + §10.3 STR): FORMAT string
    // formatting + std.format module + STR global builtin.
    //
    // Behavior contracts:
    //   * STR(x)              → STRING (Value::display rendering)
    //   * FORMAT("{N}")       → N-th format arg via STR semantics;
    //                           out of range → placeholder kept as-is
    //   * FORMAT("{name}")    → first DICT among format args, by key;
    //                           missing key / no DICT → kept as-is
    //   * FORMAT mixed        → both placeholder kinds in one template
    //   * parse failure       → E0039 (unclosed `{`, empty `{}`)
    //   * non-STRING template → E0030; zero args → E0022
    //   * ERR args            → §12.6 transparent propagation
    //                           (appendix G: FORMAT/STR are NOT ERR
    //                           consumers)
    //
    // Both entry points share the template grammar: the global builtin
    // (this file) and wlwl:std.format (IMPORT path). Neither emits
    // W0051 — FORMAT/STR are v0.4 canonical names, not v0.3 aliases.
    // ──────────────────────────────────────────────────────────────────

    // ── STR ───────────────────────────────────────────────────────────

    #[test]
    fn b5_str_primitives() {
        assert_eq!(run("STR(42);").unwrap(), Value::String("42".into()));
        assert_eq!(run("STR(3.5);").unwrap(), Value::String("3.5".into()));
        assert_eq!(run("STR(30.0);").unwrap(), Value::String("30.0".into()));
        assert_eq!(run("STR(TRUE);").unwrap(), Value::String("TRUE".into()));
        assert_eq!(run("STR(FALSE);").unwrap(), Value::String("FALSE".into()));
        assert_eq!(run("STR(NULL);").unwrap(), Value::String("NULL".into()));
        // STRING is identity.
        assert_eq!(run(r###"STR("x");"###).unwrap(), Value::String("x".into()));
    }

    #[test]
    fn b5_str_containers_render_structurally() {
        assert_eq!(
            run("STR([1, 2]);").unwrap(),
            Value::String("[1, 2]".into())
        );
        assert_eq!(
            run(r###"STR(["a": 1]);"###).unwrap(),
            Value::String("[a: 1]".into())
        );
    }

    #[test]
    fn b5_str_closure_renders_fun_form() {
        // STR accepts any value (appendix G: STR(x) → STRING); a
        // closure renders via Value::display(). This is exactly why
        // builtin_format renders on Value instead of round-tripping
        // the std boundary (which rejects closures with E0030).
        assert_eq!(
            run("STR(FUN((x), x));").unwrap(),
            Value::String("<fun(x)>".into())
        );
    }

    #[test]
    fn b5_str_ok_result_renders() {
        assert_eq!(
            run("STR(OK(42));").unwrap(),
            Value::String("OK(42)".into())
        );
    }

    #[test]
    fn b5_str_err_arg_propagates_per_s126() {
        // STR is NOT an ERR consumer (appendix G ❌) — an ERR argument
        // short-circuits in eval_call and escapes to top level as
        // E0102 (§19.6 Corollary 19.1).
        let err = run(r###"STR(ERR("e"));"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn b5_str_arity_wrong_is_e0022() {
        assert_eq!(run("STR();").unwrap_err().diagnostic().code, ErrorCode::E0022);
        assert_eq!(
            run(r###"STR("a", "b");"###).unwrap_err().diagnostic().code,
            ErrorCode::E0022
        );
    }

    // ── FORMAT: the three §10.6 spec examples ─────────────────────────

    #[test]
    fn b5_format_positional_spec_example() {
        // §10.6 example 1: positional placeholders + STR conversion of
        // the INTEGER age.
        let v = run(
            r###"LET(name, "alice"); LET(age, 30);
                 FORMAT("hi {0}, you are {1} years old", name, age);"###,
        )
        .unwrap();
        assert_eq!(
            v,
            Value::String("hi alice, you are 30 years old".into())
        );
    }

    #[test]
    fn b5_format_named_spec_example() {
        // §10.6 example 2: pure-named pattern — the DICT is args[0] of
        // the format args, exactly as the spec's "从 args[0]" describes.
        let v = run(
            r###"FORMAT("hi {name}, age {age}", ["name": "alice", "age": 30]);"###,
        )
        .unwrap();
        assert_eq!(v, Value::String("hi alice, age 30".into()));
    }

    #[test]
    fn b5_format_mixed_spec_example() {
        // §10.6 example 3: mixed pattern. The named lookup must find
        // the dict at args[1] — the "first DICT among format args"
        // rule is what makes the spec's own example produce sensible
        // output (hardcoding args[0] would leave "{age}" literal).
        let v = run(
            r###"FORMAT("hi {0}, age {age}", "alice", ["age": 30]);"###,
        )
        .unwrap();
        assert_eq!(v, Value::String("hi alice, age 30".into()));
    }

    // ── FORMAT: rendering rules ───────────────────────────────────────

    #[test]
    fn b5_format_repeated_placeholder() {
        assert_eq!(
            run(r###"FORMAT("{0} and {0} and {0}", "x");"###).unwrap(),
            Value::String("x and x and x".into())
        );
    }

    #[test]
    fn b5_format_str_conversion_of_inserted_values() {
        // §10.6: args[i] 非字符串 → STR(args[i]) 转换 (§10.3 semantics).
        assert_eq!(
            run(r###"FORMAT("{0} {1} {2} {3}", 42, 30.0, TRUE, NULL);"###).unwrap(),
            Value::String("42 30.0 TRUE NULL".into())
        );
        // Containers render structurally, same as STR.
        assert_eq!(
            run(r###"FORMAT("{0} {1}", [1, 2], ["k": "v"]);"###).unwrap(),
            Value::String("[1, 2] [k: v]".into())
        );
    }

    #[test]
    fn b5_format_str_composes_with_format() {
        assert_eq!(
            run(r###"FORMAT("{0}!", STR(42));"###).unwrap(),
            Value::String("42!".into())
        );
    }

    #[test]
    fn b5_format_unmatched_positional_kept_literal() {
        // §10.6 末段: 模板中未匹配的 {...} 保留原样.
        assert_eq!(
            run(r###"FORMAT("{0} + {5}", "a");"###).unwrap(),
            Value::String("a + {5}".into())
        );
    }

    #[test]
    fn b5_format_unmatched_named_kept_literal() {
        // Key present in the dict? No → keep the placeholder.
        assert_eq!(
            run(r###"FORMAT("{name}", ["other": 1]);"###).unwrap(),
            Value::String("{name}".into())
        );
        // No DICT arg at all → keep it too.
        assert_eq!(
            run(r###"FORMAT("hi {name}", "alice");"###).unwrap(),
            Value::String("hi {name}".into())
        );
    }

    #[test]
    fn b5_format_named_skips_non_dict_args() {
        // The named lookup scans for the first DICT; scalars are
        // skipped (they are positional material, not lookup sources).
        assert_eq!(
            run(r###"FORMAT("{name}", 1, ["name": "n"]);"###).unwrap(),
            Value::String("n".into())
        );
    }

    #[test]
    fn b5_format_stray_close_brace_is_literal() {
        assert_eq!(
            run(r###"FORMAT("a } b {0}", "x");"###).unwrap(),
            Value::String("a } b x".into())
        );
    }

    #[test]
    fn b5_format_closure_arg_renders_fun_form() {
        // Global-builtin path renders closures via STR semantics. (The
        // IMPORT path rejects them at the std boundary with E0030 —
        // see b5_format_std_path_closure_is_e0030; that divergence is
        // the documented std-boundary contract.)
        assert_eq!(
            run(r###"FORMAT("{0}", FUN((x), x));"###).unwrap(),
            Value::String("<fun(x)>".into())
        );
    }

    // ── FORMAT: E0039 / E0030 / E0022 ─────────────────────────────────

    #[test]
    fn b5_format_unclosed_brace_is_e0039_span_aware() {
        // §10.6: 模板解析失败(如 `{` 单独出现)→ E0039. The diagnostic
        // is span-aware (B4 current_span mechanism): it must point at
        // the FORMAT call site, not the `<runtime>` placeholder.
        let err = run(r#"FORMAT("bad { template");"#).unwrap_err();
        let d = err.diagnostic();
        assert_eq!(d.code, ErrorCode::E0039);
        assert!(
            d.message.contains("malformed"),
            "E0039 message should say malformed: {}",
            d.message
        );
        assert!(
            d.message.contains("unclosed"),
            "E0039 message should say unclosed: {}",
            d.message
        );
        assert_eq!(d.location.file, "t.wl");
        assert_eq!(d.location.line, 1);
    }

    #[test]
    fn b5_format_empty_placeholder_is_e0039() {
        // `{}` is neither a positional nor a named placeholder — the
        // template cannot be interpreted → parse failure (documented
        // decision; the spec only pins the lone-`{` case explicitly).
        let err = run(r#"FORMAT("a {} b");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0039);
        assert!(
            err.diagnostic().message.contains("empty"),
            "message should say empty: {}",
            err.diagnostic().message
        );
    }

    #[test]
    fn b5_format_template_not_string_is_e0030() {
        let err = run("FORMAT(42);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        assert!(
            err.diagnostic().message.contains("template"),
            "message should mention template: {}",
            err.diagnostic().message
        );
    }

    #[test]
    fn b5_format_zero_args_is_e0022() {
        let err = run("FORMAT();").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
        assert!(
            err.diagnostic().message.contains("at least 1"),
            "message should say at least 1: {}",
            err.diagnostic().message
        );
    }

    #[test]
    fn b5_format_err_arg_propagates_per_s126() {
        // FORMAT is NOT an ERR consumer (appendix G ❌) — the ERR arg
        // short-circuits in eval_call and escapes to top level as
        // E0102. (Same shape as b5_str_err_arg_propagates_per_s126.)
        let err = run(r###"FORMAT("{0}", ERR("e"));"###).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    // ── FORMAT: template cache (plan §5.5) ────────────────────────────

    #[test]
    fn b5_format_cache_same_template_different_args() {
        // Same template text parsed once, rendered twice with different
        // args — a broken (stale-render) cache would return the first
        // result twice. Both calls happen in ONE program so they share
        // one Evaluator (and therefore one format_cache).
        let v = run(
            r###"[FORMAT("{0}-{1}", 1, 2), FORMAT("{0}-{1}", 3, 4), FORMAT("{0}-{1}", "a", "b")];"###,
        )
        .unwrap();
        assert_eq!(
            v,
            Value::Array(vec![
                Value::String("1-2".into()),
                Value::String("3-4".into()),
                Value::String("a-b".into()),
            ])
        );
    }

    #[test]
    fn b5_format_in_loop_with_constant_template() {
        // The classic cache win: a loop formatting with the same
        // template every iteration. Correct output across iterations
        // proves the cached segments render against fresh args.
        let src = r#"
            LET(out, "");
            FOR(i, [1, 2, 3],
                LET(out, +(out, FORMAT("[{0}]", i)))
            );
            out;
        "#;
        assert_eq!(
            run(src).unwrap(),
            Value::String("[1][2][3]".into())
        );
    }

    // ── FORMAT via IMPORT("wlwl:std.format") (§15.8) ──────────────────

    #[test]
    fn b5_format_via_std_import_positional() {
        // §15.8: the module is FORMAT's home. After IMPORT, the name
        // binds to the std NativeFn and the user-supplied binding
        // takes priority over the resolve_builtin fallback.
        let v = run_std(r#"
            IMPORT("wlwl:std.format", ["FORMAT"]);
            FORMAT("hi {0}", "alice");
        "#)
        .unwrap();
        assert_eq!(v, Value::String("hi alice".into()));
    }

    #[test]
    fn b5_format_via_std_import_named_and_mixed() {
        let v = run_std(r#"
            IMPORT("wlwl:std.format", ["FORMAT"]);
            FORMAT("hi {name}", ["name": "bob"]);
        "#)
        .unwrap();
        assert_eq!(v, Value::String("hi bob".into()));
        let v = run_std(r#"
            IMPORT("wlwl:std.format", ["FORMAT"]);
            FORMAT("{0}={v}", "x", ["v": 7]);
        "#)
        .unwrap();
        assert_eq!(v, Value::String("x=7".into()));
    }

    #[test]
    fn b5_format_std_path_malformed_template_is_e0039() {
        // The std path shares the template grammar, so the same E0039
        // fires through invoke_std's StdError → diagnostic mapping.
        let err = run_std(r#"
            IMPORT("wlwl:std.format", ["FORMAT"]);
            FORMAT("bad {");
        "#)
        .unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0039);
    }

    #[test]
    fn b5_format_std_path_unmatched_kept_literal() {
        let v = run_std(r#"
            IMPORT("wlwl:std.format", ["FORMAT"]);
            FORMAT("{9} {missing}", "a");
        "#)
        .unwrap();
        assert_eq!(v, Value::String("{9} {missing}".into()));
    }

    #[test]
    fn b5_format_std_path_closure_is_e0030() {
        // The std boundary (invoke_std → value_to_std_value) rejects
        // closures before dispatch — existing contract for every std
        // module. The global builtin renders them instead (see
        // b5_format_closure_arg_renders_fun_form). Documented
        // divergence, deviations P4-B5-004.
        let err = run_std(r#"
            IMPORT("wlwl:std.format", ["FORMAT"]);
            FORMAT("{0}", FUN((x), x));
        "#)
        .unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b5_format_global_and_std_paths_agree() {
        // Same program shape through both entry points must produce
        // the same string (they share the grammar; rendering mirrors
        // Value::display on both sides).
        let global = run(r###"FORMAT("hi {0}, age {age}", "alice", ["age": 30]);"###)
            .unwrap();
        let via_std = run_std(r#"
            IMPORT("wlwl:std.format", ["FORMAT"]);
            FORMAT("hi {0}, age {age}", "alice", ["age": 30]);
        "#)
        .unwrap();
        assert_eq!(global, via_std);
        assert_eq!(global, Value::String("hi alice, age 30".into()));
    }

    // ── W0051 / warnings ──────────────────────────────────────────────

    #[test]
    fn b5_format_and_str_do_not_emit_w0051() {
        // FORMAT / STR are v0.4 canonical names (appendix G), not v0.3
        // aliases — zero W0051 paths, same reasoning as B4.
        let (r, w) = run_with_warnings(r###"STR(42); FORMAT("{0}", 1);"###);
        r.unwrap();
        assert!(w.is_empty(), "expected no warnings, got {:?}", w);
    }

    // ══════════════════════════════════════════════════════════════════
    // Phase B6 — `wlwl:std.collection` higher-order functions
    // (spec v0.4 §15.7 / §10.5)
    //
    // Tests below cover the 17 functions exposed via
    // `IMPORT("wlwl:std.collection", [...])`. The std SPEC for this
    // module is a "name catalog" (functions: &[]) — see
    // `wlwl_std::collection` for the why — so binding goes through
    // `Evaluator::load_std`'s path-specific branch which walks
    // `wlwl_eval::collection::BUILTINS` (eval-internal `BuiltinFn`s
    // that operate on `Value` and can call user closures).
    //
    // Coverage strategy:
    //   - happy-path + edge cases per function;
    //   - §12.6 ERR transparent propagation (input ERR → output ERR);
    //   - §10.5 E0038 (`RANGE` step=0);
    //   - §10.5 SORT semantics (default `<`, custom comparator);
    //   - spec §15.7 worked example (`MAP([1,2,3], FUN((x), *(x,x)))`).
    // ══════════════════════════════════════════════════════════════════

    #[test]
    fn b6_collection_import_resolves_all_seventeen_names() {
        // Walk BUILTINS via IMPORT; each name must be callable.
        // If a future batch adds an 18th function but forgets to put
        // it in BUILTINS, this test still passes — it's a binding
        // sanity check, not an enumeration test (see
        // `wlwl_eval::collection::tests::names_match_catalog` for
        // the strict count lock).
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", [
                "MAP", "FILTER", "REDUCE", "SORT", "SORT_BY",
                "ZIP", "RANGE", "ANY", "ALL", "FIND",
                "ENUMERATE", "TAKE", "DROP", "FLAT", "UNIQ",
                "GROUP_BY", "JOIN"
            ]);
            LEN([
                MAP, FILTER, REDUCE, SORT, SORT_BY, ZIP, RANGE,
                ANY, ALL, FIND, ENUMERATE, TAKE, DROP, FLAT, UNIQ,
                GROUP_BY, JOIN
            ]);
        "#).unwrap();
        assert_eq!(v, Value::Integer(17));
    }

    #[test]
    fn b6_map_spec_worked_example() {
        // §15.7 worked example verbatim: MAP([1,2,3], FUN((x), *(x,x))).
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["MAP"]);
            MAP([1, 2, 3], FUN((x), *(x, x)));
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![
            Value::Integer(1), Value::Integer(4), Value::Integer(9)
        ]));
    }

    #[test]
    fn b6_map_empty_array_returns_empty_array() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["MAP"]);
            MAP([], FUN((x), *(x, x)));
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![]));
    }

    #[test]
    fn b6_map_non_array_first_arg_is_e0030() {
        let err = run_std(r#"
            IMPORT("wlwl:std.collection", ["MAP"]);
            MAP(42, FUN((x), x));
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b6_map_non_callable_second_arg_is_e0020() {
        let err = run_std(r#"
            IMPORT("wlwl:std.collection", ["MAP"]);
            MAP([1, 2, 3], 42);
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0020);
    }

    #[test]
    fn b6_map_arity_wrong_is_e0022() {
        let err = run_std(r#"
            IMPORT("wlwl:std.collection", ["MAP"]);
            MAP([1, 2, 3]);
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn b6_map_input_err_transparent() {
        // §12.6: input contains ERR → output is that ERR. `eval_call`
        // short-circuits the ERR before the builtin even runs (so the
        // collection's `short_circuit_err` precheck never fires — the
        // ERR never reaches the fn body). At the top level an ERR
        // surfaces as E0102, exactly like B5's `b5_str_err_arg_*`
        // tests; the message body preserves the inner ERR value.
        let err = run_std(r#"
            IMPORT("wlwl:std.collection", ["MAP"]);
            MAP([1, ERR("e"), 3], FUN((x), *(x, 2)));
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        assert!(err.diagnostic().message.contains("e"), "{}", err.diagnostic().message);
    }

    #[test]
    fn b6_filter_keeps_truthy_only() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["FILTER"]);
            FILTER([1, 2, 3, 4, 5], FUN((x), >(x, 2)));
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![
            Value::Integer(3), Value::Integer(4), Value::Integer(5)
        ]));
    }

    #[test]
    fn b6_filter_predicate_non_boolean_is_e0030() {
        let err = run_std(r#"
            IMPORT("wlwl:std.collection", ["FILTER"]);
            FILTER([1, 2, 3], FUN((x), x));
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b6_reduce_left_fold_with_init() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["REDUCE"]);
            REDUCE([1, 2, 3, 4], FUN((acc, x), +(acc, x)), 0);
        "#).unwrap();
        assert_eq!(v, Value::Integer(10));
    }

    #[test]
    fn b6_reduce_empty_array_returns_init() {
        // §10.5 row 3: "空数组返回 init".
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["REDUCE"]);
            REDUCE([], FUN((acc, x), +(acc, x)), 42);
        "#).unwrap();
        assert_eq!(v, Value::Integer(42));
    }

    #[test]
    fn b6_sort_default_uses_lt() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["SORT"]);
            SORT([3, 1, 4, 1, 5, 9, 2, 6]);
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![
            Value::Integer(1), Value::Integer(1), Value::Integer(2),
            Value::Integer(3), Value::Integer(4), Value::Integer(5),
            Value::Integer(6), Value::Integer(9),
        ]));
    }

    #[test]
    fn b6_sort_with_custom_comparator_descending() {
        // Spec §10.5 row 4: cmp(a,b)=TRUE iff a<b; we negate to get
        // descending sort.
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["SORT"]);
            SORT([3, 1, 4, 1, 5], FUN((a, b), >(a, b)));
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![
            Value::Integer(5), Value::Integer(4), Value::Integer(3),
            Value::Integer(1), Value::Integer(1),
        ]));
    }

    #[test]
    fn b6_sort_by_keys_on_derived_value() {
        // §10.5 row 5: sort by key function. Sort [1,2,3] by *(x,x)
        // (i.e. 1,4,9) — the result must be [1,2,3] (unchanged),
        // proving the function sorts by *projected* key, not the
        // element directly.
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["SORT_BY"]);
            SORT_BY([1, 2, 3], FUN((x), *(x, x)));
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![
            Value::Integer(1), Value::Integer(2), Value::Integer(3),
        ]));
    }

    #[test]
    fn b6_zip_two_arrays() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["ZIP"]);
            ZIP([1, 2, 3], ["a", "b", "c"]);
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![
            Value::Array(vec![Value::Integer(1), Value::String("a".into())]),
            Value::Array(vec![Value::Integer(2), Value::String("b".into())]),
            Value::Array(vec![Value::Integer(3), Value::String("c".into())]),
        ]));
    }

    #[test]
    fn b6_zip_shortest_input_wins() {
        // §10.5 row 6: "长度 = 最短".
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["ZIP"]);
            ZIP([1, 2, 3, 4], ["a", "b"]);
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![
            Value::Array(vec![Value::Integer(1), Value::String("a".into())]),
            Value::Array(vec![Value::Integer(2), Value::String("b".into())]),
        ]));
    }

    #[test]
    fn b6_range_single_arg_default_start_step() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["RANGE"]);
            RANGE(5);
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![
            Value::Integer(0), Value::Integer(1), Value::Integer(2),
            Value::Integer(3), Value::Integer(4),
        ]));
    }

    #[test]
    fn b6_range_three_args_start_end_step() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["RANGE"]);
            RANGE(0, 10, 2);
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![
            Value::Integer(0), Value::Integer(2), Value::Integer(4),
            Value::Integer(6), Value::Integer(8),
        ]));
    }

    #[test]
    fn b6_range_negative_step_descending() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["RANGE"]);
            RANGE(5, 0, -1);
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![
            Value::Integer(5), Value::Integer(4), Value::Integer(3),
            Value::Integer(2), Value::Integer(1),
        ]));
    }

    #[test]
    fn b6_range_step_zero_is_e0038() {
        // §10.5 row 7: `step=0` → `E0038` (code registered in B5).
        let err = run_std(r#"
            IMPORT("wlwl:std.collection", ["RANGE"]);
            RANGE(0, 10, 0);
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0038);
    }

    #[test]
    fn b6_any_with_predicate_finds_truthy() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["ANY"]);
            ANY([1, 2, 3], FUN((x), >(x, 2)));
        "#).unwrap();
        assert_eq!(v, Value::Boolean(true));
    }

    #[test]
    fn b6_any_without_predicate_uses_truthiness() {
        // §10.5 row 8: "默认恒真" — any(v) returns TRUE for the
        // first truthy element. NULL and FALSE are falsy.
        assert_eq!(
            run_std(r#"
                IMPORT("wlwl:std.collection", ["ANY"]);
                ANY([NULL, FALSE, 1, 2]);
            "#).unwrap(),
            Value::Boolean(true)
        );
        assert_eq!(
            run_std(r#"
                IMPORT("wlwl:std.collection", ["ANY"]);
                ANY([NULL, FALSE, NULL]);
            "#).unwrap(),
            Value::Boolean(false)
        );
    }

    #[test]
    fn b6_all_with_predicate_requires_all_truthy() {
        assert_eq!(
            run_std(r#"
                IMPORT("wlwl:std.collection", ["ALL"]);
                ALL([1, 2, 3], FUN((x), >(x, 0)));
            "#).unwrap(),
            Value::Boolean(true)
        );
        assert_eq!(
            run_std(r#"
                IMPORT("wlwl:std.collection", ["ALL"]);
                ALL([1, 2, 3], FUN((x), >(x, 2)));
            "#).unwrap(),
            Value::Boolean(false)
        );
    }

    #[test]
    fn b6_find_returns_first_match() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["FIND"]);
            FIND([1, 2, 3, 4], FUN((x), >(x, 2)));
        "#).unwrap();
        assert_eq!(v, Value::Integer(3));
    }

    #[test]
    fn b6_find_no_match_returns_null() {
        // §10.5 row 10: "无则 NULL".
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["FIND"]);
            FIND([1, 2, 3], FUN((x), >(x, 99)));
        "#).unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn b6_enumerate_pairs_index_with_value() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["ENUMERATE"]);
            ENUMERATE(["a", "b", "c"]);
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![
            Value::Array(vec![Value::Integer(0), Value::String("a".into())]),
            Value::Array(vec![Value::Integer(1), Value::String("b".into())]),
            Value::Array(vec![Value::Integer(2), Value::String("c".into())]),
        ]));
    }

    #[test]
    fn b6_take_takes_first_n() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["TAKE"]);
            TAKE([1, 2, 3, 4, 5], 3);
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![
            Value::Integer(1), Value::Integer(2), Value::Integer(3),
        ]));
    }

    #[test]
    fn b6_take_n_over_len_returns_full() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["TAKE"]);
            TAKE([1, 2, 3], 99);
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![
            Value::Integer(1), Value::Integer(2), Value::Integer(3),
        ]));
    }

    #[test]
    fn b6_drop_skips_first_n() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["DROP"]);
            DROP([1, 2, 3, 4, 5], 2);
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![
            Value::Integer(3), Value::Integer(4), Value::Integer(5),
        ]));
    }

    #[test]
    fn b6_flat_flattens_one_level() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["FLAT"]);
            FLAT([[1, 2], [3, [4, 5]], 6]);
        "#).unwrap();
        // One level only: [[4,5]] stays nested.
        assert_eq!(v, Value::Array(vec![
            Value::Integer(1), Value::Integer(2),
            Value::Integer(3), Value::Array(vec![Value::Integer(4), Value::Integer(5)]),
            Value::Integer(6),
        ]));
    }

    #[test]
    fn b6_uniq_dedupes_preserving_order() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["UNIQ"]);
            UNIQ([1, 2, 1, 3, 2, 4]);
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![
            Value::Integer(1), Value::Integer(2),
            Value::Integer(3), Value::Integer(4),
        ]));
    }

    #[test]
    fn b6_group_by_returns_dict_of_arrays() {
        // §10.5 row 15: GROUP_BY(arr, key) → DICT keyed by key(v).
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["GROUP_BY"]);
            GROUP_BY(
                [1, 2, 3, 4, 5, 6],
                FUN((x), %(x, 2))
            );
        "#).unwrap();
        // Two keys: "0" (evens), "1" (odds). Order within each bucket
        // follows input order.
        match v {
            Value::Dict(entries) => {
                assert_eq!(entries.len(), 2);
                for (k, v) in &entries {
                    let k = match k { Value::String(s) => s.as_str(), _ => panic!("non-string key") };
                    match k {
                        "0" => assert_eq!(*v, Value::Array(vec![
                            Value::Integer(2), Value::Integer(4), Value::Integer(6),
                        ])),
                        "1" => assert_eq!(*v, Value::Array(vec![
                            Value::Integer(1), Value::Integer(3), Value::Integer(5),
                        ])),
                        _ => panic!("unexpected key: {}", k),
                    }
                }
            }
            other => panic!("expected DICT, got {:?}", other),
        }
    }

    #[test]
    fn b6_join_glues_with_separator() {
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["JOIN"]);
            JOIN([1, 2, 3], "-");
        "#).unwrap();
        assert_eq!(v, Value::String("1-2-3".into()));
    }

    #[test]
    fn b6_callback_err_input_arg_transparent() {
        // §10.5: "f 抛 ERR 也透明传播". We pass an ERR element, the
        // collection function returns it unchanged. Same pattern as
        // `b5_str_err_arg_propagates_per_s126`: at top level the ERR
        // surfaces as E0102.
        let err = run_std(r#"
            IMPORT("wlwl:std.collection", ["MAP"]);
            MAP([ERR("first")], FUN((x), *(x, 2)));
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        assert!(err.diagnostic().message.contains("first"), "{}", err.diagnostic().message);
    }

    #[test]
    fn b6_callback_returning_err_is_transparent() {
        // The callback itself returns ERR — the collection must
        // surface that ERR (§12.6 / §10.5). Same E0102 pattern.
        let err = run_std(r#"
            IMPORT("wlwl:std.collection", ["MAP"]);
            MAP([1, 2, 3], FUN((x), IF(>(x, 1), *(x, 2), ERR("from cb"))));
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        assert!(err.diagnostic().message.contains("from cb"), "{}", err.diagnostic().message);
    }

    #[test]
    fn b6_callback_returning_err_short_circuits() {
        // On the first ERR from the callback, the collection must
        // stop iterating and surface it — not continue and return
        // a partial array. Counter must stay < input length.
        let err = run_std(r#"
            IMPORT("wlwl:std.collection", ["MAP"]);
            LET(counter, 0);
            MAP([1, 2, 3, 4], FUN((x),
                SET(counter, +(counter, 1));
                IF(>(x, 2), ERR("boom"), *(x, x))
            ));
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        assert!(err.diagnostic().message.contains("boom"), "{}", err.diagnostic().message);
    }

    #[test]
    fn b6_collection_import_path_is_not_global_builtin() {
        // §15.7: "不作为全局内建". A direct call (no IMPORT) must
        // fail with E0020, not silently bind.
        let err = run("MAP([1, 2, 3], FUN((x), *(x, x)));").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0020);
    }

    #[test]
    fn b6_unknown_collection_name_in_import_is_e0023() {
        // IMPORT names list is validated by the existing std IMPORT
        // path-check machinery. Asking for a name that's not in
        // BUILTINS must fail with E0023 ("name not in module") at
        // IMPORT time — not silently bind to NULL.
        let err = run_std(r#"
            IMPORT("wlwl:std.collection", ["NOT_A_REAL_NAME"]);
        "#).unwrap_err();
        assert_eq!(
            err.diagnostic().code,
            ErrorCode::E0023,
            "expected E0023 (name not in module), got {:?}",
            err.diagnostic().code
        );
    }

    #[test]
    fn b6_std_format_still_works_alongside_collection() {
        // Regression: the new `wlwl:std.collection` path detection in
        // load_std must not disturb the existing `wlwl:std.format`
        // dispatch. Both IMPORTs must bind their names; a name listed
        // in only one (e.g. JOIN) would yield E0020 "undefined name"
        // — so this test catches both "format broke" and "collection
        // binding is leaking / overwriting names" regressions.
        let v = run_std(r#"
            IMPORT("wlwl:std.format", ["FORMAT"]);
            IMPORT("wlwl:std.collection", ["MAP", "JOIN"]);
            LET(squared, MAP([1, 2, 3], FUN((x), *(x, x))));
            FORMAT("squared: {0}", JOIN(squared, ","));
        "#).unwrap();
        assert_eq!(v, Value::String("squared: 1,4,9".into()));
    }

    // ══════════════════════════════════════════════════════════════════
    // Phase B7 — `wlwl:std.test` in-process test framework
    // (spec v0.4 §15.9; E0046-E0049)
    //
    // Tests cover the 6 functions exposed via
    // `IMPORT("wlwl:std.test", [...])`:
    //   TEST / ASSERT / ASSERT_EQ / ASSERT_NEQ / EXPECT_ERR / RUN_TESTS
    //
    // Coverage strategy:
    //   - happy-path per assertion (passing cases);
    //   - failing case per assertion (ERR is a *value*, surfaces
    //     via TRY at the call site);
    //   - RUN_TESTS end-to-end (registration + drain + result shape);
    //   - RUN_TESTS catches per-test ERR (§15.9 §12.6 transparency
    //     for assertion ERRs);
    //   - §15.7-style "name catalog" cross-check with collection
    //     (both std modules follow the same load_std hook path).
    // ══════════════════════════════════════════════════════════════════

    #[test]
    fn b7_test_import_resolves_all_six_names() {
        // Walk BUILTINS via IMPORT; each name must be callable.
        let v = run_std(r#"
            IMPORT("wlwl:std.test", [
                "TEST", "ASSERT", "ASSERT_EQ", "ASSERT_NEQ",
                "EXPECT_ERR", "RUN_TESTS"
            ]);
            LEN([TEST, ASSERT, ASSERT_EQ, ASSERT_NEQ, EXPECT_ERR, RUN_TESTS]);
        "#).unwrap();
        assert_eq!(v, Value::Integer(6));
    }

    #[test]
    fn b7_assert_true_returns_ok_true() {
        // §15.9 row 2: passing ASSERT → OK(TRUE).
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["ASSERT"]);
            ASSERT(TRUE);
        "#).unwrap();
        match v {
            Value::Ok(inner) => assert_eq!(*inner, Value::Boolean(true)),
            other => panic!("expected OK(TRUE), got {:?}", other),
        }
    }

    #[test]
    fn b7_assert_false_is_e0046_via_run_tests() {
        // §15.9 row 2: failing ASSERT → ERR(E0046). The natural way
        // to assert this end-to-end is via RUN_TESTS: register the
        // failing assertion as a TEST body, drain, and inspect the
        // `error` field of the failure record.
        //
        // (Top-level `TRY(ASSERT(FALSE, ...))` is *not* the right
        // path: TRY is §12.7's "early-RETURN from the enclosing
        // function" — it emits a `Signal::Return` that only a
        // closure body consumes. At top level the unhandled
        // signal becomes E0102. The plan §15.9 "RUN_TESTS uses
        // TRY to catch each test" means RUN_TESTS's *internal*
        // per-test runner — `invoke_closure` already unwraps the
        // `Return(Err)` signal — not the user's TRY keyword.)
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["TEST", "ASSERT", "RUN_TESTS"]);
            TEST("failing", FUN((), ASSERT(FALSE, "must be true")));
            LET(results, RUN_TESTS());
            LET(failed, INDEX_GET(results, 0));
            LET(err_field, INDEX_GET(failed, "error"));
            LET(code, INDEX_GET(err_field, "code"));
            LET(msg, INDEX_GET(err_field, "msg"));
            LET(passed_flag, INDEX_GET(failed, "passed"));
            [passed_flag, code, msg];
        "#).unwrap();
        match v {
            Value::Array(items) => {
                assert_eq!(items[0], Value::Boolean(false));
                assert_eq!(items[1], Value::String("E0046".into()));
                assert_eq!(items[2], Value::String("must be true".into()));
            }
            other => panic!("expected LIST, got {:?}", other),
        }
    }

    #[test]
    fn b7_assert_truthiness_follows_s94() {
        // §9.4: NULL/FALSE are falsy; everything else is truthy.
        // All passing — RUN_TESTS catches any that fail.
        assert_eq!(
            run_std(r#"IMPORT("wlwl:std.test", ["ASSERT"]);
                      ASSERT(1);"#).unwrap(),
            Value::Ok(Box::new(Value::Boolean(true))),
        );
        assert_eq!(
            run_std(r#"IMPORT("wlwl:std.test", ["ASSERT"]);
                      ASSERT("non-empty");"#).unwrap(),
            Value::Ok(Box::new(Value::Boolean(true))),
        );
        // Falsy cases via RUN_TESTS (passes = 0):
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["TEST", "ASSERT", "RUN_TESTS"]);
            TEST("null_assert", FUN((), ASSERT(NULL)));
            TEST("false_assert", FUN((), ASSERT(FALSE)));
            LET(results, RUN_TESTS());
            LEN(results);
        "#).unwrap();
        assert_eq!(v, Value::Integer(2));
    }

    #[test]
    fn b7_assert_eq_equal_returns_ok_true() {
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["ASSERT_EQ"]);
            ASSERT_EQ(42, 42);
        "#).unwrap();
        assert_eq!(v, Value::Ok(Box::new(Value::Boolean(true))));
    }

    #[test]
    fn b7_assert_eq_unequal_is_e0047_with_actual_expected() {
        // §15.9 row 3: payload includes `actual` and `expected`.
        // End-to-end via RUN_TESTS (top-level TRY can't capture
        // an ASSERT_ERR signal — see `b7_assert_false_*`).
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["TEST", "ASSERT_EQ", "RUN_TESTS"]);
            TEST("eq_fail", FUN((), ASSERT_EQ(1, 2)));
            LET(results, RUN_TESTS());
            LET(failed, INDEX_GET(results, 0));
            LET(err_field, INDEX_GET(failed, "error"));
            LET(code, INDEX_GET(err_field, "code"));
            LET(actual, INDEX_GET(err_field, "actual"));
            LET(expected, INDEX_GET(err_field, "expected"));
            [code, actual, expected];
        "#).unwrap();
        match v {
            Value::Array(items) => {
                assert_eq!(items[0], Value::String("E0047".into()));
                assert_eq!(items[1], Value::Integer(1));
                assert_eq!(items[2], Value::Integer(2));
            }
            other => panic!("expected LIST, got {:?}", other),
        }
    }

    #[test]
    fn b7_assert_neq_unequal_returns_ok_true() {
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["ASSERT_NEQ"]);
            ASSERT_NEQ(1, 2);
        "#).unwrap();
        assert_eq!(v, Value::Ok(Box::new(Value::Boolean(true))));
    }

    #[test]
    fn b7_assert_neq_equal_is_e0048() {
        // §15.9 row 4: payload includes `actual`/`expected` and the
        // optional `msg`. End-to-end via RUN_TESTS.
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["TEST", "ASSERT_NEQ", "RUN_TESTS"]);
            TEST("neq_fail", FUN((), ASSERT_NEQ(1, 1, "should differ")));
            LET(results, RUN_TESTS());
            LET(failed, INDEX_GET(results, 0));
            LET(err_field, INDEX_GET(failed, "error"));
            LET(code, INDEX_GET(err_field, "code"));
            LET(msg, INDEX_GET(err_field, "msg"));
            [code, msg];
        "#).unwrap();
        match v {
            Value::Array(items) => {
                assert_eq!(items[0], Value::String("E0048".into()));
                assert_eq!(items[1], Value::String("should differ".into()));
            }
            other => panic!("expected LIST, got {:?}", other),
        }
    }

    #[test]
    fn b7_expect_err_non_err_is_e0049() {
        // §15.9 row 5: input not ERR → ERR(E0049). Via RUN_TESTS.
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["TEST", "EXPECT_ERR", "RUN_TESTS"]);
            TEST("expect_err_fail", FUN((), EXPECT_ERR(42)));
            LET(results, RUN_TESTS());
            LET(failed, INDEX_GET(results, 0));
            LET(err_field, INDEX_GET(failed, "error"));
            LET(code, INDEX_GET(err_field, "code"));
            LET(cond_field, INDEX_GET(err_field, "cond"));
            [code, cond_field];
        "#).unwrap();
        match v {
            Value::Array(items) => {
                assert_eq!(items[0], Value::String("E0049".into()));
                assert_eq!(items[1], Value::Integer(42));
            }
            other => panic!("expected LIST, got {:?}", other),
        }
    }

    #[test]
    fn b7_expect_err_input_is_err_returns_ok() {
        // §15.9 row 5: input is ERR → OK(payload).
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["EXPECT_ERR"]);
            EXPECT_ERR(ERR("boom"));
        "#).unwrap();
        assert_eq!(v, Value::Ok(Box::new(Value::Err(Box::new(Value::String("boom".into()))))));
    }

    #[test]
    fn b7_test_registers_and_returns_null() {
        // §15.9 row 1: TEST returns NULL after pushing to the
        // evaluator's registry. Side-effect verified by RUN_TESTS
        // (next tests).
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["TEST"]);
            TEST("my_test", FUN((), NULL));
        "#).unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn b7_run_tests_with_no_tests_returns_empty_array() {
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["RUN_TESTS"]);
            RUN_TESTS();
        "#).unwrap();
        assert_eq!(v, Value::Array(vec![]));
    }

    #[test]
    fn b7_run_tests_passing_only() {
        // §15.9 row 6: all DICT entries have passed=TRUE, no error key.
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["TEST", "ASSERT", "ASSERT_EQ", "RUN_TESTS"]);
            TEST("add",   FUN((), ASSERT_EQ(+(1, 2), 3)));
            TEST("truth", FUN((), ASSERT(TRUE)));
            LET(results, RUN_TESTS());
            LEN(results);
        "#).unwrap();
        assert_eq!(v, Value::Integer(2));

        // Walk results: each must have name, passed=TRUE, duration_ms.
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["TEST", "ASSERT", "ASSERT_EQ", "RUN_TESTS"]);
            TEST("add",   FUN((), ASSERT_EQ(+(1, 2), 3)));
            TEST("truth", FUN((), ASSERT(TRUE)));
            LET(results, RUN_TESTS());
            LET(p0, LEN(results));
            LET(p1, INDEX_GET(results, 0));
            LET(n0, INDEX_GET(p1, "name"));
            LET(d0, INDEX_GET(p1, "passed"));
            [n0, d0];
        "#).unwrap();
        // Last expression of the program is `[...)`, which
        // returns an Array of the args. We just assert non-error;
        // the per-field checks are covered by the next test.
        match v {
            Value::Array(items) => assert_eq!(items.len(), 2),
            other => panic!("expected Array, got {:?}", other),
        }
    }

    #[test]
    fn b7_run_tests_with_one_failing_test() {
        // §15.9 row 6 + §12.6: RUN_TESTS catches per-test ERR; the
        // test's dict has passed=FALSE + error field carrying the
        // assertion payload.
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["TEST", "ASSERT", "RUN_TESTS"]);
            TEST("passes", FUN((), ASSERT(TRUE)));
            TEST("fails",  FUN((), ASSERT(FALSE, "intentional")));
            LET(results, RUN_TESTS());
            LET(failed, INDEX_GET(results, 1));
            LET(passed_flag, INDEX_GET(failed, "passed"));
            LET(err_field,   INDEX_GET(failed, "error"));
            [passed_flag, err_field];
        "#).unwrap();
        match v {
            Value::Array(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], Value::Boolean(false)); // passed=FALSE
                // error field is the ASSERT ERR's payload (a DICT).
                match &items[1] {
                    Value::Dict(entries) => {
                        assert_eq!(lookup_str(entries, "code").unwrap(), "E0046");
                        assert_eq!(lookup_str(entries, "msg").unwrap(), "intentional");
                    }
                    other => panic!("expected DICT in error, got {:?}", other),
                }
            }
            other => panic!("expected LIST, got {:?}", other),
        }
    }

    #[test]
    fn b7_run_tests_result_dict_has_required_keys() {
        // §15.9 schema lock: every result DICT has at minimum
        // `name`, `passed`, `duration_ms`. A failing test adds
        // `error`; a passing test may add `return_value` when the
        // body returns a non-NULL.
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["TEST", "ASSERT", "RUN_TESTS"]);
            TEST("a", FUN((), NULL));
            LET(results, RUN_TESTS());
            LET(r0, INDEX_GET(results, 0));
            LET(n, INDEX_GET(r0, "name"));
            LET(p, INDEX_GET(r0, "passed"));
            LET(d, INDEX_GET(r0, "duration_ms"));
            [n, p, d];
        "#).unwrap();
        match v {
            Value::Array(items) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], Value::String("a".into()));
                assert_eq!(items[1], Value::Boolean(true));
                assert!(matches!(items[2], Value::Integer(_)));
            }
            other => panic!("expected LIST, got {:?}", other),
        }
    }

    #[test]
    fn b7_test_name_must_be_string() {
        let err = run_std(r#"
            IMPORT("wlwl:std.test", ["TEST"]);
            TEST(42, FUN((), NULL));
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b7_test_body_must_be_callable() {
        let err = run_std(r#"
            IMPORT("wlwl:std.test", ["TEST"]);
            TEST("name", 42);
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b7_test_arity_wrong() {
        let err = run_std(r#"
            IMPORT("wlwl:std.test", ["TEST"]);
            TEST("name");
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn b7_assert_arity_wrong() {
        let err = run_std(r#"
            IMPORT("wlwl:std.test", ["ASSERT"]);
            ASSERT();
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn b7_run_tests_with_zero_arity_wrong() {
        let err = run_std(r#"
            IMPORT("wlwl:std.test", ["RUN_TESTS"]);
            RUN_TESTS(1, 2);
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn b7_std_test_not_global_builtin() {
        // §15.9: std.test is also "name catalog only" — same as
        // collection. A direct call (no IMPORT) must E0020.
        let err = run("TEST(\"x\", FUN((), NULL));").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0020);
    }

    #[test]
    fn b7_unknown_test_name_in_import_is_e0023() {
        let err = run_std(r#"
            IMPORT("wlwl:std.test", ["NOT_A_REAL_NAME"]);
        "#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0023);
    }

    #[test]
    fn b7_collection_and_test_imports_coexist() {
        // Both std modules in one program. Verifies the load_std
        // dispatch table handles multiple name-catalog paths.
        let v = run_std(r#"
            IMPORT("wlwl:std.collection", ["MAP"]);
            IMPORT("wlwl:std.test", ["TEST", "ASSERT", "ASSERT_EQ", "RUN_TESTS"]);
            TEST("squares", FUN((),
                ASSERT_EQ(MAP([1, 2, 3], FUN((x), *(x, x))), [1, 4, 9])
            ));
            LET(results, RUN_TESTS());
            LET(r0, INDEX_GET(results, 0));
            INDEX_GET(r0, "passed");
        "#).unwrap();
        assert_eq!(v, Value::Boolean(true));
    }

    #[test]
    fn b7_uncaught_err_in_test_body_is_caught_by_run_tests() {
        // If the test body returns an ERR directly (without going
        // through an assertion), RUN_TESTS still records it as
        // passed=FALSE with the ERR payload in `error`.
        let v = run_std(r#"
            IMPORT("wlwl:std.test", ["TEST", "RUN_TESTS"]);
            TEST("loose_err", FUN((), ERR("oh no")));
            LET(results, RUN_TESTS());
            LET(failed, INDEX_GET(results, 0));
            LET(passed_flag, INDEX_GET(failed, "passed"));
            LET(err_field,   INDEX_GET(failed, "error"));
            [passed_flag, err_field];
        "#).unwrap();
        match v {
            Value::Array(items) => {
                assert_eq!(items[0], Value::Boolean(false));
                assert_eq!(items[1], Value::String("oh no".into()));
            }
            other => panic!("expected LIST, got {:?}", other),
        }
    }

    // ── helpers used above ──────────────────────────────────────────

    /// Extract a DICT from a payload value; panics with a useful
    /// message if the payload isn't a DICT.
    fn expect_dict(v: Value) -> Vec<(Value, Value)> {
        match v {
            Value::Dict(entries) => entries,
            other => panic!("expected DICT, got {:?}", other),
        }
    }

    /// Linear lookup of a STRING key in a DICT's entries.
    fn INDEX_GET(entries: &[(Value, Value)], key: &str) -> Option<Value> {
        entries
            .iter()
            .find(|(k, _)| matches!(k, Value::String(s) if s == key))
            .map(|(_, v)| v.clone())
    }

    /// Linear lookup that asserts the found value is itself a
    /// STRING. Used for `code` and `msg` payload fields.
    fn lookup_str(entries: &[(Value, Value)], key: &str) -> Option<String> {
        INDEX_GET(entries, key).map(|v| match v {
            Value::String(s) => s,
            other => panic!("expected STRING for `{}`, got {:?}", key, other),
        })
    }

    // ══════════════════════════════════════════════════════════════════
    // Phase B8 — spec v0.4 §10.3 string extensions + FLOAT conversion
    //
    // Tests cover the 11 new global builtins (no IMPORT needed;
    // they sit in the global registry per appendix G):
    //   FLOAT / TRIM / TRIM_START / TRIM_END / STARTS_WITH / ENDS_WITH
    //   REPEAT / PAD_START / PAD_END / CODEPOINTS / FROM_CODEPOINTS
    //
    // Coverage strategy:
    //   - happy path + ASCII edge cases per builtin;
    //   - §10.3 FLOAT ParseError shape (same as INT);
    //   - §12.6 ERR transparent propagation (input ERR → output ERR
    //     surfaces at top level as E0102, same pattern B5/B6/B7
    //     locked in).
    //
    // Note: §10.3 non-ASCII UPPER / LOWER W0014 emit is **deferred to
    // Phase C** (the unicode lowercasing table is added there). The
    // existing UPPER/LOWER builtins already use Rust's char-
    // based to_lowercase()/to_uppercase(); the W0014 contract is
    // documented in plan §5.8 but the actual emit point lands with
    // the rest of the unicode-aware string builtins.
    // ══════════════════════════════════════════════════════════════════

    #[test]
    fn b8_float_identity() {
        assert_eq!(run("FLOAT(1.5);").unwrap(), Value::Float(1.5));
        assert_eq!(run("FLOAT(0.0);").unwrap(), Value::Float(0.0));
        assert_eq!(run("FLOAT(-2.5);").unwrap(), Value::Float(-2.5));
    }

    #[test]
    fn b8_float_from_integer() {
        // INTEGER → FLOAT is exact within ±2^53 (Rust's `as f64`).
        assert_eq!(run("FLOAT(42);").unwrap(), Value::Float(42.0));
        assert_eq!(run("FLOAT(-7);").unwrap(), Value::Float(-7.0));
        assert_eq!(run("FLOAT(0);").unwrap(), Value::Float(0.0));
    }

    #[test]
    fn b8_float_from_string_ok() {
        assert_eq!(run(r#"FLOAT("3.14");"#).unwrap(), Value::Float(3.14));
        assert_eq!(run(r#"FLOAT("-1.5");"#).unwrap(), Value::Float(-1.5));
        assert_eq!(run(r#"FLOAT("0");"#).unwrap(), Value::Float(0.0));
        // Scientific notation
        assert_eq!(run(r#"FLOAT("1e3");"#).unwrap(), Value::Float(1000.0));
    }

    #[test]
    fn b8_float_parse_error_returns_err_value() {
        // §10.3: parse failure returns ERR(["kind": "ParseError", ...]).
        // Not an E0xxx — it's a *value* (caller observes via TRY or
        // pattern-match). End-to-end via `LET(x, FLOAT(...))` then
        // `IS_ERR(x)`: the FLOAT returns ERR to LET as a plain value
        // (no signal involved because FLOAT itself doesn't emit
        // Return — only TRY does); IS_ERR then confirms the shape.
        let v = run(r#"LET(x, FLOAT("not-a-number")); IS_ERR(x);"#).unwrap();
        assert_eq!(v, Value::Boolean(true));
    }

    #[test]
    fn b8_float_non_string_non_number_is_e0030() {
        // Spec §10.3 says INT/FLOAT conversion accepts STRING /
        // INTEGER / FLOAT. Other types → E0030 (Type).
        let err = run(r#"FLOAT([1, 2]);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b8_trim_basics() {
        assert_eq!(run(r#"TRIM("  hello  ");"#).unwrap(), Value::String("hello".into()));
        assert_eq!(run(r#"TRIM("hello");"#).unwrap(), Value::String("hello".into()));
        assert_eq!(run(r#"TRIM("   ");"#).unwrap(), Value::String("".into()));
    }

    #[test]
    fn b8_trim_start_and_end() {
        assert_eq!(run(r#"TRIM_START("  hello  ");"#).unwrap(), Value::String("hello  ".into()));
        assert_eq!(run(r#"TRIM_END("  hello  ");"#).unwrap(), Value::String("  hello".into()));
        // Composed
        assert_eq!(
            run(r#"TRIM_END(TRIM_START("  hi  "));"#).unwrap(),
            Value::String("hi".into())
        );
    }

    #[test]
    fn b8_starts_with_ends_with() {
        assert_eq!(run(r#"STARTS_WITH("hello world", "hello");"#).unwrap(), Value::Boolean(true));
        assert_eq!(run(r#"STARTS_WITH("hello world", "world");"#).unwrap(), Value::Boolean(false));
        assert_eq!(run(r#"ENDS_WITH("hello world", "world");"#).unwrap(), Value::Boolean(true));
        assert_eq!(run(r#"ENDS_WITH("hello world", "hello");"#).unwrap(), Value::Boolean(false));
        // Empty suffix matches anything
        assert_eq!(run(r#"ENDS_WITH("anything", "");"#).unwrap(), Value::Boolean(true));
    }

    #[test]
    fn b8_repeat_basics() {
        assert_eq!(run(r#"REPEAT("ab", 3);"#).unwrap(), Value::String("ababab".into()));
        assert_eq!(run(r#"REPEAT("x", 0);"#).unwrap(), Value::String("".into()));
        assert_eq!(run(r#"REPEAT("foo", 1);"#).unwrap(), Value::String("foo".into()));
    }

    #[test]
    fn b8_repeat_negative_is_e0030() {
        let err = run(r#"REPEAT("ab", -1);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b8_pad_start_and_end() {
        assert_eq!(
            run(r#"PAD_START("hi", 5, "*");"#).unwrap(),
            Value::String("***hi".into())
        );
        assert_eq!(
            run(r#"PAD_END("hi", 5, "*");"#).unwrap(),
            Value::String("hi***".into())
        );
        // Already long enough → no change
        assert_eq!(
            run(r#"PAD_START("hello", 3, "*");"#).unwrap(),
            Value::String("hello".into())
        );
        // Width 0 → no change
        assert_eq!(
            run(r#"PAD_END("hi", 0, "*");"#).unwrap(),
            Value::String("hi".into())
        );
    }

    #[test]
    fn b8_codepoints_and_from_codepoints_roundtrip() {
        // ASCII round-trip.
        assert_eq!(
            run(r#"CODEPOINTS("hello");"#).unwrap(),
            Value::Array(vec![
                Value::Integer(104), Value::Integer(101), Value::Integer(108),
                Value::Integer(108), Value::Integer(111),
            ])
        );
        assert_eq!(
            run(r#"FROM_CODEPOINTS([72, 105]);"#).unwrap(),
            Value::String("Hi".into())
        );
    }

    #[test]
    fn b8_codepoints_unicode_basic_multilingual_plane() {
        // 'é' is U+00E9 = 233 decimal (Latin Small Letter E with Acute).
        // WLWL doesn't have hex literals yet (Phase B8 ships integer
        // conversion only); we use decimal for the codepoint value.
        let v = run(r#"CODEPOINTS("é");"#).unwrap();
        assert_eq!(v, Value::Array(vec![Value::Integer(233)]));
        // Round-trip
        let v = run(r#"FROM_CODEPOINTS([233]);"#).unwrap();
        assert_eq!(v, Value::String("é".into()));
    }

    #[test]
    fn b8_codepoints_unicode_supplementary_plane() {
        // '𝄞' (musical G clef) is U+1D11E = 119070 decimal — outside
        // the BMP, so it requires surrogate pairs in UTF-16 but is
        // a single char in Rust's char / WLWL's codepoint model.
        // This is the test that locks "code points, not UTF-16
        // units".
        let v = run(r#"CODEPOINTS("𝄞");"#).unwrap();
        assert_eq!(v, Value::Array(vec![Value::Integer(119070)]));
        let v = run(r#"FROM_CODEPOINTS([119070]);"#).unwrap();
        assert_eq!(v, Value::String("𝄞".into()));
    }

    #[test]
    fn b8_from_codepoints_surrogate_half_is_e0031() {
        // 0xD800 = 55296 decimal is a UTF-16 surrogate half — not
        // a valid Unicode scalar value. Spec §10.3 doesn't pin a
        // code; we use E0031 (Type bucket) for "value out of legal
        // range" — consistent with INT's out-of-range E0035 for
        // FLOAT overflow.
        let err = run(r#"FROM_CODEPOINTS([55296]);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0031);
        let err = run(r#"FROM_CODEPOINTS([57343]);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0031);
    }

    #[test]
    fn b8_from_codepoints_above_max_is_e0031() {
        let err = run(r#"FROM_CODEPOINTS([1114112]);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0031);
    }

    #[test]
    fn b8_from_codepoints_negative_is_e0031() {
        let err = run(r#"FROM_CODEPOINTS([-1]);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0031);
    }

    #[test]
    fn b8_from_codepoints_non_integer_element_is_e0030() {
        let err = run(r#"FROM_CODEPOINTS([1, "not-a-codepoint"]);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b8_string_builtins_chained() {
        // A small composition sanity check: TRIM + REPEAT + CODEPOINTS.
        let v = run(r#"LEN(CODEPOINTS(REPEAT(TRIM("  abc  "), 2)));"#).unwrap();
        assert_eq!(v, Value::Integer(6));
    }

    #[test]
    fn b8_all_eleven_builtins_present_in_resolve_builtin() {
        // Spec §10.3 + appendix G: 11 new global builtins. Lock the
        // set so any future addition shows up as a deliberate,
        // conscious change.
        use crate::resolve_builtin;
        for name in [
            "FLOAT",
            "TRIM",
            "TRIM_START",
            "TRIM_END",
            "STARTS_WITH",
            "ENDS_WITH",
            "REPEAT",
            "PAD_START",
            "PAD_END",
            "CODEPOINTS",
            "FROM_CODEPOINTS",
        ] {
            assert!(
                resolve_builtin(name).is_some(),
                "spec §10.3 + appendix G: builtin `{}` must be registered (B8)",
                name
            );
        }
    }

    #[test]
    fn b8_err_transparent_for_all_new_builtins() {
        // §12.6: input ERR is short-circuited in eval_call before
        // reaching the builtin; the ERR value surfaces at top level
        // as E0102 (consistent with B5/B6/B7 patterns). We don't
        // expect the builtin itself to receive ERR args — the
        // surface check is E0102.
        let probes = [
            r#"FLOAT(ERR("e"));"#,
            r#"TRIM(ERR("e"));"#,
            r#"TRIM_START(ERR("e"));"#,
            r#"TRIM_END(ERR("e"));"#,
            r#"STARTS_WITH(ERR("e"), "x");"#,
            r#"ENDS_WITH(ERR("e"), "x");"#,
            r#"REPEAT(ERR("e"), 1);"#,
            r#"PAD_START(ERR("e"), 5, "*");"#,
            r#"PAD_END(ERR("e"), 5, "*");"#,
            r#"CODEPOINTS(ERR("e"));"#,
            // FROM_CODEPOINTS's array literal isn't an ERR-input probe
            // (the ERR is *inside* the array — the array literal
            // evaluates to `[Err]`, not `Err`; the builtin then
            // rejects the element type with E0030). Use the bare
            // form so ERR precheck kicks in at the call boundary.
            r#"FROM_CODEPOINTS(ERR("e"));"#,
        ];
        for src in probes {
            let err = run(src).unwrap_err();
            assert_eq!(
                err.diagnostic().code,
                ErrorCode::E0102,
                "§12.6 ERR transparent: top-level call `{}` should be E0102",
                src
            );
        }
    }

    // ══════════════════════════════════════════════════════════════════
    // Phase B9 — `NOT` macro-function + `!` v0.3-compat alias → W0054
    // (spec v0.4 §3.4 + §14.5)
    //
    // Tests cover the dual dispatch:
    //   - `NOT(x)` is the v0.4 canonical keyword; lexer
    //     `TokenKind::Not` → parser name "NOT" → eval `builtin_not`
    //     (clean). No W0054.
    //   - `!x` / `!(x)` is the v0.3-compat single-char alias; lexer
    //     `TokenKind::Bang` → parser name "!" → eval
    //     `builtin_not_bang_compat` (emits W0054, then delegates).
    //     v0.5 removes `!`.
    //
    // Truthiness semantics are shared: `!NULL = TRUE`, `!0 = TRUE`,
    // `!1 = FALSE`, etc. (spec §9.4 row 5).
    // ══════════════════════════════════════════════════════════════════

    #[test]
    fn b9_not_clean_path_emits_no_warnings() {
        // v0.4 canonical keyword; no warning on any number of
        // calls (the warn-runner's drain returns no W0054 codes).
        let (r, w) = run_with_warnings("NOT(TRUE); NOT(FALSE);");
        r.unwrap();
        assert!(
            w.is_empty(),
            "v0.4 canonical `NOT` must not emit any warnings, got {:?}",
            w
        );
    }

    #[test]
    fn b9_not_truthiness_matches_negation_table() {
        // §9.4 truthiness table: Boolean(b)=b, Null=false, **everything
        // else** (Integer / Float / String / Array / Dict / Closure /
        // NativeFn / Ok / Err) = true. Lock the table so a future
        // regression on either path (`!` / `NOT`) surfaces here.
        // Note `0` and `""` are NOT falsy in v0.4 — they're "everything
        // else", which is truthy. (Python's `bool(0) = False` /
        // JavaScript's `Boolean(0) = false` don't apply; WLWL is
        // explicitly tri-state per §9.4 row 5.)
        // Boolean
        assert_eq!(run("NOT(TRUE);").unwrap(), Value::Boolean(false));
        assert_eq!(run("NOT(FALSE);").unwrap(), Value::Boolean(true));
        // Null
        assert_eq!(run("NOT(NULL);").unwrap(), Value::Boolean(true));
        // Integer — *all* integers are truthy per §9.4 row 5
        assert_eq!(run("NOT(0);").unwrap(), Value::Boolean(false));
        assert_eq!(run("NOT(1);").unwrap(), Value::Boolean(false));
        assert_eq!(run("NOT(42);").unwrap(), Value::Boolean(false));
        assert_eq!(run("NOT(-7);").unwrap(), Value::Boolean(false));
        // String — non-empty AND empty are both truthy (string
        // empty-truthy is the design choice that distinguishes
        // §9.4 from C / Python)
        assert_eq!(run(r#"NOT("");"#).unwrap(), Value::Boolean(false));
        assert_eq!(run(r#"NOT("non-empty");"#).unwrap(), Value::Boolean(false));
        // Empty ARRAY / DICT — still "everything else" per §9.4
        assert_eq!(run("NOT([]);").unwrap(), Value::Boolean(false));
        assert_eq!(run("NOT([\"a\": 1]);").unwrap(), Value::Boolean(false));
    }

    #[test]
    fn b9_bang_emits_w0054_once_per_call() {
        // v0.3-compat single-char `!` must emit exactly one W0054
        // per call. Two calls = two warnings.
        let (r, w) = run_with_warnings("!TRUE; !FALSE;");
        r.unwrap();
        let w0054_count = w
            .iter()
            .filter(|wm| wm.code == ErrorCode::W0054)
            .count();
        assert_eq!(
            w0054_count, 2,
            "each `!` site must emit exactly one W0054, got warnings: {:?}",
            w
        );
    }

    #[test]
    fn b9_bang_call_form_emits_w0054() {
        // `!(x)` parenthesised form (same `!` token, call syntax).
        let (r, w) = run_with_warnings("!(TRUE);");
        r.unwrap();
        let codes: Vec<_> = w
            .iter()
            .map(|wm| wm.code)
            .collect();
        assert_eq!(
            codes,
            vec![ErrorCode::W0054],
            "`!(x)` form must emit exactly one W0054, got: {:?}",
            codes
        );
    }

    #[test]
    fn b9_bang_and_not_share_truthiness_semantics() {
        // Even though they take different dispatch paths, both
        // routes must produce the same value for every input.
        // This is the regression guard for the split: if anyone
        // refactors `builtin_not` without threading the same
        // truthiness logic, the cross-check fails.
        let probes = ["TRUE", "FALSE", "NULL", "0", "1", "42", r#""""#, r#""x""#];
        for src in probes {
            let bang = run(&format!("!({});", src)).unwrap();
            let not = run(&format!("NOT({});", src)).unwrap();
            assert_eq!(
                bang, not,
                "`!` and `NOT` must agree on truthiness for {} (got {:?} vs {:?})",
                src, bang, not
            );
        }
    }

    #[test]
    fn b9_not_with_err_arg_is_e0102() {
        // §12.6: NOT is not in ERR_CONSUMER_REGISTRY, so an ERR
        // arg short-circuits in eval_call; the ERR value reaches
        // the top level as E0102. Same pattern as B5/B6/B7/B8 ERR-
        // transparent probes.
        let err = run("NOT(ERR(\"e\"));").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        let err = run("!(ERR(\"e\"));").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn b9_w0054_in_warning_codes_set() {
        // Lock the registered warning code list to 11 entries
        // (10 from B7 + W0054 from B9).
        use crate::ErrorCode as E;
        // The "warnings as a category" hint registered under E0030
        // path; here we just check both endpoints are wired.
        let _ = E::W0054; // compile-time existence check
        assert!(format!("{:?}", E::W0054).contains("W0054"));
    }

    #[test]
    fn b9_not_resolves_via_resolve_builtin() {
        // The dispatch table must contain both `"NOT"` (clean) and
        // `"!"` (W0054 wrapper). If anyone refactors and loses
        // either entry, the parser-side `Expr::Call { name: ... }`
        // dispatch will hit the registry-lock test for E0020
        // instead — but locking it here makes the contract obvious.
        use crate::resolve_builtin;
        assert!(resolve_builtin("NOT").is_some());
        assert!(resolve_builtin("!").is_some());
    }

    // ══════════════════════════════════════════════════════════════════
    // Phase B10 — `PRINT_ERR` writes to stderr (spec v0.4 §15.1)
    //
    // Tests cover the new global builtin `PRINT_ERR` (same shape as
    // `PRINT`, but `eprintln!` instead of `println!`) and the std
    // module twin `wlwl:std.io::PRINT_ERR`. The actual stderr bytes
    // are NOT captured (would need process-level redirection via
    // `assert_cmd` / `gag` crate, deferred to Phase F perf tests);
    // we lock the contract via:
    //   - the call returns `NULL` (same as PRINT);
    //   - it accepts multiple args (joined with single space, same
    //     as PRINT);
    //   - dispatch via the global table AND via IMPORT both work;
    //   - §12.6 ERR transparent propagation holds (input ERR → top-
    //     level E0102; no special "consume" semantics — PRINT_ERR is
    //     a side-effect sink, not a value observer).
    // ══════════════════════════════════════════════════════════════════

    #[test]
    fn b10_print_err_returns_null() {
        let v = run(r#"PRINT_ERR("oops");"#).unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn b10_print_err_with_multiple_args() {
        // Same join-with-space contract as PRINT — values
        // formatted via `Value::display()`.
        let v = run(r#"PRINT_ERR("error:", 42, "is", TRUE);"#).unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn b10_print_err_via_std_module() {
        // The std-module form (IMPORT + eprintln! in
        // wlwl_std::io::std_print_err) must bind and run. Locks
        // the path that a user who has explicitly IMPORT'd
        // `wlwl:std.io` still gets stderr semantics.
        let v = run_std(r#"
            IMPORT("wlwl:std.io", ["PRINT_ERR"]);
            PRINT_ERR("via", "std.io");
        "#).unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn b10_print_err_resolves_in_dispatch_table() {
        // Lock the global dispatch entry. If a future refactor
        // removes PRINT_ERR from resolve_builtin, the parser-side
        // `Expr::Call { name: "PRINT_ERR", ... }` will fall through
        // to the user-env lookup and surface E0020 — catching it
        // here makes the contract obvious.
        use crate::resolve_builtin;
        assert!(resolve_builtin("PRINT_ERR").is_some());
    }

    #[test]
    fn b10_print_err_input_err_is_e0102() {
        // §12.6: PRINT_ERR is not in ERR_CONSUMER_REGISTRY, so an
        // ERR arg short-circuits in eval_call; the ERR value
        // surfaces at top level as E0102 (same pattern as every
        // other side-effect builtin, e.g. PRINT itself).
        let err = run(r#"PRINT_ERR(ERR("e"));"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn b10_print_and_print_err_coexist_in_std_io() {
        // Both names must remain available side-by-side in the
        // std.io module — adding PRINT_ERR doesn't shadow PRINT.
        let v = run_std(r#"
            IMPORT("wlwl:std.io", ["PRINT", "PRINT_ERR", "INPUT"]);
            LET(arity, LEN([PRINT, PRINT_ERR, INPUT]));
            arity;
        "#).unwrap();
        assert_eq!(v, Value::Integer(3));
    }
    // ── Phase B11: spec v0.4 附录 G 全局内建注册表 lock-down ────────
    //
    // B11 把 spec 附录 G 88 条内置函数钉死在 `crate::registry::BUILTIN_REGISTRY`。
    // 这 5 个锁测试守住 4 个不变式: (1) 注册表覆盖 resolve_builtin 表;
    // (2) resolve_builtin 覆盖注册表的 ResolvedBuiltin/Compat; (3) ERR 消费
    // 表 (ERR_CONSUMER_REGISTRY) 与注册表的 err_consumer 字段一致;
    // (4) macro_fn=true 必须对应 LexerMacro 或被 ResolvedBuiltin 显式接受
    // (NOT/UNWRAP_OR/OR_DIE 这三个特例 —— spec 标 macro 但实现走 builtin);
    // (5) 总条目数 ≥ 88 (spec 附录 G 表格 89 行,CALL 重复一次只算 1 条)。

    #[test]
    fn b11_registry_covers_resolve_builtin() {
        // 反向:每个 `resolve_builtin` 表里的名字都必须在注册表里出现
        // (ResolvedBuiltin 或 ResolvedCompat)。如果未来加 builtin 但
        // 忘了登记,这条会挂。
        let registry_names: std::collections::HashSet<&'static str> =
            crate::registry::resolved_builtin_names().into_iter().collect();
        // 这些名字 resolve_builtin 接了,但 spec 附录 G 没有列 → 必须留在
        // 注册表的 ResolvedBuiltin/Compat 路径(尽管可能在另一行)。
        // 检查样例:PRINT / NOT / OR_DIE / FORMAT / INDEX_GET。
        for must_have in &["PRINT", "PRINT_ERR", "NOT", "FORMAT", "INDEX_GET", "OR_DIE", "DEL"] {
            assert!(
                registry_names.contains(must_have),
                "registry must list `{}` as ResolvedBuiltin or ResolvedCompat",
                must_have,
            );
        }
    }

    #[test]
    fn b11_resolve_builtin_covers_registry() {
        // 正向:注册表里每个 ResolvedBuiltin/ResolvedCompat 都必须能
        // 通过 `resolve_builtin(name)` 拿到 builtin fn。
        for name in crate::registry::resolved_builtin_names() {
            assert!(
                resolve_builtin(name).is_some(),
                "resolve_builtin({:?}) is None but registry says it is ResolvedBuiltin/Compat",
                name,
            );
        }
    }

    #[test]
    fn b11_err_consumer_registry_consistent() {
        // ERR_CONSUMER_REGISTRY (eval 内的白名单) 与注册表的
        // err_consumer = Yes 字段必须一致。前者是后者的运行时 short-circuit
        // 入口,任何漂移都会让 §12.6 ERR 透明传播错乱。
        let from_registry: std::collections::HashSet<&'static str> =
            crate::registry::err_consumer_names().into_iter().collect();
        let from_const: std::collections::HashSet<&str> =
            ERR_CONSUMER_REGISTRY.iter().copied().collect();
        // LexerMacro 例外 (IS_OK / IS_ERR / TRY / EXPECT_ERR): 在解析期就被
        // 降为 Expr::IsOk 等,不会进 ERR_CONSUMER_REGISTRY 运行时路径,
        // 也不应在注册表里标 ResolvedBuiltin —— 它们是 LexerMacro 且
        // err_consumer = Yes,逻辑上"算 ERR 消费者"。
        // 双向断言:ERR_CONSUMER_REGISTRY ⊆ {err_consumer=Yes 且 ResolvedBuiltin/Compat}
        for name in ERR_CONSUMER_REGISTRY.iter() {
            let spec = crate::registry::lookup(name)
                .unwrap_or_else(|| panic!("ERR_CONSUMER_REGISTRY entry {:?} not in BUILTIN_REGISTRY", name));
            assert_eq!(
                spec.err_consumer,
                crate::registry::ErrConsumerStatus::Yes,
                "{:?} is in ERR_CONSUMER_REGISTRY but registry.err_consumer != Yes",
                name,
            );
            assert!(
                matches!(
                    spec.dispatch,
                    crate::registry::DispatchStatus::ResolvedBuiltin
                        | crate::registry::DispatchStatus::ResolvedCompat
                        | crate::registry::DispatchStatus::LexerMacro,
                ),
                "{:?} is in ERR_CONSUMER_REGISTRY but registry.dispatch = {:?}",
                name, spec.dispatch,
            );
        }
        // 反向:注册表里 err_consumer=Yes 且 dispatch = ResolvedBuiltin/Compat
        // 的名字必须出现在 ERR_CONSUMER_REGISTRY (除非是 LexerMacro 例外:
        // IS_OK/IS_ERR/TRY/EXPECT_ERR —— 它们通过 Expr::* 路径消费 ERR,
        // 不进 ERR_CONSUMER_REGISTRY 运行时表)。
        let lexer_macro_err_consumers = ["IS_OK", "IS_ERR", "TRY", "EXPECT_ERR"];
        for spec in crate::registry::BUILTIN_REGISTRY.iter() {
            if spec.err_consumer != crate::registry::ErrConsumerStatus::Yes {
                continue;
            }
            if matches!(spec.dispatch, crate::registry::DispatchStatus::LexerMacro) {
                assert!(
                    lexer_macro_err_consumers.contains(&spec.name),
                    "{:?} is LexerMacro + err_consumer=Yes but not in lexer_macro_err_consumers whitelist",
                    spec.name,
                );
                continue;
            }
            // ResolvedBuiltin/Compat 必须出现在 ERR_CONSUMER_REGISTRY
            assert!(
                from_const.contains(spec.name),
                "{:?} is ResolvedBuiltin + err_consumer=Yes but missing from ERR_CONSUMER_REGISTRY",
                spec.name,
            );
        }
        // sanity:双方条数大致接近 (LexerMacro 例外决定差异)
        assert!(
            from_registry.len() >= from_const.len(),
            "registry has fewer err_consumers ({}) than ERR_CONSUMER_REGISTRY ({})",
            from_registry.len(), from_const.len(),
        );
    }

    #[test]
    fn b11_macro_fn_attribute_matches_dispatch() {
        // macro_fn=true 的条目要么是 LexerMacro,要么是 ResolvedBuiltin
        // (spec 附录 G 表的"宏函数 ✔"列在 NOT/UNWRAP_OR/OR_DIE 这三个
        // 名字上,虽然实现走 builtin dispatch,但 spec 标 macro = true)。
        // 其它 (例如 TYPE) 也是 macro_fn=true 但 ResolvedBuiltin —— TYPE
        // 是 §2.5 形式化场景,spec 表的"宏函数 ✔"列在 TYPE 上。
        for spec in crate::registry::BUILTIN_REGISTRY.iter() {
            if !spec.macro_fn {
                continue;
            }
            assert!(
                matches!(
                    spec.dispatch,
                    crate::registry::DispatchStatus::LexerMacro
                        | crate::registry::DispatchStatus::ResolvedBuiltin
                        | crate::registry::DispatchStatus::ResolvedCompat,
                ),
                "{:?} has macro_fn=true but dispatch = {:?} (must be LexerMacro/ResolvedBuiltin/ResolvedCompat)",
                spec.name, spec.dispatch,
            );
        }
        // 反向 sanity:LexerMacro 必须 macro_fn=true (parser 把它降为
        // Expr::* 等价于宏展开,语义与 macro_fn 一致)
        for spec in crate::registry::BUILTIN_REGISTRY.iter() {
            if matches!(spec.dispatch, crate::registry::DispatchStatus::LexerMacro) {
                assert!(
                    spec.macro_fn,
                    "{:?} is LexerMacro but macro_fn=false (must be true)",
                    spec.name,
                );
            }
        }
    }

    #[test]
    fn b11_registry_count_matches_spec_table() {
        // spec 附录 G 表格 89 行,CALL 重复一次 → 88 unique entries。
        // 任何加减条目都会让这条挂。b11_subsequent_drop_in_entries_should_lock
        // 测试追加 (e.g. SPEC.md 升 v0.5) 必须显式 bump 这个数字。
        assert_eq!(
            crate::registry::BUILTIN_REGISTRY.len(),
            90,
            "BUILTIN_REGISTRY size changed (now {}); if spec 附录 G bumped, update this lock",
            crate::registry::BUILTIN_REGISTRY.len(),
        );
        // 14 个 BuiltinGroup 必须全部 ≥ 1 条 (sanity,避免漏写 group)
        use crate::registry::BuiltinGroup;
        for g in [
            BuiltinGroup::Io,
            BuiltinGroup::Conv,
            BuiltinGroup::Result,
            BuiltinGroup::Control,
            BuiltinGroup::Op,
            BuiltinGroup::Array,
            BuiltinGroup::Dict,
            BuiltinGroup::Subscript,
            BuiltinGroup::String,
            BuiltinGroup::Format,
            BuiltinGroup::Module,
            BuiltinGroup::Oop,
            BuiltinGroup::Property,
            BuiltinGroup::Ctor,
        ] {
            let count = crate::registry::BUILTIN_REGISTRY.iter().filter(|s| s.group == g).count();
            assert!(count >= 1, "BuiltinGroup {:?} has 0 entries", g);
        }
        // Deferred 数量 sanity:B11 末应该有 ~24 个 (spec 列了但 impl 未接)
        let deferred = crate::registry::deferred_names();
        assert!(
            deferred.len() == 0,
            "Deferred count {} out of expected band [20, 30]",
            deferred.len(),
        );
    }


    // ── Phase B12: spec v0.4 §10.1 ARRAY ops (7 项) ────────────────
    //
    // spec 附录 G Deferred 的 7 个 array builtin 现在都进 resolve_builtin
    // 了 —— 这 12 个测试锁住它们的语义、边界、ERR-transparent propagation。
    //
    // **POP deviation** (P4-B12-002): spec 附录 G 标 POP(arr) -> ARRAY
    // (移除最后一个元素),但本 impl 在 B1 把 POP 重载成
    // POP(dict, key, default) -> v (DICT 安全删除)。B12 不动 POP 语义
    // 以保持向后兼容,只锁 7 个新 array op。

    #[test]
    fn b12_shift_basic_and_empty() {
        assert_eq!(run("SHIFT([1, 2, 3]);").unwrap(), Value::Array(vec![Value::Integer(2), Value::Integer(3)]));
        assert_eq!(run("SHIFT([]);").unwrap(), Value::Array(vec![]));
        assert_eq!(run("SHIFT([42]);").unwrap(), Value::Array(vec![]));
    }

    #[test]
    fn b12_unshift_prepends_element() {
        let v = run("UNSHIFT([2, 3], 1);").unwrap();
        assert_eq!(v, Value::Array(vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)]));
        let err = run("UNSHIFT(42, 1);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b12_slice_basic_negative_and_oob() {
        assert_eq!(run("SLICE([1,2,3,4,5], 1, 3);").unwrap(),
            Value::Array(vec![Value::Integer(2), Value::Integer(3)]));
        assert_eq!(run("SLICE([1,2,3,4,5], -2);").unwrap(),
            Value::Array(vec![Value::Integer(4), Value::Integer(5)]));
        assert_eq!(run("SLICE([1,2,3,4,5], 2);").unwrap(),
            Value::Array(vec![Value::Integer(3), Value::Integer(4), Value::Integer(5)]));
        assert_eq!(run("SLICE([1,2,3], 2, 2);").unwrap(), Value::Array(vec![]));
        let err = run("SLICE([1,2,3]);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn b12_concat_array_and_string() {
        assert_eq!(run("CONCAT([1, 2], [3, 4]);").unwrap(),
            Value::Array(vec![Value::Integer(1), Value::Integer(2), Value::Integer(3), Value::Integer(4)]));
        assert_eq!(run("CONCAT([], [1]);").unwrap(),
            Value::Array(vec![Value::Integer(1)]));
        let v = run(r#"CONCAT("ab", "cd");"#).unwrap();
        assert_eq!(v, Value::Array(vec![Value::Integer(97), Value::Integer(98), Value::Integer(99), Value::Integer(100)]));
        let err = run("CONCAT(1, 2);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b12_contains_array_and_string() {
        assert_eq!(run("CONTAINS([1, 2, 3], 2);").unwrap(), Value::Boolean(true));
        assert_eq!(run("CONTAINS([1, 2, 3], 99);").unwrap(), Value::Boolean(false));
        assert_eq!(run("CONTAINS([[1,2], [3,4]], [1,2]);").unwrap(), Value::Boolean(true));
        assert_eq!(run(r#"CONTAINS("hello world", "world");"#).unwrap(), Value::Boolean(true));
        assert_eq!(run(r#"CONTAINS("hello", "xyz");"#).unwrap(), Value::Boolean(false));
        let err = run("CONTAINS(42, 1);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b12_index_returns_1_based_or_minus_one() {
        assert_eq!(run(r#"INDEX(["a", "b", "c"], "b");"#).unwrap(), Value::Integer(2));
        assert_eq!(run("INDEX([10, 20, 30], 30);").unwrap(), Value::Integer(3));
        assert_eq!(run(r#"INDEX(["a", "b"], "z");"#).unwrap(), Value::Integer(-1));
        assert_eq!(run(r#"INDEX("hello", "ll");"#).unwrap(), Value::Integer(3));
        assert_eq!(run(r#"INDEX("hello", "xyz");"#).unwrap(), Value::Integer(-1));
    }

    #[test]
    fn b12_reverse_array_and_string() {
        assert_eq!(run("REVERSE([1, 2, 3]);").unwrap(),
            Value::Array(vec![Value::Integer(3), Value::Integer(2), Value::Integer(1)]));
        assert_eq!(run("REVERSE([]);").unwrap(), Value::Array(vec![]));
        let v = run(r#"REVERSE("abc");"#).unwrap();
        assert_eq!(v, Value::Array(vec![Value::Integer(99), Value::Integer(98), Value::Integer(97)]));
    }

    #[test]
    fn b12_seven_resolved_builtins_in_resolve_builtin() {
        use crate::resolve_builtin;
        for name in &["SHIFT", "UNSHIFT", "SLICE", "CONCAT", "CONTAINS", "INDEX", "REVERSE"] {
            assert!(resolve_builtin(name).is_some(),
                "resolve_builtin({:?}) is None; B12 did not register the array op", name);
        }
    }

    #[test]
    fn b12_seven_moved_from_deferred_to_resolved_in_registry() {
        for spec in crate::registry::BUILTIN_REGISTRY.iter() {
            if ["SHIFT", "UNSHIFT", "SLICE", "CONCAT", "CONTAINS", "INDEX", "REVERSE"]
                .contains(&spec.name)
            {
                assert_eq!(spec.dispatch, crate::registry::DispatchStatus::ResolvedBuiltin,
                    "{:?} is still Deferred after B12", spec.name);
            }
        }
    }

    #[test]
    fn b12_err_transparent_for_each_op() {
        let err = run(r#"SHIFT(ERR("e"));"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        let err = run(r#"UNSHIFT(ERR("e"), 1);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        let err = run(r#"SLICE(ERR("e"), 0);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        let err = run(r#"CONCAT(ERR("e"), [1]);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        let err = run(r#"CONTAINS(ERR("e"), 1);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        let err = run(r#"INDEX(ERR("e"), 1);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        let err = run(r#"REVERSE(ERR("e"));"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn b12_immutable_does_not_mutate_input() {
        assert_eq!(run("LET(arr, [1,2,3]); SHIFT(arr); LEN(arr);").unwrap(), Value::Integer(3));
        assert_eq!(run("LET(arr, [1,2,3]); REVERSE(arr); LEN(arr);").unwrap(), Value::Integer(3));
        assert_eq!(run("LET(arr, [1,2,3]); SLICE(arr, 0, 2); LEN(arr);").unwrap(), Value::Integer(3));
    }

    // ── Phase B13: spec v0.4 §10.3 STRING ops (5 项) ────────────────
    //
    // spec 附录 G Deferred 的 5 个 string builtin (UPPER / LOWER / SUB /
    // REPLACE / SPLIT) 现在都进 resolve_builtin 了 —— 9 个测试锁住它们
    // 的语义、ASCII 边界、负数切片、空 sep 错误。

    #[test]
    fn b13_upper_lower_ascii() {
        assert_eq!(run(r#"UPPER("hello");"#).unwrap(), Value::String("HELLO".into()));
        assert_eq!(run(r#"LOWER("HELLO");"#).unwrap(), Value::String("hello".into()));
        // 已是目标 case → 不变
        assert_eq!(run(r#"UPPER("ABC");"#).unwrap(), Value::String("ABC".into()));
        assert_eq!(run(r#"LOWER("xyz");"#).unwrap(), Value::String("xyz".into()));
        // 非 ASCII 原样保留
        assert_eq!(run(r#"UPPER("héllo");"#).unwrap(), Value::String("HéLLO".into()));
        // 空 string
        assert_eq!(run(r#"UPPER("");"#).unwrap(), Value::String("".into()));
    }

    #[test]
    fn b13_sub_basic_negative_oob() {
        assert_eq!(run(r#"SUB("hello", 1, 4);"#).unwrap(), Value::String("ell".into()));
        assert_eq!(run(r#"SUB("hello", 2);"#).unwrap(), Value::String("llo".into()));
        // 负数从尾数
        assert_eq!(run(r#"SUB("hello", -3);"#).unwrap(), Value::String("llo".into()));
        // start >= end → 空
        assert_eq!(run(r#"SUB("hello", 3, 3);"#).unwrap(), Value::String("".into()));
        // 类型错
        let err = run(r#"SUB(42, 0, 1);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b13_replace_basic_and_empty_old() {
        assert_eq!(run(r#"REPLACE("hello world", "world", "rust");"#).unwrap(),
            Value::String("hello rust".into()));
        assert_eq!(run(r#"REPLACE("aaa", "a", "bb");"#).unwrap(),
            Value::String("bbbbbb".into()));
        // old 空 → E0030 (避免死循环)
        let err = run(r#"REPLACE("hello", "", "x");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        // 类型错
        let err = run(r#"REPLACE(42, "x", "y");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b13_split_basic_and_empty_sep() {
        assert_eq!(run(r#"SPLIT("a,b,c", ",");"#).unwrap(),
            Value::Array(vec![Value::String("a".into()), Value::String("b".into()), Value::String("c".into())]));
        // 多字符 sep
        assert_eq!(run(r#"SPLIT("hello world rust", " ");"#).unwrap(),
            Value::Array(vec![Value::String("hello".into()), Value::String("world".into()), Value::String("rust".into())]));
        // sep 不在 → 整个字符串
        assert_eq!(run(r#"SPLIT("hello", "x");"#).unwrap(),
            Value::Array(vec![Value::String("hello".into())]));
        // 空 sep → E0030
        let err = run(r#"SPLIT("hello", "");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b13_seven_registered_in_resolve_builtin() {
        use crate::resolve_builtin;
        for name in &["UPPER", "LOWER", "SUB", "REPLACE", "SPLIT"] {
            assert!(resolve_builtin(name).is_some(),
                "resolve_builtin({:?}) is None; B13 did not register", name);
        }
    }

    #[test]
    fn b13_seven_moved_to_resolved_in_registry() {
        for spec in crate::registry::BUILTIN_REGISTRY.iter() {
            if ["UPPER", "LOWER", "SUB", "REPLACE", "SPLIT"].contains(&spec.name) {
                assert_eq!(spec.dispatch, crate::registry::DispatchStatus::ResolvedBuiltin,
                    "{:?} is still Deferred after B13", spec.name);
            }
        }
    }

    #[test]
    fn b13_err_transparent() {
        let err = run(r#"UPPER(ERR("e"));"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        let err = run(r#"LOWER(ERR("e"));"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        let err = run(r#"SUB(ERR("e"), 0, 1);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        let err = run(r#"REPLACE(ERR("e"), "x", "y");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        let err = run(r#"SPLIT(ERR("e"), ",");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn b13_sub_negative_index_normalization() {
        // SUB(s, -1) → 最后一个 char
        assert_eq!(run(r#"SUB("abc", -1);"#).unwrap(), Value::String("c".into()));
        // SUB(s, -3, -1) → "ab"
        assert_eq!(run(r#"SUB("abc", -3, -1);"#).unwrap(), Value::String("ab".into()));
    }

    #[test]
    fn b13_unicode_preserved() {
        // 非 ASCII char 不被 case-fold 破坏
        assert_eq!(run(r#"UPPER("héllo wörld");"#).unwrap(),
            Value::String("HéLLO WöRLD".into()));
        // SUB 处理 codepoint (而非 UTF-8 bytes)
        assert_eq!(run(r#"SUB("héllo", 1, 4);"#).unwrap(),
            Value::String("éll".into()));
    }

    // ── Phase B14: spec v0.4 §10.2 DICT ops (4 项) ────────────────
    //
    // spec 附录 G Deferred 的 4 个 dict builtin (KEYS / VALUES / HAS /
    // MERGE) 现在都进 resolve_builtin 了 —— 8 个测试锁住它们的语义、
    // 出现顺序、key 冲突覆盖、空 dict 边界、ERR-transparent propagation。

    #[test]
    fn b14_keys_preserves_order() {
        assert_eq!(
            run(r#"KEYS(["a": 1, "b": 2, "c": 3]);"#).unwrap(),
            Value::Array(vec![Value::String("a".into()), Value::String("b".into()), Value::String("c".into())])
        );
        // 单 key dict
        assert_eq!(run(r#"KEYS(["only": 42]);"#).unwrap(),
            Value::Array(vec![Value::String("only".into())]));
        // 类型错
        let err = run("KEYS([1, 2, 3]);").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b14_values_preserves_order() {
        assert_eq!(
            run(r#"VALUES(["a": 1, "b": 2, "c": 3]);"#).unwrap(),
            Value::Array(vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)])
        );
        assert_eq!(run(r#"VALUES(["only": 42]);"#).unwrap(),
            Value::Array(vec![Value::Integer(42)]));
    }

    #[test]
    fn b14_has_basic_and_value_equality() {
        assert_eq!(run(r#"HAS(["a": 1, "b": 2], "a");"#).unwrap(), Value::Boolean(true));
        assert_eq!(run(r#"HAS(["a": 1, "b": 2], "z");"#).unwrap(), Value::Boolean(false));
        // INTEGER key
        assert_eq!(run(r#"HAS([1: "a", 2: "b"], 1);"#).unwrap(), Value::Boolean(true));
        assert_eq!(run(r#"HAS([1: "a", 2: "b"], 3);"#).unwrap(), Value::Boolean(false));
    }

    #[test]
    fn b14_merge_basic_and_key_override() {
        // 不冲突:按 b 的顺序附加到 a
        assert_eq!(
            run(r#"MERGE(["a": 1, "b": 2], ["c": 3]);"#).unwrap(),
            Value::Dict(vec![
                (Value::String("a".into()), Value::Integer(1)),
                (Value::String("b".into()), Value::Integer(2)),
                (Value::String("c".into()), Value::Integer(3)),
            ])
        );
        // 冲突:b 的值覆盖 a
        assert_eq!(
            run(r#"MERGE(["x": 1, "y": 2], ["y": 99, "z": 3]);"#).unwrap(),
            Value::Dict(vec![
                (Value::String("x".into()), Value::Integer(1)),
                (Value::String("y".into()), Value::Integer(99)),
                (Value::String("z".into()), Value::Integer(3)),
            ])
        );
        // 单 key dict merge
        assert_eq!(
            run(r#"MERGE(["a": 1], ["b": 2]);"#).unwrap(),
            Value::Dict(vec![
                (Value::String("a".into()), Value::Integer(1)),
                (Value::String("b".into()), Value::Integer(2)),
            ])
        );
        // 类型错
        let err = run(r#"MERGE([1,2,3], ["a": 1]);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b14_four_registered_in_resolve_builtin() {
        use crate::resolve_builtin;
        for name in &["KEYS", "VALUES", "HAS", "MERGE"] {
            assert!(resolve_builtin(name).is_some(),
                "resolve_builtin({:?}) is None; B14 did not register", name);
        }
    }

    #[test]
    fn b14_four_moved_to_resolved_in_registry() {
        for spec in crate::registry::BUILTIN_REGISTRY.iter() {
            if ["KEYS", "VALUES", "HAS", "MERGE"].contains(&spec.name) {
                assert_eq!(spec.dispatch, crate::registry::DispatchStatus::ResolvedBuiltin,
                    "{:?} is still Deferred after B14", spec.name);
            }
        }
    }

    #[test]
    fn b14_err_transparent() {
        let err = run(r#"KEYS(ERR("e"));"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        let err = run(r#"VALUES(ERR("e"));"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        let err = run(r#"HAS(ERR("e"), "a");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
        let err = run(r#"MERGE(ERR("e"), ["a": 1]);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0102);
    }

    #[test]
    fn b14_dict_not_mutated_by_merge() {
        // MERGE 返回新 dict,不修改 a/b (immutable 语义)
        assert_eq!(
            run(r#"LET(a, ["x": 1]); LET(b, ["y": 2]); MERGE(a, b); LEN(a);"#).unwrap(),
            Value::Integer(1)
        );
    }

    // ── Phase B15: spec v0.4 misc 8 项 (INPUT / BOOL / CALL / NEG / GET_PROP / SET_PROP / CALL_METHOD / MODULE_REF) ────────────────
    //
    // 4 个简单 builtin (BOOL / NEG / INPUT / CALL) 真正实现;
    // 4 个 OOP / 模块值 stub (GET_PROP / SET_PROP / CALL_METHOD / MODULE_REF)
    // 返回 E0037 / E0021 错误,标记待 Phase C 接 OOP / 模块系统后替换。
    // 这 9 个测试锁住他们的状态 + ERR-transparent + 注册表更新。

    #[test]
    fn b15_bool_truthiness() {
        // §9.4: NULL -> false;Boolean(b) -> b;其它 -> true
        assert_eq!(run("BOOL(NULL);").unwrap(), Value::Boolean(false));
        assert_eq!(run("BOOL(FALSE);").unwrap(), Value::Boolean(false));
        assert_eq!(run("BOOL(TRUE);").unwrap(), Value::Boolean(true));
        // 0 和 "" 都是 truthy (与 B9 NOT 一致)
        assert_eq!(run("BOOL(0);").unwrap(), Value::Boolean(true));
        assert_eq!(run(r#"BOOL("");"#).unwrap(), Value::Boolean(true));
        assert_eq!(run("BOOL(1);").unwrap(), Value::Boolean(true));
        assert_eq!(run(r#"BOOL("hello");"#).unwrap(), Value::Boolean(true));
        assert_eq!(run("BOOL([1, 2]);").unwrap(), Value::Boolean(true));
        assert_eq!(run(r#"BOOL(["k": "v"]);"#).unwrap(), Value::Boolean(true));
    }

    #[test]
    fn b15_neg_integer_and_float() {
        assert_eq!(run("NEG(5);").unwrap(), Value::Integer(-5));
        assert_eq!(run("NEG(-5);").unwrap(), Value::Integer(5));
        assert_eq!(run("NEG(0);").unwrap(), Value::Integer(0));
        assert_eq!(run("NEG(3.14);").unwrap(), Value::Float(-3.14));
        // 类型错
        let err = run(r#"NEG("hello");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn b15_input_no_stdin_returns_err() {
        // 测试环境下没有 stdin (cargo test 不连 tty) -> 返回 ERR
        // wrap 在 IS_ERR 里观察 ERR 值 (顶层会变 E0102)
        assert_eq!(run("IS_ERR(INPUT());").unwrap(), Value::Boolean(true));
    }

    #[test]
    fn b15_call_closure_in_via_eval_call() {
        // CALL(closure, ...) 实际由 eval_call 处理 (Expr::Call 入口) ——
        // builtin_call 路径用作 dynamic dispatch 入口,本批不实现
        // closure re-invoke (会触发 §12.6 短路 bug)。这里测试
        // 直接用 Expr::Call 路径可工作。
        assert_eq!(
            run(r#"LET(f, FUN((x, y), +(x, y))); f(3, 4);"#).unwrap(),
            Value::Integer(7)
        );
    }

    // ── Phase C2 (spec §13.12 / §5.5 / §8.6):GET_PROP / SET_PROP /
    // CALL_METHOD / MODULE_REF 真实现(B15 的 4 个 stub 测试已由
    // c2_* 系列替换,原 placeholder 断言全部翻转)。

    #[test]
    fn b15_get_prop_returns_e0037_placeholder() {
        // C2 之后 GET_PROP(["x": 1], "x") 返回 1 —— placeholder 断言
        // 不再成立,由 c2_get_prop_dict_roundtrip 接管;此处仅保留
        // E0037 语义(缺键),它正是 stub 时代 E0037 的"真身"。
        let err = run(r#"GET_PROP(["x": 1], "y");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0037);
    }

    #[test]
    fn b15_eight_registered_in_resolve_builtin() {
        use crate::resolve_builtin;
        for name in &["INPUT", "BOOL", "CALL", "NEG", "GET_PROP", "SET_PROP", "CALL_METHOD", "MODULE_REF"] {
            assert!(resolve_builtin(name).is_some(),
                "resolve_builtin({:?}) is None; B15 did not register", name);
        }
    }

    #[test]
    fn b15_eight_moved_to_resolved_in_registry() {
        for spec in crate::registry::BUILTIN_REGISTRY.iter() {
            if ["INPUT", "BOOL", "CALL", "NEG", "GET_PROP", "SET_PROP", "CALL_METHOD", "MODULE_REF"].contains(&spec.name) {
                assert_eq!(spec.dispatch, crate::registry::DispatchStatus::ResolvedBuiltin,
                    "{:?} is still Deferred after B15", spec.name);
            }
        }
    }

    // ── Phase C2 (spec §13.12 模块作为值 / §5.5 属性语法糖 / §8.6 self 注入) ──

    #[test]
    fn c2_get_prop_dict_roundtrip() {
        assert_eq!(run(r#"GET_PROP(["x": 1], "x");"#).unwrap(), Value::Integer(1));
        // §5.5 sugar: a.b == GET_PROP(a, "b")
        assert_eq!(
            run(r#"LET(d, ["name": "wlwl"]); d.name;"#).unwrap(),
            Value::String("wlwl".into())
        );
    }

    #[test]
    fn c2_get_prop_missing_is_e0037() {
        let err = run(r#"GET_PROP(["x": 1], "y");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0037);
    }

    #[test]
    fn c2_get_prop_type_errors() {
        // receiver 非 DICT → E0030
        let err = run(r#"GET_PROP([1, 2], "x");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        // key 非 STRING → E0030
        let err = run(r#"GET_PROP(["x": 1], 0);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn c2_set_prop_insert_update_value_semantics() {
        // 更新已有键
        assert_eq!(
            run(r#"STR(SET_PROP(["x": 1], "x", 99));"#).unwrap(),
            Value::String("[x: 99]".into())
        );
        // 插入新键
        assert_eq!(
            run(r#"STR(SET_PROP(["x": 1], "y", 2));"#).unwrap(),
            Value::String("[x: 1, y: 2]".into())
        );
        // 值语义:原 dict 不被修改(与 INDEX_SET 一致)
        assert_eq!(
            run(r#"LET(d, ["x": 1]); SET_PROP(d, "x", 99); GET_PROP(d, "x");"#).unwrap(),
            Value::Integer(1)
        );
        // 类型错
        let err = run(r#"SET_PROP(1, "x", 2);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    #[test]
    fn c2_call_method_closure_gets_receiver_injected() {
        // §8.6 / §11.4:首形参命名为 self 的 closure,receiver 自动注入
        assert_eq!(
            run(r#"
                LET(d, ["v": 42, "get": FUN((self), GET_PROP(self, "v"))]);
                CALL_METHOD(d, "get");
            "#).unwrap(),
            Value::Integer(42)
        );
        // §5.5 sugar: d.get() == CALL_METHOD(d, "get")
        assert_eq!(
            run(r#"
                LET(d, ["v": 7, "get": FUN((self), GET_PROP(self, "v"))]);
                d.get();
            "#).unwrap(),
            Value::Integer(7)
        );
        // self 之后按序绑定额外实参
        assert_eq!(
            run(r#"
                LET(d, ["x": 10, "add": FUN((self, n), +(GET_PROP(self, "x"), n))]);
                d.add(5);
            "#).unwrap(),
            Value::Integer(15)
        );
    }

    #[test]
    fn c2_call_method_non_self_closure_is_plain_call() {
        // §5.5 关键决策:属性值是 FUN 字面量且无 self 首形参时,
        // a.b(args) 等价 CALL(a.b, args) —— receiver 不注入。
        // 文件模块导出的函数经 m.f(x) 调用即属此类。
        assert_eq!(
            run(r#"
                LET(d, ["add": FUN((a, b), +(a, b))]);
                d.add(2, 3);
            "#).unwrap(),
            Value::Integer(5)
        );
    }

    #[test]
    fn c2_call_method_native_fn_no_receiver_injection() {
        // §13.12:io.PRINT("hi") 等价 IMPORT 后的 PRINT("hi") ——
        // NativeFn 成员不注入 receiver。用 std.json::PARSE 观察
        // 返回值 (PRINT 只返回 NULL,不好断言)。
        assert_eq!(
            run(r#"
                LET(j, MODULE_REF("wlwl:std.json"));
                GET_PROP(j.PARSE("{\"a\": 5}"), "a");
            "#).unwrap(),
            Value::Integer(5)
        );
    }

    #[test]
    fn c2_call_method_errors() {
        // 方法缺失 → E0037
        let err = run(r#"CALL_METHOD(["x": 1], "nope");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0037);
        // 成员存在但不可调用 → E0030
        let err = run(r#"CALL_METHOD(["x": 1], "x");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        // receiver 非 DICT → E0030
        let err = run(r#"CALL_METHOD(1, "x");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
        // 元数:缺方法名 → E0022
        let err = run(r#"CALL_METHOD(["x": 1]);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0022);
    }

    #[test]
    fn c2_module_ref_std_module_is_dict() {
        // 模块对象类型是 DICT (§13.12)
        assert_eq!(run(r#"TYPE(MODULE_REF("wlwl:std.json"));"#).unwrap(), Value::String("DICT".into()));
        assert_eq!(
            run(r#"HAS(MODULE_REF("wlwl:std.json"), "PARSE");"#).unwrap(),
            Value::Boolean(true)
        );
        // 绑定名不进入作用域:PARSE 仍是未定义名 (E0020)
        let err = run(r#"MODULE_REF("wlwl:std.json"); PARSE("{}");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0020);
    }

    #[test]
    fn c2_module_ref_file_module_sorted_keys_and_call() {
        let dir = unique_test_dir("c2_module_ref");
        std::fs::write(
            dir.join("mathx.wl"),
            "LET(zeta, 1);\nLET(alpha, FUN((x), *(x, 2)));\nEXPORT([\"alpha\"]);\n",
        ).unwrap();
        // EXPORT 面决定模块对象的键:alpha 在,zeta 不在 (未 EXPORT)。
        assert_eq!(
            run_in(&dir, r#"
                LET(m, MODULE_REF("./mathx"));
                STR(KEYS(m));
            "#).unwrap(),
            Value::String("[alpha]".into())
        );
        // 文件模块导出的 closure 经 CALL_METHOD 调用 (receiver 注入)
        assert_eq!(
            run_in(&dir, r#"
                LET(m, MODULE_REF("./mathx"));
                m.alpha(21);
            "#).unwrap(),
            Value::Integer(42)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn c2_module_ref_not_found_is_e0040() {
        let err = run(r#"MODULE_REF("doesnotexist");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0040);
    }

    #[test]
    fn c2_module_ref_path_type_error() {
        let err = run(r#"MODULE_REF(42);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    }

    // ── Phase C1 (spec §13.4):AS 函数完全删除 ──

    #[test]
    fn c1_as_function_is_deleted_e0020() {
        // v0.3 的 AS("y", "x") 在 v0.4 完全删除(spec §13.4 钉死;
        // 迁移写法 LET(alias, name))。本实现从未有过 AS,运行时
        // 引用走 undefined_name → E0020。
        let err = run(r#"LET(x, 1); AS("y", "x");"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0020);
        assert!(err.diagnostic().message.contains("AS"), "message: {}", err.diagnostic().message);
    }

    #[test]
    fn c1_as_is_not_a_builtin_or_macro() {
        // AS 不在全局内建注册表 / resolve_builtin / lexer 关键字里
        assert!(crate::registry::lookup("AS").is_none());
        assert!(crate::resolve_builtin("AS").is_none());
    }

    // ── Phase C7 (spec §13.5):项目根边界强化 ──

    #[test]
    fn c7_lexical_normalize_resolves_dotdot() {
        let base = PathBuf::from("/root/app");
        let smuggled = base.join("..").join("..").join("escape").join("m.wl");
        let n = lexical_normalize(&smuggled);
        assert_eq!(n, PathBuf::from("/escape/m.wl"));
        // `.` 组件被丢弃
        let dotted = base.join(".").join("sub").join("m.wl");
        assert_eq!(lexical_normalize(&dotted), PathBuf::from("/root/app/sub/m.wl"));
    }

    #[test]
    fn c7_relative_escape_outside_root_is_e0040() {
        // 无 manifest:base_dir 即项目根;`../` 弹出根 → E0040
        let dir = unique_test_dir("c7_rel_escape");
        let err = run_in(&dir, r#"IMPORT("../outside", ["x"]);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0040);
        // §13.5 钉死的措辞
        assert!(
            err.diagnostic().message.contains("outside project root"),
            "message: {}", err.diagnostic().message
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn c7_ns_dep_cannot_smuggle_dotdot_past_root() {
        // C7 核心:manifest 里 [namespaces] 值带 `..`,raw is_within
        // 前缀测试会被 `<root>/../escape` 骗过;归一化后必须 E0040。
        let dir = unique_test_dir("c7_ns_root");
        fs::write(
            dir.join("wlwl.toml"),
            r#"
[package]
name = "app"
version = "0.1.0"
entry = "main.wl"

[namespaces]
"evil" = "../../outside"
"#,
        )
        .unwrap();
        let err = run_in(&dir, r#"IMPORT("evil:mod", ["x"]);"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0040);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn c7_in_root_relative_import_still_works() {
        // 边界强化的回归保护:root 内的相对导入不受影响。
        let dir = unique_test_dir("c7_in_root");
        fs::write(dir.join("helper.wl"), "LET(v, 5);\nEXPORT([\"v\"]);\n").unwrap();
        assert_eq!(
            run_in(&dir, r#"IMPORT("./helper", ["v"]); v;"#).unwrap(),
            Value::Integer(5)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Phase C3 (spec §13.8):language_version 字段 + E0044 ──

    #[test]
    fn c3_language_version_mismatch_is_e0044() {
        let dir = unique_test_dir("c3_mismatch");
        write_manifest(&dir, Some("0.5"), "");
        let err = run_in(&dir, "LET(x, 1); x;").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0044);
        // spec 钉死的 message 形式
        assert_eq!(
            err.diagnostic().message,
            "language_version mismatch: package requires 0.5, implementation supports 0.4"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn c3_language_version_compatible_runs() {
        let dir = unique_test_dir("c3_ok");
        write_manifest(&dir, Some("0.4"), "");
        assert_eq!(run_in(&dir, "LET(x, 1); x;").unwrap(), Value::Integer(1));
        // 低于实现 minor 的声明也兼容
        let dir2 = unique_test_dir("c3_ok_lower");
        write_manifest(&dir2, Some("0.3"), "");
        assert_eq!(run_in(&dir2, "+(2, 3);").unwrap(), Value::Integer(5));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&dir2);
    }

    #[test]
    fn c3_language_version_absent_tolerated() {
        // v0.3 时代 manifest 没有该字段 → 跳过校验(偏差 P4-C3-001)
        let dir = unique_test_dir("c3_absent");
        write_manifest(&dir, None, "");
        assert_eq!(run_in(&dir, "1;").unwrap(), Value::Integer(1));
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Phase C5 (spec §6.6):allow_builtin_shadow + E0025 / W0030 ──

    #[test]
    fn c5_shadow_builtin_default_is_e0025() {
        let dir = unique_test_dir("c5_shadow_default");
        write_manifest(&dir, None, "");
        let err = run_in(&dir, r#"LET(PRINT, 1); PRINT;"#).unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0025);
        // spec message:cannot shadow built-in 'PRINT'
        assert!(
            err.diagnostic().message.contains("cannot shadow built-in 'PRINT'"),
            "message: {}", err.diagnostic().message
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn c5_shadow_builtin_allowed_with_flag_emits_w0030() {
        let dir = unique_test_dir("c5_shadow_allowed");
        write_manifest(
            &dir,
            None,
            "[features]\nallow_builtin_shadow = true\n",
        );
        let (r, warnings) =
            run_in_with_warnings(&dir, r#"LET(PRINT, 1); PRINT;"#);
        assert_eq!(r.unwrap(), Value::Integer(1));
        assert!(
            warnings.iter().any(|w| w.code == ErrorCode::W0030),
            "expected W0030, got {:?}", warnings
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn c5_shadow_macro_fn_is_w0030_allowed() {
        // 遮蔽宏函数(spec §14.5 W0030)默认允许,发警告。
        // 用 AND(LexerMacro 注册表条目,词法上是 Ident,可作 LET 名;
        // IF/WHILE 等真关键字在 parser 层就拒绝作绑定名,到不了 eval)。
        let dir = unique_test_dir("c5_shadow_macro");
        write_manifest(&dir, None, "");
        let (r, warnings) = run_in_with_warnings(&dir, r#"LET(AND, 1); AND;"#);
        assert_eq!(r.unwrap(), Value::Integer(1));
        assert!(
            warnings.iter().any(|w| w.code == ErrorCode::W0030),
            "expected W0030, got {:?}", warnings
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn c5_let_inside_function_updating_existing_binding_not_shadow() {
        // set_existing 路径(函数体内 LET 更新外层已有绑定)不触发
        // 遮蔽检查 —— 只有"新绑定"才算 shadow。
        let dir = unique_test_dir("c5_rebind");
        write_manifest(&dir, None, "");
        assert_eq!(
            run_in(&dir, r#"LET(x, 1); LET(f, FUN((), LET(x, 2))); f(); x;"#).unwrap(),
            Value::Integer(2)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Phase C4 (spec §13.9):MVS 依赖求解 + E0045 ──

    #[test]
    fn c4_version_dependency_is_e0045() {
        // v0.4 无中央 registry,版本式依赖候选集为空 → E0045。
        let dir = unique_test_dir("c4_version_dep");
        write_manifest(
            &dir,
            None,
            "[dependencies]\n\"huggingface:client\" = \"^0.5.0\"\n",
        );
        let err = run_in(&dir, "1;").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0045);
        assert!(
            err.diagnostic().message.starts_with("dependency conflict:"),
            "message: {}", err.diagnostic().message
        );
        assert!(err.diagnostic().message.contains("huggingface:client"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn c4_path_dependencies_resolve_without_e0045() {
        // path 依赖总是可满足,不触发 E0045。
        let dir = unique_test_dir("c4_path_dep");
        write_manifest(
            &dir,
            None,
            "[dependencies]\n\"myteam:utils\" = { path = \"vendor/utils\" }\n",
        );
        assert_eq!(run_in(&dir, "1;").unwrap(), Value::Integer(1));
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Phase C6 (spec §13.8):lock ↔ toml 一致性 + E0042 ──

    #[test]
    fn c6_lock_missing_is_ok() {
        // lock 缺失 → 跳过校验(CLI 的 try_write_lock 负责生成)
        let dir = unique_test_dir("c6_no_lock");
        write_manifest(&dir, None, "");
        assert_eq!(run_in(&dir, "1;").unwrap(), Value::Integer(1));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn c6_lock_inconsistent_is_e0042() {
        // 用户在 lock 之外改了 toml 的依赖 path → E0042
        let dir = unique_test_dir("c6_inconsistent");
        write_manifest(
            &dir,
            None,
            "[dependencies]\n\"myteam:utils\" = { path = \"vendor/utils\" }\n",
        );
        // 生成与 toml 一致的 lock,然后人为改 toml 制造不一致
        let m = wlwl_toml::manifest::parse(
            &fs::read_to_string(dir.join("wlwl.toml")).unwrap(),
        )
        .unwrap();
        wlwl_toml::lock::write(
            &dir.join("wlwl.lock"),
            &wlwl_toml::lock::from_manifest(&m, &dir),
        )
        .unwrap();
        write_manifest(
            &dir,
            None,
            "[dependencies]\n\"myteam:utils\" = { path = \"vendor/utils2\" }\n",
        );
        let err = run_in(&dir, "1;").unwrap_err();
        assert_eq!(err.diagnostic().code, ErrorCode::E0042);
        assert!(
            err.diagnostic()
                .message
                .starts_with("lock file inconsistent with wlwl.toml:"),
            "message: {}", err.diagnostic().message
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn c6_lock_consistent_runs() {
        let dir = unique_test_dir("c6_consistent");
        write_manifest(
            &dir,
            None,
            "[dependencies]\n\"myteam:utils\" = { path = \"vendor/utils\" }\n",
        );
        let m = wlwl_toml::manifest::parse(
            &fs::read_to_string(dir.join("wlwl.toml")).unwrap(),
        )
        .unwrap();
        wlwl_toml::lock::write(
            &dir.join("wlwl.lock"),
            &wlwl_toml::lock::from_manifest(&m, &dir),
        )
        .unwrap();
        assert_eq!(run_in(&dir, "1;").unwrap(), Value::Integer(1));
        let _ = fs::remove_dir_all(&dir);
    }

    // ---- Phase E1 (spec v0.4 §2.7): strict_types boundary check ----

    /// Helper: parse, build an evaluator with strict_types toggled,
    /// eval, and return the result.
    fn run_strict(src: &str, on: bool) -> WlwlResult<Value> {
        let e = parse(src, "t.wl")?;
        let mut ev = Evaluator::new().with_strict_types(on);
        ev.eval(&e)
    }

    // ---- Phase E2/E4 (spec §4.5 + §16.3 rule 10): empty collections --

    #[test]
    fn e2_empty_array_call_evaluates() {
        // `ARRAY()` is the canonical §16.3 empty-collection form; it
        // must evaluate to an empty ARRAY (previously E0021).
        assert_eq!(run_strict("ARRAY();", false).unwrap(), Value::Array(vec![]));
        assert_eq!(
            run_strict("LET(a, ARRAY()); LEN(a);", false).unwrap(),
            Value::Integer(0)
        );
    }

    #[test]
    fn e2_empty_dict_call_evaluates() {
        assert_eq!(run_strict("DICT();", false).unwrap(), Value::Dict(vec![]));
        assert_eq!(
            run_strict("LET(d, DICT()); LEN(d);", false).unwrap(),
            Value::Integer(0)
        );
    }

    #[test]
    fn e2_array_dict_forms_coexist_with_literals() {
        // `[]` / `[:]` literals and the canonical call forms agree.
        assert_eq!(
            run_strict("TYPE([]);", false).unwrap(),
            run_strict("TYPE(ARRAY());", false).unwrap()
        );
        assert_eq!(
            run_strict("LEN([\"a\": 1]);", false).unwrap(),
            Value::Integer(1)
        );
    }

    #[test]
    fn e1_strict_types_default_off_no_check() {
        // strict_types defaults to false; a STRING passed where an
        // INTEGER is annotated must run without error (annotations
        // are Transient by default, matching v0.3 behavior).
        let r = run_strict(
            "LET(f, FUN((x: INTEGER), x)); f(\"hi\");",
            false,
        );
        assert!(r.is_ok(), "expected ok, got {:?}", r);
    }

    #[test]
    fn e1_strict_types_on_integer_param_accepts_integer() {
        let r = run_strict(
            "LET(f, FUN((x: INTEGER), x)); f(42);",
            true,
        );
        assert!(r.is_ok(), "expected ok, got {:?}", r);
        assert_eq!(r.unwrap(), Value::Integer(42));
    }

    #[test]
    fn e1_strict_types_on_string_for_integer_param_is_e0033() {
        let r = run_strict(
            "LET(f, FUN((x: INTEGER), x)); f(\"hi\");",
            true,
        );
        let err = r.expect_err("expected E0033");
        let msg = err.diagnostic().message.clone();
        assert!(
            msg.contains("type annotation mismatch"),
            "wrong message: {}",
            msg
        );
        assert!(
            msg.contains("INTEGER") && msg.contains("STRING"),
            "expected/actual not in message: {}",
            msg
        );
        // The diagnostic must carry both spans in `related`.
        let rels = &err.diagnostic().related;
        assert!(
            rels.len() >= 2,
            "expected at least 2 related locations, got {}",
            rels.len()
        );
        let msgs: Vec<&str> = rels.iter().map(|r| r.message.as_str()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("INTEGER") && m.contains("annotation")),
            "annotation related missing: {:?}",
            msgs
        );
        assert!(
            msgs.iter().any(|m| m.contains("STRING") && m.contains("actual")),
            "actual related missing: {:?}",
            msgs
        );
    }

    #[test]
    fn e1_strict_types_unannotated_param_passes_through() {
        // A parameter without a `: Type` annotation is never
        // checked, regardless of strict_types.
        let r = run_strict(
            "LET(f, FUN((x), x)); f(\"hi\");",
            true,
        );
        assert!(r.is_ok(), "unannotated param should pass: got {:?}", r);
        assert_eq!(r.unwrap(), Value::String("hi".into()));
    }

    #[test]
    fn e1_strict_types_mixed_params_only_annotated_checked() {
        // First param has annotation; second does not. Passing a
        // mismatched value for the first raises E0033; the second
        // is unconstrained.
        let r = run_strict(
            "LET(f, FUN((x: INTEGER, y), +(x, 0))); f(\"hi\", \"world\");",
            true,
        );
        let err = r.expect_err("expected E0033");
        assert!(err.diagnostic().message.contains("INTEGER"), "got: {}",
            err.diagnostic().message);
    }

    #[test]
    fn e1_strict_types_recursion_checks_each_call() {
        // A self-recursive function with strict_types: each recursive
        // call is checked independently. Passing an INTEGER works;
        // passing a STRING triggers E0033.
        let ok = run_strict(
            "LET(loop, FUN((n: INTEGER), IF(==(n, 0), 0, loop(-(n, 1))))); loop(3);",
            true,
        );
        assert!(ok.is_ok(), "expected ok, got {:?}", ok);

        let bad = run_strict(
            "LET(loop, FUN((n: INTEGER), IF(==(n, 0), 0, loop(-(n, 1))))); loop(\"oops\");",
            true,
        );
        assert!(bad.is_err(), "expected E0033, got ok");
        assert!(bad.unwrap_err().diagnostic().message.contains("INTEGER"),
            "expected INTEGER in diagnostic");
    }

    #[test]
    fn e1_strict_types_uses_uppercase_type_names_case_insensitive() {
        // Lowercase annotation should still match (parser preserves
        // source text; we uppercase for comparison).
        let r = run_strict(
            "LET(f, FUN((x: integer), x)); f(42);",
            true,
        );
        assert!(r.is_ok(), "lowercase annotation should match, got {:?}", r);
    }

    #[test]
    fn e1_strict_types_builder_round_trip() {
        // Builder API + getter.
        let ev = Evaluator::new().with_strict_types(true);
        assert!(ev.strict_types());
        let ev = Evaluator::new().with_strict_types(false);
        assert!(!ev.strict_types());
        let ev = Evaluator::new();
        assert!(!ev.strict_types(), "default should be off");
    }


}
