//! **X-20** 差异登记项的契约测试 —— v1.0 规范 §8.6.3「实参进**成员**时，
//! 那份未消费状态落在哪」。
//!
//! 规范基线:`Laffinty/wlwl-spec-v1` 一致性册 §5.4 的 **X-20** 行;语言核心册
//! §8.6.1~§8.6.5(尤其是 §8.6.3 的**第一条款**与**第三条款**的分工)。
//!
//! ## 这条差异登记项说的是什么
//!
//! §8.6.3 第一条款:实参的未消费状态**转给形参绑定** —— 走**用户函数**时这条链
//! 是通的(`eval_call` 算出实参集合 → `invoke_closure` 落到形参绑定 → 被调方
//! 作用域结束时按 §8.6.2 第 3 项报 `E0103`)。
//!
//! §8.6.3 第三条款:**接收位置是成员调用的实参位**时,那份未消费状态**转给该
//! 调用的结果**。标准库成员的实参位**不建立绑定**(实参不是绑定)⇒ 必须有个
//! 确定的去处 ⇒ 本条定的是「转给该调用的结果」。
//!
//! ⚠ **此前这一层是断的**:标准库成员把实参边界上的集合**吞掉** ⇒
//! `LET(a, [ERR("boom")]); PRINT(a);` **不报** `E0103`(假阴性),而
//! `LET(b, a);` 报 ⇒ 洞精确地在**成员边界**上。
//!
//! ## 两条条款的分工(**别混同**)
//!
//! | 接收位置 | 去向 | 本表的形状 |
//! |---|---|---|
//! | **建立绑定**(用户函数 / 内建 `CALL` 的被调方 / `CALL_METHOD` 的方法 / `NEW` 的初始化器) | 转给**那个绑定**(§8.6.3 第一条款)⇒ 调用帧**不得再持有** | `x20_c` / `x20_d` |
//! | **不建立绑定**(标准库成员的实参位) | 转给**该调用的结果**(§8.6.3 第三条款)⇒ **结果承担** | `x20_a` / `x20_b` |
//!
//! ⇒ 同一份状态**只在一个地方** ⇒ 同一个 `ERR` 只会报一次(§8.6.5)。
//!
//! ## 形状
//!
//! 一个用例 = 一个 `#[test]`(同 `e0103_contract.rs` 的形状)。
//!
//! ⚠ **每条用例都断言「`E0103` 的条数」**,不只是第一条诊断的消息 —— 只看
//! 第一条**看不见双报**:同一个 `ERR` 被算两遍时,两条 `E0103` 落在**同一个**
//! 报告点上(作用域结束那条会把所有待处置集合**逐个**报成多条诊断)⇒ 第一条
//! 消息**一模一样** ⇒ 只有数条数才看得见(§8.6.5「至多报告一次」)。

use wlwl_eval::Evaluator;

// 求值一段源,返回:
// * (第一条诊断的 "码 消息", 全部 `E0103` 的条数);
// * 成功 ⇒ ("", 0)。
fn actual(src: &str) -> (String, usize) {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new();
    match ev.eval(&ast) {
        Ok(_) => (String::new(), 0),
        Err(e) => {
            let first = e.diagnostic();
            let code = first.code.as_str();
            let msg = if first.message.is_empty() {
                format!("!{code}")
            } else {
                format!("!{code} {}", first.message)
            };
            // ⚠ 数**全部** `E0103` —— 多诊断变体里 `diagnostic()` 只给第一条,
            // 重复报告藏在后面那几条里(§8.6.5 的红线就靠这个数守)。
            let n = e
                .diagnostics()
                .iter()
                .filter(|d| d.code.as_str() == "E0103")
                .count();
            (msg, n)
        }
    }
}

// 一条用例 = 一个 `#[test]`:**消息**与**条数**一起断言。
macro_rules! case {
    ($name:ident, $src:expr, $expect:expr, $count:expr) => {
        #[test]
        fn $name() {
            let (msg, n) = actual($src);
            assert_eq!(
                msg,
                $expect,
                "case `{}` (§8.6.3 / X-20) 的消息",
                stringify!($name)
            );
            assert_eq!(
                n,
                $count,
                "case `{}` (§8.6.3 / X-20) 的 E0103 条数 —— §8.6.5「一个 ERR 至多一次」",
                stringify!($name)
            );
        }
    };
}

// ── 形态一:成员**转给回调** ⇒ 结果**不携带** ─────────────────────────────

// 成员(`MAP`)把数组实参的未消费状态**转给回调的形参绑定** ⇒ 回调不消费它 ⇒
// 回调自己的作用域结束是丢弃点 ⇒ 报 `E0103`。
//
// ⚠ 被处置的是那个 `ERR` **本身**(回调收到的是元素)⇒ 消息是载荷 `boom`。
//
// ⚠ 与 x07 的冻结期望**一致**(同一形状、同一条消息)—— 本表是它的**边界**
// 侧对照:这里额外钉住「**结果不携带**」(条数恰好 1)。
case!(
    x20_a_member_forwards_the_state_to_the_callback_and_the_result_carries_nothing,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(r, MAP([ERR("boom")], FUN((x), 0)));
0"#,
    "!E0103 unhandled ERR was discarded: boom",
    1
);

// ── 形态二:成员**不转给回调** ⇒ 那份状态**转给该调用的结果**(甲案) ───────

