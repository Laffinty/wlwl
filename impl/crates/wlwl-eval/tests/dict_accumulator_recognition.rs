//! [ADR-0035 D4] **字典**累加器形态的契约(三件套的第二件:契约测试)。
//!
//! 目标形状(规范 §18.4 的「循环内逐次 `INDEX_SET(d, k, v)` 写 n 个键」):
//!
//! ```wlwl
//! LET MUT(d, [0: 0]);
//! WHILE(<(LEN(d), n), SET(d, INDEX_SET(d, LEN(d), LEN(d))));
//! ```
//!
//! ## 本文件钉的四件事
//!
//! ① **命中形状的结果逐字节正确**,且计数器显示 hit;
//! ② **四条否定条件各一个用例** ⇒ 判否(`ADR-0032` S2 的同款结构:
//!    ① 被传给任何函数 ② 被赋给另一个变量 / 存进容器 ③ 被闭包捕获
//!    ④ 循环外还有对它的读);
//! ③ **识别与否,输出必须逐字节相同** —— 这是 `ADR-0032` D3 三件套第一条,
//!    **比性能更要紧**(性能可以退化,语义不许退化);
//! ④ **可观察面零变化**:`TYPE` 报 `"DICT"`、渲染逐字节相同、`==` 与普通
//!    字典逐字同结果。
//!
//! ⚠ 判据哲学:**宁可 miss 多,不可误判**。miss 的后果只是性能没提升;
//!   误判的后果是**可观察的语义破坏**(原地改被不该看到的人看到)。

mod common;

use wlwl_eval::Evaluator;

/// 跑程序,返回 `(末条语句的值, 字典累加器计数器)`。
fn run(src: &str) -> (String, (u64, u64, u64)) {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new().with_base_dir(common::scratch("wlwl_dictacc"));
    let v = ev.eval(&ast).expect("case must not fail");
    (v.display(), ev.dict_accumulator_optimizer_counters())
}

/// 命中形状的循环体(用 `LEN(d)` 当循环边界 —— 最常见的字典累加写法)。
fn acc(body: &str) -> String {
    format!("LET MUT(d, [0: 0]);\nWHILE(<(LEN(d), 4), ({body}));\nd\n")
}

// ── ① 命中:形状正确 + 计数器 hit ──────────────────────────────────────

#[test]
fn recognises_the_dict_accumulator_shape() {
    let (val, (hit, miss, fallback)) = run(&acc("SET(d, INDEX_SET(d, LEN(d), LEN(d)))"));
    assert_eq!(hit, 1, "expected 1 hit, got hit={hit} miss={miss}");
    assert_eq!(miss, 0, "expected 0 miss, got hit={hit} miss={miss}");
    assert_eq!(
        fallback, 0,
        "the runtime re-check must agree with the recogniser (fallback={fallback})"
    );
    // 结果逐字节正确:初值 1 个条目,循环到 4 个 ⇒ 键是 0(初值)/ 1 / 2 / 3(插入序)。
    assert_eq!(val, "[0: 0, 1: 1, 2: 2, 3: 3]", "accumulator result");
}

/// 键名不是 `LEN(d)` 的写法也认(判据看的是**形状**,不是键的名字)。
#[test]
fn recognises_the_shape_with_a_plain_integer_key() {
    let (val, (hit, _, _)) = run("LET MUT(d, [0: 0]);\nLET MUT(i, 1);\n\
         WHILE(<(i, 4), (SET(d, INDEX_SET(d, i, i)); SET(i, +(i, 1))));\nd\n");
    assert_eq!(hit, 1, "the accumulator shape must be recognised");
    assert_eq!(val, "[0: 0, 1: 1, 2: 2, 3: 3]", "accumulator result");
}

// ── ② 判否:四条否定条件各一个用例 ───────────────────────────────────────

