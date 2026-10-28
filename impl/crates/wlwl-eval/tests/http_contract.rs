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
//!   未登记时本文件会得到 `E0040 module not found`,而不是**假绿**。
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

use std::path::PathBuf;
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

/// 成员面**冻结**:两个成员、名字一字不差。
///
/// ⚠ 这条同时钉住「`EXPORT` 必须写在一行」:`lang_exports` 是**逐行**扫的
///   (一行一条声明)⇒ 拆成多行它会扫出**零个**成员。
#[test]
fn the_member_set_is_exactly_two_members() {
    let src = wlwl_std::LANG_SOURCES
        .iter()
        .find(|s| s.path == "wlwl:std.http")
        .expect("wlwl:std.http must be registered in LANG_SOURCES (the integrator's step)");
    assert_eq!(
        wlwl_std::lang_exports(src.source),
        vec!["PARSE_REQUEST".to_string(), "BUILD_RESPONSE".to_string()],
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
