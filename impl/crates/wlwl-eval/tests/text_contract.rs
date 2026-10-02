//! `wlwl:std.text` 成员契约 —— 标准库规范 §12(v0.11.2 M2)。
//!
//! 与 `encode_contract.rs` 同款结构。两条额外的自我约束:
//!
//! 1. **正例的期望值来自 Unicode 的简单大小写规则,不是实现输出。**
//!    实现走 Rust `char` 的内置查表;契约测试要能发现「那张表换了版本」
//!    或者「有人写成了逐字符 `to_ascii_uppercase`」—— 后者对 ASCII
//!    的期望值全对、对 `ß` 就错,所以**必须**有一条非 ASCII 的用例。
//! 2. **每条正例都同时钉住长度。** `ß` → `SS` 让字符串变长;只断言
//!    内容不��断长度的话,「逐字符映射」实现(把 ß 留在原地)会同时
//!    骗过内容断言的某些写法。`LEN` 那一列就是给这件事准备的。
//!
//! 反过来还钉一条**本模块不做**的事:全局 `UPPER` / `LOWER` 仍然只做
//! ASCII(§12 明写「两个并存,不替换」)。少了这条,下一个人看到
//! `UPPER` 的缺陷就可能直接改全局内建 —— 那是 breaking。

use std::path::PathBuf;
use wlwl_eval::Evaluator;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const CASES: &[Case] = &[
    // ── 非 ASCII 真的动了(本模块存在的理由)──────────────────────────
    Case {
        name: "to_upper_latin1_supplement",
        src: r#"IMPORT("wlwl:std.text", ["TO_UPPER"]); TO_UPPER("héllo")"#,
        expect: "HÉLLO",
    },
    Case {
        name: "to_lower_latin1_supplement",
        src: r#"IMPORT("wlwl:std.text", ["TO_LOWER"]); TO_LOWER("ÉÀÜ")"#,
        expect: "éàü",
    },
    Case {
        name: "to_upper_latin_extended_a",
        src: r#"IMPORT("wlwl:std.text", ["TO_UPPER"]); TO_UPPER("āáǎà")"#,
        expect: "ĀÁǍÀ",
    },
    Case {
        name: "to_upper_cyrillic",
        src: r#"IMPORT("wlwl:std.text", ["TO_UPPER"]); TO_UPPER("привет")"#,
        expect: "ПРИВЕТ",
    },
    Case {
        name: "to_lower_cyrillic",
        src: r#"IMPORT("wlwl:std.text", ["TO_LOWER"]); TO_LOWER("ПРИВЕТ")"#,
        expect: "привет",
    },
    Case {
        name: "to_upper_greek",
        src: r#"IMPORT("wlwl:std.text", ["TO_UPPER"]); TO_UPPER("αβγ")"#,
        expect: "ΑΒΓ",
    },
    // ── 长度会变的情形(§12 表里明写)───────────────────────────────
    Case {
        name: "to_upper_sharp_s_expands",
        src: r#"IMPORT("wlwl:std.text", ["TO_UPPER"]); TO_UPPER("straße")"#,
        expect: "STRASSE",
    },
    Case {
        name: "to_upper_sharp_s_expands_length",
        src: r#"IMPORT("wlwl:std.text", ["TO_UPPER"]); LEN(TO_UPPER("straße"))"#,
        expect: "7",
    },
    Case {
        name: "to_lower_dotted_capital_i_length",
        src: r#"IMPORT("wlwl:std.text", ["TO_LOWER"]); LEN(TO_LOWER("İ"))"#,
        expect: "2",
    },
    // ── 上下文相关:希腊 final sigma ────────────────────────────────
    Case {
        name: "to_lower_greek_final_sigma_word_final",
        src: r#"IMPORT("wlwl:std.text", ["TO_LOWER"]); TO_LOWER("ΑΣ")"#,
        expect: "ας",
    },
    Case {
        name: "to_lower_greek_sigma_medial",
        src: r#"IMPORT("wlwl:std.text", ["TO_LOWER"]); TO_LOWER("ΣΑ")"#,
        expect: "σα",
    },
    // ── ASCII 路径没被弄坏 ──────────────────────────────────────────
    Case {
        name: "to_upper_ascii",
        src: r#"IMPORT("wlwl:std.text", ["TO_UPPER"]); TO_UPPER("Hello, World 123")"#,
        expect: "HELLO, WORLD 123",
    },
    Case {
        name: "to_lower_ascii",
        src: r#"IMPORT("wlwl:std.text", ["TO_LOWER"]); TO_LOWER("Hello")"#,
        expect: "hello",
    },
    Case {
        name: "to_upper_empty",
        src: r#"IMPORT("wlwl:std.text", ["TO_UPPER"]); TO_UPPER("")"#,
        expect: "",
    },
    Case {
        name: "to_lower_empty",
        src: r#"IMPORT("wlwl:std.text", ["TO_LOWER"]); TO_LOWER("")"#,
        expect: "",
    },
    // ── 无 case 映射的字符原样穿过 ──────────────────────────────────
    Case {
        name: "to_upper_leaves_cjk_alone",
        src: r#"IMPORT("wlwl:std.text", ["TO_UPPER"]); TO_UPPER("中文 123 !@#")"#,
        expect: "中文 123 !@#",
    },
    // ── 程序员错误走原生诊断(§12 失败列)───────────────────────────
    Case {
        name: "to_upper_non_string_is_e0030",
        src: r#"IMPORT("wlwl:std.text", ["TO_UPPER"]); TO_UPPER(5)"#,
        expect: "!E0030 TO_UPPER: expected string, got integer",
    },
    Case {
        name: "to_lower_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.text", ["TO_LOWER"]); TO_LOWER("a", "b")"#,
        expect: "!E0022 TO_LOWER: function expects 1 argument(s), got 2",
    },
    // ── §12 明写:两个并存,全局 UPPER / LOWER **不动** ──────────────
    Case {
        name: "global_upper_is_still_ascii_only",
        src: r#"UPPER("straße")"#,
        expect: "STRAßE",
    },
    Case {
        name: "global_lower_is_still_ascii_only",
        src: r#"LOWER("ÉÀÜ")"#,
        expect: "ÉÀÜ",
    },
];

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_text_{nanos}"));
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

