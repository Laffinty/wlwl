//! [2026-10-28 / 需求清单 B-2] `TCP_ACCEPT` 的**惊群**（thundering herd）—— 契约。
//!
//! ## 它钉的是什么
//!
//! 同一个监听句柄上**多个 acceptor** 等待时，一次就绪事件**只唤醒一个**
//! （FIFO 队首）⇒ 其余等待者**留在名单里**继续等**下一次**就绪事件 ⇒
//! **落选者不拿到 `NULL`、不报错、保持存活，并能继续接受后续连接**。
//!
//! ## 修复前的形态（惊群本体）
//!
//! `Scheduler::wake_socket_waiter` 此前把**整张**等待者名单摘掉、**全部**唤醒
//! ⇒ 一次就绪事件只够**一个**连接 ⇒ 落选者被唤醒却**没货** ⇒ `TCP_ACCEPT`
//! 返回 `NULL`（与超时**同形**）⇒ 调用方拿 `NULL` 去 `TCP_RECV` ⇒ `E0030`
//! ⇒ 任务死 ⇒ `SCOPE` 提前退出 ⇒ 监听句柄随进程消失 ⇒ 客户端 RST。
//!
//! ## 判据为什么是「两个 acceptor 各拿到一条连接」
//!
//! ⚠ 落选者拿到的 `NULL` 与**超时**的 `NULL` **同形** ⇒ 程序**分不出来**
//!   ⇒ 判据不能是「落选者拿到的是不是 `NULL`」，而是它的**可观察后果**：
//!   ① 落选者**继续等**下一次连接 ⇒ 于是**两个** acceptor **各拿到一条**；
//!   ② 落选者**不把 `NULL` 喂给 `TCP_RECV`** ⇒ 任务**不死**。
//!
//! ## 为什么夹具里的端口是「可替换的令牌」
//!
//! ⚠ `wlwl:std.net` **没有客户端侧的 `TCP_CONNECT`** ⇒ 纯 WLWL 程序**发不出**
//!   连接 ⇒ 惊群场景**必须**由外部客户端驱动 ⇒ 夹具里的监听端口由本文件
//!   **替换**成一个空闲端口（夹具里的默认值 `0` = 临时端口 ⇒ 独立运行
//!   **不会撞端口**）。
//!
//! ## 客户端为什么**先等 300 ms** 再连
//!
//! ⚠ 惊群要求「**两个** acceptor **同时**挂在同一个监听句柄上」⇒ 客户端
//!   必须**晚于**两个 acceptor 挂起 ⇒ 客户端先睡 300 ms（挂起是微秒级的事）
//!   ⇒ 判据**结构上**确定，不靠「谁先跑」的运气。
//!   ⚠ 反过来（客户端**立刻**连）⇒ 可能只有一个 acceptor 挂上了 ⇒ 惊群
//!   **根本没发生** ⇒ 修复前也会绿 ⇒ 那样的用例**测不出**这个 bug。

use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use wlwl_eval::runtime::{Scheduler, TaskId, TaskState};
use wlwl_eval::task::Task;
use wlwl_eval::{Env, Value};

// ── 串行闸（与 `tcp_contract.rs` 同款）────────────────────────────────────
//
// ⚠ 这批用例**共用端口** ⇒ 并发跑会互相抢端口（`free_port()` 只能「先占后放」
//   ⇒ 两个用例可能拿到**同一个**端口）⇒ 一把进程内互斥锁把它们串起来。
fn socket_gate() -> std::sync::MutexGuard<'static, ()> {
    static GATE: Mutex<()> = Mutex::new(());
    GATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// 抢一个临时端口（先占后放，让操作系统挑一个空闲的）。
fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    let port = l.local_addr().expect("local addr").port();
    drop(l);
    port
}

/// 连上服务端 —— **必须带重试**（客户端线程是**并行**跑的 ⇒ `connect()` 很可能
/// **早于**监听建立 ⇒ 拿到 ECONNREFUSED）。
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

/// 并发套件里的那个夹具（惊群场景的**唯一**来源）。
const FIXTURE: &str = include_str!("../../../tests/concurrency/tcp_accept_single_wakeup.wll");

