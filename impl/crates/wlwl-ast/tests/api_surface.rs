//! P3-009c: API surface tests for every public method on every public
//! type in `wlwl-ast`.
//!
//! Where `tests/serde_roundtrip.rs` (P3-009b) covers the `Serialize` /
//! `Deserialize` paths, this file covers the **other** public methods
//! whose code paths the P3-009 coverage run showed were still cold:
//!
//!   - `TypeExpr::display()` — round-trips the parser for snapshot
//!     stability (Ident / Array / Generic arms).
//!   - `TypeExpr::span()`    — three match arms.
//!   - `Expr::span()`        — 24 match arms (only `Literal` was
//!     previously reached via the `expr_span` test in `src/lib.rs`).
//!   - `Span::new` / `Span::dummy` — constructors (line_end / col_end
//!     fall out to the input line/col).
//!   - `FunParam::new` / `TypeAnnotation::new` — convenience ctors
//!     used by the parser.
//!   - `ImportName::local_name` — alias vs. no-alias path.

use wlwl_ast::{
    Expr, FunParam, ImportName, Literal, Span, TypeAnnotation, TypeExpr,
};

// ---- builders ----

fn sp() -> Span { Span::new("t.wl", 7, 3) }
fn sp_other() -> Span { Span::new("t.wl", 8, 1) }

fn ident(name: &str) -> TypeExpr {
    TypeExpr::Ident { name: name.into(), span: sp() }
}
fn array_of(elem: TypeExpr) -> TypeExpr {
    TypeExpr::Array { element: Box::new(elem), span: sp() }
}
fn generic(name: &str, args: Vec<TypeExpr>) -> TypeExpr {
    TypeExpr::Generic { name: name.into(), args, span: sp() }
}

fn e_lit_i(n: i64) -> Expr { Expr::Literal(Literal::Integer(n), sp()) }
fn e_var(s: &str) -> Expr { Expr::Var(s.into(), sp()) }
fn e_call(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call { name: name.into(), args, span: sp() }
}
fn e_block(exprs: Vec<Expr>) -> Expr { Expr::Block { exprs, span: sp() } }
fn e_array(items: Vec<Expr>) -> Expr { Expr::Array { items, span: sp() } }
fn e_dict(entries: Vec<(Expr, Expr)>) -> Expr { Expr::Dict { entries, span: sp() } }
fn e_let(name: &str, value: Expr) -> Expr {
    Expr::Let { name: name.into(), type_annotation: None, value: Box::new(value), span: sp() }
}
fn e_let_typed(name: &str, value: Expr, ann: TypeAnnotation) -> Expr {
    Expr::Let { name: name.into(), type_annotation: Some(ann), value: Box::new(value), span: sp() }
}
fn e_if(cond: Expr, then: Expr, else_: Option<Expr>) -> Expr {
    Expr::If { cond: Box::new(cond), then_branch: Box::new(then), else_branch: else_.map(Box::new), span: sp() }
}
fn e_while(cond: Expr, body: Expr) -> Expr {
    Expr::While { cond: Box::new(cond), body: Box::new(body), span: sp() }
}
fn e_for(var: &str, iter: Expr, body: Expr) -> Expr {
    Expr::For { var: var.into(), iter: Box::new(iter), body: Box::new(body), span: sp() }
}
fn e_return(value: Option<Expr>) -> Expr {
    Expr::Return { value: value.map(Box::new), span: sp() }
}
fn e_break() -> Expr { Expr::Break { span: sp() } }
fn e_continue() -> Expr { Expr::Continue { span: sp() } }
fn e_fun(params: Vec<FunParam>, body: Expr) -> Expr {
    Expr::Fun { params, return_type: None, body: Box::new(body), span: sp() }
}
fn e_fun_typed(params: Vec<FunParam>, ret: TypeAnnotation, body: Expr) -> Expr {
    Expr::Fun { params, return_type: Some(ret), body: Box::new(body), span: sp() }
}
fn e_ok(value: Expr) -> Expr { Expr::Ok { value: Box::new(value), span: sp() } }
fn e_err(value: Expr) -> Expr { Expr::Err { value: Box::new(value), span: sp() } }
fn e_panic(value: Expr) -> Expr { Expr::Panic { value: Box::new(value), span: sp() } }
fn e_try(value: Expr) -> Expr { Expr::Try { value: Box::new(value), span: sp() } }
fn e_is_ok(value: Expr) -> Expr { Expr::IsOk { value: Box::new(value), span: sp() } }
fn e_is_err(value: Expr) -> Expr { Expr::IsErr { value: Box::new(value), span: sp() } }
fn e_or_die(value: Expr, default: Expr) -> Expr {
    Expr::OrDie { value: Box::new(value), default: Box::new(default), span: sp() }
}
fn e_import(path: &str, names: Vec<ImportName>) -> Expr {
    Expr::Import { path: path.into(), names, span: sp() }
}
fn e_export(names: Vec<ImportName>) -> Expr { Expr::Export { names, span: sp() } }

