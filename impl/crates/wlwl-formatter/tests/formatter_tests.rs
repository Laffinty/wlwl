//! Phase E2 formatter tests (plan §5.10).
//!
//! The centerpiece is `fmt_idempotent_over_100_fixtures`: 100+
//! programs (handwritten per-§16.3-rule fixtures + combinatorial
//! fold-boundary generators) must satisfy
//! `fmt(fmt(x)) == fmt(x)` and preserve the span-stripped AST.

use wlwl_formatter::format;
use wlwl_parser::parse;

fn fmt_src(src: &str) -> String {
    format(&parse(src, "t.wl").expect("parse failed"))
}

/// fmt(fmt(x)) == fmt(x) for source text (the plan §5.10 contract).
fn assert_idempotent(src: &str) {
    let once = fmt_src(src);
    let twice = format(&parse(&once, "t.wl").expect("re-parse of fmt output failed"));
    assert_eq!(once, twice, "not idempotent for source:\n{}", src);
}

/// Stronger property: the span-stripped AST of the formatted output
/// equals the span-stripped AST of the input, i.e. fmt never changes
/// semantics.
fn assert_semantics_preserved(src: &str) {
    let strip = |text: &str| -> serde_json::Value {
        let mut v = serde_json::to_value(parse(text, "t.wl").expect("parse failed")).unwrap();
        strip_spans(&mut v);
        v
    };
    let once = fmt_src(src);
    assert_eq!(strip(src), strip(&once), "semantics changed for:\n{}", src);
}

fn strip_spans(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            // Spans appear both under the named key `span` and as
            // positional variant payloads; recognize them by shape.
            if map.len() == 5
                && map.contains_key("file")
                && map.contains_key("line_start")
                && map.contains_key("col_start")
                && map.contains_key("line_end")
                && map.contains_key("col_end")
            {
                *v = serde_json::Value::Null;
                return;
            }
            for (_, child) in map.iter_mut() {
                strip_spans(child);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                strip_spans(item);
            }
        }
        _ => {}
    }
}

// ── rule-by-rule unit tests (spec §16.3) ────────────────────────────

#[test]
fn rule1_indent_4_spaces_no_tabs() {
    let out = fmt_src("LET(f, FUN((x),\n\t\tIF(==(x, 1), LET(y, 2); y, 0)\n));");
    assert!(!out.contains('\t'), "tabs must be normalized: {}", out);
    for line in out.lines() {
        let indent = line.len() - line.trim_start().len();
        assert_eq!(indent % 4, 0, "indent not multiple of 4: {:?}", line);
    }
}

#[test]
fn rule3_statements_semicolons_last_expr_none() {
    let out = fmt_src("LET(x, 1); PRINT(x);");
    assert_eq!(out, "LET(x, 1);\nPRINT(x)\n");
}

#[test]
fn rule3_semicolon_required_between_statements_in_block() {
    // A multi-statement block has no inline form, so the IF folds;
    // the block's statements sit at the folded-arg indent (+8).
    let out = fmt_src("IF(TRUE, LET(x, 1); LET(y, 2), 0);");
    assert_eq!(
        out,
        "IF(\n        TRUE,\n        LET(x, 1);\n        LET(y, 2),\n        0\n)\n"
    );
}

#[test]
fn rule6_whitespace_normalized() {
    // `( x ,1 )` spacing is normalized by rebuilding from the AST.
    let out = fmt_src("LET( x ,1 );PRINT( x );");
    assert_eq!(out, "LET(x, 1);\nPRINT(x)\n");
}

#[test]
fn rule8_import_export_one_per_line() {
    let out = fmt_src("IMPORT(\"m\", [\"a\", \"b\"]); IMPORT(\"n\", [\"c\"]); EXPORT([\"a\"]);");
    assert_eq!(
        out,
        "IMPORT(\"m\", [\"a\", \"b\"]);\nIMPORT(\"n\", [\"c\"]);\nEXPORT([\"a\"])\n"
    );
}

#[test]
fn rule8_import_alias_uses_kv_form() {
    let out = fmt_src("IMPORT(\"m\", [\"add\": \"math_add\"]);");
    assert_eq!(out, "IMPORT(\"m\", [\"add\": \"math_add\"])\n");
}

