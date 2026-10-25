//! [2026-10-25] 第三方测试报告核实后落地的三条**机制性守卫**。
//!
//! 背景：`docs/plan/wlwl-v0.12.0-rc1-audit-and-fixes.md` §3。
//!
//! ⚠ **为什么这三条是「源码顺序守卫」而不是行为用例**：
//!
//! - **N1（数据 vs 超时）**：要复现「标志说超时了、但货已经��缓冲里」，
//!   需要**截止时间先到、而就绪通知后到**。就绪等待改成**事件驱动**
//!   （见 N3）之后，那条竞态窗口已经**关不上了** ⇒ 行为层**构造不出来**。
//!   ⇒ 于是守「**顺序**」这个真正会被重构破坏的性质：
//!   「缓冲里有没有货」必须**先于**「有没有超时」被回答。
//! - **N3（就绪等待必须事件驱动）**：改之前的等待延迟是**双峰**的
//!   （数据恰好落在第一次轮询前 ⇒ 0 ms；落在轮询后 ⇒ ≈50 ms）⇒
//!   任何「延迟必须小于 X ms」的行为断言都会**偶发红** ⇒ 同样守机制。
//!
//! 三条守卫都是**确定性**的（读源码、比对顺序与符号），符合本仓
//! 「CI 对 flaky 是『重跑仍判红』⇒ 测试必须结构上确定」的纪律。

mod common;

use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::thread;
use std::time::Duration;

/// 取出 `src` 里 `fn <name>` 的**函数体**（按大括号配平）。
fn fn_body(src: &str, name: &str) -> String {
    let start = src
        .find(&format!("fn {name}("))
        .unwrap_or_else(|| panic!("cannot find `fn {name}(` in the source"));
    let open = src[start..]
        .find('{')
        .unwrap_or_else(|| panic!("`fn {name}(` has no body"));
    let open = start + open;
    let mut depth = 0usize;
    for (i, ch) in src[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return src[open..=open + i].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("`fn {name}(` has an unbalanced body");
}

fn eval_src() -> &'static str {
    include_str!("../src/lib.rs")
}

fn tcp_src() -> &'static str {
    include_str!("../../wlwl-std/src/tcp.rs")
}

/// 取一个空闲端口（先绑再放 ⇒ 端口多半可用）。
fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    let port = l.local_addr().expect("local addr").port();
    drop(l);
    port
}

/// 连不上就隔 20 ms 再连，最多 100 次（≈2 s）—— Windows 上监听刚起来时
/// 连不上是常事。
fn connect_with_retry(port: u16) -> Option<TcpStream> {
    for _ in 0..100 {
        if let Ok(s) = TcpStream::connect(("127.0.0.1", port)) {
            return Some(s);
        }
        thread::sleep(Duration::from_millis(20));
    }
    None
}

