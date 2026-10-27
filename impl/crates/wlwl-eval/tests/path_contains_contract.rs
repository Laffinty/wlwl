//! [W-24 / v0.12.1] `std.text` 归一化三成员 + `std.fs` 路径包含判断 —— 契约测试。
//!
//! 与 `text_contract.rs` / `fs_contract.rs` 同款结构(表驱动 + 一条聚合断言)。
//!
//! ## 一、期望值从哪来
//!
//! **不是从实现输出抄的** —— 那等于把实现钉成「永远正确」。三类来源:
//!
//! 1. **Unicode 标准本身**:`NFKC` 的全角 / 连字 / 上标 / 数学字母折叠、
//!    `CASE_FOLD` 的 `ß`→`ss` / `ς`→`σ` / `ſ`→`s`、`STRIP_INVISIBLE`
//!    的零宽与双向控制字符集合 —— 都是 UAX #15 / CaseFolding /
//!    DerivedGeneralCategory 的明文事实。
//! 2. **词法路径语义**:「逐段比较」的判据(`D:/root-evil` 不是 `D:/root`
//!    的子路径)是**定义**,不是实现细节 —— 字符串前缀比较必然判错。
//! 3. **既有成员的边界**:`PATH_NORMALIZE` / `TO_LOWER` 的口径被本文件
//!    钉住(W-24 **不许**动它们)⇒ 用例同时是「别顺手改了它」的守卫。
//!
//! ## 二、三条**判据**用例(plan §W-24 的验收行)
//!
//! - `PATH_CONTAINS("D:/root", "D:/root-evil/x")` ⇒ `FALSE`
//! - `PATH_CONTAINS("D:/root", "D:/root/../root2/x")` ⇒ `FALSE`
//! - `PATH_CONTAINS("D:/root", "D:/root/a/b")` ⇒ `TRUE`
//!
//! ⚠ 第一条是**反向守卫**的载体:把实现改回**字符串前缀比较**,它必红。

mod common;

