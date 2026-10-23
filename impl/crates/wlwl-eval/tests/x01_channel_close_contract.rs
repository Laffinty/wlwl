//! **X-01** 差异登记项的契约测试 —— v1.0 并发册 §17.2.3「关闭协议」+ 附录
//! H.3.7「关闭」;语言核心册 §11.2 的 `E0054`(通道已关闭)。
//!
//! 规范基线:`Laffinty/wlwl-spec-v1` 一致性册 §5.4 的 **X-01** 行;并发册
//! §17.2.3 / §17.7 / 附录 H.3.7。
//!
//! ## 这条差异登记项说的是什么
//!
//! 并发册 §17.2.3:「关闭**必须**释放该通道上全部挂起的等待方」——
//!
//! * 挂起的**接收方**被唤醒后,其 `RECV` 按「关闭且读空」完成,返回
//!   `ERR(kind="ChannelClosed")`;
//! * 挂起的**发送方**被唤醒后,其 `SEND` 以 **`E0054`** 失败(发送到已关闭通道)。
//!
//! ⇒ 此前实现**只**释放接收方 ⇒ 停泊的发送方永远不被唤醒 ⇒ 对它 `AWAIT` 得到
//! **`E0053`**(无对端唤醒)⇒ **不**是本节要的 `E0054`。
//!
//! 附录 H.3.7 给的参考转移是同一句:「停泊的发送方全部释放:其 SEND 以 E0054 失败」。
//!
//! ## 形状
//!
//! 一个用例 = 一个 `#[test]`(同 `x20_member_boundary_contract.rs` 的形状)。
//!
//! ⚠ **每条用例都断言诊断的条数** —— 「同一个失败只报一次」是这条修复的一部分:
//! 关闭是**幂等**的 ⇒ 第二次关闭**不得**把同一个发送方再释放一次 ⇒ 诊断条数
//! 恒为 1。

use wlwl_eval::Evaluator;

/// 一次求值的结果。
struct Outcome {
    /// 成功 ⇒ 值的 `display()`;失败 ⇒ 首条诊断的「码 消息」。
    value: String,
    /// 失败 ⇒ 全部诊断的「码 消息」(按报告顺序);成功 ⇒ 空。
    diagnostics: Vec<String>,
}

fn outcome(src: &str) -> Outcome {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new();
    match ev.eval(&ast) {
        Ok(v) => Outcome {
            value: v.display(),
            diagnostics: Vec::new(),
        },
        Err(e) => {
            let diagnostics: Vec<String> = e
                .diagnostics()
                .iter()
                .map(|d| {
                    if d.message.is_empty() {
                        d.code.as_str().to_string()
                    } else {
                        format!("{} {}", d.code.as_str(), d.message)
                    }
                })
                .collect();
            let value = diagnostics.first().cloned().unwrap_or_default();
            Outcome { value, diagnostics }
        }
    }
}

// 一条用例 = 一个 `#[test]`。
macro_rules! case {
    ($name:ident, $src:expr, $expect_value:expr, $expect_diags:expr) => {
        #[test]
        fn $name() {
            let o = outcome($src);
            assert_eq!(
                o.value,
                $expect_value,
                "case `{}` (§17.2.3 / X-01) 的值 / 首条诊断",
                stringify!($name)
            );
            assert_eq!(
                o.diagnostics,
                $expect_diags,
                "case `{}` (§17.2.3 / X-01) 的全部诊断(条数与顺序)",
                stringify!($name)
            );
        }
    };
}

// ── 缺口本体:停泊的**发送方**在关闭后被唤醒 ⇒ 其 `SEND` 以 `E0054` 失败 ───

// 缓冲先填满 ⇒ `CHANNEL_SEND` 停泊 ⇒ 另一个任务关闭通道 ⇒ 发送方被释放 ⇒
// 它重跑段首、再次 `SEND` ⇒ 通道已关闭 ⇒ **E0054**。
//
// ⚠ **不是 `E0053`**:`E0053` 是「无对端唤醒」(发送方永远停泊)——那正是修之前的
// 读数。⇒ 这条用例的诊断**码**就是本项的判据。
case!(
    x01_a_parked_sender_is_released_and_fails_with_e0054,
    r#"LET(out,
    SCOPE(FUN(() ,
        LET(ch, CHANNEL_NEW(1));
        CHANNEL_TRY_SEND(ch, 1);
        LET(h_send, SPAWN(FUN(() , CHANNEL_SEND(ch, 2))));
        LET(h_close, SPAWN(FUN(() , CHANNEL_CLOSE(ch))));
        LET(_c, AWAIT(h_close));
        LET(v, AWAIT(h_send));
        v
    ))
);
out"#,
    "E0054 CHANNEL_SEND on a closed channel",
    vec!["E0054 CHANNEL_SEND on a closed channel".to_string()]
);

// ── 既有行为不得改变:停泊的**接收方**仍按「关闭且读空」完成 ─────────────

