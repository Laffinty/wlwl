//! WLWL abstract syntax tree.
//!
//! Phase 2 covers v0.3 §3–§13 (subset):
//! - §3  Lexical (delegated to the lexer crate)
//! - §4  Literals
//! - §5  Expressions (all values are expressions)
//! - §6  Variables: `LET` binding
//! - §7  Control flow: `IF`, `WHILE`, `FOR`, `RETURN`, `BREAK`, `CONTINUE`
//! - §8  Functions: `FUN` literals, first-class, closures
//! - §9  Operators (exposed as built-in functions; the parser turns
//!        `+`/`-`/`*`/etc. into function calls with operator names)
//! - §10 Data structures: `ARRAY` literals, `DICT` literals
//! - §12 Error handling: `OK`, `ERR`, `PANIC`, `TRY`, `IS_OK`, `IS_ERR`,
//!        `OR_DIE`, plus **§12.6 ERR transparent propagation** (handled in
//!        the evaluator's call_with_args)
//! - §13 Modules (subset, single-directory): `IMPORT`, `EXPORT`

use serde::{Deserialize, Serialize};

/// Source code location (file + line/column spans).
///
/// Required by v0.3 §14.2 — every diagnostic must carry precise location.
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

/// Literal value (from v0.3 §4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Null,
}

/// One entry in an `IMPORT(path, names, …)` call (v0.3 §13.3).
///
/// `name` is the symbol to import from the module; `alias` is the local
/// binding name (defaults to `name` if not present).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportName {
    pub name: String,
    pub alias: Option<String>,
    pub span: Span,
}

impl ImportName {
    /// Resolve the local binding name for this import.
    pub fn local_name(&self) -> &str {
        self.alias.as_deref().unwrap_or(self.name.as_str())
    }
}

/// Expression node (Phase 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    // §4 Literals
    Literal(Literal, Span),
    // §5.2 Variable reference
    Var(String, Span),
    // §5.2 / §8.3 Function call. `name` is the function symbol; for
    // operator-built-ins (`+`, `==`, etc.) it is the operator spelling.
    Call {
        name: String,
        args: Vec<Expr>,
        span: Span,
    },
    // §5.3 / §6 Block expression (sequence of statements)
    Block {
        exprs: Vec<Expr>,
        span: Span,
    },
    // §10.1 Array literal
    Array {
        items: Vec<Expr>,
        span: Span,
    },
    // §10.2 Dict literal
    Dict {
        entries: Vec<(Expr, Expr)>,
        span: Span,
    },
    // §6.1 LET binding
    Let {
        name: String,
        value: Box<Expr>,
        span: Span,
    },
    // §7.1 IF
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
        span: Span,
    },
    // §7.2 WHILE
    While {
        cond: Box<Expr>,
        body: Box<Expr>,
        span: Span,
    },
    // §7.3 FOR
    For {
        var: String,
        iter: Box<Expr>,
        body: Box<Expr>,
        span: Span,
    },
    // §7.4 RETURN
    Return {
        value: Option<Box<Expr>>,
        span: Span,
    },
    // §7.4 BREAK
    Break {
        span: Span,
    },
    // §7.4 CONTINUE
    Continue {
        span: Span,
    },
    // §8.2 FUN literal (anonymous function; first-class; closes over env)
    Fun {
        params: Vec<String>,
        body: Box<Expr>,
        span: Span,
    },
    // §12.2 OK(value)
    Ok {
        value: Box<Expr>,
        span: Span,
    },
    // §12.2 ERR(value)
    Err {
        value: Box<Expr>,
        span: Span,
    },
    // §12.4 PANIC
    Panic {
        value: Box<Expr>,
        span: Span,
    },
    // §12.3 TRY
    Try {
        value: Box<Expr>,
        span: Span,
    },
    // §12 IS_OK
    IsOk {
        value: Box<Expr>,
        span: Span,
    },
    // §12 IS_ERR
    IsErr {
        value: Box<Expr>,
        span: Span,
    },
    // §12 OR_DIE(value, default)
    OrDie {
        value: Box<Expr>,
        default: Box<Expr>,
        span: Span,
    },
    // §13.3 IMPORT(path, names)
    Import {
        path: String,
        names: Vec<ImportName>,
        span: Span,
    },
    // §13.2 EXPORT(names)
    Export {
        names: Vec<ImportName>,
        span: Span,
    },
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
        // Without alias, local binding == imported name.
        let n = ImportName {
            name: "add".into(),
            alias: None,
            span: Span::dummy(),
        };
        assert_eq!(n.local_name(), "add");

        // With alias, local binding is the alias.
        let n = ImportName {
            name: "add".into(),
            alias: Some("math_add".into()),
            span: Span::dummy(),
        };
        assert_eq!(n.local_name(), "math_add");
    }
}
