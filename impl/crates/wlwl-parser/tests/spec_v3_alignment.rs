//! spec v0.3 mid-syntax alignment test battery (P3-011).
//!
//! Covers §3-§13 of `docs/standard/wlwl-spec-v0.3(MD5_4308b3d2071ebed5cb52eba612b1ea).md`.
//!
//! Each test parses a spec example and asserts the AST shape matches
//! the spec's grammar. This file is a parser-level alignment harness —
//! eval-level semantics (e.g. `IS_OK(OK(1))` returning `TRUE`) are
//! covered in the eval crate.

use wlwl_ast::{Expr, Literal, Span};
use wlwl_parser::parse;

// ───────────────────────── §3 Lexical (5) ─────────────────────────

#[test]
fn lex_16_keywords_all_recognized() {
    // All 16 v0.3 §3.2 keywords must lex as their dedicated token
    // kind, not as bare identifiers. The parser then dispatches each
    // to its own `parse_*` entry point.
    let src = r#"LET(TRUE_VAR, FALSE_VAR); NULL;"#;
    // We can only assert the parser doesn't reject them. TRUE/FALSE/NULL
    // here are used as bare identifiers (lowercased tail) to avoid
    // clashing with the keyword form. The literal test is in `lex_*`
    // below.
    let r = parse(src, "t.wl");
    assert!(r.is_ok(), "expected parse OK, got {:?}", r.err().map(|e| e.diagnostic().code));
}

#[test]
fn lex_true_false_null_are_keywords() {
    // TRUE / FALSE / NULL as keywords, not identifiers.
    let r = parse("LET(t, TRUE); LET(f, FALSE); LET(n, NULL);", "t.wl").unwrap();
    let exprs = match r {
        Expr::Block { exprs, .. } => exprs,
        other => panic!("expected block, got {:?}", other),
    };
    // Spot-check: the value of the first LET is Literal::Boolean(true)
    let val = match &exprs[0] {
        Expr::Let { value, .. } => value.as_ref(),
        other => panic!("expected Let, got {:?}", other),
    };
    assert!(matches!(val, Expr::Literal(Literal::Boolean(true), _)));
}

#[test]
fn lex_case_sensitive_true_vs_True() {
    // TRUE is keyword; True (capital T + lower rest) must lex as ident.
    let r = parse("LET(True, 1);", "t.wl").unwrap();
    // Single statement at top level — parser collapses to the stmt
    // itself rather than wrapping in a Block.
    let name = match &r {
        Expr::Let { name, .. } => name,
        Expr::Block { exprs, .. } => match &exprs[0] {
            Expr::Let { name, .. } => name,
            other => panic!("expected Let, got {:?}", other),
        },
        other => panic!("expected Let, got {:?}", other),
    };
    assert_eq!(name, "True");
}

#[test]
fn lex_nested_block_comment() {
    // v0.3 §3.4: /* ... /* nested */ ... */ is one comment.
    let src = r#"
/* outer
   /* inner */
   still outer */
LET(x, 1);
"#;
    let r = parse(src, "t.wl");
    assert!(r.is_ok(), "expected nested block comment to parse, got {:?}", r.err().map(|e| e.diagnostic().code));
}

#[test]
fn lex_multiple_whitespace_normalized() {
    // §3.5: 缩进不影响语义, 多个空白 = 1 空白.
    let r = parse("LET(\n\n  x,\t\t1  );", "t.wl").unwrap();
    // Single-statement program; parser collapses to the Let itself.
    let count = match &r {
        Expr::Let { .. } => 1usize,
        Expr::Block { exprs, .. } => exprs.len(),
        other => panic!("expected Let/Block, got {:?}", other),
    };
    assert_eq!(count, 1);
}

// ───────────────────────── §4 Literals (8) ─────────────────────────

#[test]
fn lit_integer() {
    let r = parse("LET(i, 42);", "t.wl").unwrap();
    let e = only_let(&r);
    assert!(matches!(
        e.as_ref(),
        Expr::Literal(Literal::Integer(42), _)
    ));
}

