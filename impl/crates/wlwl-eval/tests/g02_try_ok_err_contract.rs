//! **G-02** 契约测试 —— `TRY(OK(ERR(x)))` 的**假阴性**。
//!
//! 规范基线:v1.0 语言核心册 §8.6.1(取走型消费者)/ §8.6.3(读模式与转发)/
//! §8.6.5(至多报告一次 / 值语义即新身份);错误码 `E0103`(未消费的 `ERR` 被
//! 丢弃)、`E0102`(`ERR` 逃逸到顶层)。
//!
//! ## 这条缺口说的是什么
//!
//! `TRY` 是**取走型**消费者:实参位置走取走口径 ⇒ 源绑定里那个 `ERR` 被**取走**
//! (源绑定不再持有它)。而 `TRY` 又**剥掉 `OK` 外壳**,把内层 `ERR` 当结果交还。
//!
//! 按 §8.6.5「**值语义即新身份**」,剥壳得到的内层 `ERR` 是**新生的** ⇒ 它
//! **自己未消费** ⇒ 它的未消费状态必须**跟着结果继续**。
//!
//! 若不携带:源绑定已被取走(空)、在途值又不带 ⇒ 那个 `ERR` **一条 `E0103`
//! 都不报** ⇒ **假阴性**。
//!
//! 值层接口(`wlwl_value::ReadMode::ConsumeCarry`)已就位(提交 `a1fb4a2`):
//! 取走口径与 `Consume` 逐字相同,区别只在**读到的值是 `OK(ERR(..))`** 时
//! **在途值携带**那个被取走的 `ERR`;值**本身**是 `ERR` 时**不携带**;值是
//! **容器**时**不携带**。
//!
//! ## 形状
//!
//! 一个用例 = 一个 `#[test]`(同 `x01_channel_close_contract.rs` 的形状)。
//!
//! ⚠ **每条都断言诊断的条数**(按错误码分别计数)—— 「同一个 `ERR` 至多被
//! 报告一次」(§8.6.5)是这条修复的一部分:`OR_DIE` 另有弃用告警 `W0051`,
//! **告警不进 `e.diagnostics()`**(它走 `Evaluator::take_warnings()` 的独立
//! 通道),但这里**两条都断言**:告警条数也钉住。

use wlwl_eval::Evaluator;

/// 一次求值的读数。
struct Reading {
    /// 失败 ⇒ 全部诊断的「码 消息 @行:列」(按报告顺序);成功 ⇒ 空。
    diagnostics: Vec<String>,
    /// 失败 ⇒ 全部诊断里 `E0103` 的条数。
    e0103: usize,
    /// 失败 ⇒ 全部诊断里 `E0102` 的条数。
    e0102: usize,
    /// 求值器排空的告警(码)。
    warnings: Vec<String>,
}

fn reading(src: &str) -> Reading {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new();
    let result = ev.eval(&ast);
    let warnings: Vec<String> = ev
        .take_warnings()
        .iter()
        .map(|w| w.code.as_str().to_string())
        .collect();
    match result {
        Ok(_) => Reading {
            diagnostics: Vec::new(),
            e0103: 0,
            e0102: 0,
            warnings,
        },
        Err(e) => {
            let diagnostics: Vec<String> = e
                .diagnostics()
                .iter()
                .map(|d| {
                    format!(
                        "{} {} @{}:{}",
                        d.code.as_str(),
                        d.message,
                        d.location.line,
                        d.location.col
                    )
                })
                .collect();
            let e0103 = diagnostics
                .iter()
                .filter(|d| d.starts_with("E0103"))
                .count();
            let e0102 = diagnostics
                .iter()
                .filter(|d| d.starts_with("E0102"))
                .count();
            Reading {
                diagnostics,
                e0103,
                e0102,
                warnings,
            }
        }
    }
}

/// 一条用例 = 一个 `#[test]`。
///
/// `$expect_e0103` —— `E0103` 的**条数**(判别性读数)。
/// `$expect_e0102` —— `E0102` 的**条数**。
/// `$expect_warnings` —— 告警的**条数**。
macro_rules! case {
    ($name:ident, $src:expr, $expect_e0103:expr, $expect_e0102:expr, $expect_warnings:expr) => {
        #[test]
        fn $name() {
            let r = reading($src);
            assert_eq!(
                r.e0103,
                $expect_e0103,
                "case `{}` (§8.6.5 / G-02): E0103 条数\n  诊断: {:?}",
                stringify!($name),
                r.diagnostics
            );
            assert_eq!(
                r.e0102,
                $expect_e0102,
                "case `{}` (§8.5 / G-02): E0102 条数\n  诊断: {:?}",
                stringify!($name),
                r.diagnostics
            );
            assert_eq!(
                r.warnings.len(),
                $expect_warnings,
                "case `{}` (§8.6.5 / G-02): 告警条数\n  告警: {:?}",
                stringify!($name),
                r.warnings
            );
        }
    };
}

// ── 缺口本体:那个 `ERR` 现在一条 `E0103` 都不报 ─────────────────────────

