//! **X-06** 差异登记项的契约测试 —— v1.0 规范 §8.6「未消费状态是**集合**」。
//!
//! 规范基线:`docs/wlwl-spec-reference.md`(v1.0 合并册)语言核心册 §8.6
//! (§8.6.1 未消费的定义 / §8.6.2 丢弃点 / §8.6.3 转发 / §8.6.4 显式丢弃 /
//! §8.6.5 只报告一次),§11.1 诊断顺序。实现侧背景:
//! `docs/active/x06-x07-unconsumed-token-batch.md` §2.2 / §5。
//!
//! ## 这条差异登记项说的是什么
//!
//! 未消费状态**不是**一个标志,是**集合**:一个绑定 / 一个容器 / 一个数组元素
//! 序列都可以**同时持有多个**未消费的 `ERR`。于是三条语义:
//!
//! 1. **汇入同一接收位置时取并集** —— 全部保留,不是「保留第一个」,也不是
//!    「后者覆盖前者」;**不去重**(多重集口径:内容相同但来源不同的 `ERR`
//!    在身份未定前不能合并);
//! 2. **丢弃时逐个报告** —— 一个丢弃点上集合里有几个 `ERR` 就产生几条
//!    `E0103`,按**出现序**排列;
//! 3. **取走型消费者取走的是它拿到的那一个** —— 容器里还剩几个,不受它影响。
//!
//! ## 形状
//!
//! 一个用例 = 一个 `#[test]`(同 `e0103_contract.rs` 的形状):每条断言都能被
//! **单独**改错一次、单独变红。
//!
//! ## ⚠ 措辞与**条数**是两件事
//!
//! §8.6.3「容器本身在丢弃点被处置」⇒ 诊断的**措辞**报的是**被处置的那个
//! 值**:值是 `ERR` ⇒ 报它的**载荷**;值是容器 ⇒ 报容器的 `display()`。
//! 而**条数**由被丢弃的**集合**决定(集合里有几个 `ERR` 就几条 `E0103`)。
//!
//! ⇒ 因此「一个绑定持有两个未消费 `ERR`」⇒ **两条**诊断,但两条的措辞
//! **相同**(都是那个容器的 `display()`)—— 集合内的**出现序**决定的是
//! **发出次序**,不体现在措辞里;**可观察**的次序是**绑定之间**的
//! **名字序**(§11.1)—— 下面 `x06_scope_end_reports_every_binding_in_name_order`
//! 钉的就是它。

use wlwl_eval::Evaluator;

/// 求值一段源,返回**全部**诊断(报告顺序 = 出现序)。
///
/// ⚠ 用 `diagnostics()` 而不是 `diagnostic()`:后者只给**第一条** ⇒ 多诊断
/// 的场景(两个未消费 `ERR`)根本看不见。
///
/// ⚠ 求值**成功** ⇒ 空列表(「没有要报的东西」);「本来该报却没报」由
/// 各用例自己的断言钉住。
fn diagnostics(src: &str) -> Vec<(String, String)> {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new();
    match ev.eval(&ast) {
        Ok(_) => Vec::new(),
        Err(e) => e
            .diagnostics()
            .iter()
            .map(|d| (d.code.as_str().to_string(), d.message.clone()))
            .collect(),
    }
}

/// 求值一段源,返回**全部**诊断的消息(报告顺序)。
fn messages(src: &str) -> Vec<String> {
    diagnostics(src).into_iter().map(|(_, msg)| msg).collect()
}

/// 求值一段源,返回**全部**诊断的码(报告顺序)。
fn codes(src: &str) -> Vec<String> {
    diagnostics(src).into_iter().map(|(code, _)| code).collect()
}

/// 一条用例 = 一个 `#[test]`。
macro_rules! case {
    ($name:ident, $src:expr, $expect:expr) => {
        #[test]
        fn $name() {
            assert_eq!(
                messages($src),
                $expect,
                "case `{}` (§8.6 / X-06)",
                stringify!($name)
            );
        }
    };
}

/// 一条用例 = 一个 `#[test]`(钉诊断**码**)。
macro_rules! case_codes {
    ($name:ident, $src:expr, $expect:expr) => {
        #[test]
        fn $name() {
            assert_eq!(
                codes($src),
                $expect,
                "case `{}` (§8.6 / X-06)",
                stringify!($name)
            );
        }
    };
}