// ---- Span ----

#[test]
fn span_new_initializes_end_to_start() {
    let s = Span::new("a.wl", 4, 2);
    assert_eq!(s.file, "a.wl");
    assert_eq!(s.line_start, 4);
    assert_eq!(s.col_start, 2);
    assert_eq!(s.line_end, 4);
    assert_eq!(s.col_end, 2);
}

#[test]
fn span_dummy_is_zero_zero_unknown_file() {
    let s = Span::dummy();
    assert_eq!(s.file, "<unknown>");
    assert_eq!(s.line_start, 0);
    assert_eq!(s.col_start, 0);
    assert_eq!(s.line_end, 0);
    assert_eq!(s.col_end, 0);
}

// ---- TypeExpr ----

#[test]
fn type_expr_span_ident() { assert_eq!(ident("INTEGER").span().line_start, 7); }
#[test]
fn type_expr_span_array() { assert_eq!(array_of(ident("INTEGER")).span().line_start, 7); }
#[test]
fn type_expr_span_generic() { assert_eq!(generic("DICT", vec![ident("STRING"), ident("INTEGER")]).span().line_start, 7); }

#[test]
fn type_expr_display_ident() { assert_eq!(ident("INTEGER").display(), "INTEGER"); }

#[test]
fn type_expr_display_array() { assert_eq!(array_of(ident("INTEGER")).display(), "ARRAY<INTEGER>"); }

#[test]
fn type_expr_display_generic_two_args() {
    assert_eq!(generic("DICT", vec![ident("STRING"), ident("INTEGER")]).display(), "DICT<STRING, INTEGER>");
}

#[test]
fn type_expr_display_generic_one_arg() {
    assert_eq!(generic("OK", vec![ident("INTEGER")]).display(), "OK<INTEGER>");
}

#[test]
fn type_expr_display_nested() {
    let t = array_of(generic("OK", vec![generic("DICT", vec![ident("STRING"), ident("INTEGER")])]));
    assert_eq!(t.display(), "ARRAY<OK<DICT<STRING, INTEGER>>>");
}

// ---- TypeAnnotation::new ----

#[test]
fn type_annotation_new_carries_text_and_expr() {
    let ann = TypeAnnotation::new(ident("INTEGER"), "INTEGER".into(), sp());
    assert_eq!(ann.text, "INTEGER");
    assert_eq!(ann.expr.display(), "INTEGER");
    assert_eq!(ann.span.line_start, 7);
}

// ---- FunParam::new ----

#[test]
fn fun_param_new_default_has_no_annotation() {
    let p = FunParam::new("x".into(), sp());
    assert_eq!(p.name, "x");
    assert!(p.type_annotation.is_none());
    assert_eq!(p.span.line_start, 7);
}

#[test]
fn fun_param_typed_constructor_carries_annotation() {
    let p = FunParam {
        name: "x".into(),
        type_annotation: Some(TypeAnnotation::new(ident("INTEGER"), "INTEGER".into(), sp())),
        span: sp_other(),
    };
    assert_eq!(p.name, "x");
    assert!(p.type_annotation.is_some());
    assert_eq!(p.span.line_start, 8);
}

// ---- ImportName::local_name ----

#[test]
fn import_name_local_name_no_alias() {
    let n = ImportName { name: "add".into(), alias: None, span: sp() };
    assert_eq!(n.local_name(), "add");
}

