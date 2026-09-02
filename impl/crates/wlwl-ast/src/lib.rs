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

/// A type annotation (`name: Type` per v0.3 `Sec. 2.4`).
///
/// Phase 3 only **parses** annotations; they are not checked.
/// The inner string holds the raw source text of the type
/// expression (e.g. "INTEGER", "ARRAY[INTEGER]", "OK[ERR[STRING]]").
/// A structured `TypeExpr` enum can be introduced later without
/// breaking the parser surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeAnnotation {
    pub text: String,
    pub span: Span,
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
    // Sec. 8.2 FUN literal (v0.3 Sec. 2.4: optional return annotation)
    Fun {
        params: Vec<String>,
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
