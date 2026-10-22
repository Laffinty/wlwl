//! `wlwl:std.sanitize` 成员契约表 —— 标准库规范 §13(v0.11.3 M1 / M3)。
//!
//! 沿三份既有契约(collection / str_math / test)的形态:
//!
//! 1. **规范表对拍**(四件套第 3 条):从规范 §13 的成员表**解析**出成员面,
//!    与 `resolve("wlwl:std.sanitize")` 的实现清单对拍 —— 两侧不同源,
//!    规范被改坏而实现没动时这里变红;
//! 2. **冻结用例**:诊断码 + 消息逐字 + 结果 `display()`;
//! 3. **反向守卫**:成员集非空、点名成员在列、**两侧不得空转**(集合比较
//!    本身也要能被「多出 / 少掉 / 改名」打红)。
//!
//! M3 追加的三块(计划 W-12 / W-13):
//!
//! - [`SANITIZE_CASES`] —— OWASP XSS Filter Evasion 经典向量 + mXSS 教科书
//!   样本 + 畸形输入 + 非缺省 policy,**逐条冻结期望值并注明裁定依据**;
//! - 幂等性三组 —— 契约语料 / 324 条变异 / mXSS 样本,断言
//!   `SANITIZE(SANITIZE(x, p), p) == SANITIZE(x, p)` 字节级(规范 §13.2 的
//!   规范性安全不变量);
//! - 病态契约 —— 10 万层嵌套不栈溢出、1 MB 文本 / 2 万属性 / 10 万实体,
//!   这些是**断言**(`benches/` 里的基准只出数字,不会让 CI 变红)。
//!
//! SQL 转义的**否决记录**(规范 §13.3)也被本文件钉住:删掉那段规范的
//! 尝试会在这里变红 —— 「不做」与「做」同权,都是契约。

mod common;

use std::path::{Path, PathBuf};
use wlwl_eval::Evaluator;
use wlwl_value::Value;

const CASES: &[Case] = &[
    // ── HTML_ESCAPE(§13.1)──
    Case {
        name: "escape_five_characters",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_ESCAPE"]); HTML_ESCAPE("a<b>&\"'")"#,
        expect: "a&lt;b&gt;&amp;&quot;&#39;",
    },
    Case {
        name: "escape_apostrophe_is_numeric_ref",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_ESCAPE"]); HTML_ESCAPE("\"'\"")"#,
        expect: "&quot;&#39;&quot;",
    },
    Case {
        name: "escape_non_ascii_passthrough",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_ESCAPE"]); HTML_ESCAPE("héllo 世界 🌍")"#,
        expect: "héllo 世界 🌍",
    },
    Case {
        name: "escape_double_amp",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_ESCAPE"]); HTML_ESCAPE("&&")"#,
        expect: "&amp;&amp;",
    },
    Case {
        name: "escape_non_string",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_ESCAPE"]); HTML_ESCAPE(1)"#,
        expect: "!E0030 HTML_ESCAPE: expected string, got integer",
    },
    Case {
        name: "escape_arity_zero",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_ESCAPE"]); HTML_ESCAPE()"#,
        expect: "!E0022 HTML_ESCAPE: function expects 1 argument(s), got 0",
    },
    // ── HTML_UNESCAPE(§13.1:全表 / legacy 最长前缀 / 数字引用 / 字面量回退)──
    Case {
        name: "unescape_full_table",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&lt;b&amp;&copy;&#65;&#x41;")"#,
        expect: "<b&©AA",
    },
    Case {
        name: "unescape_two_codepoint_entity",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&NotNestedGreaterGreater;")"#,
        expect: "⪢̸",
    },
    Case {
        name: "unescape_legacy_longest_prefix",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&copyx")"#,
        expect: "©x",
    },
    Case {
        name: "unescape_legacy_before_semicolon_literal",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&copyz;")"#,
        expect: "©z;",
    },
    Case {
        name: "unescape_semicolon_form_preferred",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&amp;")"#,
        expect: "&",
    },
    Case {
        name: "unescape_legacy_no_semicolon",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&amp")"#,
        expect: "&",
    },
    Case {
        name: "unescape_numeric_decimal",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&#151;")"#,
        expect: "—",
    },
    Case {
        name: "unescape_numeric_windows1252_hole_is_fffd",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&#129;")"#,
        expect: "�",
    },
    Case {
        name: "unescape_numeric_out_of_range_is_fffd",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&#x110000;")"#,
        expect: "�",
    },
    Case {
        name: "unescape_numeric_surrogate_is_fffd",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&#xD800;")"#,
        expect: "�",
    },
    Case {
        name: "unescape_no_double_decoding",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&#38;amp;")"#,
        expect: "&amp;",
    },
    Case {
        name: "unescape_unknown_named_entity_verbatim",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&unknownent;")"#,
        expect: "&unknownent;",
    },
    Case {
        name: "unescape_stray_semicolon_verbatim",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("&;")"#,
        expect: "&;",
    },
    Case {
        name: "unescape_ampersand_alone_verbatim",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE("100% & up")"#,
        expect: "100% & up",
    },
    Case {
        name: "unescape_non_string",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_UNESCAPE"]); HTML_UNESCAPE([1])"#,
        expect: "!E0030 HTML_UNESCAPE: expected string, got array",
    },
    // ── roundtrip 定理(§13.1):UNESCAPE ∘ ESCAPE == 恒等 ──
    Case {
        name: "roundtrip_theorem",
        src: r#"IMPORT("wlwl:std.sanitize", ["HTML_ESCAPE", "HTML_UNESCAPE"]); HTML_UNESCAPE(HTML_ESCAPE("a<b>&\"' &copy; ⪢̸"))"#,
        expect: "a<b>&\"' &copy; ⪢̸",
    },
];

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

fn run(src: &str) -> Result<Value, String> {
    let ast = wlwl_parser::parse(src, "t.wll").expect("case source parses");
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(v) => Ok(v),
        Err(e) => Err(format!(
            "!{} {}",
            e.diagnostic().code.as_str(),
            e.diagnostic().message
        )),
    }
}

fn actual(case: &Case) -> String {
    match run(case.src) {
        Ok(v) => v.display(),
        Err(s) => s,
    }
}

#[test]
fn sanitize_matches_the_frozen_contract() {
    let mut bad = Vec::new();
    for case in CASES {
        let got = actual(case);
        if case.expect != got {
            bad.push(format!(
                "{name}:\n      frozen: {want}\n      actual: {got}",
                name = case.name,
                want = case.expect,
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} / {} case(s) diverged from the frozen sanitize contract:\n  - {}",
        bad.len(),
        CASES.len(),
        bad.join("\n  - ")
    );
}

// ── 规范表对拍(四件套第 3 条:两侧不同源)─────────────────────────

fn spec_text() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("wlwl-spec-reference.md");
    std::fs::read_to_string(&p).expect("stdlib spec is readable")
}

/// 本仓 ADR-0030(`std.sanitize` 扩展形状的裁决源头)正文。
fn adr_0030_text() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("adr")
        .join("0030-sanitize-extend-shape.md");
    std::fs::read_to_string(&p).expect("ADR-0030 is readable")
}

/// 把正文压成「单行、无 markdown 强调符」的形态,好让判据按**词句**匹配,
/// 而不是被排版(粗体标记 / 折行)绑架。
///
/// ⚠ 这**不是**放宽判据:被匹配的词句逐字不变,只是不再要求它恰好落在一行、
/// 恰好不带 `**` / 反引号。
///
/// 必要性的实证:ADR-0030 §4 写的是「没有 **sink**」,以及跨行的
/// 「Trusted Types do not protect against server-side / XSS」—— 这两个判据串
/// 在原文里都**不是**连续子串,直接 `contains` 会永远红。
fn flatten_markdown(text: &str) -> String {
    let stripped = text.replace("**", "").replace('`', "");
    let mut out = String::with_capacity(stripped.len());
    let mut in_space = false;
    for ch in stripped.chars() {
        if ch.is_whitespace() {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            out.push(ch);
            in_space = false;
        }
    }
    out
}

fn section(section_no: u32, next_no: u32) -> String {
    let text = spec_text();
    let start = text
        .find(&format!("## 标准库规范 · {section_no} "))
        .unwrap_or_else(|| panic!("§{section_no} header exists"));
    let end = text[start..]
        .find(&format!("\n## 标准库规范 · {next_no} "))
        .map(|i| start + i)
        .unwrap_or_else(|| panic!("§{next_no} header follows §{section_no}"));
    text[start..end].to_string()
}

