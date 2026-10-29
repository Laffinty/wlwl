//! [W-20] `std.http::GET` / `POST` / `REQUEST` 的契约 —— 客户端 chunked 解码
//! （需求 C-11）与 HTTPS 出站（需求 A-2）。
//!
//! ## 这张契约表守的是什么
//!
//! 三个**新**公共成员落在 **`std.http`**（**不是** `std.net` —— 既有成员的语义
//! 一个字都不许改，`std.net::HTTP_GET` 的 `https://` ⇒ ERR 那一刀也不许动）：
//!
//! | 成员 | 实参 | 返回 |
//! |---|---|---|
//! | `GET` | `(url, opts?)` | `DICT {status, headers, body}` |
//! | `POST` | `(url, body, opts?)` | 同上 |
//! | `REQUEST` | `(method, url, body?, opts?)` | 同上 |
//!
//! 两条能力差异（相对 `std.net` 的既有成员）：
//!
//! 1. **客户端能解 chunked**（C-11）—— `Transfer-Encoding: chunked` 的响应
//!    **解码之后**交给调用方。⚠ **只改客户端**：服务端仍然拒收 chunked 请求体。
//! 2. **能走 `https://`**（A-2）—— 经 `tls` 命名空间的运行期扩展。
//!
//! ## ⚠ 为什么本文件**直调 R2 内核**
//!
//! `std.http` 是 **R1 模块**（`ADR-0021` 的分层：R1 门面 + R2 内核）⇒ 它的成员
//! 由 `http.wll` 的 `EXPORT` 声明，实现是 R2 内核
//! （`wlwl_std::net::http_client_*`）⇒ **接线还没落地**时，门面那层还调不到。
//! ⇒ 本文件测的是**内核**（真正干活的那一层）；接线是**另一件事**（接线说明见
//! 交付件的 ② 段）。
//!
//! ## ⚠ 绝不访问外网
//!
//! 所有夹具都绑 `127.0.0.1:0` ⇒ **一个外部地址都不碰**（与仓里既有的
//! `the_contract_never_touches_an_external_address` 同款纪律）。
//!
//! ## ⚠ 反向守卫（chunked 解码）
//!
//! `the_chunked_response_is_decoded` 那条用例**只在解码器真的解码时**才绿：
//! 把内核里的 chunked 分支去掉（改成「`Transfer-Encoding` 一律拒」），它
//! **必须红**；改回来必须绿。详见交付件的 ⑤ 段。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::rc::Rc;

use wlwl_error::{ErrorCode, Location, WlwlDiagnostic};
use wlwl_std::net::{http_client_get, http_client_post, http_client_request};
use wlwl_value::{ExtMember, Outcome, StdCtx, StdHost, Value};

// ── 宿主 ────────────────────────────────────────────────────────────────────

/// 一个**只**够本文件用的宿主。
///
/// ⚠ `ctx()` / `call()` 都是 **panic** —— 那是**故意的断言**：这三个成员
///   既不读宿主上下文，也不回调宿主（它们只**问**宿主「`tls:CONNECT` 在不在」）。
struct Host {
    /// 假的运行期扩展成员（限定名 → 成员）。
    ext: Vec<(String, ExtMember)>,
}

impl Host {
    /// 没有扩展（⇒ `https://` 必然撞「no TLS provider loaded」）。
    fn plain() -> Self {
        Host { ext: Vec::new() }
    }

    /// 装上一组假的 `tls` 成员。
    fn with_tls(members: Vec<(&str, ExtMember)>) -> Self {
        Host {
            ext: members
                .into_iter()
                .map(|(q, m)| (q.to_string(), m))
                .collect(),
        }
    }
}

impl StdHost for Host {
    fn ctx(&mut self) -> &mut StdCtx {
        panic!("the client members must not touch StdCtx")
    }

    fn call(
        &mut self,
        _f: &Value,
        _args: Vec<Value>,
        _name: &str,
    ) -> Result<Outcome, wlwl_error::WlwlError> {
        panic!("the client members must not call back into the host")
    }

    fn ext_member(&mut self, qualified: &str) -> Option<ExtMember> {
        self.ext
            .iter()
            .find(|(q, _)| q == qualified)
            .map(|(_, m)| m.clone())
    }

