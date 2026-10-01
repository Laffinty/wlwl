//! `wlwl:std.str` / `wlwl:std.math` 成员契约 —— 标准库规范 §6 / §7(底座 v0.11 M3-2)。
//!
//! 与 `collection_contract.rs` 同款结构,三点不同:
//!
//! 1. **没有一个「改造前」的实现可对照** —— §6/§7 是 v0.11 新增成员面,
//!    没有 R2 版可冻结。所以这里的「应然」侧是**规范表格**(从 markdown
//!    解析)+ 逐条成员用例(手写期望,逐条注明依据规范哪一句)。
//! 2. 期望值写成**文字形态**而不是从实现生成 —— 因为没有生成源可跑。
//! 3. 域违例按 §7 规定是 `ERR(...)` **值**(不是原生诊断),与 §5 的类型错
//!    分属两类;混了这两类是最容易犯的错,所以各有独立用例。
//!
//! 「应然」侧与实现不同源是这套锁的全部意义:`stdlib_mirror` 的附录 A
//! 同步测试两侧都读实现,`stdlib_appendix_a_sync` 也只对账镜像与实现 —
//! 规范表格被改坏而实现没动时,那些测试照样全绿。

use std::path::{Path, PathBuf};
use wlwl_eval::Evaluator;

/// 尚未写期望的占位符。**不能**用空串当哨兵:`JOIN([], "-")` 的结果就是
/// 空串,拿空串当「未写」会让「已写且值为空」和「还没写」无法区分。
const UNFROZEN: &str = "~UNFROZEN";

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const STR_CASES: &[Case] = &[
    // ── JOIN ──
    Case {
        name: "join_basic",
        src: r#"IMPORT("wlwl:std.str", ["JOIN"]); JOIN(["a", "b"], "-")"#,
        expect: "a-b",
    },
    Case {
        name: "join_empty_array_is_empty_string",
        src: r#"IMPORT("wlwl:std.str", ["JOIN"]); JOIN([], "-")"#,
        expect: "",
    },
    Case {
        name: "join_renders_non_strings",
        src: r#"IMPORT("wlwl:std.str", ["JOIN"]); JOIN([1, TRUE, NULL], "|")"#,
        expect: "1|TRUE|NULL",
    },
    Case {
        name: "join_empty_separator",
        src: r#"IMPORT("wlwl:std.str", ["JOIN"]); JOIN(["a", "b"], "")"#,
        expect: "ab",
    },
    Case {
        name: "join_non_array",
        src: r#"IMPORT("wlwl:std.str", ["JOIN"]); JOIN("ab", "-")"#,
        expect: "!E0030 JOIN: expected array, got string",
    },
    Case {
        name: "join_non_string_separator",
        src: r#"IMPORT("wlwl:std.str", ["JOIN"]); JOIN(["a"], 5)"#,
        expect: "!E0030 JOIN: expected string, got integer",
    },
    // ── SPLIT_LINES:§6 逐句 ──
    Case {
        name: "split_lines_empty_string_is_empty_array",
        src: r#"IMPORT("wlwl:std.str", ["SPLIT_LINES"]); SPLIT_LINES("")"#,
        expect: "[]",
    },
    Case {
        name: "split_lines_no_newline",
        src: r#"IMPORT("wlwl:std.str", ["SPLIT_LINES"]); SPLIT_LINES("abc")"#,
        expect: "[abc]",
    },
    Case {
        name: "split_lines_basic",
        src: r#"IMPORT("wlwl:std.str", ["SPLIT_LINES"]); SPLIT_LINES("a\nb\nc")"#,
        expect: "[a, b, c]",
    },
    Case {
        name: "split_lines_trailing_newline_makes_no_empty_tail",
        src: r#"IMPORT("wlwl:std.str", ["SPLIT_LINES"]); SPLIT_LINES("a\nb\n")"#,
        expect: "[a, b]",
    },
    Case {
        name: "split_lines_strips_carriage_return",
        src: r#"IMPORT("wlwl:std.str", ["SPLIT_LINES"]); SPLIT_LINES("a\r\nb\r\n")"#,
        expect: "[a, b]",
    },
    Case {
        name: "split_lines_keeps_interior_blank_line",
        src: r#"IMPORT("wlwl:std.str", ["SPLIT_LINES"]); SPLIT_LINES("a\n\nb")"#,
        expect: "[a, , b]",
    },
    // `[""]` 与 `[]` 的 display 都是 `[]` —— 这条只能锁长度。
    Case {
        name: "split_lines_lone_newline_is_one_blank_line",
        src: r#"IMPORT("wlwl:std.str", ["SPLIT_LINES"]); LEN(SPLIT_LINES("\n"))"#,
        expect: "1",
    },
    // 注意必须写 `==(...)`:按语言规范 §4.1,`lines[0] = ""` 是 `INDEX_SET`
    // 语法糖(后缀下标后接 `=`),不是相等比较 —— 写 `=` 会静默变成赋值并
    // 返回改后的容器,断言照样「有值」但验证的根本不是同一件事。
    Case {
        name: "split_lines_lone_newline_element_is_empty",
        src: r#"IMPORT("wlwl:std.str", ["SPLIT_LINES"]); LET(lines, SPLIT_LINES("\n")); ==(lines[0], "")"#,
        expect: "TRUE",
    },
    // 反向用例:确认上一条不是碰巧绿 —— 真相等时 `==` 给布尔。
    Case {
        name: "index_set_sugar_is_not_equality",
        src: r#"LET(lines, ["x"]); TYPE(lines[0] = "y")"#,
        expect: "ARRAY",
    },
    Case {
        name: "split_lines_non_string",
        src: r#"IMPORT("wlwl:std.str", ["SPLIT_LINES"]); SPLIT_LINES(5)"#,
        expect: "!E0030 SPLIT_LINES: expected string, got integer",
    },
    // ── CHAR_AT:等价于 SUB(s, i, 1) ──
    Case {
        name: "char_at_first",
        src: r#"IMPORT("wlwl:std.str", ["CHAR_AT"]); CHAR_AT("abc", 0)"#,
        expect: "a",
    },
    Case {
        name: "char_at_middle",
        src: r#"IMPORT("wlwl:std.str", ["CHAR_AT"]); CHAR_AT("abc", 1)"#,
        expect: "b",
    },
    Case {
        name: "char_at_is_codepoint_not_byte",
        src: r#"IMPORT("wlwl:std.str", ["CHAR_AT"]); CHAR_AT("héllo", 1)"#,
        expect: "é",
    },
    Case {
        name: "char_at_out_of_range_matches_sub",
        src: r#"IMPORT("wlwl:std.str", ["CHAR_AT"]); CHAR_AT("abc", 9)"#,
        expect: "",
    },
    // 负索引自尾部计数 —— `SUB` 就是这个口径(§6:起点同 SUB 的 start),
    // 所以 CHAR_AT 负索引取的是倒数第 i 个码点,不是越界。
    Case {
        name: "char_at_negative_index_matches_sub",
        src: r#"IMPORT("wlwl:std.str", ["CHAR_AT"]); CHAR_AT("abc", -1)"#,
        expect: "c",
    },
    Case {
        name: "char_at_negative_index_is_sub",
        src: r#"IMPORT("wlwl:std.str", ["CHAR_AT"]); ==(CHAR_AT("abc", -2), SUB("abc", -2, 1))"#,
        expect: "TRUE",
    },
    Case {
        name: "char_at_non_string",
        src: r#"IMPORT("wlwl:std.str", ["CHAR_AT"]); CHAR_AT(5, 0)"#,
        expect: "!E0030 CHAR_AT: expected string, got integer",
    },
    Case {
        name: "char_at_non_integer_index",
        src: r#"IMPORT("wlwl:std.str", ["CHAR_AT"]); CHAR_AT("abc", "0")"#,
        expect: "!E0030 CHAR_AT: expected integer, got string",
    },
    // ── COUNT:非重叠出现次数 ──
    Case {
        name: "count_none",
        src: r#"IMPORT("wlwl:std.str", ["COUNT"]); COUNT("abc", "z")"#,
        expect: "0",
    },
    Case {
        name: "count_single",
        src: r#"IMPORT("wlwl:std.str", ["COUNT"]); COUNT("abc", "b")"#,
        expect: "1",
    },
    Case {
        name: "count_adjacent",
        src: r#"IMPORT("wlwl:std.str", ["COUNT"]); COUNT("aaa", "a")"#,
        expect: "3",
    },
    Case {
        name: "count_is_non_overlapping",
        src: r#"IMPORT("wlwl:std.str", ["COUNT"]); COUNT("aaaa", "aa")"#,
        expect: "2",
    },
    Case {
        name: "count_non_overlapping_tricky",
        src: r#"IMPORT("wlwl:std.str", ["COUNT"]); COUNT("abababa", "aba")"#,
        expect: "2",
    },
    Case {
        name: "count_whole_string",
        src: r#"IMPORT("wlwl:std.str", ["COUNT"]); COUNT("abc", "abc")"#,
        expect: "1",
    },
    Case {
        name: "count_empty_needle_is_e0030",
        src: r#"IMPORT("wlwl:std.str", ["COUNT"]); COUNT("abc", "")"#,
        expect: "!E0030 COUNT: sub must not be an empty string",
    },
    Case {
        name: "count_empty_subject",
        src: r#"IMPORT("wlwl:std.str", ["COUNT"]); COUNT("", "a")"#,
        expect: "0",
    },
    // ── QUOTE(M1 落地,M3-2 不改)──
    Case {
        name: "quote_plain",
        src: r#"IMPORT("wlwl:std.str", ["QUOTE"]); QUOTE("ab")"#,
        expect: r#""ab""#,
    },
    Case {
        name: "quote_escapes",
        src: r#"IMPORT("wlwl:std.str", ["QUOTE"]); QUOTE("a\"b\\c")"#,
        expect: r#""a\"b\\c""#,
    },
    // ── §6 明文:不重导出任何全局内建 ──
    Case {
        name: "str_reexports_no_global_builtin",
        src: r#"IMPORT("wlwl:std.str", ["SPLIT"]); NULL"#,
        expect: "!E0023",
    },
    Case {
        name: "str_does_not_re_export_sub",
        src: r#"IMPORT("wlwl:std.str", ["SUB"]); NULL"#,
        expect: "!E0023",
    },
    Case {
        name: "all_section6_members_import",
        src: r#"IMPORT("wlwl:std.str", ["JOIN", "SPLIT_LINES", "CHAR_AT", "COUNT", "QUOTE"]); LEN([JOIN, SPLIT_LINES, CHAR_AT, COUNT, QUOTE])"#,
        expect: "5",
    },
];

