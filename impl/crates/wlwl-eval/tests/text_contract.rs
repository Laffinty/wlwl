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
    // ── NFC:规范组合(期望值取自 CPython `unicodedata.normalize`)────
    Case {
        name: "nfc_composes_ascii_plus_acute",
        src: r#"IMPORT("wlwl:std.text", ["NFC"]); NFC("Á")"#,
        expect: "Á",
    },
    Case {
        name: "nfc_shortens_string",
        src: r#"IMPORT("wlwl:std.text", ["NFC"]); LEN(NFC("Á"))"#,
        expect: "1",
    },
    Case {
        name: "nfc_respects_blocked_combination",
        src: r#"IMPORT("wlwl:std.text", ["NFC"]); NFC("Ḍ̇")"#,
        expect: "Ḍ̇",
    },
    Case {
        name: "nfc_composes_hangul_jamo",
        src: r#"IMPORT("wlwl:std.text", ["NFC"]); NFC("각")"#,
        expect: "각",
    },
    Case {
        name: "nfc_is_idempotent",
        src: r#"IMPORT("wlwl:std.text", ["NFC"]); =(NFC(NFC("Á")), NFC("Á"))"#,
        expect: "TRUE",
    },
    Case {
        name: "nfc_leaves_cjk_and_ascii_alone",
        src: r#"IMPORT("wlwl:std.text", ["NFC"]); NFC("中文 123 !@#")"#,
        expect: "中文 123 !@#",
    },
    Case {
        name: "nfc_of_lone_combining_acute_is_unchanged",
        src: r#"IMPORT("wlwl:std.text", ["NFC"]); NFC("́")"#,
        expect: "́",
    },
    Case {
        name: "nfc_empty",
        src: r#"IMPORT("wlwl:std.text", ["NFC"]); NFC("")"#,
        expect: "",
    },
    // ── NFD:规范分解(同一份权威期望值)───────────────────────────
    Case {
        name: "nfd_expands_precomposed_accent",
        src: r#"IMPORT("wlwl:std.text", ["NFD"]); NFD("Á")"#,
        expect: "Á",
    },
    Case {
        name: "nfd_lengthens_string",
        src: r#"IMPORT("wlwl:std.text", ["NFD"]); LEN(NFD("Á"))"#,
        expect: "2",
    },
    Case {
        name: "nfd_decomposes_hangul_syllable",
        src: r#"IMPORT("wlwl:std.text", ["NFD"]); NFD("각")"#,
        expect: "각",
    },
    Case {
        name: "nfd_is_idempotent",
        src: r#"IMPORT("wlwl:std.text", ["NFD"]); =(NFD(NFD("Á")), NFD("Á"))"#,
        expect: "TRUE",
    },
    Case {
        name: "nfd_leaves_cjk_and_ascii_alone",
        src: r#"IMPORT("wlwl:std.text", ["NFD"]); NFD("中文 123 !@#")"#,
        expect: "中文 123 !@#",
    },
    // ── NFC_QC:单方向语义(§12.2)──────────────────────────────────
    Case {
        name: "nfc_qc_true_for_plain_ascii",
        src: r#"IMPORT("wlwl:std.text", ["NFC_QC"]); NFC_QC("hello")"#,
        expect: "TRUE",
    },
    Case {
        name: "nfc_qc_true_for_empty",
        src: r#"IMPORT("wlwl:std.text", ["NFC_QC"]); NFC_QC("")"#,
        expect: "TRUE",
    },
    Case {
        name: "nfc_qc_true_for_string_already_in_nfc",
        src: r#"IMPORT("wlwl:std.text", ["NFC_QC"]); NFC_QC("Á")"#,
        expect: "TRUE",
    },
    Case {
        name: "nfc_qc_false_for_decomposable_pair",
        src: r#"IMPORT("wlwl:std.text", ["NFC_QC"]); NFC_QC("Á")"#,
        expect: "FALSE",
    },
    // ⚠ **保守方向的钉子**:单独一个 U+0301 经 NFC 之后**原样不动**,
    // 但快检仍说「不保证」。少了这条,「FALSE 即不是 NFC」那个假
    // 事实就不会有人去纠正(见规范 §12.2 与 `nfc_quick_ok` 文档)。
    Case {
        name: "nfc_qc_is_conservative_not_an_equivalence",
        src: r#"IMPORT("wlwl:std.text", ["NFC", "NFC_QC"]); &&(=(NFC("́"), NFC("́")), NOT(NFC_QC("́")))"#,
        expect: "TRUE",
    },
    // ── 规范化三成员的失败形态 ──────────────────────────────────────
    Case {
        name: "nfc_non_string_is_e0030",
        src: r#"IMPORT("wlwl:std.text", ["NFC"]); NFC(5)"#,
        expect: "!E0030 NFC: expected string, got integer",
    },
    Case {
        name: "nfc_qc_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.text", ["NFC_QC"]); NFC_QC("a", "b")"#,
        expect: "!E0022 NFC_QC: function expects 1 argument(s), got 2",
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
        5,
        "§12 table extractor found {} member(s): {spec:?} — the table's shape changed \
         and the extractor needs updating",
        spec.len()
    );
    for n in ["TO_UPPER", "TO_LOWER", "NFC", "NFD", "NFC_QC"] {
        assert!(
            spec.iter().any(|m| m == n),
            "§12 table missed `{n}`: {spec:?}"
        );
        assert!(
            impls.iter().any(|m| m == n),
            "implementation does not export `{n}`: {impls:?}"
        );
    }
    assert_eq!(impls.len(), 5, "wlwl:std.text exports 5 members");
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

/// UCD 依赖的成员分两批落地,两批的理由**都**必须留在规范里。
///
/// - `NFC` / `NFD` / `NFC_QC`:v0.11.3 `addendum-03` W-01/W-02/W-03 已落地;
/// - `GRAPHEME_COUNT` / `WIDTH`:**仍推迟**(W-04),数据源与规范化无关。
///
/// 「合成一块做」这个裁决丢了的话,下一个人会挑最容易的那个先做,生成器
/// 于是要写第二遍。所以连「已落地」的那半边也要留痕 —— 不是只记推迟,
/// 也要记**为什么先做了这三、为什么后做那两**。
#[test]
fn the_ucd_batch_rationale_is_documented() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    for needle in [
        "UCD",
        "GraphemeBreakProperty",
        "CompositionExclusions",
        "EastAsianWidth",
        "半张表",
        // v0.11.3 新增:「正确性只由全量自检证明」这条纪律的落点。
        "18.0.0",
        "norm-check",
    ] {
        assert!(
            text.contains(needle),
            "stdlib spec §12 lost `{needle}` — either the batching was revisited on \
             purpose (update this test and §12.1/§12.2 together) or the rationale was trimmed"
        );
    }
}

/// `NFC_QC` 的**单方向**语义是规范条款(§12.2),不是实现细节。
///
/// 少了这条,文面随时会滑回「返回 `FALSE` 即表示不是 NFC 形态」—— 而那是
/// 假的:官方测试文件里有 202 行的串本来就是 NFC 形态,快检照样返回 `FALSE`。
#[test]
fn nfc_qc_one_directional_wording_is_documented() {
    let text = std::fs::read_to_string(spec_path()).expect("stdlib spec readable");
    for needle in ["不保证", "反向不成立", "NFC_QC"] {
        assert!(
            text.contains(needle),
            "stdlib spec §12.2 lost `{needle}` — the one-directional wording of \
             NFC_QC is a normative clause, not commentary"
        );
    }
}
