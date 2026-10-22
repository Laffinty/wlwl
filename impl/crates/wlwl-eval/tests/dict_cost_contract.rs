//! [ADR-0035 S5] 字典的**确定性工作量计数守卫**。
//!
//! 规范 §18.3 给字典的上界是**规范性**的(`d[k]` O(1) 期望、`HAS` O(1) 期望、
//! `INDEX_SET` 两条),§18.5 要求判定**必须**用**确定性工作量计数** —— 不用墙钟
//! 比值(同份线性实现在不同机器上可差 3 倍;比值里混着缓存命中率与调度噪声)。
//! 本文件把那两条落地成**可复算的读数**。
//!
//! ## 测法
//!
//! 计数点收在**字典内部**(`wlwl_value::DictWorkCounters`):键比较次数、copy-on-write
//! 的拷贝条目数、两条查找路径各多少次、索引重建次数。计数是**线程局部**的 ⇒ 每个
//! 用例只看见自己那次运行的工作量(`cargo test` 的用例是并行的)。
//!
//! **读路径的量法**用**两程序作差**:字典字面量的 n 次插入各记 1 次比较 ⇒
//! 「构造 + 一次查找」减去「只构造」= **那一次查找的比较次数**。⇒ 于是
//! n = 64 与 n = 256 两档的差值必须**相等**(与规模无关 ⇒ O(1) 期望),
//! 而线性扫描时它们是 64 与 256 ⇒ 守卫变红。
//!
//! ## 四条断言,缺一不可
//!
//! ① **读路径**:比较次数与规模无关(两档对比);
//! ② **下界断言**:比较次数必须 **> 0** —— 否则「计量点从没被走到」也会绿
//!    (§18.3.1 的要求:计量点必须收在被测数据结构**内部**,并带**下界断言**
//!    确认计量点确实被走到);
//! ③ **写路径 · 唯一拥有** ⇒ 原地改(拷贝 0 条);
//! ④ **写路径 · 累加形态** ⇒ **原地累积**(拷贝 0 条,且**不随 n 增长**)——
//!    `ADR-0035` **D4** 落地后,`SET(d, INDEX_SET(d, k, v))` 被识别成**字典
//!    累加器形态** ⇒ 载荷只被累加器 cell 持有 ⇒ 写时复制判定为「唯一拥有」
//!    ⇒ 原地改 ⇒ 规范 §18.3 的 `INDEX_SET` 写上界(新键 O(1) 摊还)**达标**。
//!    ⚠ **别名形态**(两个绑定持有同一个字典)的退化**仍然存在**,由下一条用例
//!    如实钉住 —— 累加器形态只覆盖「同一个绑定被反复写」这一种形态。

mod common;

use std::path::PathBuf;
use wlwl_eval::Evaluator;
use wlwl_value::{dict_work_counters, reset_dict_work_counters};

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_dictcost")
}

/// 跑一段源码,返回它在**字典内部**花掉的工作量。
fn work_of(src: &str) -> wlwl_value::DictWorkCounters {
    let ast = wlwl_parser::parse(src, "t.wll").expect("src must parse");
    reset_dict_work_counters();
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    ev.eval(&ast).expect("program must run");
    dict_work_counters()
}

/// 造一个 n 键的字典字面量(键全在封闭域内 ⇒ 走哈希索引那条路)。
fn dict_literal(n: usize) -> String {
    let mut s = String::from("[");
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("\"k{i}\": {i}"));
    }
    s.push(']');
    s
}

/// ① 读路径:一次键查找的比较次数**与字典规模无关**。
///
/// 两档规模(64 / 256)的「查找比较次数」必须**相等**且**很小**(≤ 2)⇒
/// 线性扫描时是 64 与 256 ⇒ 守卫红。
#[test]
fn dict_lookup_cost_is_independent_of_the_dict_size() {
    const SMALL: usize = 64;
    const LARGE: usize = 256;

    let small = work_of(&format!(
        "LET(d, {}); PRINT(d[\"k7\"]);",
        dict_literal(SMALL)
    ));
    let small_build = work_of(&format!("LET(d, {});", dict_literal(SMALL)));

    let large = work_of(&format!(
        "LET(d, {}); PRINT(d[\"k7\"]);",
        dict_literal(LARGE)
    ));
    let large_build = work_of(&format!("LET(d, {});", dict_literal(LARGE)));

    // ② 下界断言:计数点真被走到(否则「差值恒为 0」也会绿)。
    assert!(
        small.comparisons > small_build.comparisons,
        "the lookup must be counted at all (small: {} vs build {})",
        small.comparisons,
        small_build.comparisons
    );

    let small_lookup = small.comparisons - small_build.comparisons;
    let large_lookup = large.comparisons - large_build.comparisons;

    assert!(
        small_lookup <= 2,
        "a lookup on a {SMALL}-key dict must not scan the dict, got {small_lookup} comparisons"
    );
    assert_eq!(
        small_lookup, large_lookup,
        "lookup cost must not depend on the dict size (small={small_lookup}, large={large_lookup})"
    );
    assert!(
        large.indexed_lookups > large_build.indexed_lookups,
        "the lookup must go through the hash index"
    );
    assert_eq!(
        large.linear_lookups, large_build.linear_lookups,
        "a dict whose keys are all in the closed domain must never fall back to a linear scan"
    );
}

