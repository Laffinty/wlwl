//! `wlwl:std.http` 成员契约 —— W-16(C-1 / C-2:HTTP 请求解析 + 响应构造)。
//!
//! ## 为什么**不走 `StdFn` 直调**
//!
//! `log_contract.rs` 那条线(`wlwl_std::resolve(path)` 拿 `ModuleSpec` → 按
//! `StdFn` 直接调)**只对 R2 原生模块成立**。`std.http` 是 **R1 纯 wlwl 源码
//! 模块**(ADR-0021 的分层规则)⇒ `resolve("wlwl:std.http")` 给的是
//! `StdBackend::Lang(&StdSource)` —— **没有** `ModuleSpec`、**没有** `StdFn`
//! 可调。
//!
//! ⇒ 本文件走**另一条**通道:照 `str_math_contract.rs` 的做法,用 `Evaluator`
//! 跑一小段 wlwl 程序,`IMPORT("wlwl:std.http", [...])` 取成员。
//!
//! ⚠ 这条通道**要求命名空间已登记**(`wlwl-std/src/lib.rs` 的 `LANG_SOURCES`
//!   + `resolve()`)—— 登记是**集成者**的活(见本项交付的「登记清单」)。
//!     未登记时本文件会得到 `E0040 module not found`,而不是**假绿**。
//!
//! ## 期望值从哪来
//!
//! 手写(不是实现输出)—— 依据是 RFC 9110 / RFC 9112 的请求与消息体规则,
//! 以及本模块文件头写下的契约。
//!
//! ⚠ **两条语言侧的坑**(写用例时踩过,别再踩):
//! 1. 运算符是**调用形态**(`+(a, b)`),**没有**中缀写法 —— `a + b` 解析不过。
//! 2. 请求体需要 **framing**:请求的体长只由 `Transfer-Encoding` /
//!    `Content-Length` 决定(RFC 9112 §6.3)—— 两者都没有 ⇒ **体长为 0**
//!    (正文是空串);「体长由连接关闭决定」是**响应**侧的规则,请求侧**没有**
//!    ⇒ 本模块**不**看 `Connection` 头(见模块文件头的 framing 规则)。

mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use wlwl_eval::Evaluator;

/// 成员面的 IMPORT 头。
const IMP: &str = r#"IMPORT("wlwl:std.http", ["PARSE_REQUEST", "BUILD_RESPONSE"]);"#;

/// 一条用例:程序 + 期望的**渲染串**。
struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

/// 跑一段程序,取它的渲染结果;失败 ⇒ `!CODE message`。
fn actual(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
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

/// 只锁码的期望:实际以 `!EXXXX` 开头且期望恰好等于 `!EXXXX` 时按前缀匹配。
fn matches(expect: &str, got: &str) -> bool {
    if expect.len() == 6 && expect.starts_with('!') && got.starts_with(expect) {
        return true;
    }
    expect == got
}

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_http")
}

/// 跑一条用例并断言。
fn check(case: &Case) {
    let got = actual(&format!("{IMP} {}", case.src));
    assert!(
        matches(case.expect, &got),
        "case: {name}\n  expected: {want}\n  actual:   {got}",
        name = case.name,
        want = case.expect,
        got = got
    );
}

// ── 请求解析 ──────────────────────────────────────────────────────────────

