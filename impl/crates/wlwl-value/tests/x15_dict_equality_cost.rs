//! [X-15] 字典**深度相等**的复杂度与语义契约。
//!
//! 规范依据(语言核心册 v1.0):
//!
//! - **§18.3**「深度相等 `==(a, b)`:**O(n) 最坏**」—— **规范性**上界。
//! - **§2.4** 字典相等是**集合口径**:`x` 的每个条目都要在 `y` 里找到一个键值
//!   都相等的条目 —— **不看顺序**;`INSTANCE` 作键按**对象身份**判定。
//! - **§2.1** 键域封闭:`STRING` / `INTEGER` / `INSTANCE`。
//!
//! ## 这份契约钉两件事
//!
//! ① **复杂度**:键比较次数 **≤ c·n**(c = 4),不再是 n(n+1)/2。
//!    计量点是**确定性工作量计数**(`wlwl_value::value_work_counters()` 的
//!    `equality_key_comparisons`)—— 规范 §18.5.2 的三条硬要求(不用墙钟比值 /
//!    计量点收在被测结构内部 / 带下界断言)逐条照做。
//!
//! ② **语义一条不差**:相等**仍是集合口径**(顺序无关)、域外键**仍走线性回退**
//!    (判定结果与收敛前逐字一致)、`INSTANCE` 键仍按**身份**比、**重复键**形态
//!    仍满足「`x` 的每个条目都要在 `y` 里有一个键值都相等的匹配」。
//!
//! ## 计数口径(两条路径,两种读数 —— 别混)
//!
//! | 路径 | 何时 | 每个 `x` 条目的键比较次数 |
//! |---|---|---|
//! | **哈希路径** | 两侧的键**全在封闭域内** | 1 次探测 + 每个**候选** 1 次核对 ⇒ 键互异时**恰好 2** |
//! | **线性回退** | 存在**域外键**(`FLOAT` / 布尔 …) | 按**实际扫描过**的条目数计(旧实现原样) |
//!
//! ⚠ 语言表面**不许**域外键(`INDEX_GET` / `INDEX_SET` / 字面量三处都报
//!   `E0031`)⇒ 域外键字典只能从 Rust 侧构造 ⇒ 本文件在**值层**钉它。
//!
//! ⚠ **值比较不计入本计数** —— 本计数只数**键**比较(与 `wlwl-value` 里
//!   `equality_key_comparisons` 的定义一致)。

use std::cell::RefCell;
use std::rc::Rc;

use wlwl_value::{
    reset_value_work_counters, value_work_counters, values_equal, CellDict, ClassEntry, DictInner,
    ProtocolCursor, ThisToken, Value,
};

/// 规范 §18.3 的 O(n) 上界里的**小常数**(与 `complexity_cost_contract.rs` 的
/// ⑥ 守卫**同一个** c —— 判据不许放宽)。
const C: u64 = 4;

fn s(v: &str) -> Value {
    Value::String(v.to_string())
}

fn i(v: i64) -> Value {
    Value::Integer(v)
}

/// 造一个 n 条目的字典(键 `"k0".."k{n-1}"`,值 = 序号)—— 键全在封闭域内。
fn dict_n(n: usize) -> Value {
    Value::dict((0..n).map(|k| (s(&format!("k{k}")), i(k as i64))).collect())
}

/// 比一次并取**键比较次数**的读数(先清零 ⇒ 读数只含这一次比较)。
fn equal_and_work(x: &Value, y: &Value) -> (bool, u64) {
    reset_value_work_counters();
    let eq = values_equal(x, y);
    (eq, value_work_counters().equality_key_comparisons)
}

/// 造一个实例(只用到它的**身份** —— §2.4 的对象身份)。
fn instance() -> Value {
    Value::Instance {
        class: Rc::new(RefCell::new(ClassEntry {
            name: Some("K".into()),
            parent: None,
            members: Vec::new(),
            init: None,
            protocol: None,
        })),
        fields: Rc::new(RefCell::new(Vec::new())),
        this_token: Rc::new(RefCell::new(ThisToken { moved: false })),
        protocol_state: Rc::new(RefCell::new(ProtocolCursor::unrestricted())),
    }
}

