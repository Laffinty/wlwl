//! `wlwl:std.str::QUERY_PARSE` + `wlwl:std.format::FORMAT_NAMED` 成员契约 —— W-28。
//!
//! ## 为什么单独成册
//!
//! `str_math_contract.rs` 钉的是规范 §6 / §7 的**既有**成员面;
//! `format_contract.rs` 钉的是 `FORMAT` 与**全局内建**之间那条**刻意保留**//!
//! 的分歧(§4.1 / `D12-003`)。本册钉的是 **W-28 新增的两个成员**,且**不**
//! 重开上面两册的任何议题。
//!
//! ## 本册要防的两件事
//!
//! 1. **「好心统一」** —— 把 `FORMAT` 的具名查找也改成「认全部 `DICT`」,
//!    或给 `FORMAT` 也加上格式说明。那是**行为变更**,且没有经过裁决
//!    ⇒ 本册用 `FORMAT_REGRESSION` 那张表把既有 `FORMAT` 的输出**逐字**钉住。
//! 2. **「顺手加个 `TO_STRING` / `SPLIT`」** —— 两者**已由全局内建覆盖**
//!    (`STR(x)` / `SPLIT(s, sep)`)⇒ 本册有一条用例断言 `std.str` **不导出**
//!    它们(§6 明文「不重导出任何全局内建」)。
//!
//! ## 调用面
//!
//! 两个成员都是**用户侧**写法 ⇒ 本册走 `IMPORT` + 求值的**真实路径**
//! (不是直接调 `StdFn`):`QUERY_PARSE` 是 **R1** 模块成员(名字来自 `str.wll`
//! 的 `EXPORT`),`FORMAT_NAMED` 是 **R2** 原生成员(名字来自 `format.rs` 的
//! `SPEC`)。走 `IMPORT` 才能同时验到**名字解析**与**行为**。
//!
//! ## `ERR` 值的用例写法
//!
//! `ERR` 值到调用方手里是**未消费**状态(§8.6.1)⇒ 作用域结束时是丢弃点
//! (§8.6.2 第 3 项)⇒ 报 `E0103`。所以这类用例写成
//! `LET(r, <成员>); ERR_PAYLOAD(r)["kind"]` —— `ERR_PAYLOAD` 是**取走型**
//! 消费者(§8.3)⇒ 它把那份未消费状态取走 ⇒ 不产生 `E0103`。

mod common;

use std::path::PathBuf;
use wlwl_eval::Evaluator;

/// 尚未写期望的占位符。**不能**用空串当哨兵:`JOIN([], "-")` 的结果就是
/// 空串,拿空串当「未写」会让「已写且值为空」和「还没写」无法区分。
const UNFROZEN: &str = "~UNFROZEN";

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

// ── QUERY_PARSE ──────────────────────────────────────────────────────────

