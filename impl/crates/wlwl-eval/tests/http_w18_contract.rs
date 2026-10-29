//! `wlwl:std.http` 成员契约 —— **W-18（C-4 / C-5 / C-6：路由 + Cookie + 表单）**。
//!
//! ## 走哪条通道
//!
//! 与 `http_contract.rs` **同款**：`std.http` 是 **R1 纯 wlwl 源码**模块
//! （ADR-0021 的分层规则）⇒ 没有 `ModuleSpec` / `StdFn` 可直调 ⇒ 本文件用
//! `Evaluator` 跑一小段 wlwl 程序，`IMPORT("wlwl:std.http", [...])` 取成员。
//!
//! ## 期望值从哪来
//!
//! 手写（不是实现输出）—— 依据是 RFC 9110（方法 / 状态码）、RFC 6265
//! （cookie）、RFC 2046 / 7578（multipart）、以及本模块文件头写下的形状契约。
//!
//! ## ⚠ 三条语言侧的坑（写用例时踩过）
//!
//! 1. 运算符是**调用形态**（`+(a, b)`），**没有**中缀写法。
//! 2. `(` … `)` 是**块**（用 `;` 分隔），**不是**分组括号。
//! 3. `||` / `&&` 是**二元的** ⇒ 三个条件要**嵌套**。

mod common;

use std::path::{Path, PathBuf};
use wlwl_eval::Evaluator;

/// 成员面的 IMPORT 头（本批新增的六个成员）。
const IMP: &str = r#"IMPORT("wlwl:std.http", ["ROUTE", "MATCH_ROUTE", "PARSE_COOKIES", "SET_COOKIE", "PARSE_FORM", "PARSE_MULTIPART"]);"#;

/// 一条用例：程序 + 期望的**渲染串**。
struct Case {
    name: String,
    src: String,
    expect: String,
}

/// 跑一段程序，取它的渲染结果；失败 ⇒ `!CODE message`。
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

/// 只锁码的期望：实际以 `!EXXXX` 开头且期望恰好等于 `!EXXXX` 时按前缀匹配。
fn matches(expect: &str, got: &str) -> bool {
    if expect.len() == 6 && expect.starts_with('!') && got.starts_with(expect) {
        return true;
    }
    expect == got
}

/// 落盘用的临时目录 —— **一律绝对路径**。
///
/// ⚠ `std.fs` 的**相对**路径按**进程 CWD** 解析 ⇒ 相对路径会把文件写进**仓库里**
///   （见 `fs_atomic_lock_meta_contract.rs` 的文件头）⇒ IO 用例**一律**用绝对路径。
/// ⚠ `common::scratch` = `std::env::temp_dir().join(唯一名)` ⇒ 本身就是绝对的
///   （Linux `/tmp/…`、Windows `C:\Users\…\Temp\…`）⇒ 这里**断言**它，
///   别让「哪天变成相对的」悄悄溜进来。
fn scratch_dir() -> PathBuf {
    let dir = common::scratch("wlwl_http_w18");
    assert!(
        dir.is_absolute(),
        "the scratch dir must be absolute (IO paths resolve against the process CWD): {}",
        dir.display()
    );
    dir
}

/// 把一段文本编成 **wlwl 字符串字面量**（含转义）。
///
/// ⚠ multipart 的正文里有 **CRLF** 和**双引号**（`Content-Disposition` 的参数
///   带引号）⇒ 不转义就**编不过**。
fn wl_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 跑一条用例并断言。
fn check(case: &Case) {
    let got = actual(&format!("{IMP} {}", case.src));
    assert!(
        matches(&case.expect, &got),
        "case: {name}\n  expected: {want}\n  actual:   {got}",
        name = case.name,
        want = case.expect,
        got = got
    );
}

/// 跑一张表。
fn run_table(cases: &[Case]) {
    for c in cases {
        check(c);
    }
}

