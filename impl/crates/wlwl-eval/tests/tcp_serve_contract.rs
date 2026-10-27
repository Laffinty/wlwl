//! `wlwl:std.net::TCP_SERVE` 的契约测试（W-12 / B-3 候选 a）。
//!
//! ## 这张表守的是什么
//!
//! `TCP_SERVE(listener, timeout_ms, opts, handler) -> INTEGER` 在**一个原生成员
//! 内部**推进整个接受循环：
//!
//! ```text
//! accept → 读满一次请求 → 调 handler(request) → 写回响应 → 下一条连接
//! ```
//!
//! 它替代「静态展开的 accept 链」（连接数写死在源码里的那种）。这张契约表钉住
//! 它的**行为基准**：
//!
//! 1. **单条连接异常 ⇒ 记一笔、继续下一条**（对端断开 / 读失败 / handler 抛错
//!    / 响应形状不对）—— **绝不**终止整个循环。
//! 2. **accept 超时 ⇒ 退出循环**，返回**已成功服务的连接数**。
//! 3. **元数 / 类型错 ⇒ `E0022` / `E0030`**；**运行期失败 ⇒ `ERR` 值**。
//!
//! ## 串行闸（**不是**可选的洁癖）
//!
//! socket 族的用例**共用**一把进程内互斥锁（`common::handle_guard::SERIAL`）
//! —— 与 `tcp_contract.rs` 同款：句柄数与端口都是**进程级**的 ⇒ 并发跑会互相
//! 干扰。
//!
//! ## ⚠ 一个**已知**的代价（不是缺陷，是候选 a 的形状）
//!
//! 原生帧里的等待是**阻塞调用线程** ⇒ `TCP_SERVE` 期间**别的任务跑不了**
//! （与 `HTTP_*` / `SLEEP` 同类）。这与 `TCP_ACCEPT` / `TCP_RECV` 的
//! 「宿主挂起 ⇒ 别的任务照常运行」**不同** —— 那是「静态 accept 链」那条路的优点。

mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use wlwl_eval::Evaluator;

/// **串行闸**（socket 族共用一把进程内锁）。
fn socket_gate() -> std::sync::MutexGuard<'static, ()> {
    common::handle_guard::serial_gate()
}

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_tcp_serve")
}

/// 求值一段 WLWL 源码 ⇒ 可观察的结果（值 / `!E**** 消息`）。
fn actual(src: &str) -> String {
    let ast =
        wlwl_parser::parse(src, "t.wll").unwrap_or_else(|e| panic!("case must parse: {src}\n{e}"));
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
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

/// 抢一个临时端口（先占后放，让操作系统挑一个空闲的）。
fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    let port = l.local_addr().expect("local addr").port();
    drop(l);
    port
}

/// 连上服务端 —— **必须带重试**。
///
/// ⚠ 与 `tcp_contract.rs` 同款的理由：客户端线程是**并行**跑的 ⇒ 客户端的
/// `connect()` 很可能**早于**监听建立 ⇒ 拿到 ECONNREFUSED ⇒ 服务端只能等满
/// 超时。⇒ 连不上就隔 20 ms 再连，最多 100 次（≈2 s）。
fn connect_with_retry(port: u16) -> Option<TcpStream> {
    for _ in 0..100 {
        if let Ok(s) = TcpStream::connect(("127.0.0.1", port)) {
            return Some(s);
        }
        thread::sleep(Duration::from_millis(20));
    }
    None
}

/// 一次「客户端跑一遍」的产物：每条连接的 `(路径, 响应原文)`。
type ClientRun = Result<Vec<(String, String)>, String>;