#[test]
fn rule9_class_members_fold_one_per_line() {
    let out = fmt_src("CLASS(\"Rect\", NULL, [\"x\", \"y\", \"area\"]);");
    assert_eq!(
        out,
        "CLASS(\"Rect\", NULL, [\n    \"x\",\n    \"y\",\n    \"area\"\n])\n"
    );
}

#[test]
fn rule10_empty_collections() {
    // `DICT()` / `ARRAY()` as source parse as zero-arg calls and are
    // canonical already; the AST-level rule is covered by the empty
    // Array test below.
    let out = fmt_src("LET(a, ARRAY()); LET(d, DICT());");
    assert_eq!(out, "LET(a, ARRAY());\nLET(d, DICT())\n");
}

#[test]
fn rule10_empty_array_ast_renders_array_call() {
    // An empty `[]` in source parses to Expr::Array with no items;
    // the canonical rebuild renders it as `ARRAY()` (§16.3 rule 10,
    // §4.5 decision). Note this is a text-level canonicalization: the
    // re-parsed AST is a zero-arg `ARRAY()` call, so the
    // semantics-preservation property does not apply here.
    let out = fmt_src("LET(a, []);");
    assert_eq!(out, "LET(a, ARRAY())\n");
    assert_idempotent("LET(a, []);");
}

#[test]
fn rule2_long_call_folds_one_arg_per_line_plus8() {
    // > 100 chars when inline: must fold, args at +8, no trailing comma.
    let out = fmt_src(
        "CALL_SOME_FUNCTION_WITH_A_VERY_LONG_NAME(argument_number_one, argument_number_two, argument_number_three);",
    );
    let first = out.lines().next().unwrap();
    assert_eq!(first, "CALL_SOME_FUNCTION_WITH_A_VERY_LONG_NAME(");
    for line in out.lines().skip(1) {
        let indent = line.len() - line.trim_start().len();
        if line.starts_with(')') {
            assert_eq!(indent, 0, "closing paren at column 0: {:?}", line);
        } else {
            assert_eq!(indent, 8, "folded arg at +8: {:?}", line);
        }
    }
    assert!(!out.contains(",\n)"), "no trailing comma: {}", out);
}

#[test]
fn rule2_short_call_stays_inline() {
    let out = fmt_src("PRINT(1, 2, 3);");
    assert_eq!(out, "PRINT(1, 2, 3)\n");
}

#[test]
fn rule5_string_escapes_round_trip() {
    let src = "LET(s, \"a\\\"b\\\\c\\nd\\te\\rf\\0g 中文\");";
    let out = fmt_src(src);
    assert_eq!(out, "LET(s, \"a\\\"b\\\\c\\nd\\te\\rf\\0g 中文\")\n");
    assert_semantics_preserved(src);
}

#[test]
fn float_one_point_zero_stays_float() {
    let out = fmt_src("LET(x, 1.0);");
    assert_eq!(out, "LET(x, 1.0)\n");
    assert_semantics_preserved("LET(x, 1.0);");
}

#[test]
fn type_annotation_canonical_bracket_form() {
    let out = fmt_src("LET(f, FUN((x: integer): ARRAY[INTEGER], x));");
    assert_eq!(out, "LET(f, FUN((x: integer): ARRAY[INTEGER], x))\n");
}

#[test]
fn fun_param_forms_render() {
    let out = fmt_src("LET(f, FUN((a: INTEGER, b = 5, *rest), a));");
    assert_eq!(out, "LET(f, FUN((a: INTEGER, b = 5, *rest), a))\n");
}

#[test]
fn named_fun_renders() {
    let out = fmt_src("LET(f, FUN(double(x), *(x, 2)));");
    assert_eq!(out, "LET(f, FUN(double(x), *(x, 2)))\n");
}

#[test]
fn match_renders_with_explicit_default() {
    let out = fmt_src("MATCH(x, [[1, \"one\"], [OK(v), v]], NULL);");
    assert!(out.starts_with("MATCH("), "got: {}", out);
    assert!(out.contains("\"one\""), "got: {}", out);
    assert_semantics_preserved("MATCH(x, [[1, \"one\"], [OK(v), v]], NULL);");
}