/// 一条用例 = 一个 `#[test]`(钉诊断**条数** —— 「逐个报告」的那一半)。
macro_rules! case_count {
    ($name:ident, $src:expr, $expect:expr) => {
        #[test]
        fn $name() {
            let got = diagnostics($src);
            assert_eq!(
                got.len(),
                $expect,
                "case `{}` (§8.6 / X-06): 集合里有几个 `ERR` 就产生几条 `E0103`\n{:#?}",
                stringify!($name),
                got
            );
        }
    };
}

// ── 丢弃时逐个报告(§8.6.2) ───────────────────────────────────────────────

// 一个绑定**同时持有两个**未消费的 `ERR` ⇒ 丢弃时**报两条** `E0103`。
case_count!(
    x06_a_binding_with_two_unconsumed_errs_reports_two_e0103,
    r#"LET(box, [ERR("one"), ERR("two")]); 0"#,
    2
);

// 码也钉住(两条都是 `E0103`)。
case_codes!(
    x06_two_unconsumed_errs_yield_two_e0103_codes,
    r#"LET(box, [ERR("one"), ERR("two")]); 0"#,
    vec!["E0103".to_string(), "E0103".to_string()]
);

// 措辞 = **被处置的那个值**(§8.6.3「容器本身在丢弃点被处置」)⇒ 容器形态。
case!(
    x06_the_wording_is_the_disposed_value_not_the_contained_err,
    r#"LET(box, [ERR("one"), ERR("two")]); 0"#,
    vec![
        "unhandled ERR was discarded: [ERR(one), ERR(two)]".to_string(),
        "unhandled ERR was discarded: [ERR(one), ERR(two)]".to_string(),
    ]
);

// 被处置的值**本身**是 `ERR` ⇒ 报它的**载荷**(不是 `ERR(载荷)` 那个包装)。
case!(
    x06_an_err_valued_binding_is_reported_by_its_payload,
    r#"LET(a, ERR("boom")); 0"#,
    vec!["unhandled ERR was discarded: boom".to_string()]
);

// 一个 `ERR` ⇒ 一条诊断(不是 0 条,也不是 2 条)。
case_count!(
    x06_one_unconsumed_err_is_reported_exactly_once,
    r#"LET(a, ERR("boom")); 0"#,
    1
);

// ── 汇入同一接收位置时取并集(§8.6.3) ─────────────────────────────────────

// 两个**各自持有**一个未消费 `ERR` 的绑定 ⇒ 汇入同一个接收位置(数组)⇒
// **两个都保留**(不是「保留第一个」,也不是「后者覆盖前者」)。
case_count!(
    x06_the_union_at_one_receiving_position_keeps_both_errs,
    r#"LET(a, ERR("one")); LET(b, ERR("two")); LET(box, [a, b]); 0"#,
    2
);

// **不去重**(多重集口径):两个**内容相同**、来源不同的 `ERR` ⇒ 两条诊断。
case_count!(
    x06_the_union_does_not_deduplicate_identical_errs,
    r#"LET(box, [ERR("same"), ERR("same")]); 0"#,
    2
);

// `ERR(...)` 构造器也是**接收位置**(§8.6.3「读入其它任何位置 ⇒ 由接收方
// 承担」)⇒ 它把载荷读了进来 ⇒ 载荷里未消费的 `ERR` 与它自己**取并集**。
case_count!(
    x06_the_err_constructor_unions_the_payloads_unconsumed_errs,
    r#"LET(a, ERR(["k": ERR("one")])); 0"#,
    2
);

// ── 取走型消费者取走的是它拿到的那一个(§8.6.1 / §8.6.3) ─────────────────

// 取走型消费者(`ERR_PAYLOAD`)从**一个持有两个**未消费 `ERR` 的绑定上取走
// **它拿到的那一个** ⇒ **另一个还在** ⇒ 仍然报一条 `E0103`。
case_count!(
    x06_a_take_away_consumer_takes_only_the_err_it_got,
    r#"LET(a, ERR(["k": ERR("one")])); LET(p, ERR_PAYLOAD(a)); 0"#,
    1
);

// 反向守卫:取走型消费者**没有**碰到的那个 `ERR` 仍然被报出来。
case!(
    x06_the_err_a_take_away_consumer_did_not_take_is_still_reported,
    r#"LET(a, ERR("one")); LET(b, ERR("two")); LET(p, ERR_PAYLOAD(a)); 0"#,
    vec!["unhandled ERR was discarded: two".to_string()]
);

