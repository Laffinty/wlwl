//! `wlwl:std.net` 成员契约 —— RFC 3986(v0.11.3 M6 / addendum-05 W-02)。
//!
//! ## 本表的权威是 **RFC 3986 原文**,不是本实现
//!
//! [`RESOLUTION`] 那 **42 条**由脚本从 `https://www.rfc-editor.org/rfc/rfc3986.txt`
//! 的 §5.4 正文**解析产出**,不是手抄、也不是从本实现输出抄的 —— 手抄必然
//! 会抄错,而从实现输出抄就是自证。改实现前请先对着 RFC 看这张表。
//!
//! 基准 URI 是 RFC §5.4 正文的原话:`http://a/b/c/d;p?q`。
//!
//! ## 三条容易写错、且写错后**看起来对**的地方
//!
//! 1. **undefined ≠ empty**(§5.3 末段)。分隔符没出现是 `NULL`,出现了但后面
//!    立刻结束是 `""`。第 38–41 条(`g?y/./x` 一族)全靠这个区别:混同了就分不
//!    开 query 与 path,`remove_dot_segments` 会去动 query 里的 `/./`。
//! 2. **strict 模式**。§5.4.2 的 `http:g` 给了两个结果,RFC 把 `http:g` 排在
//!    **前面**(`; for strict parsers`),本实现取它。WHATWG URL 取的是另一个,
//!    两者**不等价** —— 见 `src/net.rs` 的 `resolve`。
//! 3. **`..` 不能改 authority**。第 24/25 条(`../../../g`)回退到根后**不再往上删**。

use std::path::PathBuf;
use wlwl_eval::Evaluator;

/// RFC 3986 §5.4 的参考用例,base = `http://a/b/c/d;p?q`。
/// **逐字取自 RFC 原文**;末条 `http:g` 取 RFC 标注的 strict 解析结果。
const RESOLUTION: &[(&str, &str)] = &[
    ("g:h", "g:h"),
    ("g", "http://a/b/c/g"),
    ("./g", "http://a/b/c/g"),
    ("g/", "http://a/b/c/g/"),
    ("/g", "http://a/g"),
    ("//g", "http://g"),
    ("?y", "http://a/b/c/d;p?y"),
    ("g?y", "http://a/b/c/g?y"),
    ("#s", "http://a/b/c/d;p?q#s"),
    ("g#s", "http://a/b/c/g#s"),
    ("g?y#s", "http://a/b/c/g?y#s"),
    (";x", "http://a/b/c/;x"),
    ("g;x", "http://a/b/c/g;x"),
    ("g;x?y#s", "http://a/b/c/g;x?y#s"),
    ("", "http://a/b/c/d;p?q"),
    (".", "http://a/b/c/"),
    ("./", "http://a/b/c/"),
    ("..", "http://a/b/"),
    ("../", "http://a/b/"),
    ("../g", "http://a/b/g"),
    ("../..", "http://a/"),
    ("../../", "http://a/"),
    ("../../g", "http://a/g"),
    ("../../../g", "http://a/g"),
    ("../../../../g", "http://a/g"),
    ("/./g", "http://a/g"),
    ("/../g", "http://a/g"),
    ("g.", "http://a/b/c/g."),
    (".g", "http://a/b/c/.g"),
    ("g..", "http://a/b/c/g.."),
    ("..g", "http://a/b/c/..g"),
    ("./../g", "http://a/b/g"),
    ("./g/.", "http://a/b/c/g/"),
    ("g/./h", "http://a/b/c/g/h"),
    ("g/../h", "http://a/b/c/h"),
    ("g;x=1/./y", "http://a/b/c/g;x=1/y"),
    ("g;x=1/../y", "http://a/b/c/y"),
    ("g?y/./x", "http://a/b/c/g?y/./x"),
    ("g?y/../x", "http://a/b/c/g?y/../x"),
    ("g#s/./x", "http://a/b/c/g#s/./x"),
    ("g#s/../x", "http://a/b/c/g#s/../x"),
    ("http:g", "http:g"),
];

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