const CASES: &[Case] = &[
    // ── 请求行:方法 / 目标 / 版本(RFC 9112 §3)──
    Case {
        name: "the_request_line_is_split_into_method_path_and_version",
        src: r#"LET(r, PARSE_REQUEST("GET /a?b=1 HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")); +(+(+(r["method"], " "), +(r["path"], " ")), r["version"])"#,
        expect: "GET /a?b=1 HTTP/1.1",
    },
    Case {
        name: "the_path_is_kept_verbatim",
        src: r#"LET(r, PARSE_REQUEST("POST /a%20b/c?x=1&y=2 HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")); r["path"]"#,
        expect: "/a%20b/c?x=1&y=2",
    },
    Case {
        name: "the_version_is_kept_verbatim",
        src: r#"LET(r, PARSE_REQUEST("GET / HTTP/1.0\r\nHost: x\r\nConnection: close\r\n\r\n")); r["version"]"#,
        expect: "HTTP/1.0",
    },
    // ── 头名一律小写(RFC 9110 §5.1:头名大小写不敏感)──
    Case {
        name: "header_names_are_lowercased",
        src: r#"LET(r, PARSE_REQUEST("GET / HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n")); r["headers"]["host"]"#,
        expect: "example.com",
    },
    Case {
        name: "a_mixed_case_header_is_found_by_its_lowercase_name",
        src: r#"LET(r, PARSE_REQUEST("GET / HTTP/1.1\r\nX-Custom: v\r\nConnection: close\r\n\r\n")); r["headers"]["x-custom"]"#,
        expect: "v",
    },
    Case {
        name: "the_header_value_is_trimmed",
        src: r#"LET(r, PARSE_REQUEST("GET / HTTP/1.1\r\nHost:    example.com   \r\nConnection: close\r\n\r\n")); r["headers"]["host"]"#,
        expect: "example.com",
    },
    // ── 同名多值:全部保留(RFC 9110 §5.3)──
    Case {
        name: "a_repeated_header_keeps_every_value",
        src: r#"LET(r, PARSE_REQUEST("GET / HTTP/1.1\r\nSet-Cookie: a=1\r\nSet-Cookie: b=2\r\nConnection: close\r\n\r\n")); r["headers"]["set-cookie"]"#,
        expect: "[a=1, b=2]",
    },
    Case {
        name: "a_single_header_is_a_plain_string_not_an_array",
        src: r#"LET(r, PARSE_REQUEST("GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")); TYPE(r["headers"]["host"])"#,
        expect: "STRING",
    },
    // ── 请求体:按 Content-Length 取字节(RFC 9112 §6)──
    Case {
        name: "the_body_is_cut_by_content_length",
        src: r#"LET(r, PARSE_REQUEST("POST /x HTTP/1.1\r\nContent-Length: 5\r\n\r\nhelloworld")); r["body"]"#,
        expect: "hello",
    },
    Case {
        name: "an_empty_body_is_the_empty_string",
        src: r#"LET(r, PARSE_REQUEST("POST /x HTTP/1.1\r\nContent-Length: 0\r\n\r\n")); LEN(r["body"])"#,
        expect: "0",
    },
    Case {
        name: "the_body_length_is_measured_in_bytes_not_in_codepoints",
        // "héllo" = 5 个码点 / 6 个字节 ⇒ Content-Length: 6 取到整个正文。
        src: r#"LET(r, PARSE_REQUEST("POST /x HTTP/1.1\r\nContent-Length: 6\r\n\r\nhéllo")); r["body"]"#,
        expect: "héllo",
    },
    Case {
        name: "a_content_length_that_splits_a_character_is_rejected",
        // 2 字节会把 "é"(2 字节)切成两半 ⇒ 不是合法的正文边界。
        src: r#"LET(r, PARSE_REQUEST("POST /x HTTP/1.1\r\nContent-Length: 2\r\n\r\nhéllo")); LET(_, r); IF(IS_ERR(r), ERR_PAYLOAD(r)["kind"], "NOT_ERR")"#,
        expect: "HttpError",
    },
    Case {
        name: "a_truncated_body_is_rejected",
        src: r#"LET(r, PARSE_REQUEST("POST /x HTTP/1.1\r\nContent-Length: 10\r\n\r\nhello")); LET(_, r); IF(IS_ERR(r), ERR_PAYLOAD(r)["kind"], "NOT_ERR")"#,
        expect: "HttpError",
    },
    // ── chunked 请求体:服务端**拒绝**(ADR-0028 D3)──
    Case {
        name: "a_chunked_request_body_is_rejected",
        src: r#"LET(r, PARSE_REQUEST("POST /x HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n")); LET(_, r); IF(IS_ERR(r), ERR_PAYLOAD(r)["kind"], "NOT_ERR")"#,
        expect: "HttpError",
    },
    Case {
        name: "a_chunked_request_body_is_rejected_even_with_a_content_length",
        src: r#"LET(r, PARSE_REQUEST("POST /x HTTP/1.1\r\nTransfer-Encoding: chunked\r\nContent-Length: 5\r\n\r\nhello")); LET(_, r); IF(IS_ERR(r), ERR_PAYLOAD(r)["kind"], "NOT_ERR")"#,
        expect: "HttpError",
    },
    // ── 缺 framing:没有 content-length 也没有 transfer-encoding ⇒ 体长为 0 ──
    Case {
        name: "a_request_without_content_length_and_without_transfer_encoding_has_an_empty_body",
        // RFC 9112 §6.3:「A request message that does not contain
        // Transfer-Encoding or Content-Length has a body length of zero.」
        src: r#"LET(r, PARSE_REQUEST("GET / HTTP/1.1\r\nHost: x\r\n\r\n")); r["body"]"#,
        expect: "",
    },
    Case {
        name: "a_request_without_content_length_but_with_close_has_an_empty_body",
        // ⚠ 「体长由连接关闭决定」是**响应**的 framing(RFC 9112 §6.4)，
        //   **请求**侧没有这条 ⇒ `Connection: close` **不**影响请求体长度
        //   ⇒ 正文是空串,尾随的 "abc" **不是**正文(它不在任何一条 framing
        //   规则里 ⇒ 忽略)。
        src: r#"LET(r, PARSE_REQUEST("GET / HTTP/1.1\r\nConnection: close\r\n\r\nabc")); r["body"]"#,
        expect: "",
    },
    // ── 自相矛盾的 content-length:走私手法 ⇒ 拒绝 ──
    Case {
        name: "two_different_content_lengths_are_rejected",
        src: r#"LET(r, PARSE_REQUEST("POST / HTTP/1.1\r\nContent-Length: 5\r\nContent-Length: 6\r\n\r\nhello")); LET(_, r); IF(IS_ERR(r), ERR_PAYLOAD(r)["kind"], "NOT_ERR")"#,
        expect: "HttpError",
    },
    // ── 同名多值:一律拒(即便值相同)—— 与 `std.net` 同一口径 ──
    Case {
        name: "two_identical_content_lengths_are_rejected",
        // ⚠ 「同一个头出现两次」本身就是走私信号,不同实现取第一个或最后
        //   一个会分叉 ⇒ 即便值相同也拒(与 `wlwl:std.net` 同一口径)。
        //   ⚠ 收紧前这条是**绿的**(值相同 ⇒ 视为一条)⇒ 它是本次收紧的核心用例。
        src: r#"LET(r, PARSE_REQUEST("POST / HTTP/1.1\r\nContent-Length: 5\r\nContent-Length: 5\r\n\r\nhello")); LET(_, r); IF(IS_ERR(r), ERR_PAYLOAD(r)["kind"], "NOT_ERR")"#,
        expect: "HttpError",
    },
    Case {
        name: "a_duplicate_content_length_reports_the_duplicate_reason",
        // 失败文案与 `std.net` 的 "duplicate Content-Length header" 对齐。
        src: r#"LET(r, PARSE_REQUEST("POST / HTTP/1.1\r\nContent-Length: 5\r\nContent-Length: 5\r\n\r\nhello")); LET(_, r); IF(IS_ERR(r), ERR_PAYLOAD(r)["reason"], "NOT_ERR")"#,
        expect: "duplicate Content-Length header",
    },
    Case {
        name: "a_single_content_length_is_still_accepted",
        // ⚠ 反向守卫:别把**正常的单值**也拒了(收紧只针对「同名多值」)。
        src: r#"LET(r, PARSE_REQUEST("POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello!")); r["body"]"#,
        expect: "hello",
    },
    // ── 畸形请求 ──
    Case {
        name: "a_request_without_a_header_block_terminator_is_rejected",
        src: r#"LET(r, PARSE_REQUEST("GET / HTTP/1.1")); LET(_, r); IF(IS_ERR(r), ERR_PAYLOAD(r)["kind"], "NOT_ERR")"#,
        expect: "HttpError",
    },
    Case {
        name: "a_malformed_request_line_is_rejected",
        src: r#"LET(r, PARSE_REQUEST("GET /\r\nHost: x\r\n\r\n")); LET(_, r); IF(IS_ERR(r), ERR_PAYLOAD(r)["kind"], "NOT_ERR")"#,
        expect: "HttpError",
    },
    Case {
        name: "a_header_line_without_a_colon_is_rejected",
        src: r#"LET(r, PARSE_REQUEST("GET / HTTP/1.1\r\nHost x\r\n\r\n")); LET(_, r); IF(IS_ERR(r), ERR_PAYLOAD(r)["kind"], "NOT_ERR")"#,
        expect: "HttpError",
    },
];