use std::path::PathBuf;
use wlwl_eval::Evaluator;

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const CASES: &[Case] = &[
    // ══ PATH_CONTAINS · 判据三条 ═════════════════════════════════════
    Case {
        name: "path_contains_true_for_subpath",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:/root", "D:/root/a/b")"#,
        expect: "TRUE",
    },
    Case {
        name: "path_contains_false_for_sibling_prefix",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:/root", "D:/root-evil/x")"#,
        expect: "FALSE",
    },
    Case {
        name: "path_contains_false_after_dotdot_escape",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:/root", "D:/root/../root2/x")"#,
        expect: "FALSE",
    },
    // ══ PATH_CONTAINS · 逐段比较(不是字符串前缀)═════════════════════
    // ⚠ 同前缀的兄弟目录:字符串前缀比较会判 TRUE ⇒ 这几条是反向守卫。
    Case {
        name: "path_contains_false_for_root_evil_exact",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:/root", "D:/root-evil")"#,
        expect: "FALSE",
    },
    Case {
        name: "path_contains_false_for_rootx",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:/root", "D:/rootx")"#,
        expect: "FALSE",
    },
    // ⚠ 段**边界**:目标的第一段必须是根的第一段**逐字**相同。
    Case {
        name: "path_contains_false_for_different_first_segment",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:/root", "D:/other/root")"#,
        expect: "FALSE",
    },
    // 根自身算「在根之下」。
    Case {
        name: "path_contains_true_for_root_itself",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:/root", "D:/root")"#,
        expect: "TRUE",
    },
    Case {
        name: "path_contains_true_for_root_with_trailing_slash",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:/root/", "D:/root/x")"#,
        expect: "TRUE",
    },
    // ⚠ 目标比根**短** ⇒ 不是子路径。
    Case {
        name: "path_contains_false_when_target_shorter",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:/root/a", "D:/root")"#,
        expect: "FALSE",
    },
    // ══ PATH_CONTAINS · 分隔符与盘符 ═════════════════════════════════
    // `\` 与 `/` **都**当分隔符(跨平台一致的判定需要这个)。
    Case {
        name: "path_contains_true_for_backslash_target",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:/root", "D:\\root\\a\\b")"#,
        expect: "TRUE",
    },
    Case {
        name: "path_contains_true_for_backslash_root",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:\\root", "D:/root/a")"#,
        expect: "TRUE",
    },
    // ⚠ 盘符**大小写不敏感**。
    Case {
        name: "path_contains_true_for_lowercase_drive_root",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("d:/root", "D:/root/a")"#,
        expect: "TRUE",
    },
    Case {
        name: "path_contains_true_for_lowercase_drive_target",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:/root", "d:/root/a")"#,
        expect: "TRUE",
    },
    // ⚠ **盘符不同** ⇒ 不是同一条根。
    Case {
        name: "path_contains_false_for_different_drive",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:/root", "E:/root/a")"#,
        expect: "FALSE",
    },
    // ⚠ 其余段**大小写敏感**(语言本身大小写敏感)。
    Case {
        name: "path_contains_false_for_case_different_segment",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:/Root", "D:/root/a")"#,
        expect: "FALSE",
    },
    // ══ PATH_CONTAINS · 边界 ═════════════════════════════════════════
    // 空根路径不含任何路径。
    Case {
        name: "path_contains_false_for_empty_root",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("", "x")"#,
        expect: "FALSE",
    },
    // 绝对 / 相对必须一致。
    Case {
        name: "path_contains_false_for_relative_target_under_absolute_root",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:/root", "root/a")"#,
        expect: "FALSE",
    },
    Case {
        name: "path_contains_false_for_absolute_target_under_relative_root",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("root", "/root/a")"#,
        expect: "FALSE",
    },
    // 相对路径之间的比较是允许的。
    Case {
        name: "path_contains_true_for_relative_pair",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("root", "root/a/b")"#,
        expect: "TRUE",
    },
    // POSIX 风格绝对路径。
    Case {
        name: "path_contains_true_for_posix_absolute",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("/var/www", "/var/www/html")"#,
        expect: "TRUE",
    },
    Case {
        name: "path_contains_false_for_posix_sibling_prefix",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("/var/www", "/var/www2/x")"#,
        expect: "FALSE",
    },
    // ⚠ **不访问文件系统**:两条路径都不存在,判定照样有答案。
    Case {
        name: "path_contains_works_for_nonexistent_paths",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("D:/no/such/dir", "D:/no/such/dir/a")"#,
        expect: "TRUE",
    },
    // ══ PATH_CANONICAL ═══════════════════════════════════════════════
    Case {
        name: "path_canonical_unifies_separators",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CANONICAL"]); PATH_CANONICAL("D:\\root\\a")"#,
        expect: "D:/root/a",
    },
    Case {
        name: "path_canonical_drops_dot_segment",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CANONICAL"]); PATH_CANONICAL("D:/root/./a")"#,
        expect: "D:/root/a",
    },
    Case {
        name: "path_canonical_resolves_dotdot",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CANONICAL"]); PATH_CANONICAL("D:/root/b/../a")"#,
        expect: "D:/root/a",
    },
    Case {
        name: "path_canonical_resolves_dotdot_escape",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CANONICAL"]); PATH_CANONICAL("D:/root/../root2/x")"#,
        expect: "D:/root2/x",
    },
    Case {
        name: "path_canonical_collapses_duplicate_separators",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CANONICAL"]); PATH_CANONICAL("D://root///a")"#,
        expect: "D:/root/a",
    },
    // ⚠ 盘符**保留原样**(大小写不敏感由 `PATH_CONTAINS` 在比较时处理)。
    Case {
        name: "path_canonical_keeps_drive_case",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CANONICAL"]); PATH_CANONICAL("d:/Root/A")"#,
        expect: "d:/Root/A",
    },
    // ⚠ **既有成员不动**:`PATH_NORMALIZE` 只认 `/`(`\` 是合法文件名字符)。
    Case {
        name: "path_normalize_still_ignores_backslash",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_NORMALIZE"]); PATH_NORMALIZE("D:/root\\a")"#,
        expect: "D:/root\\a",
    },
    // ══ NFKC · 兼容性字符 ═════════════════════════════════════════════
    // ⚠ **判据用例**:全角 `ｓｅｌｅｃｔ` 折回 ASCII。
    Case {
        name: "nfkc_folds_fullwidth_select",
        src: r#"IMPORT("wlwl:std.text", ["NFKC"]); NFKC("ｓｅｌｅｃｔ")"#,
        expect: "select",
    },
    Case {
        name: "nfkc_folds_fullwidth_digits",
        src: r#"IMPORT("wlwl:std.text", ["NFKC"]); NFKC("０１２３")"#,
        expect: "0123",
    },
    Case {
        name: "nfkc_expands_ligature_fi",
        src: r#"IMPORT("wlwl:std.text", ["NFKC"]); NFKC("ﬁ")"#,
        expect: "fi",
    },
    Case {
        name: "nfkc_folds_superscript_two",
        src: r#"IMPORT("wlwl:std.text", ["NFKC"]); NFKC("x²")"#,
        expect: "x2",
    },
    Case {
        name: "nfkc_folds_mathematical_letters",
        src: r#"IMPORT("wlwl:std.text", ["NFKC"]); NFKC("𝐬𝐞𝐥𝐞𝐜𝐭")"#,
        expect: "select",
    },
    Case {
        name: "nfkc_folds_circled_one",
        src: r#"IMPORT("wlwl:std.text", ["NFKC"]); NFKC("①")"#,
        expect: "1",
    },
    Case {
        name: "nfkc_folds_nbsp_to_space",
        src: "IMPORT(\"wlwl:std.text\", [\"NFKC\"]); NFKC(\"a\u{a0}b\")",
        expect: "a b",
    },
    // ⚠ **组合记号不被丢弃**(它们带语义:重音)—— `NFKC` 会把它**组合**成
    //   预组合形(U+0065 U+0301 ⇒ U+00E9),不是把它删掉。
    Case {
        name: "nfkc_composes_decomposed_accent",
        src: "IMPORT(\"wlwl:std.text\", [\"NFKC\"]); NFKC(\"e\u{301}\")",
        expect: "\u{e9}",
    },
    // ⚠ 单独的组合记号**原样保留**(不与任何基字符组合)。
    Case {
        name: "nfkc_keeps_lone_combining_accent",
        src: "IMPORT(\"wlwl:std.text\", [\"NFKC\"]); NFKC(\"\u{301}\")",
        expect: "\u{301}",
    },
    // ⚠ **零宽字符不动**(那是 `STRIP_INVISIBLE` 的活)⇒ 长度不变。
    Case {
        name: "nfkc_keeps_zero_width_space",
        src: "IMPORT(\"wlwl:std.text\", [\"NFKC\"]); LEN(NFKC(\"a\u{200b}b\"))",
        expect: "3",
    },
    Case {
        name: "nfkc_leaves_plain_ascii_alone",
        src: r#"IMPORT("wlwl:std.text", ["NFKC"]); NFKC("select 123 !@#")"#,
        expect: "select 123 !@#",
    },
    Case {
        name: "nfkc_is_idempotent",
        src: r#"IMPORT("wlwl:std.text", ["NFKC"]); =(NFKC(NFKC("ｓｅｌｅｃｔ")), NFKC("ｓｅｌｅｃｔ"))"#,
        expect: "TRUE",
    },
    Case {
        name: "nfkc_empty",
        src: r#"IMPORT("wlwl:std.text", ["NFKC"]); NFKC("")"#,
        expect: "",
    },
    // ⚠ **既有成员不动**:`NFC` 不做兼容性分解(全角 `s` 仍仍是 `ｓ`)。
    Case {
        name: "nfc_does_not_do_compatibility_decomposition",
        src: r#"IMPORT("wlwl:std.text", ["NFC"]); NFC("ｓ")"#,
        expect: "ｓ",
    },
    // ══ CASE_FOLD ════════════════════════════════════════════════════
    Case {
        name: "case_fold_expands_sharp_s",
        src: r#"IMPORT("wlwl:std.text", ["CASE_FOLD"]); CASE_FOLD("ß")"#,
        expect: "ss",
    },
    Case {
        name: "case_fold_expands_capital_sharp_s",
        src: r#"IMPORT("wlwl:std.text", ["CASE_FOLD"]); CASE_FOLD("ẞ")"#,
        expect: "ss",
    },
    Case {
        name: "case_fold_maps_long_s",
        src: r#"IMPORT("wlwl:std.text", ["CASE_FOLD"]); CASE_FOLD("ſ")"#,
        expect: "s",
    },
    // ⚠ **尾 sigma 一律折成 `σ`**(折叠没有上下文)。
    Case {
        name: "case_fold_maps_final_sigma",
        src: r#"IMPORT("wlwl:std.text", ["CASE_FOLD"]); CASE_FOLD("ς")"#,
        expect: "σ",
    },
    Case {
        name: "case_fold_maps_capital_sigma",
        src: r#"IMPORT("wlwl:std.text", ["CASE_FOLD"]); CASE_FOLD("Σ")"#,
        expect: "σ",
    },
    Case {
        name: "case_fold_expands_ligature",
        src: r#"IMPORT("wlwl:std.text", ["CASE_FOLD"]); CASE_FOLD("ﬁ")"#,
        expect: "fi",
    },
    Case {
        name: "case_fold_expands_dotted_capital_i",
        src: "IMPORT(\"wlwl:std.text\", [\"CASE_FOLD\"]); CASE_FOLD(\"\u{130}\")",
        expect: "i\u{307}",
    },
    Case {
        name: "case_fold_lowercases_ascii",
        src: r#"IMPORT("wlwl:std.text", ["CASE_FOLD"]); CASE_FOLD("SELECT")"#,
        expect: "select",
    },
    // ⚠ **切罗基语大写字母**:折叠映射**存在**,但映射**到自己** ⇒ 折叠**不**
    //   把它折到小写(U+13A0 的小写是 U+AB70,折叠是 U+13A0 自己)。
    //   ⚠ 这条是「表里登记了 identity 折叠」的钉子 —— 少登记它,实现就会
    //   退回小写映射,给出 `ꭰ`。
    Case {
        name: "case_fold_keeps_cherokee_uppercase",
        src: "IMPORT(\"wlwl:std.text\", [\"CASE_FOLD\"]); CASE_FOLD(\"\u{13a0}\")",
        expect: "\u{13a0}",
    },
    Case {
        name: "case_fold_maps_cherokee_small_to_upper",
        src: "IMPORT(\"wlwl:std.text\", [\"CASE_FOLD\"]); CASE_FOLD(\"\u{ab70}\")",
        expect: "\u{13a0}",
    },
    Case {
        name: "case_fold_leaves_cjk_alone",
        src: r#"IMPORT("wlwl:std.text", ["CASE_FOLD"]); CASE_FOLD("中文 123")"#,
        expect: "中文 123",
    },
    Case {
        name: "case_fold_empty",
        src: r#"IMPORT("wlwl:std.text", ["CASE_FOLD"]); CASE_FOLD("")"#,
        expect: "",
    },
    // ⚠ **既有成员不动**:`TO_LOWER` 是显示用的小写映射(尾 sigma 看上下文)。
    Case {
        name: "to_lower_still_context_dependent_for_final_sigma",
        src: r#"IMPORT("wlwl:std.text", ["TO_LOWER"]); TO_LOWER("ΑΣ")"#,
        expect: "ας",
    },
    // ══ STRIP_INVISIBLE ═══════════════════════════════════════════════
    Case {
        name: "strip_invisible_removes_zero_width_space",
        src:
            "IMPORT(\"wlwl:std.text\", [\"STRIP_INVISIBLE\"]); STRIP_INVISIBLE(\"sel\u{200b}ect\")",
        expect: "select",
    },
    Case {
        name: "strip_invisible_removes_zero_width_joiner",
        src: "IMPORT(\"wlwl:std.text\", [\"STRIP_INVISIBLE\"]); STRIP_INVISIBLE(\"a\u{200d}b\")",
        expect: "ab",
    },
    Case {
        name: "strip_invisible_removes_bom",
        src:
            "IMPORT(\"wlwl:std.text\", [\"STRIP_INVISIBLE\"]); STRIP_INVISIBLE(\"\u{feff}select\")",
        expect: "select",
    },
    // ⚠ **双向控制符**(Trojan Source 那族)也剥掉。
    Case {
        name: "strip_invisible_removes_bidi_override",
        src:
            "IMPORT(\"wlwl:std.text\", [\"STRIP_INVISIBLE\"]); STRIP_INVISIBLE(\"sel\u{202e}ect\")",
        expect: "select",
    },
    Case {
        name: "strip_invisible_removes_rtl_embedding",
        src: "IMPORT(\"wlwl:std.text\", [\"STRIP_INVISIBLE\"]); STRIP_INVISIBLE(\"a\u{202b}b\")",
        expect: "ab",
    },
    // ⚠ **组合记号不动**(剥掉会把 `é` 变成 `e`、毁掉内容)。
    Case {
        name: "strip_invisible_keeps_combining_marks",
        src: "IMPORT(\"wlwl:std.text\", [\"STRIP_INVISIBLE\"]); STRIP_INVISIBLE(\"e\u{301}\")",
        expect: "e\u{301}",
    },
    Case {
        name: "strip_invisible_leaves_plain_text_alone",
        src: r#"IMPORT("wlwl:std.text", ["STRIP_INVISIBLE"]); STRIP_INVISIBLE("select 123 !@#")"#,
        expect: "select 123 !@#",
    },
    Case {
        name: "strip_invisible_empty",
        src: r#"IMPORT("wlwl:std.text", ["STRIP_INVISIBLE"]); STRIP_INVISIBLE("")"#,
        expect: "",
    },
    // ══ 失败口径 ═════════════════════════════════════════════════════
    // 元数 / 类型错 ⇒ 原生诊断(E0022 / E0030),**不是** `ERR` 值。
    Case {
        name: "nfkc_non_string_is_e0030",
        src: r#"IMPORT("wlwl:std.text", ["NFKC"]); NFKC(5)"#,
        expect: "!E0030 NFKC: expected string, got integer",
    },
    Case {
        name: "nfkc_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.text", ["NFKC"]); NFKC("a", "b")"#,
        expect: "!E0022 NFKC: function expects 1 argument(s), got 2",
    },
    Case {
        name: "case_fold_non_string_is_e0030",
        src: r#"IMPORT("wlwl:std.text", ["CASE_FOLD"]); CASE_FOLD(5)"#,
        expect: "!E0030 CASE_FOLD: expected string, got integer",
    },
    Case {
        name: "case_fold_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.text", ["CASE_FOLD"]); CASE_FOLD("a", "b")"#,
        expect: "!E0022 CASE_FOLD: function expects 1 argument(s), got 2",
    },
    Case {
        name: "strip_invisible_non_string_is_e0030",
        src: r#"IMPORT("wlwl:std.text", ["STRIP_INVISIBLE"]); STRIP_INVISIBLE(5)"#,
        expect: "!E0030 STRIP_INVISIBLE: expected string, got integer",
    },
    Case {
        name: "strip_invisible_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.text", ["STRIP_INVISIBLE"]); STRIP_INVISIBLE("a", "b")"#,
        expect: "!E0022 STRIP_INVISIBLE: function expects 1 argument(s), got 2",
    },
    Case {
        name: "path_contains_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("a")"#,
        expect: "!E0022 PATH_CONTAINS: function expects 2 argument(s), got 1",
    },
    Case {
        name: "path_contains_non_string_is_e0030",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CONTAINS"]); PATH_CONTAINS("a", 5)"#,
        expect: "!E0030 PATH_CONTAINS: expected a string, got integer",
    },
    Case {
        name: "path_canonical_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CANONICAL"]); PATH_CANONICAL("a", "b")"#,
        expect: "!E0022 PATH_CANONICAL: function expects 1 argument(s), got 2",
    },
    Case {
        name: "path_canonical_non_string_is_e0030",
        src: r#"IMPORT("wlwl:std.fs", ["PATH_CANONICAL"]); PATH_CANONICAL(5)"#,
        expect: "!E0030 PATH_CANONICAL: expected a string, got integer",
    },
];

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_path_contains")
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
fn path_contains_and_normalization_contract() {
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
        "W-24 contract drift:\n    {}",
        bad.join("\n    ")
    );
}