/// 从成员表首列抽成员名(与 str_math_contract 同款口径)。
fn members_of(section_body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in section_body.lines() {
        let t = line.trim();
        if !t.starts_with('|') || t.contains("---") {
            continue;
        }
        let first = t.trim_start_matches('|').split('|').next().unwrap_or("");
        if first.trim() == "签名" {
            continue;
        }
        for cell in first.split(" / ") {
            let name: String = cell
                .trim()
                .trim_matches('`')
                .split('(')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
            {
                continue;
            }
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

fn impl_members() -> Vec<String> {
    match wlwl_std::resolve("wlwl:std.sanitize") {
        Some(wlwl_std::StdBackend::Native(spec)) => {
            spec.functions.iter().map(|(n, _)| n.to_string()).collect()
        }
        _ => panic!("wlwl:std.sanitize must resolve to a Native SPEC"),
    }
}

#[test]
fn sanitize_member_set_matches_the_spec_table() {
    // ⚠ 结束锚点是 **v1.0 规范册的下一个章号**:v1.0 **没有** stdlib §14
    // (v0.11 的「演进方向」章被拆散 ——「复活条件」进各章、「已否决」进总纲),
    // §13 之后紧跟的是 **§15**(`std.time`)。
    //
    // ⚠ 锚点写死章号 ⇒ 规范**新增**中间章时本测试会**变红**(正文会多取一章
    // ⇒ 成员表对拍失败),这是「设计变了就一起改测试」那道闸,不是静默取错片段。
    let spec = members_of(&section(13, 15));
    assert_eq!(
        spec,
        vec![
            "HTML_ESCAPE",
            "HTML_UNESCAPE",
            "HTML_SANITIZE",
            // [v0.11.3 M7 / addendum-13 W-02] §13.4 表。ADR-0030 D2 的产物:
            // JS 侧换题 —— 不做通用 `JS_ESCAPE_*`,改做收整个 inert 元素的
            // `HTML_SANITIZE_SCRIPT`。
            "HTML_SANITIZE_SCRIPT",
            // [v0.11.3 M7 / addendum-13 W-03] §13.5 表。ADR-0030 D1 的产物,
            // 顺序按 OWASP 口径:**白名单先于转义** ⇒ `HTML_SANITIZE_CSS` 在
            // 两个 `CSS_ESCAPE_*` 之后、`HTML_SANITIZE_URL` 之前。
            "CSS_ESCAPE_IDENT",
            "CSS_ESCAPE_STRING",
            "HTML_SANITIZE_CSS",
            "HTML_SANITIZE_URL",
        ],
        "§13 member table drifted"
    );
    let impls = impl_members();
    assert_eq!(
        impls,
        vec![
            "HTML_ESCAPE",
            "HTML_UNESCAPE",
            "HTML_SANITIZE",
            "HTML_SANITIZE_SCRIPT",
            "CSS_ESCAPE_IDENT",
            "CSS_ESCAPE_STRING",
            "HTML_SANITIZE_CSS",
            "HTML_SANITIZE_URL"
        ],
        "M7 surface: all eight §13 members landed"
    );
}

/// 反向守卫:集合比较不得是空转 —— 任何一边多出 / 少掉 / 改名都会红。
#[test]
fn sanitize_member_set_is_not_trivially_satisfied() {
    let impls = impl_members();
    assert!(!impls.is_empty(), "impl surface must not be empty");
    for n in &impls {
        assert!(
            [
                "HTML_ESCAPE",
                "HTML_UNESCAPE",
                "HTML_SANITIZE",
                "HTML_SANITIZE_SCRIPT",
                "CSS_ESCAPE_IDENT",
                "CSS_ESCAPE_STRING",
                "HTML_SANITIZE_CSS",
                "HTML_SANITIZE_URL"
            ]
            .contains(&n.as_str()),
            "unexpected member `{n}`"
        );
    }
}

/// 规范 §13.3 的 SQL 否决记录是**契约的一部分**:「不做」与「做」同权。
/// 删掉这段规范的尝试在这里变红。
#[test]
fn sql_escape_rejection_record_stays_in_the_spec() {
    let spec = spec_text();
    for marker in [
        "为什么没有 SQL 转义成员",
        "last resort",
        "prepared statement",
        "白名单映射",
    ] {
        assert!(
            spec.contains(marker),
            "§13.3 SQL rejection record lost its marker `{marker}` — \
             the rejection is a contract, restore it or re-adjudicate"
        );
    }
}

// ── HTML_SANITIZE_SCRIPT(§13.4 · ADR-0030 D2)──────────────────────
//
// 期望值取自 **OWASP DOM XSS Prevention Cheat Sheet 点名的五个字符**
// (`<` `>` `&` U+2028 U+2029)与 §13.4 的包装形态,不是实现输出。

const SCRIPT_IMP: &str = r#"IMPORT("wlwl:std.sanitize", ["HTML_SANITIZE_SCRIPT"]);"#;

/// 跑一段程序,返回**末条语句的值**。
///
/// ⚠ 不要把待测表达式包进 `PRINT(...)` —— 那样末条语句的值是 `PRINT` 的返回值
/// 而不是被测值,断言会拿 NULL 去比。
fn actual_raw(src: &str) -> String {
    match run(src) {
        Ok(v) => v.display(),
        Err(s) => panic!("program must not fail: {src}\n  got: {s}"),
    }
}

/// 包出期望形态:`<script type="application/json">` + payload + `</script>`。
fn script_wrapped(payload: &str) -> String {
    format!("<script type=\"application/json\">\"{payload}\"</script>")
}

/// OWASP 点名的五个字符,一个都不能留在**数据**里。
///
/// ⚠ **期望值全部用 ASCII 转义拼,不写不可见字面量。** 本测试的第一版把
/// `&` / U+2028 的期望值直接敲成「看起来像空格」的字符,结果断言拿 U+2028 去比 ——
/// 渲染上完全分不出,只有码点对账才抓得到。同理 wlwl 字符串**不支持** `\uXXXX`
/// 转义(§4 lexer 陷阱),所以**输入侧**的 U+2028 / U+2029 用 Rust 的 `\u{2028}`
/// 写成真字符。
#[test]
fn sanitize_script_hardens_the_five_owasp_characters() {
    let cases: Vec<(String, String)> = vec![
        // 闭合标签逃逸 —— 整个成员存在的理由
        (
            r#"HTML_SANITIZE_SCRIPT("</script><img src=x onerror=alert(1)>")"#.to_string(),
            script_wrapped("\\u003C/script\\u003E\\u003Cimg src=x onerror=alert(1)\\u003E"),
        ),
        (
            r#"HTML_SANITIZE_SCRIPT("a & b")"#.to_string(),
            script_wrapped("a \\u0026 b"),
        ),
        (
            "HTML_SANITIZE_SCRIPT(\"a\u{2028}b\")".to_string(),
            script_wrapped("a\\u2028b"),
        ),
        (
            "HTML_SANITIZE_SCRIPT(\"a\u{2029}b\")".to_string(),
            script_wrapped("a\\u2029b"),
        ),
    ];
    for (expr, want) in &cases {
        let src = format!("{SCRIPT_IMP} {expr}");
        assert_eq!(&actual_raw(&src), want, "expr = {expr}");
    }
}

/// **反向**断言:输出里除本成员自己写的开标签外,一个 `<` 都不该有。
///
/// 这条比逐条比对更有价值 —— 它对**任何**输入成立,是「加固真的做了」的
/// 结构性证据,而不是「这几条样例碰巧对」。
#[test]
fn sanitize_script_output_never_contains_a_raw_lt_outside_its_own_tag() {
    let src = format!(
        "{SCRIPT_IMP} HTML_SANITIZE_SCRIPT([\"a<b\", [\"c\": \"</script>\"], 1, NULL, TRUE])"
    );
    let out = actual_raw(&src);
    assert_eq!(out.matches('<').count(), 2, "unexpected raw `<` in: {out}");
    assert!(
        out.starts_with("<script type=\"application/json\">"),
        "{out}"
    );
    assert!(out.ends_with("</script>"), "{out}");
}

/// JSON 自身的转义**不被**加固逻辑破坏(§13.4:「只转这五个」)。
#[test]
fn sanitize_script_leaves_json_own_escapes_intact() {
    let cases: Vec<(String, String)> = vec![
        (
            r#"HTML_SANITIZE_SCRIPT("say \"hi\"")"#.to_string(),
            script_wrapped("say \\\"hi\\\""),
        ),
        (
            r#"HTML_SANITIZE_SCRIPT("a\\b")"#.to_string(),
            script_wrapped("a\\\\b"),
        ),
    ];
    for (expr, want) in &cases {
        let src = format!("{SCRIPT_IMP} {expr}");
        assert_eq!(&actual_raw(&src), want, "expr = {expr}");
    }
}

/// 收的是**值**不是裸串(规范 §13.4 的形状理由)。
///
/// ⚠ 注意非字符串值产出的是**裸 JSON**,不带引号 —— `script_wrapped` 只适用于
/// 字符串载荷。
#[test]
fn sanitize_script_takes_a_value_not_a_string() {
    let src = format!("{SCRIPT_IMP} HTML_SANITIZE_SCRIPT(TRUE)");
    assert_eq!(
        actual_raw(&src),
        "<script type=\"application/json\">true</script>"
    );
}

/// 全函数(§13.4):**没有任何值能让它出错** —— 元数错仍是原生诊断,但内容永远不会。
#[test]
fn sanitize_script_is_total_over_content() {
    for expr in [
        r#"HTML_SANITIZE_SCRIPT("")"#.to_string(),
        "HTML_SANITIZE_SCRIPT(\"\u{2028}\u{2029}\")".to_string(),
        r#"HTML_SANITIZE_SCRIPT(NULL)"#.to_string(),
        r#"HTML_SANITIZE_SCRIPT([])"#.to_string(),
        r#"HTML_SANITIZE_SCRIPT(["": ""])"#.to_string(),
    ] {
        let src = format!("{SCRIPT_IMP} {expr}");
        let out = actual_raw(&src);
        assert!(
            out.starts_with("<script type=\"application/json\">") && out.ends_with("</script>"),
            "expr = {expr} produced {out}"
        );
    }
}

/// ADR-0030 §4 的**否证记录**是契约的一部分(否证记录与肯定记录同权)。
/// `ADR-0030` D3 判「类型化收口不适用」—— 删掉这段记录的尝试在这里变红。
///
/// ⚠ **判据落点从「规范文本」挪到「本仓 ADR-0030 §4」** —— 理由:
///
/// - v1.0 把 v0.11 stdlib §14 的「演进方向」章**拆掉**了(「复活条件」分散到
///   各章、「已否决」移到总纲)⇒ v1.0 规范里**没有**那段否证记录;
/// - v1.0 规范仓的「一致性册」§5.3 明写:「实现侧的裁决(ADR、偏差台账)仍然
///   是实现如何落地规范的权威」⇒ 这条记录的权威落点是**本仓的 ADR-0030 §4**。
///
/// ⇒ 判据现在守的是**裁决源头**,不是规范里的抄件 —— 意图(否证记录不得被悄悄
/// 删掉)完整保住,而且更强。
#[test]
fn typed_closure_rejection_record_stays_in_the_adr() {
    // ① 裁决源头:本仓 ADR-0030 §4。
    let adr = flatten_markdown(&adr_0030_text());
    for marker in [
        "类型化收口",
        "不适用",
        "定义上的",
        "没有 sink",
        "Trusted Types do not protect against server-side XSS",
        "复活条件",
    ] {
        assert!(
            adr.contains(marker),
            "ADR-0030 §4 的否证记录丢了判据 `{marker}` — \
             否证记录是契约,恢复它或重新裁决"
        );
    }

    // ② 规范侧仍留痕:v1.0 §13 里「复活条件」(§13.3 的 SQL 否证记录)与
    //    Trusted Types 的否证追溯(§13.5 / §13.7)都还在。① 守源头,② 守「规范
    //    还认这个否证结论」—— 两者互补,缺一条都会漏。
    let spec_chapter = section(13, 15);
    for marker in ["复活条件", "Trusted Types"] {
        assert!(
            spec_chapter.contains(marker),
            "§13 的否证追溯丢了 `{marker}` — 复活条件与 Trusted Types 的否证 \
             都必须留在规范里"
        );
    }
}

// ── CSS 上下文 + URL 上下文(§13.5 · ADR-0030 D1)────────────────────
//
// 期望值取自 **CSSOM §"Common Serializing Idioms"** 的算法与例子,不是实现输出。
// ⚠ 期望值全部用 ASCII 转义拼,不把不可见字符敲进源码(W-02 吃过一次这个坑)。

const CSS_IMP: &str = concat!(
    r#"IMPORT("wlwl:std.sanitize", ["#,
    r#""CSS_ESCAPE_IDENT", "CSS_ESCAPE_STRING", "HTML_SANITIZE_CSS", "#,
    r#""HTML_SANITIZE_URL", "HTML_SANITIZE"]);"#
);

/// CSSOM「serialize an identifier」逐条 + 规范里给的例子。
///
/// ⚠ `\31 23` 里的**尾随空格**是规范的一部分(十六进制位数不定时的消歧符),
/// 漏了它 `\31 23` 与 `\3123` 会混 —— 这条专门钉它。
#[test]
fn css_escape_ident_follows_the_cssom_algorithm() {
    let cases: Vec<(String, String)> = vec![
        // 例子(CSSOM / CSS.escape())
        (
            r#"CSS_ESCAPE_IDENT("my class")"#.to_string(),
            r#"my\ class"#.to_string(),
        ),
        (
            r#"CSS_ESCAPE_IDENT("user-input#1")"#.to_string(),
            r#"user-input\#1"#.to_string(),
        ),
        // 首字符数字 ⇒ 按码点转义,**带尾随空格**
        (
            r#"CSS_ESCAPE_IDENT("123")"#.to_string(),
            r#"\31 23"#.to_string(),
        ),
        (
            r#"CSS_ESCAPE_IDENT("example.class")"#.to_string(),
            r#"example\.class"#.to_string(),
        ),
        (
            r#"CSS_ESCAPE_IDENT("a:b")"#.to_string(),
            r#"a\:b"#.to_string(),
        ),
        // 单独的 `-` ⇒ `\-`;`-a` 里的 `-` 不是首字符 ⇒ 原样
        (r#"CSS_ESCAPE_IDENT("-")"#.to_string(), r#"\-"#.to_string()),
        (
            r#"CSS_ESCAPE_IDENT("-1x")"#.to_string(),
            r#"-\31 x"#.to_string(),
        ),
        (r#"CSS_ESCAPE_IDENT("-a")"#.to_string(), "-a".to_string()),
        // `>= U+0080` 原样 —— 期望值用 Rust 的 `\u{00E9}` 写成真字符,
        // **不是** wlwl 的 `\uXXXX`(wlwl 字符串不认那个转义)。
        (
            "CSS_ESCAPE_IDENT(\"caf\u{00E9}\")".to_string(),
            "caf\u{00E9}".to_string(),
        ),
        // 引号 / 反斜杠 ⇒ 转义该字符(不是按码点)
        (
            r#"CSS_ESCAPE_IDENT("say \"hi\"")"#.to_string(),
            r#"say\ \"hi\""#.to_string(),
        ),
        (r#"CSS_ESCAPE_IDENT("")"#.to_string(), "".to_string()),
    ];
    for (expr, want) in &cases {
        let src = format!("{CSS_IMP} {expr}");
        assert_eq!(&actual_raw(&src), want, "expr = {expr}");
    }
}

/// CSSOM「serialize a string」逐条。
///
/// ⚠ **`'` 不转义** —— CSSOM 原文注明字符串**总是**用 `"` 序列化。这条最容易
/// 凭直觉写成「引号都转」,故单独钉。
#[test]
fn css_escape_string_follows_the_cssom_algorithm() {
    let cases: Vec<(String, String)> = vec![
        (
            r#"CSS_ESCAPE_STRING("abc")"#.to_string(),
            "\"abc\"".to_string(),
        ),
        (
            r#"CSS_ESCAPE_STRING("say \"hi\"")"#.to_string(),
            r#""say \"hi\"""#.to_string(),
        ),
        (
            r#"CSS_ESCAPE_STRING("a\\b")"#.to_string(),
            r#""a\\b""#.to_string(),
        ),
        // 单引号**不**转义(CSSOM 明注)。
        (
            r#"CSS_ESCAPE_STRING("it's")"#.to_string(),
            "\"it's\"".to_string(),
        ),
        // 控制字符 ⇒ 按码点转义(带尾随空格)。
        // ⚠ 用 wlwl **认得**的 `\t`,不用 `\u{0009}` —— wlwl 字符串的合法转义
        // 只有 `\" \\ / \b \f \n \r \t \0`(lexer 错误信息里写死了这份清单)。
        // 写成 Rust 的 `\u{0009}` 会原样落成 wlwl 源码里的 `\u{…}`,直接 E0001。
        (
            r#"CSS_ESCAPE_STRING("a\tb")"#.to_string(),
            r#""a\9 b""#.to_string(),
        ),
        (r#"CSS_ESCAPE_STRING("")"#.to_string(), "\"\"".to_string()),
    ];
    for (expr, want) in &cases {
        let src = format!("{CSS_IMP} {expr}");
        assert_eq!(&actual_raw(&src), want, "expr = {expr}");
    }
}

/// `HTML_SANITIZE_CSS` 的两条**分开**失败口径(§13.5)。
#[test]
fn html_sanitize_css_separates_silent_drop_from_rejection() {
    let cases: Vec<(String, &str)> = vec![
        // 白名单内 ⇒ 保留,属性名小写归一
        (
            r#"HTML_SANITIZE_CSS("color: red; font-weight: bold")"#.to_string(),
            "color: red; font-weight: bold;",
        ),
        // 不在白名单 ⇒ **静默丢弃**,不是失败
        (
            r#"HTML_SANITIZE_CSS("color: red; position: fixed")"#.to_string(),
            "color: red;",
        ),
        // 能取 `url()` 的属性一律不在白名单
        (
            r#"HTML_SANITIZE_CSS("color: red; background: red")"#.to_string(),
            "color: red;",
        ),
        // 什么都没存活 ⇒ **空串**,不是 `";"`
        (r#"HTML_SANITIZE_CSS("")"#.to_string(), ""),
        (r#"HTML_SANITIZE_CSS("garbage")"#.to_string(), ""),
    ];
    for (expr, want) in &cases {
        let src = format!("{CSS_IMP} {expr}");
        assert_eq!(&actual_raw(&src), want, "expr = {expr}");
    }
}

/// 命中拒绝构造 ⇒ `ERR([kind: SanitizeError])`,不是空串。
#[test]
fn html_sanitize_css_rejects_attack_constructs() {
    for payload in [
        r#"HTML_SANITIZE_CSS("width: expression(alert(1))")"#,
        r#"HTML_SANITIZE_CSS("@import url(evil.css)")"#,
        r#"HTML_SANITIZE_CSS("color: javascript:alert(1)")"#,
        r#"HTML_SANITIZE_CSS("color: vbscript:msgbox(1)")"#,
        r#"HTML_SANITIZE_CSS("behavior: url(#default#time2)")"#,
        r#"HTML_SANITIZE_CSS("-moz-binding: anything")"#,
        // 任意 `url(` 一律拒,不做「看着安全就放行」
        r#"HTML_SANITIZE_CSS("color: red; background: url(x)")"#,
        // ⚠ 注释里藏 `url(` **仍然被拒** —— 拒绝扫描是文本级的,先于解析
        r#"HTML_SANITIZE_CSS("color: /* url(x) */ red")"#,
    ] {
        let src = format!("{CSS_IMP} IS_ERR({payload})");
        assert_eq!(&actual_raw(&src), "TRUE", "payload = {payload}");
    }
}

/// [N-017] **CSS 转义**不能绕过拒绝扫描。
///
/// 复核样本:`width: e\78 pression(alert(1))`。浏览器按 CSS 转义把它读成
/// `width: expression(alert(1))`,而修复前的扫描只做字面比较 ⇒ 放行。
///
/// **期望值的出处**(规范原文,不是「跑一遍看它输出什么」):
/// - W3C **CSS Syntax Module Level 3**(CRD 2026-10-01,
///   <https://www.w3.org/TR/css-syntax-3/>)§4.3.7 *Consume an escaped code point*
///   (<https://www.w3.org/TR/css-syntax-3/#consume-escaped-code-point>):
///   「Consume as many hex digits as possible, but no more than 5. **Note that this
///   means 1-6 hex digits have been consumed in total.**」
///   「**If the next input code point is whitespace, consume it as well.**」
///   —— 那一个尾随空白被**消费掉**,于是 `\65 xpression` ≡ `expression`。
/// - 同节 *anything else* 分支原文只有一句:「Return the current input code
///   point.」—— **没有**「再消费一个尾随空白」这一步;消费空白的规则**只在**
///   hex 分支里。非规范性的 §2.1 *Escaping* 同口径:「Any Unicode code point that
///   is not a hex digits or a newline. The escape sequence is replaced by that
///   code point.」
/// - §4.3.8 *Check if two code points are a valid escape*:「if the second code
///   point is a newline, return false.」⇒ `\` + 换行**不**是转义。
/// - §4.2 *Definitions*:hex digit = digit ∪ `A-F` ∪ `a-f`;whitespace = newline /
///   U+0009 / U+0020。⇒ 决定下面每条走**哪个分支**:`e` `b` 是 hex digit,所以它们
///   的转义必须写成 `\78 ` `\62 `;`s` `r` `g` `t` `j` `u` 不是,所以 `\s` `\r`
///   直接返回该码位(*anything else* 分支)。
/// - ⚠ **转义必须落在 token 内部**才有绕过意义:落在 token **前面**(如 `\url(`)
///   时字面串里照样含 `url(`,原文那一遍就拒了 —— 那种写法不是绕过,下面留了两
///   条专门钉「两遍取并集、结论不变」。
///
/// ⚠ wlwl 源码里 `\\` = **一个**反斜杠(wlwl 字符串的合法转义只有
/// `\" \\ / \b \f \n \r \t \0`),故 CSS 转义前缀一律写两个。
#[test]
fn html_sanitize_css_rejects_css_escaped_attack_constructs() {
    for payload in [
        // ① **hex 分支**(`\` + 1–6 位十六进制)。每一条在**原文**里都拼不出
        //    拒绝构造(转义吃掉了 token 里的一个码位)—— 这才是「绕过」。
        // `e` + `\78 ` + `pression(` ≡ `expression(`
        r#"HTML_SANITIZE_CSS("width: e\\78 pression(alert(1))")"#,
        r#"HTML_SANITIZE_CSS("width: \\65 xpression(alert(1))")"#,
        // ⚠ 「no more than 5」+「1-6 in total」的判别样本:6 位十六进制取满后,
        // 下一个字符 `x` **不是** whitespace ⇒ 不消费,`\000065xpression(` ≡
        // `expression(`。写成「只吃一位十六进制」的实现会在这里漏放。
        r#"HTML_SANITIZE_CSS("width: \\000065xpression(alert(1))")"#,
        // 十六进制**大写**同样是 hex digit(§4.2 明写 `A-F`),且比较前先小写化。
        // ⚠ 转义只替换**一个**码位 —— 写成 `E\78 XPRESSION` 会解成 `ExXPRESSION`
        // (两个 x),那不是拒绝构造,是这条自己写错。
        r#"HTML_SANITIZE_CSS("WIDTH: \\45 XPRESSION(alert(1))")"#,
        r#"HTML_SANITIZE_CSS("color: EXPRESSIO\\4E (alert(1))")"#,
        r#"HTML_SANITIZE_CSS("color: j\\61 vascript:alert(1)")"#,
        r#"HTML_SANITIZE_CSS("color: vb\\73 cript:msgbox(1)")"#,
        r#"HTML_SANITIZE_CSS("\\62 ehavior: \\75 rl(#default#time2)")"#,
        r#"HTML_SANITIZE_CSS("@\\69 mport \\75 rl(evil.css)")"#,
        r#"HTML_SANITIZE_CSS("-\\6d oz-binding: anything")"#,
        // ② **anything else 分支**(`\` + **非** hex digit 的码位 ⇒「Return the
        //    current input code point.」)。挑的字符都落在 token **内部**,
        //    所以原文里同样拼不出 token。⚠ 用首字母(`\j` `\u`)是**不成立**的
        //    绕过:反斜杠在 token 之前,字面里照样含 token。
        r#"HTML_SANITIZE_CSS("width: expres\\sion(alert(1))")"#,
        r#"HTML_SANITIZE_CSS("color: java\\script:alert(1)")"#,
        r#"HTML_SANITIZE_CSS("color: vb\\script:msgbox(1)")"#,
        r#"HTML_SANITIZE_CSS("behavio\\r: \\75 rl(#default#time2)")"#,
        r#"HTML_SANITIZE_CSS("-moz-bindin\\g: anything")"#,
        r##"HTML_SANITIZE_CSS("@impo\\rt \"evil.css\"")"##,
        r#"HTML_SANITIZE_CSS("color: red; background: u\\rl(evil.png)")"#,
        // ⚠ 这两条用首字母写法,**字面就已经**含 token,原文那一遍就会拒;留着是
        //    为了钉住「解码不改变这个结论」(两遍取并集,不是二选一)。
        r#"HTML_SANITIZE_CSS("color: \\javascript:alert(1)")"#,
        r#"HTML_SANITIZE_CSS("color: red; background: \\url(evil.png)")"#,
        // ⚠ 注释里藏**转义版**同样被拒 —— 原文那一遍与解码那一遍是并集,
        // 任一侧撤掉,这条都会红(负向验证就是靠它)。
        r#"HTML_SANITIZE_CSS("color: /* \\75 rl(x) */ red")"#,
        // 转义版藏在**畸形声明**里(没有冒号 / 解析不出来)也一样被拒:
        // 拒绝扫描是文本级的,不靠「解析不出来就丢掉」蒙混。
        r#"HTML_SANITIZE_CSS("garbage \\65 xpression(alert(1)) garbage")"#,
    ] {
        let src = format!("{CSS_IMP} IS_ERR({payload})");
        assert_eq!(&actual_raw(&src), "TRUE", "payload = {payload}");
    }
}

/// [N-017] **反向**守卫:解码**不得**把白名单里的正常值判成攻击。
///
/// 这是「不误拒」那一半的契约,和上一条一起才构成本次修复的完整语义。
/// ⚠ 期望值是**规范推出来的**结论(§4.3.7 解完不含任何拒绝构造 ⇒ 不该
/// `ERR`),不是实现输出;断言的是 `IS_ERR` 这个**判定位**,故实现改了输出形状
/// 也不会让它跟着变。
#[test]
fn html_sanitize_css_escape_decoding_does_not_reject_legitimate_values() {
    for payload in [
        // 常见的嵌套转义:中文自定义字体名(`\5FAE\8BA4` ≡ `微认`)。
        // §4.3.7 解出的是两个字,不含任何拒绝构造 ⇒ 恒不 `ERR`。
        r#"IS_ERR(HTML_SANITIZE_CSS("font-family: \"\\5FAE\\8BA4\""))"#,
        // 转义后的普通标识符:`\65 x` ≡ `ex`,不是任何拒绝构造
        r#"IS_ERR(HTML_SANITIZE_CSS("color: \\65 x"))"#,
        // hex 分支 + 消歧空格:`\9 ` 解出的是制表符(U+0009),不是 `(`(U+0028 写成
        // `\28 ` 才是)⇒ 解完不含任何拒绝构造。
        r#"IS_ERR(HTML_SANITIZE_CSS("content: \"a\\9 b\""))"#,
        // `\` + **换行**不是 valid escape(§4.3.8),两个字节都留着 ⇒ 拼不出拒绝构造。
        // ⚠ wlwl 字符串不认 `\uXXXX`,换行只能用 wlwl 认得的 `\n`;`\\` 才是
        // 一个反斜杠 —— 于是这里的 CSS 文本是「LF + `\` + LF」。
        r#"IS_ERR(HTML_SANITIZE_CSS("color: red\n\\\n"))"#,
        // 没有 `\` 的普通输入不受解码影响
        r#"IS_ERR(HTML_SANITIZE_CSS("color: red; padding: 4px"))"#,
    ] {
        let src = format!("{CSS_IMP} {payload}");
        assert_eq!(&actual_raw(&src), "FALSE", "payload = {payload}");
    }
}

/// `HTML_SANITIZE_CSS` 的括号 / 引号感知:分号与冒号在括号或引号内**不算分隔符**。
#[test]
fn html_sanitize_css_is_paren_and_quote_aware() {
    // 合法值原样保留
    let src = format!("{CSS_IMP} HTML_SANITIZE_CSS(\"text-align: center; padding: 4px\")");
    assert_eq!(actual_raw(&src), "text-align: center; padding: 4px;");
}

/// [D13-022 · N-018] 输出侧加固:值不能越出它在 CSS / HTML 里的位置。
///
/// **期望值的出处**(不是「跑一遍看它输出什么」):
/// - 转义**形状** = CSSOM §"Common Serializing Idioms" 的 *escape a character
///   as code point*:反斜杠 + **小写**十六进制 + **一个空格**。`<` = U+003C ⇒
///   `\3c `,`>` = U+003E ⇒ `\3e `,`\` = U+005C ⇒ `\5c `,`"` = U+0022 ⇒ `\22 `,
///   `'` = U+0027 ⇒ `\27 `,`*` = U+002A ⇒ `\2a `。
/// - **为什么必须转 `<`**:HTML 分词器在 raw text 状态只靠 `</style` 转移出
///   `style` 元素(WHATWG HTML §13.2.6.4 *RAWTEXT* / *appropriate end tag*)。
///   输出里没有 `<` ⇒ 该序列无从拼出 ⇒ 任何输入都不能终止 `<style>`。
/// - **为什么必须转 `\`**:CSS 里 `\` 后跟十六进制才是转义(CSS Syntax §4.3.7
///   *consume an escaped code point*);不处理它,攻击者自带的 `\` 会与插入的
///   `\3c ` 连成别的序列(`\\3c ` = 字面反斜杠 + 标识符 `3c`)。
/// - **为什么转 `*` 而不转 `/`**:CSS 注释里不做转义解析,能提前闭合注释的
///   只有 `*/`;输出里没有未转义的 `*` ⇒ 该序列拼不出来。`/` 在合法值里
///   (`font: 12px/1.5` 族)是常见字符,转它没有收益。
/// - **为什么转 `;` `{` `}`**:调用方把输出放进规则体(`p { …输出… }`)时,`;`
///   是声明分隔符、`{` `}` 能结束整条规则并另起一条。三者在白名单属性的合法值里
///   都不出现,转义没有功能代价。
#[test]
fn html_sanitize_css_output_cannot_break_out_of_its_context() {
    let cases: Vec<(String, String)> = vec![
        // 复核样本本体:破壳串原样穿过(修复前的输出)
        (
            r#"HTML_SANITIZE_CSS("color: red</style><img src=x onerror=alert(1)>")"#.to_string(),
            r#"color: red\3c /style\3e \3c img src=x onerror=alert(1)\3e ;"#.to_string(),
        ),
        // 调用方自带的 `\` 必须**先**被转义,否则它会与插入的 `\3c ` 相连
        // (wlwl 源码里 `\\` = 一个反斜杠)
        (
            r#"HTML_SANITIZE_CSS("color: a\\</style>")"#.to_string(),
            r#"color: a\5c \3c /style\3e ;"#.to_string(),
        ),
        // CSS 注释:`*/` 拼不出来
        (
            r#"HTML_SANITIZE_CSS("color: a*/")"#.to_string(),
            r#"color: a\2a /;"#.to_string(),
        ),
        // CSS 字符串:引号与裸换行都被转义(U+0022 / U+000A;wlwl 字符串的合法
        // 转义只有 `\" \\ / \b \f \n \r \t \0`,故换行用 wlwl 认得的 `\n`)
        (
            "HTML_SANITIZE_CSS(\"color: \\\"a\\nb\\\"\")".to_string(),
            r#"color: \22 a\a b\22 ;"#.to_string(),
        ),
        // 规则体:分号与花括号。⚠ 分号要在**引号内**才留得住 —— 顶层分号是
        // 声明分隔符(`; ` 由本成员自己写,不转义),引号内的分号才会进值里。
        (
            r#"HTML_SANITIZE_CSS("font-family: \"a;b\"")"#.to_string(),
            r#"font-family: \22 a\3b b\22 ;"#.to_string(),
        ),
        (
            r#"HTML_SANITIZE_CSS("color: a}b{c")"#.to_string(),
            r#"color: a\7d b\7b c;"#.to_string(),
        ),
        // 合法值**不受影响**(加固不能顺手毁掉白名单的正常用法)
        (
            r#"HTML_SANITIZE_CSS("font-family: Arial; padding: 4px")"#.to_string(),
            "font-family: Arial; padding: 4px;".to_string(),
        ),
    ];
    for (expr, want) in &cases {
        let src = format!("{CSS_IMP} {expr}");
        assert_eq!(&actual_raw(&src), want, "expr = {expr}");
    }
}

/// **反向**断言:输出里**一个 `<` 都没有**,对**任何**输入成立。
///
/// 这条比逐条比对更有价值(与 `sanitize_script_output_never_contains_a_raw_lt…`
/// 同款判据):它对所有输入成立,是「加固真的做了」的结构性证据,而不是「这几条
/// 样例碰巧对」。⚠ 期望是**结构性命题**(输出不含 `<`),不是实现回读。
#[test]
fn html_sanitize_css_output_never_contains_a_raw_lt() {
    for payload in [
        r#"HTML_SANITIZE_CSS("color: red</style>")"#,
        r#"HTML_SANITIZE_CSS("color: <img src=x onerror=alert(1)>")"#,
        r#"HTML_SANITIZE_CSS("font-family: \"</style><script>alert(1)</script>\"")"#,
        // wlwl 源码里 `\\` = 一个反斜杠:攻击者自带的转义前缀不许生效
        r#"HTML_SANITIZE_CSS("color: \\3c /style\\3e ")"#,
        r#"HTML_SANITIZE_CSS("color: red; text-align: <!--*/-->")"#,
    ] {
        let src = format!("{CSS_IMP} {payload}");
        let out = actual_raw(&src);
        assert!(
            !out.contains('<'),
            "裸 `<` 留在 CSS 输出里: {payload} -> {out}"
        );
    }
}

/// **同款判定**:`HTML_SANITIZE` 的 `href` 判定与 `HTML_SANITIZE_URL` 结论一致。
///
/// 这是 §13.5 最要紧的一条 —— 两处判定一旦分裂,意味着其中一处是错的,
/// 而「哪一处错」要靠看代码才知道。同一组 URL 喂两个入口,结论必须相同。
///
/// ⚠ **这条只断言「一致」,不断言「哪个结论」** —— 两处同时错也照样绿。
/// 所以 D13-023 修好 `/ ? #` 豁免之后,组内补了三条**带冒号的相对 URL**
/// (让「一致」这条覆盖到新分支),并另加
/// `html_sanitize_url_allows_relative_urls_with_a_colon_after_a_delimiter`
/// 钉住**绝对**结论。两道守卫各管一半,缺一不可。
#[test]
fn html_sanitize_url_agrees_with_the_html_sanitizer() {
    let urls = [
        "https://example.com/a",
        "http://example.com",
        "mailto:a@b.c",
        "/relative/path",
        "#frag",
        "javascript:alert(1)",
        "JaVaScRiPt:alert(1)",
        "data:text/html,<script>alert(1)</script>",
        "vbscript:msgbox(1)",
        "java\tscript:alert(1)",
        // [D13-023 · N-016] 带冒号的相对 URL:`:` 落在 `/` `?` `#` **之后**
        // ⇒ 不是 scheme 的结尾(WHATWG URL §4.2 scheme start state)。
        "/wiki/Special:Search",
        "?q=a:b",
        "#frag:x",
    ];
    for u in urls {
        // 入口一:`HTML_SANITIZE_URL` 自己。
        let direct = format!("{CSS_IMP} IS_ERR(HTML_SANITIZE_URL(\"{}\"))", esc(u));
        // 入口二:同一 URL 塞进 `href`,看净化器是否**删掉整个属性**。
        // ⚠ 别把这行塞进 `concat!` —— 那里 `{CSS_IMP}` 会被当作 format 参数。
        let via_attr = format!(
            "{CSS_IMP} HTML_SANITIZE(\"<a href=\\\"{}\\\">x</a>\")",
            esc(u)
        );
        let a_keeps = actual_raw(&direct) == "FALSE";
        // 属性被删 ⇒ `<a>x</a>`;被留 ⇒ 里面有 `href=`
        let b_keeps = actual_raw(&via_attr).contains("href");
        assert_eq!(
            a_keeps, b_keeps,
            "判定分裂: URL_SANITIZE 放行={a_keeps} 而 HTML_SANITIZE 保留属性={b_keeps}  ({u})"
        );
    }
}

/// 把 URL 里的引号 / 反斜杠转义,好塞进 wlwl 字符串字面量。
fn esc(u: &str) -> String {
    u.replace('\\', "\\\\").replace('"', "\\\"")
}

/// [D13-023 · N-016] 相对 URL 恒允许 —— 规范 §13.2 / §13.5 的**明文**条款,
/// 而实现原先只实现了「没有 `:`」那一半。
///
/// **期望值的出处**(不是实现回读):
/// - WHATWG URL Standard §4.2 **scheme start state**:scheme 只在**第一个**
///   `/`(path state)、`?`(query state)、`#`(fragment state)之前才可能
///   结束;那三个字符之后的 `:` 是段内分隔符 ⇒ 该 URL 是 **relative-ref**。
/// - RFC 3986 §3.3:*"a path segment may contain a colon character, but the
///   **first path segment** of a relative-part reference cannot"* —— 与上面
///   同一结论的文法表述;§3.4(query) / §3.5(fragment)对 `:` 无限制。
/// - 因此 `/wiki/Special:Search`、`?q=a:b`、`#frag:x` 三条**恒允许**,且
///   归一(只剥首尾控制字符)后**原样**返回 —— 路径大小写敏感,不做大小写归一。
#[test]
fn html_sanitize_url_allows_relative_urls_with_a_colon_after_a_delimiter() {
    let cases: Vec<(String, String)> = vec![
        (
            r#"HTML_SANITIZE_URL("/wiki/Special:Search")"#.to_string(),
            "/wiki/Special:Search".to_string(),
        ),
        (
            r#"HTML_SANITIZE_URL("?q=a:b")"#.to_string(),
            "?q=a:b".to_string(),
        ),
        (
            // ⚠ 用 `r##"…"##`:串里有 `"#`,单 `#` 的 raw 串会在此提前闭合。
            r##"HTML_SANITIZE_URL("#frag:x")"##.to_string(),
            "#frag:x".to_string(),
        ),
        (
            r#"HTML_SANITIZE_URL("p/x:y?q=1#z")"#.to_string(),
            "p/x:y?q=1#z".to_string(),
        ),
        // 豁免是**位置**判定,不是「含冒号就放过」:`:` 前无 `/ ? #` 时,
        // `special` 是一个形态合法的 scheme token(WHATWG:URI 的 scheme 语法
        // `ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )`),不在缺省白名单 ⇒ 拒。
        (
            r#"IS_ERR(HTML_SANITIZE_URL("Special:Search"))"#.to_string(),
            "TRUE".to_string(),
        ),
        // 经典攻击串一个都不能因为豁免而复活:`:` 前无分隔符。
        (
            r#"IS_ERR(HTML_SANITIZE_URL("javascript:alert(1)"))"#.to_string(),
            "TRUE".to_string(),
        ),
        (
            r#"IS_ERR(HTML_SANITIZE_URL("data:text/html,x"))"#.to_string(),
            "TRUE".to_string(),
        ),
    ];
    for (expr, want) in &cases {
        let src = format!("{CSS_IMP} {expr}");
        assert_eq!(&actual_raw(&src), want, "expr = {expr}");
    }
}

/// `HTML_SANITIZE_URL` 的归一只做**安全判定需要**的那一步(§13.5)。
#[test]
fn html_sanitize_url_normalizes_only_what_the_judgment_needs() {
    let cases: Vec<(String, String)> = vec![
        // 剥首尾控制字符与空白
        (
            r#"HTML_SANITIZE_URL("  https://example.com/a  ")"#.to_string(),
            "https://example.com/a".to_string(),
        ),
        // ⚠ **不做大小写归一**:路径大小写敏感,小写化会改错资源
        (
            r#"HTML_SANITIZE_URL("HTTPS://Example.COM/A")"#.to_string(),
            "HTTPS://Example.COM/A".to_string(),
        ),
        (
            r#"HTML_SANITIZE_URL("/p?x=1")"#.to_string(),
            "/p?x=1".to_string(),
        ),
    ];
    for (expr, want) in &cases {
        let src = format!("{CSS_IMP} {expr}");
        assert_eq!(&actual_raw(&src), want, "expr = {expr}");
    }
}

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_sanitize")
}

// ══════════════════════════════════════════════════════════════
// W-13 · `HTML_SANITIZE` 契约表:OWASP 规避向量逐条冻结
// ══════════════════════════════════════════════════════════════
//
// **期望值的来源纪律**:每条的 `expect` 是**裁定结果**,不是实现回读。
// 逐条判读「这个输出安不安全、符不符合规范 §13.2」之后才冻结;判读发现
// 实现与规范相悖的(W-13 抓出 3 条),改实现并登记 D13-008 ~ D13-010,
// 而不是把实现当下的输出冻成「契约」。
//
// 覆盖分组(名字前缀即分组,`m3_*` 测试按前缀取子集):
//   `img_*` / `script_*` / `body_*` / `meta_*` / `object_*` / `style_*` /
//   `link_*` / `iframe_*` / `bgsound` / `base_*` / `table_*` / `div_*`
//     —— OWASP XSS Filter Evasion Cheat Sheet 经典向量(逐条);
//   `a_*`  —— **唯一有非空输出的族**:白名单只留 22 个排版级标签,上面的
//     向量绝大多数整条消失,唯一能观察到属性 / scheme 判定的是 `<a href>`;
//   `mxss_*` —— mXSS 教科书样本(Sonar cheatsheet 命名空间混淆族中
//     **作用于白名单子集**的部分);
//   `malformed_*` —— 畸形输入的容错口径;
//   `policy_*` —— 非缺省 policy 与 policy 形态错;
//   `sanitize_*` —— 成员级诊断。
//
// **不承诺往返**:输出的字节与输入无对应关系(嵌套矫正的固有属性,
// 规范 §13.2 明写)。本表冻结的是「输入 → 输出」这一个方向。

/// 冻结用例:`expect` = 裁定后的输出;`why` = 裁定依据(逐条注明)。
struct SanCase {
    name: &'static str,
    html: &'static str,
    /// `""` = 缺省策略;否则是 wlwl 的 DICT 字面量文本。
    policy: &'static str,
    expect: &'static str,
    why: &'static str,
}

const SANITIZE_CASES: &[SanCase] = &[
    // ── OWASP XSS Filter Evasion Cheat Sheet:`javascript:` / `vbscript:`
    //    系向量。`img` / `link` / `bgsound` / `base` / `meta` / `object` /
    //    `body` / `input` 全部**不在**白名单 → 拆壳留内容,而这些向量
    //    自身没有可见内容,故输出恒为空串。**空串是正确结果**:危险属性
    //    随元素一起消失,没有「标签没了但属性还在」的形态。──
    SanCase { name: "img_js_quoted", html: r#"<IMG SRC="javascript:alert('XSS');">"#, policy: "", expect: "",
        why: "img 不在白名单 → 拆壳;无子内容 → 空串。SRC 随元素消失" },
    SanCase { name: "img_js_bare", html: r#"<IMG SRC=javascript:alert('XSS')>"#, policy: "", expect: "",
        why: "无引号属性值同样随 img 消失;容错解析不得因此报错" },
    SanCase { name: "img_js_case", html: r#"<IMG SRC=JaVaScRiPt:alert('XSS')>"#, policy: "", expect: "",
        why: "标签名 / 属性名小写化;大写 scheme 随元素消失" },
    SanCase { name: "img_js_entity_quote", html: r#"<IMG SRC=javascript:alert(&quot;XSS&quot;)>"#, policy: "", expect: "",
        why: "无引号属性值里的 `&quot;` 解析期解码为 `\"`;仍随 img 消失" },
    SanCase { name: "img_js_backtick", html: "<IMG SRC=`javascript:alert(\"RSnake says, 'XSS'\")`>", policy: "", expect: "",
        why: "反引号是容错怪字符,被吞进属性值;随 img 消失" },
    SanCase { name: "img_js_fromcharcode", html: r#"<IMG SRC=javascript:alert(String.fromCharCode(88,83,83))>"#, policy: "", expect: "",
        why: "无引号值到空白 / `>` 为止;随 img 消失" },
    SanCase { name: "img_hash_onmouseover", html: r#"<IMG SRC=# onmouseover="alert('xxs')">"#, policy: "", expect: "",
        why: "`#` 开头的相对 URL 本就允许,但 img 整体拆壳,属性一并消失" },
    SanCase { name: "img_empty_src_onmouseover", html: r#"<IMG SRC= onmouseover="alert('xxs')">"#, policy: "", expect: "",
        why: "布尔属性 SRC(无值)+ onmouseover;两者随 img 消失" },
    SanCase { name: "img_bare_onmouseover", html: r#"<IMG onmouseover="alert('xxs')">"#, policy: "", expect: "",
        why: "事件属性随 img 消失" },
    SanCase { name: "img_slash_onerror", html: r#"<IMG SRC=/ onerror="alert(String.fromCharCode(88,83,83))"></img>"#, policy: "", expect: "",
        why: "自闭合 `<img/>` 不入栈、后续 `</img>` 栈内找不到 → 忽略;整体空串" },
    SanCase { name: "img_u0061lert", html: r#"<img src=x onerror="alert('XSS')">"#, policy: "", expect: "",
        why: "小写同款;属性值与标签名无关地一起消失" },
    SanCase { name: "img_numeric_entity_scheme", html: r#"<IMG SRC=&#106;&#97;&#118;&#97;&#115;&#99;&#114;&#105;&#112;&#116;&#58;alert('XSS')>"#, policy: "", expect: "",
        why: "十进制实体在解析期全表解码成 `javascript:`;元素仍拆壳" },
    SanCase { name: "img_zeropad_entity", html: r#"<IMG SRC=&#0000106&#0000097&#0000118alert('XSS')>"#, policy: "", expect: "",
        why: "零填充数字引用按 WHATWG 规则解码;元素拆壳" },
    SanCase { name: "img_hex_entity", html: r#"<IMG SRC=&#x6A&#x61&#x76&#x61&#x73&#x63&#x72&#x69&#x70&#x74&#x3A alert('XSS')>"#, policy: "", expect: "",
        why: "十六进制 + 大写 X 前缀;解码后含空格截断;元素拆壳" },
    SanCase { name: "img_space_in_scheme", html: r#"<IMG SRC="jav ascript:alert('XSS');">"#, policy: "", expect: "",
        why: "scheme 中间有空格 → 非法形态;元素拆壳" },
    SanCase { name: "img_tab_entity_scheme", html: r#"<IMG SRC="jav&#x09;ascript:alert('XSS');">"#, policy: "", expect: "",
        why: "制表实体解码后落在 scheme 内;元素拆壳" },
    SanCase { name: "img_nl_entity_scheme", html: r#"<IMG SRC="jav&#x0A;ascript:alert('XSS');">"#, policy: "", expect: "",
        why: "换行实体解码后落在 scheme 内;元素拆壳" },
    SanCase { name: "img_cr_entity_scheme", html: r#"<IMG SRC="jav&#x0D;ascript:alert('XSS');">"#, policy: "", expect: "",
        why: "回车实体;元素拆壳" },
    SanCase { name: "img_ctrl14_scheme", html: "<IMG SRC=\" \u{e}  javascript:alert('XSS');\">", policy: "", expect: "",
        why: "U+000E 控制字符在值内;元素拆壳(值不参与判定)" },
    SanCase { name: "img_vbscript", html: r#"<IMG SRC='vbscript:alert("XSS")'>"#, policy: "", expect: "",
        why: "单引号属性值合法;vbscript: 不在 scheme 白名单,且 img 拆壳" },
    SanCase { name: "img_dynsrc", html: r#"<IMG DYNSRC="javascript:alert('XSS')">"#, policy: "", expect: "",
        why: "URL 类属性之一;元素拆壳" },
    SanCase { name: "img_lowsrc", html: r#"<IMG LOWSRC="javascript:alert('XSS')">"#, policy: "", expect: "",
        why: "同上" },
    SanCase { name: "bgsound", html: r#"<BGSOUND SRC="javascript:alert('XSS')">"#, policy: "", expect: "",
        why: "bgsound 不在白名单 → 拆壳" },
    SanCase { name: "link_stylesheet_js", html: r#"<LINK REL="stylesheet" HREF="javascript:alert('XSS');">"#, policy: "", expect: "",
        why: "link 是 void 元素且不在白名单 → 拆壳;href 是 URL 属性但无从判定起" },
    SanCase { name: "base_href_js", html: r#"<BASE HREF="javascript:alert('XSS');//">"#, policy: "", expect: "",
        why: "base 不在白名单 → 拆壳(规范 §13.2 未把它列入整删集,故走 unwrap)" },
    SanCase { name: "body_background_js", html: r#"<BODY BACKGROUND="javascript:alert('XSS')">"#, policy: "", expect: "",
        why: "body 不在白名单 → 拆壳;无子内容 → 空串" },
    SanCase { name: "body_onload_bare", html: r#"<BODY ONLOAD=alert('XSS')>"#, policy: "", expect: "",
        why: "事件属性无引号;随 body 消失" },
    SanCase { name: "body_onload_garbage", html: "<BODY onload!#$%&()*~+-_.,:;?@[/|\\]^`=alert(\"XSS\")>", policy: "", expect: "",
        why: "属性名里的怪字符被容错吞进名字(不当作新属性);随 body 消失" },
    SanCase { name: "meta_refresh_js", html: r#"<META HTTP-EQUIV="refresh" CONTENT="0;url=javascript:alert('XSS');">"#, policy: "", expect: "",
        why: "meta 是 void 且不在白名单 → 拆壳;`CONTENT` 里的 `url=` 语义不解析" },
    SanCase { name: "meta_refresh_data_b64", html: r#"<META HTTP-EQUIV="refresh" CONTENT="0;url=data:text/html base64,PHNjcmlwdD4=">"#, policy: "", expect: "",
        why: "同上;`data:` + base64 载荷无任何解码路径" },
    SanCase { name: "object_scriptlet", html: r#"<OBJECT TYPE="text/x-scriptlet" DATA="http://xss.rocks/scriptlet.html"></OBJECT>"#, policy: "", expect: "",
        why: "object 在整删集(危险嵌入)→ 连内容删除;此处无子内容" },
    SanCase { name: "object_classid_param", html: r#"<OBJECT classid=clsid:ae24fdae-03c6-11d1><param name=url value=javascript:alert('XSS')></OBJECT>"#, policy: "", expect: "",
        why: "object 整棵删除 → param 子节点跟着消失" },
    SanCase { name: "iframe_js", html: r#"<IFRAME SRC="javascript:alert('XSS');"></IFRAME>"#, policy: "", expect: "",
        why: "iframe 是 raw-text 元素 → 整棵删除(封掉其内文被再解析的 mXSS 面)" },
    // ── script / style 族:整删,且验证「删除不越界」──
    SanCase { name: "script_slash_src", html: r#"<SCRIPT/XSS SRC="http://xss.rocks/xss.js"></SCRIPT>"#, policy: "", expect: "",
        why: "`/XSS` 触发自闭合容错,script 仍整删;后随的 src 属性不产生输出" },
    SanCase { name: "script_query_lt", html: r#"<SCRIPT SRC=http://xss.rocks/xss.js?< B >"#, policy: "", expect: "",
        why: "无引号值遇到 `<` 提前结束标签(容错),script 整删到 `</script>`(此处不存在 → 吞到串尾)" },
    SanCase { name: "script_malformed", html: r#"<<SCRIPT>alert("XSS");//\<</SCRIPT>"#, policy: "", expect: "&lt;",
        why: "首个 `<` 后面不是字母 → 字面量;其后的 script 整删到配对 `</SCRIPT>`。**唯一幸存的是那个字面 `<`,转义成 `&lt;`**" },
    SanCase { name: "style_li_unwrap", html: r#"<STYLE>li {list-style-image: url("javascript:alert('XSS')");}</STYLE><UL><LI>XSS</br>"#, policy: "", expect: "<ul><li>XSS</li></ul>",
        why: "style 整删但**不越界**:其后的 ul/li 在白名单,文本 XSS 留下;`</br>` 按结束标签忽略" },
    // ── `<a href>` 族:白名单子集里唯一能观察到 scheme 判定的面 ──
    SanCase { name: "a_href_js_keeps_text", html: r#"<a href="javascript:alert(1)">click</a>"#, policy: "", expect: "<a>click</a>",
        why: "a 在白名单、href 在属性白名单,但 scheme 不在 URL 白名单 → **属性删除、元素与文本保留**(不整删元素)" },
    SanCase { name: "a_href_js_location", html: r#"<A HREF="javascript:document.location='http://x/';">XSS</A>"#, policy: "", expect: "<a>XSS</a>",
        why: "同上;大写标签 / 属性名小写化不影响判定" },
    SanCase { name: "a_href_tab_entity", html: r#"<a href="jav&#x09;ascript:alert(1)">click</a>"#, policy: "", expect: "<a>click</a>",
        why: "制表实体解码后落在 scheme 内 → 剥控制字符后仍是 javascript: → 拒" },
    SanCase { name: "a_href_newline_raw", html: "<a href=\"java\nscript:alert(1)\">click</a>", policy: "", expect: "<a>click</a>",
        why: "**裸换行**在引号属性值内:值跨行读完,scheme 形态非法(非字母数字)→ 拒。浏览器会剥掉换行,本成员更严" },
    SanCase { name: "a_href_leading_ctrl", html: r#"<a href="&#01;javascript:alert(1)">click</a>"#, policy: "", expect: "<a>click</a>",
        why: "值首控制字符 → 判定前剥掉首尾空白与控制字符 → javascript: → 拒" },
    SanCase { name: "a_href_data_rejected", html: r#"<a href="data:text/html,<script>alert(1)</script>">d</a>"#, policy: "", expect: "<a>d</a>",
        why: "引号内的 `<script>` 只是属性值字符,不构成标记;`data:` 不在 scheme 白名单 → 属性删除" },
    SanCase { name: "a_href_relative_ok", html: r#"<a href="/rel/path">click</a>"#, policy: "", expect: "<a href=\"/rel/path\">click</a>",
        why: "相对 URL **恒允许**(规范 §13.2);属性值恒双引号包裹后序列化" },
    SanCase { name: "a_href_relative_with_colon_ok", html: r#"<a href="/wiki/Special:Search">s</a>"#, policy: "", expect: "<a href=\"/wiki/Special:Search\">s</a>",
        why: "D13-023:维基式命名空间(路径段里带 `:`)也是相对 URL —— WHATWG URL §4.2 scheme 只可能在首个 `/` `?` `#` 之前结束。**修复前这个 href 被删**,方向保守但违背 §13.2「相对 URL 恒允许」" },
    SanCase { name: "a_href_mailto_ok", html: r#"<a href="mailto:a@b.c">mail</a>"#, policy: "", expect: "<a href=\"mailto:a@b.c\">mail</a>",
        why: "mailto 在缺省 scheme 白名单里" },
    SanCase { name: "a_title_kept_href_dropped", html: r#"<a href="javascript:1" title="hi">x</a>"#, policy: "", expect: "<a title=\"hi\">x</a>",
        why: "逐属性独立判定:href 拒、title 留 —— 不得因为一个属性被拒就丢掉整个元素" },
    SanCase { name: "a_dup_href_first_wins", html: r#"<a href="https://ok" href="javascript:1">x</a>"#, policy: "", expect: "<a href=\"https://ok\">x</a>",
        why: "同名属性**首个生效**,第二个不再参与判定(规范 §13.2)" },
    // ── 非 a 元素里的 style / 危险属性:拆壳后属性必须一并消失 ──
    SanCase { name: "table_background_js", html: r#"<TABLE BACKGROUND="javascript:alert('XSS')"><td>x</td></TABLE>"#, policy: "", expect: "x",
        why: "table / td 都不在白名单 → 逐层拆壳;**属性随元素消失,只留文本**" },
    SanCase { name: "div_style_bg_url", html: r#"<DIV STYLE="background-image: url(javascript:alert('XSS'))">x</DIV>"#, policy: "", expect: "x",
        why: "STYLE 不在任何标签的属性白名单里 → 删除;div 拆壳留 x" },
    SanCase { name: "div_style_expression", html: r#"<DIV STYLE="width: expression(alert('XSS'));">x</DIV>"#, policy: "", expect: "x",
        why: "IE 表达式向量;同样只靠「属性不在白名单」挡住" },
    // ── mXSS 教科书样本(Sonar cheatsheet,作用于白名单子集的部分)──
    SanCase { name: "mxss_svg_onload", html: r#"<svg/onload=alert(1)>"#, policy: "", expect: "",
        why: "svg 是外来内容 → **整棵删除**(规范 §13.2),不是拆壳" },
    SanCase { name: "mxss_svg_script", html: r#"<svg><script>alert(1)</script></svg>"#, policy: "", expect: "",
        why: "svg 整删带走了内层 script;嵌套两重删除不产生残留" },
    SanCase { name: "mxss_math_xlink", html: r#"<math><mi//xlink:href="data:x,<script>alert(1)</script>">"#, policy: "", expect: "",
        why: "命名空间混淆族:math 整删;`xlink:href` 是 URL 属性但轮不到判定" },
    SanCase { name: "mxss_noscript", html: r#"<noscript><p title="</noscript><img src=x onerror=alert(1)>">"#, policy: "", expect: "",
        why: "noscript 在整删集 → 整段消失(浏览器按「脚本已启用」与否会分叉,不复制该差分)" },
    SanCase { name: "mxss_style_comment", html: "<style><!--</style><img src=x onerror=alert(1)>--></style>", policy: "", expect: "--&gt;",
        why: "**教科书 mXSS**:浏览器把 `<!--` 当 CSS 注释、img 存活;本成员的 style 是 raw-text,扫到第一个 `</style>` 即止,img 落在其外 → 拆壳为空。幸存文本 `-->` 被转义,惰性" },
    SanCase { name: "mxss_form_button", html: r#"<form><button formaction=javascript:alert(1)>X</button></form>"#, policy: "", expect: "X",
        why: "formaction 是 URL 属性,但 form / button 都不在白名单 → 属性随元素消失,只留文本" },
    SanCase { name: "mxss_details_ontoggle", html: r#"<details open ontoggle=alert(1)>x</details>"#, policy: "", expect: "x",
        why: "事件属性随 details 拆壳消失" },
    SanCase { name: "mxss_marquee_onstart", html: r#"<marquee onstart=alert(1)>x</marquee>"#, policy: "", expect: "x",
        why: "同上" },
    SanCase { name: "mxss_video_onerror", html: r#"<video><source onerror=alert(1)></video>"#, policy: "", expect: "",
        why: "video / source 都不在白名单且无可见内容" },
    SanCase { name: "mxss_input_autofocus", html: r#"<input autofocus onfocus=alert(1)>"#, policy: "", expect: "",
        why: "input 是 void 且不在白名单 → 拆壳为空;「autofocus 触发」的面随元素消失" },
    SanCase { name: "mxss_template", html: r#"<template><script>alert(1)</script></template>"#, policy: "", expect: "",
        why: "template 在整删集(其内容惰性、会被别的解析器复活)" },
    SanCase { name: "mxss_xmp", html: r#"<xmp><script>alert(1)</script></xmp>"#, policy: "", expect: "",
        why: "xmp 是 raw-text 元素 → 内文不作为标记解析、整棵删除" },
    SanCase { name: "mxss_plaintext", html: r#"<plaintext><script>alert(1)</script>"#, policy: "", expect: "",
        why: "plaintext **按普通元素拆壳**(D13-008:规范 §13.2 未把它列入整删集)→ 内层 script 照常整删。拆壳不放宽内层判定" },
    SanCase { name: "mxss_unclosed_comment", html: r#"<b title="<!--"><script>alert(1)</script>">x</b>"#, policy: "", expect: "<b>&quot;&gt;x</b>",
        why: "**引号属性值吞掉后续标记**(`>` 不终止引号内的值)→ script 沦为属性值字符;title 不在白名单 → 属性删除,值里的 `\">` 作为文本转义输出" },
    // ── 畸形 / 容错口径 ──
    // ⚠ [D13-033 / N-015] `<` + **数字**不是标签开始。WHATWG *tag open state*
    //    要求后面跟 ASCII **字母**;跟数字时 `<` 是字面量。
    SanCase { name: "malformed_lone_lt_before_a_digit_keeps_the_text", html: r#"3<4 and 5<6"#, policy: "", expect: "3&lt;4 and 5&lt;6",
        why: "**串中段**的 `<4`。修复前分词器用 `is_ascii_alnum`(含数字)⇒ 造出名为 `4` 的标签、把 `and 5` 收成属性 → 不在白名单 → 整棵拆壳 → 输出 `3&lt;6`,`\"4 and 5\"` **静默蒸发**。⚠ 既有的 `malformed_lone_lt` 用 `1 < 2 and 3<4`,那个 `3<4` 在**串尾**,修复前是靠 `parse_start_tag` 走串尾返回 `None` 才落到字面量分支的 —— **恰好蒙对**,照不到这个形状" },
    // ⚠ [D13-034 / N-019] 属性上下文里 legacy 无分号实体的额外判据。
    //    WHATWG *character reference in attribute value state*:命中后若下一
    //    字符是 `=` 或 ASCII 字母数字则**不解码**(文本上下文里要解)。
    SanCase { name: "attribute_value_keeps_a_legacy_entity_before_equals", html: r#"<a href="/x?a=1&copy=2">y</a>"#, policy: "", expect: "<a href=\"/x?a=1&amp;copy=2\">y</a>",
        why: "`&copy` 是 legacy(无分号)实体,后随 `=` ⇒ 属性上下文**不解码**,`&` 留作字面量再由序列化侧转义成 `&amp;`。修复前被解成 `©=2`。⚠ 方向是**过度解码**,不构成 XSS,且「自幂等」与「输出无无分号实体」两条守卫都验不出来 —— 两次解码是自洽的" },
    SanCase { name: "text_context_still_decodes_the_same_legacy_entity", html: r#"&copy=1"#, policy: "", expect: "©=1",
        why: "**反向守卫**:同一条判据不得泄漏到文本上下文。HTML5 文本行为是 `&copy=1` → `©=1`;只钉「该拒的」而不钉「不该拒的照旧」,闸很容易写成一刀切" },
    // ⚠🔴 [D13-062 / N-020] 同名属性**首个生效**,而「首个生效」指的是
    //    **首个占住这个名字**,不是「首个被保留」。
    SanCase { name: "first_duplicate_attr_wins_even_when_it_is_rejected", html: r#"<a href="javascript:alert(1)" href="https://ok.example/">x</a>"#, policy: "", expect: "<a>x</a>",
        why: "首个 `href` 被 scheme 拒(规范 §13.2「URL 类属性过 scheme 白名单」),但它**仍占住** `href` 这个名字 ⇒ 第二个(合法的)`href` 也被跳过 ⇒ **一个 href 都不剩**。⚠ 这是本条**唯一可观察**的形状:若首个是因为「名字不在白名单」被拒,它��所有同名者本来也会被同一条白名单拒掉,登记顺序**无差别** —— 所以语料必须用 `href`(走 scheme 判定)才能区分两种实现。反向看:若把 `seen` 的登记挪到判据**之后**,第二个 href 会活下来 —— 那等于把「首个生效」悄悄读成「首个**被保留**」" },
    SanCase { name: "a_single_legal_url_attr_survives", html: r#"<a href="https://ok.example/">x</a>"#, policy: "", expect: "<a href=\"https://ok.example/\">x</a>",
        why: "**正向守卫**:上一条不能靠「过度收紧成永远丢掉 href」而变绿" },
    SanCase { name: "malformed_unclosed_b", html: r#"<b>unclosed"#, policy: "", expect: "<b>unclosed</b>",
        why: "未闭合元素在串尾补结束标签(**输出恒是良构的**,这是幂等的前提)" },
    SanCase { name: "malformed_lone_lt", html: r#"1 < 2 and 3<4"#, policy: "", expect: "1 &lt; 2 and 3&lt;4",
        why: "`<` 后不是字母 / `!` / `/` → 字面量;文本节点恒转义 `&` `<` `>`" },
    SanCase { name: "malformed_dup_attr", html: r#"<a href="x" href="y">z</a>"#, policy: "", expect: "<a href=\"x\">z</a>",
        why: "首个生效" },
    SanCase { name: "malformed_stray_end", html: r#"</b>orphan"#, policy: "", expect: "orphan",
        why: "栈内找不到对应开标签的结束标签 → 忽略(**不生成空元素**,规范 §13.2 已登记该差分)" },
    SanCase { name: "malformed_attr_lt", html: r#"<b a<b>x</b>"#, policy: "", expect: "<b><b>x</b></b>",
        why: "标签内出现 `<` → 标签在该处结束,`<b>` 成为嵌套的开标签;未知属性 a 随外层 b 删除" },
    SanCase { name: "malformed_eof_in_tag", html: r#"<b x="unterminated"#, policy: "", expect: "<b></b>",
        why: "串尾截断的引号值:值取到串尾,`x` 不在白名单 → 属性删除;元素良构补尾" },
    // ── 非缺省 policy:键**整体替换**缺省值,不是合并(规范 §13.2)──
    SanCase { name: "policy_custom_tags", html: r#"<b>x</b><i>y</i>"#, policy: r#"["tags": ["i"]]"#, expect: "x<i>y</i>",
        why: "tags 整体替换 → b 不再在白名单 → 拆壳留 x;i 保留。**不合并**是规范明写的行为" },
    SanCase { name: "policy_custom_attrs", html: r#"<a href="https://x" title="t" rel="r">z</a>"#, policy: r#"["attributes": ["a": ["rel"]]]"#, expect: "<a rel=\"r\">z</a>",
        why: "attributes 整体替换 → href / title 都删,只留 rel;元素仍保留(在 tags 里)" },
    SanCase { name: "policy_keep_comments", html: r#"<b>x</b><!--c-->"#, policy: r#"["strip_comments": FALSE]"#, expect: "<b>x</b><!--c-->",
        why: "strip_comments=FALSE → 注释保留;注释内容里的 `--` 在序列化时全部移除(防 `-->` 提前闭合)" },
    // ── [D13-025 / N-001] abrupt closing:`<!-->` / `<!--->` 是 WHATWG 的两个
    //    **合法空注释**,注释在此结束、余文回到 data state 继续按标记解析。
    //    ⚠ 这三条必须配 `strip_comments: FALSE` —— 缺省 TRUE 时注释整删,
    //    连 bug 的入口都没有。三条一起才构成完整链路:
    //      ① 分词:余文必须继续被解析成标记(修复前整段并成一条注释);
    //      ② 过滤:后面的 `img` 必须照常被白名单拒掉;
    //      ③ 序列化:空注释写成 `<!---->`,输出里不留 `<!-->` 字面形状。
    SanCase { name: "mxss_abrupt_closing_img_does_not_survive", html: r#"<b>x</b><!--><img src=x onerror=alert(1)>"#, policy: r#"["strip_comments": FALSE]"#, expect: "<b>x</b><!---->",
        why: "**教科书 mXSS 变体**:修复前 `<!--` 找不到 `-->` → 整个余文成为一条「未闭合注释」,`img` **从未进入过滤阶段**,序列化原样写回 `<!--`+数据+`-->` ⇒ 输出含 `<!-->` ⇒ 浏览器重解析读成空注释,img 落在注释外成为活标记。修复后注释在此结束、img 照常被拒" },
    SanCase { name: "mxss_abrupt_closing_three_dashes", html: r#"<b>x</b><!---><img src=x onerror=alert(1)>"#, policy: r#"["strip_comments": FALSE]"#, expect: "<b>x</b><!---->",
        why: "同上,`<!--->` 形态(WHATWG comment end state 收尾)" },
    SanCase { name: "abrupt_closing_resumes_markup_parsing", html: r#"<b>x</b><!-->y<script>alert(1)</script>"#, policy: r#"["strip_comments": FALSE]"#, expect: "<b>x</b><!---->y",
        why: "判别式:余文**继续按标记解析** —— 文本 `y` 幸存、script 整删。修复前 `y<script>…` 会整段成为注释数据而一并消失(夹具的输出会短一截)" },
    SanCase { name: "policy_add_scheme", html: r#"<a href="ftp://h/f">z</a>"#, policy: r#"["url_schemes": ["ftp"]]"#, expect: "<a href=\"ftp://h/f\">z</a>",
        why: "url_schemes 整体替换 → 只剩 ftp;http / https / mailto 此时会被拒(调用方自己收窄的)" },
    SanCase { name: "policy_omitted_keys_take_defaults", html: r#"<b>x</b><a href="https://k" title="t">y</a><!--c-->"#, policy: r#"["url_schemes": ["ftp", "https"]]"#, expect: "<b>x</b><a href=\"https://k\" title=\"t\">y</a>",
        why: "**省略的键取缺省值**:只改 url_schemes 不影响 tags / attributes / strip_comments(注释仍删)" },
    // ── 成员级诊断(E0030 / E0022,逐字冻结)──
    SanCase { name: "sanitize_non_string", html: "", policy: "", expect: "!E0030 HTML_SANITIZE: expected string, got integer",
        why: "data 位置类型错 —— 消息说 data 要 string(**这是 data 的错,不是 policy 的**)" },
    SanCase { name: "policy_non_dict", html: "", policy: r#"[1, 2]"#, expect: "!E0030 HTML_SANITIZE: policy must be a DICT, got array",
        why: "D13-009:原先复用 data 的类型错模板,报成「expected string, got array」—— 对着一个**合法的 string 参数**说「要 string」,指错参数。契约表当场抓到" },
    SanCase { name: "policy_tags_not_array", html: "", policy: r#"["tags": "b"]"#, expect: "!E0030 HTML_SANITIZE: policy \"tags\" must be an ARRAY",
        why: "键值类型错 → E0030 带**键名**(规范 §13.2:「policy 形态错 E0030(带名)」)" },
    SanCase { name: "policy_url_schemes_not_array", html: "", policy: r#"["url_schemes": "http"]"#, expect: "!E0030 HTML_SANITIZE: policy \"url_schemes\" must be an ARRAY",
        why: "同上" },
    // ── [D13-058 / N-006]开标签栈的**索引化**(按名字查「栈内最内层位置」)
    //    的语义面 —— 换算法不得改一棵树。⚠ 期望值全部按 WHATWG 的
    //    tree-construction 规则**独立裁定**,不是抄实现输出;⚠ 且**只挑白名单
    //    内的标签** —— 非白名单标签会被拆壳(结构在输出里看不见),那样的用例
    //    照不到「关错元素」。
    SanCase { name: "closure_unmatched_end_tag_is_ignored", html: r#"<b>x</b></i>y"#, policy: "", expect: "<b>x</b>y",
        why: "WHATWG §13.2.6 *in body* 的「any other end tag」:栈内没有同名开元素时**只报 parse error、不改树**;规范 §13.2 同款登记「不在栈内则忽略」。判别式:`</i>` 之后的 `y` 仍在 **根**下,没被挂进 `<b>`" },
    SanCase { name: "closure_li_implied", html: r#"<ul><li>a<li>b</ul>"#, policy: "", expect: "<ul><li>a</li><li>b</li></ul>",
        why: "WHATWG §13.2.6 *in body*:`li` 开始标签在 **list item scope** 内有 `li` 时先闭合它(隐式闭合路径);`</ul>` 再弹到 `ul`。⚠ 这条走的是**隐式闭合**那条弹栈路径,与下面的显式 `truncate` 不是同一条" },
    SanCase { name: "closure_p_implied_by_block_start", html: r#"<p>a<pre>b</pre>"#, policy: "", expect: "<p>a</p><pre>b</pre>",
        why: "同上,`p` 元素在新的块级开始标签(`pre`)出现时被闭合(HTML5 的 `p` 元素闭合规则)" },
    SanCase { name: "closure_p_implied_by_li", html: r#"<ul><li><p>a<li>b</ul>"#, policy: "", expect: "<ul><li><p>a</p></li><li>b</li></ul>",
        why: "`li` 关闭 `li` **与** `p`(规范 §13.2 登记的子集);连着两次弹栈才轮到新的 `li` 入栈 ⇒ 判别式是 `a` 那个 `p` 被**包在**第一个 `li` 里、没有与 `b` 并列" },
    SanCase { name: "closure_nested_matched_ends_keep_nesting", html: r#"<b><i>z</i></b>y"#, policy: "", expect: "<b><i>z</i></b>y",
        why: "配对结束标签:内层 `</i>` 只弹到 `i`,外层 `</b>` 弹到 `b` ⇒ `<i>` 必须留在 `<b>` 里(索引把 `b` 记成位置 0、`i` 记成位置 1,顺序错了这条就红)" },
    SanCase { name: "closure_after_full_close_the_stack_is_empty", html: r#"<b><i>z</i></b><em>w"#, policy: "", expect: "<b><i>z</i></b><em>w</em>",
        why: "⚠ 索引同步的端到端判别式:两次闭合后栈必须**真的空**——若截断时漏清索引,`<em>` 会被挂成 `<b>` 的子节点(输出成 `<b><i>z</i><em>w</em></b>`)。`<em>` 在白名单内,故结构可见" },
    // ── [D13-059 / N-007]raw-text 关闭扫描(不物化整段余文的滑窗版)的
    //    语义面。期望值按 WHATWG 的 RAWTEXT / script data 结束条件裁定。
    SanCase { name: "rawtext_close_tag_is_case_insensitive", html: r#"<b>a</b><script>x</SCRIPT>y"#, policy: "", expect: "<b>a</b>y",
        why: "WHATWG:HTML 标签名**大小写不敏感**,故 `</SCRIPT>` 就是 `script` 的结束标签;§13.2 登记 raw-text 内文不作标记解析、整棵删除 ⇒ 余下的 `y` 幸存。判别式:若关闭扫描变成大小写敏感,`x</SCRIPT>y` 会整段成为正文而被删,输出只剩 `<b>a</b>`" },
    SanCase { name: "rawtext_close_tag_may_carry_trailing_space", html: r#"<script>x</script >y"#, policy: "", expect: "y",
        why: "WHATWG:*script data end tag name* 之后允许空白再 `>`;本模块的关闭扫描找的是 `</name`(不含 `>`),余下那一段由「跳到 `>`」吃掉 ⇒ `y` 幸存" },
    SanCase { name: "rawtext_first_close_tag_wins", html: r#"<script>a</script>b</script>c"#, policy: "", expect: "bc",
        why: "raw-text 在**首个**同名结束标签处结束;回到 data state 后第二个 `</script>` 是一个**没有对应开元素**的结束标签 ⇒ 忽略。判别式:`b` 与 `c` 都必须幸存且并列" },
    SanCase { name: "rawtext_without_close_swallows_the_tail", html: r#"<b>a</b><script>tail"#, policy: "", expect: "<b>a</b>",
        why: "WHATWG:script data state 到 EOF 即结束(未闭合的 script 内容仍是 script 内容);规范 §13.2 的 raw-text 整删 ⇒ `tail` 连同元素一起消失,而 `<b>a</b>` 完好" },
    SanCase { name: "rawtext_title_and_textarea_are_dropped_with_content", html: r#"<title>t</title><textarea>u</textarea>v"#, policy: "", expect: "v",
        why: "规范 §13.2:整删集恰好是 8 个 raw-text 元素(「不多不少」);`title` / `textarea` 都在内 ⇒ 元素与内容一起删,`v` 幸存" },
];

/// 成员级诊断(与数据无关,单独一组:html 字段不参与)。
const SANITIZE_DIAGS: &[SanCase] = &[
    SanCase { name: "diag_arity_zero", html: "", policy: "", expect: "!E0022 HTML_SANITIZE: function expects 1 or 2 argument(s), got 0",
        why: "D13-010:原先用共享 arity 模板,对着**合法的单参调用**说「expects 2」——`policy` 可省,元数下界是 1" },
    SanCase { name: "diag_arity_three", html: "", policy: "", expect: "!E0022 HTML_SANITIZE: function expects 1 or 2 argument(s), got 3",
        why: "上界 2" },
];

/// 把任意输入安全地嵌进 wlwl 字符串字面量(§4.2 转义集:`\" \\ \n \r \t
/// \0 \b \f $`)。`$` 必须转 —— 否则 `${` 会被当成插值表达式。
fn lit(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '$' => out.push_str("\\$"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// 一次 `HTML_SANITIZE` 的源程序。`policy` 空串 = 缺省策略(整个第二实参省略)。
fn program(html: &str, policy: &str) -> String {
    let l = lit(html);
    let call = if policy.is_empty() {
        l
    } else {
        format!("{l}, {}", policy)
    };
    format!(r#"IMPORT("wlwl:std.sanitize", ["HTML_SANITIZE"]); HTML_SANITIZE({call})"#)
}

/// 幂等探针用的源程序:`SANITIZE(SANITIZE(x, p), p)`,一次 eval 拿到二次结果。
fn program_twice(html: &str, policy: &str) -> String {
    if policy.is_empty() {
        format!(
            r#"IMPORT("wlwl:std.sanitize", ["HTML_SANITIZE"]); HTML_SANITIZE(HTML_SANITIZE({l}))"#,
            l = lit(html)
        )
    } else {
        format!(
            r#"IMPORT("wlwl:std.sanitize", ["HTML_SANITIZE"]); HTML_SANITIZE(HTML_SANITIZE({l}, {p}), {p})"#,
            l = lit(html),
            p = policy
        )
    }
}

fn sanitize_of(html: &str, policy: &str) -> String {
    match run(&program(html, policy)) {
        Ok(v) => v.display(),
        Err(e) => e,
    }
}

fn sanitize_twice_of(html: &str, policy: &str) -> String {
    match run(&program_twice(html, policy)) {
        Ok(v) => v.display(),
        Err(e) => e,
    }
}

// ── W-13 ①:OWASP 向量 + 策略契约逐条对拍 ──────────────────────────

#[test]
fn sanitize_evidence_matches_the_frozen_contract() {
    let mut bad = Vec::new();
    for case in SANITIZE_CASES {
        let got = if case.name == "sanitize_non_string" {
            run(r#"IMPORT("wlwl:std.sanitize", ["HTML_SANITIZE"]); HTML_SANITIZE(1)"#)
                .map(|v| v.display())
                .unwrap_or_else(|e| e)
        } else {
            sanitize_of(case.html, case.policy)
        };
        if case.expect != got {
            bad.push(format!(
                "{name} (裁定依据:{why})\n      frozen: {want}\n      actual: {got}",
                name = case.name,
                why = case.why,
                want = case.expect,
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} / {} case(s) diverged from the frozen W-13 contract:\n  - {}",
        bad.len(),
        SANITIZE_CASES.len(),
        bad.join("\n  - ")
    );
    // 反向守卫:表不得空转,也不得被「顺手删几条」悄悄缩掉。逐组下限
    // 按实际条数取(留一点余量,但砍掉一半就红)。
    for (prefix, min) in [
        ("img_", 18),
        ("script_", 3),
        ("style_", 1),
        ("a_", 9),
        ("mxss_", 12),
        ("malformed_", 5),
        ("policy_", 7),
        // [D13-058 / N-006 / D13-059]树构建索引化 + raw-text 关闭扫描的语义面。
        ("closure_", 6),
        ("rawtext_", 5),
    ] {
        let n = SANITIZE_CASES
            .iter()
            .filter(|c| c.name.starts_with(prefix))
            .count();
        assert!(
            n >= min,
            "W-13 corpus lost its `{prefix}` group ({n} < {min})"
        );
    }
    assert!(
        SANITIZE_CASES.len() >= 75,
        "W-13 corpus shrank to {} — OWASP 向量被误删?",
        SANITIZE_CASES.len()
    );
}

#[test]
fn sanitize_diagnostics_are_frozen() {
    for case in SANITIZE_DIAGS {
        let src = match case.name {
            "diag_arity_zero" => {
                r#"IMPORT("wlwl:std.sanitize", ["HTML_SANITIZE"]); HTML_SANITIZE()"#
            }
            _ => {
                r#"IMPORT("wlwl:std.sanitize", ["HTML_SANITIZE"]); HTML_SANITIZE("x", ["tags": ["b"]], 3)"#
            }
        };
        let got = match run(src) {
            Ok(v) => v.display(),
            Err(e) => e,
        };
        assert_eq!(got, case.expect, "{}:{}", case.name, case.why);
    }
}

// ── W-12 ①:固定差分集 = 契约语料逐条幂等 ──────────────────────────

#[test]
fn sanitize_is_idempotent_on_the_contract_corpus() {
    let mut bad = Vec::new();
    for case in SANITIZE_CASES {
        if case.name == "sanitize_non_string" {
            continue; // 诊断用例,不是净化语料
        }
        let once = sanitize_of(case.html, case.policy);
        let twice = sanitize_twice_of(case.html, case.policy);
        if once != twice {
            bad.push(format!(
                "{name}\n      once:  {once}\n      twice: {twice}",
                name = case.name
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} / {} corpus case(s) broke the re-parse idempotency invariant \
         (§13.2 规范性安全不变量 —— 打破即缺陷,不是文档说明事项):\n  - {}",
        bad.len(),
        SANITIZE_CASES.len(),
        bad.join("\n  - ")
    );
}

// ── W-12 ②:变异集 ──────────────────────────────────────────────
//
// 变异器入库(计划 W-12「变异脚本入库」)。**只做不改语义的变形** ——
// 同一语义的不同字节序列,正是解析差分(mXSS)的入口。覆盖面:
//
//   1. case       整串 ASCII 大写           —— 标签名 / 属性名 / scheme 大小写不敏感
//   2. nl_split   `javascript:` 里插裸换行   —— 浏览器在 URL 中剥换行,须同样不判为 js:
//   3. tab_ent    `jav&#x09;ascript:`       —— 实体在**解析期**解码,不是输出期
//   4. cr_ent     `jav&#x0D;ascript:`
//   5. dec_ent    逐字符十进制实体
//   6. hex_ent    逐字符十六进制实体
//   7. ws_prefix  属性值前加空格 + 制表      —— scheme 判定前须剥控制字符
//   8. tag_split  `="` 之间插裸换行         —— 容错解析须跨行续读
//   9. quote_swap 双引号换单引号             —— 两种引号等价
//
// 每条变形后重跑幂等断言。**期望不是「变形后无害」而是「幂等」** ——
// 变形可能合法地改变首轮输出(例如大写后属性值不同),但二次必须不动。

fn mutation_variants(html: &str) -> Vec<(&'static str, String)> {
    let mut v: Vec<(&'static str, String)> = Vec::new();
    let upper = html.to_ascii_uppercase();
    if upper != html {
        v.push(("case", upper));
    }
    if html.contains("javascript:") {
        v.push(("nl_split", html.replacen("javascript:", "java\nscript:", 1)));
        v.push((
            "tab_ent",
            html.replacen("javascript:", "jav&#x09;ascript:", 1),
        ));
        v.push((
            "cr_ent",
            html.replacen("javascript:", "jav&#x0D;ascript:", 1),
        ));
        v.push((
            "dec_ent",
            html.replacen("javascript:", &dec_entities("javascript"), 1),
        ));
        v.push((
            "hex_ent",
            html.replacen("javascript:", &hex_entities("javascript"), 1),
        ));
    }
    if html.contains("=\"") {
        v.push(("ws_prefix", html.replacen("=\"", "=\"  \t", 1)));
        v.push(("tag_split", html.replacen("=\"", "=\n\"", 1)));
        v.push(("quote_swap", html.replace('"', "'")));
    }
    v
}

fn dec_entities(s: &str) -> String {
    s.chars().map(|c| format!("&#{};", c as u32)).collect()
}

fn hex_entities(s: &str) -> String {
    s.chars().map(|c| format!("&#x{:X};", c as u32)).collect()
}

#[test]
fn sanitize_is_idempotent_on_mutations() {
    let mut variants = 0usize;
    let mut by_kind: std::collections::BTreeMap<&'static str, usize> = Default::default();
    let mut bad = Vec::new();
    for case in SANITIZE_CASES {
        if case.name == "sanitize_non_string" {
            continue;
        }
        for (kind, mutated) in mutation_variants(case.html) {
            variants += 1;
            *by_kind.entry(kind).or_default() += 1;
            let once = sanitize_of(&mutated, case.policy);
            let twice = sanitize_twice_of(&mutated, case.policy);
            if once != twice {
                bad.push(format!(
                    "{name} / {kind}\n      mutated: {mutated}\n      once:    {once}\n      twice:   {twice}",
                    name = case.name
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} mutated variant(s) broke idempotency:\n  - {}",
        bad.len(),
        bad.join("\n  - ")
    );
    // 变异集不得退化:每类变形都必须在场(否则「全绿」是因为没跑变异)。
    for kind in [
        "case",
        "nl_split",
        "tab_ent",
        "cr_ent",
        "dec_ent",
        "hex_ent",
        "ws_prefix",
        "tag_split",
        "quote_swap",
    ] {
        assert!(
            by_kind.get(kind).copied().unwrap_or(0) > 0,
            "mutation `{kind}` produced no variant — the suite would pass vacuously"
        );
    }
    assert!(
        variants >= 150,
        "mutation set shrank to {variants} variants (by kind: {by_kind:?})"
    );
    println!("W-12 mutation coverage: {variants} variants, by kind: {by_kind:?}");
}

// ── W-12 ③:mXSS 教科书样本(独立成组,便于审计「覆盖了哪一族」)────

#[test]
fn sanitize_is_idempotent_on_mxss_textbook_samples() {
    let m = SANITIZE_CASES
        .iter()
        .filter(|c| c.name.starts_with("mxss_"))
        .collect::<Vec<_>>();
    assert!(
        m.len() >= 12,
        "mXSS group shrank to {} — Sonar cheatsheet 的白名单子集部分被误删?",
        m.len()
    );
    let mut bad = Vec::new();
    for case in m {
        let once = sanitize_of(case.html, case.policy);
        let twice = sanitize_twice_of(case.html, case.policy);
        if once != twice {
            bad.push(format!(
                "{}\n      once: {once}\n      twice: {twice}",
                case.name
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "mXSS sample(s) broke idempotency:\n  - {}",
        bad.join("\n  - ")
    );
}

// ── W-11 病态输入契约(安全面,与功能同权)────────────────────────
//
// 「深嵌套不栈溢出」是**断言**不是基准:基准只出数字,不会让 CI 变红。
// 四段流水线(tokenize / build / filter / serialize)必须全迭代 —— 任一段
// 偷偷改成递归,这里就是栈溢出而不是绿。

const PATHOLOGICAL_DEPTH: usize = 100_000;

#[test]
fn sanitize_survives_pathological_nesting() {
    let deep = format!(
        "{}x{}",
        "<b>".repeat(PATHOLOGICAL_DEPTH),
        "</b>".repeat(PATHOLOGICAL_DEPTH)
    );
    let out = sanitize_of(&deep, "");
    // `<b>`×N + `x` + `</b>`×N = 3N + 1 + 4N
    assert_eq!(
        out.len(),
        7 * PATHOLOGICAL_DEPTH + 1,
        "{} 层嵌套的输出长度不对 —— 树构建或序列化丢节点了",
        PATHOLOGICAL_DEPTH
    );
    assert!(out.starts_with(&"<b>".repeat(PATHOLOGICAL_DEPTH)));
    assert!(out.ends_with(&"</b>".repeat(PATHOLOGICAL_DEPTH)));
    assert_eq!(out.matches('x').count(), 1, "最内层文本 `x` 丢了或重复了");
    // 未闭合的病态嵌套同样不得溢出(开标签栈一路涨,无结束标签)。
    let unclosed = "<b>".repeat(PATHOLOGICAL_DEPTH);
    let out = sanitize_of(&unclosed, "");
    // 注意:比**长度 + 首尾**,不比内容 —— `<b>×N</b>×N` 与 `<b></b>×N`
    // 等长但不同形,拿 assert_eq! 比字符串会在这里假红。
    assert_eq!(
        out.len(),
        7 * PATHOLOGICAL_DEPTH,
        "未闭合 {} 层:节点数不对",
        PATHOLOGICAL_DEPTH
    );
    assert!(out.starts_with(&"<b>".repeat(PATHOLOGICAL_DEPTH)));
    assert!(out.ends_with(&"</b>".repeat(PATHOLOGICAL_DEPTH)));
}

#[test]
fn sanitize_survives_pathological_text_and_attributes() {
    // 超长纯文本:单趟扫描,不得超线性。
    let text = "a".repeat(1_000_000);
    assert_eq!(sanitize_of(&text, ""), text);
    // 超多属性:属性数不设人为上限,但内存 O(n)。
    let many = format!(
        "<a {}>x</a>",
        (0..20_000)
            .map(|i| format!("k{i}=v"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    assert_eq!(sanitize_of(&many, ""), "<a>x</a>");
    // 实体风暴:全表二分不得退化成线性扫描。
    let ents = "&copy;".repeat(100_000);
    assert_eq!(sanitize_of(&ents, ""), "©".repeat(100_000));
}

/// [D13-058 / N-006 + D13-059 / N-007]审查报告点名的两条病态形状,从
/// `HTML_SANITIZE` 入口**整链**跑一遍。
///
/// ⚠ 与 `wlwl-std` 单测里的两条「计量型」契约分工:那边数扫描次数(精确、
/// 与机器无关),这边只断言**输出正确** + 规模是审查报告点名的量级 ——
/// 平方级 / n·k 级实现在这里不是「慢一点」,而是分钟级跑不完,CI 自己会红。
///
/// ⚠ 规模比单测里的小一档:这里每条都要过一次完整的解释器求值(解析 +
/// 过滤 + 序列化 + 字符串字面量转义),1 MB 级输入按这个开销算不划算。
#[test]
fn sanitize_stays_linear_on_unmatched_ends_and_raw_text_storms() {
    // ① `"<b>"×N + "</i>"×N` —— N-006:5 万个 `</i>` 每个都扫完整个栈。
    const N: usize = 20_000;
    let unmatched_ends = format!("{}x{}", "<b>".repeat(N), "</i>".repeat(N));
    let out = sanitize_of(&unmatched_ends, "");
    // `</i>` 全部忽略(§13.2「不在栈内则忽略」)⇒ 仍是 N 层 `<b>` 包着 `x`,
    // 序列化时补齐 N 个结束标签:`<b>`×N + `x` + `</b>`×N = 7N + 1。
    assert_eq!(
        out.len(),
        7 * N + 1,
        "未匹配结束标签风暴的输出长度不对(建树可能漏弹或多弹)"
    );
    assert!(out.starts_with(&"<b>".repeat(N)));
    assert!(out.ends_with(&"</b>".repeat(N)));
    assert_eq!(out.matches('x').count(), 1, "文本 `x` 丢了或重复了");
    assert!(
        !out.contains("i"),
        "被忽略的 `</i>` 不该留下痕迹(实际输出:{}…)",
        &out[..out.len().min(40)]
    );

    // ② 密集 raw-text —— N-007:每个元素都把**整个余文**小写化一份。
    const K: usize = 5_000;
    let dense_rawtext = format!("{}y", "<script></script>".repeat(K));
    assert_eq!(
        sanitize_of(&dense_rawtext, ""),
        "y",
        "K 个空 raw-text 应当整删,只余末尾的 `y`"
    );
    // 关闭标签大小写混写时同样整删(滑窗比的是 `eq_ignore_ascii_case`)。
    let mixed_case = format!("{}y", "<sCrIpT></ScRiPt>".repeat(K));
    assert_eq!(
        sanitize_of(&mixed_case, ""),
        "y",
        "raw-text 关闭扫描对大小写不敏感"
    );
}