// ── 响应构造 ──────────────────────────────────────────────────────────────

const RESP_CASES: &[Case] = &[
    // ── content-length 是**字节数**(不是码点数)──
    Case {
        name: "the_response_content_length_is_the_byte_count",
        // "héllo" = 5 个码点 / 6 个字节 ⇒ 必须是 6。
        // (若把 BYTE_LEN 写成 LEN,这里就是 5 ⇒ 必红。)
        src: r#"BUILD_RESPONSE(200, NULL, "héllo")["content_length"]"#,
        expect: "6",
    },
    Case {
        name: "the_response_content_length_header_is_the_byte_count",
        src: r#"BUILD_RESPONSE(200, NULL, "héllo")["headers"]["content-length"]"#,
        expect: "6",
    },
    Case {
        name: "the_response_content_length_is_zero_for_an_empty_body",
        src: r#"BUILD_RESPONSE(204, NULL, "")["content_length"]"#,
        expect: "0",
    },
    // ── 状态行 + 头块 ──
    Case {
        name: "the_response_head_carries_the_status_line_and_the_headers",
        src: r#"BUILD_RESPONSE(200, ["content-type": "text/plain"], "hi")["head"]"#,
        expect: "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 2\r\n\r\n",
    },
    Case {
        name: "a_multi_value_header_expands_into_several_lines",
        src: r#"BUILD_RESPONSE(200, ["set-cookie": ["a=1", "b=2"]], "")["head"]"#,
        expect:
            "HTTP/1.1 200 OK\r\nset-cookie: a=1\r\nset-cookie: b=2\r\ncontent-length: 0\r\n\r\n",
    },
    Case {
        name: "the_response_header_names_are_lowercased",
        src: r#"BUILD_RESPONSE(200, ["Content-Type": "text/plain"], "")["head"]"#,
        expect: "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: 0\r\n\r\n",
    },
    Case {
        name: "a_caller_supplied_content_length_is_overwritten_by_the_computed_one",
        src: r#"BUILD_RESPONSE(200, ["content-length": "999"], "hi")["headers"]["content-length"]"#,
        expect: "2",
    },
    // ── HEAD 语义:只发头不发体(RFC 9110 §9.3.2)──
    Case {
        name: "a_head_response_carries_the_head_only",
        src: r#"LEN(BUILD_RESPONSE(200, NULL, "hello", "HEAD")["body"])"#,
        expect: "0",
    },
    Case {
        name: "a_head_response_still_declares_the_length_of_the_body_it_withholds",
        src: r#"BUILD_RESPONSE(200, NULL, "hello", "HEAD")["content_length"]"#,
        expect: "5",
    },
    Case {
        name: "a_get_response_carries_its_body",
        src: r#"BUILD_RESPONSE(200, NULL, "hello")["body"]"#,
        expect: "hello",
    },
    // ── 失败口径:元数 / 类型错 ⇒ 既有诊断码(不新增)──
    Case {
        name: "a_non_string_request_is_e0030",
        src: r#"PARSE_REQUEST(5)"#,
        expect: "!E0030",
    },
    Case {
        name: "a_non_integer_status_is_e0030",
        src: r#"BUILD_RESPONSE("200", NULL, "hi")"#,
        expect: "!E0030",
    },
    Case {
        name: "a_non_dict_headers_argument_is_e0030",
        src: r#"BUILD_RESPONSE(200, "x", "hi")"#,
        expect: "!E0030",
    },
    Case {
        name: "a_missing_argument_is_e0022",
        src: r#"PARSE_REQUEST()"#,
        expect: "!E0022",
    },
    Case {
        name: "a_missing_response_argument_is_e0022",
        src: r#"BUILD_RESPONSE(200)"#,
        expect: "!E0022",
    },
];

