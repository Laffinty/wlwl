//! WLWL abstract syntax tree.
//!
//! Phase 3 adds v0.3 `Sec. 2.4` type-annotation syntax slots.
//! Annotations are **parsed but not checked**; they are stored as
//! raw text on the AST so AI tools and future checkers can consume
//! them.

use serde::{Deserialize, Serialize};

/// Source code location (file + line/column spans).
///
/// Required by v0.3 `Sec. 14.2` -- every diagnostic must carry precise location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub file: String,
    pub line_start: u32,
    pub col_start: u32,
    pub line_end: u32,
    pub col_end: u32,
}

impl Span {
    pub fn new(file: impl Into<String>, line_start: u32, col_start: u32) -> Self {
        Self {
            file: file.into(),
            line_start,
            col_start,
            line_end: line_start,
            col_end: col_start,
        }
    }

    pub fn dummy() -> Self {
        Self {
            file: "<unknown>".into(),
            line_start: 0,
            col_start: 0,
            line_end: 0,
            col_end: 0,
        }
    }
}

/// Literal value (from v0.3 `Sec. 4`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Null,
}

/// Structured type expression (v0.3 `Sec. 2.4`).
///
/// The grammar we parse (Phase 3 → post-Phase 4):
/// ```text
/// type_expr  ::= IDENT                    // INTEGER, FLOAT, BOOLEAN, ...
///             |  "ARRAY" "<" type_expr ">"
///             |  IDENT "<" type_expr ("," type_expr)* ">"  // DICT, OK, ERR, ...
/// ```
///
/// The two named forms are `Array { element }` and `Generic { name, args }`;
/// bare identifiers are `Ident { name }`. Function types (`FUN(...) -> T`)
/// are reserved for v0.4 -- the parser surfaces them as `Generic` with
/// a `FUN` head and a single trailing-element convention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeExpr {
    Ident {
        name: String,
        span: Span,
    },
    Array {
        element: Box<TypeExpr>,
        span: Span,
    },
    Generic {
        name: String,
        args: Vec<TypeExpr>,
        span: Span,
    },
}

impl TypeExpr {
    pub fn span(&self) -> &Span {
        match self {
            TypeExpr::Ident { span, .. } => span,
            TypeExpr::Array { span, .. } => span,
            TypeExpr::Generic { span, .. } => span,
        }
    }

    /// Render back to source text (round-trips the parser for
    /// stable, readable snapshots). Not used for any semantic check
    /// -- the runtime ignores type annotations.
    pub fn display(&self) -> String {
        match self {
            TypeExpr::Ident { name, .. } => name.clone(),
            TypeExpr::Array { element, .. } => {
                format!("ARRAY<{}>", element.display())
            }
            TypeExpr::Generic { name, args, .. } => {
                let parts: Vec<String> = args.iter().map(|a| a.display()).collect();
                format!("{}<{}>", name, parts.join(", "))
            }
        }
    }
}

/// A type annotation (`name: Type` per v0.3 `Sec. 2.4`). Wraps a
/// `TypeExpr` together with the source span the annotation
/// occupied; only the parsed `expr` is meaningful, the `text`
/// field stays for back-compat with older snapshots / docs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeAnnotation {
    pub expr: TypeExpr,
    pub span: Span,
    /// Raw source text of the annotation, kept so a diagnostic can
    /// show "expected INTEGER, got ARRAY[INTEGER]" without re-deriving
    /// the original source slice.
    pub text: String,
}

impl TypeAnnotation {
    pub fn new(expr: TypeExpr, text: String, span: Span) -> Self {
        Self { expr, span, text }
    }
}

/// One parameter of a `FUN` literal (v0.3 `Sec. 8.2` plus `Sec. 2.4`
/// per-param annotation support; P3-011 adds default and rest).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunParam {
    pub name: String,
    /// Optional `name: Type` annotation. Runtime semantics: ignored
    /// (Transient v0.3). The annotation is preserved on the AST so
    /// tools / docs / future strict-mode can use it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TypeAnnotation>,
    /// Optional default expression (`name = expr`, spec §8.2). P3-011.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_expr: Option<Box<Expr>>,
    /// `*rest` variadic tail marker (spec §8.2). P3-011.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_rest: bool,
    pub span: Span,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl FunParam {
    pub fn new(name: String, span: Span) -> Self {
        Self {
            name,
            type_annotation: None,
            default_expr: None,
            is_rest: false,
            span,
        }
    }
}