#[test]
fn rejects_an_accumulator_that_escapes_the_loop() {
    // ① 被传给任何函数(会逃逸)
    let (_, (hit, _, _)) = run(&acc("PRINT(d); SET(d, INDEX_SET(d, LEN(d), LEN(d)))"));
    assert_eq!(
        hit, 0,
        "an accumulator passed to a call must not be recognised"
    );
    // ② 被赋给另一个变量(会被别名)
    let (_, (hit, _, _)) = run("LET MUT(d, [0: 0]);\nLET MUT(e, NULL);\n\
         WHILE(<(LEN(d), 4), (SET(e, d); SET(d, INDEX_SET(d, LEN(d), LEN(d)))));\nd\n");
    assert_eq!(hit, 0, "an aliased accumulator must not be recognised");
    // ③ 被存进容器(会逃逸)
    let (_, (hit, _, _)) = run("LET MUT(d, [0: 0]);\nLET MUT(e, NULL);\n\
         WHILE(<(LEN(d), 4), (SET(e, [\"k\": d]); SET(d, INDEX_SET(d, LEN(d), LEN(d)))));\nd\n");
    assert_eq!(
        hit, 0,
        "an accumulator stored into a container must not be recognised"
    );
    // ④ 被闭包捕获(生命周期跨出循环)
    let (_, (hit, _, _)) = run(&acc(
        "SET(d, INDEX_SET(d, LEN(d), LEN(d))); LET(g, FUN(() , d));",
    ));
    assert_eq!(
        hit, 0,
        "a closure-captured accumulator must not be recognised"
    );
    // ⑤ 循环体里还有一处**非白名单**的读(不是只读内建的直接实参)
    //    ⇒ 保守判否(`STR` 不在只读白名单里 —— 少认几个只是慢,不会错)。
    let (_, (hit, _, _)) = run(&acc("SET(d, INDEX_SET(d, LEN(d), LEN(d))); STR(d)"));
    assert_eq!(
        hit, 0,
        "an accumulator read outside the whitelisted read positions must not be recognised"
    );
}

/// 没有字典累加器的循环 ⇒ 判否(但仍计一次 miss)。
#[test]
fn rejects_a_loop_with_no_dict_accumulator() {
    let (_, (hit, miss, _)) = run("LET MUT(d, [0: 0]);\nLET MUT(i, 0);\n\
         WHILE(<(i, 4), (SET(i, +(i, 1))));\nd\n");
    assert_eq!(hit, 0, "a loop without the accumulator shape has no hit");
    assert_eq!(
        miss, 1,
        "a loop without the accumulator shape must count as a miss"
    );
}

/// 两个不同的字典累加器不是本判据覆盖的形状 ⇒ **判否**,不猜。
#[test]
fn rejects_two_distinct_dict_accumulators() {
    let (_, (hit, _, _)) = run("LET MUT(d, [0: 0]);\nLET MUT(e, [0: 0]);\n\
         WHILE(<(LEN(d), 4), (SET(d, INDEX_SET(d, LEN(d), LEN(d)));\
         SET(e, INDEX_SET(e, LEN(e), LEN(e)))));\nd\n");
    assert_eq!(
        hit, 0,
        "the ambiguous two-accumulator shape must be rejected"
    );
}

// ── ③ 契约第一条:识别与否,输出必须**逐字节**相同 ──────────────────────

/// 逐条对照:**同一份逻辑**写成「被识别」与「不被识别」两种形状,末值必须相同。
#[test]
fn recognition_never_changes_the_result() {
    let cases: &[(&str, &str)] = &[
        // 命中形状
        (
            "LET MUT(d, [0: 0]);\nWHILE(<(LEN(d), 4), (SET(d, INDEX_SET(d, LEN(d), LEN(d)))));\nd\n",
            "LET MUT(d, [0: 0]);\nWHILE(<(LEN(d), 4), (SET(d, INDEX_SET(d, LEN(d), LEN(d)))));\nd\n",
        ),
        // 同一份结果,但循环体里多做一次**非白名单**的读 ⇒ 判否,值不变
        (
            "LET MUT(d, [0: 0]);\nWHILE(<(LEN(d), 4), (SET(d, INDEX_SET(d, LEN(d), LEN(d)))));\nd\n",
            "LET MUT(d, [0: 0]);\nWHILE(<(LEN(d), 4), (SET(d, INDEX_SET(d, LEN(d), LEN(d))); STR(d)));\nd\n",
        ),
    ];
    for (i, (src, want_src)) in cases.iter().enumerate() {
        let (v, _) = run(src);
        let (w, _) = run(want_src);
        assert_eq!(
            v, w,
            "case {i}: recognition changed the result ({v} vs {w})"
        );
    }
}

// ── ④ 可观察面零变化 ───────────────────────────────────────────────────

/// 累加器形态下,字典在**每一个可观察面**上与普通字典逐字相同。
#[test]
fn the_accumulator_stays_observable_as_a_plain_dict() {
    // `TYPE` 报 `"DICT"`(变体名不外泄)。
    let (t, (hit, _, _)) = run(&format!(
        "{}TYPE(d)\n",
        acc("SET(d, INDEX_SET(d, LEN(d), LEN(d)))").replace(";\nd\n", ";\n")
    ));
    assert_eq!(hit, 1, "the accumulator shape must be recognised");
    assert_eq!(t, "DICT", "the accumulator must stay observable as a dict");
}

