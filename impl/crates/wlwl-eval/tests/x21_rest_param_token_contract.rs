//! **rest 形参的未消费令牌通道**契约测试 —— v1.0 规范 §8.6.3「实参的未消费
//! 状态转给形参绑定」。
//!
//! ## 这条缺口说的是什么
//!
//! §8.6.3:实参的未消费 `ERR` 状态**转给形参绑定**。**固定形参**那一位一直
//! 是通的(`invoke_closure` 用 `set_local_with_token(name, value, set)`)。
//!
//! **rest 形参**(`FUN((*args), …)`)那一位此前用 `set_local`(**不带令牌**)
//! ⇒ **富余实参持有的未消费 `ERR` 被整个丢弃** ⇒ 被调方作用域结束时**一条
//! `E0103` 都不报**(⇒ 假阴性)。
//!
//! ## 判别性读数
//!
//! * 富余实参**不是** `ERR` 形态、却**持有**未消费 `ERR`(例如富余实参是装着
//!   `ERR` 的数组)⇒ **修前 0 条 → 修后恰好 1 条 `E0103`**。
//! * 富余实参**是** `ERR` 形态 ⇒ 走 §12.6 透明传播**短路**(被调方**根本没跑**)
//!   ⇒ rest 绑定**没参与** ⇒ 修前修后**完全一样**(仍报在**调用点**)。
//!
//! ## 报告点
//!
//! 作用域结束处的 `E0103` 沿用**既有**约定:报在**调用点**那一行(`span` 是
//! 调用方给的实参位置)—— 固定形参那条路径一直就是这个位置 ⇒ rest 位现在与
//! 它**同一口径** ⇒ ⇒ **本项是纯增量:条数 0 → 1,报告位置不动**。
//!
//! ⚠ **刻意不改**报告位置:把 span 改成被调方的函数体(`body.span()`)会**全局**
//! 挪动所有闭包的 scope-end 报告位置(超出本项范围)⇒ 留作单独提案。
//!
//! ## 形状
//!
//! 一个用例 = 一个 `#[test]`(同 `x01_channel_close_contract.rs` 的形状):
//! ⚠ **每条都断言诊断的条数**(按错误码分别计数),并且**断言报告所在的行**
//! —— 行断言的职责是「位置不许动」(本批是纯增量)。

use wlwl_eval::Evaluator;

/// 一次求值的读数。
struct Reading {
    /// 失败 ⇒ 全部诊断的「码 @行:列」(按报告顺序);成功 ⇒ 空。
    diagnostics: Vec<String>,
    /// 失败 ⇒ `E0103` 的条数。
    e0103: usize,
    /// 失败 ⇒ `E0103` 所在的行号(按出现序)。
    e0103_lines: Vec<u32>,
}

fn reading(src: &str) -> Reading {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new();
    match ev.eval(&ast) {
        Ok(_) => Reading {
            diagnostics: Vec::new(),
            e0103: 0,
            e0103_lines: Vec::new(),
        },
        Err(e) => {
            let diagnostics: Vec<String> = e
                .diagnostics()
                .iter()
                .map(|d| {
                    format!(
                        "{} @{}:{}",
                        d.code.as_str(),
                        d.location.line,
                        d.location.col
                    )
                })
                .collect();
            let e0103_lines: Vec<u32> = e
                .diagnostics()
                .iter()
                .filter(|d| d.code.as_str() == "E0103")
                .map(|d| d.location.line)
                .collect();
            let e0103 = e0103_lines.len();
            Reading {
                diagnostics,
                e0103,
                e0103_lines,
            }
        }
    }
}

/// 一条用例 = 一个 `#[test]`。
///
/// `$expect_e0103` —— `E0103` 的**条数**(判别性读数)。
/// `$expect_lines` —— 那些 `E0103` 所在的**行号**(按出现序)。
macro_rules! case {
    ($name:ident, $src:expr, $expect_e0103:expr, $expect_lines:expr) => {
        #[test]
        fn $name() {
            let r = reading($src);
            assert_eq!(
                r.e0103,
                $expect_e0103,
                "case `{}` (§8.6.3 / rest 通道): E0103 条数\n  诊断: {:?}",
                stringify!($name),
                r.diagnostics
            );
            assert_eq!(
                r.e0103_lines,
                $expect_lines,
                "case `{}` (§8.6.3 / rest 通道): E0103 所在的行\n  诊断: {:?}",
                stringify!($name),
                r.diagnostics
            );
        }
    };
}

// ── 缺口本体:富余实参持有的未消费 `ERR` 被 rest 绑定整个丢弃 ─────────────

// `CALL(f, [ERR("boom")])` —— `[ERR("boom")]` 是**一个**数组实参 ⇒ 它整个进
// rest 绑定 ⇒ 它持有的未消费 `ERR` 归那个绑定 ⇒ 被调方作用域结束时是丢弃点
// ⇒ 恰好 1 条 `E0103`(修前 **0 条** ⇒ 判别),报在**调用点**那一行。
case!(
    x21_rest_param_carries_the_surplus_arg_token_into_the_callee_scope,
    r#"LET(f, FUN((*args), 0));
CALL(f, [ERR("boom")]);
PRINT("done");"#,
    1,
    vec![2]
);