/// ① **n = 16**:深度相等 ⇒ TRUE,键比较次数 **≤ c·n**(不再是 n(n+1)/2 = 136)。
#[test]
fn equal_dicts_of_16_entries_stay_within_the_linear_bound() {
    const N: usize = 16;
    let (eq, work) = equal_and_work(&dict_n(N), &dict_n(N));

    assert!(eq, "两个内容相同的 {N} 条目字典必须判相等");
    assert!(
        work <= C * N as u64,
        "n = {N} 的字典深度相等必须落在 O(n) 上界内(键比较 {work} 次,上界 {})",
        C * N as u64
    );
    // 哈希路径的**精确**读数:每个条目 1 次探测 + 1 次候选核对 = 2n。
    assert_eq!(
        work,
        2 * N as u64,
        "哈希路径下 n = {N} 的键比较次数必须恰好是 2n(实际 {work})"
    );
    // 下界断言(§18.5.2 的纪律):计量点真被走到。
    assert!(work > 0, "字典相等路径必须被计数");
}

/// ② **n = 64**:同一组判据的第二档 —— 读数必须**随 n 线性**增长。
#[test]
fn equal_dicts_of_64_entries_stay_within_the_linear_bound() {
    const N: usize = 64;
    let (eq, work) = equal_and_work(&dict_n(N), &dict_n(N));

    assert!(eq, "两个内容相同的 {N} 条目字典必须判相等");
    assert!(
        work <= C * N as u64,
        "n = {N} 的字典深度相等必须落在 O(n) 上界内(键比较 {work} 次,上界 {})",
        C * N as u64
    );
    assert_eq!(
        work,
        2 * N as u64,
        "哈希路径下 n = {N} 的键比较次数必须恰好是 2n(实际 {work})"
    );
    // 两档之比 = 规模之比(= 4)⇒ **线性**,不是二次(= 16)。
    let (_, small) = equal_and_work(&dict_n(16), &dict_n(16));
    assert_eq!(
        work,
        small * 4,
        "键比较次数必须随 n 线性增长(16 条目 {small} 次,64 条目 {work} 次)"
    );
}

/// ③ **红线:相等不看顺序**(§2.4 的集合口径)。
///
/// ⚠ 任何「同序逐位比较」的简化都会让本条变红 —— 那是**语义破坏**。
#[test]
fn equality_ignores_insertion_order() {
    let x = Value::dict(vec![(s("a"), i(1)), (s("b"), i(2))]);
    let y = Value::dict(vec![(s("b"), i(2)), (s("a"), i(1))]);

    let (eq, work) = equal_and_work(&x, &y);
    assert!(eq, "同内容、不同插入序的字典必须判相等(集合口径,不看顺序)");
    assert_eq!(
        work, 4,
        "两个 2 条目字典的键比较次数必须是 2×2 = 4(实际 {work})"
    );

    // 大一档的逆序:16 个条目、顺序完全颠倒 ⇒ 仍判相等,读数仍是 2n。
    const N: usize = 16;
    let x = Value::dict((0..N).map(|k| (s(&format!("k{k}")), i(k as i64))).collect());
    let y = Value::dict(
        (0..N)
            .rev()
            .map(|k| (s(&format!("k{k}")), i(k as i64)))
            .collect(),
    );
    let (eq, work) = equal_and_work(&x, &y);
    assert!(eq, "逆序的 {N} 条目字典必须判相等");
    assert_eq!(
        work,
        2 * N as u64,
        "逆序不改变读数:仍是 2n = {} 次(实际 {work})",
        2 * N as u64
    );
}

/// ④ **值不等 ⇒ FALSE**(且读数只花在真正比过的那几条上)。
#[test]
fn unequal_values_are_not_equal() {
    let x = Value::dict(vec![(s("a"), i(1)), (s("b"), i(2))]);
    let y = Value::dict(vec![(s("a"), i(9)), (s("b"), i(2))]);

    let (eq, work) = equal_and_work(&x, &y);
    assert!(!eq, "值不同的字典必须判不等");
    // 第一个条目就比出不等 ⇒ 1 次探测 + 1 次核对 ⇒ 2 次,然后短路。
    assert_eq!(work, 2, "首个条目比出不等就必须短路(键比较 {work} 次)");
    assert!(
        work <= C * 2,
        "不相等时的读数同样要落在 O(n) 上界内(实际 {work})"
    );
}