#[test]
fn text_contract() {
    let bad: Vec<String> = CASES
        .iter()
        .filter_map(|c| {
            let got = actual(c.src);
            if got == c.expect {
                None
            } else {
                Some(format!(
                    "{name}:\n      expected: {want}\n      actual:   {got}",
                    name = c.name,
                    want = c.expect
                ))
            }
        })
        .collect();
    assert!(
        bad.is_empty(),
        "wlwl:std.text contract drift:\n    {}",
        bad.join("\n    ")
    );
}

/// 规范 §12 的成员表 ↔ `SPEC.functions`。
#[test]
fn text_member_set_matches_the_spec_table() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    let start = text.find("## 12 ").expect("§12 header exists");
    let end = text[start..]
        .find("\n## 13 ")
        .map(|i| start + i)
        .expect("§13 header follows §12");
    let body = &text[start..end];
    // 只扫**规范性成员表**:§12.1 是推迟裁决的理由表,里面有 `NFC` /
    // `GRAPHEME_COUNT` / `WIDTH` 这些行,但它们**不是成员**。遇到第一个
    // `### ` 子标题就停 —— 表格的权威性止于它自己的小节。
    let body = match body.find("\n### ") {
        Some(i) => &body[..i],
        None => body,
    };

    let mut spec: Vec<String> = Vec::new();
    for line in body.lines() {
        let Some(rest) = line.strip_prefix("| `") else {
            continue;
        };
        // 同 `encode_contract`:名字在反引号或左括号中取较早的那个。
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

    let impls: Vec<String> = wlwl_std::resolve("wlwl:std.text")
        .expect("wlwl:std.text resolves")
        .functions()
        .iter()
        .map(|(n, _)| (*n).to_string())
        .collect();

    assert_eq!(
        spec.len(),
        2,
        "§12 table extractor found {} member(s): {spec:?} — the table's shape changed \
         and the extractor needs updating",
        spec.len()
    );
    for n in ["TO_UPPER", "TO_LOWER"] {
        assert!(
            spec.iter().any(|m| m == n),
            "§12 table missed `{n}`: {spec:?}"
        );
        assert!(
            impls.iter().any(|m| m == n),
            "implementation does not export `{n}`: {impls:?}"
        );
    }
    assert_eq!(impls.len(), 2, "wlwl:std.text exports 2 members");
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

/// §12.1 的推迟裁决必须留在规范里。
///
/// 四个 UCD 依赖的成员(`NFC` / `NFD` / `GRAPHEME_COUNT` / `WIDTH`)是
/// **一个**决定,不是一个成员一个决定。裁决丢了的话,下一个人会挑最容易
/// 的那个先做,生成器于是要写第二遍。
#[test]
fn the_ucd_deferral_rationale_is_documented() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    for needle in [
        "UCD",
        "GraphemeBreakProperty",
        "CompositionExclusions",
        "EastAsianWidth",
        "半张表",
    ] {
        assert!(
            text.contains(needle),
            "stdlib spec §12.1 lost `{needle}` — either the deferral was revisited on \
             purpose (update this test and §12.1 together) or the rationale was trimmed"
        );
    }
}
