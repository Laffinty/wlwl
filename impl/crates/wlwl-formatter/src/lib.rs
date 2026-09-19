//! WLWL canonical formatter (spec v0.4 §16.3, Phase E2).
//!
//! Strategy: **parse → AST → rebuild** (never text rewriting), so the
//! output is a pure function of the AST and `fmt(fmt(x)) == fmt(x)`
//! holds by construction as long as every rendered form re-parses to
//! the same AST (up to spans). Each rendering choice below cites the
//! §16.3 rule it implements.
//!
//! The 10 canonical rules (§16.3):
//! 1. 4-space indent, no tabs.
//! 2. ≤ 100 columns; over-long calls fold one argument per line at +8.
//! 3. `;` required between statements; no empty statements; the last
//!    expression of a block carries no `;` (§5.3 decision).
//! 4. Commas required between arguments; never a trailing comma.
//! 5. `"` string boundaries; non-ASCII allowed; no string templates.
//! 6. 1 space around operators; none before `,` `;`, one after;
//!    no space padding inside `(` `)`.
//! 7. Multi-statement blocks: one statement per line; last expression
//!    gets no `;`.
//! 8. Every `IMPORT` / `EXPORT` on its own line; never merged.
//! 9. `CLASS(name, parent, [...])` folds members one per line.
//! 10. Empty collections render `ARRAY()` / `DICT()`, never `[]`.
//!
//! Known limitation (deviations P4-E2-001): the AST does not carry
//! comments, so a canonical rebuild drops them. `wlwl fmt` therefore
//! prints to stdout and never writes files in place.

use wlwl_ast::{Expr, FunParam, Literal, Pattern, TypeAnnotation, TypeExpr};

/// Maximum line length (§16.3 rule 2).
const MAX_LINE: usize = 100;
/// Indentation unit (§16.3 rule 1).
const INDENT: usize = 4;
/// Extra indent when a call folds one argument per line (§16.3 rule 2).
const FOLD_INDENT: usize = 8;

/// Format a parsed program (or any expression) into canonical source
/// text. The result ends with exactly one `\n` (empty input renders
/// as an empty string).
pub fn format(expr: &Expr) -> String {
    // An empty source file parses to a single `Null` literal
    // (`parse_block` materializes it for zero statements); render it
    // back as an empty file instead of the text `NULL`.
    if matches!(expr, Expr::Literal(Literal::Null, _)) && is_synthetic_null_root(expr) {
        return String::new();
    }
    match expr {
        // Top-level block: statements on their own lines (rules 3, 7).
        Expr::Block { exprs, .. } => {
            let mut out = String::new();
            for (i, e) in exprs.iter().enumerate() {
                out.push_str(&render(e, 0));
                if i + 1 < exprs.len() {
                    out.push_str(";\n");
                }
            }
            out.push('\n');
            out
        }
        other => format!("{}\n", render(other, 0)),
    }
}

/// The synthesized root of an empty program is a `Null` literal whose
/// span starts at 0:0. A user-written `NULL;` also produces a bare
/// `Null`, but distinguishing them is impossible on the AST alone —
/// treat any top-level bare `NULL` as the empty program (both round-
/// trip to the same AST, so idempotency holds either way).
fn is_synthetic_null_root(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal(Literal::Null, _))
}

// ── inline rendering ────────────────────────────────────────────────

