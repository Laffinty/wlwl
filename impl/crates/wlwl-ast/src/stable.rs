//! Stable AST node IDs + content hashes (spec v0.4 §16.4.1, Phase E3).
//!
//! For AI multi-round editing the AST is exposed as a tree where every
//! node carries:
//!
//! - `node_id`: a stable *semantic path* that does NOT depend on line
//!   numbers, so it survives reformatting and line shifts.
//! - `parent_id`: the parent's `node_id`.
//! - `kind`: the `Expr` variant name (`"Call"`, `"Let"`, ...).
//! - `span`: the node's source span (for locating the node today).
//! - `hash`: SHA-256 (`sha256:<hex>`) over the span-stripped serde
//!   serialization of the subtree; any content change changes the
//!   hash, which is what concurrent-edit conflict detection needs.
//!
//! `node_id` scheme: `{module}:fn:{fn}/body/{label}:{sibling}`...
//! - `module`: the module name the AST was parsed from (the CLI uses
//!   the source file path).
//! - `fn`: innermost enclosing named function; `<anon>` for anonymous
//!   `FUN` literals; `<top>` outside any function.
//! - path segments: role labels (`cond`, `then`, `value`, `arg`,
//!   `stmt`, ...) plus the sibling ordinal, purely structural.
//!
//! Patterns (destructuring / MATCH) are not `Expr` nodes; the `Expr`
//! payloads they contain (e.g. dict-pattern keys) are still visited
//! as children.

use serde::Serialize;
use serde_json::Value;

use crate::{Expr, Pattern, Span};

/// A serialized AST node with stable identity (§16.4.1 example shape:
/// `node_id` / `parent_id` / `kind` / `span` / `hash` / children).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StableNode {
    pub node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub kind: String,
    pub span: Span,
    /// `"sha256:<hex>"` over the span-stripped subtree JSON.
    pub hash: String,
    pub children: Vec<StableNode>,
}

/// Build the stable-ID tree for a parsed program.
///
/// `module` is the module name (the CLI passes the source file path).
pub fn stable_tree(root: &Expr, module: &str) -> StableNode {
    build(root, None, &format!("{}:fn:<top>/body", module))
}

/// One direct `Expr` child edge.
struct Edge<'a> {
    label: &'static str,
    expr: &'a Expr,
    /// `Some(fun_name)` when the edge crosses a `FUN` body boundary:
    /// the child's path embeds a `fn:{name}` segment (`<anon>` for
    /// anonymous `FUN` literals), so descendants are addressable as
    /// `.../fn:NAME/body:N/...`.
    fn_segment: Option<String>,
}

fn edge<'a>(label: &'static str, e: &'a Expr) -> Edge<'a> {
    Edge {
        label,
        expr: e,
        fn_segment: None,
    }
}

fn fun_body_edge<'a>(name: &Option<String>, body: &'a Expr) -> Edge<'a> {
    Edge {
        label: "body",
        expr: body,
        fn_segment: Some(name.clone().unwrap_or_else(|| "<anon>".to_string())),
    }
}

/// Recursively build one node. `path` IS the node's id (the parent's
/// id plus this node's segment); children extend it.
fn build(e: &Expr, parent_id: Option<String>, path: &str) -> StableNode {
    let mut children = Vec::new();
    for (sibling, fe) in child_edges(e).into_iter().enumerate() {
        let child_path = match &fe.fn_segment {
            // FUN body: embed the enclosing-function segment. The full
            // structural prefix is preserved, so same-named functions
            // in different scopes never collide.
            Some(name) => format!("{}/fn:{}/body:{}", path, name, sibling),
            None => format!("{}/{}:{}", path, fe.label, sibling),
        };
        children.push(build(fe.expr, Some(path.to_string()), &child_path));
    }

    StableNode {
        node_id: path.to_string(),
        parent_id,
        kind: kind_of(e).to_string(),
        span: e.span().clone(),
        hash: format!("sha256:{}", content_hash(e)),
        children,
    }
}

