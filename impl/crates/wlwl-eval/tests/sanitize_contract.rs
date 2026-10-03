//! `wlwl:std.sanitize` 成员契约表 —— 标准库规范 §13(v0.11.3 M1)。
//!
//! 沿三份既有契约(collection / str_math / test)的形态:
//!
//! 1. **规范表对拍**(四件套第 3 条):从规范 §13 的成员表**解析**出成员面,
//!    与 `resolve("wlwl:std.sanitize")` 的实现清单对拍 —— 两侧不同源,
//!    规范被改坏而实现没动时这里变红;
//! 2. **冻结用例**:诊断码 + 消息逐字 + 结果 `display()`;
//! 3. **反向守卫**:成员集非空、点名成员在列、`SANITIZE_HTML` 在 M1 阶段
//!    **不得**出现在实现里(它随 M3 落地 —— 这条断言同时证明集合比较
//!    不是空转:任何一边多出名字都会红)。
//!
//! SQL 转义的**否决记录**(规范 §13.3)也被本文件钉住:删掉那段规范的
//! 尝试会在这里变红 —— 「不做」与「做」同权,都是契约。

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
        .join("stdlib")
        .join("wlwl-stdlib-spec-v0.11.md");
    std::fs::read_to_string(&p).expect("stdlib spec is readable")
}

fn section(section_no: u32, next_no: u32) -> String {
    let text = spec_text();
    let start = text
        .find(&format!("## {section_no} "))
        .unwrap_or_else(|| panic!("§{section_no} header exists"));
    let end = text[start..]
        .find(&format!("\n## {next_no} "))
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
    let spec = members_of(&section(13, 14));
    assert_eq!(
        spec,
        vec!["HTML_ESCAPE", "HTML_UNESCAPE", "HTML_SANITIZE"],
        "§13 member table drifted"
    );
    // M1 只落地转义两成员;M3 已落地 `HTML_SANITIZE` —— 按 M1 时写在
    // 本处的交接说明,impl 断言改为三员全等(集合等值,顺序同 §13 表)。
    let impls = impl_members();
    assert_eq!(
        impls,
        vec!["HTML_ESCAPE", "HTML_UNESCAPE", "HTML_SANITIZE"],
        "M3 surface: all three §13 members landed"
    );
}

/// 反向守卫:集合比较不得是空转 —— 任何一边多出 / 少掉 / 改名都会红。
#[test]
fn sanitize_member_set_is_not_trivially_satisfied() {
    let impls = impl_members();
    assert!(!impls.is_empty(), "impl surface must not be empty");
    for n in &impls {
        assert!(
            ["HTML_ESCAPE", "HTML_UNESCAPE", "HTML_SANITIZE"].contains(&n.as_str()),
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

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_sanitize_{nanos}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}