#[test]
fn destructuring_let_renders() {
    let out = fmt_src("LET([a, b, *rest], arr); LET([\"k\": v], d);");
    assert_eq!(out, "LET([a, b, *rest], arr);\nLET([\"k\": v], d)\n");
}

#[test]
fn control_flow_renders() {
    let out = fmt_src("WHILE(TRUE, BREAK()); FOR(i, items, PRINT(i)); RETURN();");
    assert_eq!(
        out,
        "WHILE(TRUE, BREAK());\nFOR(i, items, PRINT(i));\nRETURN()\n"
    );
}

#[test]
fn unary_minus_desugar_reparses() {
    assert_semantics_preserved("LET(a, -1); LET(b, !TRUE);");
    let out = fmt_src("LET(a, -x);");
    // `-x` desugars to the binary-minus call at parse time; the
    // canonical rebuild prints the desugared form, which parses back
    // to the same AST.
    assert_eq!(out, "LET(a, -(0, x))\n");
}

#[test]
fn result_ctors_render() {
    let src = "OR_DIE(PARSE(\"{}\"), DICT()); IS_OK(OK(1)); IS_ERR(ERR(2)); TRY(PANIC(3));";
    assert_semantics_preserved(src);
    let out = fmt_src(src);
    assert!(out.starts_with("OR_DIE("), "got: {}", out);
}

#[test]
fn empty_program_formats_to_empty() {
    assert_eq!(fmt_src(""), "");
    assert_eq!(fmt_src("// just a comment\n"), "");
}

#[test]
fn comments_are_dropped_known_limitation() {
    // Deviations P4-E2-001: the AST carries no comment nodes, so the
    // canonical rebuild drops them. This test pins that behavior so
    // a future comment-preserving change is deliberate.
    let out = fmt_src("// header\nLET(x, 1); // trailing\n");
    assert_eq!(out, "LET(x, 1)\n");
}

// ── idempotency over 100+ fixtures (plan §5.10) ─────────────────────

/// Handwritten fixtures covering every §16.3 rule and every AST node
/// kind.
fn handwritten_fixtures() -> Vec<&'static str> {
    vec![
        "PRINT(1);",
        "LET(x, 1);",
        "LET(x, 1); PRINT(x);",
        "LET(x, 1.0);",
        "LET(x, 1.5);",
        "LET(s, \"hello 世界\");",
        "LET(s, \"tab\\tnewline\\nquote\\\"backslash\\\\\");",
        "LET(b, TRUE);",
        "LET(n, NULL);",
        "LET(a, [1, 2, 3]);",
        "LET(a, ARRAY());",
        "LET(d, [\"a\": 1, \"b\": 2]);",
        "LET(d, DICT());",
        "LET(nested, [[1, 2], [\"k\": [3, 4]]]);",
        "LET(v, x);",
        "LET(sum, +(1, 2));",
        "LET(eq, ==(1, 2));",
        "LET(lt, <=(1, 2));",
        "LET(and, &&(TRUE, FALSE));",
        "LET(neq, !=(1, 2));",
        "LET(mod, %(7, 3));",
        "LET(div, /(7, 3));",
        "LET(neg, -(0, 5));",
        "LET(not, !(TRUE));",
        "IF(TRUE, 1, 2);",
        "IF(FALSE, 1);",
        "IF(==(1, 1),\n    LET(x, 1);\n    PRINT(x),\n    PRINT(\"no\")\n);",
        "WHILE(FALSE, BREAK());",
        "WHILE(TRUE,\n    LET(x, 1);\n    IF(x, BREAK(), CONTINUE())\n);",
        "FOR(i, [1, 2, 3], PRINT(i));",
        "FOR(i, items,\n    LET(square, *(i, i));\n    PRINT(square)\n);",
        "RETURN(1);",
        "RETURN();",
        "BREAK();",
        "CONTINUE();",
        "LET(f, FUN((x), x));",
        "LET(f, FUN((a, b), +(a, b)));",
        "LET(f, FUN((a: INTEGER, b: STRING), a));",
        "LET(f, FUN(double(x), *(x, 2)));",
        "LET(f, FUN((x): INTEGER, x));",
        "LET(f, FUN((a = 1, *rest), a));",
        "LET(f, FUN((x),\n    LET(y, *(x, 2));\n    y\n));",
        "LET(f, FUN((x),\n    LET(y, *(x, 2));\n    IF(>(y, 10),\n        PRINT(\"big\");\n        y,\n        y\n)\n));",
        "OK(1);",
        "ERR(\"boom\");",
        "PANIC(\"bad\");",
        "TRY(OK(1));",
        "IS_OK(OK(1));",
        "IS_ERR(ERR(2));",
        "OR_DIE(OK(1), 0);",
        "MATCH(x, [[1, \"one\"]], NULL);",
        "MATCH(x, [[1, \"one\"], [_, \"other\"]], \"default\");",
        "MATCH(v,\n    [[OK(x), x], [ERR(e), e]],\n    NULL\n);",
        "LET([a, b], arr);",
        "LET([a, *rest], arr);",
        "LET([\"k\": v], dict);",
        "LET(_, 1);",
        "IMPORT(\"./math\", [\"add\"]);",
        "IMPORT(\"wlwl:std.io\", [\"PRINT\"]);",
        "IMPORT(\"m\", [\"add\": \"math_add\", \"PI\"]);",
        "EXPORT([\"add\", \"PI\"]);",
        "CLASS(\"Rect\", NULL, [\"x\", \"y\"]);",
        "CLASS(\"Square\", \"Rect\", [\"side\"]);",
        "LET(c, CALL(f, 1, 2));",
        "LET(g, INDEX_GET(arr, 0));",
        "LET(r, RANGE(1, 10, 2));",
        "LET(f2, FORMAT(\"{} and {}\", 1, 2));",
        "LET(big, 9223372036854775807);",
        "LET(deep, FUN((x), FUN((y), +(x, y))));",
        "LET(apply, CALL(FUN((f, v), CALL(f, v)), FUN((x), x), 42));",
        "IF(TRUE, IF(FALSE, 1, 2), 3);",
        "LET(chain, INDEX_GET(INDEX_GET(matrix, 0), 1));",
        "LET(mix, [1, \"two\", [3], [\"four\": 5], NULL, TRUE]);",
        "LET(closure, FUN((), 42));",
        "LET(ann, FUN((a: INTEGER, b: ARRAY[STRING], c: DICT[STRING, INTEGER]), a));",
        "LET(un, UNWRAP(OR_DIE(PARSE(\"[1,2]\"), ARRAY())));",
    ]
}

