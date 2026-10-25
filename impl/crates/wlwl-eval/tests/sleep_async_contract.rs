//! [G-08] `std.time::SLEEP_ASYNC` —— **可挂起**的睡眠的契约表。
//!
//! ## 它补的是什么
//!
//! 上游的 `SLEEP` 是**阻塞原语**（2026-10-06 裁决「方案 E」）：`SLEEP` 期间
//! **不调度任何其他任务** ⇒ 整个运行时停摆 ⇒ 服务端的空闲退避 / 优雅关闭
//! 写不出来。
//!
//! ## 三条硬约束（都是实测出来的，不是偏好）
//!
//! 1. **「整段重跑 ⇒ 再睡一次 ⇒ 无限循环」** —— 段模型没有真状态机恢复
//!    （ADR-0017 Step 3 未落地）⇒ 任务被唤醒后**整段重跑** ⇒ 睡眠成员**再被调一次**。
//!    上游自己的夹具 `M6_std_time_sleep` 记着这条：挂起路线下 **8 秒 79 次不终止**。
//!    ⇒ 本成员靠宿主接缝 `take_sleep_elapsed()` 认出「我是被唤醒后重跑的」⇒ 直接返回。
//! 2. **「已经睡过」必须按**调用点**记** —— ⚠ [v0.12.0 / Q8]
//!    段重跑落回**同一处** ⇒ 同一处 ⇒ 同一个身份 ⇒ 成员认得出「我这次睡过了」。
//!    ⇒ **一个任务体里可以睡多次**（各调用点各睡一次），**循环里每轮也都真的睡**。
//!    ⇒ 交付包原形态（「一个任务体至多一次睡眠 ⇒ 第二次报 `E0030`」）已删：
//!    那条失败口径**不在规范里**（标准库·系统册 §15.1 的失败列只有元数 / 类型 / 负值）
//!    ⇒ 凭空多出来的诊断。
//! 3. **重跑会重放副作用** —— 睡眠**之前**的语句会再执行一遍（与 socket / 通道等待
//!    同款约束：「挂起点必须是所在段的第一条语句」）。
//!
//! ## 时间断言为什么写成「区间」
//!
//! 时间相关的断言**不能**钉死精确值（机器负载一变就红 ⇒ 假门禁）⇒ 这里钉
//! 「量级」：睡眠确实**睡够了**（下界），且**别的任务能在这段时间里前进**。

use wlwl_eval::Evaluator;

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

#[test]
fn a_sleep_suspends_the_task_and_it_continues_afterwards() {
    // 任务体末尾睡 50 ms ⇒ 醒来后继续往下走 ⇒ 交出 42。
    let out = actual(
        r#"IMPORT("wlwl:std.time", ["SLEEP_ASYNC"]);
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , (SLEEP_ASYNC(50); 42))))))"#,
    );
    assert_eq!(out, "42", "the task must continue after the sleep ends");
}

#[test]
fn the_sleep_actually_waits() {
    // 下界断言：睡够了。写成「恰好 50」在负载下必红 ⇒ 那是假门禁。
    let out = actual(
        r#"IMPORT("wlwl:std.time", ["SLEEP_ASYNC", "MONOTONIC"]);
LET(t0, MONOTONIC());
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , (SLEEP_ASYNC(60); -(MONOTONIC(), t0)))))))"#,
    );
    let elapsed: i64 = out
        .parse()
        .unwrap_or_else(|_| panic!("want an integer, got {out}"));
    assert!(
        elapsed >= 50,
        "the sleep must wait at least ~its budget, waited {elapsed}ms"
    );
}

#[test]
fn other_tasks_keep_running_while_a_task_is_sleeping() {
    // 判据：**别的任务能在这段时间里前进** —— 这是「可挂起」与「阻塞」的分水岭。
    //
    // 形状：A 睡 80 ms；B 让出一次后立刻读钟 ⇒ B 的读数**远早于** A 醒来的时刻
    // ⇒ B 是在 A **挂着**的时候跑的。
    let out = actual(
        r#"IMPORT("wlwl:std.time", ["SLEEP_ASYNC", "MONOTONIC"]);
LET(r, SCOPE(FUN(() ,
    LET(a, SPAWN(FUN(() , (SLEEP_ASYNC(80); MONOTONIC()))));
    LET(b, SPAWN(FUN(() , (YIELD(); MONOTONIC()))));
    LET(bt, AWAIT(b));
    LET(at, AWAIT(a));
    ["bt": bt, "at": at]
)));
[AT_K(r, "bt", 0), AT_K(r, "at", 0)]"#,
    );
    assert!(out.starts_with('['), "want a 2-element array, got {out}");
    let nums: Vec<i64> = out
        .trim_matches(|c| c == '[' || c == ']')
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<i64>()
                .unwrap_or_else(|_| panic!("want ints, got {out}"))
        })
        .collect();
    assert_eq!(nums.len(), 2, "want two readings, got {out}");
    let (b_finished, a_woke) = (nums[0], nums[1]);
    assert!(
        a_woke - b_finished >= 50,
        "the other task must advance while this one sleeps (b={b_finished}, a={a_woke})"
    );
}

