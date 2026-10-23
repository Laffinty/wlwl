//! **X-19** 差异登记项的契约测试 —— v1.0 规范 §8.6.3「内建 `NEW(C, …)` 的
//! 实参通道必须携带令牌」。
//!
//! 规范基线:`Laffinty/wlwl-spec-v1` 一致性册 §5.4 的 **X-19** 行;语言核心册
//! §8.6.1~§8.6.5;对象册 §13.3(`NEW`)。
//!
//! ## 这条差异登记项说的是什么
//!
//! §8.6.3:实参的未消费状态**转给形参绑定**。走**用户函数**时这条链是通的;
//! 走**内建 `NEW`**时这条链此前**断在内建边界上**:内建函数的签名是
//! `fn(&mut Evaluator, Vec<Value>)` ⇒ **没有**传未消费集合的参数位 ⇒ 集合在
//! 进入内建时被丢掉 ⇒ **初始化器**的形参绑定拿不到 ⇒ **不报** `E0103`
//! (假阴性)。
//!
//! ⚠ 与 **X-16**(`CALL`)、**X-18**(`CALL_METHOD`)是**同根同形**的三条通道。
//!
//! ## 修完之后的行为(本文件钉的就是这个)
//!
//! `NEW(C, [ERR("boom")])` —— 数组里的 `ERR` 没被初始化器消费 ⇒ 初始化器自己
//! 的作用域结束时它是丢弃点 ⇒ 报 `E0103`,被处置的是**容器** ⇒ 消息是
//! `[ERR(boom)]`。
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
                "case `{}` (§8.6.3 / X-19)",
                stringify!($name)
            );
        }
    };
}

// ── 缺口本体:内建 `NEW` 的实参带着未消费的 `ERR` ───────────────────────────

// 容器实参里的 `ERR` 没被初始化器消费 ⇒ 初始化器作用域结束时是丢弃点 ⇒ `E0103`。
case!(
    x19_new_with_a_container_holding_an_unconsumed_err_reports_e0103,
    r#"LET(C, CLASS("C", NULL, [["init", FUN((self, x), 0)]]));
LET(o, NEW(C, [ERR("boom")]));
0"#,
    "!E0103 unhandled ERR was discarded: [ERR(boom)]"
);

// 同一件事的**绑定**形态:未消费状态在**绑定**上,读它时转进这次调用。
case!(
    x19_new_with_a_binding_holding_an_unconsumed_err_reports_e0103,
    r#"LET(a, [ERR("boom")]);
LET(C, CLASS("C", NULL, [["init", FUN((self, x), 0)]]));
LET(o, NEW(C, a));
0"#,
    "!E0103 unhandled ERR was discarded: [ERR(boom)]"
);

// **逐参对齐**:两个实参,只有**第二个**持有未消费状态 ⇒ 集合必须落在**第二个**
// 形参上(不是第一个)。
//
// ⚠ 这条是「跳过第 0 个实参(类本身)」与「逐参数对齐」两处的**对齐闸**:
// 偏移错一位 ⇒ 集合落错形参 ⇒ 消息形态随之变 ⇒ 本条变红。
case!(
    x19_the_unconsumed_set_lands_on_the_matching_parameter,
    r#"LET(C, CLASS("C", NULL, [["init", FUN((self, a, b), 0)]]));
LET(o, NEW(C, 1, [ERR("boom")]));
0"#,
    "!E0103 unhandled ERR was discarded: [ERR(boom)]"
);

// ── 反向守卫:合规程序不许被误判(假阳性) ───────────────────────────────

// 初始化器**消费**了那个 `ERR` ⇒ 不在未消费状态 ⇒ **不**报。
//
// ⚠ 这条是本项修复的**假阳性闸**:集合若被转给**实例的结果**(而不是初始化器
// 的形参绑定),初始化器无从知道自己已经消费了它 ⇒ 合规程序会被判成不合规。
case!(
    x19_an_init_that_consumes_the_err_is_clean,
    r#"IMPORT("wlwl:std.collection", ["MAP"]);
LET(C, CLASS("C", NULL, [["init", FUN((self, x), MAP(x, FUN((e), ERR_PAYLOAD(e))))]]));
LET(o, NEW(C, [ERR("boom")]));
0"#,
    "0"
);

// 初始化器**显式丢弃**那个值(§8.6.4)⇒ 不报。
case!(
    x19_an_init_that_explicitly_discards_is_clean,
    r#"LET(C, CLASS("C", NULL, [["init", FUN((self, x), (LET(_, x); 0))]]));
LET(o, NEW(C, [ERR("boom")]));
0"#,
    "0"
);

// 实参里没有未消费的 `ERR` ⇒ 一路干净。
case!(
    x19_new_without_err_arguments_is_clean,
    r#"LET(C, CLASS("C", NULL, [["init", FUN((self, x), 0)]]));
LET(o, NEW(C, 5));
0"#,
    "0"
);
