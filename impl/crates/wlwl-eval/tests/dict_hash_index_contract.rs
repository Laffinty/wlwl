//! [ADR-0035] 字典的**内部哈希索引**与 **copy-on-write 写路径**的可观察面守卫。
//!
//! 规范依据(语言核心册 v1.0):
//!
//! - **§18.3.1**「字典的**键序是插入序**,这一点是可观察的(§2.1、§2.5:JSON 渲染、
//!   错误载荷、任务结果都依赖它)。O(1) 期望的键查找与插入序**不冲突**:实现
//!   **可以**在内部维护一张哈希索引指向位置,对外仍按插入序呈现。」
//! - **§18.3.1**「实现**不得**因为哈希而改变可观察的键序。」
//! - **§2.1**「键**必须**是 `STRING`、`INTEGER` 或 `INSTANCE`。`INSTANCE` 作键按
//!   **对象身份**判定(§2.4)。」
//! - **§2.4** 数字跨类型比较;`CLASS` / `INSTANCE` 按**对象身份**恒等。
//! - **§19.1** 值不可变;值的生存期由绑定决定 ⇒ **别名下旧值必须原样可见**。
//!
//! ⚠ 这份守卫的**分工**:哈希索引是**内部**实现(§18.3.1 明写「键的哈希由实现内部
//!   决定」)⇒ 本文件钉的是**可观察面**:键序、查找结果、相等语义、别名下的值语义。
//!   索引**本身**的复杂度读数由 ADR-0035 的 S5(计数守卫)负责 —— 本文件**不**替代它。
//!
//! ⚠ JSON 序列化的键序**不在**本文件:`STRINGIFY` 走 `serde_json::Map`(按键排序),
//!   那条基线由 `json_contract.rs::stringify_surface_is_frozen` 钉住。

mod common;

use std::path::PathBuf;
use wlwl_eval::Evaluator;

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_dictidx")
}