// ── C-4 · 路由 ──────────────────────────────────────────────────────────────

/// 路由表：模式 ⇒ 方法。
///
/// · 模式 `"/article/{slug}.html"` ⇒ 参数 `slug` 抽 `.html` 前面那截。
/// · 方法是**大小写敏感**的（RFC 9110 §9.1）。
const ROUTE_TABLE: &str = r#"["/article/{slug}.html": "GET", "/health": ["GET", "HEAD"], "/files/{name}": ["GET", "PUT"]]"#;

/// 路由命中的用例表。
fn route_cases() -> Vec<Case> {
    vec![
        Case {
            name: "a_route_match_extracts_the_parameter".into(),
            src: format!(
                r#"LET(routes, ROUTE({table})); LET(m, MATCH_ROUTE(routes, ["method": "GET", "path": "/article/hello.html"])); m["params"]["slug"]"#,
                table = ROUTE_TABLE
            ),
            expect: "hello".into(),
        },
        Case {
            name: "a_route_match_reports_200".into(),
            src: format!(
                r#"LET(routes, ROUTE({table})); LET(m, MATCH_ROUTE(routes, ["method": "GET", "path": "/article/hello.html"])); STR(m["status"])"#,
                table = ROUTE_TABLE
            ),
            expect: "200".into(),
        },
        Case {
            name: "a_route_match_sets_matched_to_true".into(),
            src: format!(
                r#"LET(routes, ROUTE({table})); LET(m, MATCH_ROUTE(routes, ["method": "GET", "path": "/article/hello.html"])); STR(m["matched"])"#,
                table = ROUTE_TABLE
            ),
            expect: "TRUE".into(),
        },
        Case {
            name: "a_route_match_ignores_the_query_string".into(),
            src: format!(
                r#"LET(routes, ROUTE({table})); LET(m, MATCH_ROUTE(routes, ["method": "GET", "path": "/article/hello.html?utm=1"])); m["params"]["slug"]"#,
                table = ROUTE_TABLE
            ),
            expect: "hello".into(),
        },
        Case {
            name: "a_path_that_matches_but_the_method_does_not_is_405".into(),
            src: format!(
                r#"LET(routes, ROUTE({table})); LET(m, MATCH_ROUTE(routes, ["method": "POST", "path": "/article/hello.html"])); STR(m["status"])"#,
                table = ROUTE_TABLE
            ),
            expect: "405".into(),
        },
        Case {
            name: "a_405_is_not_a_match".into(),
            src: format!(
                r#"LET(routes, ROUTE({table})); LET(m, MATCH_ROUTE(routes, ["method": "POST", "path": "/article/hello.html"])); STR(m["matched"])"#,
                table = ROUTE_TABLE
            ),
            expect: "FALSE".into(),
        },
        Case {
            name: "a_405_lists_the_allowed_methods".into(),
            src: format!(
                r#"LET(routes, ROUTE({table})); LET(m, MATCH_ROUTE(routes, ["method": "POST", "path": "/files/report.txt"])); m["allow"]"#,
                table = ROUTE_TABLE
            ),
            expect: "GET, PUT".into(),
        },
        Case {
            name: "a_path_that_does_not_match_at_all_is_404".into(),
            src: format!(
                r#"LET(routes, ROUTE({table})); LET(m, MATCH_ROUTE(routes, ["method": "GET", "path": "/nope"])); STR(m["status"])"#,
                table = ROUTE_TABLE
            ),
            expect: "404".into(),
        },
        Case {
            name: "a_404_has_no_allowed_methods".into(),
            src: format!(
                r#"LET(routes, ROUTE({table})); LET(m, MATCH_ROUTE(routes, ["method": "GET", "path": "/nope"])); m["allow"]"#,
                table = ROUTE_TABLE
            ),
            expect: "".into(),
        },
        Case {
            name: "a_route_match_takes_the_first_matching_entry".into(),
            src: r#"LET(routes, ROUTE(["/a/{x}": "GET", "/a/{y}": "GET"])); LET(m, MATCH_ROUTE(routes, ["method": "GET", "path": "/a/1"])); m["params"]["x"]"#.into(),
            expect: "1".into(),
        },
        Case {
            name: "a_literal_route_matches_exactly".into(),
            src: r#"LET(routes, ROUTE(["/health": "GET"])); LET(m, MATCH_ROUTE(routes, ["method": "GET", "path": "/health"])); STR(m["status"])"#.into(),
            expect: "200".into(),
        },
        Case {
            name: "a_trailing_slash_is_a_different_path".into(),
            src: r#"LET(routes, ROUTE(["/health": "GET"])); LET(m, MATCH_ROUTE(routes, ["method": "GET", "path": "/health/"])); STR(m["status"])"#.into(),
            expect: "404".into(),
        },
        Case {
            name: "a_parameter_must_capture_something".into(),
            // `/{slug}.html` 要求段以 `.html` 结尾 **且** 前面**非空**。
            src: r#"LET(routes, ROUTE(["/{slug}.html": "GET"])); LET(m, MATCH_ROUTE(routes, ["method": "GET", "path": "/.html"])); STR(m["status"])"#.into(),
            expect: "404".into(),
        },
        Case {
            name: "the_methods_are_case_sensitive".into(),
            // RFC 9110 §9.1：方法**大小写敏感** ⇒ 表里的 `get` 不匹配 `GET`。
            src: r#"LET(routes, ROUTE(["/a": "get"])); LET(m, MATCH_ROUTE(routes, ["method": "GET", "path": "/a"])); STR(m["status"])"#.into(),
            expect: "405".into(),
        },
    ]
}