// 直接调用形态(不经 `CALL`)⇒ 同款。
case!(
    x21_the_direct_call_form_carries_the_token_too,
    r#"LET(f, FUN((*args), 0));
f([ERR("boom")]);
PRINT("done");"#,
    1,
    vec![2]
);

// 富余的是**第二个**实参(`x` 吃掉第一个)⇒ 令牌落在 rest 绑定上 ⇒ 恰好 1 条。
case!(
    x21_the_surplus_arg_being_the_second_one_still_carries_its_token,
    r#"LET(f, FUN((x, *args), 0));
CALL(f, 1, [ERR("boom")]);
PRINT("done");"#,
    1,
    vec![2]
);

// 结果被**绑定**时同样是恰好 1 条(不许多出一条)。
case!(
    x21_a_bound_result_does_not_add_a_second_report,
    r#"LET(f, FUN((*args), 0));
LET(x, CALL(f, [ERR("boom")]));
PRINT("done");"#,
    1,
    vec![2]
);

// 被调方**读走** rest 绑定(Transfer)⇒ 令牌跟着**值**走 ⇒ 值进 `x` ⇒ 处置点是
// **模块作用域结束**(不是被调方的作用域)⇒ 恰好 1 条。
//
// ⚠ **期望于 2026-10-24 改过**(审计 F1):此前钉「模块作用域结束的 span =
// 第 1 行(整个块的起点)」—— 当令牌来自**后面的** `LET` 时,那个 span 会指到
// 无关的首句(例如先写 `LET(f, FUN(...))` 再写 `LET(x, CALL(...))` 时报在
// `LET(f)` 上)⇒ 现改为钉**绑定建立处**(值表达式的位置 = `CALL` 那一行,
// 第 2 行)。⚠ 条数与丢弃点不变(仍是模块作用域结束、仍恰好 1 条);变的
// **只是报告位置**。⇒ 旧期望**不再代表**「可定位到出事绑定」这一目标。
case!(
    x21_a_callee_that_reads_the_rest_binding_moves_the_token_with_the_value,
    r#"LET(f, FUN((*args), args));
LET(x, CALL(f, [ERR("boom")]));
PRINT("done");"#,
    1,
    vec![2]
);

// ── 并集:富余实参**不止一个**时,每一份的集合都要转给 rest 绑定 ───────────

// 两个富余实参**各自**持有一个未消费的 `ERR` ⇒ 并集 = 2 个 ⇒ 恰好 2 条。
case!(
    x21_two_surplus_args_each_holding_an_err_report_two_e0103,
    r#"LET(f, FUN((*args), 0));
CALL(f, [ERR("a")], [ERR("b")]);
PRINT("done");"#,
    2,
    vec![2, 2]
);

// 只有**第一个**富余实参持有未消费的 `ERR` ⇒ 恰好 1 条(只取最后一个的实现
// 在这条上会得到 0 条 ⇒ 判别)。
case!(
    x21_only_the_first_surplus_arg_holding_an_err_reports_one_e0103,
    r#"LET(f, FUN((*args), 0));
CALL(f, [ERR("a")], [1]);
PRINT("done");"#,
    1,
    vec![2]
);

// 只有**最后一个**富余实参持有未消费的 `ERR` ⇒ 恰好 1 条(只取第一个的实现
// 在这条上会得到 0 条 ⇒ 判别)。
case!(
    x21_only_the_last_surplus_arg_holding_an_err_reports_one_e0103,
    r#"LET(f, FUN((*args), 0));
CALL(f, [1], [ERR("b")]);
PRINT("done");"#,
    1,
    vec![2]
);

// ── 回归守卫:短路路径(rest 绑定**没参与**)不许变 ───────────────────────

// 富余实参**是** `ERR` 形态 ⇒ §12.6 透明传播**短路**整个调用 ⇒ 被调方**根本
// 没跑** ⇒ rest 绑定没参与 ⇒ 修前修后**完全一样**:恰好 1 条,且仍报在
// **调用点**那一行(第 2 行)。
case!(
    x21_an_err_shaped_surplus_arg_still_short_circuits_at_the_call_site,
    r#"LET(f, FUN((*args), 0));
CALL(f, ERR("boom"));
PRINT("done");"#,
    1,
    vec![2]
);

// 富余实参**没有**未消费的 `ERR` ⇒ 不许凭空多报。
case!(
    x21_a_surplus_arg_without_an_unconsumed_err_is_clean,
    r#"LET(f, FUN((*args), 0));
CALL(f, [1]);
PRINT("done");"#,
    0,
    Vec::<u32>::new()
);

// 固定形参那一位**一直**是通的 ⇒ rest 位现在与它**同一口径**(条数与报告位置
// 都与修前一致)。
case!(
    x21_the_fixed_param_path_keeps_the_same_report_point_convention,
    r#"LET(f, FUN((x), 0));
CALL(f, [ERR("boom")]);
PRINT("done");"#,
    1,
    vec![2]
);