// ── 形态：元数 / 类型 / 范围 ──────────────────────────────────────────────

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const SHAPE: &[Case] = &[
    Case {
        name: "serve_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.net", ["TCP_SERVE"]); TCP_SERVE(1, 2, [])"#,
        expect: "!E0022 TCP_SERVE: function expects 4 argument(s), got 3",
    },
    Case {
        name: "serve_listener_non_integer_is_e0030",
        src: r#"IMPORT("wlwl:std.net", ["TCP_SERVE"]); TCP_SERVE("x", 10, NULL, FUN((r), 1))"#,
        expect: "!E0030 TCP_SERVE: expected integer, got string",
    },
    Case {
        name: "serve_timeout_non_integer_is_e0030",
        src: r#"IMPORT("wlwl:std.net", ["TCP_SERVE"]); TCP_SERVE(1, "x", NULL, FUN((r), 1))"#,
        expect: "!E0030 TCP_SERVE: expected integer, got string",
    },
    Case {
        name: "serve_negative_timeout_is_e0030",
        src: r#"IMPORT("wlwl:std.net", ["TCP_SERVE"]); TCP_SERVE(1, -5, NULL, FUN((r), 1))"#,
        expect: "!E0030 TCP_SERVE: expected a non-negative timeout in ms, got -5",
    },
    Case {
        name: "serve_opts_non_dict_is_e0030",
        src: r#"IMPORT("wlwl:std.net", ["TCP_SERVE"]); TCP_SERVE(1, 10, 5, FUN((r), 1))"#,
        expect: "!E0030 TCP_SERVE: expected a DICT of options or NULL, got integer",
    },
    Case {
        name: "serve_unknown_option_is_e0030",
        src: r#"IMPORT("wlwl:std.net", ["TCP_SERVE"]); TCP_SERVE(1, 10, ["bogus": 1], FUN((r), 1))"#,
        expect: "!E0030 TCP_SERVE: unknown option \"bogus\"",
    },
    Case {
        name: "serve_handler_not_callable_is_e0030",
        src: r#"IMPORT("wlwl:std.net", ["TCP_SERVE"]); TCP_SERVE(1, 10, NULL, 5)"#,
        expect: "!E0030 TCP_SERVE: expected a function handler, got integer",
    },
];

#[test]
fn serve_shape_cases() {
    for c in SHAPE {
        let got = actual(c.src);
        assert!(
            got.starts_with(c.expect),
            "case {}: expected {:?}, got {:?}",
            c.name,
            c.expect,
            got
        );
    }
}

#[test]
fn unknown_listener_handle_is_an_err_value_not_a_diagnostic() {
    // ⚠ ERR 若是程序的**最后一个表达式**会逃逸到顶层 ⇒ 报 E0102（语言既有行为）。
    //   所以这里必须**消费**它（`IS_ERR`）才能看见 ERR 的形状。
    let got = actual(
        r#"IMPORT("wlwl:std.net", ["TCP_SERVE"]);
           LET(r, TCP_SERVE(999999, 10, NULL, FUN((q), 1)));
           IF(IS_ERR(r), AT_K(ERR_PAYLOAD(r), "kind", "?"), "NOT-ERR")"#,
    );
    assert_eq!(
        got, "SocketError",
        "the payload must name the family, got {got:?}"
    );
}

// ── 语义：连续 10+ 条连接全部拿到正确响应 ────────────────────────────────

/// 逐条连接跑一遍客户端：每条发一个请求、读回响应。
///
/// ⚠ `broken_at` = `Some(i)` ⇒ 第 `i` 条**发一半就断开**（对端异常）。
fn run_client(port: u16, n: usize, broken_at: Option<usize>) -> ClientRun {
    let mut seen = Vec::new();
    for i in 0..n {
        let Some(mut s) = connect_with_retry(port) else {
            return Err(format!("client {i}: could not connect"));
        };
        let path = format!("/req{i}");
        if broken_at == Some(i) {
            // ⚠ 发**一半**就断开 ⇒ 服务端读不满一次请求 ⇒ 这一条连接作废。
            let _ = s.write_all(b"GET /broken HTTP/1.1\r\n");
            drop(s);
            continue;
        }
        if let Err(e) = s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes()) {
            return Err(format!("client {i}: write failed: {e}"));
        }
        let _ = s.set_read_timeout(Some(Duration::from_millis(5000)));
        let mut buf = String::new();
        if let Err(e) = s.read_to_string(&mut buf) {
            return Err(format!("client {i}: read failed: {e}"));
        }
        seen.push((path, buf));
    }
    Ok(seen)
}

/// 服务端程序：`TCP_SERVE` 把路径**原样回显**在响应体里。
fn serve_echoing_path(port: u16, timeout_ms: u64) -> String {
    actual(&format!(
        r#"IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_SERVE"]);
           LET(l, TCP_LISTEN("127.0.0.1", {port}));
           TCP_SERVE(l, {timeout_ms}, NULL, FUN((req),
               ["status": 200,
                "headers": ["content-type": "text/plain"],
                "body": AT_K(req, "path", "?")]))"#
    ))
}