    fn diag(&mut self, code: ErrorCode, message: String) -> wlwl_error::WlwlError {
        WlwlDiagnostic::new(code, message, Location::point("<test>", 0, 0)).into()
    }
}

/// 造一个假的扩展成员。
fn fake_ext(
    name: &str,
    f: impl Fn(Vec<Value>) -> Result<Outcome, wlwl_error::WlwlError> + 'static,
) -> ExtMember {
    ExtMember {
        name: name.to_string(),
        qualified: format!("tls:{name}"),
        arity_min: 1,
        arity_max: 3,
        call: Rc::new(f),
    }
}

/// 一个假的 `tls` 提供方：`CONNECT` 交回给定的 `ERR` 载荷。
///
/// ⚠ **其余三个成员**（`SEND` / `RECV` / `CLOSE`）也给上 —— 内核在调
///   `CONNECT` **之前**就把四个成员都取好了（缺一个 ⇒ 另一个错误）。
fn fake_tls_provider(connect_result: Value) -> Vec<(&'static str, ExtMember)> {
    let connect = fake_ext("CONNECT", move |_args| {
        Ok(Outcome::normal(connect_result.clone()))
    });
    let ok = fake_ext("SEND", |_args| Ok(Outcome::normal(Value::Integer(0))));
    let recv = fake_ext("RECV", |_args| {
        Ok(Outcome::normal(Value::dict(vec![
            (Value::String("bytes".into()), Value::Array(Vec::new())),
            (Value::String("text".into()), Value::String(String::new())),
            (Value::String("eof".into()), Value::Boolean(true)),
        ])))
    });
    let close = fake_ext("CLOSE", |_args| Ok(Outcome::normal(Value::Null)));
    vec![
        ("tls:CONNECT", connect),
        ("tls:SEND", ok),
        ("tls:RECV", recv),
        ("tls:CLOSE", close),
    ]
}

// ── 本机夹具 ────────────────────────────────────────────────────────────────

/// 起一个**本机** HTTP 夹具：把 `response` 原样写回去，然后关连接。
///
/// ⚠ 客户端会先写请求；本夹具**不读**它（请求很小 ⇒  socket 缓冲区吃得下 ⇒
///   不会死锁）。
fn serve_once(response: Vec<u8>) -> u16 {
    serve_pieces(vec![response])
}

/// 起一个本机夹具：把 `pieces` **分多次**写回去（每次之间歇 20 ms）。
///
/// ⚠ 这是「块边界与 TCP 段边界无关」那条用例的**关键**：分片点落在块大小行的
///   中间 ⇒ 解码器必须是**增量**的。
fn serve_pieces(pieces: Vec<Vec<u8>>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1");
    let port = listener.local_addr().expect("local addr").port();
    std::thread::spawn(move || {
        let Ok((mut sock, _)) = listener.accept() else {
            return;
        };
        for piece in pieces {
            if sock.write_all(&piece).is_err() {
                return;
            }
            let _ = sock.flush();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    });
    port
}

/// 起一个本机夹具：把**收到的请求**原样回显成响应体。
fn serve_echo() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1");
    let port = listener.local_addr().expect("local addr").port();
    std::thread::spawn(move || {
        let Ok((mut sock, _)) = listener.accept() else {
            return;
        };
        let mut buf = [0u8; 4096];
        let mut got: Vec<u8> = Vec::new();
        // 读到头块结束。
        let mut head_end = got.len();
        loop {
            let Ok(n) = sock.read(&mut buf) else {
                return;
            };
            if n == 0 {
                break;
            }
            got.extend_from_slice(&buf[..n]);
            if let Some(i) = got.windows(4).position(|w| w == b"\r\n\r\n") {
                head_end = i + 4;
                break;
            }
        }
        // 再读到体结束（`Content-Length`）。
        let head = String::from_utf8_lossy(&got[..head_end]).to_string();
        let len: usize = head
            .lines()
            .find_map(|l| {
                let (k, v) = l.split_once(':')?;
                if k.trim().eq_ignore_ascii_case("content-length") {
                    v.trim().parse().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);
        while got.len() < head_end + len {
            let Ok(n) = sock.read(&mut buf) else {
                return;
            };
            if n == 0 {
                break;
            }
            got.extend_from_slice(&buf[..n]);
        }
        let echo = String::from_utf8_lossy(&got).to_string();
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{echo}",
            echo.len()
        );
        let _ = sock.write_all(resp.as_bytes());
        let _ = sock.flush();
    });
    port
}

// ── 取值助手 ────────────────────────────────────────────────────────────────

fn dict_get<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Dict(inner) => inner.get(&Value::String(key.to_string())),
        _ => None,
    }
}