/// ⑤ **长度不等 ⇒ FALSE**,且**一次键比较都不做**(长度先判)。
#[test]
fn unequal_lengths_are_not_equal() {
    let x = Value::dict(vec![(s("a"), i(1))]);
    let y = Value::dict(vec![(s("a"), i(1)), (s("b"), i(2))]);

    let (eq, work) = equal_and_work(&x, &y);
    assert!(!eq, "长度不同的字典必须判不等");
    assert_eq!(work, 0, "长度不等时不得产生任何键比较(实际 {work} 次)");
}

/// ⑥ **域外键 ⇒ 线性回退**,判定结果与收敛前**逐字一致**。
///
/// ⚠ 域外键的键相等可能是**跨类型数值相等**(§2.4:`FLOAT(1.0)` ≡ `INTEGER(1)`)
///   ⇒ 哈希索引**不覆盖**它 ⇒ 这一路必须线性扫(不许悄悄去掉回退)。
#[test]
fn out_of_domain_keys_keep_the_legacy_linear_verdict() {
    // ① 两侧都含域外键(浮点 / 布尔)⇒ 走线性回退 ⇒ 判定与旧实现一致。
    let x = Value::dict(vec![
        (Value::Float(1.0), s("v")),
        (Value::Boolean(true), s("w")),
    ]);
    let y = Value::dict(vec![
        (Value::Float(1.0), s("v")),
        (Value::Boolean(true), s("w")),
    ]);
    let (eq, work) = equal_and_work(&x, &y);
    assert!(eq, "含域外键的两个相同字典必须判相等");
    // 线性回退的读数 = 实际扫描过的条目数 = 1 + 2 = 3(旧实现原样)。
    assert_eq!(
        work, 3,
        "域外键字典必须走线性回退,读数 = 实际扫描过的条目数(实际 {work})"
    );

    // ② **跨类型数值相等**:存储侧 `FLOAT(1.0)` 与探测侧 `INTEGER(1)` 在 §2.4 下
    //    **相等** ⇒ 只查索引会**漏掉**这一项 ⇒ 回退必须保住这个 TRUE。
    let x = Value::dict(vec![(Value::Float(1.0), s("v"))]);
    let y = Value::dict(vec![(i(1), s("v"))]);
    let (eq, work) = equal_and_work(&x, &y);
    assert!(
        eq,
        "FLOAT(1.0) 的键与 INTEGER(1) 的键在 §2.4 下相等 ⇒ 必须判相等"
    );
    assert_eq!(
        work, 1,
        "这一条必须由线性回退给出(读数 = 实际扫描过的条目数,实际 {work})"
    );
}

/// ⑦ **`INSTANCE` 键按身份比**(§2.4)—— 同实例命中,异实例不命中。
#[test]
fn instance_keys_are_matched_by_identity() {
    let inst = instance();
    let other = instance();

    let x = Value::dict(vec![(inst.clone(), i(1))]);
    let y = Value::dict(vec![(inst.clone(), i(1))]);
    let (eq, work) = equal_and_work(&x, &y);
    assert!(eq, "同一个实例作键 ⇒ 必须判相等");
    assert_eq!(
        work, 2,
        "同实例的键比较次数必须是 1 次探测 + 1 次核对(实际 {work})"
    );

    // 另一个实例(字段完全相同)⇒ 身份不同 ⇒ 不等。
    let y = Value::dict(vec![(other, i(1))]);
    let (eq, work) = equal_and_work(&x, &y);
    assert!(!eq, "不同实例作键 ⇒ 必须判不等(§2.4 的对象身份)");
    assert_eq!(
        work, 1,
        "异实例时哈希探测一次就落空 ⇒ 1 次键比较(实际 {work})"
    );
}

