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

mod common;

use std::path::{Path, PathBuf};
use wlwl_eval::Evaluator;

// ---------------------------------------------------------------------------
// [v1.0 规范 §8.6] 期望值不变,**用例程序**补一句显式丢弃
//
// 域违例按 §7 是 `ERR(...)` **值**。那个值到调用方手里时处于**未消费**状态
// (§8.6.1);`IS_ERR` 是**观察型**消费者,不解除它 ⇒ 作用域结束时是丢弃点
// (§8.6.2 第 3 项)⇒ 报 `E0103`。所以这类用例现在写成
// `LET(r, <成员>); LET(_, r); IF(IS_ERR(r), …)` —— `LET(_, …)` 是本语言
// **唯一**的显式丢弃构造(§8.6.4)。**冻结的 `expect` 一个字没动**。
// ---------------------------------------------------------------------------

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
    // ── [v0.11.2 M2] INDEX_OF ──
    Case {
        name: "index_of_first_occurrence",
        src: r#"IMPORT("wlwl:std.str", ["INDEX_OF"]); INDEX_OF("hello", "ll")"#,
        expect: "2",
    },
    Case {
        name: "index_of_miss_is_minus_one",
        src: r#"IMPORT("wlwl:std.str", ["INDEX_OF"]); INDEX_OF("hello", "zz")"#,
        expect: "-1",
    },
    Case {
        name: "index_of_respects_from",
        src: r#"IMPORT("wlwl:std.str", ["INDEX_OF"]); INDEX_OF("hello", "l", 3)"#,
        expect: "3",
    },
    // from 越过末尾:不是 0 也不是越界异常,就是「没找到」。
    Case {
        name: "index_of_from_past_end",
        src: r#"IMPORT("wlwl:std.str", ["INDEX_OF"]); INDEX_OF("hello", "l", 99)"#,
        expect: "-1",
    },
    // 负 from 钳到 0(本成员按前向检索定义,不继承 SUB 的从尾计数)
    Case {
        name: "index_of_negative_from_clamps_to_zero",
        src: r#"IMPORT("wlwl:std.str", ["INDEX_OF"]); INDEX_OF("hello", "l", -5)"#,
        expect: "2",
    },
    // needle 比 s 长:SUB 越界钳制,取到的短串自然 != needle
    Case {
        name: "index_of_needle_longer_than_haystack",
        src: r#"IMPORT("wlwl:std.str", ["INDEX_OF"]); INDEX_OF("hi", "hello")"#,
        expect: "-1",
    },
    // 下标是**码点**不是字节:é 算一个
    Case {
        name: "index_of_index_is_codepoint_based",
        src: r#"IMPORT("wlwl:std.str", ["INDEX_OF"]); INDEX_OF("héllo", "llo")"#,
        expect: "2",
    },
    // 用它切回来:证明下标与 SUB 同一坐标系
    Case {
        name: "index_of_round_trips_through_sub",
        src: r#"IMPORT("wlwl:std.str", ["INDEX_OF"]); SUB("héllo", INDEX_OF("héllo", "llo"), 3)"#,
        expect: "llo",
    },
    // 空 needle 是硬错,与 COUNT 同口径(§6 注)
    Case {
        name: "index_of_empty_needle_is_e0030",
        src: r#"IMPORT("wlwl:std.str", ["INDEX_OF"]); INDEX_OF("abc", "")"#,
        expect: "!E0030 INDEX_OF: sub must not be an empty string",
    },
    // COUNT 早就拒空串 —— 上表已有 `count_empty_needle_is_e0030` 锁住那条消息,
    // M2 把三处内联的检查收成 `_NEED_SUB` 助手后**该用例仍绿**(消息逐字不变)。
    // 这里不再重复一条,避免同表重名。
    Case {
        name: "index_of_arity_is_e0022",
        src: r#"IMPORT("wlwl:std.str", ["INDEX_OF"]); INDEX_OF("a")"#,
        expect: "!E0022 INDEX_OF: function expects 3 argument(s), got 1",
    },
    Case {
        name: "index_of_non_string_is_e0030",
        src: r#"IMPORT("wlwl:std.str", ["INDEX_OF"]); INDEX_OF(5, "a")"#,
        expect: "!E0030 INDEX_OF: expected string, got integer",
    },
    // ── [v0.11.2 M2] CONTAINS_SUB ──
    Case {
        name: "contains_sub_true",
        src: r#"IMPORT("wlwl:std.str", ["CONTAINS_SUB"]); CONTAINS_SUB("hello", "ell")"#,
        expect: "TRUE",
    },
    Case {
        name: "contains_sub_false",
        src: r#"IMPORT("wlwl:std.str", ["CONTAINS_SUB"]); CONTAINS_SUB("hello", "xyz")"#,
        expect: "FALSE",
    },
    Case {
        name: "contains_sub_needle_longer_than_haystack",
        src: r#"IMPORT("wlwl:std.str", ["CONTAINS_SUB"]); CONTAINS_SUB("hi", "hello")"#,
        expect: "FALSE",
    },
    Case {
        name: "contains_sub_empty_needle_is_e0030",
        src: r#"IMPORT("wlwl:std.str", ["CONTAINS_SUB"]); CONTAINS_SUB("abc", "")"#,
        expect: "!E0030 CONTAINS_SUB: sub must not be an empty string",
    },
    Case {
        name: "str_does_not_re_export_sub",
        src: r#"IMPORT("wlwl:std.str", ["SUB"]); NULL"#,
        expect: "!E0023",
    },
    Case {
        name: "all_section6_members_import",
        src: r#"IMPORT("wlwl:std.str", ["JOIN", "SPLIT_LINES", "CHAR_AT", "COUNT", "QUOTE", "INDEX_OF", "CONTAINS_SUB"]); LEN([JOIN, SPLIT_LINES, CHAR_AT, COUNT, QUOTE, INDEX_OF, CONTAINS_SUB])"#,
        expect: "7",
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
        src: r#"IMPORT("wlwl:std.math", ["SQRT"]); LET(r, SQRT(-1)); LET(_, r); IF(IS_ERR(r), "ERR", "NOT_ERR")"#,
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
        src: r#"IMPORT("wlwl:std.math", ["POW"]); LET(r, POW(0, -1)); LET(_, r); IF(IS_ERR(r), "ERR", "NOT_ERR")"#,
        expect: "ERR",
    },
    Case {
        name: "pow_negative_base_fractional_exponent_is_domain_error",
        src: r#"IMPORT("wlwl:std.math", ["POW"]); LET(r, POW(-8, 0.5)); LET(_, r); IF(IS_ERR(r), "ERR", "NOT_ERR")"#,
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
        src: r#"IMPORT("wlwl:std.math", ["CLAMP"]); LET(r, CLAMP(5, 10, 0)); LET(_, r); IF(IS_ERR(r), "ERR", "NOT_ERR")"#,
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
    // 导入一个**不存在**的成员名 → E0023。
    //
    // [v0.11.2 M4] 原文这条用的名字是 `SIN`,而 M4 把 `SIN` 变成了真成员 ——
    // 于是导入成功、表达式求值成 `NULL`,契约表打回。同一条断言第二次被
    // 「成员面增长」打断,故选一个**不会**被补上的名字:`LOG`。
    // 本册刻意只发 `LN` / `LOG2` / `LOG10` 三个固定底,不发基数为参数的
    // `LOG`(要它就得定默认底与类型提升,那是另一次裁决)。所以 `LOG` 是
    // 最可能被误写、也确实不该存在的名字。
    Case {
        name: "math_rejects_non_member",
        src: r#"IMPORT("wlwl:std.math", ["LOG"]); NULL"#,
        expect: "!E0023",
    },
    Case {
        name: "all_section7_members_import",
        src: r#"IMPORT("wlwl:std.math", ["ABS", "MIN", "MAX", "FLOOR", "CEIL", "ROUND", "SQRT", "POW", "CLAMP", "PI", "E", "LN", "LOG2", "LOG10", "EXP", "TRUNC", "SIN", "COS", "TAN", "ASIN", "ACOS", "ATAN", "ATAN2", "SINH", "COSH", "TANH", "SIGN", "DIV_CEIL", "GCD", "LCM", "IS_SQRT", "POW_MOD"]); LEN([ABS, MIN, MAX, FLOOR, CEIL, ROUND, SQRT, POW, CLAMP, PI, E, LN, LOG2, LOG10, EXP, TRUNC, SIN, COS, TAN, ASIN, ACOS, ATAN, ATAN2, SINH, COSH, TANH, SIGN, DIV_CEIL, GCD, LCM, IS_SQRT, POW_MOD])"#,
        expect: "32",
    },
    // 取绝对值时撞上 INTEGER 下界 → E0034 并中止整个运行。规范 §7 该行
    // 原写失败列「—」,已更正;这条用例把更正后的行为钉住。
    // (`imin` 只能用两步构造:字面量写不出 i64::MIN,`-x` 是 `-(0, x)` 而
    //  `2^63` 本身已溢出。)
    Case {
        name: "gcd_at_integer_min_is_e0034",
        src: r#"IMPORT("wlwl:std.math", ["GCD"]); LET(m, -(0, 9223372036854775807)); GCD(-(m, 1), 3)"#,
        expect: "!E0034 NEG: cannot negate INTEGER_MIN (-9223372036854775808); use -INTEGER_MIN+1 or special-case",
    },
    // ── [v0.11.2 M4] 超越函数族 ──
    Case {
        name: "ln_of_one_is_zero",
        src: r#"IMPORT("wlwl:std.math", ["LN"]); LN(1.0)"#,
        expect: "0.0",
    },
    Case {
        name: "ln_of_e_is_one",
        src: r#"IMPORT("wlwl:std.math", ["LN", "E"]); LN(E)"#,
        expect: "1.0",
    },
    // §7.1-8:`LN(0)` 是 -inf,不是错误 —— 与 C / Python / Go 一致
    Case {
        name: "ln_of_zero_is_neg_inf_not_an_error",
        src: r#"IMPORT("wlwl:std.math", ["LN"]); IS_ERR(LN(0.0))"#,
        expect: "FALSE",
    },
    Case {
        name: "ln_of_negative_is_domain_error",
        src: r#"IMPORT("wlwl:std.math", ["LN"]); AT_K(ERR_PAYLOAD(LN(-1.0)), "kind", "?")"#,
        expect: "DomainError",
    },
    Case {
        name: "log2_of_eight_is_three",
        src: r#"IMPORT("wlwl:std.math", ["LOG2"]); LOG2(8.0)"#,
        expect: "3.0",
    },
    Case {
        name: "log10_of_thousand_is_three",
        src: r#"IMPORT("wlwl:std.math", ["LOG10"]); LOG10(1000.0)"#,
        expect: "3.0",
    },
    Case {
        name: "exp_of_one_is_e",
        src: r#"IMPORT("wlwl:std.math", ["EXP", "E"]); EXP(1.0)"#,
        expect: "2.718281828459045",
    },
    // 超越函数之间的互逆关系:比逐个记字面值更耐改
    Case {
        name: "exp_and_ln_are_inverses",
        src: r#"IMPORT("wlwl:std.math", ["EXP", "LN"]); LN(EXP(2.5))"#,
        expect: "2.5",
    },
    // 互逆关系要挑对:`EXP` 的逆是 `LN`,不是 `LOG2` / `LOG10`。
    // `LOG2(EXP(3))` = 3·log2(e) ≈ 4.328,不是 3 —— 第一版把这条写错了,
    // 而且错在**用例名字**上(叫「互逆」而表达式并不是互逆对)。
    Case {
        name: "exp_of_ln_is_inverse",
        src: r#"IMPORT("wlwl:std.math", ["EXP", "LN"]); EXP(LN(2.5))"#,
        expect: "2.5",
    },
    // LOG2 / LOG10 与底数幂互逆
    Case {
        name: "log2_and_pow2_are_inverses",
        src: r#"IMPORT("wlwl:std.math", ["LOG2", "POW"]); LOG2(POW(2.0, 10.0))"#,
        expect: "10.0",
    },
    // ── TRUNC:与全局 INT 的差别只在界外(§7.1-1)──
    Case {
        name: "trunc_toward_zero",
        src: r#"IMPORT("wlwl:std.math", ["TRUNC"]); TRUNC(-1.7)"#,
        expect: "-1.0",
    },
    Case {
        name: "trunc_integer_is_identity",
        src: r#"IMPORT("wlwl:std.math", ["TRUNC"]); TRUNC(5)"#,
        expect: "5",
    },
    // 这条是 TRUNC 存在的全部理由:同一个输入,INT 中止而 TRUNC 返回
    Case {
        name: "trunc_survives_where_int_e0035s",
        src: r#"IMPORT("wlwl:std.math", ["TRUNC"]); TRUNC(1e20)"#,
        expect: "100000000000000000000.0",
    },
    Case {
        name: "trunc_non_number_is_a_diagnostic",
        src: r#"IMPORT("wlwl:std.math", ["TRUNC"]); TRUNC("x")"#,
        expect: "!E0030 TRUNC: expected number, got string",
    },
    // ── 三角 / 反三角 ──
    Case {
        name: "sin_cos_tan_at_zero",
        src: r#"IMPORT("wlwl:std.math", ["SIN","COS","TAN","ATAN"]); [SIN(0.0), COS(0.0), TAN(0.0), ATAN(0.0)]"#,
        expect: "[0.0, 1.0, 0.0, 0.0]",
    },
    Case {
        name: "sin_and_asin_are_inverses",
        src: r#"IMPORT("wlwl:std.math", ["SIN", "ASIN"]); ASIN(SIN(0.5))"#,
        expect: "0.5",
    },
    Case {
        name: "asin_of_one_is_half_pi",
        src: r#"IMPORT("wlwl:std.math", ["ASIN", "PI"]); ASIN(1.0)"#,
        expect: "1.5707963267948966",
    },
    Case {
        name: "acos_of_one_is_zero",
        src: r#"IMPORT("wlwl:std.math", ["ACOS"]); ACOS(1.0)"#,
        expect: "0.0",
    },
    Case {
        name: "asin_outside_unit_interval_is_domain_error",
        src: r#"IMPORT("wlwl:std.math", ["ASIN"]); AT_K(ERR_PAYLOAD(ASIN(2.0)), "kind", "?")"#,
        expect: "DomainError",
    },
    Case {
        name: "acos_outside_unit_interval_is_domain_error",
        src: r#"IMPORT("wlwl:std.math", ["ACOS"]); AT_K(ERR_PAYLOAD(ACOS(-2.0)), "kind", "?")"#,
        expect: "DomainError",
    },
    // ATAN2 全定义域 —— (0,0) 不报错(§7 表格)
    Case {
        name: "atan2_of_zeros_is_zero_not_an_error",
        src: r#"IMPORT("wlwl:std.math", ["ATAN2"]); IS_ERR(ATAN2(0.0, 0.0))"#,
        expect: "FALSE",
    },
    Case {
        name: "atan2_first_quadrant",
        src: r#"IMPORT("wlwl:std.math", ["ATAN2", "ATAN"]); ATAN2(1.0, 1.0)"#,
        expect: "0.7853981633974483",
    },
    // 象限:ATAN2(y, x) 在第二象限给 > π/2
    Case {
        name: "atan2_respects_quadrants",
        src: r#"IMPORT("wlwl:std.math", ["ATAN2"]); >(ATAN2(1.0, -1.0), 1.5707963267948966)"#,
        expect: "TRUE",
    },
    Case {
        name: "hyperbolic_at_zero",
        src: r#"IMPORT("wlwl:std.math", ["SINH","COSH","TANH"]); [SINH(0.0), COSH(0.0), TANH(0.0)]"#,
        expect: "[0.0, 1.0, 0.0]",
    },
    // sinh 是 exp 定义的,故 sinh(x) = (e^x - e^-x)/2
    Case {
        name: "sinh_matches_its_definition",
        src: r#"IMPORT("wlwl:std.math", ["SINH", "EXP"]); SINH(1.0)"#,
        expect: "1.1752011936438014",
    },
    Case {
        name: "tan_is_sin_over_cos",
        src: r#"IMPORT("wlwl:std.math", ["SIN","COS","TAN"]); TAN(0.5)"#,
        expect: "0.5463024898437905",
    },
    // ── 整数数学(§7.1-2…5)──
    Case {
        name: "sign_values",
        src: r#"IMPORT("wlwl:std.math", ["SIGN"]); [SIGN(-3), SIGN(0), SIGN(3)]"#,
        expect: "[-1, 0, 1]",
    },
    // SIGN 恒为 INTEGER,`SIGN(1.0)` 不给 1.0
    Case {
        name: "sign_always_returns_integer",
        src: r#"IMPORT("wlwl:std.math", ["SIGN"]); TYPE(SIGN(-3.5))"#,
        expect: "INTEGER",
    },
    Case {
        name: "sign_of_non_number_is_e0030",
        src: r#"IMPORT("wlwl:std.math", ["SIGN"]); SIGN("x")"#,
        expect: "!E0030 SIGN: expected number, got string",
    },
    // DIV_CEIL 的四个符号组合,§7.1-3
    Case {
        name: "div_ceil_all_sign_combinations",
        src: r#"IMPORT("wlwl:std.math", ["DIV_CEIL"]); [DIV_CEIL(7,2), DIV_CEIL(-7,2), DIV_CEIL(7,-2), DIV_CEIL(-7,-2)]"#,
        expect: "[4, -3, -3, 4]",
    },
    Case {
        name: "div_ceil_exact_does_not_bump",
        src: r#"IMPORT("wlwl:std.math", ["DIV_CEIL"]); DIV_CEIL(8, 2)"#,
        expect: "4",
    },
    // 只收整数:浮点得 E0030,而不是悄悄提升
    Case {
        name: "div_ceil_rejects_float",
        src: r#"IMPORT("wlwl:std.math", ["DIV_CEIL"]); DIV_CEIL(7.0, 2)"#,
        expect: "!E0030 DIV_CEIL: expected integer, got float",
    },
    Case {
        name: "div_ceil_zero_divisor_is_domain_error",
        src: r#"IMPORT("wlwl:std.math", ["DIV_CEIL"]); AT_K(ERR_PAYLOAD(DIV_CEIL(1, 0)), "kind", "?")"#,
        expect: "DomainError",
    },
    Case {
        name: "gcd_and_lcm_basic",
        src: r#"IMPORT("wlwl:std.math", ["GCD", "LCM"]); [GCD(12, 18), LCM(4, 6)]"#,
        expect: "[6, 12]",
    },
    // 只取绝对值,§7.1-4
    Case {
        name: "gcd_takes_absolute_value",
        src: r#"IMPORT("wlwl:std.math", ["GCD", "LCM"]); [GCD(-4, 6), LCM(-4, 6)]"#,
        expect: "[2, 12]",
    },
    Case {
        name: "gcd_with_zero_is_the_other",
        src: r#"IMPORT("wlwl:std.math", ["GCD", "LCM"]); [GCD(0, 5), LCM(0, 5), LCM(0, 0)]"#,
        expect: "[5, 0, 0]",
    },
    // 先除后乘:LCM(2^30, 2^31) 若先乘会溢出(§7.1-4)
    Case {
        name: "lcm_divides_before_multiplying",
        src: r#"IMPORT("wlwl:std.math", ["LCM"]); LCM(1073741824, 2147483648)"#,
        expect: "2147483648",
    },
    Case {
        name: "is_sqrt_floors",
        src: r#"IMPORT("wlwl:std.math", ["IS_SQRT"]); [IS_SQRT(17), IS_SQRT(16)]"#,
        expect: "[4, 4]",
    },
    // 上界压到 floor(sqrt(i64::MAX)):这一条在没压上界时会 E0035 中止(§7.1-5)
    Case {
        name: "is_sqrt_survives_huge_input",
        src: r#"IMPORT("wlwl:std.math", ["IS_SQRT"]); IS_SQRT(4611686018427387904)"#,
        expect: "2147483648",
    },
    Case {
        name: "is_sqrt_zero",
        src: r#"IMPORT("wlwl:std.math", ["IS_SQRT"]); IS_SQRT(0)"#,
        expect: "0",
    },
    Case {
        name: "is_sqrt_negative_is_domain_error",
        src: r#"IMPORT("wlwl:std.math", ["IS_SQRT"]); AT_K(ERR_PAYLOAD(IS_SQRT(-1)), "kind", "?")"#,
        expect: "DomainError",
    },
    // ── POW_MOD(§7.1-6、7)──
    Case {
        name: "pow_mod_basic",
        src: r#"IMPORT("wlwl:std.math", ["POW_MOD"]); POW_MOD(2, 10, 1000)"#,
        expect: "24",
    },
    Case {
        name: "pow_mod_zero_exponent_is_one",
        src: r#"IMPORT("wlwl:std.math", ["POW_MOD"]); POW_MOD(5, 0, 7)"#,
        expect: "1",
    },
    // 大模数:门面做不了这件事,必须 kernel(§7.1-6)。2^61 模 (2^61-1) = 1
    Case {
        name: "pow_mod_handles_cryptographic_modulus",
        src: r#"IMPORT("wlwl:std.math", ["POW_MOD"]); POW_MOD(2, 61, 2305843009213693951)"#,
        expect: "1",
    },
    // 欧几里得余数:Rust 的 % 会给 -1,这里给 2(§7.1-7)
    Case {
        name: "pow_mod_uses_euclidean_remainder",
        src: r#"IMPORT("wlwl:std.math", ["POW_MOD"]); POW_MOD(-7, 3, 5)"#,
        expect: "2",
    },
    Case {
        name: "pow_mod_zero_modulus_is_domain_error",
        src: r#"IMPORT("wlwl:std.math", ["POW_MOD"]); AT_K(ERR_PAYLOAD(POW_MOD(2, 3, 0)), "kind", "?")"#,
        expect: "DomainError",
    },
    Case {
        name: "pow_mod_negative_exponent_is_domain_error",
        src: r#"IMPORT("wlwl:std.math", ["POW_MOD"]); AT_K(ERR_PAYLOAD(POW_MOD(2, -3, 5)), "kind", "?")"#,
        expect: "DomainError",
    },
    // ── kernel 不外泄(§0.2)──
    Case {
        name: "float_kernels_stay_private",
        src: r#"IMPORT("wlwl:std.math", ["_LN"]); NULL"#,
        expect: "!E0023",
    },
    Case {
        name: "pow_mod_kernel_stays_private",
        src: r#"IMPORT("wlwl:std.math", ["_POW_MOD"]); NULL"#,
        expect: "!E0023",
    },
];

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_strmath")
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
    assert_eq!(impls.len(), 7, "§6 declares 7 members");
    for n in [
        "JOIN",
        "SPLIT_LINES",
        "CHAR_AT",
        "COUNT",
        "QUOTE",
        // [v0.11.2 M2] 追加这两个时,原计划还写了第三个 `COUNT_SUB`。
        // 实测 `COUNT` 早就是非重叠子串计数(§6 表里一直有),`COUNT_SUB`
        // 会是重复成员;真正的缺口只有**位置**与**布尔判定**。
        "INDEX_OF",
        "CONTAINS_SUB",
    ] {
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
    // [v0.11.3 M7 / addendum-10 W-03] 32 -> 40:新增 `DECIMAL` 族(规范 §7.2,
    // 裁决见 `ADR-0031` D1)。
    assert_eq!(impls.len(), 61, "§7 declares 32 members");
    for n in [
        "ABS",
        "MIN",
        "MAX",
        "FLOOR",
        "CEIL",
        "ROUND",
        "SQRT",
        "POW",
        "CLAMP",
        "PI",
        "E",
        // [v0.11.2 M4] 追加的二十一个。超越函数族 + TRUNC 走 R2 浮点内核,
        // SIGN / DIV_CEIL / GCD / LCM / IS_SQRT 是纯 wlwl 门面,POW_MOD 走
        // R2(门面算不出中间积是否溢出 —— 见 stdlib §7.1-6)。
        "LN",
        "LOG2",
        "LOG10",
        "EXP",
        "TRUNC",
        "SIN",
        "COS",
        "TAN",
        "ASIN",
        "ACOS",
        "ATAN",
        "ATAN2",
        "SINH",
        "COSH",
        "TANH",
        "SIGN",
        "DIV_CEIL",
        "GCD",
        "LCM",
        "IS_SQRT",
        "POW_MOD",
        // [v0.11.3 M7 / addendum-10 W-03] 任意精度十进制族(§7.2)。
        // ⚠ **本族不含 `MEAN`** —— `SUM` + `/` 已覆盖(`ADR-0031` D2)。
        "DECIMAL",
        "DECIMAL_ADD",
        "DECIMAL_SUB",
        "DECIMAL_MUL",
        "DECIMAL_DIV",
        "DECIMAL_CMP",
        "DECIMAL_TO_STRING",
        "DECIMAL_ROUND",
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

// ── `DECIMAL` 族(规范 §7.2 · ADR-0031 D1)──────────────────────────────
//
// ⚠ **期望值全部取自 Python `decimal` 模块**(默认精度 28、舍入 `ROUND_HALF_EVEN`),
// **不是实现输出** —— 由 `D:\Temp\py_decimal_ref.py` 独立跑一遍再落表。
// 这一条是本契约表的立足点:`DECIMAL` 的价值就是「比 f64 准」,若期望值来自
// 自己的实现,那它对「准不准」这件事什么都不能证明。

const DEC_IMP: &str = r#"IMPORT("wlwl:std.math", ["DECIMAL", "DECIMAL_ADD", "DECIMAL_SUB", "DECIMAL_MUL", "DECIMAL_DIV", "DECIMAL_CMP", "DECIMAL_TO_STRING", "DECIMAL_ROUND"]);"#;

fn dec_run(expr: &str) -> String {
    actual(&format!("{DEC_IMP} {expr}"))
}

/// 构造 + 四则(`DECIMAL("1.25")` 与 `DECIMAL("0.5")`)。
#[test]
fn decimal_arithmetic_matches_python_decimal() {
    let cases: Vec<(&str, &str)> = vec![
        (
            r#"DECIMAL_TO_STRING(DECIMAL_ADD(DECIMAL("1.25"), DECIMAL("0.5")))"#,
            "1.75",
        ),
        (
            r#"DECIMAL_TO_STRING(DECIMAL_SUB(DECIMAL("1.25"), DECIMAL("0.5")))"#,
            "0.75",
        ),
        (
            r#"DECIMAL_TO_STRING(DECIMAL_MUL(DECIMAL("1.25"), DECIMAL("0.5")))"#,
            "0.625",
        ),
        (
            r#"DECIMAL_TO_STRING(DECIMAL_ADD(DECIMAL("1.50"), DECIMAL("2.25")))"#,
            "3.75",
        ),
    ];
    for (expr, want) in cases {
        assert_eq!(&dec_run(expr), want, "expr = {expr}");
    }
}

/// `scale` 保留 —— 金额场景靠它保留「几位」。
#[test]
fn decimal_preserves_scale() {
    assert_eq!(&dec_run(r#"DECIMAL_TO_STRING(DECIMAL("1.00"))"#), "1.00");
    assert_eq!(&dec_run(r#"DECIMAL_TO_STRING(DECIMAL("1.5"))"#), "1.5");
}

/// 除法:`scale` 必填 + `HALF_EVEN` + 除不尽。
#[test]
fn decimal_division_respects_scale_and_half_even() {
    let cases: Vec<(&str, &str)> = vec![
        (
            r#"DECIMAL_TO_STRING(DECIMAL_DIV(DECIMAL("1.25"), DECIMAL("0.5"), 2))"#,
            "2.50",
        ),
        (
            r#"DECIMAL_TO_STRING(DECIMAL_DIV(DECIMAL("1.25"), DECIMAL("0.5"), 4))"#,
            "2.5000",
        ),
        // 恰好一半 ⇒ half-even 取偶数(2.5 -> 2,而不是 3)
        (
            r#"DECIMAL_TO_STRING(DECIMAL_DIV(DECIMAL("1.25"), DECIMAL("0.5"), 0))"#,
            "2",
        ),
        (
            r#"DECIMAL_TO_STRING(DECIMAL_DIV(DECIMAL("1"), DECIMAL("2"), 2))"#,
            "0.50",
        ),
        (
            r#"DECIMAL_TO_STRING(DECIMAL_DIV(DECIMAL("1"), DECIMAL("3"), 5))"#,
            "0.33333",
        ),
    ];
    for (expr, want) in cases {
        assert_eq!(&dec_run(expr), want, "expr = {expr}");
    }
}

/// `b = 0` ⇒ `ERR([kind: DomainError, op: DIV])`,**不是**原生诊断。
#[test]
fn decimal_division_by_zero_is_a_domain_err() {
    let src = format!(
        "{DEC_IMP} LET(e, DECIMAL_DIV(DECIMAL(\"1\"), DECIMAL(\"0\"), 2));\
         [IS_ERR(e), AT_K(ERR_PAYLOAD(e), \"kind\", \"?\"), AT_K(ERR_PAYLOAD(e), \"op\", \"?\")]"
    );
    assert_eq!(actual(&src), "[TRUE, DomainError, DIV]");
}

/// 舍入:六种模式,含**负数上的方向差**(那是 `FLOOR` 与 `DOWN` 的唯一区别)。
#[test]
fn decimal_rounding_matches_python_decimal_modes() {
    let cases: Vec<(&str, &str)> = vec![
        (r#"DECIMAL_ROUND(DECIMAL("2.5"), 0, "HALF_EVEN")"#, "2"),
        (r#"DECIMAL_ROUND(DECIMAL("3.5"), 0, "HALF_EVEN")"#, "4"),
        (r#"DECIMAL_ROUND(DECIMAL("2.5"), 0, "HALF_UP")"#, "3"),
        (r#"DECIMAL_ROUND(DECIMAL("2.4"), 0, "HALF_UP")"#, "2"),
        (r#"DECIMAL_ROUND(DECIMAL("2.9"), 0, "DOWN")"#, "2"),
        (r#"DECIMAL_ROUND(DECIMAL("2.1"), 0, "UP")"#, "3"),
        (r#"DECIMAL_ROUND(DECIMAL("-2.5"), 0, "HALF_EVEN")"#, "-2"),
        // ⚠ FLOOR 向 -inf、DOWN 向零 —— 负数上这是**两个不同的结果**
        (r#"DECIMAL_ROUND(DECIMAL("-2.1"), 0, "FLOOR")"#, "-3"),
        (r#"DECIMAL_ROUND(DECIMAL("-2.1"), 0, "DOWN")"#, "-2"),
        (r#"DECIMAL_ROUND(DECIMAL("-2.1"), 0, "CEILING")"#, "-2"),
        (
            r#"DECIMAL_ROUND(DECIMAL("1.0049"), 3, "HALF_EVEN")"#,
            "1.005",
        ),
        (
            r#"DECIMAL_ROUND(DECIMAL("0.0049"), 3, "HALF_EVEN")"#,
            "0.005",
        ),
        // 恰好一半且前一位为偶 ⇒ 不进(1.005 -> 1.00)
        (r#"DECIMAL_ROUND(DECIMAL("1.005"), 2, "HALF_EVEN")"#, "1.00"),
    ];
    for (expr, want) in cases {
        assert_eq!(
            &dec_run(&format!("DECIMAL_TO_STRING({expr})")),
            want,
            "expr = {expr}"
        );
    }
}

/// 比较按**数值**,不受 `scale` 影响。
#[test]
fn decimal_cmp_is_numeric_not_lexical() {
    let cases: Vec<(&str, &str)> = vec![
        (r#"DECIMAL_CMP(DECIMAL("1.0"), DECIMAL("1.00"))"#, "0"),
        (r#"DECIMAL_CMP(DECIMAL("1.5"), DECIMAL("2.5"))"#, "-1"),
        (r#"DECIMAL_CMP(DECIMAL("2.5"), DECIMAL("1.5"))"#, "1"),
        (r#"DECIMAL_CMP(DECIMAL("-1"), DECIMAL("1"))"#, "-1"),
        // 跨 scale 的字面量比较:`10 < 9` 是**错的**,`10.0 > 9.5` 才对
        (r#"DECIMAL_CMP(DECIMAL("10.0"), DECIMAL("9.5"))"#, "1"),
    ];
    for (expr, want) in cases {
        assert_eq!(&dec_run(expr), want, "expr = {expr}");
    }
}

/// **本族存在的理由**:超出 `f64` 的 `2^53` 之后 `FLOAT` 会静默丢精度。
#[test]
fn decimal_keeps_precision_beyond_the_f64_boundary() {
    // f64 给 9007199254740992(丢掉了 +1);DECIMAL 给 9007199254740994。
    assert_eq!(
        &dec_run(r#"DECIMAL_TO_STRING(DECIMAL_ADD(DECIMAL("9007199254740993"), DECIMAL("1")))"#),
        "9007199254740994"
    );
    assert_eq!(
        &dec_run(r#"DECIMAL_TO_STRING(DECIMAL_MUL(DECIMAL("9007199254740993"), DECIMAL("1")))"#),
        "9007199254740993"
    );
}

/// 规范形式:`-0` 归零、`scale < 0` 非法、`digits` 带前导零非法。
#[test]
fn decimal_canonical_form_rejects_and_normalises() {
    assert_eq!(
        &dec_run(r#"DECIMAL("-0")"#),
        "[sign: 1, digits: 0, scale: 0]"
    );
    assert_eq!(
        &dec_run(r#"DECIMAL(".5")"#),
        "[sign: 1, digits: 5, scale: 1]"
    );
    // 非数字 digits ⇒ E0030
    let bad = r#"DECIMAL_ADD(DECIMAL("1"), ["sign": 1, "digits": "x", "scale": 0])"#;
    assert!(
        matches("!E0030", &dec_run(bad)),
        "illegal digits must be E0030"
    );
    // 前导零 = 非法形状(**不修复**,ADR-0031 D1)
    let lead = r#"DECIMAL_ADD(DECIMAL("1"), ["sign": 1, "digits": "012", "scale": 0])"#;
    assert!(
        matches("!E0030", &dec_run(lead)),
        "leading zeros must be E0030"
    );
    // sign 只能是 -1 / 1
    let sgn = r#"DECIMAL_ADD(DECIMAL("1"), ["sign": 0, "digits": "12", "scale": 0])"#;
    assert!(matches("!E0030", &dec_run(sgn)), "sign must be +-1");
}

/// `DECIMAL` 的字面量语法与失败口径。
#[test]
fn decimal_literal_parsing_and_its_diagnostics() {
    for bad in ["1.2.3", "", "-", ".", "1a", "1e5"] {
        let expr = format!(r#"DECIMAL("{bad}")"#);
        assert!(
            matches("!E0030", &dec_run(&expr)),
            "literal {bad:?} must be E0030"
        );
    }
}

/// ⚠ **不误伤既有的 32 个成员** —— 这是回归锁。
#[test]
fn decimal_family_does_not_disturb_the_original_thirty_two() {
    let src = r#"IMPORT("wlwl:std.math", ["SQRT", "POW_MOD", "GCD", "LCM", "SIGN"]); [SQRT(9), POW_MOD(5, 3, 7), GCD(-4, 6), LCM(4, 6), SIGN(-3)]"#;
    assert_eq!(actual(src), "[3.0, 6, 2, 12, -1]");
}

// ── `FRACTION` 族(规范 §7.3 · ADR-0031 D1)──────────────────────────────
//
// ⚠ 期望值取自 **Python `fractions` 模块**,**不是实现输出** —— 由
// `D:\Temp\py_fraction_ref.py` 独立跑一遍再落表。

const FR_IMP: &str = r#"IMPORT("wlwl:std.math", ["FRACTION", "FRACTION_ADD", "FRACTION_SUB", "FRACTION_MUL", "FRACTION_DIV", "FRACTION_CMP", "FRACTION_TO_STRING", "FRACTION_TO_DECIMAL", "DECIMAL_TO_STRING"]);"#;

fn fr_run(expr: &str) -> String {
    actual(&format!("{FR_IMP} {expr}"))
}

/// 构造与约分。分母为负时**分子分母同时取反**(规范形式的 `den > 0`)。
#[test]
fn fraction_construction_reduces_and_normalises_sign() {
    let cases: Vec<(&str, &str)> = vec![
        (r#"FRACTION(6, 4)"#, "3/2"),
        (r#"FRACTION(-3, 6)"#, "-1/2"),
        (r#"FRACTION(3, -6)"#, "-1/2"),
        (r#"FRACTION(4, 2)"#, "2"),
        (r#"FRACTION(7, 7)"#, "1"),
    ];
    for (expr, want) in cases {
        assert_eq!(
            &fr_run(&format!("FRACTION_TO_STRING({expr})")),
            want,
            "expr = {expr}"
        );
    }
    // 零恒为 {0, 1} —— 不是 0/5。
    assert_eq!(&fr_run(r#"FRACTION(0, 5)"#), "[num: 0, den: 1]");
}

/// 四则,逐条对照 Python `fractions`(`a = 1/3`, `b = 1/6`)。
#[test]
fn fraction_arithmetic_matches_python_fractions() {
    let cases: Vec<(&str, &str)> = vec![
        (r#"FRACTION_ADD(FRACTION(1, 3), FRACTION(1, 6))"#, "1/2"),
        (r#"FRACTION_SUB(FRACTION(1, 3), FRACTION(1, 6))"#, "1/6"),
        (r#"FRACTION_MUL(FRACTION(1, 3), FRACTION(1, 6))"#, "1/18"),
        // 整除的结果**不写分母**
        (r#"FRACTION_DIV(FRACTION(1, 3), FRACTION(1, 6))"#, "2"),
        // 链式
        (
            r#"FRACTION_MUL(FRACTION_ADD(FRACTION(1, 3), FRACTION(1, 6)), FRACTION(6, 1))"#,
            "3",
        ),
    ];
    for (expr, want) in cases {
        assert_eq!(
            &fr_run(&format!("FRACTION_TO_STRING({expr})")),
            want,
            "expr = {expr}"
        );
    }
}

/// 比较按**数值**,且已约分的等价分数必须判等。
#[test]
fn fraction_cmp_is_numeric() {
    let cases: Vec<(&str, &str)> = vec![
        (r#"FRACTION_CMP(FRACTION(1, 3), FRACTION(1, 6))"#, "1"),
        (r#"FRACTION_CMP(FRACTION(1, 2), FRACTION(2, 4))"#, "0"),
        (r#"FRACTION_CMP(FRACTION(-1, 2), FRACTION(1, 2))"#, "-1"),
        // 交叉相乘在 i64 下会溢出(`3 * 10^19` 量级),比较**不该**因此出错
        (r#"FRACTION_CMP(FRACTION(1, 7), FRACTION(1, 11))"#, "1"),
    ];
    for (expr, want) in cases {
        assert_eq!(&fr_run(expr), want, "expr = {expr}");
    }
}

/// 转十进制,`scale` 必填,舍入恒 `HALF_EVEN`(Python 侧同款)。
#[test]
fn fraction_to_decimal_matches_python() {
    let cases: Vec<(&str, &str)> = vec![
        (r#"FRACTION_TO_DECIMAL(FRACTION(3, 4), 4)"#, "0.7500"),
        (r#"FRACTION_TO_DECIMAL(FRACTION(1, 3), 5)"#, "0.33333"),
        (r#"FRACTION_TO_DECIMAL(FRACTION(1, 2), 2)"#, "0.50"),
        (r#"FRACTION_TO_DECIMAL(FRACTION(-1, 8), 3)"#, "-0.125"),
    ];
    for (expr, want) in cases {
        assert_eq!(
            &fr_run(&format!("DECIMAL_TO_STRING({expr})")),
            want,
            "expr = {expr}"
        );
    }
}

/// 域违例 ⇒ `ERR([kind: DomainError])`;非法 `DICT` ⇒ `E0030` **不修复**。
#[test]
fn fraction_rejects_domain_errors_and_illegal_dicts() {
    for (expr, want) in [
        // 分母为零 ⇒ DomainError(不是原生诊断)
        (
            r#"AT_K(ERR_PAYLOAD(FRACTION(1, 0)), "kind", "?")"#,
            "DomainError",
        ),
        (
            r#"AT_K(ERR_PAYLOAD(FRACTION_DIV(FRACTION(1, 3), FRACTION(0, 1))), "kind", "?")"#,
            "DomainError",
        ),
        // 未约分 / 负分母 / 零不是 0/1 ⇒ E0030
        (
            r#"FRACTION_ADD(FRACTION(1, 3), ["num": 2, "den": 4])"#,
            "!E0030",
        ),
        (
            r#"FRACTION_ADD(FRACTION(1, 3), ["num": 1, "den": -2])"#,
            "!E0030",
        ),
        (
            r#"FRACTION_ADD(FRACTION(1, 3), ["num": 0, "den": 5])"#,
            "!E0030",
        ),
        // `den` 必须是 INTEGER
        (r#"FRACTION(1, "2")"#, "!E0030"),
        // `scale` 越界
        (r#"FRACTION_TO_DECIMAL(FRACTION(1, 3), 99)"#, "!E0030"),
    ] {
        // ⚠ 诊断带名字和消息,只锁码 ⇒ 用 `matches!` 的「只锁码」前缀规则,
        // 不能用 `assert_eq!`(那会拿整条消息去比)。
        assert!(
            matches(want, &fr_run(expr)),
            "expr = {expr}\n  expected code: {want}"
        );
    }
}

/// **溢出边界**:`(1/3)^k` 的分母是 `3^k`。`3^39 < 2^63-1 < 3^40`
/// (Python 侧实测 `3^40 = 12157665459056928801`)⇒ k=40 必须报
/// `OverflowError`,**绝不允许**让语言层的 `E0035` 冒出来(那个会**中止整个运行**)。
#[test]
fn fraction_overflow_is_caught_before_it_happens() {
    let src = format!(
        "{FR_IMP}
         LET MUT(f, FRACTION(1, 1));
         LET MUT(k, 0);
         WHILE(<(k, 39), (SET(f, FRACTION_DIV(f, FRACTION(3, 1))); SET(k, +(k, 1))));
         [FRACTION_TO_STRING(f), k]"
    );
    assert_eq!(actual(&src), "[1/4052555153018976267, 39]");
    // 再来一次 ⇒ 越界
    let src2 = format!(
        "{FR_IMP}
         LET MUT(f, FRACTION(1, 1));
         LET MUT(k, 0);
         WHILE(<(k, 40), (SET(f, FRACTION_DIV(f, FRACTION(3, 1))); SET(k, +(k, 1))));
         AT_K(ERR_PAYLOAD(f), \"kind\", \"?\")"
    );
    assert_eq!(actual(&src2), "OverflowError");
}

// ── 近似比较(规范 §7.4 · ADR-0031 D4)─────────────────────────────────

const W06_IMP: &str =
    r#"IMPORT("wlwl:std.math", ["APPROX_EQ", "IS_NAN", "IS_INF", "SIGNIFICANT_DIGITS", "LN"]);"#;

/// `0.1 + 0.2 == 0.3` 这个经典陷阱必须过 —— 而裸 `==` 必须仍然失败。
#[test]
fn approx_eq_covers_the_classic_float_trap() {
    let cases: Vec<(&str, &str)> = vec![
        // 裸 `==` 失败(实测 `+(0.1, 0.2)` = 0.30000000000000004)
        ("==(+(0.1, 0.2), 0.3)", "FALSE"),
        // 绝对容差兜住
        ("APPROX_EQ(+(0.1, 0.2), 0.3, 0.0, 1e-9)", "TRUE"),
        // 容差太紧 ⇒ 仍然 FALSE(不是无脑放行)
        ("APPROX_EQ(+(0.1, 0.2), 0.3, 0.0, 1e-18)", "FALSE"),
        // 相对容差在大数量级上才起作用
        ("APPROX_EQ(1000000.0, 1000000.1, 1e-6, 0.0)", "TRUE"),
        ("APPROX_EQ(1000000.0, 1000000.1, 1e-9, 0.0)", "FALSE"),
    ];
    for (expr, want) in cases {
        // ⚠ 这张表混了纯值(`TRUE`/`FALSE`)与诊断码(`!E0030`),而诊断带
        // 名字和消息 —— 统一走 `matches!` 的「只锁码」前缀规则。
        assert!(
            matches(want, &actual(&format!("{W06_IMP} {expr}"))),
            "expr = {expr}\n  expected: {want}"
        );
    }
}

/// ⚠ **`IS_INF` 补的是缺口**:`==(x, x)` 对 `±inf` 为 TRUE,自等绕法抓不到它。
/// `IS_NAN` 则有 `x != x` 可用 —— 但两条都值得有成员。
#[test]
fn is_nan_and_is_inf_close_a_real_gap() {
    let nan = "-(LN(0), LN(0))";
    // ⚠ 类型必须是 `String` 而非 `&str` —— 头两条用 `format!` 拼 `NaN` 表达式。
    let cases: Vec<(String, &str)> = vec![
        // NaN 可达且判得出;自等判据也判得出(所以 IS_NAN 是便利)
        (format!("IS_NAN({nan})"), "TRUE"),
        (format!("==({nan}, {nan})"), "FALSE"),
        // ±inf 判得出;**自等判据失效** ⇒ 这是缺口
        ("IS_INF(LN(0))".to_string(), "TRUE"),
        ("IS_INF(-(0, LN(0)))".to_string(), "TRUE"),
        ("IS_INF(1.0)".to_string(), "FALSE"),
        ("==(LN(0), LN(0))".to_string(), "TRUE"),
    ];
    for (expr, want) in cases {
        // ⚠ 这张表混了纯值(`TRUE`/`FALSE`)与诊断码(`!E0030`),而诊断带
        // 名字和消息 —— 统一走 `matches!` 的「只锁码」前缀规则。
        assert!(
            matches(want, &actual(&format!("{W06_IMP} {expr}"))),
            "expr = {expr}\n  expected: {want}"
        );
    }
}

/// 三个成员在特殊值上都必须是**总函数**:不崩,且给出规定结论。
#[test]
fn approximate_members_are_total_over_special_values() {
    let nan = "-(LN(0), LN(0))";
    let cases: Vec<(String, &str)> = vec![
        // NaN 与任何值(含自己)都不近似
        (format!("APPROX_EQ({nan}, {nan}, 1.0, 1.0)"), "FALSE"),
        (format!("APPROX_EQ({nan}, 1.0, 1.0, 1.0)"), "FALSE"),
        // 同号同无穷才近似
        ("APPROX_EQ(LN(0), LN(0), 0.0, 0.0)".to_string(), "TRUE"),
        (
            "APPROX_EQ(LN(0), -(0, LN(0)), 0.0, 0.0)".to_string(),
            "FALSE",
        ),
        ("APPROX_EQ(LN(0), 1.0, 0.0, 0.0)".to_string(), "FALSE"),
        // SIGNIFICANT_DIGITS 对特殊值恒 FALSE,零恒 TRUE
        (format!("SIGNIFICANT_DIGITS({nan}, 5)"), "FALSE"),
        ("SIGNIFICANT_DIGITS(LN(0), 5)".to_string(), "FALSE"),
        ("SIGNIFICANT_DIGITS(0.0, 5)".to_string(), "TRUE"),
    ];
    for (expr, want) in cases {
        // ⚠ 这张表混了纯值(`TRUE`/`FALSE`)与诊断码(`!E0030`),而诊断带
        // 名字和消息 —— 统一走 `matches!` 的「只锁码」前缀规则。
        assert!(
            matches(want, &actual(&format!("{W06_IMP} {expr}"))),
            "expr = {expr}\n  expected: {want}"
        );
    }
}

/// `SIGNIFICANT_DIGITS(x, n)`:`x` 是否恰好等于它保留 `n` 位有效数字后的值。
#[test]
fn significant_digits_is_a_predicate_on_n() {
    let cases: Vec<(&str, &str)> = vec![
        ("SIGNIFICANT_DIGITS(1.234, 3)", "FALSE"),
        ("SIGNIFICANT_DIGITS(1.234, 4)", "TRUE"),
        // 跨数量级:小数点位置不影响有效数字
        ("SIGNIFICANT_DIGITS(0.0012340, 3)", "FALSE"),
        ("SIGNIFICANT_DIGITS(0.0012340, 5)", "TRUE"),
        ("SIGNIFICANT_DIGITS(3.14159265, 7)", "FALSE"),
        // ⚠ 3.14159265 需要 **9** 位(第 9 位是 5,half-even 落在偶数位 6 上 ⇒
        // 8 位给 3.1415926 ≠ 原值)。Python 侧逐位核对过。
        ("SIGNIFICANT_DIGITS(3.14159265, 8)", "FALSE"),
        ("SIGNIFICANT_DIGITS(3.14159265, 9)", "TRUE"),
        // n 越界 ⇒ E0030(1..=17,f64 精度上限)
        ("SIGNIFICANT_DIGITS(1.0, 0)", "!E0030"),
        ("SIGNIFICANT_DIGITS(1.0, 18)", "!E0030"),
    ];
    for (expr, want) in cases {
        // ⚠ 这张表混了纯值(`TRUE`/`FALSE`)与诊断码(`!E0030`),而诊断带
        // 名字和消息 —— 统一走 `matches!` 的「只锁码」前缀规则。
        assert!(
            matches(want, &actual(&format!("{W06_IMP} {expr}"))),
            "expr = {expr}\n  expected: {want}"
        );
    }
}

/// ⚠ **回归锁**:W-02 / W-03 / W-04 / W-06 都不许动既有的 32 个成员。
#[test]
fn new_families_do_not_disturb_the_original_thirty_two() {
    let src = r#"IMPORT("wlwl:std.math", ["SQRT", "POW_MOD", "GCD", "LCM", "SIGN", "TRUNC"]); [SQRT(9), POW_MOD(5, 3, 7), GCD(-4, 6), LCM(4, 6), SIGN(-3), TRUNC(-2.7)]"#;
    assert_eq!(actual(src), "[3.0, 6, 2, 12, -1, -2.0]");
}

// ── 描述统计族(规范 §7.5 · ADR-0031 D2 / D3)───────────────────────────
//
// ⚠ 期望值取自 **Python `statistics` 模块**(由 `D:\Temp\py_stats_ref.py` 独立跑
// 一遍落表),**不是实现输出**。
//
// ⚠ 一处**有意分歧**:Python 的 `statistics.stdev([7])` 抛 `StatisticsError`,
// 本成员的 `STDDEV_S([7])` 返回 **`NULL`** —— 见规范 §7.5「空与无定义都用 NULL」。
// 这条不是抄漏,是有口径的,契约把它显式钉住。

const ST_IMP: &str = r#"IMPORT("wlwl:std.math", ["MEDIAN", "MODE", "VARIANCE_S", "VARIANCE_P", "STDDEV_S", "STDDEV_P", "PERCENTILE", "CORRELATION", "LINEAR_FIT", "LN"]);"#;

fn st_run(expr: &str) -> String {
    actual(&format!("{ST_IMP} {expr}"))
}

fn st_f(expr: &str, want: &str) {
    assert_eq!(&st_run(expr), want, "expr = {expr}");
}

/// `MEDIAN`:偶数长度取中间两个的**平均**,且平均那一步必须走 `f64`
/// ——整数 `/` 会截断(ADR-0031 §3)。
#[test]
fn median_matches_python_statistics() {
    st_f("MEDIAN([3, 1, 2])", "2.0");
    st_f("MEDIAN([4, 1, 3, 2])", "2.5");
    st_f("MEDIAN([1.5, 2.5, 3.5, 10.5])", "3.0");
    st_f("MEDIAN([7])", "7.0");
}

/// `MEDIAN([])` ⇒ `NULL`,沿用 `std.collection` §5 口径。
#[test]
fn empty_array_gives_null_for_every_statistic() {
    for expr in [
        "MEDIAN([])",
        "MODE([])",
        "VARIANCE_S([])",
        "VARIANCE_P([])",
        "STDDEV_S([])",
        "STDDEV_P([])",
    ] {
        assert_eq!(&st_run(expr), "NULL", "expr = {expr}");
    }
    st_f("PERCENTILE([], 50.0)", "NULL");
}

/// `MODE`:并列取**先出现者**;全部只出现一次 ⇒ **没有众数** ⇒ `NULL`。
#[test]
fn mode_takes_the_first_of_a_tie_and_reports_no_mode() {
    // ⚠ 期望值是 `2` 而不是 `2.0`:`MODE` 返回**原数组里那个元素的原值**
    // (F-05 裁决,2026-10-20)⇒ 纯 `INTEGER` 数组返回 `INTEGER`。
    // ⚠ **取哪个值**的口径(并列取先出现者)一字未动,变的只是返回**类型**。
    st_f("MODE([1, 2, 2, 3])", "2");
    st_f("MODE([2, 1, 2, 1])", "2");
    // ⚠ `multimode` 才会给 [1, 2];本成员是单数 `mode`,没有众数就 `NULL`。
    st_f("MODE([1, 2, 3])", "NULL");
    // ⚠ 与 Python 的**有意分歧**:`statistics.mode([1])` 返回 `1`,本成员返回
    // `NULL`(只出现一次 ⇒ 没有众数)。同 `STDDEV_S([7])` 那条分歧同款理由:
    // 契约把它显式钉住,免得下一个「对齐 Python」的人改回去。
    st_f("MODE([1.0])", "NULL");
}

/// **D13-015 的回归守卫**:`0.0` 与 `-0.0` 是**同一组**,返回值保留**首次出现者
/// 的符号**。
///
/// 期望值取自 **Python `statistics.mode`**(不是本实现输出),并且 Python 的行为与
/// 本成员逐例一致 —— 因为 Python 的 dict 把 `-0.0` 哈希成 `0.0`,而
/// `0.0 == -0.0` 在 IEEE 下为真。
///
/// ⚠ **这一组是 `MODE` 从 Θ(n²) 改成 HashMap 计数时的全部风险所在**:
/// Rust 的 `impl Hash for f64` 按**位**哈希,而 `f64` 的 `Eq` 用 IEEE 相等 ⇒
/// `0.0` 与 `-0.0` **相等但哈希不同**,直接拿 `f64` 当 `HashMap` 的键会把它们
/// 拆成两组,于是 A/B 两条会分别退化成「各出现一次」⇒ `NULL`。
/// 那不是精度差异,是**可观察语义变了**,所以必须钉死。
#[test]
fn mode_treats_signed_zeros_as_one_group_and_keeps_the_first_sign() {
    // A: +0.0 在前 ⇒ 返回 +0.0(不是 -0.0,也不是 NULL)
    st_f("MODE([0.0, NEG(0.0), 1.0])", "0.0");
    // B: -0.0 在前 ⇒ 返回 **-0.0**。渲染必须区分这两个零,否则这条断言是空的。
    st_f("MODE([NEG(0.0), 0.0, 1.0])", "-0.0");
    // C/D: 两个都出现、且只有这一组 ⇒ 返回先出现的那个
    st_f("MODE([0.0, NEG(0.0)])", "0.0");
    st_f("MODE([NEG(0.0), NEG(0.0)])", "-0.0");
    // F: 同一个有符号零重复出现
    st_f("MODE([0.0, 0.0])", "0.0");
}

/// 并列口径的完整形态:「先出现者」指的是**全局最大计数里的第一个**,
/// 不是「最值」「最小值」或「最后出现的那个」。
///
/// 期望值同样取自 Python `statistics.mode`。
#[test]
fn mode_tie_break_is_first_among_the_global_maximum() {
    // J / K / M:两组并列,答案随**位置**变,与数值大小无关。
    //   J: [1,1,2,3,3] → 1.0(不是 3.0 —— 3 更大)
    st_f("MODE([1.0, 1.0, 2.0, 3.0, 3.0])", "1.0");
    //   K: [3,3,1,1,2] → 3.0(不是 1.0 —— 1 更小)
    st_f("MODE([3.0, 3.0, 1.0, 1.0, 2.0])", "3.0");
    //   M: 负数也照同一口径
    st_f("MODE([-1.0, -1.0, 1.0, 1.0])", "-1.0");
    // L: 三者最高 ⇒ 无并列可言
    st_f("MODE([5.0, 5.0, 5.0, 1.0, 2.0])", "5.0");
    // ⚠ 反向陷阱:「先出现者」必须是**第一次达到全局最大值**的那个元素。
    //   这里 2.0 先出现但只占 1 次,而 7.0 占 3 次 ⇒ 答案必须是 7.0;
    //   若实现改成「无脑取第一个多数元素」就会错答 2.0。
    st_f("MODE([2.0, 7.0, 7.0, 7.0, 1.0])", "7.0");
}

/// 样本 / 总体**显式分成两个成员**,不给默认。
#[test]
fn variance_and_stddev_separate_sample_from_population() {
    st_f("VARIANCE_S([2, 4, 4, 4, 5, 5, 7, 9])", "4.571428571428571");
    st_f("VARIANCE_P([2, 4, 4, 4, 5, 5, 7, 9])", "4.0");
    st_f("STDDEV_S([2, 4, 4, 4, 5, 5, 7, 9])", "2.138089935299395");
    st_f("STDDEV_P([2, 4, 4, 4, 5, 5, 7, 9])", "2.0");
}

/// ⚠ **样本 `n = 1` 返回 `NULL`(不是错误)**,总体 `n = 1` 有定义。
///
/// 这是与 Python 的**有意分歧**:Python 抛 `StatisticsError`。契约把它显式钉住,
/// 免得下一个「对齐 Python」的人改回去。
#[test]
fn sample_of_one_is_null_but_population_of_one_is_zero() {
    st_f("STDDEV_S([7])", "NULL");
    st_f("VARIANCE_S([7])", "NULL");
    st_f("STDDEV_P([7])", "0.0");
    st_f("VARIANCE_P([7])", "0.0");
}

/// `PERCENTILE` 的插值方式必须钉死 —— 它是**可观察的**(同样数据五种方法五个值)。
#[test]
fn percentile_uses_linear_interpolation_including_endpoints() {
    st_f("PERCENTILE([1, 2, 3, 4], 0.0)", "1.0");
    st_f("PERCENTILE([1, 2, 3, 4], 100.0)", "4.0");
    st_f("PERCENTILE([1, 2, 3, 4], 50.0)", "2.5");
    st_f("PERCENTILE([1, 2, 3, 4], 25.0)", "1.75");
    st_f("PERCENTILE([1, 2, 3, 4], 10.0)", "1.3");
    st_f("PERCENTILE([9], 42.0)", "9.0");
    // ⚠ `p` 越界是**数据**不是参数错 ⇒ `NULL` 而非 `E0030`。
    st_f("PERCENTILE([1, 2, 3, 4], 150.0)", "NULL");
    st_f("PERCENTILE([1, 2, 3, 4], -1.0)", "NULL");
}

#[test]
fn correlation_and_linear_fit_match_python() {
    st_f(
        "CORRELATION([1, 2, 3, 4, 5], [2, 4, 5, 4, 5])",
        "0.7745966692414834",
    );
    st_f(
        "AT_K(LINEAR_FIT([1, 2, 3, 4, 5], [2, 4, 5, 4, 5]), \"slope\", 0)",
        "0.6",
    );
    st_f(
        "AT_K(LINEAR_FIT([1, 2, 3, 4, 5], [2, 4, 5, 4, 5]), \"intercept\", 0)",
        "2.2",
    );
    st_f("CORRELATION([1, 2, 3], [2, 4, 6])", "1.0");
    // ⚠ `n = 2` 恒 `±1` —— 两点永远共线,这是数学事实不是缺陷。
    st_f("CORRELATION([1, 2], [1, 3])", "1.0");
    st_f("CORRELATION([1, 2], [3, 2])", "-1.0");
}

/// 任一方向方差为零 ⇒ 相关系数 / 斜率**无定义** ⇒ `NULL`(不是 0)。
#[test]
fn zero_variance_gives_null_not_zero() {
    st_f("CORRELATION([1, 2, 3], [5, 5, 5])", "NULL");
    st_f("LINEAR_FIT([2, 2, 2], [1, 2, 3])", "NULL");
}

/// ⚠ **`NaN` / `±inf` 不渗进统计量** —— wlwl 没有浮点异常传播约定
/// (`/(0.0, 0.0)` 直接 `E1003`),让 `NaN` 渗进结果会让调用方**看不出**它不可信,
/// 而 `NULL` 是看得见的。
#[test]
fn non_finite_inputs_yield_null_not_nan() {
    let nan = "-(LN(0), LN(0))";
    for expr in [
        format!("MEDIAN([1, 2, {nan}])"),
        format!("STDDEV_S([1, 2, {nan}])"),
        "STDDEV_P([1, 2, LN(0)])".to_string(),
        format!("CORRELATION([1, 2], [1, {nan}])"),
        format!("LINEAR_FIT([1, 2], [1, {nan}])"),
        format!("MODE([1, {nan}])"),
    ] {
        assert_eq!(&st_run(&expr), "NULL", "expr = {expr}");
    }
}

/// 混装**静默提升**(`ADR-0031` D3):与既有 `SUM` 同口径,**不**报 `E0030`。
#[test]
fn mixed_integer_float_promotes_silently_like_sum() {
    st_f("MEDIAN([1, 2.5, 3])", "2.5");
    st_f("STDDEV_P([1, 2.5, 3])", "0.8498365855987975");
}

/// 形状错 ⇒ `E0030`(带名字与消息,只锁码)。
#[test]
fn wrong_shapes_are_e0030() {
    for (expr, code) in [
        ("MEDIAN(5)", "!E0030"),
        ("MEDIAN([1, \"x\"])", "!E0030"),
        ("CORRELATION([1, 2], [1, 2, 3])", "!E0030"),
        ("PERCENTILE([1, 2], 50)", "!E0030"),
        ("MODE([1, 2], 1)", "!E0022"),
    ] {
        assert!(
            matches(code, &st_run(expr)),
            "expr = {expr}\n  expected code: {code}"
        );
    }
}

// ── F-05:`MODE` 的**返回类型**(业主裁决 2026-10-20)──────────────────────
//
// ⚠ **这是可观察语义变更(breaking)**:`MODE` 此前一律返回 `FLOAT`
// (返回类型由 `f64` 内核决定,与输入无关),现在返回**数组中那个元素的原值**,
// 原样保留其静态类型:纯 `INTEGER` ⇒ `INTEGER`、纯 `FLOAT` ⇒ `FLOAT`、
// **混装 ⇒ `FLOAT`**(语言规范 §2.2 的混装提升)。
//
// ⚠ **取哪个值**的口径(并列取先出现者 / 全部只出现一次 ⇒ `NULL`)一字未动。
// ⚠ 规范侧(标准库规范 §7.5)的类型口径文字归第二波补,本轮只改实现 + 契约。

/// `MODE` 返回**原数组里那个元素的原值**,静态类型原样保留。
#[test]
fn mode_returns_the_original_element_keeping_its_static_type() {
    // 纯 INTEGER ⇒ INTEGER(此前是 `1.0` / FLOAT)
    st_f("MODE([1, 1, 2])", "1");
    st_f("TYPE(MODE([1, 1, 2]))", "INTEGER");
    // 纯 FLOAT ⇒ FLOAT
    st_f("MODE([1.0, 1.0, 2.0])", "1.0");
    st_f("TYPE(MODE([1.0, 1.0, 2.0]))", "FLOAT");
}

/// ⚠ **混装按语言规范 §2.2 提升为 `FLOAT`** —— 输入里只要有**一个** `FLOAT`。
///
/// 这是 F-05 裁决里最容易漏的一半:若按「谁赢就返回谁」实现,
/// `MODE([1, 1.0])` 会返回 `INTEGER` 的 `1`,与 §2.2「混装提升为浮点」相悖。
#[test]
fn mode_promotes_mixed_integer_float_arrays_to_float() {
    st_f("MODE([1, 1.0])", "1.0");
    st_f("TYPE(MODE([1, 1.0]))", "FLOAT");
    // ⚠ 混装判据看**整个输入**:那个 `FLOAT` 不是众数也一样算混装。
    st_f("MODE([1, 1, 2.0])", "1.0");
    st_f("TYPE(MODE([1, 1, 2.0]))", "FLOAT");
}

/// 空数组 / 全部只出现一次 ⇒ **没有众数** ⇒ `NULL`。
#[test]
fn mode_reports_null_for_empty_and_for_all_unique_arrays() {
    st_f("MODE([])", "NULL");
    st_f("MODE([1, 2])", "NULL");
}

/// 并列取**先出现者**,且纯整数数组的返回值是 `INTEGER`(不是 `2.0`)。
#[test]
fn mode_tie_among_integers_takes_the_first_one_and_keeps_integer_type() {
    st_f("MODE([2, 1, 2, 1])", "2");
    st_f("TYPE(MODE([2, 1, 2, 1]))", "INTEGER");
}
