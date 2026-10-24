//! **X-24** 差异登记项的契约测试 —— 停泊在通道上的子任务**不得**活过词法 `SCOPE`。
//!
//! 规范基线:`Laffinty/wlwl-spec-v1` 一致性册 §5.4 的 **X-24** 行;并发册
//! §17.1(「子任务寿命**不**超过词法 `SCOPE`;`SCOPE` 返回前**必须**等待其
//! 子任务结束」)+ §17.2.3(关闭**必须**释放全部挂起等待方;停泊发送方被唤醒后
//! 其 `SEND` 以 `E0054` 失败)。
//!
//! ## 这条差异说的是什么
//!
//! `pop_scope` 此前只做「排干运行队列 + 强关**本作用域**的通道」:
//!
//! 1. 强关后只把停泊方 `enqueue`,**没有** `Suspended → Pending` ⇒
//!    `begin_run` 拒绝运行 ⇒ 停泊方**从未真正重跑**(§17.2.3 的
//!    E0054 / ChannelClosed **观察不到**);
//! 2. 强关后**不**排干 ⇒ 即便唤醒了,也在 `SCOPE` 返回**之后**才跑;
//! 3. 硬清理只覆盖「屏蔽块内 / 已被请求取消」⇒ 停在**外层作用域**通道上的
//!    子任务永远醒不过来 ⇒ **泄漏**。
//!
//! ⇒ `SCOPE` 在子任务仍存活时就返回了 ⇒ 违 §17.1 的**第二条**义务。
//!
//! ## 甲案(先放行、再兜底)—— 本表钉的口径
//!
//! | 步骤 | 动作 | 兑现 |
//! |---|---|---|
//! | 1 | 强关本作用域通道 + **真正唤醒**停泊方 | §17.2.3「释放」 |
//! | 2 | **再排干** ⇒ 停泊方在 `SCOPE` 返回前重跑完 | §17.2.3「E0054 / ChannelClosed」 |
//! | 3 | 硬清理**所有**仍非终态子任务(含外层通道停泊) | §17.1 |
//!
//! ⇒ §17.2.3 与 §17.1 **同时**成立,**不需要**新增规范豁免。
//!
//! ## 形状
//!
//! 一个用例 = 一个 `#[test]`(同 `x01_channel_close_contract.rs` 的形状)。
//!
//! ⚠ **每条用例都断言诊断的条数**。
//! ⚠ 断言一律钉**精确值**(含 `[a, b]` 整体字面量)。
//! ⚠ 判别性用例的「修前读数」写在各自注释里 —— 回退实现必红。

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
                "case `{}` (§17.1 / X-24) 的值 / 首条诊断",
                stringify!($name)
            );
            assert_eq!(
                o.diagnostics,
                $expect_diags,
                "case `{}` (§17.1 / X-24) 的全部诊断(条数与顺序)",
                stringify!($name)
            );
        }
    };
}

// ── 缺口本体 ①:本作用域通道上的停泊方 ⇒ 必须在 SCOPE 返回前收尾 ─────────

// 子任务先 `SET(n, n+1)` 再 `CHANNEL_RECV` 停泊;恢复是**段首/体重放** ⇒
// 若它在 `SCOPE` 返回**之前**真正重跑,`n` 会再加一次。
//
// ⚠ **判别性用例**:修前强关后只 `enqueue`、不唤醒 ⇒ 子任务仍 Suspended ⇒
// 从不重跑 ⇒ 读数 **`1`**;修后唤醒 + 排干 ⇒ 体重放 ⇒ 读数 **`2`**。
// ⚠ 断言看的是**计数**,不是「有没有打印」(§7.2 坑 19:没打印 ≠ 没醒)。
case!(
    x24_a_own_channel_parked_child_completes_before_scope_returns,
    r#"LET MUT(n, 0);
LET(out, SCOPE(FUN(() ,
    LET(ch, CHANNEL_NEW(0));
    LET(h, SPAWN(FUN(() ,
        SET(n, +(n, 1));
        CHANNEL_RECV(ch);
        0
    )));
    0
)));
LET(_, out);
n"#,
    "2",
    Vec::<String>::new()
);

// ── 缺口本体 ②:外层作用域通道上的停泊方 ⇒ 作用域退出时被终止 ───────────

// 通道**归外层** ⇒ 内层作用域无法强关它 ⇒ 子任务永远收不到唤醒。
// 内层 `SCOPE` 返回后对其 `AWAIT` ⇒ 修前仍是停泊 ⇒ **E0053**;
// 修后被硬清理 ⇒ `ERR(kind="Cancelled")` ⇒ 读数 **`"Cancelled"`**。
//
// ⚠ **判别性用例**:修前 `AWAIT` 报 `E0053`(无对端唤醒)⇒ 期望钉
// `"Cancelled"` 能区分两侧。
case!(
    x24_b_outer_channel_parked_child_is_terminated_at_inner_scope_exit,
    r#"LET(ch, CHANNEL_NEW(0));
LET(out, SCOPE(FUN(() ,
    SPAWN(FUN(() ,
        CHANNEL_RECV(ch);
        0
    ))
)));
LET(r, AWAIT(out));
AT_K(ERR_PAYLOAD(r), "kind", "?")"#,
    "Cancelled",
    Vec::<String>::new()
);

// ── 兜底 ③:被终止的子任务不把主程序带崩(无假阳性) ───────────────────────

// 与 `x01_f` 同一口径:分离任务的失败/终止属于它自己 ⇒ 程序照常拿到作用域值
// 且**无诊断**。⚠ 本条是**回归守卫**(两侧读数相同):防的是「硬清理把
// 主程序的值或诊断面弄脏」。
case!(
    x24_c_force_terminate_does_not_leak_into_the_parent_program,
    r#"LET(out, SCOPE(FUN(() ,
    LET(ch, CHANNEL_NEW(0));
    SPAWN(FUN(() ,
        CHANNEL_RECV(ch);
        0
    ));
    0
)));
out"#,
    "0",
    Vec::<String>::new()
);

// ── §17.2.3 不回归:停泊**发送方**在 SCOPE 返回前被唤醒 ⇒ E0054 ─────────

// 作用域退出强关通道后,停泊的发送方**必须**真正重跑并以 `E0054` 失败
// (并发册 §17.2.3)。把句柄带出作用域再 `AWAIT` ⇒ 修后重跑已完成 ⇒
// `AWAIT` 重抛 **E0054**;修前从未重跑 ⇒ `AWAIT` 得 **E0053**。
//
// ⚠ 与 `x01_f` 的分工:`x01_f` 断言「不外泄」;本条断言「确实重跑失败了」
// —— 甲案第 2 步(排干)的直接判据,防止把 §17.2.3 杀掉换 §17.1。
case!(
    x24_d_own_channel_parked_sender_fails_with_e0054_before_scope_returns,
    r#"LET(h, 0);
LET(out, SCOPE(FUN(() ,
    LET(ch, CHANNEL_NEW(1));
    CHANNEL_TRY_SEND(ch, 1);
    LET(t, SPAWN(FUN(() , CHANNEL_SEND(ch, 2))));
    SET(h, t);
    0
)));
LET(_, out);
LET(r, AWAIT(h));
r"#,
    "E0054 CHANNEL_SEND on a closed channel",
    vec!["E0054 CHANNEL_SEND on a closed channel".to_string()]
);