/// 形态 / 失败 / 边界用例。期望值分别来自 RFC 3986 的正文、Appendix A 的
/// ABNF,以及本模块文件头写明的「非法 URL 边界」条款。
const CASES: &[Case] = &[
    // ── URL_PARSE:Appendix B 正文那一个例子(RFC 自己列的 5 个分量)──
    Case {
        name: "parse_scheme",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://www.ics.uci.edu/pub/ietf/uri/#Related"), "scheme", "MISSING")"#,
        expect: "http",
    },
    Case {
        name: "parse_host",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://www.ics.uci.edu/pub/ietf/uri/#Related"), "host", "MISSING")"#,
        expect: "www.ics.uci.edu",
    },
    Case {
        name: "parse_path",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://www.ics.uci.edu/pub/ietf/uri/#Related"), "path", "MISSING")"#,
        expect: "/pub/ietf/uri/",
    },
    Case {
        name: "parse_fragment",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://www.ics.uci.edu/pub/ietf/uri/#Related"), "fragment", "MISSING")"#,
        expect: "Related",
    },
    Case {
        name: "parse_query_is_undefined_not_empty",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://www.ics.uci.edu/pub/ietf/uri/#Related"), "query", "MISSING")"#,
        expect: "NULL",
    },
    // ── undefined 与 empty 的区别(§5.3 末段,规范性)──────────────────
    Case {
        name: "parse_empty_query_is_empty_not_null",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://a/?"), "query", "MISSING")"#,
        expect: "",
    },
    Case {
        name: "parse_empty_fragment_is_empty_not_null",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://a/#"), "fragment", "MISSING")"#,
        expect: "",
    },
    // ── userinfo 必须单独拆出来(RFC §7.6 的语义攻击)────────────────
    Case {
        name: "parse_scheme_with_userinfo",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://user:pw@host.example:8080/p"), "scheme", "MISSING")"#,
        expect: "http",
    },
    Case {
        name: "parse_userinfo_with_userinfo",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://user:pw@host.example:8080/p"), "userinfo", "MISSING")"#,
        expect: "user:pw",
    },
    Case {
        name: "parse_host_with_userinfo",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://user:pw@host.example:8080/p"), "host", "MISSING")"#,
        expect: "host.example",
    },
    Case {
        name: "parse_port_with_userinfo",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://user:pw@host.example:8080/p"), "port", "MISSING")"#,
        expect: "8080",
    },
    Case {
        name: "parse_path_with_userinfo",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://user:pw@host.example:8080/p"), "path", "MISSING")"#,
        expect: "/p",
    },
    // RFC §7.6 原话的那个例子:人类会以为 host 是 cnn.example.com,
    // 实际是 10.0.0.1。丢掉了 userinfo 就等于替调用方藏起这个攻击面。
    Case {
        name: "parse_rfc_7_6_semantic_attack_host",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("ftp://cnn.example.com&story=breaking_news@10.0.0.1/top_story.htm"), "host", "MISSING")"#,
        expect: "10.0.0.1",
    },
    // ── IPv6 literal 保留方括号;空端口记 undefined ──────────────────
    Case {
        name: "parse_ipv6_literal_keeps_brackets",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://[2001:db8::7]:8080/x"), "host", "MISSING")"#,
        expect: "[2001:db8::7]",
    },
    Case {
        name: "parse_empty_port_is_undefined",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://example.com:/x"), "port", "MISSING")"#,
        expect: "NULL",
    },
    Case {
        name: "parse_ipv6_port",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://[::1]:99/"), "port", "MISSING")"#,
        expect: "99",
    },
    // ── 刻意宽松的两条(否则 §5.4 里的正常输入会被误判)───────────────
    Case {
        name: "parse_accepts_empty_host",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://"), "host", "MISSING")"#,
        expect: "",
    },
    Case {
        name: "parse_accepts_network_path_reference",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("//g"), "host", "MISSING")"#,
        expect: "g",
    },
    Case {
        name: "parse_relative_reference_has_no_scheme",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("a/b/c"), "scheme", "MISSING")"#,
        expect: "NULL",
    },
    Case {
        name: "parse_mailto_is_opaque_rootless_path",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("mailto:John.Doe@example.com"), "path", "MISSING")"#,
        expect: "John.Doe@example.com",
    },
    // ── 非法边界(判据 = 不满足 Appendix A 的 `URI-reference` ABNF)────
    Case {
        name: "reject_control_char_in_path",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); URL_PARSE("http://a/pa\0th")"#,
        expect: "E0030",
    },
    Case {
        name: "reject_space_in_url",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); URL_PARSE("http://a/a b")"#,
        expect: "E0030",
    },
    Case {
        name: "reject_scheme_not_alpha",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); URL_PARSE("1a:b")"#,
        expect: "E0030",
    },
    Case {
        name: "reject_scheme_bad_tail",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); URL_PARSE("ht_tp://a/")"#,
        expect: "E0030",
    },
    Case {
        name: "reject_bare_percent",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); URL_PARSE("http://a/100%")"#,
        expect: "E0030",
    },
    Case {
        name: "reject_truncated_percent",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); URL_PARSE("http://a/%zz")"#,
        expect: "E0030",
    },
    Case {
        name: "reject_port_not_digit",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); URL_PARSE("http://a:80x/")"#,
        expect: "E0030",
    },
    Case {
        name: "reject_port_too_large",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); URL_PARSE("http://a:70000/")"#,
        expect: "E0030",
    },
    Case {
        name: "reject_unclosed_ip_literal",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); URL_PARSE("http://[2001:db8::7/")"#,
        expect: "E0030",
    },
    Case {
        name: "reject_bracket_outside_authority",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); URL_PARSE("http://a/[x]")"#,
        expect: "E0030",
    },
    // ── 形态与元数 ──────────────────────────────────────────────────
    Case {
        name: "parse_non_string_is_e0030",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); URL_PARSE(5)"#,
        expect: "!E0030 URL_PARSE: expected string, got integer",
    },
    Case {
        name: "parse_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); URL_PARSE("http://a/", "b")"#,
        expect: "!E0022 URL_PARSE: function expects 1 argument(s), got 2",
    },
    Case {
        name: "join_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.net", ["URL_JOIN"]); URL_JOIN("http://a/")"#,
        expect: "!E0022 URL_JOIN: function expects 2 argument(s), got 1",
    },
    Case {
        name: "join_base_non_string_is_e0030",
        src: r#"IMPORT("wlwl:std.net", ["URL_JOIN"]); URL_JOIN(5, "b")"#,
        expect: "!E0030 URL_JOIN: expected string, got integer",
    },
];

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_net_{nanos}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
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