const QUERY_CASES: &[Case] = &[
    // ── 基本形状 ──
    Case {
        name: "query_parse_basic_pairs",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("a=1&b=2")"#
        ),
        expect: "[a: 1, b: 2]",
    },
    Case {
        name: "query_parse_empty_string_is_empty_dict",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("")"#
        ),
        expect: "[]",
    },
    Case {
        name: "query_parse_returns_a_dict",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"TYPE(QUERY_PARSE("a=1"))"#
        ),
        expect: "DICT",
    },
    // ── 重复键:**全部保留** ──
    //
    // ⚠ 这一条是本成员与「后写覆盖先写」那类解析器的**分界**:本成员的值
    //   在键重复时升级成 `ARRAY[STRING]`(按出现序)。
    Case {
        name: "query_parse_duplicate_key_keeps_all_values",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("a=1&a=2")"#
        ),
        expect: "[a: [1, 2]]",
    },
    Case {
        name: "query_parse_duplicate_key_three_times",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("a=1&a=2&a=3")"#
        ),
        expect: "[a: [1, 2, 3]]",
    },
    Case {
        name: "query_parse_duplicate_key_preserves_order",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("a=x&a=y&a=z")"#
        ),
        expect: "[a: [x, y, z]]",
    },
    // ── `+` ⇒ 空格(表单约定)──
    Case {
        name: "query_parse_plus_becomes_space",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("q=hello+world")"#
        ),
        expect: "[q: hello world]",
    },
    Case {
        name: "query_parse_plus_in_the_key_becomes_space",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("a+b=1")"#
        ),
        expect: "[a b: 1]",
    },
    // ⚠ **顺序不能反**:`%2B` 是**编码后的字面 `+`** ⇒ 它必须解出 `+`,
    //   而**不是**空格。若实现把 `%XX` 解码放在 `+` → 空格**之前**,
    //   这一条会红(解出的是空格)。
    Case {
        name: "query_parse_percent_encoded_plus_stays_a_plus",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("q=%2B")"#
        ),
        expect: "[q: +]",
    },
    // ── `%XX` 百分号解码 ──
    Case {
        name: "query_parse_percent_decodes_space",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("q=a%20b")"#
        ),
        expect: "[q: a b]",
    },
    Case {
        name: "query_parse_percent_decodes_reserved_chars",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("q=a%26b%3Dc")"#
        ),
        expect: "[q: a&b=c]",
    },
    Case {
        name: "query_parse_percent_decodes_the_key_too",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("a%20b=1")"#
        ),
        expect: "[a b: 1]",
    },
    // ⚠ **编码过的分隔符不是分隔符**:`%26` 解出来是 `&`,但它**不**该被
    //   当成「下一对」——切分发生在解码**之前**。
    Case {
        name: "query_parse_encoded_separator_is_not_a_separator",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("a=1%262")"#
        ),
        expect: "[a: 1&2]",
    },
    // ── 无 `=` 的键 ⇒ 值取空串 ──
    Case {
        name: "query_parse_key_without_equals_gets_empty_value",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("flag")"#
        ),
        expect: "[flag: ]",
    },
    Case {
        name: "query_parse_empty_value",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("a=")"#
        ),
        expect: "[a: ]",
    },
    // ⚠ **只切第一个 `=`**:值里可以合法地含 `=`。
    Case {
        name: "query_parse_value_may_contain_equals",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("a=b=c")"#
        ),
        expect: "[a: b=c]",
    },
    // ── 空段跳过 ──
    Case {
        name: "query_parse_empty_segments_are_skipped",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE("a=1&&b=2&")"#
        ),
        expect: "[a: 1, b: 2]",
    },
    // ── 失败口径 ──
    //
    // ⚠ 非法百分号转义是**运行期可预期失败** ⇒ `ERR` **值**(不是诊断),
    //   `kind` 是 `"QueryError"`(`kind` 是开放字符串 ⇒ **不新增诊断码**)。
    Case {
        name: "query_parse_invalid_percent_escape_is_a_query_error",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"LET(r, QUERY_PARSE("a=%zz")); ERR_PAYLOAD(r)["kind"]"#
        ),
        expect: "QueryError",
    },
    Case {
        name: "query_parse_truncated_percent_escape_is_a_query_error",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"LET(r, QUERY_PARSE("a=%2")); ERR_PAYLOAD(r)["kind"]"#
        ),
        expect: "QueryError",
    },
    // ── 元数 / 类型错 ⇒ **既有**诊断码 ──
    Case {
        name: "query_parse_non_string_is_e0030",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE(5)"#
        ),
        expect: "!E0030 QUERY_PARSE: expected string, got integer",
    },
    Case {
        name: "query_parse_zero_args_is_e0022",
        src: concat!(
            r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); "#,
            r#"QUERY_PARSE()"#
        ),
        expect: "!E0022 function expects 1..1 argument(s), got 0",
    },
];

// ── FORMAT_NAMED ────────────────────────────────────────────────────────