/// `ERR` 的载荷（不是 `ERR` ⇒ `None`）。
fn err_payload(v: &Value) -> Option<&Value> {
    match v {
        Value::Err(inner) => Some(inner.as_ref()),
        _ => None,
    }
}

/// 响应里的 `status`。
fn status_of(v: &Value) -> i64 {
    match dict_get(v, "status") {
        Some(Value::Integer(n)) => *n,
        other => panic!("the response has no integer `status`: {other:?}"),
    }
}

/// 响应里的 `body`（**文本**响应）。
fn body_of(v: &Value) -> String {
    match dict_get(v, "body") {
        Some(Value::String(s)) => s.clone(),
        other => panic!("the response has no string `body`: {other:?}"),
    }
}

/// 响应里的某个头（**单值**）。
fn header_of(v: &Value, name: &str) -> String {
    let headers = dict_get(v, "headers").expect("the response has no `headers`");
    match dict_get(headers, name) {
        Some(Value::String(s)) => s.clone(),
        other => panic!("the response has no string header {name:?}: {other:?}"),
    }
}

/// `ERR` 载荷里的某个键（**字符串**）。
fn err_field(v: &Value, key: &str) -> String {
    let payload = err_payload(v).expect("the result is not an ERR value");
    match dict_get(payload, key) {
        Some(Value::String(s)) => s.clone(),
        other => panic!("the ERR payload has no string {key:?}: {other:?}"),
    }
}

/// `ERR` 载荷的**键**（用来断言「只有 kind 与 reason」）。
fn err_keys(v: &Value) -> Vec<String> {
    let payload = err_payload(v).expect("the result is not an ERR value");
    match payload {
        Value::Dict(inner) => inner
            .entries()
            .iter()
            .map(|(k, _)| match k {
                Value::String(s) => s.clone(),
                other => format!("{other:?}"),
            })
            .collect(),
        Value::DictCell(cell) => cell
            .inner_rc()
            .entries()
            .iter()
            .map(|(k, _)| match k {
                Value::String(s) => s.clone(),
                other => format!("{other:?}"),
            })
            .collect(),
        other => panic!("the ERR payload is not a dict: {other:?}"),
    }
}

// ── 调用助手 ────────────────────────────────────────────────────────────────

fn url_of(port: u16, path: &str) -> String {
    format!("http://127.0.0.1:{port}{path}")
}

fn run_get(port: u16, opts: Option<Value>) -> Value {
    let mut host = Host::plain();
    let mut args = vec![Value::String(url_of(port, "/"))];
    if let Some(o) = opts {
        args.push(o);
    }
    http_client_get(&mut host, args)
        .expect("GET must not raise a native diagnostic")
        .value
}

fn run_post(port: u16, body: &str, opts: Option<Value>) -> Value {
    let mut host = Host::plain();
    let mut args = vec![
        Value::String(url_of(port, "/submit")),
        Value::String(body.to_string()),
    ];
    if let Some(o) = opts {
        args.push(o);
    }
    http_client_post(&mut host, args)
        .expect("POST must not raise a native diagnostic")
        .value
}

fn run_request(method: &str, port: u16, args_tail: Vec<Value>) -> Value {
    let mut host = Host::plain();
    let mut args = vec![
        Value::String(method.to_string()),
        Value::String(url_of(port, "/")),
    ];
    args.extend(args_tail);
    http_client_request(&mut host, args)
        .expect("REQUEST must not raise a native diagnostic")
        .value
}

// ── 用例 ────────────────────────────────────────────────────────────────────

#[test]
fn a_plain_response_with_a_content_length_is_returned() {
    let port = serve_once(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello".to_vec(),
    );
    let out = run_get(port, None);
    assert_eq!(status_of(&out), 200);
    assert_eq!(body_of(&out), "hello");
    assert_eq!(header_of(&out, "content-type"), "text/plain");
}

#[test]
fn the_chunked_response_is_decoded() {
    let port = serve_once(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n"
            .to_vec(),
    );
    let out = run_get(port, None);
    assert_eq!(status_of(&out), 200);
    assert_eq!(body_of(&out), "hello world");
    assert_eq!(header_of(&out, "transfer-encoding"), "chunked");
}