const MATH_CASES: &[Case] = &[
    // ── ABS ──
    Case {
        name: "abs_positive",
        src: r#"IMPORT("wlwl:std.math", ["ABS"]); ABS(3)"#,
        expect: "3",
    },
    Case {
        name: "abs_negative",
        src: r#"IMPORT("wlwl:std.math", ["ABS"]); ABS(-3)"#,
        expect: "3",
    },
    Case {
        name: "abs_keeps_type",
        src: r#"IMPORT("wlwl:std.math", ["ABS"]); TYPE(ABS(-1.5))"#,
        expect: "FLOAT",
    },
    // [D11-019] `INTEGER` 下界取负是语言规范 §2.2 的 `E0034`,stdlib §7 的
    // `ABS` 失败格也是 `E0034`(D11-007 更正过)。此前 `NEG` 用裸 `-i`,
    // release 档溢出检查关闭 ⇒ `ABS(INT_MIN)` 静默回绕返回**负数**,而
    // `-(0, INT_MIN)`(同一取负的运算符路径)正确抛 `E0034`。
    // 这三条把两条路径钉在同一口径上:门面的 ABS、名为 NEG 的内建、
    // 以及运算符路径。
    Case {
        name: "abs_integer_min_is_e0034",
        src: r#"IMPORT("wlwl:std.math", ["ABS"]); LET(m, -(0, 9223372036854775807)); ABS(-(m, 1))"#,
        expect: "!E0034 NEG: cannot negate INTEGER_MIN (-9223372036854775808); use -INTEGER_MIN+1 or special-case",
    },
    Case {
        name: "neg_builtin_integer_min_is_e0034",
        src: r#"LET(m, -(0, 9223372036854775807)); NEG(-(m, 1))"#,
        expect: "!E0034 NEG: cannot negate INTEGER_MIN (-9223372036854775808); use -INTEGER_MIN+1 or special-case",
    },
    Case {
        name: "neg_operator_integer_min_is_e0034",
        src: r#"LET(m, -(0, 9223372036854775807)); -(0, -(m, 1))"#,
        expect: "!E0034 NEG: cannot negate INTEGER_MIN (-9223372036854775808); use -INTEGER_MIN+1 or special-case",
    },
    // 边界内侧一步:INT_MIN+1(即 `-(0, 2^63-1)`)取负必须正常,证明上面
    // 不是「一律拒绝」。
    Case {
        name: "neg_one_above_integer_min_is_fine",
        src: r#"NEG(-(0, 9223372036854775807))"#,
        expect: "9223372036854775807",
    },
    // ── MIN / MAX(含 §2.2 提升)──
    Case {
        name: "min_integers",
        src: r#"IMPORT("wlwl:std.math", ["MIN"]); MIN(1, 2)"#,
        expect: "1",
    },
    Case {
        name: "min_reversed",
        src: r#"IMPORT("wlwl:std.math", ["MIN"]); MIN(2, 1)"#,
        expect: "1",
    },
    Case {
        name: "min_mixed_promotes_to_float",
        src: r#"IMPORT("wlwl:std.math", ["MIN"]); TYPE(MIN(1, 2.5))"#,
        expect: "FLOAT",
    },
    Case {
        name: "min_mixed_value",
        src: r#"IMPORT("wlwl:std.math", ["MIN"]); MIN(1, 2.5)"#,
        expect: "1.0",
    },
    Case {
        name: "max_integers",
        src: r#"IMPORT("wlwl:std.math", ["MAX"]); MAX(1, 2)"#,
        expect: "2",
    },
    Case {
        name: "max_mixed_promotes_to_float",
        src: r#"IMPORT("wlwl:std.math", ["MAX"]); MAX(1, 2.5)"#,
        expect: "2.5",
    },
    Case {
        name: "min_non_number",
        src: r#"IMPORT("wlwl:std.math", ["MIN"]); MIN(1, "a")"#,
        expect: "!E0030 MIN: expected number, got string",
    },
    // ── FLOOR / CEIL / ROUND(INTEGER 恒等;不是 INT!)──
    Case {
        name: "floor_integer_is_identity",
        src: r#"IMPORT("wlwl:std.math", ["FLOOR"]); FLOOR(3)"#,
        expect: "3",
    },
    Case {
        name: "floor_float",
        src: r#"IMPORT("wlwl:std.math", ["FLOOR"]); FLOOR(1.5)"#,
        expect: "1.0",
    },
    Case {
        name: "floor_negative_is_not_truncation",
        src: r#"IMPORT("wlwl:std.math", ["FLOOR"]); FLOOR(-1.5)"#,
        expect: "-2.0",
    },
    Case {
        name: "floor_exact_integer_float",
        src: r#"IMPORT("wlwl:std.math", ["FLOOR"]); FLOOR(-2.0)"#,
        expect: "-2.0",
    },
    Case {
        name: "ceil_integer_is_identity",
        src: r#"IMPORT("wlwl:std.math", ["CEIL"]); CEIL(3)"#,
        expect: "3",
    },
    Case {
        name: "ceil_float",
        src: r#"IMPORT("wlwl:std.math", ["CEIL"]); CEIL(1.5)"#,
        expect: "2.0",
    },
    Case {
        name: "ceil_negative_equals_truncation",
        src: r#"IMPORT("wlwl:std.math", ["CEIL"]); CEIL(-1.5)"#,
        expect: "-1.0",
    },
    Case {
        name: "round_integer_is_identity",
        src: r#"IMPORT("wlwl:std.math", ["ROUND"]); ROUND(3)"#,
        expect: "3",
    },
    Case {
        name: "round_half_away_from_zero_positive",
        src: r#"IMPORT("wlwl:std.math", ["ROUND"]); ROUND(0.5)"#,
        expect: "1.0",
    },
    Case {
        name: "round_half_away_from_zero_negative",
        src: r#"IMPORT("wlwl:std.math", ["ROUND"]); ROUND(-0.5)"#,
        expect: "-1.0",
    },
    Case {
        name: "round_below_half",
        src: r#"IMPORT("wlwl:std.math", ["ROUND"]); ROUND(1.4)"#,
        expect: "1.0",
    },
    Case {
        name: "round_below_half_negative",
        src: r#"IMPORT("wlwl:std.math", ["ROUND"]); ROUND(-1.4)"#,
        expect: "-1.0",
    },
    Case {
        name: "round_2_5",
        src: r#"IMPORT("wlwl:std.math", ["ROUND"]); ROUND(2.5)"#,
        expect: "3.0",
    },
    Case {
        name: "round_past_2_pow_53_is_identity",
        src: r#"IMPORT("wlwl:std.math", ["FLOOR"]); FLOOR(1.0E20)"#,
        expect: "100000000000000000000.0",
    },
    Case {
        name: "floor_non_number",
        src: r#"IMPORT("wlwl:std.math", ["FLOOR"]); FLOOR("a")"#,
        expect: "!E0030 FLOOR: expected number, got string",
    },
    // ── SQRT / POW(域违例是 ERR **值**,不是诊断)──
    Case {
        name: "sqrt_basic",
        src: r#"IMPORT("wlwl:std.math", ["SQRT"]); SQRT(9)"#,
        expect: "3.0",
    },
    Case {
        name: "sqrt_zero",
        src: r#"IMPORT("wlwl:std.math", ["SQRT"]); SQRT(0)"#,
        expect: "0.0",
    },
    Case {
        name: "sqrt_negative_zero",
        src: r#"IMPORT("wlwl:std.math", ["SQRT"]); SQRT(-0.0)"#,
        expect: "0.0",
    },
    Case {
        name: "sqrt_negative_is_domain_error_value",
        src: r#"IMPORT("wlwl:std.math", ["SQRT"]); LET(r, SQRT(-1)); IF(IS_ERR(r), "ERR", "NOT_ERR")"#,
        expect: "ERR",
    },
    Case {
        name: "sqrt_negative_payload_kind",
        src: r#"IMPORT("wlwl:std.math", ["SQRT"]); LET(r, SQRT(-1)); ERR_PAYLOAD(r)["kind"]"#,
        expect: "DomainError",
    },
    Case {
        name: "sqrt_non_number",
        src: r#"IMPORT("wlwl:std.math", ["SQRT"]); SQRT("a")"#,
        expect: "!E0030 SQRT: expected number, got string",
    },
    Case {
        name: "pow_basic",
        src: r#"IMPORT("wlwl:std.math", ["POW"]); POW(2, 10)"#,
        expect: "1024.0",
    },
    Case {
        name: "pow_promotes_integers",
        src: r#"IMPORT("wlwl:std.math", ["POW"]); TYPE(POW(2, 3))"#,
        expect: "FLOAT",
    },
    Case {
        name: "pow_negative_base_integer_exponent",
        src: r#"IMPORT("wlwl:std.math", ["POW"]); POW(-2, 3)"#,
        expect: "-8.0",
    },
    Case {
        name: "pow_zero_to_negative_is_domain_error",
        src: r#"IMPORT("wlwl:std.math", ["POW"]); LET(r, POW(0, -1)); IF(IS_ERR(r), "ERR", "NOT_ERR")"#,
        expect: "ERR",
    },
    Case {
        name: "pow_negative_base_fractional_exponent_is_domain_error",
        src: r#"IMPORT("wlwl:std.math", ["POW"]); LET(r, POW(-8, 0.5)); IF(IS_ERR(r), "ERR", "NOT_ERR")"#,
        expect: "ERR",
    },
    // ── CLAMP ──
    Case {
        name: "clamp_below",
        src: r#"IMPORT("wlwl:std.math", ["CLAMP"]); CLAMP(-5, 0, 10)"#,
        expect: "0",
    },
    Case {
        name: "clamp_inside",
        src: r#"IMPORT("wlwl:std.math", ["CLAMP"]); CLAMP(5, 0, 10)"#,
        expect: "5",
    },
    Case {
        name: "clamp_above",
        src: r#"IMPORT("wlwl:std.math", ["CLAMP"]); CLAMP(50, 0, 10)"#,
        expect: "10",
    },
    Case {
        name: "clamp_equal_to_bounds",
        src: r#"IMPORT("wlwl:std.math", ["CLAMP"]); CLAMP(0, 0, 10)"#,
        expect: "0",
    },
    Case {
        name: "clamp_promotes_like_min_max",
        src: r#"IMPORT("wlwl:std.math", ["CLAMP"]); TYPE(CLAMP(5, 0, 10.0))"#,
        expect: "FLOAT",
    },
    Case {
        name: "clamp_lo_gt_hi_is_domain_error",
        src: r#"IMPORT("wlwl:std.math", ["CLAMP"]); LET(r, CLAMP(5, 10, 0)); IF(IS_ERR(r), "ERR", "NOT_ERR")"#,
        expect: "ERR",
    },
    Case {
        name: "clamp_lo_eq_hi_is_fine",
        src: r#"IMPORT("wlwl:std.math", ["CLAMP"]); CLAMP(5, 3, 3)"#,
        expect: "3",
    },
    // ── PI / E ──
    Case {
        name: "pi_is_float_constant",
        src: r#"IMPORT("wlwl:std.math", ["PI"]); TYPE(PI)"#,
        expect: "FLOAT",
    },
    Case {
        name: "pi_value",
        src: r#"IMPORT("wlwl:std.math", ["PI"]); PI"#,
        expect: "3.141592653589793",
    },
    Case {
        name: "e_is_float_constant",
        src: r#"IMPORT("wlwl:std.math", ["E"]); TYPE(E)"#,
        expect: "FLOAT",
    },
    Case {
        name: "e_value",
        src: r#"IMPORT("wlwl:std.math", ["E"]); E"#,
        expect: "2.718281828459045",
    },
    Case {
        name: "pi_is_usable_in_arithmetic",
        src: r#"IMPORT("wlwl:std.math", ["PI", "ROUND"]); ROUND(PI)"#,
        expect: "3.0",
    },
    // ── §7 明文:类型错报 E0030(诊断);域违例返 ERR(值)—— 两类分开 ──
    Case {
        name: "math_rejects_non_member",
        src: r#"IMPORT("wlwl:std.math", ["SIN"]); NULL"#,
        expect: "!E0023",
    },
    Case {
        name: "all_section7_members_import",
        src: r#"IMPORT("wlwl:std.math", ["ABS", "MIN", "MAX", "FLOOR", "CEIL", "ROUND", "SQRT", "POW", "CLAMP", "PI", "E"]); LEN([ABS, MIN, MAX, FLOOR, CEIL, ROUND, SQRT, POW, CLAMP, PI, E])"#,
        expect: "11",
    },
];

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("wlwl_strmath_{nanos}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn actual(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => {
            let code = e.diagnostic().code.as_str();
            // `expect: "!E0023"` 这类只锁码不锁消息的写法
            if e.diagnostic().message.is_empty() {
                format!("!{code}")
            } else {
                format!("!{code} {}", e.diagnostic().message)
            }
        }
    }
}