/// ③ 写路径,形态一:**唯一拥有** ⇒ 原地改 ⇒ 拷贝 0 条。
///
/// `INDEX_SET` 的实参是**临时值**(字面量)⇒ 强引用计数为 1 ⇒ copy-on-write
/// 走原地分支 ⇒ 没有 O(n) 拷贝。
#[test]
fn writing_to_a_uniquely_owned_dict_copies_nothing() {
    let w = work_of("PRINT(INDEX_SET([\"z\": 1], \"w\", 2));");
    assert_eq!(
        w.copied_entries, 0,
        "a uniquely owned dict must be mutated in place (copied {} entries)",
        w.copied_entries
    );
    assert!(
        w.comparisons > 0,
        "the write must be counted at all (comparisons = {})",
        w.comparisons
    );
}

/// ④ 写路径,形态二:**被别名**(两个绑定持有同一个字典)⇒ 退化为拷贝。
///
/// `LET(e, d)` 让两个绑定持有同一个 `Rc` ⇒ 写之前必须克隆整个字典 ⇒ 拷贝条目数
/// = 当时的条目数。
#[test]
fn writing_to_an_aliased_dict_copies_the_whole_dict() {
    let w = work_of("LET(d, [\"z\": 1]); LET(e, d); PRINT(INDEX_SET(d, \"w\", 2));");
    assert_eq!(
        w.copied_entries, 1,
        "an aliased dict must be cloned before mutation (copied {} entries, expected 1)",
        w.copied_entries
    );
}

/// ④ 写路径,形态三(**最常见的写法**):循环里逐次 `SET(d, INDEX_SET(d, k, v))`。
///
/// ✅ **这条钉住的是「达标」**(`ADR-0035` D4 落地后改写):循环体内对同一个
/// 绑定反复 `INDEX_SET` ⇒ 被识别成**字典累加器形态**(`ADR-0032` 的
/// `Value::Cell(CellVec)` 的字典版)⇒ 载荷只被累加器 cell 持有 ⇒ 写时复制
/// 判定为「唯一拥有」⇒ **原地改** ⇒ 累加 n 个键的**总拷贝是 0**。
///
/// ⚠ **判定用确定性工作量计数**(规范 §18.5:不用墙钟比值)⇒ 三条断言:
///
/// ① **总拷贝为 0** —— 原地累积,一次拷贝都没有;
/// ② **不随 n 增长** —— 两档 n(16 / 64)的拷贝读数**相同**(与规模无关);
/// ③ **序列级上界**(规范 §18.3.1 第 2 条:「n 次写 ⇒ 总工作量 ≤ c·n」)
///    —— 比较次数(键比较 + 索引插入)对 n **线性**。
///
/// ③ 的常数 `c` **不是拍的**:每次写恰好 2 次键比较(1 次哈希查找 + 1 次索引
/// 插入),循环条件里的 `LEN(d)` 不做键比较 ⇒ 读数是 `2·(n-1)`;这里取
/// `c = 4`(两倍余量)—— 确定性计数不抖,余量留给将来内建实现的细枝末节。
///
/// **实测读数**(2026-10-22,本机):n = 16 ⇒ 比较 30 / 拷贝 **0** / 索引查找 15;
/// n = 64 ⇒ 比较 126 / 拷贝 **0** / 索引查找 63。⇒ 工作量**线性**(每次写 2 次
/// 比较),拷贝**恒为 0**。
#[test]
fn accumulating_keys_in_a_loop_copies_nothing() {
    const SMALL: usize = 16;
    const LARGE: usize = 64;
    // 键用 **INTEGER**(`LEN(d)`)—— 它在封闭域内(规范 §2.1)⇒ 走哈希索引那条路。
    let src = |n: usize| {
        format!(
            "LET MUT(d, [0: 0]);\
             WHILE(<(LEN(d), {n}), SET(d, INDEX_SET(d, LEN(d), LEN(d))));\
             PRINT(LEN(d));"
        )
    };
    let small = work_of(&src(SMALL));
    let large = work_of(&src(LARGE));

    // ① 总拷贝为 0(原地累积)。
    assert_eq!(
        small.copied_entries, 0,
        "the accumulator form must accumulate in place (copied {} entries)",
        small.copied_entries
    );
    // ② 不随 n 增长:两档读数相同。
    assert_eq!(
        small.copied_entries, large.copied_entries,
        "the total copy count must not depend on n (small={}, large={})",
        small.copied_entries, large.copied_entries
    );
    // ③ 序列级上界:总工作量 ≤ c·n。
    let c = 4u64;
    for (n, w) in [(SMALL as u64, small), (LARGE as u64, large)] {
        assert!(
            w.comparisons <= c * n,
            "n writes must cost at most c·n comparisons (n={n}, comparisons={}, c={c})",
            w.comparisons
        );
    }
    // 下界断言:计量点真被走到(否则「恒为 0」也会绿)。
    assert!(
        small.comparisons > 0,
        "the write path must be counted at all (comparisons = {})",
        small.comparisons
    );
    // 且工作量确实**随 n 增长**(线性,不是常数)—— 说明计数点走的是真路径。
    assert!(
        large.comparisons > small.comparisons,
        "the counted work must grow with n (small={}, large={})",
        small.comparisons,
        large.comparisons
    );
}