/// 跑一段源,回显示值 / `!CODE message`。
fn out(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
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

// ──────────────────────────────────────────────────────────────────────
// 一、插入序 = 可观察序(§18.3.1 / §2.5)
// ──────────────────────────────────────────────────────────────────────
//
// ⚠ 这一组**全是只读断言**:钉的是「哈希索引不改变键序」这条**不变量**。
//   键名故意取成**非字典序**(zeta / alpha / mid)⇒ 任何「顺手排序」的改动都会红。

#[test]
fn dict_literal_keeps_insertion_order_in_keys_values_and_display() {
    // KEYS / VALUES / display 三条呈现面都走**插入序**。
    assert_eq!(
        out(r#"KEYS(["zeta": 1, "alpha": 2, "mid": 3])"#),
        "[zeta, alpha, mid]"
    );
    assert_eq!(
        out(r#"VALUES(["zeta": 1, "alpha": 2, "mid": 3])"#),
        "[1, 2, 3]"
    );
    // §2.5:字典渲染为 `[k: v, ...]`。
    assert_eq!(
        out(r#"["zeta": 1, "alpha": 2, "mid": 3]"#),
        "[zeta: 1, alpha: 2, mid: 3]"
    );
}

#[test]
fn insertion_order_survives_a_lookup_that_misses() {
    // 一次**没命中**的查找不得把字典的键序弄乱(哈希索引不得在查找时重排)。
    assert_eq!(
        out(r#"LET(d, ["zeta": 1, "alpha": 2]); AT_K(d, "nope", "miss"); KEYS(d)"#),
        "[zeta, alpha]"
    );
    // 命中也不得改序。
    assert_eq!(
        out(r#"LET(d, ["zeta": 1, "alpha": 2]); INDEX_GET(d, "zeta"); KEYS(d)"#),
        "[zeta, alpha]"
    );
}

#[test]
fn insertion_order_survives_a_has_probe() {
    assert_eq!(
        out(r#"LET(d, ["zeta": 1, "alpha": 2]); HAS(d, "alpha"); KEYS(d)"#),
        "[zeta, alpha]"
    );
}

#[test]
fn integer_keys_keep_insertion_order_too() {
    // 整数键与字符串键**混**在同一个字典里时,顺序仍按插入序。
    assert_eq!(out(r#"KEYS([3: "c", "b": "b", 1: "a"])"#), "[3, b, 1]");
}

// ──────────────────────────────────────────────────────────────────────
// 二、查找语义必须与 §2.4 逐字一致(索引只换「怎么找」,不换「什么算相等」)
// ──────────────────────────────────────────────────────────────────────

#[test]
fn string_and_integer_keys_are_different_keys() {
    // ⚠ `"1"` 与 `1` 在 §2.4 下**不相等**(不是数字跨类型比较那一对)。
    //   哈希索引的键域把两族分开(`DictKey::Str` / `DictKey::Int`)⇒ 不能互相命中。
    assert_eq!(out(r#"INDEX_GET(["1": "s", 1: "i"], "1")"#), "s");
    assert_eq!(out(r#"INDEX_GET(["1": "s", 1: "i"], 1)"#), "i");
    assert_eq!(out(r#"HAS(["1": "s"], 1)"#), "FALSE");
    assert_eq!(out(r#"HAS([1: "i"], "1")"#), "FALSE");
}

#[test]
fn instance_keys_are_matched_by_identity_not_by_content() {
    // §2.1:「`INSTANCE` 作键按**对象身份**判定(§2.4):同一实例命中,异实例不命中。」
    let same = r#"LET(C, CLASS("C", NULL, [])); LET(a, NEW(C)); LET(d, INDEX_SET(DICT(), a, "hit")); AT_K(d, a, "miss")"#;
    assert_eq!(out(same), "hit");
    // 另一个实例 ⇒ 不命中(即使字段相同)。
    let other = r#"LET(C, CLASS("C", NULL, [])); LET(a, NEW(C)); LET(b, NEW(C)); LET(d, INDEX_SET(DICT(), a, "hit")); AT_K(d, b, "miss")"#;
    assert_eq!(out(other), "miss");
}

#[test]
fn duplicate_keys_resolve_to_the_first_occurrence() {
    // ⚠ 字典字面量可以写重复键。旧实现是**线性扫描取第一个匹配** ⇒
    //   哈希索引必须保留**首次**出现的位置,否则这里会从 1 变成 2。
    assert_eq!(out(r#"INDEX_GET(["a": 1, "a": 2], "a")"#), "1");
    assert_eq!(out(r#"LEN(["a": 1, "a": 2])"#), "2");
}

// ──────────────────────────────────────────────────────────────────────
// 三、copy-on-write 写路径(ADR-0035 D3)+ 值语义(§19.1)
// ──────────────────────────────────────────────────────────────────────

#[test]
fn writing_to_an_aliased_dict_leaves_the_alias_untouched() {
    // ⚠ 这是 D3 的**语义**面:原地改只在**旧值不可观察**时合法。
    //   这里 `d` 与 `e` 指向同一个字典 ⇒ 写 `d` 不得改 `e`。
    assert_eq!(
        out(r#"LET MUT(d, ["a": 1]); LET(e, d); SET(d, INDEX_SET(d, "a", 2)); e"#),
        "[a: 1]"
    );
    assert_eq!(
        out(r#"LET MUT(d, ["a": 1]); LET(e, d); SET(d, INDEX_SET(d, "a", 2)); d"#),
        "[a: 2]"
    );
}

#[test]
fn writing_to_a_dict_that_is_only_held_once_still_produces_the_right_value() {
    // 唯一拥有 ⇒ 原地改(旧值不可观察 ⇒ 合法)。可观察面必须与「克隆后改」**逐字相同**。
    assert_eq!(out(r#"INDEX_SET(["a": 1], "b", 2)"#), "[a: 1, b: 2]");
    assert_eq!(out(r#"INDEX_SET(["a": 1], "a", 2)"#), "[a: 2]");
}

#[test]
fn index_set_appends_new_keys_and_keeps_the_position_of_updated_ones() {
    // §18.3.1:「键序是插入序,新键追加到末尾」。
    assert_eq!(
        out(r#"LET MUT(d, ["b": 1]); SET(d, INDEX_SET(d, "a", 2)); KEYS(d)"#),
        "[b, a]"
    );
    // 已存在的键**就地**改值,位置不动。
    assert_eq!(
        out(r#"LET MUT(d, ["b": 1, "a": 2]); SET(d, INDEX_SET(d, "b", 9)); KEYS(d)"#),
        "[b, a]"
    );
    assert_eq!(
        out(r#"LET MUT(d, ["b": 1, "a": 2]); SET(d, INDEX_SET(d, "b", 9)); INDEX_GET(d, "b")"#),
        "9"
    );
}

#[test]
fn remove_key_and_merge_keep_the_remaining_order() {
    assert_eq!(
        out(r#"LET MUT(d, ["a": 1, "b": 2, "c": 3]); SET(d, REMOVE_KEY(d, "b")); KEYS(d)"#),
        "[a, c]"
    );
    // MERGE:b 的新键按 b 的顺序**追加**到末尾;同键时 b 的值覆盖 a 的值。
    assert_eq!(
        out(r#"KEYS(MERGE(["a": 1], ["b": 2, "c": 3]))"#),
        "[a, b, c]"
    );
    assert_eq!(out(r#"INDEX_GET(MERGE(["a": 1], ["a": 2]), "a")"#), "2");
}

// ──────────────────────────────────────────────────────────────────────
// 四、累加器 cell(ADR-0032 的 `Value::Cell`)与字典的交互
// ──────────────────────────────────────────────────────────────────────
//
// ⚠ `Value::Cell` 是**共享可变数组**(ADR-0032 D1)。字典进 cell 之后,
//   「就地改」这条路**必须**仍然只对**唯一拥有者**开放 ——
//   cell 里的那份是**别名**,写它不得反过来改到 cell 里的内容。

#[test]
fn a_dict_stored_in_an_accumulator_cell_is_not_mutated_through_an_alias() {
    assert_eq!(
        out(
            r#"LET MUT(a, []); SET(a, +(a, [["k": 1]])); LET MUT(d, INDEX_GET(a, 0)); SET(d, INDEX_SET(d, "k", 2)); INDEX_GET(a, 0)"#
        ),
        "[k: 1]"
    );
}
