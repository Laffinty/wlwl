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

mod common;

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
    // ── D13-020 / N-008:`port` 的位数没有上限,`is_digit` 只保证「全是数字」────
    //
    // 判据来源(三者之一):
    // ① **独立第二实现** —— Python 3.13 `urllib.parse.urlsplit` 实测:
    //    `http://a:65536/`、`http://a:99999999999999/`、`http://a:999999999999999999999999/`
    //    三条**全部** `ValueError: Port out of range 0-65535`,
    //    `http://a:65535/` 给 `port=65535`,`http://a:/` 给 `None`。
    //    ⇒ Python 用**任意精度整数**,所以「超范围」这条判据**与位数无关**;
    //    本实现改用 `u64`(≤ 20 位)后拿到的是**同一条判据**,不是一条新判据。
    // ② **规范** §21 拒绝表:「`port` 非 `*DIGIT`,或数值 > 65535 → `E0030`」。
    // ③ RFC 3986 §3.2.2 只定义 `port = *DIGIT`(纯文法,**不含范围**)⇒ 范围是规范
    //    自己加的约束,故引用规范而不引 RFC。
    //
    // ⚠ 修复前 `http://a:99999999999999/` 会让 `"…".parse::<u32>()` 返 `Err`,
    //   再被 `.expect("all digits")` **panic** —— 纯输入打死解释器进程。
    //   下面 4 条合起来钉住「超范围一律 `E0030` + 如实报值,**永不 panic**」。
    Case {
        name: "reject_port_above_u32_range_reports_the_real_value",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); URL_PARSE("http://a:99999999999999/")"#,
        expect: r#"!E0030 URL_PARSE: not a valid URI reference: port 99999999999999 exceeds 65535: "http://a:99999999999999/""#,
    },
    // ⚠ > 20 位:`u64` 也放不下 ⇒ 文案带**原始串**而不是数字,但判据**不变**
    //   (Python 对同一输入同样只给「超出 0-65535」一条判决,见上)。
    Case {
        name: "reject_port_beyond_u64_range_reports_the_raw_digits",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); URL_PARSE("http://a:999999999999999999999999/")"#,
        expect: r#"!E0030 URL_PARSE: not a valid URI reference: port 999999999999999999999999 exceeds 65535: "http://a:999999999999999999999999/""#,
    },
    // 边界值两个方向。期望值 = Python 实测(①),且 65535 本来就在旧契约里合法。
    Case {
        name: "accept_port_at_the_u16_upper_bound",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); AT_K(URL_PARSE("http://a:65535/"), "port", "MISSING")"#,
        expect: "65535",
    },
    Case {
        name: "reject_port_one_past_the_u16_upper_bound",
        src: r#"IMPORT("wlwl:std.net", ["URL_PARSE"]); URL_PARSE("http://a:65536/")"#,
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
    common::scratch("wlwl_net")
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
        .join("wlwl-spec-reference.md")
}

/// 规范 §21 的成员表 ↔ `SPEC.functions`。
///
/// 规范 §21 的成员表 ↔ `SPEC.functions`(五个成员全部落地)。
/// W-03 落地时这个数与名单**一起**改,别只改一边。
#[test]
fn net_member_set_matches_the_spec_table() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    let start = text
        .find("## 标准库规范 · 21 ")
        .expect("\u{00a7}21 header exists");
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