/// **RFC 3986 §5.4 全量**:一条不落。
///
/// 这条测试是 `URL_JOIN` 的**唯一**正确性证据 —— 少跑一条也能「全绿」,
/// 所以它**逐条跑**,而不是跑一个汇总断言。
#[test]
fn url_join_matches_rfc_3986_section_5_4() {
    let mut bad = Vec::new();
    for (r, want) in RESOLUTION {
        let got = actual(&format!(
            r#"IMPORT("wlwl:std.net", ["URL_JOIN"]); URL_JOIN("http://a/b/c/d;p?q", "{r}")"#
        ));
        if got != *want {
            bad.push(format!(
                "  URL_JOIN(base, {r:?})\n    expected: {want}\n    actual:   {got}"
            ));
        }
    }
    assert_eq!(
        RESOLUTION.len(),
        42,
        "RFC 3986 §5.4 的用例数变了 —— §5.4 原文若有修订,先对着 RFC 核,再改这个数"
    );
    assert!(
        bad.is_empty(),
        "URL_JOIN 与 RFC 3986 §5.4 漂移:\n{}",
        bad.join("\n")
    );
}

#[test]
fn net_contract() {
    let bad: Vec<String> = CASES
        .iter()
        .filter_map(|c| {
            let got = actual(c.src);
            if got == c.expect {
                None
            } else if c.expect.starts_with("E0030") || c.expect.starts_with("E0022") {
                // 非法用例只断言诊断**码**,不冻结整条消息(措辞会变,码不会)。
                // `actual()` 的失败形态是 `!CODE msg`,所以先剥掉那个 `!`。
                let code = got.trim_start_matches('!').split(' ').next().unwrap_or("");
                if code == c.expect {
                    return None;
                }
                Some(format!(
                    "{}\n      want code: {}\n      actual:   {got}",
                    c.name, c.expect
                ))
            } else {
                Some(format!(
                    "{}\n      expected: {}\n      actual:   {got}",
                    c.name, c.expect
                ))
            }
        })
        .collect();
    assert!(
        bad.is_empty(),
        "wlwl:std.net contract drift:\n    {}",
        bad.join("\n    ")
    );
}