/// Render `e` on a single line. Returns `None` when the subtree
/// contains a multi-statement block, which has no inline form.
fn render_inline(e: &Expr) -> Option<String> {
    Some(match e {
        Expr::Literal(lit, _) => render_literal(lit),
        Expr::Var(name, _) => name.clone(),
        Expr::Call { name, args, .. } => {
            let parts = render_args_inline(args)?;
            format!("{}({})", name, parts.join(", "))
        }
        Expr::Block { exprs, .. } => {
            // Mirrors `parse_block`: a single-statement block parses
            // back to the bare expression, so inline it; longer blocks
            // have no inline form.
            if exprs.len() == 1 {
                return render_inline(&exprs[0]);
            }
            return None;
        }
        Expr::Array { items, .. } => {
            if items.is_empty() {
                // Rule 10: empty collections never render `[]`.
                "ARRAY()".to_string()
            } else {
                let parts = render_args_inline(items)?;
                format!("[{}]", parts.join(", "))
            }
        }
        Expr::Dict { entries, .. } => {
            if entries.is_empty() {
                // Rule 10.
                "DICT()".to_string()
            } else {
                let mut parts = Vec::new();
                for (k, v) in entries {
                    parts.push(format!("{}: {}", render_inline(k)?, render_inline(v)?));
                }
                format!("[{}]", parts.join(", "))
            }
        }
        Expr::Let {
            name,
            type_annotation,
            value,
            ..
        } => {
            let ann = type_annotation.as_ref().map(type_ann_text);
            let v = render_inline(value)?;
            match ann {
                Some(a) => format!("LET({}: {}, {})", name, a, v),
                None => format!("LET({}, {})", name, v),
            }
        }
        Expr::LetPattern {
            pattern,
            type_annotation,
            value,
            ..
        } => {
            let ann = type_annotation.as_ref().map(type_ann_text);
            let p = render_pattern_inline(pattern)?;
            let v = render_inline(value)?;
            match ann {
                Some(a) => format!("LET({}: {}, {})", p, a, v),
                None => format!("LET({}, {})", p, v),
            }
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            let c = render_inline(cond)?;
            let t = render_inline(then_branch)?;
            match else_branch {
                Some(e) => format!("IF({}, {}, {})", c, t, render_inline(e)?),
                None => format!("IF({}, {})", c, t),
            }
        }
        Expr::While { cond, body, .. } => {
            format!("WHILE({}, {})", render_inline(cond)?, render_inline(body)?)
        }
        Expr::For {
            var, iter, body, ..
        } => {
            format!(
                "FOR({}, {}, {})",
                var,
                render_inline(iter)?,
                render_inline(body)?
            )
        }
        Expr::Return { value, .. } => match value {
            Some(v) => format!("RETURN({})", render_inline(v)?),
            None => "RETURN()".to_string(),
        },
        Expr::Break { .. } => "BREAK()".to_string(),
        Expr::Continue { .. } => "CONTINUE()".to_string(),
        Expr::Fun {
            name,
            params,
            return_type,
            body,
            ..
        } => {
            let ps = render_params_inline(params)?;
            let head = match name {
                Some(n) => format!("FUN({}({})", n, ps),
                None => format!("FUN(({})", ps),
            };
            let head = match return_type {
                Some(rt) => format!("{}: {}", head, type_ann_text(rt)),
                None => head,
            };
            format!("{}, {})", head, render_inline(body)?)
        }
        Expr::Ok { value, .. } => format!("OK({})", render_inline(value)?),
        Expr::Err { value, .. } => format!("ERR({})", render_inline(value)?),
        Expr::Panic { value, .. } => format!("PANIC({})", render_inline(value)?),
        Expr::Try { value, .. } => format!("TRY({})", render_inline(value)?),
        Expr::IsOk { value, .. } => format!("IS_OK({})", render_inline(value)?),
        Expr::IsErr { value, .. } => format!("IS_ERR({})", render_inline(value)?),
        Expr::OrDie { value, default, .. } => {
            format!(
                "OR_DIE({}, {})",
                render_inline(value)?,
                render_inline(default)?
            )
        }
        Expr::Match {
            value,
            clauses,
            default,
            ..
        } => {
            let cs = render_clauses_inline(clauses)?;
            format!(
                "MATCH({}, [{}], {})",
                render_inline(value)?,
                cs,
                render_inline(default)?
            )
        }
        Expr::Import { path, names, .. } => {
            format!("IMPORT({}, [{}])", quote(path), render_names_inline(names)?)
        }
        Expr::Export { names, .. } => {
            format!("EXPORT([{}])", render_names_inline(names)?)
        }
    })
}

fn render_args_inline(args: &[Expr]) -> Option<Vec<String>> {
    args.iter().map(render_inline).collect()
}

fn render_params_inline(params: &[FunParam]) -> Option<String> {
    let mut parts = Vec::new();
    for p in params {
        let mut s = String::new();
        if p.is_rest {
            s.push('*');
        }
        s.push_str(&p.name);
        if let Some(ann) = &p.type_annotation {
            s.push_str(": ");
            s.push_str(&type_ann_text(ann));
        }
        if let Some(d) = &p.default_expr {
            s.push_str(" = ");
            s.push_str(&render_inline(d)?);
        }
        parts.push(s);
    }
    Some(parts.join(", "))
}

