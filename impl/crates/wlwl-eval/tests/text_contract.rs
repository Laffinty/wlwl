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

mod common;

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
    }, // ── GRAPHEME_COUNT(坐标系是**字素簇**,不是码点)──────────────────────
    Case {
        name: "grapheme_empty_is_zero",
        src: r#"IMPORT("wlwl:std.text", ["GRAPHEME_COUNT"]); GRAPHEME_COUNT("")"#,
        expect: "0",
    },
    Case {
        name: "grapheme_ascii_one_per_char",
        src: r#"IMPORT("wlwl:std.text", ["GRAPHEME_COUNT"]); GRAPHEME_COUNT("abc")"#,
        expect: "3",
    },
    Case {
        name: "grapheme_cjk_one_each",
        src: r#"IMPORT("wlwl:std.text", ["GRAPHEME_COUNT"]); GRAPHEME_COUNT("中文")"#,
        expect: "2",
    },
    Case {
        name: "grapheme_family_emoji_is_one",
        src: r#"IMPORT("wlwl:std.text", ["GRAPHEME_COUNT"]); GRAPHEME_COUNT("👨‍👩‍👧")"#,
        expect: "1",
    },
    Case {
        name: "grapheme_combining_mark_joins_its_base",
        src: r#"IMPORT("wlwl:std.text", ["GRAPHEME_COUNT"]); GRAPHEME_COUNT("é")"#,
        expect: "1",
    },
    Case {
        name: "grapheme_flag_pair_is_one",
        src: r#"IMPORT("wlwl:std.text", ["GRAPHEME_COUNT"]); GRAPHEME_COUNT("🇨🇳")"#,
        expect: "1",
    },
    Case {
        name: "grapheme_non_string_is_e0030",
        src: r#"IMPORT("wlwl:std.text", ["GRAPHEME_COUNT"]); GRAPHEME_COUNT(5)"#,
        expect: "!E0030 GRAPHEME_COUNT: expected string, got integer",
    },
    Case {
        name: "grapheme_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.text", ["GRAPHEME_COUNT"]); GRAPHEME_COUNT("a", "b")"#,
        expect: "!E0022 GRAPHEME_COUNT: function expects 1 argument(s), got 2",
    },
    // ── WIDTH(三条口径 + 簇折叠)──────────────────────────────────────
    Case {
        name: "width_empty_is_zero",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH("")"#,
        expect: "0",
    },
    Case {
        name: "width_ascii_one_each",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH("abc")"#,
        expect: "3",
    },
    Case {
        name: "width_cjk_is_two_each",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH("中文")"#,
        expect: "4",
    },
    // D-W1:`A` 类按 1。U+00AE REGISTERED SIGN / U+2122 TRADE MARK 的 EAW 是 `A`。
    Case {
        name: "width_ambiguous_is_one_dw1",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH("®")"#,
        expect: "1",
    },
    Case {
        name: "width_ambiguous_trademark_is_one_dw1",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH("™")"#,
        expect: "1",
    },
    // D-W3:零宽。U+0301 是 Mn,U+200B / U+200D 是 Cf。
    Case {
        name: "width_combining_acute_is_zero_dw3",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH("́")"#,
        expect: "0",
    },
    Case {
        name: "width_zero_width_space_is_zero_dw3",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH("")"#,
        expect: "0",
    },
    Case {
        name: "width_zero_width_joiner_is_zero_dw3",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH("‍")"#,
        expect: "0",
    },
    // D-W3 的例外:`Mc` 带间距,不置 0。天城文 visarga = U+0903。
    Case {
        name: "width_spacing_mark_mc_is_not_zero_dw3",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH("ः")"#,
        expect: "1",
    },
    // D-W2:默认 emoji 呈现的 `N` 抬到 2。U+1F1E8 是区域指示符。
    Case {
        name: "width_regional_indicator_is_two_dw2",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH("🇨")"#,
        expect: "2",
    },
    // D-W4:簇折叠。三个 emoji + 两个 ZWJ 逐码点是 8,但终端渲染 2 列。
    Case {
        name: "width_zwj_family_is_two_columns_dw4",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH("👨‍👩‍👧")"#,
        expect: "2",
    },
    Case {
        name: "width_flag_is_two_columns_not_four",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH("🇨🇳")"#,
        expect: "2",
    },
    Case {
        name: "width_base_plus_combining_is_one_not_two",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH("é")"#,
        expect: "1",
    },
    Case {
        name: "width_single_emoji_is_two",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH("😀")"#,
        expect: "2",
    },
    Case {
        name: "width_non_string_is_e0030",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH(5)"#,
        expect: "!E0030 WIDTH: expected string, got integer",
    },
    Case {
        name: "width_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.text", ["WIDTH"]); WIDTH("a", "b")"#,
        expect: "!E0022 WIDTH: function expects 1 argument(s), got 2",
    },
    // ── TO_INT(规范 v1.0 §12.4 的**宽松**口径)────────────────────────
    //
    // ⚠ **与全局内建 `INT` 故意不同**(§12.4 的对比表):`INT` 严格(一律不裁
    //   空白 / 只收十进制 / 小数形态拒);`TO_INT` 宽松(裁首尾空白 / 另收
    //   `0x`·`0X` 十六进制文本 / 小数向零截断)⇒ **不要**把它们「统一」。
    // ⚠ 失败是 **`ERR` 值**(不是诊断)⇒ 程序**最后一条**若是那个 `ERR` 值,
    //   它会**逃逸** ⇒ E0102 ⇒ 失败用例必须先 `LET` 绑住,再用 `IS_ERR` /
    //   `ERR_PAYLOAD` / `AT_K` 观察(那条 `ERR_PAYLOAD` 是**取走型**消费者
    //   ⇒ 顺带把未消费状态取走 ⇒ 作用域结束不报 E0103)。
    // 正例:§12.4「接受的形态」三条放宽 + 符号 + 范围两端。
    Case {
        name: "to_int_plain_decimal",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); TO_INT("42")"#,
        expect: "42",
    },
    Case {
        name: "to_int_trims_the_ends_only",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); TO_INT("  7  ")"#,
        expect: "7",
    },
    Case {
        name: "to_int_truncates_decimal_toward_zero",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); TO_INT("3.9")"#,
        expect: "3",
    },
    Case {
        name: "to_int_truncates_negative_decimal_toward_zero",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); TO_INT("-3.9")"#,
        expect: "-3",
    },
    Case {
        name: "to_int_accepts_hex_text",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); TO_INT("0x1F")"#,
        expect: "31",
    },
    Case {
        name: "to_int_accepts_hex_text_case_insensitively",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); TO_INT("0X1f")"#,
        expect: "31",
    },
    Case {
        name: "to_int_accepts_one_leading_sign",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); [TO_INT("-5"), TO_INT("+5")]"#,
        expect: "[-5, 5]",
    },
    // §12.4:结果必须落在 `INTEGER` 的有符号 64 位区间内 ⇒ `i64::MIN`
    // **必须**解析成功(它的幅度是 `i64::MAX + 1` ⇒ 只能靠「先取幅度再带
    // 符号」这条路;按幅度直接 `parse::<i64>()` 会失败)。
    Case {
        name: "to_int_parses_i64_min",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); TO_INT("-9223372036854775808")"#,
        expect: "-9223372036854775808",
    },
    Case {
        name: "to_int_parses_i64_max",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); TO_INT("9223372036854775807")"#,
        expect: "9223372036854775807",
    },
    // ── 失败形态:一律 `ERR([kind:"ParseError", input, reason])` **值** ──
    Case {
        name: "to_int_rejects_empty_string",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); LET(r, TO_INT("")); IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); AT_K(p, "kind", "?")), "NOT-ERR")"#,
        expect: "ParseError",
    },
    Case {
        name: "to_int_rejects_whitespace_only_string",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); LET(r, TO_INT("   ")); IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); AT_K(p, "kind", "?")), "NOT-ERR")"#,
        expect: "ParseError",
    },
    Case {
        name: "to_int_rejects_sign_only_string",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); LET(r, TO_INT("-")); IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); AT_K(p, "kind", "?")), "NOT-ERR")"#,
        expect: "ParseError",
    },
    Case {
        name: "to_int_rejects_non_numeric_text",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); LET(r, TO_INT("abc")); IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); AT_K(p, "kind", "?")), "NOT-ERR")"#,
        expect: "ParseError",
    },
    // ⚠ §12.4:「符号之后**必须紧跟**数字」⇒ 符号与数字之间的空白**不**
    //   是「可裁剪的首尾空白」(裁剪只作用于首尾)⇒ `TO_INT("- 42")` 失败。
    Case {
        name: "to_int_rejects_sign_followed_by_space",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); LET(r, TO_INT("- 42")); IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); AT_K(p, "kind", "?")), "NOT-ERR")"#,
        expect: "ParseError",
    },
    // ⚠ §12.4:「含非数字字符的串 ⇒ 解析失败」⇒ 小数**尾巴**上的非数字字符
    //   不能被静默丢掉。
    Case {
        name: "to_int_rejects_non_digit_tail",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); LET(r, TO_INT("3.9abc")); IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); AT_K(p, "kind", "?")), "NOT-ERR")"#,
        expect: "ParseError",
    },
    Case {
        name: "to_int_rejects_second_decimal_point",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); LET(r, TO_INT("1.2.3")); IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); AT_K(p, "kind", "?")), "NOT-ERR")"#,
        expect: "ParseError",
    },
    // ⚠ §12.4:「裁剪**只**作用于首尾,串内部的空白使解析失败」。
    Case {
        name: "to_int_rejects_internal_whitespace",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); LET(r, TO_INT("1 2")); IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); AT_K(p, "kind", "?")), "NOT-ERR")"#,
        expect: "ParseError",
    },
    // ⚠ §12.4:越界 ⇒ 解析失败(`i64::MAX + 1` 是**正号**下的越界)。
    Case {
        name: "to_int_rejects_value_above_i64_max",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); LET(r, TO_INT("9223372036854775808")); IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); AT_K(p, "kind", "?")), "NOT-ERR")"#,
        expect: "ParseError",
    },
    // ⚠ 下面三条是**规范沉默处**的裁决(§12.4 只说「另接受 0x/0X 十六进制
    //   文本」⇒ 接受的形态 = 十进制 + 十六进制,别的都不接受):
    //   * 下划线分隔符(`1_000`)不是任何一种形态 ⇒ 失败;
    //   * 二进制前缀(`0b101`)只认 `0x` / `0X` ⇒ 失败;
    //   * 前导小数点(`.5`)没有整数部分可截断 ⇒ 失败。
    Case {
        name: "to_int_rejects_underscore_separators",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); LET(r, TO_INT("1_000")); IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); AT_K(p, "kind", "?")), "NOT-ERR")"#,
        expect: "ParseError",
    },
    Case {
        name: "to_int_rejects_binary_prefix",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); LET(r, TO_INT("0b101")); IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); AT_K(p, "kind", "?")), "NOT-ERR")"#,
        expect: "ParseError",
    },
    Case {
        name: "to_int_rejects_leading_decimal_point",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); LET(r, TO_INT(".5")); IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); AT_K(p, "kind", "?")), "NOT-ERR")"#,
        expect: "ParseError",
    },
    // ── 载荷:`input` 是**原始**实参(裁剪前)—— §12.4 的载荷口径 ──────
    Case {
        name: "to_int_payload_input_is_the_untrimmed_original",
        src: r#"IMPORT("wlwl:std.text", ["TO_INT"]); LET(r, TO_INT("  x  ")); IF(IS_ERR(r), (LET(p, ERR_PAYLOAD(r)); AT_K(p, "input", "?")), "NOT-ERR")"#,
        expect: "  x  ",
    },
];

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_text")
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
    // ⚠ 结束锚点**写死 §13** ⇒ 规范**新增**中间章时这段正文会**多取一章** ⇒
    //   成员表对拍失败 ⇒ 变红(「设计变了就一起改测试」那道闸)⇒ 必须用
    //   `chapter_body_until`(结束于**指定**的下一个章标题),不能换成
    //   `chapter_body`(结束于下一个 `## `)—— 那会静默截断,把那道闸拆掉。
    let body = common::chapter_body_until(&text, "## 标准库规范 · 12 ", "\n## 标准库规范 · 13 ");
    // 只扫**规范性成员表**:§12.1 是推迟裁决的理由表,里面有 `NFC` /
    // `GRAPHEME_COUNT` / `WIDTH` 这些行,但它们**不是成员**。遇到第一个
    // `### ` 子标题就停 —— 表格的权威性止于它自己的小节。
    let body: &str = match body.find("\n### ") {
        Some(i) => &body[..i],
        None => &body,
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
        11,
        "§12 table extractor found {} member(s): {spec:?} — the table's shape changed \
         and the extractor needs updating",
        spec.len()
    );
    for n in [
        "TO_UPPER",
        "TO_LOWER",
        "NFC",
        "NFD",
        "NFC_QC",
        "GRAPHEME_COUNT",
        "WIDTH",
        // [规范 v1.0.2 新增成员] `TO_INT` 是 §12 的**规范性**成员(§12.4),
        // 不是项目扩展 —— 少了这一条,「表里加了成员却没实现」这类漂移不会被
        // 这道闸拦住。
        "TO_INT",
        // [v0.12.1 / W-24] 匹配前归一化三员(D-2)。
        "NFKC",
        "CASE_FOLD",
        "STRIP_INVISIBLE",
    ] {
        assert!(
            spec.iter().any(|m| m == n),
            "§12 table missed `{n}`: {spec:?}"
        );
        assert!(
            impls.iter().any(|m| m == n),
            "implementation does not export `{n}`: {impls:?}"
        );
    }
    // ⚠ **对着规范表比,不写死数字**。
    //   写死 `8` 会在「规范侧还没跟上」时给出**误导性的消息**(断言文本说
    //   「exports 8 members」而实现侧其实是 11)⇒ 把「诚实」换成「误导」。
    //   ⇒ 这一条**红就是那笔规范侧欠账的红**(§11.2 步骤 ①–③),不是实现缺陷;
    //   规范侧落表后它会自己转绿。
    assert_eq!(
        impls.len(),
        spec.len(),
        "wlwl:std.text exports {} members but the §12 table lists {} ⇒ the extra ones are \
         pending 规范侧 登记",
        impls.len(),
        spec.len()
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
    // ⚠ 判据**收窄到 §12**。落在整个文件上时,别处的同名文字会让这条一直绿 ——
    //   修之前实测把 §12.1 的「数据覆盖必须完整」整段删掉,本条**仍然绿**。
    let body = common::chapter_body_until(&text, "## 标准库规范 · 12 ", "\n## 标准库规范 · 13 ");
    for needle in [
        "UCD",
        "GraphemeBreakProperty",
        "CompositionExclusions",
        "EastAsianWidth",
        // ⚠ v1.0 把 v0.11 的「**明确不做半张表**」改写成「**数据覆盖必须完整**」
        //   —— 「半张表」这个措辞在 v1.0 合并册里**只剩 std.json §3.5 的同义
        //   比喻**(「半张表比没有更糟」),拿它当 needle 等于判「std.json 里得有
        //   这句话」⇒ std.text 的覆盖纪律被删光也照样绿。改钉 v1.0 的等价表述。
        "数据覆盖必须完整",
        // v0.11.3 新增:「正确性只由全量自检证明」这条纪律的落点。
        "18.0.0",
        "norm-check",
    ] {
        assert!(
            body.contains(needle),
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
    // ⚠ 判据**收窄到 §12**。落在整册上时,别处的同名文字会让这条一直绿 ——
    //   修之前实测把 §12.2 里「**必定**已是 NFC 形态」改成「**必定**是某形态」,
    //   整册判据**仍然绿**:旧针脚只查 `不保证` / `反向不成立` / `NFC_QC`,
    //   而这三样**都没被动到**(`NFC_QC` 整册出现 8 次,§12 内就有 7 次)
    //   ⇒ 旧判据压根**没在查**「正向定义」那一句。
    // ⚠ 结束锚点是**下一个 `## `**(通用口径):判据的范围就是「§12 这一章」。
    let body = common::chapter_body(&text, "## 标准库规范 · 12 `std.text`");
    // ⚠ 四个锚点在 §12 内**各自唯一**:`NFC_QC` 这个**成员名**在 §12 里出现 7 次
    //   (分散在成员表 / §12.2 小节标题 / 条款正文 / §12.4 示例 4 处),拿它当
    //   needle 等于判「§12 里提过 NFC_QC」⇒ 条款正文被删光也照样绿。改钉
    //   **条款里的那几句**:正向定义 /「不保证」/ 反向不成立 / 文面禁止 ——
    //   它们各自只出现一次。
    for needle in ["已是 NFC 形态", "不保证", "反向不成立", "本规范的文面不得"]
    {
        assert!(
            body.contains(needle),
            "stdlib spec §12.2 lost `{needle}` — the one-directional wording of \
             NFC_QC is a normative clause, not commentary"
        );
    }
}
