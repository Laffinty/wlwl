//! [W-38] `TCP_SERVE` 的 handler 里调**宿主挂起点成员** ⇒ 那条连接**静默作废**。
//!
//! ## 这张契约表守的是什么
//!
//! `TCP_SERVE` 的 handler 跑在**原生帧**里（`StdHost::call` 回调进 WLWL 代码）
//! ⇒ 那里**挂不了**（没有调度器槽位）⇒ 成员尝试挂起 ⇒ 挂起信号**丢失** ⇒
//! 连接**既没被服务、也没有任何诊断**（失败只落进宿主告警槽的**软告警**，
//! 而软告警**不打印**）。
//!
//! ⚠ 实测旧读数：handler 被调用 **1** 次，但 `TCP_SERVE` 返回 **0**、程序
//!   **exit 0**、**stderr 一个字都没有**。
//!
//! ## 修法（= 计划的「① 诊断化」方向）
//!
//! 并发册 §17.0 的措辞是「**宿主挂起点成员的调用点必须在任务体内**」⇒
//! **原生帧不是任务体** ⇒ 按字面读法**本来就该报错**。
//!
//! ⇒ **原生帧回调进 WLWL 代码时**，把「当前在任务体内」的标志**清掉**
//! （`task_body_depth` 加深一层 + `current_task` 清空）⇒ handler 里调
//! `TCP_RECV` / `TCP_ACCEPT` / `SLEEP_ASYNC` ⇒ 走 W-10 的**既有**判据 ⇒
//! 报**既有** `E0014`（**不新增诊断码**）。
//!
//! ## ⚠ `TCP_SERVE` 的既有可观察语义**不许**改
//!
//! * **成功服务的连接数** —— 误用的那条连接**不算**被服务。
//! * **失败只记一笔、继续下一条** —— 误用**不得**终止整个接受循环。
//!
//! ⇒ 下面那条用例用**两条连接**（一条误用、一条正常）把这两条都钉住。
//!
//! ## ⚠ 占位符为什么是 `__PORT__`（不是 `PORT`）
//!
//! ⚠ 本文件第一版用 `SERVER.replace("PORT", &port.to_string())` ⇒ **`IMPORT` 里的
//!   "PORT" 也被替换掉**（`IMPORT` ⇒ `IM<port>`）⇒ 服务端程序第一行就
//!   `E0020: undefined name 'IM<port>'` ⇒ **端口从未被 listen** ⇒ 客户端的
//!   `connect` 一直连不上 ⇒ **本用例挂死**（不是断言失败，是永远等下去）。
//! ⇒ 现在用 `__PORT__` 这种**不会出现在源码里**的占位符。

mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use wlwl_eval::Evaluator;

/// **串行闸**（socket 族共用一把进程内锁 —— 同 `tcp_serve_contract.rs`）。
fn socket_gate() -> std::sync::MutexGuard<'static, ()> {
    common::handle_guard::serial_gate()
}

/// 抢一个临时端口（先占后放，让操作系统挑一个空闲的）。
fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    let port = l.local_addr().expect("local addr").port();
    drop(l);
    port
}

/// 连上服务端 —— **必须带重试**（客户端线程是**并行**跑的 ⇒ 可能早于监听建立）。
fn connect_with_retry(port: u16) -> Option<TcpStream> {
    for _ in 0..100 {
        if let Ok(s) = TcpStream::connect(("127.0.0.1", port)) {
            return Some(s);
        }
        thread::sleep(Duration::from_millis(20));
    }
    None
}

/// 服务端程序：handler 对 `/bad` **误用**挂起点成员，对别的路径正常服务。
///
/// ⚠ `SLEEP_ASYNC` 是**宿主挂起点成员**（`std.time`）⇒ 在原生帧里调用它
///   ⇒ 正是 W-38 的形状（它**不需要**连接句柄 ⇒ 夹具最小）。
const SERVER: &str = r#"IMPORT("wlwl:std.io", ["PRINT"]);
IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_SERVE", "TCP_CLOSE_LISTENER"]);
IMPORT("wlwl:std.time", ["SLEEP_ASYNC"]);
PRINT("[dbg-wll] before-listen");
LET(l, TCP_LISTEN("127.0.0.1", __PORT__));
PRINT("[dbg-wll] listening");
LET(served, TCP_SERVE(l, 2000, NULL, FUN((req), (
  IF(==(AT_K(req, "path", ""), "/bad"),
     (SLEEP_ASYNC(50); ["status": 200, "headers": [], "body": "bad"]),
     ["status": 200, "headers": ["content-type": "text/plain"], "body": "ok"])
))));
PRINT("[dbg-wll] served");
TCP_CLOSE_LISTENER(l);
served"#;