/// One entry in an `IMPORT(path, names, ...)` call (v0.3 `Sec. 13.3`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportName {
    pub name: String,
    pub alias: Option<String>,
    pub span: Span,
}

impl ImportName {
    pub fn local_name(&self) -> &str {
        self.alias.as_deref().unwrap_or(self.name.as_str())
    }
}

/// Expression node (Phase 3 -- with type-annotation slots).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    // Sec. 4 Literals
    Literal(Literal, Span),
    // Sec. 5.2 Variable reference
    Var(String, Span),
    // Sec. 5.2 / Sec. 8.3 Function call.
    Call {
        name: String,
        args: Vec<Expr>,
        span: Span,
    },
    // Sec. 5.3 / Sec. 6 Block expression
    Block { exprs: Vec<Expr>, span: Span },
    // Sec. 10.1 Array literal
    Array { items: Vec<Expr>, span: Span },
    // Sec. 10.2 Dict literal
    Dict { entries: Vec<(Expr, Expr)>, span: Span },
    // Sec. 6.1 LET binding (v0.3 Sec. 2.4: optional annotation)
    Let {
        name: String,
        type_annotation: Option<TypeAnnotation>,
        value: Box<Expr>,
        span: Span,
    },
    // Sec. 7.1 IF
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
        span: Span,
    },
    // Sec. 7.2 WHILE
    While { cond: Box<Expr>, body: Box<Expr>, span: Span },
    // Sec. 7.3 FOR
    For { var: String, iter: Box<Expr>, body: Box<Expr>, span: Span },
    // Sec. 7.4 RETURN
    Return { value: Option<Box<Expr>>, span: Span },
    Break { span: Span },
    Continue { span: Span },
    // Sec. 8.2 FUN literal (v0.3 Sec. 2.4: optional return annotation;
    // P3-011 adds optional `name` for the named form `FUN(name(params), body)`)
    Fun {
        /// Optional function name (`FUN(name(params), body)`). `None` for
        /// the anonymous form `FUN((params), body)`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        params: Vec<FunParam>,
        return_type: Option<TypeAnnotation>,
        body: Box<Expr>,
        span: Span,
    },
    // Sec. 12.2 OK(value)
    Ok { value: Box<Expr>, span: Span },
    // Sec. 12.2 ERR(value)
    Err { value: Box<Expr>, span: Span },
    // Sec. 12.4 PANIC
    Panic { value: Box<Expr>, span: Span },
    // Sec. 12.3 TRY
    Try { value: Box<Expr>, span: Span },
    IsOk { value: Box<Expr>, span: Span },
    IsErr { value: Box<Expr>, span: Span },
    OrDie { value: Box<Expr>, default: Box<Expr>, span: Span },
    Import { path: String, names: Vec<ImportName>, span: Span },
    Export { names: Vec<ImportName>, span: Span },
}

impl Expr {
    pub fn span(&self) -> &Span {
        match self {
            Expr::Literal(_, s) => s,
            Expr::Var(_, s) => s,
            Expr::Call { span, .. } => span,
            Expr::Block { span, .. } => span,
            Expr::Array { span, .. } => span,
            Expr::Dict { span, .. } => span,
            Expr::Let { span, .. } => span,
            Expr::If { span, .. } => span,
            Expr::While { span, .. } => span,
            Expr::For { span, .. } => span,
            Expr::Return { span, .. } => span,
            Expr::Break { span, .. } => span,
            Expr::Continue { span, .. } => span,
            Expr::Fun { span, .. } => span,
            Expr::Ok { span, .. } => span,
            Expr::Err { span, .. } => span,
            Expr::Panic { span, .. } => span,
            Expr::Try { span, .. } => span,
            Expr::IsOk { span, .. } => span,
            Expr::IsErr { span, .. } => span,
            Expr::OrDie { span, .. } => span,
            Expr::Import { span, .. } => span,
            Expr::Export { span, .. } => span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_dummy() {
        let s = Span::dummy();
        assert_eq!(s.line_start, 0);
    }

    #[test]
    fn expr_span() {
        let e = Expr::Literal(Literal::Integer(1), Span::new("t.wl", 1, 1));
        assert_eq!(e.span().line_start, 1);
    }

    #[test]
    fn import_name_local_default() {
        let n = ImportName { name: "add".into(), alias: None, span: Span::dummy() };
        assert_eq!(n.local_name(), "add");
        let n = ImportName { name: "add".into(), alias: Some("math_add".into()), span: Span::dummy() };
        assert_eq!(n.local_name(), "math_add");
    }
}
