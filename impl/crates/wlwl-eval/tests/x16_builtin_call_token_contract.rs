//! **X-16** 差异登记项的契约测试 —— v1.0 规范 §8.6.3「内建 `CALL(f, …)` 的
//! 实参通道必须携带令牌」。
//!
//! 规范基线:`Laffinty/wlwl-spec-v1` 一致性册 §5.4 的 **X-16** 行;语言核心册
//! §8.6.1~§8.6.5。
//!
//! ## 这条差异登记项说的是什么
//!
//! §8.6.3:实参的未消费状态**转给形参绑定**。走**用户函数**时这条链是通的
//! (`eval_call` 算出实参集合 → `invoke_closure` 落到形参绑定 → 被调方作用域
//! 结束时按 §8.6.2 第 3 项报 `E0103`)。
//!
//! 走**内建 `CALL`**时这条链此前**断在内建边界上**:内建函数的签名是
//! `fn(&mut Evaluator, Vec<Value>)` ⇒ **没有**传未消费集合的参数位 ⇒ 集合在
//! 进入内建时被丢掉 ⇒ 被调方的形参绑定拿不到 ⇒ **不报** `E0103`(假阴性)。
//!
//! ⚠ 与 **X-07**(标准库成员的实参通道)是**两条不同的通道**:X-07 修的是
//! 「标准库成员回调用户函数」,X-16 修的是「语言内建 `CALL` 调任意可调用值」。
//!
//! ## 修完之后的行为(本文件钉的就是这个)
//!
//! `CALL(f, [ERR("boom")])` —— 数组里的 `ERR` 没被被调方消费 ⇒ 被调方自己的
//! 作用域结束时它是丢弃点 ⇒ 报 `E0103`,被处置的是**容器** ⇒ 消息是
//! `[ERR(boom)]`。
//!
//! ⚠ **与走标准库成员时的形态不同,这是对的**:走标准库成员时回调收到的是
//! **元素** ⇒ 被处置的是那个 `ERR` 本身 ⇒ 消息是 `boom`。两处形态**不统一**,
//! 但各自的形态都是「被处置的那个值」(§8.6.3「容器本身在丢弃点被处置」)。
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
                "case `{}` (§8.6.3 / X-16)",
                stringify!($name)
            );
        }
    };
}

// ── 缺口本体:内建 `CALL` 的实参带着未消费的 `ERR` ──────────────────────────

// 容器实参里的 `ERR` 没被被调方消费 ⇒ 被调方作用域结束时是丢弃点 ⇒ `E0103`。
case!(
    x16_builtin_call_with_a_container_holding_an_unconsumed_err_reports_e0103,
    r#"LET(f, FUN((x), 0));
LET(r, CALL(f, [ERR("boom")]));
0"#,
    "!E0103 unhandled ERR was discarded: [ERR(boom)]"
);

// 同一件事的**绑定**形态:未消费状态在**绑定**上,读它时转进这次调用。
case!(
    x16_builtin_call_with_a_binding_holding_an_unconsumed_err_reports_e0103,
    r#"LET(a, [ERR("boom")]);
LET(f, FUN((x), 0));
LET(r, CALL(f, a));
0"#,
    "!E0103 unhandled ERR was discarded: [ERR(boom)]"
);

// **逐参对齐**:两个实参,只有**第二个**持有未消费状态 ⇒ 集合必须落在**第二个**
// 形参上(不是第一个)。
//
// ⚠ 这条是「跳过第 0 个实参(被调者)」与「逐参数对齐」两处的**对齐闸**:
// 偏移错一位 ⇒ 集合落错形参 ⇒ 消息形态随之变 ⇒ 本条变红。
case!(
    x16_the_unconsumed_set_lands_on_the_matching_parameter,
    r#"LET(f, FUN((a, b), 0));
LET(r, CALL(f, 1, [ERR("boom")]));
0"#,
    "!E0103 unhandled ERR was discarded: [ERR(boom)]"
);

// 内建 `CALL` 调**标准库成员**(`NativeFn`)⇒ 成员那侧走回调通道 ⇒ 被调方
// 收到的是**元素** ⇒ 被处置的是那个 `ERR` 本身 ⇒ 消息是 `boom`。
case!(
    x16_builtin_call_to_a_native_member_reports_the_element_form,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(r, CALL(MAP, [ERR("boom")], FUN((x), 0)));
0"#,
    "!E0103 unhandled ERR was discarded: boom"
);

// ── 反向守卫:合规程序不许被误判(假阳性) ───────────────────────────────

// 被调方**消费**了那个 `ERR` ⇒ 不在未消费状态 ⇒ **不**报。
//
// ⚠ 这条是本项修复的**假阳性闸**:集合若被转给**被调方的结果**(而不是它的
// 形参绑定),被调方无从知道自己已经消费了它 ⇒ 合规程序会被判成不合规。
case!(
    x16_a_callback_that_consumes_the_err_is_clean,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(f, FUN((x), MAP(x, FUN((e), ERR_PAYLOAD(e)))));
LET(r, CALL(f, [ERR("boom")]));
0"#,
    "0"
);

// 被调方**显式丢弃**那个值(§8.6.4)⇒ 不报。
case!(
    x16_an_explicit_discard_inside_the_callee_is_clean,
    r#"LET(f, FUN((x), (LET(_, x); 0)));
LET(r, CALL(f, [ERR("boom")]));
0"#,
    "0"
);

// 实参里没有未消费的 `ERR` ⇒ 一路干净。
case!(
    x16_a_plain_argument_through_builtin_call_is_clean,
    r#"LET(f, FUN((x), 0));
LET(r, CALL(f, 5));
0"#,
    "0"
);

// ── 既有行为不得改变 ───────────────────────────────────────────────────

// 直接调用**用户函数**(不经 `CALL`)⇒ 整个数组进形参绑定 ⇒ 被处置的是容器。
// 这条与本项修复**无关**(修之前就报)⇒ 用来钉住「没有把既有行为改坏」。
case!(
    x16_the_direct_call_path_is_unchanged,
    r#"LET(f, FUN((x), 0));
LET(r, f([ERR("boom")]));
0"#,
    "!E0103 unhandled ERR was discarded: [ERR(boom)]"
);

// 被调方**返回**那个实参 ⇒ 未消费状态随之**转给调用方** ⇒ 由调用方持有 ⇒
// 在调用方的作用域结束时处置 ⇒ 报。
case!(
    x16_the_state_transfers_to_the_caller_when_the_callee_returns_the_argument,
    r#"LET(f, FUN((x), x));
LET(r, CALL(f, [ERR("boom")]));
0"#,
    "!E0103 unhandled ERR was discarded: [ERR(boom)]"
);
