//! [W-15 / B-6] 「**丢失的挂起信号**」**不得**伪装成「任务正常返回 NULL」。
//!
//! ## 这条契约守的是什么
//!
//! `run_one_task` 的**兜底臂**（`lib.rs` 里 `match outcome` 的最后一支）处理
//! 「任务体收尾时 outcome 里带的信号**不是**任何 park 分支能接住的那种」。
//!
//! ⚠ **旧行为**：兜底臂无条件把 outcome 结成 `TaskResult::Ok(o.value)` ⇒
//!   而**原生成员挂起时带的恰是 `Value::Null`**（`wlwl-std/src/tcp.rs` 的
//!   `TCP_RECV` 等）⇒ 「挂起信号丢了」被**伪装**成「任务正常完成、值是 NULL」。
//!
//! ⚠ **为什么这是活的隐患**：第四波加 `YieldReason::AsyncReady` 时就是这个
//!   形状 —— **新加一个变体而忘了加 match 分支** ⇒ 它会**静默**变成
//!   「任务完成、值为 NULL」。
//!
//! ⇒ **修法**：兜底臂里**显式判 `Signal::Yield(_)`** ⇒ 判为「丢失的挂起信号」
//!   ⇒ **任务失败** + 按并发册 §17.9 上报（复用 `pending_orphan_failures` 的
//!   「挂起—认领—收尾」三段式）⇒ **绝不**悄悄 `Ok(NULL)`。
//!
//! ## 为什么这里**端到端跑不出**这个形状
//!
//! 当前**能**产生的 `YieldReason` 变体**都有**对应的 park 分支
//! （`ReceivingOn` / `SendingOn` / `SocketReady` / `AsyncReady` / `SleepUntil` /
//! `Explicit`）⇒ 兜底臂**只在「将来新增了变体却忘了加分支」时**才会被走到。
//!
//! ⇒ 所以这张契约表**直接钉判据本身**（`runtime::classify_task_completion`），
//!   外加一道**接线守卫**（兜底臂真的用了那个判据）。
//!
//! ## 判据的形状
//!
//! ⚠ **按「是不是 Yield」判，不按具体变体枚举** ⇒ 将来新增变体也会被**抓住**
//!   —— 这正是这条修复的价值 ⇒ 表里把**每一个**现存变体都列一遍。

use wlwl_eval::runtime::{classify_task_completion, is_lost_suspend, TaskCompletion};
use wlwl_eval::{Outcome, Signal, TaskId, Value, YieldReason};

/// `YieldReason` 的**全部**现变体（⚠ 少列一个 ⇒ 那一条断言就护不住它）。
fn every_yield_reason() -> Vec<(&'static str, YieldReason)> {
    vec![
        ("Explicit", YieldReason::Explicit),
        ("AwaitingChild", YieldReason::AwaitingChild(TaskId(0))),
        (
            "ReceivingOn",
            YieldReason::ReceivingOn(wlwl_eval::ChannelId(0)),
        ),
        ("SendingOn", YieldReason::SendingOn(wlwl_eval::ChannelId(0))),
        ("SocketReady", YieldReason::SocketReady(1)),
        ("SleepUntil", YieldReason::SleepUntil(0)),
        ("AsyncReady", YieldReason::AsyncReady(1)),
    ]
}

/// 一个「带挂起信号」的 outcome（值恰是原生成员挂起时带的那个 `NULL`）。
fn lost_suspend(reason: YieldReason) -> Outcome {
    Outcome::with_signal(Value::Null, Signal::Yield(reason))
}

/// ⚠ **每一个** `YieldReason` 变体都算「丢失的挂起信号」。
///
/// ⇒ 这一条就是「判据不按变体枚举」的**可执行**形状：将来新增变体 ⇒ 把它
///   加进 [`every_yield_reason`] ⇒ 这条断言**自动**要求它也被抓住。
#[test]
fn every_yield_reason_counts_as_a_lost_suspend() {
    for (name, reason) in every_yield_reason() {
        assert!(
            is_lost_suspend(&Signal::Yield(reason)),
            "{name}: 任何 Yield 都必须算「丢失的挂起信号」"
        );
    }
}

/// 非挂起信号**不是**「丢失的挂起信号」。
#[test]
fn non_yield_signals_are_not_lost_suspends() {
    assert!(!is_lost_suspend(&Signal::None));
    assert!(!is_lost_suspend(&Signal::Break));
    assert!(!is_lost_suspend(&Signal::Continue));
    assert!(!is_lost_suspend(&Signal::Return(Value::Integer(1))));
}