fn render_clauses_inline(clauses: &[wlwl_ast::MatchClause]) -> Option<String> {
    let mut parts = Vec::new();
    for c in clauses {
        parts.push(format!(
            "[{}, {}]",
            render_pattern_inline(&c.pattern)?,
            render_inline(&c.body)?
        ));
    }
    Some(parts.join(", "))
}

fn render_names_inline(names: &[wlwl_ast::ImportName]) -> Option<String> {
    let mut parts = Vec::new();
    for n in names {
        match &n.alias {
            Some(a) => parts.push(format!("{}: {}", quote(&n.name), quote(a))),
            None => parts.push(quote(&n.name)),
        }
    }
    Some(parts.join(", "))
}

fn render_pattern_inline(p: &Pattern) -> Option<String> {
    Some(match p {
        Pattern::Ident(name, _) => name.clone(),
        Pattern::Wildcard(_) => "_".to_string(),
        Pattern::Literal(lit, _) => render_literal(lit),
        Pattern::Array(items, rest, _) => {
            let mut parts: Vec<String> = Vec::new();
            for i in items {
                parts.push(render_pattern_inline(i)?);
            }
            if let Some(r) = rest {
                parts.push(format!("*{}", render_pattern_inline(r)?));
            }
            format!("[{}]", parts.join(", "))
        }
        Pattern::Dict(entries, _) => {
            let mut parts = Vec::new();
            for (k, v) in entries {
                parts.push(format!(
                    "{}: {}",
                    render_inline(k)?,
                    render_pattern_inline(v)?
                ));
            }
            format!("[{}]", parts.join(", "))
        }
        Pattern::Constructor { name, inner, .. } => {
            format!("{}({})", name, render_pattern_inline(inner)?)
        }
    })
}

// ── literal / type rendering ────────────────────────────────────────

fn render_literal(lit: &Literal) -> String {
    match lit {
        Literal::Integer(n) => n.to_string(),
        Literal::Float(f) => render_float(*f),
        Literal::String(s) => quote(s),
        Literal::Boolean(true) => "TRUE".to_string(),
        Literal::Boolean(false) => "FALSE".to_string(),
        Literal::Null => "NULL".to_string(),
    }
}

/// Floats must re-parse as floats: `1.0` must never render as `1`
/// (the lexer would read it back as an INTEGER). Rust's `{}` already
/// round-trips the significand; we only need to guarantee a `.` or
/// exponent is present.
fn render_float(f: f64) -> String {
    let s = format!("{}", f);
    if s.contains('.') || s.contains('e') || s.contains('E') {
        s
    } else {
        format!("{}.0", s)
    }
}

/// Escape a string for a `"`-bounded literal (§16.3 rule 5). The
/// escape set matches the lexer exactly (`\n \t \r \\ \" \0`,
/// see `wlwl-lexer::read_string`), so quoting is an exact inverse
/// of lexing and non-ASCII passes through unescaped.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\0' => out.push_str("\\0"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// Canonical type-annotation text. The AST's `text` field preserves
/// the raw source spelling (whitespace as typed), so we rebuild from
/// the structured `TypeExpr` instead. `[]` is the canonical bracket
/// form (spec §2.4 examples, strict_types tests).
fn type_ann_text(ann: &TypeAnnotation) -> String {
    type_expr_text(&ann.expr)
}

fn type_expr_text(t: &TypeExpr) -> String {
    match t {
        TypeExpr::Ident { name, .. } => name.clone(),
        TypeExpr::Array { element, .. } => {
            format!("ARRAY[{}]", type_expr_text(element))
        }
        TypeExpr::Generic { name, args, .. } => {
            let parts: Vec<String> = args.iter().map(type_expr_text).collect();
            format!("{}[{}]", name, parts.join(", "))
        }
    }
}

// ── full rendering (with folding) ───────────────────────────────────