/// 只锁码的期望:实际以 `!EXXXX` 开头且期望恰好等于 `!EXXXX` 时按前缀匹配。
fn matches(expect: &str, got: &str) -> bool {
    if expect.len() == 6 && expect.starts_with('!') && got.starts_with(expect) {
        return true;
    }
    expect == got
}

fn run_table(label: &str, cases: &[Case]) {
    let bad: Vec<String> = cases
        .iter()
        .filter_map(|c| {
            let got = actual(c.src);
            if matches(c.expect, &got) {
                None
            } else {
                Some(format!(
                    "{name}:\n      spec/expected: {want}\n      actual:         {got}",
                    name = c.name,
                    want = c.expect
                ))
            }
        })
        .collect();
    assert!(
        bad.is_empty(),
        "{label}: {}/{} case(s) diverged:\n  - {}",
        bad.len(),
        cases.len(),
        bad.join("\n  - ")
    );
}

#[test]
fn str_matches_section_6() {
    run_table("std.str §6", STR_CASES);
}

#[test]
fn math_matches_section_7() {
    run_table("std.math §7", MATH_CASES);
}

/// 反向守卫:两张表本身不能塌掉,否则上面两条会退化成检查零件事。
#[test]
fn the_case_tables_are_not_empty() {
    assert!(
        STR_CASES.len() >= 30,
        "str table collapsed to {}",
        STR_CASES.len()
    );
    assert!(
        MATH_CASES.len() >= 40,
        "math table collapsed to {}",
        MATH_CASES.len()
    );
    for (label, cases) in [("str", STR_CASES), ("math", MATH_CASES)] {
        let mut names: Vec<&str> = cases.iter().map(|c| c.name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(
            before,
            names.len(),
            "{label} table has a duplicate case name"
        );
        for c in cases {
            assert_ne!(
                c.expect, UNFROZEN,
                "{label}/{} has no expectation yet",
                c.name
            );
        }
    }
}

// ── 与标准库规范表格的外部对照 ──────────────────────────────────────

fn spec_text() -> String {
    let p: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
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

/// 从成员表首列抽出成员名(与 collection 的同名助手同款口径)。
///
/// 一格可以声明多个成员(`SORT(arr)` / `SORT_BY(arr, key)`),按 ` / ` 切;
/// 同一个成员可以在多格里出现(`RANGE` 的三种形态),按首现去重。
fn members_of(section_body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in section_body.lines() {
        let t = line.trim();
        if !t.starts_with('|') || t.contains("---") {
            continue;
        }
        let first = t.trim_start_matches('|').split('|').next().unwrap_or("");
        // 跳过表头(「签名」)与表体首列不是签名的行
        if first.trim() == "签名" {
            continue;
        }
        for cell in first.split(" / ") {
            let name: String = cell
                .trim()
                // `trim_matches` 而不是 `trim_start_matches`:像 §7 的
                // `` | `PI` / `E` | `` 这种**无括号**的裸名单元格没有 `(` 可
                // 供 `split('(')` 顺带清掉尾反引号,只去头的话 `E` 会带着尾
                // 反引号过不了 UPPER_SNAKE 校验,于是整行被静默跳过。
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

fn r1_exports(path: &str) -> Vec<String> {
    let src = wlwl_std::LANG_SOURCES
        .iter()
        .find(|s| s.path == path)
        .unwrap_or_else(|| panic!("{path} is an R1 module"));
    wlwl_std::lang_exports(src.source)
}

fn assert_member_set(label: &str, spec_members: &[String], impls: &[String]) {
    let mut a = spec_members.to_vec();
    let mut b = impls.to_vec();
    a.sort();
    b.sort();
    assert_eq!(
        a, b,
        "{label}: the spec table and the R1 EXPORT list disagree.\n  spec: {spec_members:?}\n  impl: {impls:?}"
    );
    let mut seen = std::collections::HashSet::new();
    for n in impls {
        assert!(seen.insert(n.as_str()), "{label} exports `{n}` twice");
    }
}

#[test]
fn str_member_set_matches_the_spec_table() {
    let spec = members_of(&section(6, 7));
    let impls = r1_exports("wlwl:std.str");
    assert_member_set("std.str §6", &spec, &impls);
    assert_eq!(impls.len(), 5, "§6 declares 5 members");
    for n in ["JOIN", "SPLIT_LINES", "CHAR_AT", "COUNT", "QUOTE"] {
        assert!(
            spec.iter().any(|m| m == n),
            "§6 extractor missed `{n}`: {spec:?}"
        );
    }
}

#[test]
fn math_member_set_matches_the_spec_table() {
    let spec = members_of(&section(7, 8));
    let impls = r1_exports("wlwl:std.math");
    assert_member_set("std.math §7", &spec, &impls);
    assert_eq!(impls.len(), 11, "§7 declares 11 members");
    for n in [
        "ABS", "MIN", "MAX", "FLOOR", "CEIL", "ROUND", "SQRT", "POW", "CLAMP", "PI", "E",
    ] {
        assert!(
            spec.iter().any(|m| m == n),
            "§7 extractor missed `{n}`: {spec:?}"
        );
    }
}

/// `PI` / `E` 是常量不是函数,别让它们「碰巧能用」就算数:锁它们不是函数值。
#[test]
fn math_constants_are_not_callable() {
    let ast = wlwl_parser::parse(r#"IMPORT("wlwl:std.math", ["PI", "E"]); PI(1)"#, "t.wll")
        .expect("parse");
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    let err = ev.eval(&ast).expect_err("PI is a constant, not a function");
    assert_eq!(err.diagnostic().code, wlwl_error::ErrorCode::E0020);
}

/// §0.4 的「不重导出同名全局内建」:§6 里的 5 个名字都不得与全局内建重名。
#[test]
fn str_and_math_reexport_no_global_builtin() {
    let builtins = wlwl_eval::registry::resolved_builtin_names();
    for (path, names) in [
        ("wlwl:std.str", r1_exports("wlwl:std.str")),
        ("wlwl:std.math", r1_exports("wlwl:std.math")),
    ] {
        for n in &names {
            assert!(
                !builtins.iter().any(|b| b == n),
                "{path} exports `{n}`, which is also a global builtin — \
                 stdlib spec §0.4 forbids re-exporting one"
            );
        }
    }
}