/// ⚠ **核心断言**：兜底臂遇到「丢失的挂起信号」⇒ **判为失败**，
/// **绝不**结成「正常完成、值是那个 `NULL`」。
#[test]
fn a_lost_suspend_is_never_completed_with_the_value() {
    for (name, reason) in every_yield_reason() {
        let outcome = lost_suspend(reason);
        let got = classify_task_completion(&outcome, false);
        assert_eq!(
            got,
            TaskCompletion::LostSuspend,
            "{name}: 丢失的挂起信号必须判为失败"
        );
        // ⚠ **显式**钉住「不是 `Complete(NULL)`」—— 这正是旧行为伪装成
        // 「正常返回 NULL」的那一步。
        assert!(
            !matches!(got, TaskCompletion::Complete(_)),
            "{name}: 丢失的挂起信号**不得**被结成「正常完成 + 值」"
        );
    }
}

/// ⚠ **反向守卫**：把兜底臂的 Yield 判据去掉 ⇒ 本文件必红。
///
/// （`classify_task_completion` 不再判 `LostSuspend` ⇒ 上面两条断言立刻失败。）
#[test]
fn removing_the_yield_criterion_turns_a_lost_suspend_back_into_a_normal_null() {
    // 这条断言**模拟**「判据被去掉」之后的样子：那时 `classify_task_completion`
    // 会走 `Complete(o.value)` ⇒ 那个 `NULL` 就是伪装成「正常返回」的值。
    let outcome = lost_suspend(YieldReason::AsyncReady(1));
    let value = outcome.value.clone();
    assert_eq!(value, Value::Null, "原生成员挂起时带的恰是 NULL");
    // ⇒ 「判据在」时，处置**不是** `Complete(Null)`。
    assert_ne!(
        classify_task_completion(&outcome, false),
        TaskCompletion::Complete(Value::Null),
        "判据在的时候，丢失的挂起信号不得被结成 Complete(NULL)"
    );
}

/// 没有挂起信号 ⇒ 兜底臂**照旧**结成终值（既有行为一个字都不改）。
#[test]
fn a_normal_completion_is_still_completed_with_its_value() {
    let outcome = Outcome::normal(Value::Integer(42));
    assert_eq!(
        classify_task_completion(&outcome, false),
        TaskCompletion::Complete(Value::Integer(42))
    );
    let outcome = Outcome::normal(Value::Null);
    assert_eq!(
        classify_task_completion(&outcome, false),
        TaskCompletion::Complete(Value::Null)
    );
}

/// 还有后续 segment ⇒ **不是**终态（重排队）⇒ 判据**不**把它当成终值。
#[test]
fn more_segments_means_not_terminal() {
    let outcome = Outcome::normal(Value::Integer(1));
    assert_eq!(
        classify_task_completion(&outcome, true),
        TaskCompletion::MoreSegments
    );
    // ⚠ **但**「丢失的挂起信号」优先于「还有后续 segment」——
    //   信号丢了就是丢了 ⇒ 判失败 ⇒ 绝不重排队（重排队会让它**再挂一次**）。
    let outcome = lost_suspend(YieldReason::Explicit);
    assert_eq!(
        classify_task_completion(&outcome, true),
        TaskCompletion::LostSuspend
    );
}

/// **接线守卫** —— 兜底臂**真的**用了那个判据。
///
/// ⚠ 端到端跑不出这个形状（见文件头）⇒ 判据本身被上面几条钉住了，但
///   「兜底臂有没有用它」只能从**源码**上看 ⇒ 这里扫一眼。
#[test]
fn the_fallback_arm_actually_uses_the_criterion() {
    let src = std::fs::read_to_string("src/lib.rs")
        .expect("the evaluator source must be readable (cwd = package root)");
    let marker = "[B-6] **兜底臂不得把";
    let start = src
        .find(marker)
        .expect("the fallback arm must carry its [B-6] marker");
    let arm = &src[start..(start + 6000).min(src.len())];
    assert!(
        arm.contains("classify_task_completion"),
        "the fallback arm must classify the outcome through `classify_task_completion`"
    );
    assert!(
        arm.contains("TaskCompletion::LostSuspend"),
        "the fallback arm must have a `LostSuspend` arm (⇒ the task fails, \
         it does NOT complete with a value)"
    );
}