const FORMAT_NAMED_CASES: &[Case] = &[
    // ── 多键具名 ──
    Case {
        name: "format_named_two_named_keys",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("hi {name}, age {age}", ["name": "alice", "age": 30])"#
        ),
        expect: "hi alice, age 30",
    },
    // ⚠ **反向守卫**:本成员认**全部** `DICT` 实参。
    //
    //   若把具名解析改回「只认第一个 `DICT` 参数」(既有 `FORMAT` 的口径),
    //   `b` 在第二个字典里 ⇒ 取不到 ⇒ 输出会是 `1/{b}` ⇒ **本条必红**。
    Case {
        name: "format_named_looks_in_every_dict",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("{a}/{b}", ["a": 1], ["b": 2])"#
        ),
        expect: "1/2",
    },
    // 多个字典里**同一个键** ⇒ **第一个**命中者胜出(按实参出现序)。
    Case {
        name: "format_named_first_dict_wins_on_a_shared_key",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("{a}", ["a": 1], ["a": 2])"#
        ),
        expect: "1",
    },
    // 非字典实参被**跳过**(与 `FORMAT` 同款:具名只看 `DICT`)。
    Case {
        name: "format_named_skips_non_dict_args",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("{k}", 7, ["k": 9])"#
        ),
        expect: "9",
    },
    // ── 宽度 / 对齐 / 填充 ──
    //
    // ⚠ `{0:>10}` 是**位置**占位符带格式说明 ⇒ 宽度 10、右对齐、空格填充。
    Case {
        name: "format_named_positional_with_right_alignment",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("{0:>10}", 42)"#
        ),
        expect: "        42",
    },
    Case {
        name: "format_named_named_with_left_alignment",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("{name:<8}", ["name": "ab"])"#
        ),
        expect: "ab      ",
    },
    Case {
        name: "format_named_custom_fill_char",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("{name:*>6}", ["name": "ab"])"#
        ),
        expect: "****ab",
    },
    Case {
        name: "format_named_center_alignment",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("{name:^7}", ["name": "ab"])"#
        ),
        expect: "  ab   ",
    },
    // ⚠ **只给宽度、不给对齐** ⇒ **左对齐**(本成员的口径:渲染值一律先
    //   走 `STR` 语义变成字符串 ⇒ 一条规则,不做「数字右对齐 / 字符串左对齐」
    //   的按类型分叉)。
    Case {
        name: "format_named_width_only_defaults_to_left",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("{name:6}", ["name": "ab"])"#
        ),
        expect: "ab    ",
    },
    // 内容比宽度长 ⇒ **不截断**(原样返回)。
    Case {
        name: "format_named_does_not_truncate",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("{name:<3}", ["name": "abcdef"])"#
        ),
        expect: "abcdef",
    },
    // 宽度按**码点**算(与 `LEN` 同一口径),不是字节。
    Case {
        name: "format_named_width_counts_codepoints",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("{name:<4}", ["name": "éé"])"#
        ),
        expect: "éé  ",
    },
    // ── 缺失的键 ⇒ **保留原样** ──
    //
    // ⚠ 本成员**不因缺键报错** —— 与 `FORMAT` 的「未匹配的 `{...}` 保留
    //   原样」同款(§10.6 末段)。
    Case {
        name: "format_named_missing_key_is_kept_literal",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("hi {name}", ["other": 1])"#
        ),
        expect: "hi {name}",
    },
    // 缺键**且带格式说明** ⇒ 整个占位符(含说明)原样保留。
    Case {
        name: "format_named_missing_key_with_spec_is_kept_literal",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("{name:<8}", ["other": 1])"#
        ),
        expect: "{name:<8}",
    },
    // 没有 `DICT` 实参 ⇒ 具名占位符原样保留。
    Case {
        name: "format_named_without_a_dict_is_kept_literal",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("hi {name}", "alice")"#
        ),
        expect: "hi {name}",
    },
    // 越界的**位置**下标 ⇒ 原样保留(与 `FORMAT` 同款)。
    Case {
        name: "format_named_out_of_range_index_is_kept_literal",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("{9}", 1)"#
        ),
        expect: "{9}",
    },
    // ── 畸形 ⇒ **既有** `E0039`(不新增诊断码)──
    Case {
        name: "format_named_malformed_spec_is_e0039",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("{name:zz}", ["name": "a"])"#
        ),
        expect: "!E0039 FORMAT_NAMED: malformed format spec: zz",
    },
    Case {
        name: "format_named_empty_placeholder_name_is_e0039",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("{:>10}", 1)"#
        ),
        expect:
            "!E0039 FORMAT_NAMED: malformed format template: empty placeholder name in '{:>10}'",
    },
    Case {
        name: "format_named_unclosed_brace_is_e0039",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("hi {name", ["name": "a"])"#
        ),
        expect: "!E0039 FORMAT_NAMED: malformed format template: unclosed '{' at offset 3",
    },
    Case {
        name: "format_named_empty_placeholder_is_e0039",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED("a {} b", ["a": 1])"#
        ),
        expect:
            "!E0039 FORMAT_NAMED: malformed format template: empty placeholder '{}' at offset 2",
    },
    // ── 元数 / 类型错 ⇒ **既有**诊断码 ──
    Case {
        name: "format_named_non_string_template_is_e0030",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED(42)"#
        ),
        expect: "!E0030 FORMAT_NAMED: expected string (template) as first arg, got number",
    },
    Case {
        name: "format_named_zero_args_is_e0022",
        src: concat!(
            r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); "#,
            r#"FORMAT_NAMED()"#
        ),
        expect: "!E0022 FORMAT_NAMED: function expects at least 1 argument (template), got 0",
    },
];

