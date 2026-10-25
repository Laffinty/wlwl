//! **X-14** 差异登记的**最小可复现 + 可复算读数**。
//!
//! 规范基线：`Laffinty/wlwl-spec-v1` 一致性册 §5.4 的 **X-14** 行
//! （语言核心册 §18.3.1 第 2 条：被别名的写序列**必须**达成序列级摊还）。
//!
//! ## 这条差异说的是哪一种形态
//!
//! §5.4 的 X-14 行点名的形态是「**别名在写序列内部每轮重建**」：
//!
//! ```wlwl
//! LET MUT(d, [0: 0]);
//! WHILE(<(LEN(d), n), LET MUT(e, d); SET(d, INDEX_SET(d, LEN(d), LEN(d))));
//! ```
//!
//! 每一轮都新建一份别名 `e` ⇒ 旧值在**当轮内**仍可观察 ⇒（§19.1 的值语义）
//! 当轮那次写**必须**产出新值 ⇒ 每轮都拷贝**整份字典** ⇒ 总拷贝是**二次的**。
//!
//! ⚠ **与 X-12 的别名形态不是同一件事**：X-12 的别名在写序列**之前**建立
//! （`LET(e, d)` 然后写 n 次）⇒ 只在**首次**写拷贝一次 ⇒ 之后原地写 ⇒ 达标。
//! X-14 的别名**每轮重建** ⇒ 退化。
//!
//! ## 为什么读数必须来自 Rust 侧
//!
//! 这条差异的判据是**成本**（§18.5 要求用**确定性工作量计数**，不用墙钟比值）
//! ⇒ **WLWL 源码里没有任何可观测读数** —— 程序的语义完全正确，只有工作量超标。
//! ⇒ 读数只能来自字典内部的计数器（`wlwl_value::DictWorkCounters`）。
//!
//! ## 本文件有两个用例，角色不同
//!
//! | 用例 | 状态 | 角色 |
//! |---|---|---|
//! | `x14_the_copy_path_is_measured_at_all` | 绿 | **计量点被走到**的下界断言（§18.5 的纪律）＋ 把当前读数打出来 |
//! | `x14_the_write_sequence_must_reach_the_sequence_level_bound` | `#[ignore]`（**红**） | **目标测试**：断言序列级摊还成立 ⇒ 现在**红** ⇒ 缺口仍在 |
//!
//! ⚠ **目标测试 `#[ignore]` 的理由**（与上游 `complexity_cost_contract` 里
//! 「规范性上界」那条同款）：它断言的是**要求应当成立**，而要求**现在不成立**
//! ⇒ 放进常规 `cargo test` 会让 CI 恒红。⇒ 用 `#[ignore]` 挂起，
//! 由 `script/check_conformance_registry.py`（§5.4 状态列门禁）**显式**跑它。
//!
//! ⇒ 上游一旦收敛（惰性 CoW / 结构共享），这条目标测试**转绿** ⇒
//! 门禁会判「缺口已闭合但状态没翻」⇒ 该行转「已实现」＋ 去掉 `#[ignore]`。

mod common;

use std::path::PathBuf;
use wlwl_eval::Evaluator;
use wlwl_value::{dict_work_counters, reset_dict_work_counters};

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_x14")
}

/// 跑一段源码，返回它在**字典内部**花掉的工作量（确定性计数）。
fn work_of(src: &str) -> wlwl_value::DictWorkCounters {
    let ast = wlwl_parser::parse(src, "t.wll").expect("src must parse");
    reset_dict_work_counters();
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    ev.eval(&ast).expect("program must run");
    dict_work_counters()
}

/// X-14 的最小复现（别名**每轮重建**）：n 轮写，每轮新建一份别名。
fn x14_src(n: usize) -> String {
    format!(
        "LET MUT(d, [0: 0]);\n\
         WHILE(<(LEN(d), {n}), LET MUT(e, d); SET(d, INDEX_SET(d, LEN(d), LEN(d))));\n\
         0"
    )
}

