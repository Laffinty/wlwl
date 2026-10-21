//! `E0103` 未消费 `ERR` 的报告 —— **v1.0 规范 §8.6 契约测试**。
//!
//! 规范基线:`Laffinty/wlwl-spec-v1` 提交 `24d7a6e`,语言核心册 §8.5 /
//! §8.6(含 §8.6.1 未消费的定义 / §8.6.2 丢弃点 / §8.6.3 转发 /
//! §8.6.4 显式丢弃 / §8.6.5 只报告一次),错误码表 §11.2 的 `E0103` 行。
//!
//! ## 这份契约的**形状**
//!
//! 一个用例 = 一个 `#[test]`。这样每条断言都能被**单独**改错一次、单独变红
//! ——「整张表一起跑」的写法只能证明「有一条红了」,证不了「每条都有判别力」。
//!
//! ## 判别力(反向守卫)
////!
//! 每个用例都做过负向验证:把 `expect` 故意改成一个错的值,确认该用例**变红**。
//! 记录见 `docs/active/addendum-15-e0103-unconsumed-err.md` §5。
//!
//! ## 与 v0.11 的**破坏性差异**
//!
//! v0.11 把「非末条语句的 `ERR`」**静默丢弃**(零诊断)。v1.0 删掉了这条限制:
//! 被丢弃的未消费 `ERR` **必须**报 `E0103`。所以本文件里那些「以前是绿的、
//! 现在必须红」的用例正是这一批的验收点。

use wlwl_eval::Evaluator;

// 求值一段源,返回:
// * 成功 ⇒ 值的 `display()`;
// * 失败 ⇒ `!{码} {消息}`。
fn actual(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new();
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => {
            let code = e.diagnostic().code.as_str();
            if e.diagnostic().message.is_empty() {
                format!("!{code}")
            } else {
                format!("!{code} {}", e.diagnostic().message)
            }
        }
    }
}

// 一条用例 = 一个 `#[test]`(见模块文档的「形状」段)。
macro_rules! case {
    ($name:ident, $src:expr, $expect:expr) => {
        #[test]
        fn $name() {
            assert_eq!(actual($src), $expect, "case `{}` (§8.6)", stringify!($name));
        }
    };
}

// ── §8.6.1 未消费的定义 ────────────────────────────────────────────────
//
// 消费者按是否把 `ERR` 的载荷取走分成两类:取走型 / 观察型。

// §8.6.1 取走型:该 `ERR` 被取走,不再处于未消费状态 ⇒ 不报 `E0103`。
case!(
    s861_takeaway_consumer_takes_the_err_away,
    r#"LET(a, ERR("boom")); LET(p, ERR_PAYLOAD(a)); p"#,
    "boom"
);

