//! [X-26] **字典字面量的重复键必须折叠** —— 语言规范 §2.1。
//!
//! ## 它补的是什么
//!
//! 语言规范 §2.1 把 `DICT` 定义为「从键(STRING、INTEGER 或 INSTANCE)到值的
//! **插入序映射**」⇒ **映射没有重复键**。缺陷形态:字典**字面量**把重复键
//! **全部**留在条目序列里(`["a": 1, "a": 2]` ⇒ `LEN` = 2、`KEYS` = `[a, a]`、
//! 查值取**首个**)⇒ 该值**不是**其键的函数(与任何单键字典都不相等),
//! 而 `INDEX_SET` 路径**正确折叠** ⇒ **两条构造路径口径不一致**。
//!
//! ## 折叠规则（对齐 `INDEX_SET` 路径）
//!
//! * **后写的值覆盖先写的值**（`["a": 1, "a": 2]` ⇒ `"a"` 的值是 `2`）；
//! * **键的位置保留首次出现的位置**（`["b": 1, "a": 2, "b": 3]` ⇒ 键序
//!   仍是 `[b, a]`，`"b"` 的值是 `3`）。
//!
//! ## 负向验证
//!
//! 把折叠去掉（回到「重复键全部保留」）⇒ **本表至少 8 条变红**
//! （`dict.dupkey.*` / `dict.dupkey.position.*` / `dict.dupkey.value.*`
//! / `dict.dupkey.equal.*` 全部）。

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
    std::env::temp_dir().join("wlwl_x26_dict_dupkey")
}

const CASES: &[Case] = &[
    // ── 折叠本身:重复键只留一项 ────────────────────────────────────────
    Case {
        name: "dict.dupkey.len",
        src: r#"LEN(["a": 1, "a": 2])"#,
        expect: "1",
    },
    Case {
        name: "dict.dupkey.keys",
        src: r#"KEYS(["a": 1, "a": 2])"#,
        expect: "[a]",
    },
    // ⚠ 取值取**后写**的值(与 `INDEX_SET` 路径同口径)。
    Case {
        name: "dict.dupkey.value_is_the_later_one",
        src: r#"["a": 1, "a": 2]["a"]"#,
        expect: "2",
    },
    Case {
        name: "dict.dupkey.equal_to_the_folded_single_key_dict",
        src: r#"=(["a": 1, "a": 2], ["a": 2])"#,
        expect: "TRUE",
    },
    // ⚠ 折叠后**不是**「先写」那个单键字典 —— 缺陷形态下这一条是 FALSE。
    Case {
        name: "dict.dupkey.not_equal_to_the_first_single_key_dict",
        src: r#"=(["a": 1, "a": 2], ["a": 1])"#,
        expect: "FALSE",
    },
    // ── 键的位置保留**首次**出现的位置 ─────────────────────────────────
    Case {
        name: "dict.dupkey.position_is_the_first_occurrence",
        src: r#"KEYS(["b": 1, "a": 2, "b": 3])"#,
        expect: "[b, a]",
    },
    Case {
        name: "dict.dupkey.three_keys_len",
        src: r#"LEN(["a": 1, "b": 2, "a": 3])"#,
        expect: "2",
    },
    Case {
        name: "dict.dupkey.three_keys_value",
        src: r#"["a": 1, "b": 2, "a": 3]["a"]"#,
        expect: "3",
    },
    // ── 两条构造路径**逐字同口径** ─────────────────────────────────────
    Case {
        name: "dict.dupkey.literal_equals_the_index_set_path",
        src: r#"=(["a": 1, "a": 2], INDEX_SET(INDEX_SET(DICT(), "a", 1), "a", 2))"#,
        expect: "TRUE",
    },
    // ⚠ `INDEX_SET` 路径的既有口径**不许**被这次改动带偏(回归守卫)。
    Case {
        name: "dict.dupkey.index_set_path_is_unchanged",
        src: r#"LEN(INDEX_SET(INDEX_SET(DICT(), "a", 1), "a", 2))"#,
        expect: "1",
    },
    Case {
        name: "dict.dupkey.index_set_keeps_the_first_position",
        src: r#"KEYS(INDEX_SET(INDEX_SET(INDEX_SET(DICT(), "b", 1), "a", 2), "b", 3))"#,
        expect: "[b, a]",
    },
    // ── 无重复键的普通字面量**逐字不变**(回归守卫) ─────────────────────
    Case {
        name: "dict.plain_literal_is_unchanged",
        src: r#"KEYS(["b": 1, "a": 2, "c": 3])"#,
        expect: "[b, a, c]",
    },
    Case {
        name: "dict.plain_literal_len_is_unchanged",
        src: r#"LEN(["b": 1, "a": 2, "c": 3])"#,
        expect: "3",
    },
];

#[test]
fn a_dict_literal_folds_duplicate_keys_like_the_index_set_path() {
    for c in CASES {
        let got = actual(c.src);
        assert_eq!(got, c.expect, "case {}: {}", c.name, c.src);
    }
}

/// ⚠ **不同类型的同名键不是重复键**:`"1"`(STRING)与 `1`(INTEGER)在
///   §2.4 下**不相等** ⇒ 两条都在(缺陷形态下也是两条 ⇒ 这条是**回归守卫**,
///   防止折叠时把「键相等」错写成「渲染相等」)。
#[test]
fn keys_of_different_types_are_not_duplicates() {
    assert_eq!(actual(r#"LEN(["1": 1, 1: 2])"#), "2");
    assert_eq!(actual(r#"KEYS(["1": 1, 1: 2])"#), r#"[1, 1]"#);
}