#[test]
fn ten_plus_consecutive_connections_are_all_served() {
    let _gate = socket_gate();
    let port = free_port();
    let n = 12;
    let client = thread::spawn(move || run_client(port, n, None));

    let served = serve_echoing_path(port, 1000);
    let seen = client.join().expect("client thread").expect("client run");

    assert_eq!(
        served,
        n.to_string(),
        "every connection must be served — TCP_SERVE returns the number of \
         successfully served connections; got {served:?}"
    );
    assert_eq!(
        seen.len(),
        n,
        "the client must have talked to the server {n} times"
    );
    for (path, resp) in seen {
        assert!(
            resp.contains("200 OK"),
            "the client must get a 200 for {path}, got {resp:?}"
        );
        assert!(
            resp.contains(&path),
            "the response body must echo the request path {path}, got {resp:?}"
        );
    }
}

// ── 资源：socket 句柄**不许**留在进程里 ─────────────────────────────────

#[test]
fn tcp_serve_leaves_no_socket_behind() {
    // ⚠ 句柄计数是**进程级**的 ⇒ 必须走仓内的串行闸（`assert_no_handle_leak`
    //   自己取闸 ⇒ 本文件里所有会开句柄的用例都共用它 ⇒ 不会互相污染）。
    //
    // ⚠ **监听器由程序自己关**（`TCP_SERVE` **不**关监听 —— 它只关**连接**）
    //   ⇒ 程序末尾必须 `TCP_CLOSE_LISTENER`，否则监听 socket 会一直挂着。
    let n = 6usize;
    common::handle_guard::assert_no_handle_leak(
        "TCP_SERVE socket family",
        || {
            let port = free_port();
            let client = thread::spawn(move || run_client(port, n, None));
            let served = actual(&format!(
                r#"IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_SERVE", "TCP_CLOSE_LISTENER"]);
                   LET(l, TCP_LISTEN("127.0.0.1", {port}));
                   LET(served, TCP_SERVE(l, 1000, NULL, FUN((req),
                       ["status": 200,
                        "headers": ["content-type": "text/plain"],
                        "body": AT_K(req, "path", "?")])));
                   TCP_CLOSE_LISTENER(l);
                   served"#
            ));
            let seen = client.join().expect("client thread").expect("client run");
            assert_eq!(
                served,
                n.to_string(),
                "every connection must be served, got {served:?}"
            );
            assert_eq!(
                seen.len(),
                n,
                "the client must have talked to the server {n} times"
            );
            port
        },
        // 监听器**已经**由程序自己关掉了 ⇒ 这里没有别的要关。
        |_port| {},
    );
}

// ── 语义：中途断开一条**不影响后续** ─────────────────────────────────────

#[test]
fn a_disconnect_in_the_middle_does_not_stop_the_loop() {
    let _gate = socket_gate();
    let port = free_port();
    let n = 12;
    let broken = 5;
    let client = thread::spawn(move || run_client(port, n, Some(broken)));

    let served = serve_echoing_path(port, 1000);
    let seen = client.join().expect("client thread").expect("client run");

    // ⚠ 断开的**那一条**不计入「成功服务」；其余 11 条必须**全部**被服务。
    assert_eq!(
        served,
        (n - 1).to_string(),
        "one broken connection must void only itself — the other {} must still be served; \
         got {served:?}",
        n - 1
    );
    assert_eq!(
        seen.len(),
        n - 1,
        "the client must have completed {} good exchanges",
        n - 1
    );
    for (path, resp) in seen {
        assert!(
            resp.contains("200 OK"),
            "the client must get a 200 for {path} (the broken connection came earlier \
             and must not have stopped the loop), got {resp:?}"
        );
        assert!(
            resp.contains(&path),
            "the response body must echo the request path {path}, got {resp:?}"
        );
    }
}

// ── 语义：handler 返回坏形状 ⇒ **只废这一条** ────────────────────────────