/// D13-050:规范 §21 的**状态码定界规则**必须写在册子里。
///
/// ⚠ 这条守卫守的是**规范文本**,不是实现 —— 那个缺陷之所以能活这么久,
///   正是因为规范里只写了「实现未特判 1xx / 204 / 304」这一句**缺口描述**,
///   从来没写下**目标语义**。⇒ 现在目标语义必须成文,且不许被删回去。
/// ⚠ 三个锚点**刻意只挑目标语义那一句里才有的措辞** —— 「`204` / `304` / `HEAD`」
///   在拒绝表里也出现,拿它当锚点等于没守。
#[test]
fn the_status_code_delimitation_rules_are_written_down() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    let start = text
        .find("## 标准库规范 · 21 ")
        .expect("\u{00a7}21 header exists");
    let end = text[start..]
        .find("\n## ")
        .map(|i| start + i)
        .unwrap_or(text.len());
    let body = &text[start..end];
    for needle in [
        // §21.4 的目标语义:1xx 跳过继续读。
        "跳过它,继续读下一个响应",
        // §21.4 的目标语义:body 恒空 + 忽略 Content-Length。
        "忽略 `Content-Length`",
        // ADR-0023 §v0.x 2 的 breaking 申报(成员行标「自 vX.Y 变更」)。
        "自 v0.11.3 变更",
    ] {
        assert!(
            body.contains(needle),
            "stdlib spec §21 lost `{needle}` — D13-050 的目标语义必须成文,\
             不许退回「只登记缺口」那一句"
        );
    }
}

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
                // ── D13-040(N-033):慢速滴流。头一次给全(带 Content-Length),
                //    body 每 100 ms 给 1 字节、共 10 字节。
                //    ⚠ 这条端点存在的**唯一**理由是让「每次 `read` 各有一份完整
                //    `dur`」的缺陷形态**永远超不了时**:每轮 `read` 都在 100 ms 内
                //    拿到了数据,旧实现在这里**必然成功**(总耗时 ~1 s ≫ `timeout`),
                //    而总闸形态必须在 `timeout` 处失败。所以它是一条**真红**守卫。
                //    ⚠ 客户端一旦放弃就会 `shutdown`,`write_all` 随即报错 —— 那个
                //    `break` 是为了让这条线程早点退出,不是被断言的行为。
                if path == "/dribble" {
                    let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n");
                    let _ = s.flush();
                    for _ in 0..10 {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        if s.write_all(b"x").is_err() {
                            break;
                        }
                        let _ = s.flush();
                    }
                    return;
                }
                let out: Vec<u8> = match path.as_str() {
                    "/ok" => b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nX-Fixture: yes\r\n\r\nhello".to_vec(),
                    "/notfound" => b"HTTP/1.1 404 Not Found\r\nContent-Length: 3\r\n\r\nnah".to_vec(),
                    "/boom" => b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 5\r\n\r\nbroke".to_vec(),
                    "/redirect" => b"HTTP/1.1 302 Found\r\nLocation: /ok\r\nContent-Length: 4\r\n\r\ngoin".to_vec(),
                    "/chunked" => b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n".to_vec(),
                    "/nolength" => b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nhello".to_vec(),
                    "/huge" => b"HTTP/1.1 200 OK\r\nContent-Length: 99999999\r\n\r\n".to_vec(),
                    // ── D13-041(N-034):HTTP/1.0 + 无 Content-Length + 无
                    //    `Connection: close` ⇒ **close 定界**(读到 EOF)。
                    //    ⚠ 刻意与 `/nolength` 配对:两条只差**版本号**,于是
                    //    「HTTP/1.1 仍严格 / HTTP/1.0 放宽」这个区分本身被钉住。
                    "/close10" => b"HTTP/1.0 200 OK\r\nContent-Type: text/plain\r\n\r\nhello".to_vec(),
                    // ── D13-050:按**状态码**定界(RFC 9112 §6.3 第 1 条)──
                    //
                    // ⚠ 这六条路由存在的**唯一**理由是让四种响应形态在 CI 里
                    //   **可复现** —— 缺陷形态(完全不看状态码)在每一条上的
                    //   可观察后果都**不同**,所以每条各钉一件事。
                    //
                    // `204` 不带 `Content-Length` ⇒ 缺陷形态:`ERR`
                    //   (「既无 CL 又无 Connection: close」)。
                    "/204" => b"HTTP/1.1 204 No Content\r\n\r\n".to_vec(),
                    // `204` 带 `Content-Length: 5` 且**真的有 5 字节 body**
                    //   ⇒ 缺陷形态:那 5 字节被**真的读出来**(RFC 说这些响应
                    //   本就无 body,无论头里写了什么)。
                    "/204cl5" => b"HTTP/1.1 204 No Content\r\nContent-Length: 5\r\n\r\nhello".to_vec(),
                    // `204` 带一个**非数字** `Content-Length` ⇒ 缺陷形态:`ERR`
                    //   (「不是数字」);新形态:**忽略**它。
                    "/204badcl" => b"HTTP/1.1 204 No Content\r\nContent-Length: abc\r\n\r\n".to_vec(),
                    // `304` 不带 `Content-Length` ⇒ 缺陷形态:`ERR`。
                    "/304" => b"HTTP/1.1 304 Not Modified\r\n\r\n".to_vec(),
                    // `100 Continue` + **同一条连接上**的终响应
                    //   ⇒ 缺陷形态:把 `100` 当**终响应**(读到的 status 是 100)。
                    "/100" => {
                        let mut v = b"HTTP/1.1 100 Continue\r\n\r\n".to_vec();
                        v.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello");
                        v
                    }
                    // `HEAD` 的响应:头里有 `Content-Length: 5` 但**没有 body**
                    //   ⇒ 缺陷形态:`ERR`(等 5 字节 body 等不到)。
                    "/head" => b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n".to_vec(),
                    // ── D13-043(N-036):body 是**非法 UTF-8**(含 `0x00` 与 `0xFF`)。
                    //    ⚠ 这四个字节取自 Python 3.13 `http.client` 的实测输出,
                    //    那是本机上可用的**独立第二实现**(见下面的用例注释)。
                    "/binary" => {
                        let mut v =
                            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nContent-Type: application/octet-stream\r\n\r\n"
                                .to_vec();
                        v.extend_from_slice(&[0xFF, 0xFE, 0x00, 0x41]);
                        v
                    }
                    "/slow" => {
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec()
                    }
                    "/echo" => {
                        let body = req.rsplit("\r\n\r\n").next().unwrap_or("");
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len())
                            .into_bytes()
                    }
                    _ => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec(),
                };
                let _ = s.write_all(&out);
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
        // ⚠ D13-041:`no_length_is_err_value` **没有被放宽**,而且它现在是
        //   「HTTP/1.1 仍严格」这条边界的**正向守卫**。RFC 9112 §6.3 第 8 条
        //   其实不分版本(见下面 `http10_*` 两条的注释),规范 §21.4 为了反走私
        //   要求更严 —— 这一条是**刻意比 RFC 紧**,不许顺手改掉。
        ("no_length_is_err_value".into(), src(&err("/nolength")), "TRUE".into()),
        ("body_over_cap_is_err_value".into(), src(&err("/huge")), "TRUE".into()),
        // ── D13-041(N-034):HTTP/1.0 的 close 定界 ──
        //
        // 期望值出处:RFC 9112 Appendix C.2.2 ——「In HTTP/1.0, each connection is
        // established by the client prior to the request and **closed by the server
        // after sending the response**」;RFC 9112 §6.3 第 8 条(响应定界判定的
        // 兜底条)——「this is a response message without a declared message body
        // length, so the message body length is determined by the number of octets
        // received prior to the server closing the connection」。
        // ⚠ **独立第二实现**:Python 3.13 `http.client` 对同一个「HTTP/1.0 + 无
        //   `Content-Length`」的夹具实测返回 `status 200 / version 10 / body
        //   b'hello'` —— 它**不报错**,直接读到 EOF。
        ("http10_no_length_is_close_delimited".into(), src(&field(&get("/close10"), "body")), "hello".into()),
        ("http10_status_is_readable".into(), src(&field(&get("/close10"), "status")), "200".into()),
        // ── D13-042(N-035):`tchar` 收紧(精确地只少那五个)──
        //
        // 期望值出处:**只有 RFC 9110 §5.6.2**(与 RFC 7230 §3.2.6 逐字相同)——
        // `tchar = "!" / "#" / "$" / "%" / "&" / "'" / "*" / "+" / "-" / "." /
        //  "^" / "_" / "`" / "|" / "~" / DIGIT / ALPHA`,其下的注释是
        // 「any VCHAR, except delimiters」,分隔符被点名为 `DQUOTE` 与
        // `"(),/:;<=>?@[\]{}"` —— 五个被剔除的字符全在其中。
        // `field-name = token`(RFC 9110 §5.1)、`method = token`(RFC 9112 §3.1)。
        // ⚠ **本机上没有可用的第二实现**:Python 3.13 的
        //   `http.client._is_legal_header_name` 接受除 `:` 外**全部**可见 ASCII
        //   (实测 `!"#$%&'()*+,-./0123456789;<=>?@A-Z[\]^_`a-z{|}~`),那是
        //   CPython `tokenize` 的 `_TOKEN`,**比 RFC 宽得多** —— 不能拿来佐证。
        //   故这 5 条**只引 RFC**,不伪造「我以为浏览器会这样」。
        (
            "header_name_paren_is_e0030".into(),
            src(&format!("HTTP_GET(\"{}\", [\"X-Bad(\": \"v\"])", u("/ok"))),
            "!E0030 HTTP_GET: header name \"X-Bad(\" is not a valid token".into(),
        ),
        (
            "header_name_semicolon_is_e0030".into(),
            src(&format!("HTTP_GET(\"{}\", [\"X-Bad;\": \"v\"])", u("/ok"))),
            "!E0030 HTTP_GET: header name \"X-Bad;\" is not a valid token".into(),
        ),
        (
            "method_paren_is_e0030".into(),
            src(&format!("HTTP_REQUEST(\"GE(\", \"{}\")", u("/ok"))),
            "!E0030 HTTP_REQUEST: method \"GE(\" is not a valid HTTP token".into(),
        ),
        // ⚠ 收紧**只许少那五个** —— 这条把剩下 15 个非字母数字 tchar 逐个点名,
        //   防止有人「顺手再紧一点」把 `~` / `|` / `^` 之类也砍掉。
        (
            "header_name_full_tchar_set_is_accepted".into(),
            src(&format!(
                "AT_K(HTTP_GET(\"{}\", [\"X-A!#$%&'*+-.^_`|~\": \"v\"]), \"status\", \"MISSING\")",
                u("/ok")
            )),
            "200".into(),
        ),
        (
            "method_full_tchar_set_is_accepted".into(),
            src(&format!("AT_K(HTTP_REQUEST(\"M-SEARCH!#\", \"{}\"), \"status\", \"MISSING\")", u("/ok"))),
            "200".into(),
        ),
        // ── D13-043(N-036):非 UTF-8 的 body 走 `ARRAY[INTEGER]`,不再整条 ERR ──
        //
        // 期望值出处:① **独立第二实现** —— Python 3.13 `http.client` 读同一个
        // 「4 字节非法 UTF-8」夹具实测 `type(resp.read()) is bytes`、
        // `list(b) == [255, 254, 0, 65]`,**不报错、不做 U+FFFD 替换**
        // (`b.decode('utf-8')` 抛 `invalid start byte`);② **规范** §2.3 / §11.6
        // 立 `ARRAY[INTEGER]` 为本语言**唯一无损字节载体**,元素 `0..=255`,与
        // `std.fs::READ_BYTES` 同款;③ 方括号里逗号分隔的写法是 `Value::display`
        // 对 `Value::Array` 的**渲染格式**(`wlwl-value/src/lib.rs:318`),
        // 属测试夹具的格式化事实,不是语义期望值。
        // ⚠ `0x00` 在里面是关键:它证明这不是「UTF-8 解码后补了什么」。
        ("binary_body_is_a_byte_array".into(), src(&field(&get("/binary"), "body")), "[255, 254, 0, 65]".into()),
        // 配对:`binary_body_is_a_byte_array` 放宽了**非** UTF-8 那一侧,
        // 这一条守住**文本**那一侧没被顺手改成「一律数组」—— `body` 仍是 `STRING`。
        ("text_body_is_still_a_string".into(), src(&field(&get("/ok"), "body")), "hello".into()),
        // 超时:只给一个位置参即可(实现按类型区分 DICT / INTEGER)。
        (
            "timeout_is_err_value".into(),
            src(&format!("IS_ERR(HTTP_GET(\"{}\", 300))", u("/slow"))),
            "TRUE".into(),
        ),
        // ── D13-040(N-033):`timeout` 是**整条请求**的上界,不是每次读的 ──
        //
        // 期望值出处:规范 §21.4 的「**超时必填且有保守默认**…调用方可覆盖」+
        // `ADR-0028` D2(30 s)—— 那是对**这次调用**的承诺,不是对每次系统调用的;
        // RFC 9112 §6.3 第 6 条同向:「If the sender closes the connection or **the
        // recipient times out** before the indicated number of octets are received,
        // the recipient MUST consider the message to be incomplete」。
        // ⚠ `/dribble` 每 100 ms 给 1 字节共 10 字节 ⇒ 缺陷形态总耗时 ~1 s,
        //   而这里只给 500 ms。**这条不可能假绿**:新形态在 500 ms 处必然失败,
        //   旧形态在 ~1 s 处必然成功 —— 两边都离 500 ms 有 2 倍以上余量。
        (
            "total_budget_caps_a_slow_dribble".into(),
            src(&format!("IS_ERR(HTTP_GET(\"{}\", 500))", u("/dribble"))),
            "TRUE".into(),
        ),
        // ── D13-050:按**状态码**定界(RFC 9112 §6.3 第 1 条)──
        //
        // 期望值出处:**RFC 9112 §6.3 第 1 条**原文 ——
        // 「Any response to a HEAD request and any response with a 1xx, 204, or
        // 304 status code is always terminated by the first empty line after the
        // header fields, **regardless of the header fields present in the
        // message**, and thus cannot contain a message body or trailer section.」
        // ⚠ **独立第二实现**:Python 3.13 `http.client` 对 `204` / `304` 同样
        //   **不读 body**(`length` 为 0),对 `1xx` 则**跳过并继续读下一个响应**
        //   —— 与本实现的新形态一致。
        (
            "get_204_without_content_length_is_readable".into(),
            src(&field(&get("/204"), "status")),
            "204".into(),
        ),
        (
            "get_204_body_is_empty".into(),
            src(&field(&get("/204"), "body")),
            "".into(),
        ),
        // ⚠ **这一条钉的是缺陷形态的核心**:`204` + `Content-Length: 5` + 真 body
        //   ⇒ 旧实现把那 5 字节**真的读出来**(RFC 说这些响应本就无 body)。
        (
            "get_204_ignores_content_length".into(),
            src(&field(&get("/204cl5"), "body")),
            "".into(),
        ),
        (
            "get_204_ignores_a_non_numeric_content_length".into(),
            src(&field(&get("/204badcl"), "status")),
            "204".into(),
        ),
        (
            "get_304_without_content_length_is_readable".into(),
            src(&field(&get("/304"), "status")),
            "304".into(),
        ),
        (
            "get_304_body_is_empty".into(),
            src(&field(&get("/304"), "body")),
            "".into(),
        ),
        // ⚠ **`1xx` 必须被跳过**:缺陷形态把 `100 Continue` 当**终响应**
        //   ⇒ 调用方拿到的 status 是 100 而不是 200。
        (
            "get_1xx_is_skipped_and_the_next_response_is_returned".into(),
            src(&field(&get("/100"), "status")),
            "200".into(),
        ),
        (
            "get_1xx_skipped_body_is_the_final_response_body".into(),
            src(&field(&get("/100"), "body")),
            "hello".into(),
        ),
        // ⚠ `HEAD` 响应头里的 `Content-Length` 描述的是「`GET` 会返回的实体有多长」,
        //   **不是**本次 body 的长度 ⇒ 拿它当 body 长度去读,就会读到**下一个
        //   响应**或一直挂住。
        (
            "head_response_status_is_readable".into(),
            src(&field(&format!("HTTP_REQUEST(\"HEAD\", \"{}\")", u("/head")), "status")),
            "200".into(),
        ),
        (
            "head_response_body_is_empty".into(),
            src(&field(&format!("HTTP_REQUEST(\"HEAD\", \"{}\")", u("/head")), "body")),
            "".into(),
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
