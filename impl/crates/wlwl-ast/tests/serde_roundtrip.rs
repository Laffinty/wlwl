//! P3-009b: serde roundtrip tests for every public type in wlwl-ast.
//!
//! The v0.3 spec pins the AST wire format via the AI-friendly tooling
//! contract (Section 14.7 + Section 2.4 type-annotation slots). This file
//! round-trips every public type through `serde_json` to make sure the
//! `Serialize` / `Deserialize` derive paths are all exercised -- the
//! P3-009 coverage run showed these paths were uncovered because the
//! only consumer is `wlwl ast --format=json` which dumps the Expr enum.

use wlwl_ast::{
    Expr, FunParam, ImportName, Literal, Span, TypeAnnotation, TypeExpr,
};

fn sp() -> Span {
    Span::new("t.wl", 1, 1)
}

fn sp2() -> Span {
    Span::new("t.wl", 2, 5)
}

fn ident(name: &str) -> TypeExpr {
    TypeExpr::Ident { name: name.into(), span: sp() }
}

fn array_of(elem: TypeExpr) -> TypeExpr {
    TypeExpr::Array { element: Box::new(elem), span: sp() }
}

fn generic(name: &str, args: Vec<TypeExpr>) -> TypeExpr {
    TypeExpr::Generic { name: name.into(), args, span: sp() }
}

fn roundtrip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug + PartialEq,
{
    let json = serde_json::to_string(value).expect("serialize");
    let de: T = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(&de, value, "roundtrip mismatch: {json}");
    de
}

// ── Span ─────────────────────────────────────────────────────────
#[test]
fn span_roundtrip() {
    roundtrip(&sp());
    roundtrip(&Span::new("other.wl", 99, 12));
    roundtrip(&Span::dummy());
}

// ── Literal (all 5 variants) ────────────────────────────────────
#[test]
fn literal_integer() {
    roundtrip(&Literal::Integer(0));
    roundtrip(&Literal::Integer(i64::MIN));
    roundtrip(&Literal::Integer(i64::MAX));
}
#[test]
fn literal_float() {
    roundtrip(&Literal::Float(0.0));
    roundtrip(&Literal::Float(3.14159));
    // NaN is not tested here: serde_json serializes non-finite floats as `null`
    // (its default behavior) and refuses to round-trip them. The WLWL source
    // never produces NaN at runtime, so this is acceptable for the wire format.
}
#[test]
fn literal_string() {
    roundtrip(&Literal::String(String::new()));
    roundtrip(&Literal::String("hello".into()));
    roundtrip(&Literal::String("with \"quotes\" and \n newlines".into()));
}
#[test]
fn literal_boolean_and_null() {
    roundtrip(&Literal::Boolean(true));
    roundtrip(&Literal::Boolean(false));
    roundtrip(&Literal::Null);
}

// ── TypeExpr (all 3 variants) ───────────────────────────────────
#[test]
fn type_expr_ident() {
    roundtrip(&ident("INTEGER"));
    roundtrip(&ident("FLOAT"));
    roundtrip(&ident("BOOLEAN"));
}
#[test]
fn type_expr_array() {
    roundtrip(&array_of(ident("INTEGER")));
    roundtrip(&array_of(array_of(ident("FLOAT"))));
}
#[test]
fn type_expr_generic() {
    roundtrip(&generic("DICT", vec![ident("STRING"), ident("INTEGER")]));
    roundtrip(&generic("OK", vec![ident("INTEGER")]));
    roundtrip(&generic("ERR", vec![ident("STRING")]));
    roundtrip(&generic("UNIT", vec![]));
}

// ── TypeAnnotation ──────────────────────────────────────────────
#[test]
fn type_annotation_roundtrip() {
    let a = TypeAnnotation::new(ident("INTEGER"), "INTEGER".into(), sp());
    roundtrip(&a);
    let b = TypeAnnotation::new(
        array_of(ident("FLOAT")),
        "ARRAY[FLOAT]".into(),
        sp2(),
    );
    roundtrip(&b);
}