#[test]
fn the_chunk_boundaries_do_not_have_to_match_the_tcp_segments() {
    // 分片点落在**块大小行的中间**与**块数据的中间** ⇒ 解码器必须增量。
    let port = serve_pieces(vec![
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhel".to_vec(),
        b"lo\r\n6\r\n wo".to_vec(),
        b"rld\r\n0\r\n\r\n".to_vec(),
    ]);
    let out = run_get(port, None);
    assert_eq!(status_of(&out), 200);
    assert_eq!(body_of(&out), "hello world");
}

#[test]
fn the_chunk_extensions_and_the_trailer_section_are_accepted() {
    let port = serve_once(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5;name=value\r\nhello\r\n0\r\nx-checksum: 1234\r\n\r\n"
            .to_vec(),
    );
    let out = run_get(port, None);
    assert_eq!(body_of(&out), "hello");
    // trailer 段的头**并进**头表。
    assert_eq!(header_of(&out, "x-checksum"), "1234");
}

#[test]
fn a_malformed_chunk_size_is_refused() {
    let port = serve_once(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\nhello\r\n0\r\n\r\n".to_vec(),
    );
    let out = run_get(port, None);
    assert_eq!(err_field(&out, "kind"), "HttpError");
    assert!(
        err_field(&out, "reason").contains("malformed chunk size"),
        "the reason must point at the malformed chunk size: {:?}",
        err_field(&out, "reason")
    );
}

#[test]
fn the_duplicate_content_length_in_a_response_is_refused() {
    // ⚠ 值**相同**也拒 —— 「同一个头出现两次」本身就是走私信号。
    let port = serve_once(
        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nContent-Length: 5\r\n\r\nhello".to_vec(),
    );
    let out = run_get(port, None);
    assert_eq!(err_field(&out, "kind"), "HttpError");
    assert_eq!(err_field(&out, "reason"), "duplicate Content-Length header");
}

#[test]
fn the_transfer_encoding_and_the_content_length_together_are_refused() {
    let port = serve_once(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 5\r\n\r\n5\r\nhello\r\n0\r\n\r\n"
            .to_vec(),
    );
    let out = run_get(port, None);
    assert_eq!(err_field(&out, "kind"), "HttpError");
    assert!(
        err_field(&out, "reason").contains("both Transfer-Encoding and Content-Length"),
        "the reason must point at the smuggling shape: {:?}",
        err_field(&out, "reason")
    );
}

#[test]
fn the_duplicate_content_length_in_a_request_is_refused() {
    // ⚠ 端口 1 上**没有**监听者 ⇒ 若实现是「先连后拼」，这里会撞连接错误 ⇒
    //   这条用例顺带守住了「内容错就别去碰网络」这条次序。
    let opts = Value::dict(vec![(
        Value::String("headers".into()),
        Value::dict(vec![
            (
                Value::String("content-length".into()),
                Value::String("5".into()),
            ),
            (
                Value::String("Content-Length".into()),
                Value::String("5".into()),
            ),
        ]),
    )]);
    let mut host = Host::plain();
    let out = http_client_get(
        &mut host,
        vec![Value::String("http://127.0.0.1:1/".into()), opts],
    )
    .expect("GET must not raise a native diagnostic")
    .value;
    assert_eq!(err_field(&out, "kind"), "HttpError");
    assert_eq!(err_field(&out, "reason"), "duplicate Content-Length header");
}

#[test]
fn the_post_member_sends_the_body() {
    let port = serve_echo();
    let out = run_post(port, "hello", None);
    let echoed = body_of(&out);
    assert!(
        echoed.starts_with("POST /submit HTTP/1.1\r\n"),
        "the request line must be the POST line: {echoed:?}"
    );
    assert!(
        echoed.contains("Content-Length: 5\r\n"),
        "the request must declare its body length: {echoed:?}"
    );
    assert!(
        echoed.ends_with("hello"),
        "the body must reach the server: {echoed:?}"
    );
}