// ── C-5 · Cookie ────────────────────────────────────────────────────────────

fn cookie_cases() -> Vec<Case> {
    vec![
        Case {
            name: "a_cookie_is_parsed_by_its_name".into(),
            src: r#"LET(c, PARSE_COOKIES("sid=abc123")); c["sid"]"#.into(),
            expect: "abc123".into(),
        },
        Case {
            name: "several_cookies_are_parsed_at_once".into(),
            src: r#"LET(c, PARSE_COOKIES("sid=abc123; theme=dark")); c["theme"]"#.into(),
            expect: "dark".into(),
        },
        Case {
            name: "a_quoted_cookie_value_is_unquoted".into(),
            src: r#"LET(c, PARSE_COOKIES("a=1; b=\"x y\"")); c["b"]"#.into(),
            expect: "x y".into(),
        },
        Case {
            name: "a_cookie_segment_without_an_equals_sign_is_skipped".into(),
            src: r#"LET(c, PARSE_COOKIES("junk; sid=abc")); c["sid"]"#.into(),
            expect: "abc".into(),
        },
        Case {
            name: "the_last_cookie_of_a_name_wins".into(),
            src: r#"LET(c, PARSE_COOKIES("sid=first; sid=second")); c["sid"]"#.into(),
            expect: "second".into(),
        },
        Case {
            name: "an_empty_cookie_header_gives_an_empty_dict".into(),
            src: r#"LEN(PARSE_COOKIES(""))"#.into(),
            expect: "0".into(),
        },
        Case {
            name: "a_set_cookie_without_attributes_is_just_the_pair".into(),
            src: r#"SET_COOKIE("sid", "abc123")"#.into(),
            expect: "sid=abc123".into(),
        },
        Case {
            name: "a_set_cookie_carries_its_attributes".into(),
            src: r#"SET_COOKIE("sid", "abc123", ["path": "/", "domain": "example.com", "max_age": 60, "same_site": "Lax", "secure": TRUE, "http_only": TRUE])"#.into(),
            expect: "sid=abc123; Path=/; Domain=example.com; Max-Age=60; SameSite=Lax; Secure; HttpOnly".into(),
        },
        Case {
            name: "a_set_cookie_leaves_out_the_absent_attributes".into(),
            src: r#"SET_COOKIE("sid", "abc123", ["http_only": TRUE])"#.into(),
            expect: "sid=abc123; HttpOnly".into(),
        },
        Case {
            name: "a_same_site_value_is_normalized".into(),
            src: r#"SET_COOKIE("sid", "abc", ["same_site": "strict"])"#.into(),
            expect: "sid=abc; SameSite=Strict".into(),
        },
        Case {
            name: "a_set_cookie_rejects_an_illegal_value".into(),
            // 值里的 `;` 能塞进**另一个属性** ⇒ 一律拒（不替调用方猜怎么编码）。
            src: r#"LET(c, SET_COOKIE("sid", "a; b")); LET(_, c); IF(IS_ERR(c), ERR_PAYLOAD(c)["kind"], "NOT_ERR")"#.into(),
            expect: "HttpError".into(),
        },
        Case {
            name: "a_set_cookie_rejects_an_unknown_option".into(),
            src: r#"LET(c, SET_COOKIE("sid", "abc", ["httponly": TRUE])); LET(_, c); IF(IS_ERR(c), ERR_PAYLOAD(c)["kind"], "NOT_ERR")"#.into(),
            expect: "HttpError".into(),
        },
        Case {
            name: "a_set_cookie_rejects_a_bad_same_site_value".into(),
            src: r#"LET(c, SET_COOKIE("sid", "abc", ["same_site": "Nope"])); LET(_, c); IF(IS_ERR(c), ERR_PAYLOAD(c)["kind"], "NOT_ERR")"#.into(),
            expect: "HttpError".into(),
        },
    ]
}