fn spec_path() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("stdlib")
        .join("wlwl-stdlib-spec-v0.11.md")
}

/// 规范 §21 的成员表 ↔ `SPEC.functions`。
///
/// 规范 §21 的成员表 ↔ `SPEC.functions`(五个成员全部落地)。
/// W-03 落地时这个数与名单**一起**改,别只改一边。
#[test]
fn net_member_set_matches_the_spec_table() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    let start = text.find("## 21 ").expect("\u{00a7}21 header exists");
    let end = text[start..]
        .find("\n## ")
        .map(|i| start + i)
        .unwrap_or(text.len());
    let body = &text[start..end];
    let body = match body.find("\n### ") {
        Some(i) => &body[..i],
        None => body,
    };
    let mut spec: Vec<String> = Vec::new();
    for line in body.lines() {
        let Some(rest) = line.strip_prefix("| `") else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| *c != '`' && *c != '(')
            .collect();
        if name.is_empty() {
            continue;
        }
        if name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        {
            spec.push(name);
        }
    }
    let impls: Vec<String> = wlwl_std::resolve("wlwl:std.net")
        .expect("wlwl:std.net resolves")
        .functions()
        .iter()
        .map(|(n, _)| (*n).to_string())
        .collect();
    for n in [
        "URL_PARSE",
        "URL_JOIN",
        "HTTP_GET",
        "HTTP_POST",
        "HTTP_REQUEST",
    ] {
        assert!(
            spec.iter().any(|m| m == n),
            "\u{00a7}21 table missed `{n}`: {spec:?}"
        );
        assert!(
            impls.iter().any(|m| m == n),
            "implementation does not export `{n}`: {impls:?}"
        );
    }
    assert_eq!(
        spec.len(),
        5,
        "\u{00a7}21 table extractor found {} member(s): {spec:?}",
        spec.len()
    );
    assert_eq!(impls.len(), 5, "wlwl:std.net exports 5 members");
}

// ── HTTP 夹具服务器(W-04)────────────────────────────────────────
//
// ⚠ **只绑 127.0.0.1,绝不访问任何外部地址。** CI 不稳定是一层,违反本仓「可复现」卖点是另一层。
// `the_contract_never_touches_an_external_address` 守着这条,且它自己**不依赖任何环境变量**
// (环境变量可以忘设; 扫源码不可以)。

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::OnceLock;

static PORT: OnceLock<u16> = OnceLock::new();

/// 按路径分派的本地夹具。每条连接**各开一个线程** —— 否则一条 `/slow`
/// 会把后续所有请求一起堵住(而超时用例正要靠它)。
fn start_fixture() -> u16 {
    if let Some(p) = PORT.get() {
        return *p;
    }
    let l = TcpListener::bind("127.0.0.1:0").expect("fixture bind");
    let port = l.local_addr().expect("fixture addr").port();
    std::thread::spawn(move || {
        for conn in l.incoming() {
            let Ok(mut s) = conn else { break };
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                let out: String = match path.as_str() {
                    "/ok" => "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nX-Fixture: yes\r\n\r\nhello".into(),
                    "/notfound" => "HTTP/1.1 404 Not Found\r\nContent-Length: 3\r\n\r\nnah".into(),
                    "/boom" => "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 5\r\n\r\nbroke".into(),
                    "/redirect" => "HTTP/1.1 302 Found\r\nLocation: /ok\r\nContent-Length: 4\r\n\r\ngoin".into(),
                    "/chunked" => "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n".into(),
                    "/nolength" => "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nhello".into(),
                    "/huge" => "HTTP/1.1 200 OK\r\nContent-Length: 99999999\r\n\r\n".into(),
                    "/slow" => {
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".into()
                    }
                    "/echo" => {
                        let body = req.rsplit("\r\n\r\n").next().unwrap_or("");
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len())
                    }
                    _ => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".into(),
                };
                let _ = s.write_all(out.as_bytes());
                let _ = s.flush();
            });
        }
    });
    let _ = PORT.set(port);
    port
}