#[test]
fn the_request_member_takes_the_method_as_an_argument() {
    let port = serve_echo();
    let out = run_request("PUT", port, vec![Value::String("body!".into())]);
    let echoed = body_of(&out);
    assert!(
        echoed.starts_with("PUT / HTTP/1.1\r\n"),
        "the request line must carry the given method: {echoed:?}"
    );
    assert!(
        echoed.ends_with("body!"),
        "the body must reach the server: {echoed:?}"
    );
}

#[test]
fn the_method_option_must_match_the_member() {
    let opts = Value::dict(vec![(
        Value::String("method".into()),
        Value::String("POST".into()),
    )]);
    let mut host = Host::plain();
    let out = http_client_get(
        &mut host,
        vec![Value::String("http://127.0.0.1:1/".into()), opts],
    )
    .expect("GET must not raise a native diagnostic")
    .value;
    assert_eq!(err_field(&out, "kind"), "HttpError");
    assert!(
        err_field(&out, "reason").contains("does not match"),
        "the reason must point at the method mismatch: {:?}",
        err_field(&out, "reason")
    );
}

#[test]
fn the_same_header_twice_keeps_both_values() {
    let port = serve_once(
        b"HTTP/1.1 200 OK\r\nSet-Cookie: a=1\r\nSet-Cookie: b=2\r\nContent-Length: 0\r\n\r\n"
            .to_vec(),
    );
    let out = run_get(port, None);
    let headers = dict_get(&out, "headers").expect("the response has no `headers`");
    match dict_get(headers, "set-cookie") {
        Some(Value::Array(items)) => {
            let values: Vec<&str> = items
                .iter()
                .map(|v| match v {
                    Value::String(s) => s.as_str(),
                    other => panic!("not a string: {other:?}"),
                })
                .collect();
            assert_eq!(values, vec!["a=1", "b=2"]);
        }
        other => panic!("a repeated header must be an array: {other:?}"),
    }
}

#[test]
fn the_https_url_without_a_tls_provider_is_refused() {
    let mut host = Host::plain();
    let out = http_client_get(
        &mut host,
        vec![Value::String("https://127.0.0.1:1/".into())],
    )
    .expect("GET must not raise a native diagnostic")
    .value;
    assert_eq!(err_field(&out, "kind"), "TlsError");
    assert_eq!(err_field(&out, "reason"), "no TLS provider loaded");
    // ⚠ 形状**逐字**：`kind` 与 `reason` 两个键，**没有** `op`。
    assert_eq!(
        err_keys(&out),
        vec!["kind".to_string(), "reason".to_string()]
    );
}

#[test]
fn the_certificate_error_is_passed_through_verbatim() {
    let payload = Value::dict(vec![
        (
            Value::String("kind".into()),
            Value::String("TlsError".into()),
        ),
        (Value::String("op".into()), Value::String("CONNECT".into())),
        (
            Value::String("reason".into()),
            Value::String("certificate expired".into()),
        ),
    ]);
    let mut host = Host::with_tls(fake_tls_provider(Value::Err(Box::new(payload))));
    let out = http_client_get(
        &mut host,
        vec![Value::String("https://127.0.0.1:1/".into())],
    )
    .expect("GET must not raise a native diagnostic")
    .value;
    assert_eq!(err_field(&out, "kind"), "TlsError");
    // ⚠ **逐字**透传：一个字都不改（不改措辞、不加前缀）。
    assert_eq!(err_field(&out, "reason"), "certificate expired");
    assert_eq!(
        err_keys(&out),
        vec!["kind".to_string(), "reason".to_string()]
    );
}

#[test]
fn the_unknown_option_name_is_a_native_diagnostic() {
    let mut host = Host::plain();
    let opts = Value::dict(vec![(
        Value::String("timeuot_ms".into()),
        Value::Integer(10),
    )]);
    let err = http_client_get(
        &mut host,
        vec![Value::String("http://127.0.0.1:1/".into()), opts],
    )
    .expect_err("an unknown option name must be a native diagnostic");
    assert_eq!(err.diagnostic().code, ErrorCode::E0030);
    assert!(
        err.diagnostic().message.contains("unknown option"),
        "the message must name the unknown option: {}",
        err.diagnostic().message
    );
}

#[test]
fn the_wrong_arity_is_a_native_diagnostic() {
    let mut host = Host::plain();
    let err = http_client_get(&mut host, Vec::new())
        .expect_err("GET without a url must be a native diagnostic");
    assert_eq!(err.diagnostic().code, ErrorCode::E0022);
}