#[test]
fn two_sleeps_in_one_body_both_sleep_and_neither_is_a_diagnostic() {
    // ⚠ [v0.12.0 / Q8] **一个任务体里可以睡多次** —— 交付包的形态是「至多一次」，
    //   第二次直接报 `E0030`；可**那条失败口径不在规范里**
    //   （标准库·系统册 §15.1 的 `SLEEP_ASYNC` 失败列只有元数 / 类型 / 负值）
    //   ⇒ 凭空多出来的诊断 ⇒ 已删。
    //
    // ⇒ 记账的粒度改成**调用点**（源码位置）：段重跑落回**同一处** ⇒ 同一处 ⇒
    //   「我这次已经睡过了」认得出来 ⇒ 不会睡第二次 ⇒ 也不会无限循环。
    //
    // 判据：两个**不同**调用点 ⇒ **各睡一次** ⇒ 总耗时 ≈ 2 × 20 ms，
    //   且**不**报任何诊断。
    let src = r#"IMPORT("wlwl:std.time", ["SLEEP_ASYNC"]);
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() , (SLEEP_ASYNC(20); SLEEP_ASYNC(20); 0))))))"#;
    let start = std::time::Instant::now();
    let out = actual(src);
    let elapsed = start.elapsed().as_millis() as i64;
    assert_eq!(
        out, "0",
        "two sleeps in one body must both run and must not be a diagnostic, got {out}"
    );
    assert!(
        elapsed >= 30,
        "both sleeps must actually have waited (≈40ms), took {elapsed}ms"
    );
}

#[test]
fn a_sleep_in_a_loop_sleeps_every_iteration() {
    // ⚠ [v0.12.0 / Q8] **循环里的睡眠每轮都要真的睡** ——
    //   「按调用点记账」不能把**第二轮**也当成「已经睡过」（那会让循环飞快跑完）。
    // ⇒ 判据：3 轮 × 30 ms ⇒ 总耗时 ≳ 90 ms。
    //
    // ⚠ 用平铺语句而不是 `WHILE`：`WHILE` 里放 `YIELD` 是已知缺口 G-03
    //   （循环只跑一轮且无任何诊断）⇒ 这里用 `YIELD()` 把每轮隔成独立的段。
    let src = r#"IMPORT("wlwl:std.time", ["SLEEP_ASYNC"]);
SCOPE(FUN(() , AWAIT(SPAWN(FUN(() ,
    SLEEP_ASYNC(30);
    YIELD();
    SLEEP_ASYNC(30);
    YIELD();
    SLEEP_ASYNC(30);
    0
)))))"#;
    let start = std::time::Instant::now();
    let out = actual(src);
    let elapsed = start.elapsed().as_millis() as i64;
    assert_eq!(out, "0", "the loop body must finish, got {out}");
    assert!(
        elapsed >= 80,
        "every iteration must actually sleep (3 × 30ms), took {elapsed}ms"
    );
}

#[test]
fn a_negative_duration_is_a_programmer_error() {
    let out = actual(
        r#"IMPORT("wlwl:std.time", ["SLEEP_ASYNC"]);
SLEEP_ASYNC(-1)"#,
    );
    assert!(
        out.starts_with("!E0030"),
        "a negative duration must stay a programmer error, got {out}"
    );
}

#[test]
fn inside_a_clock_bubble_it_behaves_like_sleep() {
    // 气泡内：虚拟钟**只在 `SLEEP` 时前进** ⇒ 挂起就没有任何代码去推进它
    // ⇒ 死等 ⇒ 气泡内**等同 `SLEEP`**（拨快虚拟钟并立即返回）。
    let out = actual(
        r#"IMPORT("wlwl:std.time", ["SLEEP_ASYNC", "TEST_BUBBLE", "MONOTONIC"]);
TEST_BUBBLE(FUN(() , (SLEEP_ASYNC(3600000); MONOTONIC())))"#,
    );
    assert_eq!(
        out, "3600000",
        "inside a bubble the virtual clock must jump by the budget (no suspension)"
    );
}