/// 跑一张表。
fn run_table(cases: &[Case]) {
    for c in cases {
        check(c);
    }
}

// ── 成员面 ────────────────────────────────────────────────────────────────

/// 成员面**冻结**:十三个成员、名字一字不差。
///
/// ⚠ 这条同时钉住「`EXPORT` 必须写在一行」:`lang_exports` 是**逐行**扫的
///   (一行一条声明)⇒ 拆成多行它会扫出**零个**成员。
#[test]
fn the_member_set_is_exactly_thirteen_members() {
    let src = wlwl_std::LANG_SOURCES
        .iter()
        .find(|s| s.path == "wlwl:std.http")
        .expect("wlwl:std.http must be registered in LANG_SOURCES (the integrator's step)");
    assert_eq!(
        wlwl_std::lang_exports(src.source),
        vec![
            "PARSE_REQUEST".to_string(),
            "BUILD_RESPONSE".to_string(),
            "SERVE_STREAM".to_string(),
            "ROUTE".to_string(),
            "MATCH_ROUTE".to_string(),
            "PARSE_COOKIES".to_string(),
            "SET_COOKIE".to_string(),
            "PARSE_FORM".to_string(),
            "PARSE_MULTIPART".to_string(),
            "STATIC_FILE".to_string(),
            "GET".to_string(),
            "POST".to_string(),
            "REQUEST".to_string(),
        ],
        "the export surface of wlwl:std.http changed"
    );
}

// ── 请求解析 ──────────────────────────────────────────────────────────────

#[test]
fn the_request_is_parsed() {
    run_table(CASES);
}

// ── 响应构造 ──────────────────────────────────────────────────────────────

#[test]
fn the_response_is_built() {
    run_table(RESP_CASES);
}

