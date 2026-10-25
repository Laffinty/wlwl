//! `wlwl:std.net` 传输层成员契约（`TCP_*`）—— **项目扩展**。
//!
//! ## 这张表守的是什么
//!
//! `TCP_*` 不在 v1.0 冻结快照（附录 A）里 —— 它们是本项目为了「用 WLWL 写一个
//! HTTP 服务器」补进去的**标准库能力**。这张契约表是它们的**行为基准**：
//! 语义一旦漂移，这里先红。
//!
//! ## 三个容易写错、且写错后**看起来对**的地方
//!
//! 1. **「暂时没有」是正常值，不是错误**。`TCP_ACCEPT` 超时 ⇒ `NULL`；
//!    `TCP_RECV` 超时 ⇒ 空字节 + `eof = FALSE`。做成 `ERR` 会让「轮询式服务器」
//!    每轮都走错误分支。
//! 2. **运行期失败是 `ERR` 值，程序员错误才是诊断**。句柄不存在 ⇒ `ERR(SocketError)`；
//!    元数 / 类型不对 ⇒ `E0022` / `E0030`。
//! 3. **原生层不许认识 HTTP**。`tcp.rs` 里不得出现 `HTTP` / `GET` / `POST` /
//!    `Content-Length` 等字样 —— HTTP 解析与响应构造必须在 WLWL 侧。
//!    （`no_http_semantics_in_the_transport_layer` 机械地守着这条线。）

mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use wlwl_eval::Evaluator;

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_tcp")
}

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

// ── 形态：元数 / 类型 / 范围 ──────────────────────────────────────────────

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const SHAPE: &[Case] = &[
    Case {
        name: "listen_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.net", ["TCP_LISTEN"]); TCP_LISTEN("127.0.0.1")"#,
        expect: "!E0022 TCP_LISTEN: function expects 2 argument(s), got 1",
    },
    Case {
        name: "listen_host_non_string_is_e0030",
        src: r#"IMPORT("wlwl:std.net", ["TCP_LISTEN"]); TCP_LISTEN(5, 80)"#,
        expect: "!E0030 TCP_LISTEN: expected string, got integer",
    },
    Case {
        name: "listen_port_out_of_range_is_e0030",
        src: r#"IMPORT("wlwl:std.net", ["TCP_LISTEN"]); TCP_LISTEN("127.0.0.1", 70000)"#,
        expect: "!E0030 TCP_LISTEN: port out of range: 70000",
    },
    Case {
        name: "recv_handle_non_integer_is_e0030",
        src: r#"IMPORT("wlwl:std.net", ["TCP_RECV"]); TCP_RECV("x", 10, 10)"#,
        expect: "!E0030 TCP_RECV: expected integer, got string",
    },
    Case {
        name: "recv_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.net", ["TCP_RECV"]); TCP_RECV(1, 2)"#,
        expect: "!E0022 TCP_RECV: function expects 3 argument(s), got 2",
    },
    Case {
        name: "send_payload_type_is_e0030",
        src: r#"IMPORT("wlwl:std.net", ["TCP_SEND"]); TCP_SEND(1, 5)"#,
        expect: "!E0030 TCP_SEND: expected string or array of integers, got integer",
    },
    Case {
        name: "send_byte_out_of_range_is_e0030",
        src: r#"IMPORT("wlwl:std.net", ["TCP_SEND"]); TCP_SEND(1, [300])"#,
        expect: "!E0030 TCP_SEND: byte out of range: 300",
    },
    Case {
        name: "close_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.net", ["TCP_CLOSE"]); TCP_CLOSE(1, 2)"#,
        expect: "!E0022 TCP_CLOSE: function expects 1 argument(s), got 2",
    },
];