// 取走型消费者拿到的**值本身**就是一个 `ERR` ⇒ 它离开未消费状态 ⇒ 不报。
case_count!(
    x06_a_take_away_consumer_leaves_no_unconsumed_err_behind,
    r#"LET(a, ERR("boom")); LET(p, ERR_PAYLOAD(a)); p"#,
    0
);

// §8.6.5「一个 `ERR` **至多**被报告一次」—— 被 `OK` 包装的 `ERR` 被取走型
// 消费者取走时(`UNWRAP_OR` 剥掉 `OK` 外壳拿到的是**内层**的 `ERR`),源绑定
// 里那一份也必须一起消失 ⇒ **只**报一条。
case_count!(
    x06_an_ok_wrapped_err_is_reported_at_most_once,
    r#"LET(r, OK(ERR("boom"))); LET(p, UNWRAP_OR(r, NULL)); 0"#,
    1
);

// ── 三个丢弃点,逐个报告(§8.6.2) ──────────────────────────────────────────

// 丢弃点① **语句值丢弃** —— 集合里有几个就几条。
case_count!(
    x06_discard_point_1_statement_value_discard_reports_every_err,
    r#"PRINT("first"); [ERR("one"), ERR("two")]; PRINT("second")"#,
    2
);

// 丢弃点② **绑定被覆盖**(同作用域重绑定)—— 集合里有几个就几条。
case_count!(
    x06_discard_point_2_rebinding_reports_every_err,
    r#"LET(box, [ERR("one"), ERR("two")]); LET(box, 0); 0"#,
    2
);

// 丢弃点② **绑定被覆盖**(`SET` 改写单元格)—— 同款。
case_count!(
    x06_discard_point_2_set_overwrite_reports_every_err,
    r#"LET MUT(box, [ERR("one"), ERR("two")]); SET(box, 0); 0"#,
    2
);

// 丢弃点③ **作用域结束时仍未消费** —— 集合里有几个就几条。
case_count!(
    x06_discard_point_3_scope_end_reports_every_err,
    r#"LET(box, [ERR("one"), ERR("two")])"#,
    2
);

// 丢弃点③:作用域里**多个绑定**各自持有未消费 `ERR` ⇒ 全部报出来,顺序 =
// **绑定间按名字序**(§11.1)—— `a` 在 `b` 前 ⇒ `aye` 在 `bee` 前。
//
// ⚠ 这是**可观察**的次序:每个绑定**各报各的**(措辞 = 那个绑定持有的值)。
case!(
    x06_scope_end_reports_every_binding_in_name_order,
    r#"LET(b, ERR("bee")); LET(a, ERR("aye")); 0"#,
    vec![
        "unhandled ERR was discarded: aye".to_string(),
        "unhandled ERR was discarded: bee".to_string(),
    ]
);

// ── §8.6.4 显式丢弃 / §8.6.5 只报告一次 / X-17 ──────────────────────────

// §8.6.4:`LET(_, expr)` 是唯一的显式丢弃构造 ⇒ 那个 `ERR` **不**产生
// `E0103` ⇒ 作用域结束也不报。
case_count!(
    x06_an_explicitly_discarded_err_is_not_reported,
    r#"LET(a, ERR("boom")); LET(_, a); 0"#,
    0
);

// 显式丢弃一个**新生的** `ERR` ⇒ 同样不报。
case_count!(
    x06_an_explicitly_discarded_new_err_is_not_reported,
    r#"LET(_, ERR("boom")); 0"#,
    0
);

// 读走之后源绑定不再持有它 ⇒ 作用域结束**不**再报一次(§8.6.5)。
case_count!(
    x06_a_transferred_err_is_not_reported_twice,
    r#"LET(a, ERR("boom")); LET(b, a); 0"#,
    1
);

// X-17:容器持有的未消费 `ERR` 在语句值丢弃点也要报 —— `LET(a, [ERR("boom")]);`
// 之后 `a;` 这条语句的值是**容器**,容器**持有**一个未消费的 `ERR` ⇒ 丢弃点。
case!(
    x06_a_container_holding_an_unconsumed_err_is_reported_at_the_statement_discard_point,
    r#"LET(a, [ERR("boom")]); a; 0"#,
    vec!["unhandled ERR was discarded: [ERR(boom)]".to_string()]
);