const IMP: &str = r#"IMPORT("wlwl:std.net", ["HTTP_GET", "HTTP_POST", "HTTP_REQUEST"]); "#;

/// `(用例名, 源码, 期望)`。端口在运行时才确定, 所以这张表**不能**是 `const`。
fn http_cases() -> Vec<(String, String, String)> {
    let p = start_fixture();
    let u = |path: &str| format!("http://127.0.0.1:{p}{path}");
    let get = |path: &str| format!("HTTP_GET(\"{}\")", u(path));
    let err = |path: &str| format!("IS_ERR(HTTP_GET(\"{}\"))", u(path));
    let field = |expr: &str, key: &str| format!(r#"AT_K({expr}, "{key}", "MISSING")"#);
    // IMPORT 是**每条源码的前缀**;表达式本身不带 IMPORT,否则它会被包进
    // `AT_K(...)` 的第一个实参里(那正是先前那个 E0011 的成因)。
    let src = |body: &str| format!("{IMP}{body}");

    vec![
        // ── 2xx / 4xx / 5xx 都是**值**,不是诊断 ──
        ("get_200_status".into(), src(&field(&get("/ok"), "status")), "200".into()),
        ("get_200_body".into(), src(&field(&get("/ok"), "body")), "hello".into()),
        (
            "get_200_header".into(),
            src(&field(&field(&get("/ok"), "headers"), "X-Fixture")),
            "yes".into(),
        ),
        ("get_404_is_a_value".into(), src(&field(&get("/notfound"), "status")), "404".into()),
        ("get_404_body".into(), src(&field(&get("/notfound"), "body")), "nah".into()),
        ("get_500_status".into(), src(&field(&get("/boom"), "status")), "500".into()),
        // ── 不跟随重定向(D4):302 原样返回 ──
        ("redirect_status_is_302".into(), src(&field(&get("/redirect"), "status")), "302".into()),
        (
            "redirect_location_exposed".into(),
            src(&field(&field(&get("/redirect"), "headers"), "Location")),
            "/ok".into(),
        ),
        // ── 不支持的写法 → ERR **值**,不是原生诊断 ──
        ("chunked_is_err_value".into(), src(&err("/chunked")), "TRUE".into()),
        ("no_length_is_err_value".into(), src(&err("/nolength")), "TRUE".into()),
        ("body_over_cap_is_err_value".into(), src(&err("/huge")), "TRUE".into()),
        // 超时:只给一个位置参即可(实现按类型区分 DICT / INTEGER)。
        (
            "timeout_is_err_value".into(),
            src(&format!("IS_ERR(HTTP_GET(\"{}\", 300))", u("/slow"))),
            "TRUE".into(),
        ),
        // ── POST / REQUEST ──
        (
            "post_body_reaches_server".into(),
            src(&field(&format!("HTTP_POST(\"{}\", \"ping\")", u("/echo")), "body")),
            "ping".into(),
        ),
        (
            "request_arbitrary_method".into(),
            src(&field(&format!("HTTP_REQUEST(\"PUT\", \"{}\")", u("/ok")), "status")),
            "200".into(),
        ),
        // ── 失败形态(原生诊断,断言完整消息)──
        (
            "https_is_refused".into(),
            r#"IMPORT("wlwl:std.net", ["HTTP_GET"]); IS_ERR(HTTP_GET("https://127.0.0.1/x"))"#.to_string(),
            "TRUE".into(),
        ),
        (
            "connection_refused_is_err_value".into(),
            r#"IMPORT("wlwl:std.net", ["HTTP_GET"]); IS_ERR(HTTP_GET("http://127.0.0.1:1/"))"#.to_string(),
            "TRUE".into(),
        ),
        (
            "get_arity_is_e0022".into(),
            r#"IMPORT("wlwl:std.net", ["HTTP_GET"]); HTTP_GET("a", "b", "c", "d")"#.to_string(),
            "!E0022 HTTP_GET: function expects 1 argument(s), got 4".to_string(),
        ),
        (
            "get_non_string_is_e0030".into(),
            r#"IMPORT("wlwl:std.net", ["HTTP_GET"]); HTTP_GET(5)"#.to_string(),
            "!E0030 HTTP_GET: expected string, got integer".to_string(),
        ),
        (
            "request_bad_method_is_e0030".into(),
            r#"IMPORT("wlwl:std.net", ["HTTP_REQUEST"]); HTTP_REQUEST("BAD METHOD", "http://127.0.0.1:1/")"#.to_string(),
            "!E0030 HTTP_REQUEST: method \"BAD METHOD\" is not a valid HTTP token".to_string(),
        ),
        (
            "header_injection_is_e0030".into(),
            r#"IMPORT("wlwl:std.net", ["HTTP_GET"]); HTTP_GET("http://127.0.0.1:1/", ["X-Evil": "a\rb"])"#.to_string(),
            "!E0030 HTTP_GET: header \"X-Evil\" value contains CR or LF".to_string(),
        ),
    ]
}

#[test]
fn http_contract() {
    let bad: Vec<String> = http_cases()
        .into_iter()
        .filter_map(|(name, src, want)| {
            let got = actual(&src);
            if got == want {
                return None;
            }
            if want == "E0030" || want == "E0022" {
                let code = got.trim_start_matches('!').split(' ').next().unwrap_or("");
                if code == want {
                    return None;
                }
            }
            Some(format!(
                "{name}\n      expected: {want}\n      actual:   {got}"
            ))
        })
        .collect();
    assert!(
        bad.is_empty(),
        "wlwl:std.net HTTP contract drift:\n    {}",
        bad.join("\n    ")
    );
}

// **离线门控**:`这个契约表里的 HTTP 用例**不得打非回环地址**。
/// ⚠ **扫的是「将要实际执行的那份源码」**(`http_cases()` 的返回值),不是整个文件。
/// 第一版扫整文件,结果把 RFC 3986 §5.4 里那些「`http://a/b/c/g`」全报了——
/// 但**它们是 `URL_JOIN` 的纯函数输入,根本不发请求**。
/// 扫描范围必须**u7b49于被门控的行为**,否则它只会堆错报。
#[test]
fn the_contract_never_touches_an_external_address() {
    let mut bad = Vec::new();
    for (name, src, _) in http_cases() {
        for scheme in ["http", "https"] {
            // 拼出来而不写成字面量:不然下面这段自己就会被自己报了。
            let needle = format!("{scheme}://");
            let mut from = 0;
            while let Some(i) = src[from..].find(&needle) {
                let at = from + i + needle.len();
                let host: String = src[at..]
                    .chars()
                    // authority 在 `/` `?` `#` 处终止(RFC 3986)——不停在那里的话,
                    // 无端口的 `https://127.0.0.1/x` 会把 `127.0.0.1/x` 整个当 host。
                    .take_while(|c| {
                        !c.is_whitespace() && !matches!(c, '"' | ')' | ',' | '/' | '?' | '#')
                    })
                    .collect();
                from = at;
                let bare = host.split(':').next().unwrap_or("");
                if !(bare == "127.0.0.1" || bare == "localhost" || bare == "[::1]") {
                    bad.push(format!("  {name}: {scheme} host {host:?}"));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "net_contract.rs 的 HTTP 契约指向了非回环地址 —— 它必须只打仓内夹具:{}",
        bad.join("\n")
    );
}