#[test]
fn transport_shape_cases() {
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

// ── 语义：失败形状 ────────────────────────────────────────────────────────

#[test]
fn unknown_handle_is_an_err_value_not_a_diagnostic() {
    // ⚠ ERR 若是程序的**最后一个表达式**会逃逸到顶层 ⇒ 报 E0102（语言既有行为）。
    //   所以这里必须**消费**它（`IS_ERR`）才能看见 ERR 的形状。
    let got = actual(
        r#"IMPORT("wlwl:std.net", ["TCP_ACCEPT"]);
           LET(r, TCP_ACCEPT(999999, 10));
           IF(IS_ERR(r), AT_K(ERR_PAYLOAD(r), "kind", "?"), "NOT-ERR")"#,
    );
    assert_eq!(
        got, "SocketError",
        "the payload must name the family, got {got:?}"
    );
}

#[test]
fn close_is_idempotent() {
    let got = actual(r#"IMPORT("wlwl:std.net", ["TCP_CLOSE"]); TCP_CLOSE(999999)"#);
    assert_eq!(
        got, "NULL",
        "closing an unknown handle is a no-op, got {got:?}"
    );
}

// ── 语义：超时是**正常值** ────────────────────────────────────────────────

#[test]
fn accept_timeout_returns_null() {
    let _gate = socket_gate();
    // ⚠ [v0.12.0 / Q1] 等待**一律是宿主挂起** ⇒ 等待**必须**在**任务**里
    //   （顶层求值里挂起 ⇒ `E0014`；挂起是任务的特权）。
    //
    // ⚠ **段结构**（并发册 §17.0：宿主挂起 = 不切段格 ⇒ 恢复时**段首重放**）:
    //   等待**必须是所在段的第一条语句** ⇒ `LET(c, TCP_ACCEPT(...))` 是段 0 的
    //   第一条 ⇒ 重放只会重跑这次等待（不会重复建立监听器 —— 监听在 `SCOPE` 体里）。
    let got = actual(
        r#"IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_ACCEPT"]);
           SCOPE(FUN(() ,
               LET(l, TCP_LISTEN("127.0.0.1", 0));
               LET(t, SPAWN(FUN(() ,
                   LET(c, TCP_ACCEPT(l, 50));
                   c
               )));
               LET(v, AWAIT(t));
               v
           ))"#,
    );
    assert_eq!(
        got, "NULL",
        "no client within the timeout => NULL, got {got:?}"
    );
}

#[test]
fn recv_timeout_returns_empty_bytes_and_no_eof() {
    let _gate = socket_gate();
    // 客户端连上但**不发数据** ⇒ 读超时 ⇒ 「空字节 + eof = FALSE」。
    // ⚠ 「连上了但没数据」与「没连上」是两种形状：前者是正常值，后者是 NULL。
    let port = free_port();
    let holder = thread::spawn(move || {
        if let Some(s) = connect_with_retry(port) {
            // 握住连接不发数据，等主线程那边读完就退出。
            thread::sleep(Duration::from_millis(1500));
            drop(s);
        }
    });

    let got = actual(&format!(
        r#"IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_ACCEPT", "TCP_RECV"]);
           SCOPE(FUN(() ,
               LET(l, TCP_LISTEN("127.0.0.1", {port}));
               LET(t, SPAWN(FUN(() ,
                   LET(c, TCP_ACCEPT(l, 5000));
                   YIELD();
                   LET(chunk, TCP_RECV(c, 100, 200));
                   IF(AT_K(chunk, "eof", TRUE), "EOF", +("bytes:", STR(LEN(AT_K(chunk, "bytes", [])))))
               )));
               LET(v, AWAIT(t));
               v
           ))"#
    ));
    let _ = holder.join();
    assert_eq!(
        got, "bytes:0",
        "a read timeout must be an empty result, not an error, got {got:?}"
    );
}

// ── 端到端：真 socket ─────────────────────────────────────────────────────

/// 抢一个临时端口（先占后放，让操作系统挑一个空闲的）。
/// **串行闸**：这批用例**共用端口与文件系统** ⇒ 并发跑会互相抢端口
/// （`free_port()` 只能「先占后放」⇒ 两个用例可能拿到**同一个**端口）。
/// ⚠ 仓里已有同款纪律：`resource_release_contract` 的句柄计数也是**进程级**
///   ⇒ 那五条共用一把互斥锁（否则并行跑的另一条会把差值搅脏）。
static SOCKET_GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 取那把串行闸的锁（用例开头 `let _gate = socket_gate();`）。
fn socket_gate() -> std::sync::MutexGuard<'static, ()> {
    SOCKET_GATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    let port = l.local_addr().expect("local addr").port();
    drop(l);
    port
}

/// 连上服务端 —— **必须带重试**。
///
/// ⚠⚠ **为什么必须重试(2026-10-25 CI 实测)**:本文件的形状是
///   「**先** `free_port()` 抢一个端口 ⇒ **后** WLWL 侧 `TCP_LISTEN` 绑定它」,
///   而客户端线程是**并行**跑的 ⇒ 客户端的 `connect()` 很可能**早于**监听建立
///   ⇒ 拿到 ECONNREFUSED。
///   ⇒ 客户端**只连一次**就放弃 ⇒ 服务端 `TCP_ACCEPT(l, 5000)` 只能等满超时
///   ⇒ 返回 **NULL**(本批新规约:超时的**正常值**,不是 ERR)⇒ 紧接着那句
///   `TCP_RECV(NULL, …)` 就成了 `E0030 expected integer, got null`
///   ⇒ 失败信息指向「参数类型不对」,而真因是「客户端没连上」——**指错方向**。
///   ⚠ **CI 上 ubuntu / macos 稳定复现**(客户端先跑),Windows 上不出现
///   (服务端先跑)⇒ **本机 Windows 跑绿不能当作这关过了**。
///   ⇒ 统一改成「连不上就隔 20 ms 再连,最多 100 次(≈2 s)」
///   (与同文件里本来就带重试的 `tcp_round_trip_serves_an_http_request` 同款)。
fn connect_with_retry(port: u16) -> Option<TcpStream> {
    for _ in 0..100 {
        if let Ok(s) = TcpStream::connect(("127.0.0.1", port)) {
            return Some(s);
        }
        thread::sleep(Duration::from_millis(20));
    }
    None
}

#[test]
fn tcp_round_trip_serves_an_http_request() {
    let _gate = socket_gate();
    let port = free_port();

    // 客户端线程：连上去，发一条 HTTP 请求，读响应。
    // ⚠ 连接走 `connect_with_retry`（见它的注释：客户端与 `TCP_LISTEN` 有先后竞态）。
    let client = thread::spawn(move || {
        if let Some(mut s) = connect_with_retry(port) {
            s.write_all(b"GET /probe HTTP/1.1\r\nHost: x\r\n\r\n")
                .expect("write request");
            // ⚠ [v0.12.0] 读**也要有超时** ⇒ 服务端出错时本线程**快速失败**
            //   而不是无限期挂着（否则一次失败会把整个测试挂到 CI 超时）。
            let _ = s.set_read_timeout(Some(Duration::from_millis(5000)));
            let mut buf = String::new();
            s.read_to_string(&mut buf).expect("read response");
            return buf;
        }
        String::new()
    });

    let src = format!(
        r#"
IMPORT("wlwl:std.net", ["TCP_LISTEN","TCP_ACCEPT","TCP_RECV","TCP_SEND","TCP_CLOSE"]);
SCOPE(FUN(() ,
  LET(l, TCP_LISTEN("127.0.0.1", {port}));
  LET(t, SPAWN(FUN(() ,
    LET(conn, TCP_ACCEPT(l, 5000));
    YIELD();
    LET(chunk, TCP_RECV(conn, 65536, 5000));
    YIELD();
    LET(text, AT_K(chunk, "text", ""));
    LET(body, "probe-ok");
    LET(resp, +("HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\n", body));
    TCP_SEND(conn, resp);
    TCP_CLOSE(conn);
    text
  )));
  LET(v, AWAIT(t));
  v
))
"#
    );
    let server_side = actual(&src);
    let response = client.join().expect("client thread");

    assert!(
        server_side.contains("GET /probe HTTP/1.1"),
        "the server must see the request, got {server_side:?}"
    );
    assert!(
        response.contains("200 OK"),
        "the client must get a 200, got {response:?}"
    );
    assert!(
        response.contains("probe-ok"),
        "the client must get the body, got {response:?}"
    );
}

#[test]
fn other_tasks_keep_running_while_a_socket_wait_is_pending() {
    let _gate = socket_gate();
    // ⚠ [v0.12.0 / Q1] **这条是「宿主挂起」与「阻塞冻结」的唯一可观察差别。**
    //
    // 系统册 §21.6.4（Q1 裁决后）：等待期间**挂起当前任务**，**别的任务照常运行**。
    // ⇒ 判据：等待期间**另一个任务**能不能把数据送进这条连接。
    //
    // ⚠ 旧口径（阻塞冻结整个 `SCOPE`）下 ⇒ 另一个任务**跑不了**
    //   ⇒ 这次读只能等到超时 ⇒ 读到 **0** 字节。
    // ⚠ 新口径（宿主挂起）下 ⇒ 另一个任务**跑得了** ⇒ 它的写**被这次读收到**
    //   ⇒ 读到 **5** 字节（"child"）。
    //
    // ⚠ **段结构**（并发册 §17.0：宿主挂起 = 不切段格 ⇒ 段首重放）:
    //   每次等待**独占一段**、且是**该段的第一条语句** ⇒ 重放只重跑那一次等待。
    //   ⚠ 一段里**不能有两次等待** ⇒ 第二次等待重放时会重跑第一次 ⇒ 活锁。
    let port = free_port();
    // 外部客户端：连上去，**等 300 ms 再发数据**（⇒ 父任务的读会先挂起，
    // 子任务才有时间在「等待期间」跑起来）。
    let holder = thread::spawn(move || {
        if let Some(mut s) = connect_with_retry(port) {
            thread::sleep(Duration::from_millis(300));
            let _ = s.write_all(b"hello");
            thread::sleep(Duration::from_millis(3000));
        }
    });
    let got = actual(&format!(
        r#"IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_ACCEPT", "TCP_RECV"]);
           LET MUT(ticks, 0);
           SCOPE(FUN(() ,
               LET(l, TCP_LISTEN("127.0.0.1", {port}));
               LET(t, SPAWN(FUN(() ,
                   LET(conn, TCP_ACCEPT(l, 5000));
                   YIELD();
                   LET(child, SPAWN(FUN(() , (SET(ticks, 1); 0))));
                   YIELD();
                   LET(chunk, TCP_RECV(conn, 100, 5000));
                   ticks
               )));
               LET(v, AWAIT(t));
               v
           ))"#
    ));
    let _ = holder.join();
    assert_eq!(
        got, "1",
        "another task must be able to run while a socket wait is pending \
         — the parent reads `ticks` **immediately** after its wait, so under a \
         blocking wait (the runtime frozen) the sibling could not have run yet; \
         got {got:?}"
    );
}

#[test]
fn max_bytes_of_zero_reads_zero_bytes_and_consumes_nothing() {
    let _gate = socket_gate();
    // ⚠ [v0.12.0 / R4] `max_bytes` **逐字兑现**「读至多 `max_bytes` 个字节」
    //   （系统册 §21.5）⇒ `max_bytes == 0` 读 **0** 字节。
    //   交付包原实现把它 `.max(1)` ⇒ 读走 **1** 字节 ⇒ 下面的第二条读就少了 1 字节。
    //
    // 判据：客户端发 "hello"（5 字节）⇒ 第一次读 `max_bytes = 0`（读 0 字节、
    // **不消费**）⇒ 第二次读 `max_bytes = 100` 仍应拿到 **5** 字节。
    // ⇒ 若实现把 0 抬成 1，第二次只会拿到 **4** 字节 ⇒ 这条会红。
    let port = free_port();
    let holder = thread::spawn(move || {
        if let Some(mut s) = connect_with_retry(port) {
            let _ = s.write_all(b"hello");
            thread::sleep(Duration::from_millis(3000));
        }
    });
    let got = actual(&format!(
        r#"IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_ACCEPT", "TCP_RECV"]);
           SCOPE(FUN(() ,
               LET(l, TCP_LISTEN("127.0.0.1", {port}));
               LET(t, SPAWN(FUN(() ,
                   LET(conn, TCP_ACCEPT(l, 5000));
                   YIELD();
                   LET(chunk0, TCP_RECV(conn, 0, 1000));
                   YIELD();
                   LET(chunk1, TCP_RECV(conn, 100, 2000));
                   LEN(AT_K(chunk1, "bytes", []))
               )));
               LET(v, AWAIT(t));
               v
           ))"#
    ));
    let _ = holder.join();
    assert_eq!(
        got, "5",
        "a zero-byte read must consume nothing — the next read must still see all 5 bytes; \
         got {got:?}"
    );
}

#[test]
fn tcp_set_timeout_observably_bounds_a_read() {
    let _gate = socket_gate();
    // ⚠ [v0.12.0 / R3] `TCP_SET_TIMEOUT` 设的是**该连接上的读超时**（系统册 §21.6.4）
    //   ⇒ 它**真的**给读设上界。交付包原实现只把它当**轮询粒度**用
    //   ⇒ **语义空转**：设了也没用（读仍然等满调用点的预算）。
    //
    // 判据：连接上设 **100 ms**；读给 **5000 ms** 的调用点预算。
    // ⇒ 真正生效 ⇒ 读在 ~100 ms 后就以「暂时没有」结束（**0 字节**）；
    // ⇒ 空转 ⇒ 读等满 5000 ms 才结束。
    // ⇒ 用**时间窗口**区分（100 ms vs 5000 ms 差 50 倍 ⇒ 窗口取 3000 ms 很安全）。
    let port = free_port();
    let holder = thread::spawn(move || {
        if let Some(s) = connect_with_retry(port) {
            // 握住连接**不发数据** ⇒ 读只能靠超时结束。
            thread::sleep(Duration::from_millis(3000));
            drop(s);
        }
    });
    let start = std::time::Instant::now();
    let got = actual(&format!(
        r#"IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_ACCEPT", "TCP_RECV", "TCP_SET_TIMEOUT"]);
           SCOPE(FUN(() ,
               LET(l, TCP_LISTEN("127.0.0.1", {port}));
               LET(t, SPAWN(FUN(() ,
                   LET(conn, TCP_ACCEPT(l, 5000));
                   YIELD();
                   TCP_SET_TIMEOUT(conn, 100);
                   YIELD();
                   LET(chunk, TCP_RECV(conn, 100, 5000));
                   LEN(AT_K(chunk, "bytes", []))
               )));
               LET(v, AWAIT(t));
               v
           ))"#
    ));
    let elapsed = start.elapsed().as_millis() as i64;
    let _ = holder.join();
    assert_eq!(
        got, "0",
        "a read past the connection's read timeout must be the empty shape, not an error; \
         got {got:?}"
    );
    assert!(
        elapsed < 3000,
        "the connection read timeout (100ms) must bound the read — it took {elapsed}ms, \
         i.e. the read waited for the call-site budget (5000ms) instead"
    );
}

#[test]
fn local_addr_reports_the_bound_port() {
    let got = actual(
        r#"IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_LOCAL_ADDR"]);
           LET(l, TCP_LISTEN("127.0.0.1", 0));
           AT_K(TCP_LOCAL_ADDR(l), "port", 0)"#,
    );
    let port: u16 = got.parse().expect("a port number");
    assert!(port > 0, "an ephemeral port must be reported, got {got:?}");
}

// ── 字节层（`std.encode` 的 UTF8_ENCODE / UTF8_DECODE / BYTE_LEN）─────────

#[test]
fn byte_len_counts_bytes_not_codepoints() {
    // "héllo" = 5 个码点，6 个字节。
    let got = actual(r#"IMPORT("wlwl:std.encode", ["BYTE_LEN"]); BYTE_LEN("héllo")"#);
    assert_eq!(got, "6", "BYTE_LEN must count UTF-8 bytes, got {got:?}");
}

#[test]
fn utf8_encode_decode_round_trips() {
    let got = actual(
        r#"IMPORT("wlwl:std.encode", ["UTF8_ENCODE", "UTF8_DECODE"]);
           UTF8_DECODE(UTF8_ENCODE("héllo"))"#,
    );
    assert_eq!(got, "héllo", "the round trip must be lossless, got {got:?}");
}

#[test]
fn utf8_decode_rejects_invalid_bytes() {
    // 同上：ERR 必须被消费才能看见形状（否则逃逸成 E0102）。
    let got = actual(
        r#"IMPORT("wlwl:std.encode", ["UTF8_DECODE"]);
           LET(r, UTF8_DECODE([255, 255]));
           IF(IS_ERR(r), AT_K(ERR_PAYLOAD(r), "kind", "?"), "NOT-ERR")"#,
    );
    assert_eq!(
        got, "DecodeError",
        "invalid UTF-8 must be a DecodeError, got {got:?}"
    );
}

// ── AD-5：原生层不许认识 HTTP ─────────────────────────────────────────────

/// 去掉行注释后再查 —— 注释里**可以**讨论 HTTP（那是文档），**代码里**不行。
fn strip_line_comments(src: &str) -> String {
    src.lines()
        .map(|l| {
            let mut in_str = false;
            let mut prev = '\0';
            let mut out = String::new();
            for ch in l.chars() {
                if in_str {
                    out.push(ch);
                    if ch == '"' && prev != '\\' {
                        in_str = false;
                    }
                } else if ch == '"' {
                    in_str = true;
                    out.push(ch);
                } else {
                    out.push(ch);
                }
                prev = ch;
            }
            // 去掉 `//` 之后的部分（粗但够用：字符串里的 `//` 极少出现在本文件）。
            let code = match out.find("//") {
                Some(i) => &out[..i],
                None => &out[..],
            };
            code.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn no_http_semantics_in_the_transport_layer() {
    let src = include_str!("../../wlwl-std/src/tcp.rs");
    let code = strip_line_comments(src);
    for token in [
        "HTTP",
        "GET",
        "POST",
        "PUT",
        "HEAD",
        "DELETE",
        "Content-Length",
        "Content-Type",
        "status",
        "200 OK",
    ] {
        assert!(
            !code.contains(token),
            "the transport layer must not know HTTP, but it mentions {token:?}"
        );
    }
}