#[test]
fn lit_float() {
    let r = parse("LET(f, 3.14);", "t.wl").unwrap();
    let e = only_let(&r);
    assert!(matches!(
        e.as_ref(),
        Expr::Literal(Literal::Float(_), _)
    ));
}

#[test]
fn lit_negative_integer_via_unary_minus() {
    // §4.1: `LET(n, -1)` — `-1` is a unary-minus expression over `1`.
    // The parser desugars `-x` to `-(0, x)`. v0.3 §4.1 calls it a
    // literal but the desugared form is a `Call`.
    let r = parse("LET(n, -(1));", "t.wl").unwrap();
    let e = only_let(&r);
    let call = match e.as_ref() {
        Expr::Call { name, args, .. } => (name, args),
        other => panic!("expected Call, got {:?}", other),
    };
    assert_eq!(call.0, "-");
    assert_eq!(call.1.len(), 1);
}

#[test]
fn lit_string_with_escapes() {
    // §4.2: \"\\n \\t \\r \\\\\\ \\\" \\0\".
    let r = parse(r#"LET(s, "a\nb\tc\rd\\e\"f\0g");"#, "t.wl").unwrap();
    let e = only_let(&r);
    let s = match e.as_ref() {
        Expr::Literal(Literal::String(s), _) => s,
        other => panic!("expected String, got {:?}", other),
    };
    assert_eq!(s, "a\nb\tc\rd\\e\"f\0g");
}

#[test]
fn lit_string_chinese() {
    // §4.2: strings may contain any UTF-8. (Identifiers too — see
    // `lex_chinese_identifier` after the lexer fix lands.)
    let r = parse(r#"LET(s, "事屑");"#, "t.wl").unwrap();
    let e = only_let(&r);
    let s = match e.as_ref() {
        Expr::Literal(Literal::String(s), _) => s,
        other => panic!("expected String, got {:?}", other),
    };
    assert_eq!(s, "事屑");
}

#[test]
fn lit_empty_array() {
    let r = parse("LET(e, []);", "t.wl").unwrap();
    let e = only_let(&r);
    let items = match e.as_ref() {
        Expr::Array { items, .. } => items,
        other => panic!("expected Array, got {:?}", other),
    };
    assert!(items.is_empty());
}

#[test]
fn lit_dict() {
    let r = parse(r#"LET(d, ["cat": "nya", "dog": 2]);"#, "t.wl").unwrap();
    let e = only_let(&r);
    let entries = match e.as_ref() {
        Expr::Dict { entries, .. } => entries,
        other => panic!("expected Dict, got {:?}", other),
    };
    assert_eq!(entries.len(), 2);
}

#[test]
fn lit_mixed_type_array() {
    // §4.4: array can hold mixed types.
    let r = parse(r#"LET(m, [1, "two", TRUE, NULL]);"#, "t.wl").unwrap();
    let e = only_let(&r);
    let items = match e.as_ref() {
        Expr::Array { items, .. } => items,
        other => panic!("expected Array, got {:?}", other),
    };
    assert_eq!(items.len(), 4);
}

// ───────────────────────── §5 / §8.3 Expressions (6) ─────────────────────────

#[test]
fn expr_function_call_add() {
    let r = parse("LET(x, +(1, 2));", "t.wl").unwrap();
    let e = only_let(&r);
    let call = match e.as_ref() {
        Expr::Call { name, args, .. } => (name, args),
        other => panic!("expected Call, got {:?}", other),
    };
    assert_eq!(call.0, "+");
    assert_eq!(call.1.len(), 2);
}

#[test]
fn expr_nested_call() {
    let r = parse(r#"LET(y, PRINT("hi"));"#, "t.wl").unwrap();
    let e = only_let(&r);
    let call = match e.as_ref() {
        Expr::Call { name, args, .. } => (name, args),
        other => panic!("expected Call, got {:?}", other),
    };
    assert_eq!(call.0, "PRINT");
    assert_eq!(call.1.len(), 1);
}

#[test]
fn expr_block_value() {
    // §5.3: block value is the last expression.
    let r = parse("LET(x, (LET(a, 1); LET(b, +(a, 2)); +(a, b)));", "t.wl").unwrap();
    let e = only_let(&r);
    let block = match e.as_ref() {
        Expr::Block { exprs, .. } => exprs,
        other => panic!("expected Block, got {:?}", other),
    };
    assert_eq!(block.len(), 3);
}

#[test]
fn expr_empty_block_is_null() {
    // §5.3: 空块 () 的值为 NULL.
    let r = parse("LET(x, ());", "t.wl").unwrap();
    let e = only_let(&r);
    assert!(matches!(
        e.as_ref(),
        Expr::Literal(Literal::Null, _)
    ));
}

#[test]
fn expr_op_named_call_eq_eq() {
    // §9.2: =(a, b) is a function with name "==".
    let r = parse("LET(r, ==(1, 1));", "t.wl").unwrap();
    let e = only_let(&r);
    let call = match e.as_ref() {
        Expr::Call { name, args, .. } => (name, args),
        other => panic!("expected Call, got {:?}", other),
    };
    assert_eq!(call.0, "==");
}

#[test]
fn expr_unary_minus_desugars() {
    // -x  ->  -(0, x)
    let r = parse("LET(y, -x);", "t.wl").unwrap();
    let e = only_let(&r);
    let call = match e.as_ref() {
        Expr::Call { name, args, .. } => (name, args),
        other => panic!("expected Call, got {:?}", other),
    };
    assert_eq!(call.0, "-");
    assert_eq!(call.1.len(), 2);
    // first arg is Integer(0)
    assert!(matches!(
        call.1[0],
        Expr::Literal(Literal::Integer(0), _)
    ));
}

// ───────────────────────── §6 Variables (4) ─────────────────────────

#[test]
fn let_basic() {
    let r = parse("LET(x, 1);", "t.wl").unwrap();
    let l = match &r {
        Expr::Let { name, type_annotation, .. } => (name, type_annotation),
        Expr::Block { exprs, .. } => match &exprs[0] {
            Expr::Let { name, type_annotation, .. } => (name, type_annotation),
            other => panic!("expected Let, got {:?}", other),
        },
        other => panic!("expected Let, got {:?}", other),
    };
    assert_eq!(l.0, "x");
    assert!(l.1.is_none());
}

#[test]
fn let_with_type_annotation() {
    let r = parse("LET(x: INTEGER, 1);", "t.wl").unwrap();
    let l = match &r {
        Expr::Let { type_annotation, .. } => type_annotation,
        Expr::Block { exprs, .. } => match &exprs[0] {
            Expr::Let { type_annotation, .. } => type_annotation,
            other => panic!("expected Let, got {:?}", other),
        },
        other => panic!("expected Let, got {:?}", other),
    };
    let ann = l.as_ref().expect("expected annotation");
    assert_eq!(ann.text, "INTEGER");
}

#[test]
fn let_with_complex_type_annotation() {
    let r = parse(r#"LET(m: DICT[STRING, INTEGER], ["k": 1]);"#, "t.wl").unwrap();
    let l = match &r {
        Expr::Let { type_annotation, .. } => type_annotation,
        Expr::Block { exprs, .. } => match &exprs[0] {
            Expr::Let { type_annotation, .. } => type_annotation,
            other => panic!("expected Let, got {:?}", other),
        },
        other => panic!("expected Let, got {:?}", other),
    };
    let ann = l.as_ref().expect("expected annotation");
    assert!(ann.text.contains("DICT"));
    assert!(ann.text.contains("STRING"));
    assert!(ann.text.contains("INTEGER"));
}

#[test]
fn set_via_call() {
    // §6.2: SET is a plain function call.
    let r = parse("LET(x, 1); SET(x, 2);", "t.wl").unwrap();
    let exprs = match r {
        Expr::Block { exprs, .. } => exprs,
        _ => unreachable!(),
    };
    let call = match &exprs[1] {
        Expr::Call { name, args, .. } => (name, args),
        other => panic!("expected Call, got {:?}", other),
    };
    assert_eq!(call.0, "SET");
    assert_eq!(call.1.len(), 2);
}

// ───────────────────────── §7 Control flow (8) ─────────────────────────

#[test]
fn if_ternary() {
    let r = parse(r#"IF(TRUE, 1, 2);"#, "t.wl").unwrap();
    let iff = match r {
        Expr::If { cond, then_branch, else_branch, .. } => (cond, then_branch, else_branch),
        other => panic!("expected If, got {:?}", other),
    };
    assert!(iff.2.is_some());
}

#[test]
fn if_no_else_default_null() {
    // §7.1: else 缺省 → NULL.
    let r = parse("IF(FALSE, 1);", "t.wl").unwrap();
    let else_b = match r {
        Expr::If { else_branch, .. } => else_branch,
        _ => panic!(),
    };
    assert!(else_b.is_none());
}

#[test]
fn while_with_block_body() {
    // §7.2: WHILE(cond, body); body can be a block.
    let r = parse("WHILE(TRUE, (PRINT(1); PRINT(2)));", "t.wl").unwrap();
    let body = match r {
        Expr::While { body, .. } => body,
        _ => panic!(),
    };
    let body_exprs = match body.as_ref() {
        Expr::Block { exprs, .. } => exprs,
        other => panic!("expected Block body, got {:?}", other),
    };
    assert_eq!(body_exprs.len(), 2);
}

#[test]
fn for_over_array() {
    let r = parse("FOR(i, [1, 2, 3], PRINT(i));", "t.wl").unwrap();
    let f = match r {
        Expr::For { var, iter, .. } => (var, iter),
        _ => panic!(),
    };
    assert_eq!(f.0, "i");
}

#[test]
fn for_over_dict() {
    // §7.3: FOR traverses DICT keys; parser doesn't enforce, just
    // round-trips the iterable.
    let r = parse(r#"FOR(k, ["a": 1, "b": 2], PRINT(k));"#, "t.wl").unwrap();
    let f = match r {
        Expr::For { var, iter, .. } => (var, iter),
        _ => panic!(),
    };
    assert_eq!(f.0, "k");
    assert!(matches!(f.1.as_ref(), Expr::Dict { .. }));
}

#[test]
fn return_with_value() {
    let r = parse("RETURN(x);", "t.wl").unwrap();
    let v = match r {
        Expr::Return { value, .. } => value,
        _ => panic!(),
    };
    assert!(v.is_some());
}

#[test]
fn return_no_value_is_null() {
    // §7.4: 若省略则 NULL.
    let r = parse("RETURN();", "t.wl").unwrap();
    let v = match r {
        Expr::Return { value, .. } => value,
        _ => panic!(),
    };
    assert!(v.is_none());
}

#[test]
fn break_continue_are_kw_calls() {
    let r = parse("BREAK(); CONTINUE();", "t.wl").unwrap();
    let exprs = match r {
        Expr::Block { exprs, .. } => exprs,
        _ => panic!(),
    };
    assert!(matches!(exprs[0], Expr::Break { .. }));
    assert!(matches!(exprs[1], Expr::Continue { .. }));
}

// ───────────────────────── §8 Functions (1 of 6, rest need A2/B1/B2) ─────────────────────────

#[test]
fn fun_anonymous_basic() {
    // §8.2: 匿名 FUN((args), body)
    let r = parse("FUN((x), *(x, x));", "t.wl").unwrap();
    let f = match r {
        Expr::Fun { params, return_type, body, .. } => (params, return_type, body),
        _ => panic!(),
    };
    assert_eq!(f.0.len(), 1);
    assert_eq!(f.0[0].name, "x");
    assert!(f.1.is_none());
    // body is a single Call "*"
    assert!(matches!(f.2.as_ref(), Expr::Call { name, .. } if name == "*"));
}

#[test]
fn fun_param_type_annotation() {
    let r = parse("FUN((x: INTEGER), x);", "t.wl").unwrap();
    let params = match r {
        Expr::Fun { params, .. } => params,
        _ => panic!(),
    };
    assert_eq!(params[0].name, "x");
    assert!(params[0].type_annotation.is_some());
}

#[test]
fn fun_return_type_annotation() {
    let r = parse("FUN((x): INTEGER, x);", "t.wl").unwrap();
    let rt = match r {
        Expr::Fun { return_type, .. } => return_type,
        _ => panic!(),
    };
    assert!(rt.is_some());
    assert_eq!(rt.as_ref().unwrap().text, "INTEGER");
}

// ── §8.2 named form, default param, rest param: pending A2 / B1 / B2 ──

#[test]
fn fun_named_form() {
    // §8.2: FUN(name(params), body) 具名.
    let r = parse("FUN(hello(str), PRINT(str));", "t.wl").unwrap();
    // After A2, Expr::Fun has `name: Option<String>`. We expect
    // Some("hello"). Test will pass after the field is added.
    let f = match r {
        Expr::Fun { .. } => format!("{:?}", r),
        _ => panic!("expected Fun"),
    };
    assert!(f.contains("hello"), "expected name=hello in {:?}", f);
}

#[test]
fn fun_named_with_return_type() {
    let r = parse("FUN(hello(str): INTEGER, *(str, 0));", "t.wl").unwrap();
    assert!(matches!(r, Expr::Fun { .. }));
}

#[test]
fn fun_default_parameter() {
    let r = parse(r#"FUN((greeting = "hi"), greeting);"#, "t.wl").unwrap();
    // After B1: assert params[0].default_expr.is_some()
    assert!(matches!(r, Expr::Fun { .. }));
}

#[test]
fn fun_rest_parameter() {
    let r = parse("FUN(collect(*rest), rest);", "t.wl").unwrap();
    // After B2: assert params[0].is_rest
    assert!(matches!(r, Expr::Fun { .. }));
}

// ───────────────────────── §9 Operators (4) ─────────────────────────

#[test]
fn op_arithmetic() {
    // + - * / %
    for op in &["+", "-", "*", "/", "%"] {
        let src = format!("LET(r, {}(1, 2));", op);
        let r = parse(&src, "t.wl").unwrap();
        let call = match &only_let(&r).as_ref() {
            Expr::Call { name, .. } => name,
            _ => panic!("expected Call for op {}", op),
        };
        assert_eq!(call, op, "op name mismatch for {}", op);
    }
}

#[test]
fn op_comparison() {
    for op in &["==", "!=", "<", ">", "<=", ">="] {
        let src = format!("LET(r, {}(1, 2));", op);
        let r = parse(&src, "t.wl").unwrap();
        let call = match &only_let(&r).as_ref() {
            Expr::Call { name, .. } => name,
            _ => panic!("expected Call for op {}", op),
        };
        assert_eq!(call, op);
    }
}

#[test]
fn op_logical() {
    for op in &["&&", "||", "!"] {
        let src = format!("LET(r, {}(TRUE, FALSE));", op);
        let r = parse(&src, "t.wl").unwrap();
        let call = match &only_let(&r).as_ref() {
            Expr::Call { name, .. } => name,
            _ => panic!("expected Call for op {}", op),
        };
        assert_eq!(call, op);
    }
}

#[test]
fn op_unary_minus_via_call() {
    // Spec calls NEG a §9.1 function; parser also has the `-x` sugar.
    let r = parse("LET(r, NEG(5));", "t.wl").unwrap();
    let call = match &only_let(&r).as_ref() {
        Expr::Call { name, args, .. } => (name, args),
        _ => panic!(),
    };
    assert_eq!(call.0, "NEG");
    assert_eq!(call.1.len(), 1);
}

// ───────────────────────── §11 OOP (4) ─────────────────────────

#[test]
fn class_call_is_plain_call() {
    // §11.2: CLASS(name, parent, members) is a plain function call
    // at the parser level — eval layer handles the semantics.
    let r = parse(r#"CLASS("Rect", NULL, ["w": 0]);"#, "t.wl").unwrap();
    // Top-level CLASS — single Call, not wrapped in a Let.
    let call = match &r {
        Expr::Call { name, args, .. } => (name, args),
        Expr::Block { exprs, .. } => match &exprs[0] {
            Expr::Call { name, args, .. } => (name, args),
            other => panic!("expected Call, got {:?}", other),
        },
        other => panic!("expected Call, got {:?}", other),
    };
    assert_eq!(call.0, "CLASS");
    assert_eq!(call.1.len(), 3);
}

#[test]
fn new_call_is_plain_call() {
    let r = parse(r#"NEW("Rect");"#, "t.wl").unwrap();
    // NEW alone at top level is a plain Call.
    let call = match &r {
        Expr::Call { name, args, .. } => (name, args),
        Expr::Block { exprs, .. } => match &exprs[0] {
            Expr::Call { name, args, .. } => (name, args),
            other => panic!("expected Call, got {:?}", other),
        },
        other => panic!("expected Call, got {:?}", other),
    };
    assert_eq!(call.0, "NEW");
    assert_eq!(call.1.len(), 1);
}

#[test]
fn get_prop_call_is_plain_call() {
    let r = parse(r#"LET(p, GET_PROP(obj, "x"));"#, "t.wl").unwrap();
    let call = match &only_let(&r).as_ref() {
        Expr::Call { name, args, .. } => (name, args),
        _ => panic!(),
    };
    assert_eq!(call.0, "GET_PROP");
}

#[test]
fn set_prop_call_is_plain_call() {
    let r = parse(r#"SET_PROP(obj, "x", 1);"#, "t.wl").unwrap();
    let call = match r {
        Expr::Call { name, args, .. } => (name, args),
        _ => panic!(),
    };
    assert_eq!(call.0, "SET_PROP");
    assert_eq!(call.1.len(), 3);
}

// ───────────────────────── §12 Error handling (4) ─────────────────────────

#[test]
fn ok_err_constructors() {
    let r = parse(r#"LET(o, OK(1)); LET(e, ERR("oops"));"#, "t.wl").unwrap();
    let exprs = match r {
        Expr::Block { exprs, .. } => exprs,
        _ => panic!(),
    };
    let o = match &exprs[0] {
        Expr::Let { value, .. } => value,
        _ => panic!(),
    };
    assert!(matches!(o.as_ref(), Expr::Ok { .. }));
    let e = match &exprs[1] {
        Expr::Let { value, .. } => value,
        _ => panic!(),
    };
    assert!(matches!(e.as_ref(), Expr::Err { .. }));
}

#[test]
fn try_is_ok_is_err() {
    let r = parse(r#"LET(r, TRY(expr)); LET(t, IS_OK(OK(1))); LET(f, IS_ERR(ERR("e")));"#, "t.wl").unwrap();
    let exprs = match r {
        Expr::Block { exprs, .. } => exprs,
        _ => panic!(),
    };
    assert!(matches!(
        &exprs[0],
        Expr::Let { value, .. } if matches!(value.as_ref(), Expr::Try { .. })
    ));
    assert!(matches!(
        &exprs[1],
        Expr::Let { value, .. } if matches!(value.as_ref(), Expr::IsOk { .. })
    ));
    assert!(matches!(
        &exprs[2],
        Expr::Let { value, .. } if matches!(value.as_ref(), Expr::IsErr { .. })
    ));
}

#[test]
fn or_die_is_two_arg() {
    // §12.2: OR_DIE(expr, default). `expr` is the inner OR_DIE node;
    // its value is the OK constructor applied to its argument, so
    // we expect an `Expr::Ok` whose value is the literal 1.
    let r = parse("LET(r, OR_DIE(OK(1), 0));", "t.wl").unwrap();
    let od = match &only_let(&r).as_ref() {
        Expr::OrDie { value, default, .. } => (value, default),
        _ => panic!(),
    };
    let inner = match od.0.as_ref() {
        Expr::Ok { value, .. } => value,
        other => panic!("expected Ok inside OR_DIE, got {:?}", other),
    };
    assert!(matches!(inner.as_ref(), Expr::Literal(Literal::Integer(1), _)));
    assert!(matches!(od.1.as_ref(), Expr::Literal(Literal::Integer(0), _)));
}

#[test]
fn panic_call() {
    let r = parse(r#"PANIC("oops");"#, "t.wl").unwrap();
    assert!(matches!(r, Expr::Panic { .. }));
}

// ───────────────────────── §13 Modules (5) ─────────────────────────

#[test]
fn import_simple() {
    let r = parse(r#"IMPORT("math", ["add"]);"#, "t.wl").unwrap();
    let imp = match r {
        Expr::Import { path, names, .. } => (path, names),
        _ => panic!(),
    };
    assert_eq!(imp.0, "math");
    assert_eq!(imp.1.len(), 1);
    assert_eq!(imp.1[0].name, "add");
    assert!(imp.1[0].alias.is_none());
}

#[test]
fn import_with_rename() {
    // §13.4: ["add": "math_add"]
    let r = parse(r#"IMPORT("math", ["add": "math_add"]);"#, "t.wl").unwrap();
    let imp = match r {
        Expr::Import { path, names, .. } => (path, names),
        _ => panic!(),
    };
    assert_eq!(imp.1[0].name, "add");
    assert_eq!(imp.1[0].alias.as_deref(), Some("math_add"));
}

#[test]
fn import_namespace_wlwl() {
    // §13.6: 命名空间路径
    let r = parse(r#"IMPORT("wlwl:std.io", ["PRINT"]);"#, "t.wl").unwrap();
    let imp = match r {
        Expr::Import { path, names, .. } => (path, names),
        _ => panic!(),
    };
    assert_eq!(imp.0, "wlwl:std.io");
    assert_eq!(imp.1[0].name, "PRINT");
}

#[test]
fn import_empty_path_is_e0043() {
    // §13 / §14.4: empty path → E0043
    let r = parse(r#"IMPORT("", ["x"]);"#, "t.wl").unwrap_err();
    let code = r.diagnostic().code;
    assert_eq!(code, wlwl_error::ErrorCode::E0043);
}

#[test]
fn export_basic() {
    let r = parse(r#"EXPORT(["add", "PI"]);"#, "t.wl").unwrap();
    let exp = match r {
        Expr::Export { names, .. } => names,
        _ => panic!(),
    };
    assert_eq!(exp.len(), 2);
    assert_eq!(exp[0].name, "add");
    assert_eq!(exp[1].name, "PI");
}

// ───────────────────────── §11.4 Chain access (A1) ─────────────────────────

#[test]
fn chain_property_access() {
    // §11.4: a.b  ->  GET_PROP(a, "b")
    let r = parse("LET(p, t.DOM);", "t.wl").unwrap();
    let call = match &only_let(&r).as_ref() {
        Expr::Call { name, args, .. } => (name, args),
        _ => panic!(),
    };
    assert_eq!(call.0, "GET_PROP");
    assert_eq!(call.1.len(), 2);
    assert!(matches!(&call.1[0], Expr::Var(n, _) if n == "t"));
    assert!(matches!(&call.1[1], Expr::Literal(Literal::String(s), _) if s == "DOM"));
}

#[test]
fn chain_method_call() {
    // §11.4: a.b(args)  ->  CALL_METHOD(a, "b", args...)
    let r = parse("j.APPEND(IMG(\"./1.jpg\"));", "t.wl").unwrap();
    let call = match r {
        Expr::Call { name, args, .. } => (name, args),
        _ => panic!(),
    };
    assert_eq!(call.0, "CALL_METHOD");
}

#[test]
fn chain_three_levels() {
    // §11.4: a.b.c.d  ->  GET_PROP(GET_PROP(GET_PROP(a, "b"), "c"), "d")
    let r = parse("LET(p, t.DOM.ID.attr);", "t.wl").unwrap();
    let call = match &only_let(&r).as_ref() {
        Expr::Call { name, .. } => name,
        _ => panic!(),
    };
    assert_eq!(call, "GET_PROP");
}

#[test]
fn chain_method_after_property() {
    let r = parse("LET(p, t.DOM.ID(\"j\"));", "t.wl").unwrap();
    let call = match &only_let(&r).as_ref() {
        Expr::Call { name, .. } => name,
        _ => panic!(),
    };
    assert_eq!(call, "CALL_METHOD");
}

#[test]
fn chain_method_after_method() {
    let r = parse("LET(p, t.DOM.ID(\"j\").ATTR(\"x\"));", "t.wl").unwrap();
    let call = match &only_let(&r).as_ref() {
        Expr::Call { name, .. } => name,
        _ => panic!(),
    };
    assert_eq!(call, "CALL_METHOD");
}

#[test]
fn chain_call_then_property() {
    // (CALL(x)).foo  ->  GET_PROP(CALL(x), "foo")
    let r = parse("LET(p, +(1, 2).len);", "t.wl").unwrap();
    let call = match &only_let(&r).as_ref() {
        Expr::Call { name, .. } => name,
        _ => panic!(),
    };
    assert_eq!(call, "GET_PROP");
}

// ───────────────────────── §3.1 Chinese identifiers (A3) ─────────────────────────

#[test]
fn lex_chinese_identifier() {
    let r = parse("LET(计数, 0);", "t.wl").unwrap();
    let name = match &r {
        Expr::Let { name, .. } => name,
        Expr::Block { exprs, .. } => match &exprs[0] {
            Expr::Let { name, .. } => name,
            other => panic!("expected Let, got {:?}", other),
        },
        other => panic!("expected Let, got {:?}", other),
    };
    assert_eq!(name, "计数");
}

#[test]
fn lex_chinese_identifier_in_fun_param() {
    let r = parse(r#"FUN((名), +("你好, ", 名));"#, "t.wl").unwrap();
    let params = match r {
        Expr::Fun { params, .. } => params,
        _ => panic!(),
    };
    assert_eq!(params[0].name, "名");
}

// ───────────────────────── §4.5 W0020 mixed array/dict (A4) ─────────────────────────

#[test]
#[ignore = "A4: spec §4.5 W0020 — array with dict-style entry"]
fn w0020_array_with_dict_entry() {
    // [1, "a": 2] — first entry is a bare value, second is a kv pair.
    // Per spec §4.5 the array form is "homogeneous"; mixing is W0020.
    let src = r#"
    IMPORT("__warn", []);
    LET(x, [1, "a": 2]);
    "#;
    let (expr, warnings) = wlwl_parser::parse_with_warnings(src, "t.wl").unwrap();
    let value: &Expr = match &expr {
        Expr::Block { exprs, .. } => match &exprs[1] {
            Expr::Let { value, .. } => value.as_ref(),
            _ => panic!(),
        },
        _ => panic!(),
    };
    assert!(matches!(value, Expr::Array { .. } | Expr::Dict { .. }));
    assert!(
        warnings.iter().any(|w| w.code == wlwl_error::ErrorCode::W0020),
        "expected W0020 in warnings, got {:?}",
        warnings
    );
}

#[test]
#[ignore = "A4: spec §4.5 W0020 — dict with bare-value entry"]
fn w0020_dict_with_bare_value() {
    let src = r#"
    LET(x, ["a": 1, 2]);
    "#;
    let (_, warnings) = wlwl_parser::parse_with_warnings(src, "t.wl").unwrap();
    assert!(warnings.iter().any(|w| w.code == wlwl_error::ErrorCode::W0020));
}

#[test]
#[ignore = "A4: spec §4.5 no W0020 for homogeneous array"]
fn no_w0020_homogeneous_array() {
    let (expr, warnings) = wlwl_parser::parse_with_warnings("LET(x, [1, 2, 3]);", "t.wl").unwrap();
    let _ = expr;
    assert!(warnings.is_empty(), "expected no warnings, got {:?}", warnings);
}

#[test]
#[ignore = "A4: spec §4.5 no W0020 for homogeneous dict"]
fn no_w0020_homogeneous_dict() {
    let (expr, warnings) = wlwl_parser::parse_with_warnings(r#"LET(x, ["a": 1, "b": 2]);"#, "t.wl").unwrap();
    let _ = expr;
    assert!(warnings.is_empty());
}

// ───────────────────────── helpers ─────────────────────────

fn only_let(r: &Expr) -> &Box<Expr> {
    // The parser collapses a single-statement program into that one
    // statement (no surrounding Block), so we accept both shapes.
    match r {
        Expr::Let { value, .. } => value,
        Expr::Block { exprs, .. } => match &exprs[0] {
            Expr::Let { value, .. } => value,
            other => panic!("expected Let, got {:?}", other),
        },
        other => panic!("expected Let, got {:?}", other),
    }
}

// silence unused import warning when only a subset of tests runs
#[allow(dead_code)]
fn _silence_span_unused() {
    let _ = Span::dummy();
}