/// Render `e` starting at column `indent` (in spaces). May span
/// multiple lines; never ends with a newline.
pub fn render(e: &Expr, indent: usize) -> String {
    // Rule 9 routes around the inline attempt: CLASS members always
    // fold one per line, however short the class.
    if let Expr::Call { name, args, .. } = e {
        if name == "CLASS" {
            if let Some(out) = render_class(name, args, indent) {
                return out;
            }
        }
    }
    // Try the inline form first; fold when it does not exist (multi-
    // statement block inside) or would overflow the line budget.
    if let Some(inline) = render_inline(e) {
        if indent + inline.len() <= MAX_LINE {
            return inline;
        }
    }
    match e {
        Expr::Block { exprs, .. } => render_block_body(exprs, indent),
        Expr::Call { name, args, .. } => {
            render_folded(name, &args.iter().collect::<Vec<_>>(), indent)
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            let mut args = vec![cond.as_ref(), then_branch.as_ref()];
            if let Some(e) = else_branch {
                args.push(e);
            }
            render_folded("IF", &args, indent)
        }
        Expr::While { cond, body, .. } => {
            render_folded("WHILE", &[cond.as_ref(), body.as_ref()], indent)
        }
        Expr::For {
            var, iter, body, ..
        } => {
            let head = Expr::Var(var.clone(), wlwl_ast::Span::dummy());
            render_folded("FOR", &[&head, iter.as_ref(), body.as_ref()], indent)
        }
        Expr::Fun {
            name,
            params,
            return_type,
            body,
            ..
        } => render_fun(name.as_deref(), params, return_type.as_ref(), body, indent),
        Expr::Match {
            value,
            clauses,
            default,
            ..
        } => render_match(value, clauses, default, indent),
        Expr::Let {
            name,
            type_annotation,
            value,
            ..
        } => {
            let head = match type_annotation {
                Some(a) => format!("LET({}: {}", name, type_ann_text(a)),
                None => format!("LET({}", name),
            };
            render_folded_head(&head, &[value.as_ref()], indent)
        }
        Expr::LetPattern {
            pattern,
            type_annotation,
            value,
            ..
        } => {
            let head = match type_annotation {
                Some(a) => format!(
                    "LET({}: {}",
                    render_pattern_inline(pattern).unwrap_or_default(),
                    type_ann_text(a)
                ),
                None => format!("LET({}", render_pattern_inline(pattern).unwrap_or_default()),
            };
            render_folded_head(&head, &[value.as_ref()], indent)
        }
        Expr::Return { value, .. } => match value {
            Some(v) => render_folded("RETURN", &[v.as_ref()], indent),
            None => "RETURN()".to_string(),
        },
        Expr::OrDie { value, default, .. } => {
            render_folded("OR_DIE", &[value.as_ref(), default.as_ref()], indent)
        }
        Expr::Ok { value, .. } => render_folded("OK", &[value.as_ref()], indent),
        Expr::Err { value, .. } => render_folded("ERR", &[value.as_ref()], indent),
        Expr::Panic { value, .. } => render_folded("PANIC", &[value.as_ref()], indent),
        Expr::Try { value, .. } => render_folded("TRY", &[value.as_ref()], indent),
        Expr::IsOk { value, .. } => render_folded("IS_OK", &[value.as_ref()], indent),
        Expr::IsErr { value, .. } => render_folded("IS_ERR", &[value.as_ref()], indent),
        _ => {
            // No multiline form for this node (e.g. a very long literal
            // or LET). Emit the inline text even if it overflows —
            // rule 2's folding contract only covers call-shaped nodes,
            // and splitting them would break re-parsing.
            render_inline(e).unwrap_or_default()
        }
    }
}

/// Multi-statement block body: one statement per line, `;` after
/// every statement except the last (rules 3, 7). Contract (shared by
/// all `render` entry points): the returned text's FIRST line is not
/// indented (the caller places it), every continuation line is fully
/// indented to its own column.
fn render_block_body(exprs: &[Expr], indent: usize) -> String {
    let pad = " ".repeat(indent);
    let mut out = String::new();
    for (i, e) in exprs.iter().enumerate() {
        out.push_str(&render(e, indent));
        if i + 1 < exprs.len() {
            out.push_str(";\n");
            out.push_str(&pad);
        }
    }
    out
}

/// Fold a call-shaped node: one argument per line at `indent + 8`
/// (§16.3 rule 2), no trailing comma (rule 4), 1 space after `,`
/// is provided by the newline (rule 6).
fn render_folded(name: &str, args: &[&Expr], indent: usize) -> String {
    let inner = indent + FOLD_INDENT;
    let mut out = format!("{}(\n", name);
    for (i, a) in args.iter().enumerate() {
        out.push_str(&" ".repeat(inner));
        out.push_str(&render(a, inner));
        if i + 1 < args.len() {
            out.push_str(",\n");
        }
    }
    out.push('\n');
    out.push_str(&" ".repeat(indent));
    out.push(')');
    out
}