// ── C-6 · 表单 ──────────────────────────────────────────────────────────────

fn form_cases() -> Vec<Case> {
    vec![
        Case {
            name: "a_form_field_is_parsed".into(),
            src: r#"LET(f, PARSE_FORM("name=alice")); f["name"]"#.into(),
            expect: "alice".into(),
        },
        Case {
            name: "a_form_value_is_percent_decoded".into(),
            src: r#"LET(f, PARSE_FORM("msg=hi%20there")); f["msg"]"#.into(),
            expect: "hi there".into(),
        },
        Case {
            name: "a_form_plus_is_a_space".into(),
            src: r#"LET(f, PARSE_FORM("msg=hi+there")); f["msg"]"#.into(),
            expect: "hi there".into(),
        },
        Case {
            name: "a_form_value_may_contain_an_equals_sign".into(),
            src: r#"LET(f, PARSE_FORM("k=b=c")); f["k"]"#.into(),
            expect: "b=c".into(),
        },
        Case {
            name: "a_repeated_form_field_keeps_every_value".into(),
            src: r#"LET(f, PARSE_FORM("a=1&a=2")); f["a"]"#.into(),
            expect: "[1, 2]".into(),
        },
        Case {
            name: "an_empty_form_body_gives_an_empty_dict".into(),
            src: r#"LEN(PARSE_FORM(""))"#.into(),
            expect: "0".into(),
        },
        Case {
            name: "a_form_rejects_a_bad_percent_escape".into(),
            src: r#"LET(f, PARSE_FORM("a=%zz")); LET(_, f); IF(IS_ERR(f), ERR_PAYLOAD(f)["kind"], "NOT_ERR")"#.into(),
            expect: "HttpError".into(),
        },
    ]
}

// ── C-6 · multipart ─────────────────────────────────────────────────────────

/// 边界记号。
const BOUND: &str = "BOUND";