/// 累加器形态下的字典与**同样内容的普通字典**相等(两种口径都算)。
#[test]
fn the_accumulator_equals_a_plain_dict_with_the_same_content() {
    let (v, (hit, _, _)) = run(&format!(
        "{}==(d, [0: 0, 1: 1, 2: 2, 3: 3])\n",
        acc("SET(d, INDEX_SET(d, LEN(d), LEN(d)))").replace(";\nd\n", ";\n")
    ));
    assert_eq!(hit, 1, "the accumulator shape must be recognised");
    assert_eq!(v, "TRUE", "the accumulator must equal a plain dict");
}

/// 累加器形态下的字典与普通字典的 `!=` 同样成立(反向口径)。
#[test]
fn the_accumulator_differs_from_a_dict_with_other_content() {
    let (v, (hit, _, _)) = run(&format!(
        "{}!=(d, [1: 1])\n",
        acc("SET(d, INDEX_SET(d, LEN(d), LEN(d)))").replace(";\nd\n", ";\n")
    ));
    assert_eq!(hit, 1, "the accumulator shape must be recognised");
    assert_eq!(
        v, "TRUE",
        "the accumulator must differ from a dict with other content"
    );
}

// ── [ADR-0035 D5] 别名形态:**就地写不得泄漏给别名** ─────────────────────
//
// 规范 §19.1(值语义):旧值只要还有别的持有者,原地改就是**可观察的语义破坏**。
// ⇒ 字典累加器的写是**就地**进行的 ⇒ 别名 `e` 必须仍看到**循环开始前**的字典,
// 而 `d` 必须看到循环结束后的字典。
//
// ⚠ 这条是「就地写」的**语义守卫**(工作量守卫在 `dict_cost_contract.rs`):
//   计数对了但值错了 ⇒ 那是最糟的失败模式(优化悄悄改了语义)。
#[test]
fn an_aliased_accumulator_still_sees_the_dict_from_before_the_loop() {
    // 别名在循环**之前**建立 ⇒ 它必须看到循环开始前的那份字典。
    let (alias, (hit, miss, fallback)) = run("LET MUT(d, [0: 0]);\n\
         LET MUT(e, d);\n\
         WHILE(<(LEN(d), 4), (SET(d, INDEX_SET(d, LEN(d), LEN(d)))));\n\
         e\n");
    assert_eq!(hit, 1, "the accumulator shape must be recognised");
    assert_eq!(miss, 0, "expected 0 miss, got hit={hit} miss={miss}");
    assert_eq!(
        fallback, 0,
        "the runtime re-check must agree with the recogniser (fallback={fallback})"
    );
    assert_eq!(
        alias, "[0: 0]",
        "the alias must still see the dict from before the loop"
    );
}

/// 同一条,但看**被写的那一份**:循环结束后 `d` 必须累积到 4 个条目(插入序)。
#[test]
fn the_aliased_accumulator_itself_still_accumulates() {
    let (val, (hit, _, _)) = run("LET MUT(d, [0: 0]);\n\
         LET MUT(e, d);\n\
         WHILE(<(LEN(d), 4), (SET(d, INDEX_SET(d, LEN(d), LEN(d)))));\n\
         d\n");
    assert_eq!(hit, 1, "the accumulator shape must be recognised");
    assert_eq!(val, "[0: 0, 1: 1, 2: 2, 3: 3]", "accumulator result");
}

/// ── [D13-099] 回归守卫:优化**不得**改掉绑定的可变性 ─────────────────────
//
// 规范 §3.1:可变性在建绑定时定、**永不变**。⇒ 字典累加器形态的「临时表示」
// 只能**就地改写已有绑定的值**(`Env::replace_binding_value`),不得遮蔽外层
// 绑定、不得把它改成可变的。
//
// ⚠ 与数组版同款:数组累加器此前用 `set_local_mut`,插入了**一个可变的影子
// 绑定** ⇒ 不可变绑定被静默改成了可变 ⇒ 程序从「报 E0024」变成「不报错」。
// 字典版从一开始就用的 `replace_binding_value` ⇒ 本条是它的**回归守卫**。
#[test]
fn an_immutable_dict_accumulator_binding_still_reports_e0024() {
    let ast = wlwl_parser::parse(
        "LET(d, [0: 0]);\nLET MUT(i, 0);\n\
         WHILE(<(i, 3), (SET(d, INDEX_SET(d, LEN(d), LEN(d))); SET(i, +(i, 1))));\nd\n",
        "t.wll",
    )
    .expect("case source must parse");
    let mut ev = Evaluator::new().with_base_dir(common::scratch("wlwl_dictacc"));
    let err = ev
        .eval(&ast)
        .expect_err("an immutable dict accumulator binding must report E0024");
    assert_eq!(
        err.diagnostic().code,
        wlwl_error::ErrorCode::E0024,
        "got {:?}",
        err.diagnostic().code
    );
}