/// 头块与正文的分界:`\n\n` 也认(现实里常见的非合规写法)。
///
/// ⚠ 只认 CRLFCRLF 与 LFLF **两种**;别的分隔约定不猜(那正是走私的入口)。
#[test]
fn a_lf_only_header_block_is_accepted() {
    check(&Case {
        name: "a_lf_only_header_block_is_accepted",
        src: r#"LET(r, PARSE_REQUEST("GET / HTTP/1.1\nHost: x\nConnection: close\n\n")); r["headers"]["host"]"#,
        expect: "x",
    });
}

/// 请求体的 framing 规则**只有三条**,别偷偷多出一条。
///
/// `Transfer-Encoding` 一律拒绝(不只 `chunked`)—— 任何 TE 都意味着
/// 「体长由 TE 决定」⇒ 服务端不认(ADR-0028 D3)。
#[test]
fn any_transfer_encoding_is_refused() {
    check(&Case {
        name: "any_transfer_encoding_is_refused",
        src: r#"LET(r, PARSE_REQUEST("POST / HTTP/1.1\r\nTransfer-Encoding: gzip\r\nContent-Length: 5\r\n\r\nhello")); LET(_, r); IF(IS_ERR(r), ERR_PAYLOAD(r)["kind"], "NOT_ERR")"#,
        expect: "HttpError",
    });
}

/// `Connection` 头**不影响**请求体的长度。
///
/// ⚠ 「体长由连接关闭决定」是**响应**的 framing（RFC 9112 §6.4）—— **请求**
///   侧没有这条（RFC 9112 §6.3：两者都没有 ⇒ 体长为 0）⇒ `close` /
///   `keep-alive` / `x-close` 一律**不**参与请求体的定界。
#[test]
fn the_connection_header_does_not_affect_the_request_body_length() {
    check(&Case {
        name: "x_close_does_not_make_the_body_read_until_the_connection_closes",
        src: r#"LET(r, PARSE_REQUEST("GET / HTTP/1.1\r\nConnection: x-close\r\n\r\n")); LEN(r["body"])"#,
        expect: "0",
    });
    check(&Case {
        name: "keep_alive_close_does_not_make_the_body_read_until_the_connection_closes",
        src: r#"LET(r, PARSE_REQUEST("GET / HTTP/1.1\r\nConnection: keep-alive, close\r\n\r\n")); LEN(r["body"])"#,
        expect: "0",
    });
}

// ── Keep-Alive / 分级预算 / 大小上限（W-17 · C-3 / C-8 / C-9）─────────────
//
// ## 这张表守的是什么
//
// `std.http::SERVE_STREAM(conn, opts, handler)` 在**一条**连接上**循环**处理请求：
//
// ```text
// 等首字节 → 读头块 → 读体 → 调 handler → 写响应 → （下一条请求…）
// ```
//
// 它钉住三件事：
//
// 1. **Keep-Alive（C-3）**：一条连接上**连续**处理多条请求；空闲超时后**服务端
//    主动关**；客户端说 `Connection: close` ⇒ **立即收尾**；handler 自己也能
//    选择「读完这条就关」。
// 2. **分级超时（C-8）**：首字节 / 头完成 / 体读取 / 响应写出 **各自**可配、
//    **各自**生效。
// 3. **大小上限（C-9）**：头块超限 ⇒ **431**；体超限 ⇒ **413**（**不是**把整个
//    体读进内存再判）。
//
// ## ⚠ 串行闸（**不是**可选的洁癖）
//
// socket 族的用例**共用**一把进程内互斥锁（`common::handle_guard::SERIAL`）——
// 与 `tcp_serve_contract.rs` 同款：句柄数与端口都是**进程级**的 ⇒ 并发跑会
// 互相干扰。

/// **串行闸**（socket 族共用一把进程内锁）。
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

/// 响应头块里的 `Content-Length`（缺省 0）。
fn content_length_of(head: &str) -> usize {
    for line in head.split("\r\n") {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        if k.trim().eq_ignore_ascii_case("content-length") {
            return v.trim().parse().unwrap_or(0);
        }
    }
    0
}