// ── FunParam ────────────────────────────────────────────────────
#[test]
fn fun_param_no_annotation() {
    roundtrip(&FunParam::new("x".into(), sp()));
}
#[test]
fn fun_param_with_annotation() {
    let p = FunParam {
        name: "n".into(),
        type_annotation: Some(TypeAnnotation::new(ident("INTEGER"), "INTEGER".into(), sp())),
        default_expr: None,
        is_rest: false,
        span: sp(),
    };
    roundtrip(&p);
}

// ── ImportName ──────────────────────────────────────────────────
#[test]
fn import_name_plain() {
    roundtrip(&ImportName { name: "foo".into(), alias: None, span: sp() });
}
#[test]
fn import_name_aliased() {
    roundtrip(&ImportName {
        name: "foo".into(),
        alias: Some("f".into()),
        span: sp(),
    });
}

// ── Expr (every variant) ────────────────────────────────────────
#[test]
fn expr_literal() {
    roundtrip(&Expr::Literal(Literal::Integer(42), sp()));
    roundtrip(&Expr::Literal(Literal::String("s".into()), sp()));
    roundtrip(&Expr::Literal(Literal::Null, sp()));
}
#[test]
fn expr_var() {
    roundtrip(&Expr::Var("counter".into(), sp()));
}
#[test]
fn expr_call() {
    roundtrip(&Expr::Call {
        name: "PRINT".into(),
        args: vec![Expr::Literal(Literal::Integer(1), sp())],
        span: sp(),
    });
}
#[test]
fn expr_block() {
    roundtrip(&Expr::Block {
        exprs: vec![
            Expr::Literal(Literal::Null, sp()),
            Expr::Var("x".into(), sp()),
        ],
        span: sp(),
    });
}
#[test]
fn expr_array_and_dict() {
    roundtrip(&Expr::Array {
        items: vec![
            Expr::Literal(Literal::Integer(1), sp()),
            Expr::Literal(Literal::Integer(2), sp()),
        ],
        span: sp(),
    });
    roundtrip(&Expr::Dict {
        entries: vec![(
            Expr::Literal(Literal::String("a".into()), sp()),
            Expr::Literal(Literal::Integer(1), sp()),
        )],
        span: sp(),
    });
}
#[test]
fn expr_let_with_and_without_annotation() {
    roundtrip(&Expr::Let {
        name: "x".into(),
        type_annotation: None,
        value: Box::new(Expr::Literal(Literal::Integer(1), sp())),
        span: sp(),
    });
    roundtrip(&Expr::Let {
        name: "n".into(),
        type_annotation: Some(TypeAnnotation::new(ident("INTEGER"), "INTEGER".into(), sp())),
        value: Box::new(Expr::Literal(Literal::Integer(0), sp())),
        span: sp(),
    });
}
#[test]
fn expr_if_with_and_without_else() {
    roundtrip(&Expr::If {
        cond: Box::new(Expr::Literal(Literal::Boolean(true), sp())),
        then_branch: Box::new(Expr::Literal(Literal::Integer(1), sp())),
        else_branch: None,
        span: sp(),
    });
    roundtrip(&Expr::If {
        cond: Box::new(Expr::Literal(Literal::Boolean(false), sp())),
        then_branch: Box::new(Expr::Literal(Literal::Integer(1), sp())),
        else_branch: Some(Box::new(Expr::Literal(Literal::Integer(2), sp()))),
        span: sp(),
    });
}
#[test]
fn expr_loops() {
    roundtrip(&Expr::While {
        cond: Box::new(Expr::Literal(Literal::Boolean(true), sp())),
        body: Box::new(Expr::Literal(Literal::Null, sp())),
        span: sp(),
    });
    roundtrip(&Expr::For {
        var: "i".into(),
        iter: Box::new(Expr::Array {
            items: vec![Expr::Literal(Literal::Integer(1), sp())],
            span: sp(),
        }),
        body: Box::new(Expr::Literal(Literal::Null, sp())),
        span: sp(),
    });
}
#[test]
fn expr_control_flow() {
    roundtrip(&Expr::Return {
        value: Some(Box::new(Expr::Literal(Literal::Integer(7), sp()))),
        span: sp(),
    });
    roundtrip(&Expr::Return { value: None, span: sp() });
    roundtrip(&Expr::Break { span: sp() });
    roundtrip(&Expr::Continue { span: sp() });
}
#[test]
fn expr_fun_with_and_without_return_annotation() {
    roundtrip(&Expr::Fun {
        name: None,
        params: vec![FunParam::new("x".into(), sp())],
        return_type: None,
        body: Box::new(Expr::Var("x".into(), sp())),
        span: sp(),
    });
    roundtrip(&Expr::Fun {
        name: None,
        params: vec![
            FunParam::new("a".into(), sp()),
            FunParam::new("b".into(), sp()),
        ],
        return_type: Some(TypeAnnotation::new(ident("INTEGER"), "INTEGER".into(), sp())),
        body: Box::new(Expr::Call {
            name: "+".into(),
            args: vec![
                Expr::Var("a".into(), sp()),
                Expr::Var("b".into(), sp()),
            ],
            span: sp(),
        }),
        span: sp(),
    });
}
#[test]
fn expr_error_handling() {
    roundtrip(&Expr::Ok { value: Box::new(Expr::Literal(Literal::Integer(1), sp())), span: sp() });
    roundtrip(&Expr::Err { value: Box::new(Expr::Literal(Literal::String("boom".into()), sp())), span: sp() });
    roundtrip(&Expr::Panic { value: Box::new(Expr::Literal(Literal::Null, sp())), span: sp() });
    roundtrip(&Expr::Try { value: Box::new(Expr::Literal(Literal::Null, sp())), span: sp() });
    roundtrip(&Expr::IsOk { value: Box::new(Expr::Literal(Literal::Null, sp())), span: sp() });
    roundtrip(&Expr::IsErr { value: Box::new(Expr::Literal(Literal::Null, sp())), span: sp() });
    roundtrip(&Expr::OrDie {
        value: Box::new(Expr::Literal(Literal::Null, sp())),
        default: Box::new(Expr::Literal(Literal::Integer(-1), sp())),
        span: sp(),
    });
}
#[test]
fn expr_import_export() {
    roundtrip(&Expr::Import {
        path: "math".into(),
        names: vec![ImportName { name: "add".into(), alias: None, span: sp() }],
        span: sp(),
    });
    roundtrip(&Expr::Export {
        names: vec![ImportName {
            name: "add".into(),
            alias: Some("plus".into()),
            span: sp(),
        }],
        span: sp(),
    });
}

// ── Span wire format ────────────────────────────────────────────
#[test]
fn span_wire_format() {
    let s = Span::new("path/to/file.wl", 7, 3);
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains("\"file\""), "missing file key: {json}");
    assert!(json.contains("\"line_start\""), "missing line_start: {json}");
    assert!(json.contains("\"col_start\""), "missing col_start: {json}");
    assert!(json.contains("\"line_end\""), "missing line_end: {json}");
    assert!(json.contains("\"col_end\""), "missing col_end: {json}");
}

// ── Expr wire format (what `wlwl ast --format=json` emits) ──────
#[test]
fn expr_call_wire_format() {
    let e = Expr::Call {
        name: "PRINT".into(),
        args: vec![Expr::Literal(Literal::Integer(1), sp())],
        span: sp(),
    };
    let v: serde_json::Value = serde_json::to_value(&e).unwrap();
    let call = v.get("Call").expect("Call tag");
    assert_eq!(call.get("name").unwrap().as_str(), Some("PRINT"));
    assert_eq!(call.get("args").unwrap().as_array().unwrap().len(), 1);
}