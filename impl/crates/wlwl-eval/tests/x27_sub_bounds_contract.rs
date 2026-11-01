//! [X-27] `SUB` / `CHAR_AT` 的**越界口径正负对称** —— 语言规范 §10.5 +
//! 标准库·核心册 §6。
//!
//! ## 它补的是什么
//!
//! 标准库·核心册 §6 的 `CHAR_AT` 行写「越界按 `SUB` 口径取到**空串**,不报错」,
//! 且同册 §6 前言写「起点同 `SUB` 的 `start` —— 含**负索引自尾部计数**」。
//!
//! 缺陷形态:**正**越界取空串(`SUB("abc", 9, 1)` ⇒ `""`),**负**越界却被
//! **钳到 0**(`SUB("abc", -9, 1)` ⇒ `"a"`,取首字符)⇒ 正/负两侧口径
//! **不对称** ⇒ 按「越界取空串」写断言的人会写错。
//!
//! ## 负向验证
//!
//! 把负侧越界改回「钳到 0」⇒ **本表至少 7 条变红**
//! (`sub.neg_oob.*` / `char_at.neg_oob.*` / `sub.neg_oob.i64_min.*`)。

use std::path::PathBuf;
use wlwl_eval::Evaluator;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

/// 跑一段源码,返回「诊断形态」(`!CODE message`)或「值形态」(`Value::display`)。
fn actual(src: &str) -> String {
    let ast = match wlwl_parser::parse(src, "t.wll") {
        Ok(a) => a,
        Err(e) => return format!("!PARSE {}", e.diagnostic().message),
    };
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => format!(
            "!{} {}",
            e.diagnostic().code.as_str(),
            e.diagnostic().message
        ),
    }
}

fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join("wlwl_x27_sub_bounds")
}

const CASES: &[Case] = &[
    // ── 负侧越界 ⇒ 空串(与正侧同口径) ─────────────────────────────────
    Case {
        name: "sub.neg_oob",
        src: r#"SUB("abc", -9, 1)"#,
        expect: "",
    },
    Case {
        name: "sub.neg_oob.just_past_the_tail",
        src: r#"SUB("abc", -4, 1)"#,
        expect: "",
    },
    Case {
        name: "sub.neg_oob.no_length_argument",
        src: r#"SUB("abc", -9)"#,
        expect: "",
    },
    Case {
        name: "sub.neg_oob.i64_min",
        // ⚠ 极端负起点:实现必须**不溢出**(`saturating_add`)且取空串。
        src: r#"SUB("abc", -9223372036854775807, 1)"#,
        expect: "",
    },
    Case {
        name: "char_at.neg_oob",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["CHAR_AT"]); "#,
            r#"CHAR_AT("abc", -9)"#
        ),
        expect: "",
    },
    Case {
        name: "char_at.neg_oob.just_past_the_tail",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["CHAR_AT"]); "#,
            r#"CHAR_AT("abc", -4)"#
        ),
        expect: "",
    },
    // ── 正侧越界 ⇒ 空串(既有口径,回归守卫) ───────────────────────────
    Case {
        name: "sub.pos_oob",
        src: r#"SUB("abc", 9, 1)"#,
        expect: "",
    },
    Case {
        name: "char_at.pos_oob",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["CHAR_AT"]); "#,
            r#"CHAR_AT("abc", 9)"#
        ),
        expect: "",
    },
    // ── 范围内的负索引**逐字不变**(回归守卫) ─────────────────────────
    Case {
        name: "sub.neg_in_range.tail",
        src: r#"SUB("abc", -1, 1)"#,
        expect: "c",
    },
    Case {
        name: "sub.neg_in_range.head",
        src: r#"SUB("abc", -3, 1)"#,
        expect: "a",
    },
    Case {
        name: "sub.neg_in_range.span",
        src: r#"SUB("abc", -2, 5)"#,
        expect: "bc",
    },
    Case {
        name: "char_at.neg_in_range",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["CHAR_AT"]); "#,
            r#"CHAR_AT("abc", -1)"#
        ),
        expect: "c",
    },
    // ── 正侧起点**逐字不变**(回归守卫) ───────────────────────────────
    Case {
        name: "sub.pos_in_range",
        src: r#"SUB("abc", 1, 1)"#,
        expect: "b",
    },
    Case {
        name: "sub.pos_at_the_end",
        src: r#"SUB("abc", 3, 1)"#,
        expect: "",
    },
    // ── `CHAR_AT` 仍**等价于** `SUB(s, i, 1)`(核心册 §6 的定义) ───────
    Case {
        name: "char_at.is_sub",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["CHAR_AT"]); "#,
            r#"=(CHAR_AT("abc", -2), SUB("abc", -2, 1))"#
        ),
        expect: "TRUE",
    },
];

#[test]
fn sub_and_char_at_take_an_empty_string_when_the_start_is_out_of_range() {
    for c in CASES {
        let got = actual(c.src);
        assert_eq!(got, c.expect, "case {}: {}", c.name, c.src);
    }
}

/// ⚠ **越界判定不许吞掉第三个实参的类型校验**:缺陷形态若把「越界 ⇒ 空串」
///   提前到类型检查之前,`SUB("abc", -9, "x")` 会从 `E0030` 变成静默 `""`
///   ⇒ 那是一条**静默吞错**,比原缺陷更坏。
#[test]
fn the_length_argument_is_still_type_checked_when_the_start_is_out_of_range() {
    assert_eq!(
        actual(r#"SUB("abc", -9, "x")"#),
        "!E0030 SUB: len must be INTEGER, got string"
    );
}