/// 一份 multipart 正文：三个 part（两个字段 + 一个文件）。
///
/// ⚠ 第 2 个 part（`note`）的内容**故意**是一行 `--BOUNDX` ——
///   它**以边界记号开头但不相等** ⇒ 按 RFC 2046 §5.1.1 它是**内容**，
///   不是分隔行（**反向守卫**用的就是这一行）。
fn multipart_body() -> String {
    let mut b = String::new();
    b.push_str(&format!("--{BOUND}\r\n"));
    b.push_str("Content-Disposition: form-data; name=\"name\"\r\n");
    b.push_str("\r\n");
    b.push_str("alice\r\n");
    b.push_str(&format!("--{BOUND}\r\n"));
    b.push_str("Content-Disposition: form-data; name=\"note\"\r\n");
    b.push_str("\r\n");
    b.push_str(&format!("--{BOUND}X\r\n"));
    b.push_str(&format!("--{BOUND}\r\n"));
    b.push_str("Content-Disposition: form-data; name=\"upload\"; filename=\"a.txt\"\r\n");
    b.push_str("Content-Type: text/plain\r\n");
    b.push_str("\r\n");
    b.push_str("hello\r\n");
    b.push_str(&format!("--{BOUND}--\r\n"));
    b
}

/// 一段 multipart 程序（把 `dir` 指到给定的目录）。
fn multipart_src(dir: &Path, expr: &str) -> String {
    format!(
        r#"LET(m, PARSE_MULTIPART({body}, {ct}, ["dir": {dir}])); {expr}"#,
        body = wl_str(&multipart_body()),
        ct = wl_str(&format!("multipart/form-data; boundary={BOUND}")),
        dir = wl_str(&dir.display().to_string()),
        expr = expr
    )
}

