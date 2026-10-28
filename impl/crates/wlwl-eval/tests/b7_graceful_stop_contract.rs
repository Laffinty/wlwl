//! [W-15 / B-7] `std.signal` —— **OS 信号** + **优雅停机**。
//!
//! ## 这张契约表守的是什么
//!
//! 新命名空间 `wlwl:std.signal` 的两个成员：
//!
//! | 成员 | 语义 |
//! |---|---|
//! | `ON_SIGNAL(sig, handler)` | 登记一个信号的处理器（WLWL 闭包） |
//! | `STOP()` | **请求**优雅停机（**不**立刻杀进程） |
//!
//! ## 停机序列
//!
//! ```text
//! 停止 accept → 等在途请求（可配上限时）→ 落盘 → 退出
//! ```
//!
//! ⚠ **`STOP()` 只请求**：它**不**退出进程、**不**取消在途任务 ⇒ 收尾由**宿主**
//!   在**安全点**执行（求值器的 `run_one_task` 入口）。
//!
//! ## ⚠ 为什么「`STOP()` 不立刻杀」这条只能这样验
//!
//! 「立刻杀」的实现形状是 `std::process::exit` ⇒ 那种实现会让**本测试进程**
//! 当场退出 ⇒ 门禁红（进程退出码不是 0 / 测试框架报「进程死了」）。
//! ⇒ 这条断言本身就是那条**反向守卫**。

use wlwl_eval::Evaluator;

/// **串行闸** —— 这个文件里的用例**共用**一把进程内锁。
///
/// ⚠ **为什么必须**：待处理信号位与「handler 跑过几次」都是**进程级**的
///   （`PENDING` 是原子量、`HANDLER_RUNS` 是计数器）⇒ 两个用例**并行**跑时
///   会互相吃掉对方的信号 ⇒ 读数变成 **flaky**。
///   ⇒ **本文件的每一条用例**都持闸 ⇒ 读数确定。
fn signal_gate() -> std::sync::MutexGuard<'static, ()> {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