/// 跑服务端 ⇒ (`TCP_SERVE` 的返回值, 诊断台账)。
fn run_server(src: &str) -> (String, Vec<wlwl_error::WlwlDiagnostic>) {
    let ast =
        wlwl_parser::parse(src, "t.wll").unwrap_or_else(|e| panic!("case must parse: {src}\n{e}"));
    let mut ev = Evaluator::new();
    eprintln!("[dbg] server evaluating");
    let out = match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => format!(
            "!{} {}",
            e.diagnostic().code.as_str(),
            e.diagnostic().message
        ),
    };
    eprintln!("[dbg] server evaluated");
    (out, ev.native_frame_diagnostics.clone())
}

/// 连一次、发一条路径、读回响应。
fn one_request(port: u16, path: &str) -> String {
    eprintln!("[dbg] client connecting {path}");
    let Some(mut s) = connect_with_retry(port) else {
        eprintln!("[dbg] client NO-CONNECTION {path}");
        return "NO-CONNECTION".to_string();
    };
    eprintln!("[dbg] client connected {path}");
    let _ = s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes());
    eprintln!("[dbg] client wrote {path}");
    let _ = s.set_read_timeout(Some(Duration::from_millis(5000)));
    let mut buf = String::new();
    let _ = s.read_to_string(&mut buf);
    eprintln!("[dbg] client read {} bytes for {path}", buf.len());
    buf
}

/// ⚠ **主证据**：handler 里调宿主挂起点成员 ⇒ **有诊断**（`E0014`，指名成员 +
/// 指向**调用点**），而**不是**「静默作废」。
///
/// ⚠ **反向守卫**：把原生帧的「任务体标志清掉」去掉 ⇒ 本用例必红
///   （诊断台账空 ⇒ 回到「静默少一条」）。
#[test]
fn a_host_suspend_member_in_the_handler_is_diagnosed() {
    let _gate = socket_gate();
    let port = free_port();
    let src = SERVER.replace("__PORT__", &port.to_string());

    // ⚠ 客户端线程**并行**跑 ⇒ 两条连接依次来。
    let client = thread::spawn(move || {
        let bad = one_request(port, "/bad");
        let good = one_request(port, "/good");
        (bad, good)
    });

    let (served, diags) = run_server(&src);
    let (bad, good) = client.join().expect("client thread");

    // ① **有诊断**：一条 `E0014`，指名成员，位置指向**调用点**。
    assert_eq!(
        diags.len(),
        1,
        "the native-frame misuse must produce exactly one diagnostic; got {diags:#?}"
    );
    let d = &diags[0];
    assert_eq!(
        d.code.as_str(),
        "E0014",
        "the diagnostic must use the existing E0014 (no new code); got {d:#?}"
    );
    assert!(
        d.message.contains("SLEEP_ASYNC"),
        "the diagnostic must name the member; got {:?}",
        d.message
    );
    assert!(
        d.message.contains("used outside a task body"),
        "the diagnostic must reuse the W-10 wording; got {:?}",
        d.message
    );
    // ⚠ 位置指向**调用点**（那一行就是 `SLEEP_ASYNC(...)` 本身）。
    let expect_line = src
        .lines()
        .position(|l| l.contains("SLEEP_ASYNC(50)"))
        .expect("the fixture must contain the SLEEP_ASYNC call")
        + 1;
    assert_eq!(
        d.location.line as usize, expect_line,
        "the diagnostic must point at the call site; got {d:#?}"
    );

    // ② **`TCP_SERVE` 的既有可观察语义不变**：
    //    误用的那条连接**不算**被服务 ⇒ 计数是 1（只有 `/good` 那条）。
    assert_eq!(
        served, "1",
        "only the well-behaved connection counts as served; got {served:?}"
    );
    // ③ **失败只记一笔、继续下一条** ⇒ `/good` 那条**照常服务**。
    assert!(
        good.contains("200 OK"),
        "the loop must continue after the failed connection; got {good:?}"
    );
    assert!(
        bad.contains("500"),
        "the misused connection must be voided (the client gets a 500); got {bad:?}"
    );
}

/// ⚠ **反向守卫**的另一半：原生帧**不是**任务体 ⇒ handler 里调挂起点成员
/// **当场**报 `E0014` ⇒ **不是**「挂起信号丢了 ⇒ 任务正常返回 NULL」。
#[test]
fn the_native_frame_is_not_a_task_body() {
    let _gate = socket_gate();
    let port = free_port();
    let src = SERVER.replace("__PORT__", &port.to_string());
    let client = thread::spawn(move || one_request(port, "/bad"));
    let (_served, diags) = run_server(&src);
    let _ = client.join().expect("client thread");
    assert!(
        !diags.is_empty(),
        "the misuse must be reported, not silently swallowed"
    );
    // ⚠ 那条诊断**必须**是「挂起点成员在任务体之外」这一族 ⇒ 消息里指名成员。
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("outside a task body")),
        "the diagnostic must be the host-suspend-site one; got {diags:#?}"
    );
}