/// 读一条响应（头块 + 按 `Content-Length` 的体）⇒ 原文。
///
/// ⚠ 头块还没读完就 EOF ⇒ 返回**已经读到的**那些字节（够断言用）。
fn read_response(s: &mut TcpStream) -> String {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 8192];
    let head_end = loop {
        let n = s.read(&mut tmp).expect("read the response head");
        if n == 0 {
            return String::from_utf8_lossy(&buf).into_owned();
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let want = content_length_of(&head);
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < want {
        let n = s.read(&mut tmp).expect("read the response body");
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    format!("{head}\r\n\r\n{}", String::from_utf8_lossy(&body))
}

/// 回显路径的 handler（**纯计算**：不碰任何挂起点成员 —— handler 跑在**原生帧**
/// 里，那儿挂不了；碰了就是**既有 `E0014`**）。
const ECHO_HANDLER: &str = r#"FUN((req), ["status": 200, "headers": ["content-type": "text/plain"], "body": AT_K(req, "path", "?")])"#;

/// 服务端程序：`TCP_ACCEPT` 一条连接 ⇒ `SERVE_STREAM` 在**它**上面循环服务。
///
/// ⚠ `TCP_ACCEPT` 是**宿主挂起点成员** ⇒ 它的调用点必须在**任务体**里 ⇒ 它在
///   **`SPAWN(...)` 的体**里（并发册 §17.0：「任务体**自己**的顶层语句」才合法；
///   `SCOPE(...)` 的体**不是**任务体 —— 那是闭包）。
/// ⚠ `AWAIT(t)` 把调度器**驱动起来** ⇒ socket 就绪时那个任务被唤醒 ⇒ accept 返回。
/// ⚠ `SCOPE` **返回体里的值** ⇒ 拿得到 `SERVE_STREAM` 的返回值（服务过的
///   **请求数**）。
fn serve_stream(port: u16, opts: &str, handler: &str) -> String {
    let out = actual(&format!(
        r#"IMPORT("wlwl:std.net", ["TCP_LISTEN", "TCP_ACCEPT", "TCP_CLOSE_LISTENER"]);
           IMPORT("wlwl:std.http", ["SERVE_STREAM"]);
           IMPORT("wlwl:std.time", ["SLEEP"]);
           LET(l, TCP_LISTEN("127.0.0.1", {port}));
           LET(r, SCOPE(FUN(() , (
             LET(t, SPAWN(FUN(() , (
               LET(c, TCP_ACCEPT(l, 5000));
               SERVE_STREAM(c, {opts}, {handler})
             ))));
             LET(v, AWAIT(t));
             v
           ))));
           TCP_CLOSE_LISTENER(l);
           r"#
    ));
    out
}

/// **一条连接上连续处理 3 个请求**（C-3 的正面读数）。
#[test]
fn one_connection_serves_three_requests_in_a_row() {
    let _gate = socket_gate();
    let port = free_port();
    let client = thread::spawn(move || {
        let Some(mut s) = connect_with_retry(port) else {
            return Err("could not connect".to_string());
        };
        let _ = s.set_read_timeout(Some(Duration::from_millis(5000)));
        let mut seen = Vec::new();
        for i in 0..3 {
            let req = format!("GET /r{i} HTTP/1.1\r\nHost: x\r\n\r\n");
            if let Err(e) = s.write_all(req.as_bytes()) {
                return Err(format!("write {i}: {e}"));
            }
            seen.push(read_response(&mut s));
        }
        Ok(seen)
    });
    let served = serve_stream(port, r#"["idle_timeout_ms": 1000]"#, ECHO_HANDLER);
    let seen = client.join().expect("client thread").expect("client run");

    assert_eq!(
        served, "3",
        "three requests on ONE connection must be served; got {served:?}"
    );
    assert_eq!(seen.len(), 3, "the client must have got three responses");
    for (i, resp) in seen.iter().enumerate() {
        assert!(resp.contains("200 OK"), "response {i}: {resp:?}");
        assert!(
            resp.contains(&format!("/r{i}")),
            "response {i} must echo the path; got {resp:?}"
        );
    }
}

/// **空闲超时 ⇒ 服务端主动关**（C-3）。
#[test]
fn the_server_closes_the_connection_after_the_idle_timeout() {
    let _gate = socket_gate();
    let port = free_port();
    let client = thread::spawn(move || {
        let Some(mut s) = connect_with_retry(port) else {
            return Err("could not connect".to_string());
        };
        let _ = s.set_read_timeout(Some(Duration::from_millis(5000)));
        s.write_all(b"GET /one HTTP/1.1\r\nHost: x\r\n\r\n")
            .expect("write");
        let first = read_response(&mut s);
        // ⚠ 服务端空闲 ⇒ **它自己**关连接 ⇒ 这里读到 EOF（0 字节）。
        let mut rest = Vec::new();
        let n = s.read_to_end(&mut rest).expect("read to end");
        Ok((first, n))
    });
    let served = serve_stream(port, r#"["idle_timeout_ms": 300]"#, ECHO_HANDLER);
    let (first, n) = client.join().expect("client thread").expect("client run");

    assert!(
        first.contains("200 OK"),
        "the first response must arrive before the idle timeout: {first:?}"
    );
    assert_eq!(
        n, 0,
        "the server must close the connection after the idle timeout"
    );
    assert_eq!(served, "1", "one request served; got {served:?}");
}

/// **客户端说 `Connection: close` ⇒ 立即收尾**（C-3）。
#[test]
fn a_connection_close_request_ends_the_stream_immediately() {
    let _gate = socket_gate();
    let port = free_port();
    let client = thread::spawn(move || {
        let Some(mut s) = connect_with_retry(port) else {
            return Err("could not connect".to_string());
        };
        // ⚠ 读超时 **短于**服务端的空闲窗口（8 s）⇒ 「服务端没立刻关」会**超时**
        //   （而不是悄悄通过）。
        let _ = s.set_read_timeout(Some(Duration::from_millis(3000)));
        s.write_all(b"GET /bye HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
            .expect("write");
        let first = read_response(&mut s);
        let mut rest = Vec::new();
        let n = s.read_to_end(&mut rest).expect("read to end");
        Ok((first, n))
    });
    let served = serve_stream(port, r#"["idle_timeout_ms": 8000]"#, ECHO_HANDLER);
    let (first, n) = client.join().expect("client thread").expect("client run");

    assert!(
        first.contains("200 OK"),
        "the response must arrive: {first:?}"
    );
    assert_eq!(
        n, 0,
        "the server must close the connection right after the response"
    );
    assert_eq!(served, "1", "one request served; got {served:?}");
}

/// **handler 自己选择「读完这条就关」**（C-3：`keep_alive: FALSE`）。
#[test]
fn the_handler_can_choose_to_stop_the_stream() {
    let _gate = socket_gate();
    let port = free_port();
    let client = thread::spawn(move || {
        let Some(mut s) = connect_with_retry(port) else {
            return Err("could not connect".to_string());
        };
        let _ = s.set_read_timeout(Some(Duration::from_millis(5000)));
        s.write_all(b"GET /first HTTP/1.1\r\nHost: x\r\n\r\n")
            .expect("write");
        let first = read_response(&mut s);
        // ⚠ 第二条请求：**写不写**都无所谓（服务端已经关了）⇒ 写失败不算错。
        let _ = s.write_all(b"GET /second HTTP/1.1\r\nHost: x\r\n\r\n");
        let second = read_response(&mut s);
        Ok((first, second))
    });
    let served = serve_stream(
        port,
        r#"["idle_timeout_ms": 1000]"#,
        r#"FUN((req), ["status": 200, "headers": DICT(), "body": "one", "keep_alive": FALSE])"#,
    );
    let (first, second) = client.join().expect("client thread").expect("client run");

    assert!(
        first.contains("200 OK"),
        "the first response must arrive: {first:?}"
    );
    assert!(
        second.is_empty(),
        "the handler asked to stop ⇒ the second request must not be served; got {second:?}"
    );
    assert_eq!(served, "1", "only the first request counts; got {served:?}");
}

/// **头块超上限 ⇒ 431**（C-9）。
#[test]
fn a_header_block_over_the_cap_gets_431() {
    let _gate = socket_gate();
    let port = free_port();
    let client = thread::spawn(move || {
        let Some(mut s) = connect_with_retry(port) else {
            return Err("could not connect".to_string());
        };
        let _ = s.set_read_timeout(Some(Duration::from_millis(5000)));
        let big = format!("X-Big: {}", "a".repeat(600));
        let req = format!("GET / HTTP/1.1\r\nHost: x\r\n{big}\r\n\r\n");
        s.write_all(req.as_bytes()).expect("write");
        Ok(read_response(&mut s))
    });
    let served = serve_stream(
        port,
        r#"["max_header_bytes": 200, "idle_timeout_ms": 500]"#,
        ECHO_HANDLER,
    );
    let resp = client.join().expect("client thread").expect("client run");

    assert!(
        resp.contains("431"),
        "a header block over the cap must get 431; got {resp:?}"
    );
    assert!(
        !resp.contains("200 OK"),
        "the handler must not have run for an over-sized header block; got {resp:?}"
    );
    assert_eq!(
        served, "0",
        "the over-sized request must not count as served"
    );
}

/// **体超上限 ⇒ 413**（C-9）。
#[test]
fn a_body_over_the_cap_gets_413() {
    let _gate = socket_gate();
    let port = free_port();
    let client = thread::spawn(move || {
        let Some(mut s) = connect_with_retry(port) else {
            return Err("could not connect".to_string());
        };
        let _ = s.set_read_timeout(Some(Duration::from_millis(5000)));
        let req = format!(
            "POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 100\r\n\r\n{}",
            "b".repeat(100)
        );
        s.write_all(req.as_bytes()).expect("write");
        Ok(read_response(&mut s))
    });
    let served = serve_stream(
        port,
        r#"["max_body_bytes": 10, "idle_timeout_ms": 500]"#,
        ECHO_HANDLER,
    );
    let resp = client.join().expect("client thread").expect("client run");

    assert!(
        resp.contains("413"),
        "a body over the cap must get 413; got {resp:?}"
    );
    assert!(
        !resp.contains("200 OK"),
        "the handler must not have run for an over-sized body; got {resp:?}"
    );
    assert_eq!(
        served, "0",
        "the over-sized request must not count as served"
    );
}

/// **首字节预算用尽 ⇒ 408**（C-8 的第一档）。
#[test]
fn the_first_byte_budget_expires_with_408() {
    let _gate = socket_gate();
    let port = free_port();
    let client = thread::spawn(move || {
        let Some(mut s) = connect_with_retry(port) else {
            return Err("could not connect".to_string());
        };
        let _ = s.set_read_timeout(Some(Duration::from_millis(5000)));
        // ⚠ **一个字节都不发** ⇒ 服务端在首字节预算处放弃。
        Ok(read_response(&mut s))
    });
    let served = serve_stream(port, r#"["first_byte_ms": 300]"#, ECHO_HANDLER);
    let resp = client.join().expect("client thread").expect("client run");

    assert!(
        resp.contains("408"),
        "the first-byte budget must expire with a 408; got {resp:?}"
    );
    assert_eq!(served, "0", "no request was served; got {served:?}");
}

/// **头完成预算用尽 ⇒ 408**（C-8 的第二档）。
#[test]
fn the_header_budget_expires_with_408() {
    let _gate = socket_gate();
    let port = free_port();
    let client = thread::spawn(move || {
        let Some(mut s) = connect_with_retry(port) else {
            return Err("could not connect".to_string());
        };
        let _ = s.set_read_timeout(Some(Duration::from_millis(5000)));
        // ⚠ 首字节到了，但头块**没写完**（没有空行）⇒ 头完成预算处放弃。
        s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n")
            .expect("write");
        Ok(read_response(&mut s))
    });
    let served = serve_stream(port, r#"["headers_ms": 300]"#, ECHO_HANDLER);
    let resp = client.join().expect("client thread").expect("client run");

    assert!(
        resp.contains("408"),
        "the header budget must expire with a 408; got {resp:?}"
    );
    assert_eq!(served, "0", "no request was served; got {served:?}");
}

/// **体读取预算用尽 ⇒ 408**（C-8 的第三档）。
#[test]
fn the_body_budget_expires_with_408() {
    let _gate = socket_gate();
    let port = free_port();
    let client = thread::spawn(move || {
        let Some(mut s) = connect_with_retry(port) else {
            return Err("could not connect".to_string());
        };
        let _ = s.set_read_timeout(Some(Duration::from_millis(5000)));
        // ⚠ 头写完了，但体**只到一半**（`Content-Length: 10`，实到 3）⇒ 体预算处放弃。
        s.write_all(b"POST / HTTP/1.1\r\nContent-Length: 10\r\n\r\nabc")
            .expect("write");
        Ok(read_response(&mut s))
    });
    let served = serve_stream(port, r#"["body_ms": 300]"#, ECHO_HANDLER);
    let resp = client.join().expect("client thread").expect("client run");

    assert!(
        resp.contains("408"),
        "the body budget must expire with a 408; got {resp:?}"
    );
    assert_eq!(served, "0", "no request was served; got {served:?}");
}

/// **响应写出预算用尽 ⇒ 放弃这次响应**（C-8 的第四档）。
#[test]
fn the_response_budget_expires_and_the_response_is_dropped() {
    let _gate = socket_gate();
    let port = free_port();
    let client = thread::spawn(move || {
        let Some(mut s) = connect_with_retry(port) else {
            return Err("could not connect".to_string());
        };
        let _ = s.set_read_timeout(Some(Duration::from_millis(5000)));
        s.write_all(b"GET /slow HTTP/1.1\r\nHost: x\r\n\r\n")
            .expect("write");
        let mut buf = String::new();
        let n = s.read_to_string(&mut buf).expect("read to end");
        Ok((n, buf))
    });
    // ⚠ handler 睡 200 ms > `response_ms` 50 ⇒ **响应阶段**预算用尽 ⇒ 不写响应。
    let served = serve_stream(
        port,
        r#"["response_ms": 50]"#,
        r#"FUN((req), (SLEEP(200); ["status": 200, "headers": DICT(), "body": "late"]))"#,
    );
    let (n, buf) = client.join().expect("client thread").expect("client run");

    assert_eq!(
        n, 0,
        "the response must NOT be written once the respond budget is exhausted; got {buf:?}"
    );
    assert_eq!(served, "0", "the request was not served; got {served:?}");
}