// ── 既有 `FORMAT` 的**逐字**回归 ────────────────────────────────────────
//
// ⚠ 这张表是本册**最重要**的部分:它把既有 `FORMAT` 的输出**逐字**钉住。
//
//   `FORMAT_NAMED` 是**新成员**(更宽的写法),**不是** `FORMAT` 的替代品
//   ⇒ 既有 `FORMAT` 的语义**一字不许改**。下表里那两条「否定」用例
//   尤其关键:它们证明 `FORMAT` **没有**被顺带改成新口径。
const FORMAT_REGRESSION: &[Case] = &[
    // ── §10.6 的三条规范示例(逐字)──
    Case {
        name: "format_regression_positional_example",
        src: r#"IMPORT("wlwl:std.format", ["FORMAT"]); FORMAT("hi {0}, you are {1} years old", "alice", 30)"#,
        expect: "hi alice, you are 30 years old",
    },
    Case {
        name: "format_regression_named_example",
        src: r#"IMPORT("wlwl:std.format", ["FORMAT"]); FORMAT("hi {name}, age {age}", ["name": "alice", "age": 30])"#,
        expect: "hi alice, age 30",
    },
    Case {
        name: "format_regression_mixed_example",
        src: r#"IMPORT("wlwl:std.format", ["FORMAT"]); FORMAT("hi {0}, age {age}", "alice", ["age": 30])"#,
        expect: "hi alice, age 30",
    },
    // ── 否定用例:`FORMAT` **只认第一个 `DICT`**(§10.6 的明文口径)──
    //
    // ⚠ 若有人把 `FORMAT` 也改成「认全部 `DICT`」⇒ 这条变红。
    Case {
        name: "format_regression_only_the_first_dict_is_searched",
        src: r#"IMPORT("wlwl:std.format", ["FORMAT"]); FORMAT("{b}", ["a": 1], ["b": 2])"#,
        expect: "{b}",
    },
    // ── 否定用例:`FORMAT` **不懂格式说明** ──
    //
    // ⚠ `{0:>10}` 在 `FORMAT` 里是**一个键名**(`0:>10`)⇒ 查不到 ⇒ 原样。
    //   若有人给 `FORMAT` 也加上格式说明 ⇒ 这条变红。
    Case {
        name: "format_regression_format_spec_is_not_understood",
        src: r#"IMPORT("wlwl:std.format", ["FORMAT"]); FORMAT("{0:>10}", 42)"#,
        expect: "{0:>10}",
    },
    // ── 否定用例:`FORMAT` 的成员名与签名没变 ──
    Case {
        name: "format_regression_member_is_still_importable",
        src: r#"IMPORT("wlwl:std.format", ["FORMAT"]); LEN([FORMAT])"#,
        expect: "1",
    },
];