#[test]
fn import_name_local_name_with_alias() {
    let n = ImportName { name: "add".into(), alias: Some("math_add".into()), span: sp() };
    assert_eq!(n.local_name(), "math_add");
}

// ---- Expr::span — 24 arms ----

#[test] fn expr_span_literal()  { assert_eq!(e_lit_i(1).span().line_start, 7); }
#[test] fn expr_span_var()      { assert_eq!(e_var("x").span().line_start, 7); }
#[test] fn expr_span_call()     { assert_eq!(e_call("PRINT", vec![e_lit_i(1)]).span().line_start, 7); }
#[test] fn expr_span_block()    { assert_eq!(e_block(vec![e_lit_i(1)]).span().line_start, 7); }
#[test] fn expr_span_array()    { assert_eq!(e_array(vec![e_lit_i(1)]).span().line_start, 7); }
#[test] fn expr_span_dict()     { assert_eq!(e_dict(vec![(e_lit_i(1), e_lit_i(2))]).span().line_start, 7); }
#[test] fn expr_span_let()      { assert_eq!(e_let("x", e_lit_i(1)).span().line_start, 7); }
#[test] fn expr_span_let_typed() {
    let ann = TypeAnnotation::new(ident("INTEGER"), "INTEGER".into(), sp());
    assert_eq!(e_let_typed("x", e_lit_i(1), ann).span().line_start, 7);
}
#[test] fn expr_span_if()            { assert_eq!(e_if(e_lit_i(1), e_lit_i(2), None).span().line_start, 7); }
#[test] fn expr_span_if_else()       { assert_eq!(e_if(e_lit_i(1), e_lit_i(2), Some(e_lit_i(3))).span().line_start, 7); }
#[test] fn expr_span_while()         { assert_eq!(e_while(e_lit_i(1), e_lit_i(2)).span().line_start, 7); }
#[test] fn expr_span_for()           { assert_eq!(e_for("i", e_lit_i(1), e_lit_i(2)).span().line_start, 7); }
#[test] fn expr_span_return_some()   { assert_eq!(e_return(Some(e_lit_i(1))).span().line_start, 7); }
#[test] fn expr_span_return_none()   { assert_eq!(e_return(None).span().line_start, 7); }
#[test] fn expr_span_break()         { assert_eq!(e_break().span().line_start, 7); }
#[test] fn expr_span_continue()      { assert_eq!(e_continue().span().line_start, 7); }
#[test] fn expr_span_fun() {
    assert_eq!(e_fun(vec![FunParam::new("x".into(), sp())], e_lit_i(1)).span().line_start, 7);
}
#[test] fn expr_span_fun_typed_return() {
    let ann = TypeAnnotation::new(ident("INTEGER"), "INTEGER".into(), sp());
    let f = e_fun_typed(vec![FunParam::new("x".into(), sp())], ann, e_lit_i(1));
    assert_eq!(f.span().line_start, 7);
}
#[test] fn expr_span_ok()      { assert_eq!(e_ok(e_lit_i(1)).span().line_start, 7); }
#[test] fn expr_span_err()     { assert_eq!(e_err(e_lit_i(1)).span().line_start, 7); }
#[test] fn expr_span_panic()   { assert_eq!(e_panic(e_lit_i(1)).span().line_start, 7); }
#[test] fn expr_span_try()     { assert_eq!(e_try(e_lit_i(1)).span().line_start, 7); }
#[test] fn expr_span_is_ok()   { assert_eq!(e_is_ok(e_lit_i(1)).span().line_start, 7); }
#[test] fn expr_span_is_err()  { assert_eq!(e_is_err(e_lit_i(1)).span().line_start, 7); }
#[test] fn expr_span_or_die()  { assert_eq!(e_or_die(e_lit_i(1), e_lit_i(2)).span().line_start, 7); }
#[test] fn expr_span_import()  {
    let names = vec![ImportName { name: "PRINT".into(), alias: None, span: sp() }];
    assert_eq!(e_import("wlwl:std.io", names).span().line_start, 7);
}
#[test] fn expr_span_export()  {
    let names = vec![ImportName { name: "add".into(), alias: None, span: sp() }];
    assert_eq!(e_export(names).span().line_start, 7);
}