#[test]
fn a_bad_handler_shape_voids_only_that_connection() {
    let _gate = socket_gate();
    let port = free_port();
    let n = 6;
    let bad = 2;

    // ⚠ 客户端：第 `bad` 条请求的路径是 "/bad" ⇒ handler 对它返回**缺 `body`** 的
    //   响应字典 ⇒ 那一条连接作废；其余 5 条必须正常。
    let client = thread::spawn(move || {
        let mut seen = Vec::new();
        for i in 0..n {
            let Some(mut s) = connect_with_retry(port) else {
                return Err(format!("client {i}: could not connect"));
            };
            let path = if i == bad {
                "/bad".to_string()
            } else {
                format!("/req{i}")
            };
            if let Err(e) =
                s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
            {
                return Err(format!("client {i}: write failed: {e}"));
            }
            let _ = s.set_read_timeout(Some(Duration::from_millis(5000)));
            let mut buf = String::new();
            if let Err(e) = s.read_to_string(&mut buf) {
                return Err(format!("client {i}: read failed: {e}"));
            }
            seen.push((path, buf));
        }
        Ok(seen)
    });

    let served = actual(&format!(
        r#"IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_SERVE"]);
           LET(l, TCP_LISTEN("127.0.0.1", {port}));
           TCP_SERVE(l, 1000, NULL, FUN((req),
               IF(==(AT_K(req, "path", ""), "/bad"),
                  ["status": 200, "headers": ["x-note": "no body here"]],
                  ["status": 200,
                   "headers": ["content-type": "text/plain"],
                   "body": AT_K(req, "path", "?")])))"#
    ));
    let seen = client.join().expect("client thread").expect("client run");

    assert_eq!(
        served,
        (n - 1).to_string(),
        "the bad-shape connection must void only itself; the other {} must be served; \
         got {served:?}",
        n - 1
    );
    assert_eq!(
        seen.len(),
        n,
        "the client must have talked to the server {n} times"
    );
    for (path, resp) in seen {
        if path == "/bad" {
            assert!(
                resp.contains("500"),
                "the bad-shape connection must be voided (the client gets a 500), \
                 got {resp:?}"
            );
            assert!(
                !resp.contains("200 OK"),
                "the bad-shape connection must not get the handler's response, \
                 got {resp:?}"
            );
        } else {
            assert!(
                resp.contains("200 OK"),
                "the good connections must still be served (the bad one came earlier \
                 and must not have stopped the loop), got {resp:?}"
            );
            assert!(
                resp.contains(&path),
                "the response body must echo the request path {path}, got {resp:?}"
            );
        }
    }
}

// ── 语义：请求字典的形状 ─────────────────────────────────────────────────

#[test]
fn the_handler_sees_the_request_shape() {
    let _gate = socket_gate();
    let port = free_port();
    let client = thread::spawn(move || {
        let Some(mut s) = connect_with_retry(port) else {
            return Err("could not connect".to_string());
        };
        let _ = s.write_all(
            b"POST /submit?q=1 HTTP/1.1\r\nHost: example\r\nContent-Type: text/plain\r\n\
              Content-Length: 5\r\n\r\nhello",
        );
        let _ = s.set_read_timeout(Some(Duration::from_millis(5000)));
        let mut buf = String::new();
        s.read_to_string(&mut buf)
            .map_err(|e| format!("read failed: {e}"))?;
        Ok(buf)
    });

    // handler 把请求的五个字段拼成一句话回显 ⇒ 一次读完就看出形状。
    //
    // ⚠ `+` 是**二元**的 ⇒ 九个操作数（五个值 + 四个分隔符）⇒ **八层**嵌套。
    //   写成一行（括号一多，换行就看不清哪层是哪层了）：
    //   `+(+(+(+(+(+(+(+(A, s), B), s), C), s), D), s), E)`
    let body_expr = r#"+(+(+(+(+(+(+(+(AT_K(req, "method", "?"), " "), AT_K(req, "path", "?")), " "), AT_K(req, "version", "?")), " "), AT_K(AT_K(req, "headers", []), "content-type", "-")), " "), AT_K(req, "body", "?"))"#;
    let served = actual(&format!(
        r#"IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_SERVE"]);
           LET(l, TCP_LISTEN("127.0.0.1", {port}));
           TCP_SERVE(l, 1000, NULL, FUN((req),
               ["status": 200,
                "headers": ["content-type": "text/plain"],
                "body": {body_expr}]))"#
    ));
    let resp = client.join().expect("client thread").expect("client run");

    assert_eq!(
        served, "1",
        "exactly one connection must be served, got {served:?}"
    );
    assert!(
        resp.contains("POST /submit?q=1 HTTP/1.1 text/plain hello"),
        "the handler must see method/path/version/headers/body; got {resp:?}"
    );
}