// ── 「不要新增同义成员」的守卫 ──────────────────────────────────────────
//
// ⚠ `STR(x)` 与 `SPLIT(s, sep)` 都是**全局内建** ⇒ `std.str` **不导出**它们
//   (§6 明文「不重导出任何全局内建」)。`std.text` 也**没有** `TO_STRING`
//   ⇒ 不需要、也不该新增一个。
const NO_DUPLICATE_BUILTIN: &[Case] = &[
    Case {
        name: "str_does_not_reexport_split",
        src: r#"IMPORT("wlwl:std.str", ["SPLIT"]); NULL"#,
        expect: "!E0023 'SPLIT' is not exported by module 'wlwl:std.str'",
    },
    Case {
        name: "str_does_not_reexport_str",
        src: r#"IMPORT("wlwl:std.str", ["STR"]); NULL"#,
        expect: "!E0023 'STR' is not exported by module 'wlwl:std.str'",
    },
    Case {
        name: "str_does_not_export_to_string",
        src: r#"IMPORT("wlwl:std.str", ["TO_STRING"]); NULL"#,
        expect: "!E0023 'TO_STRING' is not exported by module 'wlwl:std.str'",
    },
    Case {
        name: "text_does_not_export_to_string",
        src: r#"IMPORT("wlwl:std.text", ["TO_STRING"]); NULL"#,
        expect: "!E0023 'TO_STRING' is not exported by module 'wlwl:std.text'",
    },
];

// ── 跑起来 ──────────────────────────────────────────────────────────────

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_str_format")
}

/// 求一段 wlwl 源 ⇒ 它的显示值(或 `!CODE message` 形态的诊断)。
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

/// 跑一张用例表,收集**全部**不符项(不是遇到第一个就停)——
/// 一次红能看出「改坏了几条」,而不是「改坏了一条还得再跑一次」。
fn run_table(label: &str, cases: &[Case]) {
    let bad: Vec<String> = cases
        .iter()
        .filter_map(|c| {
            let got = actual(c.src);
            if c.expect == UNFROZEN || got == c.expect {
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
        "{label} contract drift:\n    {}",
        bad.join("\n    ")
    );
}

#[test]
fn query_parse_contract() {
    run_table("wlwl:std.str::QUERY_PARSE", QUERY_CASES);
}

#[test]
fn format_named_contract() {
    run_table("wlwl:std.format::FORMAT_NAMED", FORMAT_NAMED_CASES);
}

/// 既有 `FORMAT` 的**逐字**回归 —— `FORMAT_NAMED` 落地**之后**它必须逐字不变。
#[test]
fn format_is_byte_identical_after_format_named_landed() {
    run_table("wlwl:std.format::FORMAT (regression)", FORMAT_REGRESSION);
}

/// `STR` / `SPLIT` / `TO_STRING` 都**已由全局内建覆盖** ⇒ 不许有同义成员。
#[test]
fn no_duplicate_of_the_global_builtins() {
    run_table("wlwl:std duplicate-builtin guard", NO_DUPLICATE_BUILTIN);
}

/// 成员面:`std.str` 多一个 `QUERY_PARSE`,`std.format` 多一个 `FORMAT_NAMED`。
///
/// ⚠ 这两条断言的是**导入面**(用户能写什么),不是附录 A 的镜像 ——
///   附录 A 是**冻结快照**,由集成者同步(一致性册 §5 的差异登记流程)。
#[test]
fn the_new_members_are_importable() {
    assert_eq!(
        actual(r#"IMPORT("wlwl:std.str", ["QUERY_PARSE"]); LEN([QUERY_PARSE])"#),
        "1"
    );
    assert_eq!(
        actual(r#"IMPORT("wlwl:std.format", ["FORMAT_NAMED"]); LEN([FORMAT_NAMED])"#),
        "1"
    );
}