/// Enumerate the direct `Expr` children of a node, in source order.
fn child_edges(e: &Expr) -> Vec<Edge<'_>> {
    match e {
        Expr::Literal(_, _) | Expr::Var(_, _) | Expr::Break { .. } | Expr::Continue { .. } => {
            Vec::new()
        }
        Expr::Call { args, .. } => args.iter().map(|a| edge("arg", a)).collect(),
        Expr::Block { exprs, .. } => exprs.iter().map(|s| edge("stmt", s)).collect(),
        Expr::Array { items, .. } => items.iter().map(|i| edge("item", i)).collect(),
        Expr::Dict { entries, .. } => {
            let mut out = Vec::new();
            for (k, v) in entries {
                out.push(edge("dict-key", k));
                out.push(edge("dict-value", v));
            }
            out
        }
        Expr::Let { value, .. } => vec![edge("value", value)],
        Expr::LetPattern { pattern, value, .. } => {
            let mut out = vec![edge("value", value)];
            if let Pattern::Dict(entries, _) = pattern.as_ref() {
                for (k, _) in entries {
                    out.push(edge("pattern-key", k));
                }
            }
            out
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            let mut out = vec![edge("cond", cond), edge("then", then_branch)];
            if let Some(el) = else_branch {
                out.push(edge("else", el));
            }
            out
        }
        Expr::While { cond, body, .. } => vec![edge("cond", cond), edge("body", body)],
        Expr::For { iter, body, .. } => vec![edge("iter", iter), edge("body", body)],
        Expr::Return { value, .. } => match value {
            Some(v) => vec![edge("value", v)],
            None => Vec::new(),
        },
        Expr::Fun {
            name, params, body, ..
        } => {
            let mut out = Vec::new();
            for p in params {
                if let Some(d) = &p.default_expr {
                    out.push(edge("param-default", d));
                }
            }
            out.push(fun_body_edge(name, body));
            out
        }
        Expr::Ok { value, .. }
        | Expr::Err { value, .. }
        | Expr::Panic { value, .. }
        | Expr::Try { value, .. }
        | Expr::IsOk { value, .. }
        | Expr::IsErr { value, .. } => {
            vec![edge("value", value)]
        }
        Expr::OrDie { value, default, .. } => {
            vec![edge("value", value), edge("default", default)]
        }
        Expr::Match {
            value,
            clauses,
            default,
            ..
        } => {
            let mut out = vec![edge("value", value)];
            for c in clauses {
                out.push(edge("clause-body", &c.body));
                if let Pattern::Dict(entries, _) = &c.pattern {
                    for (k, _) in entries {
                        out.push(edge("pattern-key", k));
                    }
                }
            }
            out.push(edge("default", default));
            out
        }
        Expr::Import { .. } | Expr::Export { .. } => Vec::new(),
    }
}

fn kind_of(e: &Expr) -> &'static str {
    match e {
        Expr::Literal(_, _) => "Literal",
        Expr::Var(_, _) => "Var",
        Expr::Call { .. } => "Call",
        Expr::Block { .. } => "Block",
        Expr::Array { .. } => "Array",
        Expr::Dict { .. } => "Dict",
        Expr::Let { .. } => "Let",
        Expr::LetPattern { .. } => "LetPattern",
        Expr::If { .. } => "If",
        Expr::While { .. } => "While",
        Expr::For { .. } => "For",
        Expr::Return { .. } => "Return",
        Expr::Break { .. } => "Break",
        Expr::Continue { .. } => "Continue",
        Expr::Fun { .. } => "Fun",
        Expr::Ok { .. } => "Ok",
        Expr::Err { .. } => "Err",
        Expr::Panic { .. } => "Panic",
        Expr::Try { .. } => "Try",
        Expr::IsOk { .. } => "IsOk",
        Expr::IsErr { .. } => "IsErr",
        Expr::OrDie { .. } => "OrDie",
        Expr::Match { .. } => "Match",
        Expr::Import { .. } => "Import",
        Expr::Export { .. } => "Export",
    }
}

/// SHA-256 over the span-stripped serde serialization of the node
/// (subtree-inclusive: changing any descendant changes the hash).
fn content_hash(e: &Expr) -> String {
    let mut v = serde_json::to_value(e).expect("Expr is always serializable");
    strip_spans(&mut v);
    let canonical = serde_json::to_string(&v).expect("Value re-serialization");
    crate::sha256::sha256_hex(canonical.as_bytes())
}

/// Remove span payloads so the hash reflects content, not layout:
/// spans appear under the named key `span` and as positional enum
/// payloads with the 5-key shape.
fn strip_spans(v: &mut Value) {
    match v {
        Value::Object(map) => {
            if map.len() == 5
                && map.contains_key("file")
                && map.contains_key("line_start")
                && map.contains_key("col_start")
                && map.contains_key("line_end")
                && map.contains_key("col_end")
            {
                *v = Value::Null;
                return;
            }
            map.remove("span");
            for (_, child) in map.iter_mut() {
                strip_spans(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_spans(item);
            }
        }
        _ => {}
    }
}
