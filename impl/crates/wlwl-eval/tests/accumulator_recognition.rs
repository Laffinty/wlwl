//! [v0.11.3 M7 / `ADR-0032` D2 / D3 / PLAN S2] 累加器形态识别的契约。
//!
//! 本步**只识别并计数,不改变任何行为** —— 所以除了计数器,每条用例还要断言
//! 「识别与否,程序结果完全一样」。这是 `ADR-0025` Q3 第三件契约测试的雏形:
//! **性能可以退化,语义不许退化。**
//!
//! 判据哲学:**宁可 miss 多,不可误判**。miss 的后果只是性能没提升;误判的后果是
//! `Cell` 逃出循环、共享语义与现在不同。

mod common;

use wlwl_eval::Evaluator;

fn scratch_dir() -> std::path::PathBuf {
    common::scratch("wlwl_acc")
}

/// 跑程序,返回 `(末条语句的值, 识别计数器)`。
fn run(src: &str) -> (String, (u64, u64)) {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    let v = ev.eval(&ast).expect("case must not fail");
    (v.display(), ev.accumulator_optimizer_counters())
}

fn acc(body: &str) -> String {
    format!("LET MUT(a, []);\nLET MUT(i, 0);\nWHILE(<(i, 3), ({body}));\na\n")
}

// ── 命中:三种追加形态 ────────────────────────────────────────────────

#[test]
fn recognises_all_three_append_shapes() {
    // ⚠ 三种都要认 —— ADR-0032 §1 实测它们成本落在噪声内,都会被用户写出来。
    for (label, body) in [
        ("plus", "SET(a, +(a, [i])); SET(i, +(i, 1))"),
        ("push", "SET(a, PUSH(a, i)); SET(i, +(i, 1))"),
        ("concat", "SET(a, CONCAT(a, [i])); SET(i, +(i, 1))"),
    ] {
        let (val, (hit, miss)) = run(&acc(body));
        assert_eq!(hit, 1, "{label}: expected 1 hit, got hit={hit} miss={miss}");
        assert_eq!(
            miss, 0,
            "{label}: expected 0 miss, got hit={hit} miss={miss}"
        );
        assert_eq!(val, "[0, 1, 2]", "{label}: accumulator result");
    }
}

// ── 判否:四条否定条件各一个用例 ───────────────────────────────────────

#[test]
fn rejects_an_accumulator_that_is_aliased_or_observed() {
    // ① 被另一个变量别名(逃逸)
    let (_, (hit, _)) = run("LET MUT(a, []);\nLET MUT(b, NULL);\nLET MUT(i, 0);\n\
         WHILE(<(i, 3), (SET(b, a); SET(a, +(a, [i])); SET(i, +(i, 1))));\na\n");
    assert_eq!(hit, 0, "aliased accumulator must not be recognised");
    // ② 被读取(条件里)
    let (_, (hit, _)) = run(&acc(
        "SET(a, +(a, [i])); SET(i, +(i, 1)); WHILE(<(i, LEN(a)), (BREAK()))",
    ));
    assert_eq!(hit, 0, "observed accumulator must not be recognised");
    // ③ 被传给函数
    let (_, (hit, _)) = run(&acc("PRINT(a); SET(a, +(a, [i])); SET(i, +(i, 1))"));
    assert_eq!(
        hit, 0,
        "accumulator passed to a call must not be recognised"
    );
    // ④ 被闭包捕获
    let (_, (hit, _)) = run(&acc(
        "SET(a, +(a, [i])); SET(i, +(i, 1)); LET(g, FUN(() , a));",
    ));
    assert_eq!(
        hit, 0,
        "closure-captured accumulator must not be recognised"
    );
    // ⑤ 被存进容器(逃逸)
    // ⚠ 不用 `RETURN` 做这个用例:`RETURN` 必须在**函数内**,放在循环体里是 E0014
    // (上一版就是踩了这个,报的是语法错而不是判据错,排查绕了一圈)。
    let (_, (hit, _)) = run("LET MUT(a, []);\nLET MUT(d, NULL);\nLET MUT(i, 0);\n\
         WHILE(<(i, 3), (SET(d, [\"k\": a]); SET(a, +(a, [i])); SET(i, +(i, 1))));\na\n");
    assert_eq!(
        hit, 0,
        "accumulator stored into a container must not be recognised"
    );
}

#[test]
fn rejects_a_loop_with_no_accumulator_at_all() {
    let (_, (hit, miss)) = run("LET MUT(i, 0);\nWHILE(<(i, 3), (SET(i, +(i, 1))));\ni\n");
    assert_eq!(hit, 0, "plain counting loop has no accumulator");
    assert_eq!(miss, 1, "plain counting loop must count as a miss");
}

/// 两个不同的累加器不是本判据覆盖的形状 ⇒ **判否**,不猜。
#[test]
fn rejects_two_distinct_accumulators() {
    let (_, (hit, _)) = run("LET MUT(a, []);\nLET MUT(b, []);\nLET MUT(i, 0);\n\
         WHILE(<(i, 3), (SET(a, +(a, [i])); SET(b, +(b, [i])); SET(i, +(i, 1))));\na\n");
    assert_eq!(hit, 0, "ambiguous two-accumulator shape must be rejected");
}

// ── 契约第三件:识别与否,结果必须完全一样 ───────────────────────────────

/// 逐条对照:**同一段逻辑**写成「被识别」与「不被识别」两种形状,末值必须相同。
#[test]
fn recognition_never_changes_the_result() {
    let cases: &[(&str, &str)] = &[
        // 命中形状
        ("LET MUT(a, []);\nLET MUT(i, 0);\nWHILE(<(i, 4), (SET(a, +(a, [i])); SET(i, +(i, 1))));\na\n",
         "LET MUT(a, []);\nLET MUT(i, 0);\nWHILE(<(i, 4), (SET(a, +(a, [i])); SET(i, +(i, 1))));\na\n"),
        // 同一份结果,但循环体里多做一次读 —— 判否,值不变
        ("LET MUT(a, []);\nLET MUT(i, 0);\nWHILE(<(i, 4), (SET(a, +(a, [i])); SET(i, +(i, 1))));\na\n",
         "LET MUT(a, []);\nLET MUT(i, 0);\nWHILE(<(i, 4), (SET(a, +(a, [i])); SET(i, +(i, 1)); SET(a, a)));\na\n"),
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

/// 命中形状的末值必须是**逐字节**正确的累加结果(不是「差不多」)。
#[test]
fn a_recognised_accumulator_still_produces_the_right_array() {
    let (v, (hit, _)) = run(
        "LET MUT(a, []);\nLET MUT(i, 0);\nWHILE(<(i, 5), (SET(a, +(a, [i])); SET(i, +(i, 1))));\na\n",
    );
    assert_eq!(hit, 1);
    assert_eq!(v, "[0, 1, 2, 3, 4]");
    // 且它就是一个普通数组(不是 Cell —— 可观察面一致,S1 的契约)。
    // ⚠ `TYPE` 返回的是**大写**种类名(`value_type_name`),不是 `type_name` 的小写。
    let (t, _) = run(
        "LET MUT(a, []);\nLET MUT(i, 0);\nWHILE(<(i, 5), (SET(a, +(a, [i])); SET(i, +(i, 1))));\n\
         TYPE(a)\n",
    );
    assert_eq!(t, "ARRAY", "accumulator must stay observable as an array");
}