// §8.6.1 观察型:`IS_ERR` 只看不带走 ⇒ 未消费状态**留在被读取的绑定** ⇒
// 作用域结束时按 §8.6.2 报 `E0103`。
case!(
    s861_observe_consumer_keeps_the_state_in_the_source_binding,
    r#"LET(a, ERR("boom")); IS_ERR(a)"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// §8.6.1 观察型:`BOOL` 同样是观察型(它的返回值是布尔,不是 `ERR`)⇒
// 令牌**没有**被取走,只是被看了一眼 ⇒ 仍在源绑定上 ⇒ 报 `E0103`。
case!(
    s861_observe_consumer_bool_keeps_the_state_in_the_source_binding,
    r#"LET(a, ERR("boom")); BOOL(a)"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// §8.6.1 / §8.6.3:`==` / `!=` 在 §8.6.1 的观察型那一行,但 §8.6.3 明文
// 「按 §2.4 **透传**该 `ERR`」⇒ 未消费状态**转给结果值** ⇒ 走 `Transfer`
// ⇒ 源绑定不再持有它,新绑定持有它 ⇒ 报 `E0103`(且只报一次,见 §8.6.5)。
case!(
    s861_equality_transfers_the_unconsumed_state_to_the_result,
    r#"LET(a, ERR("boom")); LET(b, ==(a, 1)); 0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// ── §8.6.2 丢弃点(恰好三处) ────────────────────────────────────────────

// §8.6.2 第 1 项 **语句值丢弃** —— 一个表达式语句的值被丢弃(既不成为
// 所在作用域的返回值,也不成为程序值)⇒ 丢弃点。
case!(
    s862_discard_point_1_statement_value_is_discarded,
    r#"PRINT("first"); ERR("boom"); PRINT("second")"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// §8.6.2 第 2 项 **绑定被覆盖** —— 同作用域重绑定(§3.2)改写了持有未消费
// `ERR` 的旧绑定 ⇒ 丢弃点。
case!(
    s862_discard_point_2_rebinding_overwrites_the_binding,
    r#"LET(a, ERR("boom")); LET(a, 1); 0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// §8.6.2 第 2 项 **绑定被覆盖** —— `SET` 改写单元格(§3.4)⇒ 丢弃点。
case!(
    s862_discard_point_2_set_overwrites_the_cell,
    r#"LET MUT(z, ERR("boom")); SET(z, 1); 0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// §8.6.2 第 3 项 **作用域结束时仍未消费** —— 作用域结束时仍有绑定持有
// 未消费 `ERR` ⇒ 丢弃点。
case!(
    s862_discard_point_3_scope_ends_with_an_unconsumed_err,
    r#"LET(a, ERR("boom"))"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// §8.6.2 第 3 项:内层作用域(括号序列)结束时同样处置 —— 遮蔽**不**构成
// 丢弃点,但内层作用域结束**是**。
case!(
    s862_discard_point_3_inner_scope_ends_with_an_unconsumed_err,
    r#"(LET(a, ERR("boom"))); 0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// §8.6.2 说明:**遮蔽不构成丢弃点** —— 内层作用域的 `LET` 建立新单元格,
// 外层绑定不受影响 ⇒ 外层的 `ERR` 在**外层**作用域结束时才被处置。
case!(
    s862_shadowing_is_not_a_discard_point,
    r#"LET(a, ERR("boom")); (LET(a, 1); 0); 0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// §8.5 + §8.6.2:程序值**不**属丢弃点 —— 顶层最后一条表达式的值是程序值,
// 按 §8.5 报 `E0102`,**不**报 `E0103`。
case!(
    s862_program_value_is_not_a_discard_point,
    r#"ERR("boom")"#,
    "!E0102 unhandled ERR escaped to top level: boom"
);

// §8.6.2 说明:容器装盛**不**构成丢弃点,但构成转发(§8.6.3)⇒ 容器在
// 丢弃点被处置。
//
// ⚠ 被丢弃的值是**容器**(`[ERR(boom)]`),不是里面那个 `ERR` ⇒ 消息里
// 出现的是容器的 `display()`。
case!(
    s862_container_packing_is_a_forward_not_a_discard_point,
    r#"LET(box, [ERR("boom")]); 0"#,
    "!E0103 unhandled ERR was discarded: [ERR(boom)]"
);

// ── §8.6.3 转发 ────────────────────────────────────────────────────────

// §8.6.3 的**规范示例**(逐字):`LET(a, err); LET(b, a); ERR_PAYLOAD(b);`
// 因此**不**误报 —— `b` 取走了那个 `ERR`,`a` 的状态在读取时已转出。
case!(
    s863_spec_example_forwarding_does_not_misreport,
    r#"LET(a, ERR("boom")); LET(b, a); LET(p, ERR_PAYLOAD(b)); p"#,
    "boom"
);

// §8.6.3 的**规范示例**(逐字):`LET(_, ERR("boom")); PRINT("ok");`
case!(
    s864_spec_example_explicit_discard_does_not_misreport,
    r#"LET(_, ERR("boom")); PRINT("ok"); "ok""#,
    "ok"
);

// §8.6.3:读入**其它任何位置** ⇒ 由接收方承担 —— 接收位置建立绑定时,
// 未消费状态转给该绑定。
case!(
    s863_binding_position_takes_over_the_unconsumed_state,
    r#"LET(a, ERR("boom")); LET(b, a); 0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// §8.6.3 条件位:`IF` **无** else 臂 ⇒ 条件位的 `ERR` 是 `IF` 的结果值
// ⇒ 未消费状态**转给该结果值** ⇒ 走 `Transfer`。
case!(
    s863_if_without_else_transfers_the_condition_state_to_the_result,
    r#"LET(a, ERR("boom")); LET(b, IF(a, 1)); 0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// §8.6.3 条件位:`IF` **有** else 臂 ⇒ 条件位的 `ERR` 被该 `IF` 放弃 ⇒
// 未消费状态**留在被读取的绑定** ⇒ 走 `Observe`。
case!(
    s863_if_with_else_keeps_the_condition_state_in_the_source_binding,
    r#"LET(a, ERR("boom")); LET(b, IF(a, 1, 2)); 0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// §8.6.3 条件位:`&&` / `||` 的左操作数为 `ERR` 时,该 `ERR` 是该构造的
// 结果值(§8.2 透传),未消费状态**转给该结果值**。
case!(
    s863_short_circuit_left_operand_transfers_to_the_result,
    r#"LET(a, ERR("boom")); LET(b, &&(a, TRUE)); 0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// ── §8.6.4 显式丢弃 ────────────────────────────────────────────────────

// §8.6.4:`LET(_, expr)` 是本语言**唯一**的显式丢弃构造 ⇒ 不产生 `E0103`。
case!(
    s864_let_wildcard_is_the_explicit_discard_construct,
    r#"LET(_, ERR("boom")); 0"#,
    "0"
);

// §8.6.4 的说明:观察与处置是两回事 —— 观察不解除未消费状态,处置才解除。
// `IS_ERR(r)` 之后令牌仍在 `r` 上,`LET(_, r)` 才把它显式丢弃。
case!(
    s864_observe_then_explicit_discard_is_clean,
    r#"LET(r, ERR("boom")); LET(_, r); 0"#,
    "0"
);

// ── §8.6.5 只报告一次 ──────────────────────────────────────────────────

// §8.6.5:一个 `ERR` **至多**被报告一次。读走之后源绑定不再持有它 ⇒
// 语句值丢弃与作用域结束**不会**各报一次。
#[test]
fn s865_one_err_is_reported_at_most_once() {
    let out = actual(r#"LET(a, ERR("boom")); a; 0"#);
    assert_eq!(
        out, "!E0103 unhandled ERR was discarded: boom",
        "the ERR must be reported exactly once"
    );
    assert_eq!(
        out.matches("boom").count(),
        1,
        "the payload must be mentioned once — a second report would mean the \\
         same ERR was counted twice (§8.6.5)"
    );
}

// ── 函数返回(令牌随返回值带出函数体) ───────────────────────────────────

// 函数返回的未消费令牌随返回值带出函数体 ⇒ 调用点处置它 ⇒ 报 `E0103`。
case!(
    fn_return_carries_the_unconsumed_state_out_of_the_body,
    r#"LET(f, FUN((), ERR("boom"))); LET(v, f()); 0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// 函数返回的令牌带出函数体 ⇒ 函数体作用域内**不留**令牌 ⇒ 不重复报告。
#[test]
fn fn_return_does_not_double_report() {
    let out = actual(r#"LET(f, FUN((), ERR("boom"))); LET(v, f()); 0"#);
    assert_eq!(
        out.matches("boom").count(),
        1,
        "the function body's scope end and the call site must not each report \\
         the same ERR (§8.6.5)"
    );
}

// 函数体里**没有**返回的未消费 `ERR` ⇒ 函数体作用域结束时处置 ⇒ `E0103`。
case!(
    fn_body_scope_end_reports_an_unconsumed_err_that_stays_inside,
    r#"LET(f, FUN((), (LET(x, ERR("boom")); 1))); LET(v, f()); 0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// 显式 `RETURN` 的令牌同样带出函数体。
case!(
    fn_explicit_return_carries_the_unconsumed_state_out_of_the_body,
    r#"LET(f, FUN((), RETURN(ERR("boom")))); LET(v, f()); 0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// ── 已消费的 `ERR` 不报 ────────────────────────────────────────────────

// 消费过的 `ERR` 一路干净:取走型消费者取走 ⇒ 无令牌 ⇒ 无 `E0103`。
case!(
    consumed_err_is_clean_end_to_end,
    r#"LET(a, ERR("boom")); LET(p, ERR_PAYLOAD(a)); p"#,
    "boom"
);

// `TRY` 把 `ERR` 变成返回值逃逸 ⇒ 按 §8.5 走 `E0102`,**不**报 `E0103`。
case!(
    try_escapes_as_e0102_not_e0103,
    r#"LET(a, ERR("boom")); TRY(a)"#,
    "!E0102 unhandled ERR escaped to top level: boom"
);