/// Like `render_folded`, but the opening line is pre-built without
/// its trailing comma (used when the head carries non-`Expr`
/// payload, e.g. `LET(x: T`); the comma is added here.
fn render_folded_head(head: &str, args: &[&Expr], indent: usize) -> String {
    let inner = indent + FOLD_INDENT;
    let mut out = format!("{},\n", head);
    for (i, a) in args.iter().enumerate() {
        out.push_str(&" ".repeat(inner));
        out.push_str(&render(a, inner));
        if i + 1 < args.len() {
            out.push_str(",\n");
        }
    }
    out.push('\n');
    out.push_str(&" ".repeat(indent));
    out.push(')');
    out
}

/// `FUN((params): RET, body)` / `FUN(name(params): RET, body)`,
/// folding the parameter list and body as two "arguments" when the
/// inline form does not fit or the body is a multi-statement block.
fn render_fun(
    name: Option<&str>,
    params: &[FunParam],
    return_type: Option<&TypeAnnotation>,
    body: &Expr,
    indent: usize,
) -> String {
    let inner = indent + FOLD_INDENT;
    let head = match name {
        Some(n) => format!("FUN({}(", n),
        None => "FUN((".to_string(),
    };

    // Inline attempt.
    let ps = render_params_inline(params).unwrap_or_default();
    let head_inline = match return_type {
        Some(rt) => format!("{}{}): {}", head, ps, type_ann_text(rt)),
        None => format!("{}{})", head, ps),
    };
    if let Some(b) = render_inline(body) {
        let inline = format!("{}, {})", head_inline, b);
        if indent + inline.len() <= MAX_LINE {
            return inline;
        }
    }

    // Folded: parameter list (with return type) on its own line,
    // body on the next.
    let mut out = format!("{},\n", head_inline);
    out.push_str(&" ".repeat(inner));
    out.push_str(&render(body, inner));
    out.push('\n');
    out.push_str(&" ".repeat(indent));
    out.push(')');
    out
}

/// `MATCH(value, clauses, default)`: fold to one clause per line.
fn render_match(
    value: &Expr,
    clauses: &[wlwl_ast::MatchClause],
    default: &Expr,
    indent: usize,
) -> String {
    let inner = indent + FOLD_INDENT;
    let mut out = "MATCH(\n".to_string();
    out.push_str(&" ".repeat(inner));
    out.push_str(&render(value, inner));
    out.push_str(",\n");
    out.push_str(&" ".repeat(inner));
    out.push_str("[\n");
    let clause_indent = inner + INDENT;
    for (i, c) in clauses.iter().enumerate() {
        out.push_str(&" ".repeat(clause_indent));
        out.push('[');
        out.push_str(&render_pattern_inline(&c.pattern).unwrap_or_default());
        out.push_str(",\n");
        out.push_str(&" ".repeat(clause_indent + INDENT));
        out.push_str(&render(&c.body, clause_indent + INDENT));
        out.push('\n');
        out.push_str(&" ".repeat(clause_indent));
        out.push(']');
        if i + 1 < clauses.len() {
            out.push_str(",\n");
        }
    }
    out.push('\n');
    out.push_str(&" ".repeat(inner));
    out.push(']');
    out.push_str(",\n");
    out.push_str(&" ".repeat(inner));
    out.push_str(&render(default, inner));
    out.push('\n');
    out.push_str(&" ".repeat(indent));
    out.push(')');
    out
}

/// Rule 9: `CLASS(name, parent, [...])` with members one per line.
/// Returns `None` when the shape does not match (then it is treated
/// as an ordinary call).
fn render_class(name: &str, args: &[Expr], indent: usize) -> Option<String> {
    if args.len() != 3 {
        return None;
    }
    let members = match &args[2] {
        Expr::Array { items, .. } if !items.is_empty() => items,
        _ => return None,
    };
    let head = render_inline(&args[0])?;
    let parent = render_inline(&args[1])?;
    let member_indent = indent + INDENT;
    let mut out = format!("{}({}, {}, [\n", name, head, parent);
    for (i, m) in members.iter().enumerate() {
        out.push_str(&" ".repeat(member_indent));
        out.push_str(&render(m, member_indent));
        if i + 1 < members.len() {
            out.push_str(",\n");
        }
    }
    out.push('\n');
    out.push_str(&" ".repeat(indent));
    out.push_str("])");
    Some(out)
}