/// 求值一段 WLWL，返回它的显示值（诊断 ⇒ `!CODE message`）。
fn actual(src: &str) -> String {
    let ast =
        wlwl_parser::parse(src, "t.wll").unwrap_or_else(|e| panic!("case must parse: {src}\n{e}"));
    let mut ev = wlwl_eval::Evaluator::new();
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

// ── N1：缓冲先于超时 ──────────────────────────────────────────────────────

/// [N1] `TCP_RECV` 必须**先看缓冲里有没有货**，再问「是不是等超时了」。
///
/// ⚠ 反过来 ⇒ 数据在截止时间**之前**就到了，却因截止时间先到而被丢弃
///   （返回「空字节 + `eof = FALSE`」⇒ 已经到达的数据没了）。
#[test]
fn recv_looks_at_the_buffer_before_the_timeout_flag() {
    let src = tcp_src();
    let body = fn_body(src, "tcp_recv");
    let buf_at = body
        .find("state.buf.lock()")
        .unwrap_or_else(|| panic!("tcp_recv no longer reads the connection buffer"));
    let timeout_at = body
        .find("take_timed_out()")
        .unwrap_or_else(|| panic!("tcp_recv no longer consults the timeout flag"));
    assert!(
        buf_at < timeout_at,
        "tcp_recv must read the buffer **before** consulting the timeout flag — \
         otherwise data that arrived before the deadline is thrown away \
         (buffer at byte {buf_at}, timeout flag at byte {timeout_at})"
    );
}

/// [N1] `TCP_ACCEPT` 必须**先看 `pending` 队列里有没有连接**，再问「是不是等超时了」。
#[test]
fn accept_looks_at_the_pending_queue_before_the_timeout_flag() {
    let src = tcp_src();
    let body = fn_body(src, "tcp_accept");
    let pending_at = body
        .find("state.pending.lock()")
        .unwrap_or_else(|| panic!("tcp_accept no longer reads the pending queue"));
    let timeout_at = body
        .find("take_timed_out()")
        .unwrap_or_else(|| panic!("tcp_accept no longer consults the timeout flag"));
    assert!(
        pending_at < timeout_at,
        "tcp_accept must pop the pending queue **before** consulting the timeout \
         flag — otherwise a connection that arrived before the deadline is \
         thrown away (pending at byte {pending_at}, timeout flag at byte {timeout_at})"
    );
}

// ── N3：就绪等待必须事件驱动 ──────────────────────────────────────────────

/// [N3] 解释器侧等就绪事件**必须事件驱动**（条件变量），不得靠固定轮询间隔。
///
/// ⚠ 改之前是「睡 50 ms 再看一眼」⇒ 一次「就绪」最多白等 50 ms
///   （第三方报告实测：客户端 33 ms 发数据，服务端 51 ms 才拿到）。
#[test]
fn the_readiness_wait_is_event_driven_not_a_fixed_poll() {
    let eval = eval_src();
    let body = fn_body(eval, "pump_socket_readiness");
    assert!(
        body.contains("await_ready"),
        "pump_socket_readiness must wait on the readiness condvar (`await_ready`) — \
         a fixed sleep-based poll adds up to 50 ms of latency to every socket wait"
    );
    assert!(
        !body.contains("thread::sleep"),
        "pump_socket_readiness must not busy-wait with `thread::sleep` — \
         the readiness queue signals a condvar instead"
    );
    let tcp = tcp_src();
    assert!(
        tcp.contains("fn await_ready("),
        "wlwl-std must expose `await_ready` — it is the event-driven half of \
         the readiness handoff"
    );
    assert!(
        tcp.contains("READY_CV"),
        "the readiness queue must carry a condvar so the interpreter can sleep \
         on it instead of polling"
    );
}

// ── N2：重放中的 `TCP_ACCEPT` 不得再次挂起 ─────────────────────────────────

/// [N2] **重放中的 `TCP_ACCEPT` 不得再次挂起**（并发册 §17.0 第 2 条）。
///
/// ⚠ 修复前的实测：任务体**不切段**（没有 `YIELD`）时，恢复 ⇒ **整段重跑**
///   ⇒ `TCP_ACCEPT` 被**再次执行** ⇒ 它上一次的等待条件（「队列里有连接」）
///   **已经不成立**（连接已被取走）⇒ 它**又挂了一次** ⇒ 违反「不得再挂」
///   ⇒ 任务原地活锁：accept 烧光自己的 5000 ms 预算 ⇒ 返回 `NULL` ⇒ 调用方
///   拿 `NULL` 去 `TCP_RECV` ⇒ **`E0030`**；而第一次 accept 到的那个连接
///   **被覆盖、再也无法关闭**。
/// ⇒ 修复后：重放时 accept **返回同一个连接** ⇒ 读照常拿到数据。
#[test]
fn a_replayed_accept_does_not_suspend_again_and_lose_the_connection() {
    let port = free_port();
    let holder = thread::spawn(move || {
        if let Some(mut s) = connect_with_retry(port) {
            let _ = s.write_all(b"hello");
            thread::sleep(Duration::from_millis(3000));
        }
    });
    // ⚠ **不切段**的任务体（没有 `YIELD`）—— 那正是「整段重跑」那条路径。
    let got = actual(&format!(
        r#"IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_ACCEPT", "TCP_RECV"]);
           LET(l, TCP_LISTEN("127.0.0.1", {port}));
           SCOPE(FUN(() ,
               LET(t, SPAWN(FUN(() ,
                   LET(conn, TCP_ACCEPT(l, 5000));
                   LET(chunk, TCP_RECV(conn, 100, 5000));
                   LEN(AT_K(chunk, "bytes", []))
               )));
               AWAIT(t)
           ))"#
    ));
    let _ = holder.join();
    assert_eq!(
        got, "5",
        "the read must return the 5 bytes that arrived — before the fix the \
         replayed accept suspended again, burned its whole timeout and handed \
         back NULL, which surfaced as `E0030 TCP_RECV: expected integer, got \
         null`; got {got:?}"
    );
}

/// [N2] **accept 循环必须给出不同的连接** —— 「记住上次那个连接」不能变成
/// 「永远返回同一个连接」。
#[test]
fn the_accept_loop_hands_out_a_different_connection_each_round() {
    let port = free_port();
    let holder = thread::spawn(move || {
        for payload in [b"one".as_slice(), b"two".as_slice()] {
            if let Some(mut s) = connect_with_retry(port) {
                let _ = s.write_all(payload);
                thread::sleep(Duration::from_millis(200));
            }
        }
        thread::sleep(Duration::from_millis(2000));
    });
    let got = actual(&format!(
        r#"IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_ACCEPT"]);
           LET(l, TCP_LISTEN("127.0.0.1", {port}));
           SCOPE(FUN(() ,
               LET(t, SPAWN(FUN(() ,
                   LET(c1, TCP_ACCEPT(l, 5000));
                   YIELD();
                   LET(c2, TCP_ACCEPT(l, 5000));
                   ==(c1, c2)
               )));
               AWAIT(t)
           ))"#
    ));
    let _ = holder.join();
    assert_eq!(
        got, "FALSE",
        "two accepts must hand out two different connections — the memo of \
         \"the connection this task accepted\" must not turn the accept loop \
         into a one-connection loop; got {got:?}"
    );
}

// ── N5：超时路径上的恢复提示 ───────────────────────────────────────────────

/// [N5] **超时唤醒也必须算「挂起点的再次求值」** —— 否则重放时 `TCP_ACCEPT`
/// 不会去查它的备忘 ⇒ 弹出空队列 ⇒ 返回 `NULL` ⇒ `TCP_RECV(NULL)` ⇒ **`E0030`**
/// ⇒ 任务死掉、连接句柄丢失、客户端收到 reset。
///
/// ⚠ 复现形态：**非切段**任务体（accept + recv 写在同一段）＋ 客户端**晚于**读
///   截止发数据 ⇒ `TCP_RECV` 超时 ⇒ 段首重放（**超时唤醒不设 socket 唤醒提示**）
///   ⇒ accept 被再次求值 ⇒ 空队列 ⇒ `NULL` ⇒ `E0030`。
/// ⇒ 修复后：accept 返回**同一个**连接 ⇒ 读拿到**超时的正常形态**
///   （`{bytes: [], text: "", eof: FALSE}`）⇒ 任务**活着**。
#[test]
fn a_timed_out_recv_does_not_kill_the_task() {
    let port = free_port();
    let holder = thread::spawn(move || {
        if let Some(mut s) = connect_with_retry(port) {
            // 晚于读截止（50 ms）才发数据 ⇒ 服务端那次读**超时**。
            thread::sleep(Duration::from_millis(300));
            let _ = s.write_all(b"hello");
            thread::sleep(Duration::from_millis(2000));
        }
    });
    let got = actual(&format!(
        r#"IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_ACCEPT", "TCP_RECV"]);
           LET(l, TCP_LISTEN("127.0.0.1", {port}));
           SCOPE(FUN(() ,
               LET(t, SPAWN(FUN(() ,
                   LET(conn, TCP_ACCEPT(l, 5000));
                   LET(chunk, TCP_RECV(conn, 100, 50));
                   LEN(AT_K(chunk, "bytes", []))
               )));
               AWAIT(t)
           ))"#
    ));
    let _ = holder.join();
    assert_eq!(
        got, "0",
        "a read that times out must yield the documented timeout shape (0 bytes, \
         eof = FALSE) and leave the task alive — before the fix the replayed \
         accept returned NULL and the task died with `E0030 TCP_RECV: expected \
         integer, got null`; got {got:?}"
    );
}

// ── 守卫自身：源文件必须在（路径写错 ⇒ 守卫假绿）───────────────────────────

#[test]
fn the_guarded_sources_are_the_real_ones() {
    for rel in ["crates/wlwl-std/src/tcp.rs", "crates/wlwl-eval/src/lib.rs"] {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(rel);
        assert!(
            p.exists(),
            "guarded source {rel} is missing at {} — the guards would be vacuous",
            p.display()
        );
    }
    let _ = common::scratch("wlwl_socket_wait");
}
