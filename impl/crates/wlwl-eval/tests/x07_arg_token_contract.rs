//! **X-07** 差异登记项的契约测试 —— v1.0 规范 §8.6.3「标准库回调的实参通道
//! 必须携带令牌」。
//!
//! 规范基线:`Laffinty/wlwl-spec-v1` 提交 `24d7a6e`,语言核心册 §8.6.1~§8.6.5。
//! 实现侧背景:`docs/active/addendum-15-e0103-unconsumed-err.md` §4.3 裁决 3
//! ②(一致性册 §5.4 的 **X-07**)。
//!
//! ## 这条差异登记项说的是什么
//!
//! §8.6.3:实参的未消费状态**转给形参绑定**。走**用户函数**时这条链是通的
//! (`eval_call` 算出实参令牌 → `invoke_closure` 落到形参绑定 → 回调作用域
//! 结束时按 §8.6.2 第 3 项报 `E0103`)。
//!
//! 走**标准库成员**时这条链此前**断在第一跳**:成员的实参通道不带令牌 ⇒
//! 成员不知道「我的实参带着未消费的 `ERR`」⇒ 成员回调用户函数时传不出令牌
//! ⇒ **回调里那个没被消费的 `ERR` 不报 `E0103`**(假阴性)。
//!
//! ## 修完之后的行为(本文件钉的就是这个)
//!
//! `MAP([ERR("boom")], FUN((x), 0))` —— 数组里的 `ERR` 没被回调消费 ⇒
//! 回调自己的作用域结束时它是丢弃点 ⇒ 报 `E0103`,载荷是 **`boom`**。
//!
//! ⚠ **与「直接调用用户函数」的形态不同,这是对的**:直接调用时整个数组进了
//! 形参绑定 ⇒ 被处置的是**容器** ⇒ 消息是 `[ERR(boom)]`;走标准库成员时
//! 回调收到的是**元素** ⇒ 被处置的是那个 `ERR` 本身 ⇒ 消息是 `boom`。
//! 两处形态**不统一**,但各自的形态都是「被处置的那个值」。
//!
//! ## 形状
//!
//! 一个用例 = 一个 `#[test]`(同 `e0103_contract.rs` 的形状):每条断言都能被
//! **单独**改错一次、单独变红。

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

// 一条用例 = 一个 `#[test]`。
macro_rules! case {
    ($name:ident, $src:expr, $expect:expr) => {
        #[test]
        fn $name() {
            assert_eq!(
                actual($src),
                $expect,
                "case `{}` (§8.6.3 / X-07)",
                stringify!($name)
            );
        }
    };
}

// ── 缺口本体:数组带着未消费的 `ERR` 进了标准库成员 ──────────────────────

// 数组实参里的 `ERR` 没被回调消费 ⇒ 回调作用域结束时是丢弃点 ⇒ `E0103`。
case!(
    x07_map_over_an_array_holding_an_unconsumed_err_reports_e0103,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(r, MAP([ERR("boom")], FUN((x), 0)));
0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// 同一件事的**绑定**形态:未消费状态在**绑定**上,读它时转进这次调用。
case!(
    x07_map_over_a_binding_holding_an_unconsumed_err_reports_e0103,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(a, [ERR("boom")]);
MAP(a, FUN((x), 0));
0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// 四个同族成员(迭代型)同款:`FILTER` / `GROUP_BY` / `DEDUP_BY` / `KEY_BY`。
case!(
    x07_filter_reports_the_same_way,
    r#"IMPORT("wlwl:std.collection", ["FILTER"]);
LET(r, FILTER([ERR("boom")], FUN((x), TRUE)));
0"#,
    "!E0103 unhandled ERR was discarded: boom"
);
case!(
    x07_group_by_reports_the_same_way,
    r#"IMPORT("wlwl:std.collection", ["GROUP_BY"]);
LET(r, GROUP_BY([ERR("boom")], FUN((x), 1)));
0"#,
    "!E0103 unhandled ERR was discarded: boom"
);
case!(
    x07_dedup_by_reports_the_same_way,
    r#"IMPORT("wlwl:std.collection", ["DEDUP_BY"]);
LET(r, DEDUP_BY([ERR("boom")], FUN((x), 1)));
0"#,
    "!E0103 unhandled ERR was discarded: boom"
);
case!(
    x07_key_by_reports_the_same_way,
    r#"IMPORT("wlwl:std.collection", ["KEY_BY"]);
LET(r, KEY_BY([ERR("boom")], FUN((x), 1)));
0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// `ERR` 嵌在**容器**里(不是元素本身)⇒ 被处置的是那个容器 ⇒ 消息是
// `[ERR(boom)]` —— 与「直接调用用户函数」的形态一致(见下面那条)。
case!(
    x07_a_nested_container_is_disposed_as_a_container,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(r, MAP([[ERR("boom")]], FUN((x), 0)));
0"#,
    "!E0103 unhandled ERR was discarded: [ERR(boom)]"
);

// ── 反向守卫:合规程序不许被误判(假阳性) ───────────────────────────────

// 回调**消费**了那个 `ERR` ⇒ 不在未消费状态 ⇒ **不**报。
//
// ⚠ 这条是本项修复的**假阳性闸**:令牌若被转给成员的**结果**(而不是回调的
// 形参绑定),成员无从知道回调已经消费了它 ⇒ 合规程序会被判成不合规。
case!(
    x07_a_callback_that_consumes_the_err_is_clean,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(r, MAP([ERR("boom")], FUN((x), (LET(p, ERR_PAYLOAD(x)); 0))));
0"#,
    "0"
);

// 数组里没有未消费的 `ERR` ⇒ 一路干净。
case!(
    x07_a_plain_array_through_a_std_member_is_clean,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(r, MAP([1, 2], FUN((x), *(x, 10))));
0"#,
    "0"
);

// ── 既有行为不得改变 ───────────────────────────────────────────────────

// 直接调用**用户函数**(不经标准库)⇒ 整个数组进形参绑定 ⇒ 被处置的是容器。
case!(
    x07_the_direct_user_function_path_still_reports_the_container_form,
    r#"LET(f, FUN((x), 0));
LET(r, f([ERR("boom")]));
0"#,
    "!E0103 unhandled ERR was discarded: [ERR(boom)]"
);

// 回调**返回** `ERR` ⇒ 照旧报(这条与 X-07 无关,修之前就报)。
case!(
    x07_a_callback_returning_an_err_is_still_reported,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(r, MAP([1, 2], FUN((x), ERR("boom"))));
0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// 回调**体内部**造了一个 `ERR` ⇒ 照旧报(回调自己的作用域结束)。
case!(
    x07_an_err_created_inside_the_callback_is_still_reported,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(r, MAP([1], FUN((x), (LET(y, ERR("boom")); 0))));
0"#,
    "!E0103 unhandled ERR was discarded: boom"
);