// 接收方被唤醒后,其 `RECV` 返回 `ERR(kind="ChannelClosed")` ⇒ `IS_ERR` 为 TRUE。
//
// ⚠ [v1.0 spec §8.6] `IS_ERR` 是**观察型**消费者 ⇒ 那个 `ERR` 仍未消费 ⇒ 作用域
// 结束时是丢弃点(§8.6.2 第 3 项)⇒ `E0103` ⇒ 故显式丢弃(§8.6.4)。
case!(
    x01_b_parked_receiver_still_gets_channelclosed,
    r#"LET(out,
    SCOPE(FUN(() ,
        LET(ch, CHANNEL_NEW(0));
        LET(h_recv, SPAWN(FUN(() , CHANNEL_RECV(ch))));
        LET(h_close, SPAWN(FUN(() , CHANNEL_CLOSE(ch))));
        LET(_c, AWAIT(h_close));
        LET(v, AWAIT(h_recv));
        LET(observed, IS_ERR(v));
        LET(_, v);
        observed
    ))
);
out"#,
    "TRUE",
    Vec::<String>::new()
);

// ── **两个**停泊的发送方 ⇒ 关闭**必须**释放**全部**(不是只释放一个) ────────

// 缓冲先填满 ⇒ **两个**发送方都停泊 ⇒ 关闭 ⇒ **两个**都必须被释放。
//
// ⚠ 这条是「释放**全部**」的判据:实现里另有**一条部分释放**的路径 ——
// `CHANNEL_RECV` 在**关闭后**读到 `Closed` 时会 `pop_sender_waiter` 唤醒**一个**
// 停泊的发送方(§8.6 那条「关掉后发送方看到 E0054」)。⇒ 若把本例的第二个发送方
// 交给那条路径,**它也会被释放** ⇒ 本例就**测不出**关闭本身释放了几个 ⇒
// ⇒ 所以本例**不放任何接收方** ⇒ 唯一的释放来源就是**关闭** ⇒ 且第二个发送方
// 必须在 `AWAIT` 里被观察到 ⇒ 只释放一个的话这里会读到 `E0053`(仍停泊)。
case!(
    x01_c_close_releases_every_parked_sender_not_just_one,
    r#"LET(out,
    SCOPE(FUN(() ,
        LET(ch, CHANNEL_NEW(1));
        CHANNEL_TRY_SEND(ch, 1);
        LET(h_s1, SPAWN(FUN(() , CHANNEL_SEND(ch, 2))));
        LET(h_s2, SPAWN(FUN(() , CHANNEL_SEND(ch, 3))));
        LET(h_close, SPAWN(FUN(() , CHANNEL_CLOSE(ch))));
        LET(_c, AWAIT(h_close));
        LET(v, AWAIT(h_s2));
        v
    ))
);
out"#,
    "E0054 CHANNEL_SEND on a closed channel",
    vec!["E0054 CHANNEL_SEND on a closed channel".to_string()]
);

// ── 关闭**幂等** ⇒ 不得把同一个发送方再释放一次 ──────────────────────────

// 连关两次 ⇒ 第一次释放发送方 ⇒ 第二次是 no-op ⇒ **恰好一条** `E0054`。
//
// ⚠ 这条钉的是「重复报告」那一侧:若第二次关闭又把发送方释放一次 ⇒ 它会再跑一次
// ⇒ 再报一条 ⇒ **两条** `E0054`。
case!(
    x01_d_close_is_idempotent_and_does_not_release_twice,
    r#"LET(out,
    SCOPE(FUN(() ,
        LET(ch, CHANNEL_NEW(1));
        CHANNEL_TRY_SEND(ch, 1);
        LET(h_send, SPAWN(FUN(() , CHANNEL_SEND(ch, 2))));
        LET(h_close, SPAWN(FUN(() ,
            CHANNEL_CLOSE(ch);
            CHANNEL_CLOSE(ch)
        )));
        LET(_c, AWAIT(h_close));
        LET(v, AWAIT(h_send));
        v
    ))
);
out"#,
    "E0054 CHANNEL_SEND on a closed channel",
    vec!["E0054 CHANNEL_SEND on a closed channel".to_string()]
);

// ── 反向守卫:合规程序不许被误判(假阳性) ───────────────────────────────

// 通道上**没有**停泊方时关闭 ⇒ 不产生任何诊断。
case!(
    x01_e_closing_a_channel_with_no_waiters_is_clean,
    r#"LET(ch, CHANNEL_NEW(2));
CHANNEL_TRY_SEND(ch, 1);
CHANNEL_CLOSE(ch);
0"#,
    "0",
    Vec::<String>::new()
);

// 作用域退出时的**强制关闭**同样释放停泊的发送方 ⇒ 那个发送方的 `SEND` 以
// `E0054` 失败 ⇒ 但它是**分离**任务 ⇒ 失败**不**外泄到主程序 ⇒ 程序照常结束。
//
// ⚠ 这条钉的是「释放了、但别把主程序带崩」——强制关闭是实现的**资源回收**手段
// (不是用户写的 `CHANNEL_CLOSE`)⇒ 它的失败属于那条分离任务自己的事。
case!(
    x01_f_scope_exit_force_close_releases_the_sender_without_leaking,
    r#"LET(out,
    SCOPE(FUN(() ,
        LET(ch, CHANNEL_NEW(1));
        CHANNEL_TRY_SEND(ch, 1);
        SPAWN(FUN(() , CHANNEL_SEND(ch, 2)));
        0
    ))
);
out"#,
    "0",
    Vec::<String>::new()
);