/// multipart 的用例表（**不**碰文件落盘的那一半 —— 那是单独一条用例）。
fn multipart_cases(dir: &Path) -> Vec<Case> {
    vec![
        Case {
            name: "a_multipart_field_is_parsed".into(),
            src: multipart_src(dir, r#"m["fields"]["name"]"#),
            expect: "alice".into(),
        },
        Case {
            name: "a_multipart_file_part_is_reported".into(),
            src: multipart_src(dir, r#"m["files"][0]["filename"]"#),
            expect: "a.txt".into(),
        },
        Case {
            name: "a_multipart_file_part_reports_its_size_in_bytes".into(),
            src: multipart_src(dir, r#"STR(m["files"][0]["size"])"#),
            expect: "5".into(),
        },
        Case {
            name: "a_multipart_file_part_reports_its_content_type".into(),
            src: multipart_src(dir, r#"m["files"][0]["content_type"]"#),
            expect: "text/plain".into(),
        },
        Case {
            name: "a_multipart_file_part_reports_its_field_name".into(),
            src: multipart_src(dir, r#"m["files"][0]["name"]"#),
            expect: "upload".into(),
        },
        Case {
            name: "a_line_that_only_looks_like_the_boundary_stays_in_the_content".into(),
            // ⚠ **反向守卫**：这一行以 `--BOUND` 开头但**不相等** ⇒ 按 RFC 2046
            //   §5.1.1 它是**内容**。若实现把边界匹配改成「前缀匹配」，
            //   这一行会被当成分隔行 ⇒ 内容被吃掉 ⇒ 本用例**必红**。
            src: multipart_src(dir, r#"m["fields"]["note"]"#),
            expect: format!("--{BOUND}X"),
        },
        Case {
            name: "a_multipart_without_a_boundary_is_rejected".into(),
            src: format!(
                r#"LET(m, PARSE_MULTIPART({body}, {ct}, ["dir": {dir}])); LET(_, m); IF(IS_ERR(m), ERR_PAYLOAD(m)["kind"], "NOT_ERR")"#,
                body = wl_str(&multipart_body()),
                ct = wl_str("multipart/form-data"),
                dir = wl_str(&dir.display().to_string()),
            ),
            expect: "HttpError".into(),
        },
        Case {
            name: "a_multipart_with_a_non_multipart_content_type_is_rejected".into(),
            src: format!(
                r#"LET(m, PARSE_MULTIPART({body}, {ct}, ["dir": {dir}])); LET(_, m); IF(IS_ERR(m), ERR_PAYLOAD(m)["kind"], "NOT_ERR")"#,
                body = wl_str(&multipart_body()),
                ct = wl_str("application/json"),
                dir = wl_str(&dir.display().to_string()),
            ),
            expect: "HttpError".into(),
        },
        Case {
            name: "a_multipart_body_without_the_closing_boundary_is_rejected".into(),
            // 截断的正文不该被当成完整的。
            src: format!(
                r#"LET(m, PARSE_MULTIPART({body}, {ct}, ["dir": {dir}])); LET(_, m); IF(IS_ERR(m), ERR_PAYLOAD(m)["kind"], "NOT_ERR")"#,
                body = wl_str(&format!(
                    "--{BOUND}\r\nContent-Disposition: form-data; name=\"name\"\r\n\r\nalice\r\n"
                )),
                ct = wl_str(&format!("multipart/form-data; boundary={BOUND}")),
                dir = wl_str(&dir.display().to_string()),
            ),
            expect: "HttpError".into(),
        },
    ]
}

// ── 用例入口 ────────────────────────────────────────────────────────────────

/// 模块源码**必须能解析** —— 语法错时这里带**位置**报出来（比「用例红了」有用）。
#[test]
fn the_module_parses() {
    let src = wlwl_std::LANG_SOURCES
        .iter()
        .find(|s| s.path == "wlwl:std.http")
        .expect("wlwl:std.http must be registered in LANG_SOURCES");
    if let Err(e) = wlwl_parser::parse(src.source, "http.wll") {
        panic!("{}", e.diagnostic().render_human());
    }
}

/// 路由：命中 / 参数抽取 / 405 / 404。
#[test]
fn the_route_is_matched() {
    run_table(&route_cases());
}

/// Cookie：解析 / 生成。
#[test]
fn the_cookies_are_parsed_and_built() {
    run_table(&cookie_cases());
}

/// 表单：`x-www-form-urlencoded`。
#[test]
fn the_form_is_parsed() {
    run_table(&form_cases());
}

/// multipart：解析 + **文件 part 落盘**。
#[test]
fn the_multipart_is_parsed() {
    let dir = scratch_dir();
    run_table(&multipart_cases(&dir));
}

/// **文件 part 真的落在磁盘上** —— 路径交给调用方，内容**逐字**对得上。
///
/// ⚠ 这一条是「落盘」这件事的**正面**证据：只看返回的 `path` 字符串不够
///   （路径可以是编的）⇒ 必须**从磁盘把文件读回来**。
#[test]
fn a_multipart_file_part_lands_on_disk() {
    let dir = scratch_dir();
    let src = multipart_src(&dir, r#"m["files"][0]["path"]"#);
    let got = actual(&format!("{IMP} {src}"));
    assert!(
        !got.starts_with('!'),
        "the multipart parse must not fail: {got}"
    );

    // 交回来的路径必须是**绝对路径**且落在临时目录**之下** ⇒
    // 「相对路径（按进程 CWD 解析）」这条退化会被本断言当场抓住
    // （Linux 上 `std.fs::PATH_JOIN` 会把根 `/` 丢掉 ⇒ 落进仓库 ⇒ 正是这条）。
    let landed = PathBuf::from(&got);
    assert!(
        landed.is_absolute(),
        "the returned path must be absolute: {got}"
    );
    assert!(
        landed.starts_with(&dir),
        "the returned path must live under the scratch dir: {got} (dir: {})",
        dir.display()
    );

    // 落盘名 = `<part 序号>_<安全文件名>`（第三个 part ⇒ 序号 2）。
    let expected = dir.join("2_a.txt");
    assert!(
        expected.exists(),
        "the file part must land on disk at {}",
        expected.display()
    );
    let content =
        std::fs::read_to_string(expected.as_path()).expect("the landed file must be readable");
    assert_eq!(
        content, "hello",
        "the landed file must carry the part's bytes"
    );
}