// `PRINT` 没有回调 ⇒ 实参位**不建立绑定** ⇒ 按 §8.6.3 第三条款,那份状态
// **转给该调用的结果** ⇒ 结果(`NULL`)承担它 ⇒ 本语句即语句值丢弃点 ⇒ 报。
//
// ⚠ **breaking**:修之前这一条**不报**(集合在成员边界被吞掉)⇒ 已发布的程序
// 从这一条开始会多一条 `E0103`。
case!(
    x20_b_a_member_without_a_callback_carries_the_state_to_its_result,
    r#"LET(a, [ERR("boom")]);
PRINT(a);
0"#,
    "!E0103 unhandled ERR was discarded: NULL",
    1
);

// 同一件事的**标准库成员**形态(`IMPORT` 之后走 `std.io` 的成员,不是内建
// fallback)⇒ 两条路**同款**同报。
//
// ⚠ 这条钉住的是「两条路都要接」—— 内建 fallback 与 `std.io` 成员分叉就会
// 出现「一条路报、一条路不报」。
case!(
    x20_b2_the_std_member_path_reports_the_same_way,
    r#"IMPORT("wlwl:std.io", ["PRINT"]);
LET(a, [ERR("boom")]);
PRINT(a);
0"#,
    "!E0103 unhandled ERR was discarded: NULL",
    1
);

// ── 红线:同一个 `ERR` **至多**一条 `E0103`(§8.6.5) ──────────────────────

// 被调方**返回**那个实参 ⇒ 未消费状态随结果**转给调用方** ⇒ 由调用方持有。
//
// ⚠ **这条是本项修复最硬的红线**:内建 `CALL` 的帧若把自己那份集合**留在手上**
// (复制而不是取走),结果承接会把它**再挂回结果** ⇒ 同一个 `ERR` 在同一个
// 报告点上被算**两遍** ⇒ **两条** `E0103` ⇒ 违 §8.6.5。
//
// ⚠ 只断言「第一条消息」**看不见**这个缺陷(两条消息一模一样)⇒ 必须数条数。
case!(
    x20_c_the_same_err_is_reported_at_most_once,
    r#"LET(f, FUN((x), x));
LET(r, CALL(f, [ERR("boom")]));
0"#,
    "!E0103 unhandled ERR was discarded: [ERR(boom)]",
    1
);

// 数组里同时装**两个**未消费 `ERR` ⇒ 成员把集合**按元素**分发给回调 ⇒ 每个
// `ERR` 归**它自己那个**回调的形参绑定 ⇒ 第一个回调的作用域结束即报 ⇒
// 求值在那里中止 ⇒ 看到**一条**。
//
// ⚠ 这条钉住「**按元素分发**」而不是「挑一个承载者」:旧口径在候选不唯一时
// **一个都不转** ⇒ 那两个 `ERR` 从来没进过任何回调 ⇒ 回调处置了它们之后它们
// 仍被报 ⇒ **假阳性**。
case!(
    x20_d_two_errs_each_go_to_their_own_callback,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(r, MAP([ERR("a"), ERR("b")], FUN((x), 0)));
0"#,
    "!E0103 unhandled ERR was discarded: a",
    1
);

// 同一形状,但两个回调都**显式丢弃**自己的元素 ⇒ 两个 `ERR` 都被**处置**了
// ⇒ **一条都不报**。
//
// ⚠ **这是旧口径的假阳性闸**:旧口径一个都不分发 ⇒ 那两个 `ERR` 留在「成员
// 手上」⇒ 结果携带 ⇒ 在下一个丢弃点被报 ⇒ 合规程序被判成不合规。
case!(
    x20_e_two_errs_each_disposed_in_their_own_callback_are_clean,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(r, MAP([ERR("a"), ERR("b")], FUN((x), (LET(_, x); 0))));
0"#,
    "",
    0
);

// ── 反向守卫:合规程序不许被误判(假阳性) ───────────────────────────────

// 回调**消费**了那个 `ERR` ⇒ 不在未消费状态 ⇒ **不**报。
//
// ⚠ 这条是本项修复的**假阳性闸**:状态若被转给**成员的结果**(而不是回调的
// 形参绑定),成员无从知道回调已经消费了它 ⇒ 合规程序会被判成不合规。
case!(
    x20_f_a_callback_that_consumes_the_err_is_clean,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(r, MAP([ERR("boom")], FUN((x), ERR_PAYLOAD(x))));
0"#,
    "",
    0
);

// 回调**显式丢弃**那个值(§8.6.4)⇒ 不报。
case!(
    x20_g_a_callback_that_explicitly_discards_is_clean,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(r, MAP([ERR("boom")], FUN((x), (LET(_, x); 0))));
0"#,
    "",
    0
);

// ── 既有口径不得改变 ───────────────────────────────────────────────────

// `ERR` 嵌在**容器**里(不是元素本身)⇒ 被处置的是那个容器 ⇒ 消息是
// `[ERR(boom)]` —— 与 x07 的冻结期望**逐字一致**(两处形制的口径不许分叉)。
case!(
    x20_h_a_nested_container_is_disposed_as_a_container,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(r, MAP([[ERR("boom")]], FUN((x), 0)));
0"#,
    "!E0103 unhandled ERR was discarded: [ERR(boom)]",
    1
);

// 直接调用**用户函数**(不经成员)⇒ 整个数组进形参绑定 ⇒ 被处置的是容器 ⇒
// 与「走成员」**不同形制**,但各自都是「被处置的那个值」(§8.6.3)。
//
// ⚠ 这条钉住「成员边界**没有**把这条也改了」—— 两条路的形制不同是对的。
case!(
    x20_i_the_direct_user_function_path_still_reports_the_container_form,
    r#"LET(f, FUN((x), 0));
LET(r, f([ERR("boom")]));
0"#,
    "!E0103 unhandled ERR was discarded: [ERR(boom)]",
    1
);