/// Generated fixtures: combinatorial call shapes that stress the
/// fold boundary (inline vs folded, nested chains, block sizes).
fn generated_fixtures() -> Vec<String> {
    let mut out = Vec::new();
    for nargs in 1..=6usize {
        for pad in 0..=12usize {
            let name = format!("FN{}_{}", nargs, "x".repeat(pad));
            let args: Vec<String> = (0..nargs)
                .map(|i| format!("argument_value_{}", i))
                .collect();
            out.push(format!("{}({});", name, args.join(", ")));
            let mut nested = "1".to_string();
            for _ in 0..nargs {
                nested = format!("WRAP({})", nested);
            }
            out.push(format!("LET(n, {});", nested));
        }
    }
    for nstmt in 1..=5usize {
        let body: Vec<String> = (0..nstmt).map(|i| format!("LET(v{}, {})", i, i)).collect();
        out.push(format!("IF(TRUE, {}, PRINT(\"done\"));", body.join("; ")));
        out.push(format!("LET(f, FUN((x), {}));", body.join("; ")));
    }
    out
}

#[test]
fn fmt_idempotent_over_100_fixtures() {
    let mut total = 0usize;
    for src in handwritten_fixtures() {
        assert_idempotent(src);
        assert_semantics_preserved(src);
        total += 1;
    }
    for src in generated_fixtures() {
        assert_idempotent(&src);
        total += 1;
    }
    assert!(
        total >= 100,
        "fixture count must reach 100 (plan §5.10), got {}",
        total
    );
}

#[test]
fn fmt_output_always_reparses() {
    for src in handwritten_fixtures() {
        let once = fmt_src(src);
        parse(&once, "t.wl").expect("fmt output must be valid WLWL source");
    }
}

#[test]
fn fmt_examples_dir_files_idempotent() {
    // The repo's own example programs must be idempotent too.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).expect("examples dir") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("wl") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        assert_idempotent(&src);
        checked += 1;
    }
    assert!(checked >= 3, "expected to check several examples");
}
