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

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::path::Path;
use std::sync::Arc;

use wlwl_ast::{Expr, FunParam, ImportName, Literal, Span};
use wlwl_error::{
    extract_line, ErrorCode, Location, Suggestion, WlwlDiagnostic, WlwlError, WlwlResult,
};
use wlwl_std;

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
                    if !is_within(&file_path, &self.project.project_root) {
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
            if !is_within(&file_path, &self.project.project_root) {
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
        WlwlDiagnostic::new(
            ErrorCode::E0040,
            format!(
                "module '{}' is outside the project root ({})",
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
    /// Per-evaluator context for `wlwl_std` calls (argv, env). Set to
    /// `wlwl_std::StdCtx::from_process()` in `new`; tests can override
    /// via the `std_ctx` field directly.
    std_ctx: wlwl_std::StdCtx,
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
            std_ctx: wlwl_std::StdCtx::default(),
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
            if let Value::NativeFn { invoke, .. } = v {
                return match invoke {
                    NativeInvoke::Std(f) => invoke_std(self, f, arg_values, span),
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
            return b(self, arg_values);
        }
        Err(self.undefined_name(name, span))
    }

    fn invoke_closure(
        &mut self,
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
            self.env.set_local(p.name.clone(), v);
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

    fn undefined_name(&self, name: &str, span: &Span) -> WlwlError {
        let loc = Location {
            file: span.file.clone(),
            line: span.line_start,
            col: span.col_start,
            line_end: span.line_end,
            col_end: span.col_end,
        };
        let mut d = WlwlDiagnostic::new(
            ErrorCode::E0020,
            format!("undefined name `{}`", name),
            loc,
        );
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
        assert_eq!(v["error_schema_version"], "0.3.1");
        assert_eq!(v["code"], "E0020");
        assert_eq!(v["error_category"], "name");
        assert_eq!(v["retryable"], false);
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
        assert!(versions.contains("0.3.1"));
    }

    #[test]
    fn ai_contract_required_fields_present() {
        let err = run("PRINT(zzz);").unwrap_err();
        let v: serde_json::Value = serde_json::from_str(&err.diagnostic().render_jsonl()).unwrap();
        for key in &["error_schema_version", "code", "error_category", "severity", "message", "location", "retryable", "suggestion_code", "related"] {
            assert!(v.get(*key).is_some(), "missing required key: {}", key);
        }
        let loc = &v["location"];
        for k in &["file", "line", "col"] {
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
        let dir = unique_test_dir("ns_resolve");
        let dep_dir = dir.join("..").join("wlwl_test_dep");
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
"myteam:utils" = {{ path = "../wlwl_test_dep" }}
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
        let _ = std::fs::remove_dir_all(&dep_dir);
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
}