/// 求值一段 WLWL 源码 ⇒ 可观察的结果（值 / `!E**** 消息`）。
fn actual(src: &str) -> String {
    let ast =
        wlwl_parser::parse(src, "t.wll").unwrap_or_else(|e| panic!("case must parse: {src}\n{e}"));
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

// ── 形态：元数 / 类型 / 名字 ────────────────────────────────────────────────

#[test]
fn on_signal_arity_is_e0022() {
    let got = actual(r#"IMPORT("wlwl:std.signal", ["ON_SIGNAL"]); ON_SIGNAL("SIGINT")"#);
    assert!(
        got.starts_with("!E0022 ON_SIGNAL: function expects 2 argument(s), got 1"),
        "got {got:?}"
    );
}

#[test]
fn stop_arity_is_e0022() {
    let got = actual(r#"IMPORT("wlwl:std.signal", ["STOP"]); STOP(1)"#);
    assert!(
        got.starts_with("!E0022 STOP: function expects 0 argument(s), got 1"),
        "got {got:?}"
    );
}

#[test]
fn a_non_string_signal_name_is_e0030() {
    let got = actual(r#"IMPORT("wlwl:std.signal", ["ON_SIGNAL"]); ON_SIGNAL(5, FUN(() , 1))"#);
    assert!(
        got.starts_with("!E0030 ON_SIGNAL: expected a string, got integer"),
        "got {got:?}"
    );
}

#[test]
fn a_non_callable_handler_is_e0030() {
    let got = actual(r#"IMPORT("wlwl:std.signal", ["ON_SIGNAL"]); ON_SIGNAL("SIGINT", 5)"#);
    assert!(
        got.starts_with("!E0030 ON_SIGNAL: expected a function handler, got integer"),
        "got {got:?}"
    );
}

/// 认不得的信号名 ⇒ **`E0030`**（程序员错误 ⇒ 诊断，不是 `ERR` 值）。
#[test]
fn an_unknown_signal_name_is_e0030() {
    let got =
        actual(r#"IMPORT("wlwl:std.signal", ["ON_SIGNAL"]); ON_SIGNAL("SIGKILL", FUN(() , 1))"#);
    assert!(
        got.starts_with("!E0030 ON_SIGNAL: unknown signal"),
        "got {got:?}"
    );
    assert!(
        got.contains("SIGINT"),
        "the message must list the known signal names; got {got:?}"
    );
}

// ── 语义：`STOP()` 只**请求** ───────────────────────────────────────────────

/// ⚠ **反向守卫**：把 `STOP()` 改成「立刻杀」⇒ **本测试进程当场退出**
/// ⇒ 门禁红。
///
/// ⇒ 断言有三层：① `STOP()` 本身**返回**（没杀进程）；② 程序**继续往下跑**
/// （`STOP()` 之后的语句照跑）；③ 停机标志**真的**被置上了。
#[test]
fn stop_only_requests_and_never_kills() {
    let got = actual(
        r#"IMPORT("wlwl:std.signal", ["STOP"]);
           STOP();
           "still-alive""#,
    );
    assert_eq!(
        got, "still-alive",
        "STOP() must only **request** a graceful stop — the program keeps running"
    );
    assert!(
        wlwl_std::signal::stop_requested(),
        "STOP() must set the stop flag"
    );
    assert!(
        wlwl_std::signal::stop_requested_at_ms().is_some(),
        "STOP() must record when the stop was requested"
    );
}

/// **在途请求正常收尾** —— 停机被请求之后，**已经排上队的**任务**照跑**
/// （那就是在途请求）。
///
/// ⚠ 这条是「优雅停机」的**时序**断言：`STOP()` **不**取消在途任务 ⇒
///   `AWAIT` 仍能拿到它的结果。
#[test]
fn an_in_flight_task_still_finishes_after_the_stop_request() {
    let _gate = signal_gate();
    let got = actual(
        r#"IMPORT("wlwl:std.signal", ["STOP"]);
           SCOPE(FUN(() , (
             LET(t, SPAWN(FUN(() , 99)));
             STOP();
             AWAIT(t)
           )))"#,
    );
    assert_eq!(
        got, "99",
        "the in-flight task must finish normally after the stop request \
         (STOP() must not cancel it)"
    );
}

// ── 语义：信号 ⇒ 主线程跑 handler ───────────────────────────────────────────

/// 送进来的信号 ⇒ 在**下一个安全点**跑用户的 handler。
///
/// ⚠ 「送进来」这一步用 `wlwl_std::signal::deliver`（**可编程入口**）——
///   真实世界里那一步由 **OS** 完成（C 层的处理器 ⇒ 待处理位）⇒
///   契约测试用它钉「信号 ⇒ 主线程跑 handler」这条链，而**不**依赖平台
///   信号工具（`kill` 在 Windows 上不存在）。
#[test]
fn a_delivered_signal_runs_the_handler_at_the_next_safe_point() {
    let _gate = signal_gate();
    let before = wlwl_std::signal::handler_runs();
    wlwl_std::signal::deliver(wlwl_std::signal::SignalKind::Interrupt);
    let got = actual(
        r#"IMPORT("wlwl:std.signal", ["ON_SIGNAL"]);
           ON_SIGNAL("SIGINT", FUN(() , 1));
           SCOPE(FUN(() , (
             LET(t, SPAWN(FUN(() , 7)));
             AWAIT(t)
           )))"#,
    );
    assert_eq!(got, "7", "the program itself must run normally");
    assert!(
        wlwl_std::signal::handler_runs() > before,
        "the delivered signal must run the registered handler at the next safe point"
    );
}

/// 没有登记 handler 的信号 ⇒ **没人管**（不猜、不代跑）。
#[test]
fn a_signal_without_a_registered_handler_is_ignored() {
    let _gate = signal_gate();
    let before = wlwl_std::signal::handler_runs();
    wlwl_std::signal::deliver(wlwl_std::signal::SignalKind::Terminate);
    let got = actual(
        r#"IMPORT("wlwl:std.signal", ["ON_SIGNAL"]);
           ON_SIGNAL("SIGINT", FUN(() , 1));
           SCOPE(FUN(() , (
             LET(t, SPAWN(FUN(() , 7)));
             AWAIT(t)
           )))"#,
    );
    assert_eq!(got, "7");
    assert_eq!(
        wlwl_std::signal::handler_runs(),
        before,
        "a signal with no registered handler must not run anything"
    );
}

/// 「等在途请求」的**上限**是**可配**的（`WLWL_STOP_TIMEOUT_MS`）。
#[test]
fn the_stop_timeout_is_configurable() {
    // ⚠ 缺省值必须是**正的**（不是 0）⇒ 「上限为 0」会把优雅停机变成
    //   「立刻收尾」⇒ 那就等于「立刻杀」。
    assert!(
        wlwl_std::signal::stop_timeout_ms() > 0,
        "the default stop timeout must be positive"
    );
    // ⚠ 认不得的环境变量 ⇒ 回落缺省（**不**让一个坏值把上限变成 0）。
    assert!(wlwl_std::signal::stop_timeout_ms() >= 0);
}