/// ⑧ **重复键**形态:集合口径必须仍然成立(哈希定位只给候选,**不消费**候选)。
///
/// ⚠ 字典字面量允许重复键(`["a": 1, "a": 2]`)⇒ 同一键在 `y` 里可能有多个
///   位置 ⇒ 候选是**位置列表**;且**同一个** `y` 条目可以匹配**多个** `x` 条目。
#[test]
fn duplicate_keys_keep_the_set_semantics() {
    // ① 同键两个不同值,顺序颠倒 ⇒ 相等(两个 `x` 条目各自在 `y` 里找到匹配)。
    let x = Value::dict(vec![(s("a"), i(1)), (s("a"), i(2))]);
    let y = Value::dict(vec![(s("a"), i(2)), (s("a"), i(1))]);
    let (eq, work) = equal_and_work(&x, &y);
    assert!(eq, "重复键的同内容字典必须判相等");
    // 1 次探测 + 2 次候选核对(第一个值落在第二个候选上)+ 1 次探测 + 1 次核对 = 5。
    assert_eq!(
        work, 5,
        "重复键的键比较次数 = 探测 2 次 + 候选核对 3 次 = 5(实际 {work})"
    );

    // ② `x` 的两个条目**共用** `y` 的同一条目(候选不消费)⇒ 旧口径判**相等**。
    let x = Value::dict(vec![(s("a"), i(1)), (s("a"), i(1))]);
    let y = Value::dict(vec![(s("a"), i(1)), (s("b"), i(2))]);
    let (eq, work) = equal_and_work(&x, &y);
    assert!(
        eq,
        "x 的每个条目都在 y 里找到匹配 ⇒ 集合口径判相等(候选可被多个 x 条目共用)"
    );
    assert_eq!(
        work, 4,
        "两个 x 条目各 1 次探测 + 1 次核对 = 4(实际 {work})"
    );

    // ③ `y` 里没有 `x` 的某个键 ⇒ 不等(哈希探测直接落空)。
    let x = Value::dict(vec![(s("a"), i(1)), (s("b"), i(2))]);
    let y = Value::dict(vec![(s("a"), i(1)), (s("a"), i(2))]);
    let (eq, work) = equal_and_work(&x, &y);
    assert!(!eq, "y 缺少 x 的键 ⇒ 必须判不等");
    assert_eq!(
        work, 3,
        "1 次探测 + 1 次核对(命中)+ 1 次探测(落空) = 3(实际 {work})"
    );
}

/// ⑨ **嵌套字典**:每一层各自计数,总读数是各层之和(仍是 O(n))。
#[test]
fn nested_dicts_count_every_level() {
    let x = Value::dict(vec![(s("a"), Value::dict(vec![(s("k"), i(1))]))]);
    let y = Value::dict(vec![(s("a"), Value::dict(vec![(s("k"), i(1))]))]);

    let (eq, work) = equal_and_work(&x, &y);
    assert!(eq, "嵌套字典必须判相等");
    // 外层 1 次探测 + 1 次核对;内层 1 次探测 + 1 次核对。
    assert_eq!(work, 4, "两层字典的键比较次数必须是 2 + 2 = 4(实际 {work})");
}

/// ⑩ **累加器形态**(`DictCell`)与普通字典**同口径**(ADR-0035 D4)。
#[test]
fn a_dict_cell_compares_equal_to_a_plain_dict() {
    let entries = vec![(s("a"), i(1)), (s("b"), i(2))];
    let plain = Value::dict(entries.clone());
    let cell = Value::DictCell(CellDict::new(Rc::new(DictInner::new(entries))));

    let (eq, work) = equal_and_work(&cell, &plain);
    assert!(eq, "累加器形态的字典必须与普通字典逐字同结果");
    assert_eq!(
        work, 4,
        "累加器形态的键比较次数必须与普通字典一致(实际 {work})"
    );
}

/// ⑪ **本计数只数键比较**:值是长字符串也不额外计数。
#[test]
fn the_counter_only_counts_key_comparisons() {
    let long = "x".repeat(256);
    let x = Value::dict(vec![(s("a"), Value::String(long.clone()))]);
    let y = Value::dict(vec![(s("a"), Value::String(long))]);

    let (eq, work) = equal_and_work(&x, &y);
    assert!(eq, "值是长字符串时也必须判相等");
    assert_eq!(
        work, 2,
        "值比较不计入本计数 ⇒ 只有 1 次探测 + 1 次核对(实际 {work})"
    );
}