/// **计量点确实被走到**（§18.5 的下界纪律）＋ **把当前读数打出来**。
///
/// 当前读数（`wlwl 0.11.4` 基线，2026-10-25 实测）：
///
/// | n | `copied_entries` | `comparisons` |
/// |---|---|---|
/// | 16 | 120 | 30 |
/// | 64 | 2016 | 126 |
///
/// ⇒ 总拷贝是**二次的**（120 ≈ Σ1..15，2016 ≈ Σ1..63）。
#[test]
fn x14_the_copy_path_is_measured_at_all() {
    const SMALL: usize = 16;
    const LARGE: usize = 64;

    let small = work_of(&x14_src(SMALL));
    let large = work_of(&x14_src(LARGE));

    println!(
        "X-14 n={}: copied_entries={} comparisons={} indexed_lookups={} linear_lookups={}",
        SMALL, small.copied_entries, small.comparisons, small.indexed_lookups, small.linear_lookups
    );
    println!(
        "X-14 n={}: copied_entries={} comparisons={} indexed_lookups={} linear_lookups={}",
        LARGE, large.copied_entries, large.comparisons, large.indexed_lookups, large.linear_lookups
    );

    // ① 下界断言（§18.5）：计量点真被走到 —— 否则「没被计量」也会绿。
    assert!(
        small.copied_entries > 0,
        "the copy-on-write path must be counted at all (copied {} entries)",
        small.copied_entries
    );
    assert!(
        small.comparisons > 0,
        "the write path must be counted at all (comparisons = {})",
        small.comparisons
    );
}

/// **目标测试**（§18.3.1 第 2 条）：n 次写 ⇒ 总工作量 ≤ c·n。
///
/// ⚠ **现在应当是红的** —— 别名每轮重建 ⇒ 每轮写必须产出新值（§19.1）⇒
/// 每轮拷贝整份字典 ⇒ 总拷贝二次 ⇒ 序列级判据**不成立**。
///
/// 收敛路线（§5.4 该行已写）：**惰性 CoW / 结构共享** ⇒ 别名不再每轮拿到
/// 一份快照 ⇒ 写只拷当轮新增的那一条。
///
/// **上游收敛后**：去掉 `#[ignore]` ⇒ 本用例转绿 ⇒ 同时把 §5.4 的 X-14 行
/// 翻成「已实现」（`script/check_conformance_registry.py` 会盯着这件事）。
#[test]
#[ignore = "X-14 尚未收敛：别名每轮重建 ⇒ 总拷贝二次（见一致性册 §5.4 X-14 行）"]
fn x14_the_write_sequence_must_reach_the_sequence_level_bound() {
    const SMALL: usize = 16;
    const LARGE: usize = 64;
    const C: u64 = 4; // 序列级上界「n 次写 ⇒ 总工作量 ≤ c·n」的 c（留一倍余量）

    let small = work_of(&x14_src(SMALL));
    let large = work_of(&x14_src(LARGE));

    // ① 语义（§19.1）必须仍然正确 —— 收敛**不得**改语义。
    assert!(
        small.comparisons > 0 && large.comparisons > 0,
        "the write path must be counted at all"
    );

    // ② 序列级判据：n 次写 ⇒ 总工作量 ≤ c·n。
    for (n, w) in [(SMALL as u64, small), (LARGE as u64, large)] {
        assert!(
            w.comparisons <= C * n,
            "n writes must cost at most c·n comparisons (n={n}, comparisons={}, c={C})",
            w.comparisons
        );
    }

    // ③ 总拷贝**不随 n 增长**（每轮只拷当轮新增的那一条）。
    assert_eq!(
        small.copied_entries, large.copied_entries,
        "the total copy count must not depend on n (small={}, large={})",
        small.copied_entries, large.copied_entries
    );
}
