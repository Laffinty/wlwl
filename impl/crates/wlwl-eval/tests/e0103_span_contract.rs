//! **E0103 报告位置**契约 —— 作用域结束的 `E0103` **必须**钉到绑定建立处。
//!
//! ## 背景(审计 F1,2026-10-24)
//!
//! 作用域结束的丢弃点此前统一用「作用域块的 span」做报告位置 ⇒ 该 span
//! 总是落在**首句**上。当持有未消费 `ERR` 的绑定来自**后面的** `LET` 时,
//! 诊断会指到无关的首句(例如先写 `LET(f, FUN(..))` 再写 `LET(x, f(..))`
//! 时报在 `LET(f)` 上,列还是 1:1)⇒ 工具链/LSP 无法定位出事绑定。
//!
//! 修法:绑定记录**建立处**(`origin_span` = 值表达式的位置 / 形参对应的
//! 调用点)⇒ 报告优先钉那里。条数与丢弃点语义**逐字不变**。
//!
//! ## 形状
//!
//! 与 `x21_rest_param_token_contract.rs` 同款:一个用例 = 一个 `#[test]`,
//! 同时断言**条数**与**行号**。
//!
//! ⚠ 四象限(裸实参/容器 × 语句/绑定)全钉 —— 审计时正是「裸实参 + 绑定」
//! 这一格漏了覆盖才让 span 退化溜进主干。

use wlwl_eval::Evaluator;

struct Report {
    count: usize,
    lines: Vec<u32>,
    cols: Vec<u32>,
    codes: Vec<String>,
}

fn run(src: &str) -> Report {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new();
    match ev.eval(&ast) {
        Ok(_) => Report {
            count: 0,
            lines: Vec::new(),
            cols: Vec::new(),
            codes: Vec::new(),
        },
        Err(e) => {
            let diags = e.diagnostics();
            Report {
                count: diags.len(),
                lines: diags.iter().map(|d| d.location.line).collect(),
                cols: diags.iter().map(|d| d.location.col).collect(),
                codes: diags.iter().map(|d| d.code.as_str().to_string()).collect(),
            }
        }
    }
}

macro_rules! case {
    ($name:ident, $src:expr, $expect_count:expr, $expect_lines:expr) => {
        #[test]
        fn $name() {
            let r = run($src);
            assert_eq!(
                r.count,
                $expect_count,
                "case `{}` (E0103 span) 诊断条数\n  codes: {:?}",
                stringify!($name),
                r.codes
            );
            assert!(
                r.codes.iter().all(|c| c == "E0103"),
                "case `{}` 只允许 E0103, got {:?}",
                stringify!($name),
                r.codes
            );
            assert_eq!(
                r.lines,
                $expect_lines,
                "case `{}` (E0103 span) 报告行 —— 必须钉在绑定建立处\n  codes: {:?} cols: {:?}",
                stringify!($name),
                r.codes,
                r.cols
            );
        }
    };
}

// ── 四象限:裸实参 / 容器 × 语句 / 绑定 ─────────────────────────────────

// (1) 裸实参 + 语句:§12.6 短路 ⇒ 语句丢弃点 = 调用语句那一行。
case!(
    bare_arg_statement_reports_at_the_call_statement,
    r#"LET(f, FUN((a, b), 0));
f(1, ERR("boom"));
PRINT("done");"#,
    1,
    vec![2]
);

// (2) 裸实参 + 绑定(审计 F1 本尊):令牌随结果进绑定 ⇒ 模块作用域结束丢弃
//     ⇒ **必须**钉在绑定建立处(值表达式 = 调用那一行),不是块首句。
case!(
    bare_arg_bound_reports_at_the_binding_not_the_block_start,
    r#"LET(f, FUN((a, b), 0));
LET(x, f(1, ERR("boom")));
PRINT("done");"#,
    1,
    vec![2]
);

// (3) 容器实参 + 语句:rest 接管令牌 ⇒ 被调方作用域结束 ⇒ 调用点。
case!(
    container_arg_statement_reports_at_the_call_statement,
    r#"LET(f, FUN((*args), 0));
f([ERR("boom")]);
PRINT("done");"#,
    1,
    vec![2]
);

// (4) 容器实参 + 绑定,但被调方**读走** rest(Transfer)⇒ 令牌随值进绑定
//     ⇒ 模块作用域结束 ⇒ 钉在绑定建立处(CALL 那一行)。
case!(
    container_arg_bound_after_transfer_reports_at_the_binding,
    r#"LET(f, FUN((*args), args));
LET(x, CALL(f, [ERR("boom")]));
PRINT("done");"#,
    1,
    vec![2]
);

// ── 首句是无关定义时,不得指到块首 ────────────────────────────────────

// 审计里最刺眼的形态:先写一个 `LET(f, FUN(..))`,令牌却在后面的绑定上。
// 修前报在第 1 行 / 1:1;修后报在第 3 行(值表达式)。
case!(
    block_start_fun_definition_is_not_the_report_site,
    r#"LET(f, FUN((a, b), 0));
LET(unrelated, 1);
LET(x, f(1, ERR("boom")));
PRINT("done");"#,
    1,
    vec![3]
);

// ── 回归:真正在第 1 行建立绑定时,仍报第 1 行 ─────────────────────────

case!(
    first_line_binding_still_reports_at_line_1,
    r#"LET(a, ERR("one"));
PRINT("done");"#,
    1,
    vec![1]
);

// ── 回归:不许多报(结果被绑定不额外加一条) ───────────────────────────

case!(
    bound_result_does_not_add_a_second_report,
    r#"LET(f, FUN((a, b), 0));
LET(x, f(1, ERR("boom")));
PRINT("done");"#,
    1,
    vec![2]
);