// 绑���形态:源绑定持有 `OK(ERR(..))` ⇒ `TRY` 剥壳 ⇒ 内层 `ERR` 是**新生的**
// ⇒ 它自己未消费 ⇒ 必须**跟着结果继续** ⇒ 报**恰好一条** `E0103`。
case!(
    g02_try_of_a_binding_holding_ok_of_err_reports_exactly_one_e0103,
    r#"LET(a, OK(ERR("boom")));
LET(b, TRY(a));
PRINT("done");"#,
    1,
    0,
    0
);

// **不经过绑定**的形态:`OK` 构造器直接带集合 ⇒ 同一个缺口在这里也成立。
case!(
    g02_try_of_a_literal_ok_of_err_reports_exactly_one_e0103,
    r#"LET(b, TRY(OK(ERR("boom"))));
PRINT("done");"#,
    1,
    0,
    0
);

// `OR_DIE` 是 `UNWRAP_OR` 的弃用别名 ⇒ **同款**处理(另发一条 `W0051` 告警)。
case!(
    g02_or_die_of_a_literal_ok_of_err_reports_exactly_one_e0103,
    r#"LET(b, OR_DIE(OK(ERR("boom")), 0));
PRINT("done");"#,
    1,
    0,
    1
);

// `OR_DIE` 的**绑定**形态。
case!(
    g02_or_die_of_a_binding_holding_ok_of_err_reports_exactly_one_e0103,
    r#"LET(a, OK(ERR("boom")));
LET(b, OR_DIE(a, 0));
PRINT("done");"#,
    1,
    0,
    1
);

// ── 既有行为不许变:值**本身**是 `ERR` 时**不携带**(§8.6.5) ─────────────

// `ERR` 作为返回值逃逸 ⇒ 按 §8.5 走 `E0102`,**不许**变成 `E0103`。
case!(
    g02_try_of_a_plain_err_still_escapes_as_e0102,
    r#"LET(a, ERR("boom"));
TRY(a);
PRINT("done");"#,
    0,
    1,
    0
);

// `OK` 包着一个**普通值** ⇒ 没有未消费的 `ERR` ⇒ 一条诊断都没有。
case!(
    g02_try_of_ok_of_a_plain_value_is_clean,
    r#"LET(a, OK(1));
TRY(a);
PRINT("done");"#,
    0,
    0,
    0
);

// `OR_DIE` **处理掉**那个 `ERR`(走默认值)⇒ 默认值 `0` 不许白背一个
// ⇒ 一条诊断都没有。
case!(
    g02_or_die_of_a_handled_err_leaves_the_default_clean,
    r#"LET(a, ERR("boom"));
LET(b, OR_DIE(a, 0));
PRINT("done");"#,
    0,
    0,
    1
);

// ── `UNWRAP` 对照组:内建调用路径**另一套机制**,不许被本修复打成 2 条 ────

// `UNWRAP` 的结果作为**末条**语句 ⇒ 它成了**程序值** ⇒ 按 §8.5 走 `E0102`
// (程序值不属丢弃点)⇒ **0 条 `E0103`** + 1 条 `E0102`。
case!(
    g02_unwrap_as_the_last_statement_keeps_the_e0102_escape,
    r#"LET(a, OK(ERR("boom")));
PRINT(UNWRAP(a));"#,
    0,
    1,
    0
);

// `UNWRAP` 的结果作为**非末条**语句 ⇒ 那是「语句值丢弃」点 ⇒ 恰好 1 条 `E0103`。
case!(
    g02_unwrap_as_a_non_last_statement_reports_exactly_one_e0103,
    r#"LET(a, OK(ERR("boom")));
PRINT(UNWRAP(a));
PRINT("done");"#,
    1,
    0,
    0
);

// `UNWRAP` 的结果绑到**绑定**上 ⇒ 作用域结束时它是丢弃点 ⇒ 恰好 1 条 `E0103`。
case!(
    g02_unwrap_bound_to_a_binding_reports_exactly_one_e0103,
    r#"LET(a, OK(ERR("boom")));
LET(b, UNWRAP(a));
PRINT("done");"#,
    1,
    0,
    0
);

// `UNWRAP_OR` **处理掉**那个 `ERR` ⇒ 默认值 `0` 不许白背一个 ⇒ 一条诊断都没有。
case!(
    g02_unwrap_or_of_a_handled_err_leaves_the_default_clean,
    r#"LET(a, ERR("boom"));
PRINT(UNWRAP_OR(a, 0));
PRINT("done");"#,
    0,
    0,
    0
);

// ── §8.6.5「至多报告一次」:同一个 `ERR` 不许报两条 ─────────────────────

// 源绑定 `a` 已被 `TRY` **取走** ⇒ 它**不得**再报一次 ⇒ 全程恰好 1 条。
case!(
    g02_the_source_binding_does_not_report_the_err_a_second_time,
    r#"LET(a, OK(ERR("boom")));
LET(b, TRY(a));
PRINT("done");"#,
    1,
    0,
    0
);

// 结果又被 `IS_ERR` **观察**了一次(`IS_ERR` 是观察型 ⇒ 不消费)⇒ 那个 `ERR`
// 仍只报一次。
case!(
    g02_observing_the_result_again_does_not_report_it_twice,
    r#"LET(a, OK(ERR("boom")));
LET(b, TRY(a));
LET(c, IS_ERR(b));
PRINT("done");"#,
    1,
    0,
    0
);