/// 夹具里**可替换**的监听端口令牌（夹具默认用 `0` = 临时端口）。
const LISTEN_TOKEN: &str = r#"TCP_LISTEN("127.0.0.1", 0)"#;

/// 起一个客户端：睡 300 ms（让两个 acceptor 都挂上）⇒ 连第一条 ⇒ 隔 250 ms
/// 连第二条 ⇒ 两条都写 `payload` ⇒ 握住不放。
fn spawn_client(port: u16, payload: &'static [u8]) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        let mut first = match connect_with_retry(port) {
            Some(s) => s,
            None => return,
        };
        let _ = first.write_all(payload);
        thread::sleep(Duration::from_millis(250));
        if let Ok(mut second) = TcpStream::connect(("127.0.0.1", port)) {
            let _ = second.write_all(payload);
            // ⚠ 握住两条连接，直到服务端跑完（否则服务端读到的是 EOF）。
            thread::sleep(Duration::from_millis(3000));
        }
        thread::sleep(Duration::from_millis(500));
    })
}

// ── 判据 ①：夹具 —— 两个 acceptor 各拿到一条连接 ──────────────────────────

/// [B-2] 同一个监听句柄上**两个 acceptor** ⇒ **各拿到一条连接**。
///
/// ⚠ 修复前（唤醒全部）：第一个就绪事件把**两个**都唤醒 ⇒ 一个拿到连接、
///   另一个拿到 `NULL` ⇒ 输出是 `[1, 0]` ⇒ 这条会红。
/// ⚠ 修复后（只唤醒一个）：落选者**留在名单里** ⇒ 等到**第二条**连接 ⇒
///   输出是 `[1, 1]`。
#[test]
fn two_acceptors_on_one_listener_each_get_a_connection() {
    let _gate = socket_gate();
    let port = free_port();
    let src = FIXTURE.replace(LISTEN_TOKEN, &format!(r#"TCP_LISTEN("127.0.0.1", {port})"#));
    assert_ne!(
        src, FIXTURE,
        "the fixture must still contain the substitutable listen token \
         ({LISTEN_TOKEN:?}) — otherwise this case would silently stop driving it"
    );
    let client = spawn_client(port, b"hello");
    let got = actual(&src);
    let _ = client.join();
    assert_eq!(
        got, "[1, 1]",
        "both acceptors must end up with one connection each — the loser of the race \
         must **stay parked** (and pick up the next connection) instead of being \
         woken with nothing to take; got {got:?}"
    );
}

// ── 判据 ②：落选者不死（`E0030` 那条真实死法）────────────────────────────

/// [B-2] 落选者**不把 `NULL` 喂给 `TCP_RECV`** ⇒ 任务**不死**。
///
/// ⚠ 这是需求方报的**真实死法**：落选者拿到 `NULL` ⇒ `TCP_RECV(NULL)` ⇒
///   `E0030: TCP_RECV: expected integer, got null` ⇒ 任务失败 ⇒ `SCOPE` 提前
///   退出 ⇒ 监听句柄随进程消失 ⇒ 客户端 RST。
/// ⚠ 修复前：这条的返回值是 `!E0030 TCP_RECV: expected integer, got null`。
/// ⚠ 修复后：两个 acceptor **各读到 5 字节** ⇒ `55`。
#[test]
fn the_loser_of_the_accept_race_does_not_die_with_e0030() {
    let _gate = socket_gate();
    let port = free_port();
    let client = spawn_client(port, b"hello");
    let got = actual(&format!(
        r#"IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_ACCEPT", "TCP_RECV"]);
           LET(l, TCP_LISTEN("127.0.0.1", {port}));
           SCOPE(FUN(() ,
               LET(a, SPAWN(FUN(() ,
                   LET(conn, TCP_ACCEPT(l, 1500));
                   YIELD();
                   LET(chunk, TCP_RECV(conn, 100, 1500));
                   LEN(AT_K(chunk, "bytes", []))
               )));
               LET(b, SPAWN(FUN(() ,
                   LET(conn, TCP_ACCEPT(l, 1500));
                   YIELD();
                   LET(chunk, TCP_RECV(conn, 100, 1500));
                   LEN(AT_K(chunk, "bytes", []))
               )));
               LET(ra, AWAIT(a));
               LET(rb, AWAIT(b));
               +( *(ra, 10), rb)
           ))"#
    ));
    let _ = client.join();
    assert_eq!(
        got, "55",
        "the loser of the accept race must stay alive and read its own connection — \
         before the fix it was woken with nothing to take, got a `NULL`, and died \
         with `E0030 TCP_RECV: expected integer, got null`; got {got:?}"
    );
}

// ── 判据 ③：调度器层 —— 一次就绪事件只唤醒一个 ───────────────────────────

/// [B-2] 调度器层：**一个** socket 上可以有**多个**等待者，但一次就绪事件
/// **只唤醒队首一个**，其余**留在名单里**。
///
/// ⚠ 「可以挂多个」这条**不变**（审计 N4：同一条连接可以有多个读者）⇒ 变的是
///   **唤醒几个**，不是**允许挂几个**。
/// ⚠ 「其余留在名单里」也是硬要求：整张名单被摘掉 ⇒ 他们**永远等不到**下一次
///   唤醒 ⇒ 变成「永久挂起」——比惊群更糟。
#[test]
fn one_ready_event_wakes_exactly_one_waiter_and_keeps_the_others_registered() {
    let mut s = Scheduler::new();
    let a = push_test_task(&mut s, 0);
    let b = push_test_task(&mut s, 1);
    s.park_for_socket(a, 21);
    s.park_for_socket(b, 21);
    assert_eq!(
        s.socket_waiter_count(),
        2,
        "both waiters must be registered — a socket may have more than one waiter"
    );
    assert!(
        s.wake_socket_waiter(21),
        "the socket is ready ⇒ somebody must wake"
    );
    assert!(
        matches!(s.tasks[0].state, TaskState::Pending),
        "the FIFO head must be woken by the ready event"
    );
    assert!(
        matches!(s.tasks[1].state, TaskState::Suspended { .. }),
        "the second waiter must NOT be woken — one ready event carries one connection; \
         waking it too is the thundering herd (it would get a `NULL`)"
    );
    assert_eq!(
        s.socket_waiter_count(),
        1,
        "the surviving waiter must stay registered so the next ready event can wake it"
    );
    assert!(
        s.has_socket_waiter(21),
        "the socket still has a waiter ⇒ the scheduler must keep waiting for readiness"
    );
    // ⚠ 下一次就绪事件 ⇒ 轮到**它**。
    assert!(
        s.wake_socket_waiter(21),
        "the second ready event must wake the remaining waiter"
    );
    assert!(
        matches!(s.tasks[1].state, TaskState::Pending),
        "the surviving waiter must be woken by the next ready event"
    );
    assert_eq!(
        s.socket_waiter_count(),
        0,
        "nobody is waiting any more ⇒ the scheduler may stop waiting for readiness"
    );
}

/// 造一个 `Pending` 任务放进调度器（测试用；与 `runtime.rs` 里的同名助手同款）。
fn push_test_task(s: &mut Scheduler, tid: usize) -> TaskId {
    let gen = s.bump_generation();
    let id = TaskId(tid);
    s.tasks.push(Task::new_pending(
        id,
        gen,
        Value::Null,
        Env::new(),
        wlwl_eval::runtime::ScopeId(0),
    ));
    id
}

// ── 守卫自身：夹具必须在（路径写错 ⇒ 守卫假绿）────────────────────────────

#[test]
fn the_guarded_fixture_is_the_real_one() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("concurrency")
        .join("tcp_accept_single_wakeup.wll");
    assert!(
        p.exists(),
        "the guarded fixture is missing at {} — the guards would be vacuous",
        p.display()
    );
    assert!(
        FIXTURE.contains(LISTEN_TOKEN),
        "the fixture must keep the substitutable listen token ({LISTEN_TOKEN:?})"
    );
}
