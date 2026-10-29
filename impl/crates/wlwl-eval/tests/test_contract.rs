//! `wlwl:std.test` 成员契约表 —— 标准库规范 §8(标准库底座 v0.11 M3-3)。
//!
//! 与 `collection_contract.rs` 同款结构,冻结顺序也一样:**R2 全量实现还在位
//! 时**跑一遍把实际结果写进 `expect`(并验证绿),再切成「R1 门面 + R2 内核」
//! 的混合形态、跑**同一张表**。全绿即证明混合化没有改变任何可观察行为。
//!
//! M3-3 的难点集中在 `EXPECT_ERR`:规范 §8 要求「`x` 为 `ERR` → `OK(载荷)`」,
//! 而语言自身的 ERR 透明调用语义(§8.2)在**调用边界**就把 ERR 实参短路掉,
//! 成员体根本不执行 —— 实测任何 `FUN` 收到 ERR 实参,调用结果都是那个 ERR,
//! 与函数体无关。所以 `EXPECT_ERR` 只能由门面**改名导出**内核,包一层
//! `FUN` 反而会把 §8 的语义丢掉。表里 `expect_err_on_err_is_ok` 一条钉住它。
//!
//! 「应然」侧是**标准库规范 §8 表格**(从 markdown 解析,与实现不同源)。

mod common;

use std::path::{Path, PathBuf};
use wlwl_eval::Evaluator;

/// 尚未冻结的占位符。**不能**用空串当哨兵:`RUN_TESTS()` 无用例时返回
/// `[]`,其 `display` 恰是空串级形态;更一般地,任何「结果就是空」的用例
/// 都会与「还没写」混淆。
const UNFROZEN: &str = "~UNFROZEN";

struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

const CASES: &[Case] = &[
    // ── TEST(name, body) -> NULL ──
    Case {
        name: "test_registers_and_returns_null",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "RUN_TESTS"]); TEST("t", FUN((), TRUE)); TEST("t2", FUN((), TRUE));"#,
        expect: "NULL",
    },
    Case {
        name: "test_name_must_be_string",
        src: r#"IMPORT("wlwl:std.test", ["TEST"]); TEST(1, FUN((), NULL));"#,
        expect: "!E0030 TEST: expected string, got integer",
    },
    Case {
        name: "test_body_must_be_callable",
        src: r#"IMPORT("wlwl:std.test", ["TEST"]); TEST("t", 5);"#,
        expect: "!E0030 TEST: expected function, got integer",
    },
    Case {
        name: "test_arity_zero",
        src: r#"IMPORT("wlwl:std.test", ["TEST"]); TEST();"#,
        expect: "!E0022 TEST: function expects 2 argument(s), got 0",
    },
    Case {
        name: "test_arity_one",
        src: r#"IMPORT("wlwl:std.test", ["TEST"]); TEST("t");"#,
        expect: "!E0022 TEST: function expects 2 argument(s), got 1",
    },
    Case {
        name: "test_arity_three",
        src: r#"IMPORT("wlwl:std.test", ["TEST"]); TEST("t", FUN((), NULL), 1);"#,
        expect: "!E0022 TEST: function expects 2 argument(s), got 3",
    },
    // ── ASSERT(cond, msg?) ──
    Case {
        name: "assert_true_is_ok_true",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); ASSERT(TRUE)"#,
        expect: "OK(TRUE)",
    },
    Case {
        name: "assert_truthy_non_boolean",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); ASSERT(1)"#,
        expect: "OK(TRUE)",
    },
    Case {
        name: "assert_truthy_string",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); ASSERT("x")"#,
        expect: "OK(TRUE)",
    },
    Case {
        name: "assert_false_payload",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); LET(r, ASSERT(FALSE)); ERR_PAYLOAD(r)"#,
        expect: "[code: E0046, kind: test_assertion_failed]",
    },
    Case {
        name: "assert_null_payload_omits_cond",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); LET(r, ASSERT(NULL)); ERR_PAYLOAD(r)"#,
        expect: "[code: E0046, kind: test_assertion_failed]",
    },
    // [v0.11 M3-3 + 业主 2026-10-01 裁决] `ASSERT` 的真值口径按语言规范 §2.3
    // 的**八个**假值(此前只把 `BOOLEAN(false)` / `NULL` 当假,于是 `ASSERT(0)`
    // 之类会通过)。这一组是该次变更的**探测器**:口径一旦被改回「只认
    // BOOLEAN / NULL」,它们立刻变红。改动前查过全仓 208 个 .wll 与 26 处
    // 内嵌源码,除本表外没有任何调用点依赖旧口径。
    //
    // 注意别把「假值」与「载荷里带不带 `cond` 键」混为一谈:后者的规则是
    // 「`cond` 是 `NULL` 或 `BOOLEAN(false)` 才省掉」(R2 `build_payload` 的原口径),
    // 与真值判定无关。所以 `ASSERT("")` / `ASSERT([])` 的载荷里**带着**
    // `cond`,只是断言失败。
    Case {
        name: "assert_zero_is_falsy_per_2_3",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); LET(r, ASSERT(0)); ERR_PAYLOAD(r)"#,
        expect: "[code: E0046, kind: test_assertion_failed, cond: 0]",
    },
    Case {
        name: "assert_zero_float_is_falsy_per_2_3",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); LET(r, ASSERT(0.0)); ERR_PAYLOAD(r)"#,
        expect: "[code: E0046, kind: test_assertion_failed, cond: 0.0]",
    },
    Case {
        name: "assert_empty_string_is_falsy_per_2_3",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); LET(r, ASSERT("")); ERR_PAYLOAD(r)"#,
        expect: "[code: E0046, kind: test_assertion_failed, cond: ]",
    },
    Case {
        name: "assert_empty_array_is_falsy_per_2_3",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); LET(r, ASSERT([])); ERR_PAYLOAD(r)"#,
        expect: "[code: E0046, kind: test_assertion_failed, cond: []]",
    },
    Case {
        name: "assert_empty_dict_is_falsy_per_2_3",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); LET(r, ASSERT(DICT())); ERR_PAYLOAD(r)"#,
        expect: "[code: E0046, kind: test_assertion_failed, cond: []]",
    },
    Case {
        name: "assert_non_falsy_values_still_pass",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); ASSERT(1); ASSERT("x"); ASSERT([0]); ASSERT(["a": 0])"#,
        expect: "OK(TRUE)",
    },
    Case {
        name: "assert_with_message",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); LET(r, ASSERT(FALSE, "boom")); ERR_PAYLOAD(r)"#,
        expect: "[code: E0046, kind: test_assertion_failed, msg: boom]",
    },
    Case {
        name: "assert_empty_message_is_kept",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); LET(r, ASSERT(FALSE, "")); ERR_PAYLOAD(r)"#,
        expect: "[code: E0046, kind: test_assertion_failed, msg: ]",
    },
    Case {
        name: "assert_arity_zero",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); ASSERT()"#,
        expect: "!E0022 ASSERT: function expects 2 argument(s), got 0",
    },
    Case {
        name: "assert_arity_three",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); ASSERT(TRUE, "m", 1)"#,
        expect: "!E0022 ASSERT: function expects 2 argument(s), got 3",
    },
    // ── ASSERT_EQ / ASSERT_NEQ ──
    Case {
        name: "assert_eq_equal",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_EQ"]); ASSERT_EQ(1, 1)"#,
        expect: "OK(TRUE)",
    },
    Case {
        name: "assert_eq_cross_type_numeric",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_EQ"]); ASSERT_EQ(1, 1.0)"#,
        expect: "OK(TRUE)",
    },
    Case {
        name: "assert_eq_deep_array",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_EQ"]); ASSERT_EQ([1, [2]], [1, [2]])"#,
        expect: "OK(TRUE)",
    },
    Case {
        name: "assert_eq_result",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_EQ"]); ASSERT_EQ(OK(1), OK(1))"#,
        expect: "OK(TRUE)",
    },
    Case {
        name: "assert_eq_unequal_payload",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_EQ"]); LET(r, ASSERT_EQ(1, 2)); ERR_PAYLOAD(r)"#,
        expect: "[code: E0047, kind: test_assertion_eq_failed, actual: 1, expected: 2]",
    },
    Case {
        name: "assert_eq_with_message",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_EQ"]); LET(r, ASSERT_EQ(1, 2, "m")); ERR_PAYLOAD(r)"#,
        expect: "[code: E0047, kind: test_assertion_eq_failed, actual: 1, expected: 2, msg: m]",
    },
    Case {
        name: "assert_eq_null_payload_keeps_actual",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_EQ"]); LET(r, ASSERT_EQ(NULL, 1)); ERR_PAYLOAD(r)"#,
        expect: "[code: E0047, kind: test_assertion_eq_failed, actual: NULL, expected: 1]",
    },
    Case {
        name: "assert_neq_unequal",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_NEQ"]); ASSERT_NEQ(1, 2)"#,
        expect: "OK(TRUE)",
    },
    Case {
        name: "assert_neq_equal_payload",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_NEQ"]); LET(r, ASSERT_NEQ(1, 1)); ERR_PAYLOAD(r)"#,
        expect: "[code: E0048, kind: test_assertion_neq_failed, actual: 1, expected: 1]",
    },
    Case {
        name: "assert_neq_with_message",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_NEQ"]); LET(r, ASSERT_NEQ(1, 1, "m")); ERR_PAYLOAD(r)"#,
        expect: "[code: E0048, kind: test_assertion_neq_failed, actual: 1, expected: 1, msg: m]",
    },
    Case {
        name: "assert_eq_arity_one",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_EQ"]); ASSERT_EQ(1)"#,
        expect: "!E0022 ASSERT_EQ: function expects 3 argument(s), got 1",
    },
    Case {
        name: "assert_neq_arity_one",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_NEQ"]); ASSERT_NEQ(1)"#,
        expect: "!E0022 ASSERT_NEQ: function expects 3 argument(s), got 1",
    },
    // ── EXPECT_ERR ──
    Case {
        name: "expect_err_on_err_is_ok",
        src: r#"IMPORT("wlwl:std.test", ["EXPECT_ERR"]); EXPECT_ERR(ERR("boom"))"#,
        expect: "OK(ERR(boom))",
    },
    Case {
        name: "expect_err_on_err_dict_payload",
        src: r#"IMPORT("wlwl:std.test", ["EXPECT_ERR"]); EXPECT_ERR(ERR(["code": "E0046"]))"#,
        expect: "OK(ERR([code: E0046]))",
    },
    Case {
        name: "expect_err_non_err_payload",
        src: r#"IMPORT("wlwl:std.test", ["EXPECT_ERR"]); LET(r, EXPECT_ERR(5)); ERR_PAYLOAD(r)"#,
        expect: "[code: E0049, kind: test_expect_err_failed, cond: 5]",
    },
    Case {
        name: "expect_err_null_payload_omits_cond",
        src: r#"IMPORT("wlwl:std.test", ["EXPECT_ERR"]); LET(r, EXPECT_ERR(NULL)); ERR_PAYLOAD(r)"#,
        expect: "[code: E0049, kind: test_expect_err_failed]",
    },
    Case {
        name: "expect_err_arity_zero",
        src: r#"IMPORT("wlwl:std.test", ["EXPECT_ERR"]); EXPECT_ERR()"#,
        expect: "!E0022 EXPECT_ERR: function expects 1 argument(s), got 0",
    },
    // ── RUN_TESTS ──
    // `duration_ms` 非确定,所以只投影确定性的字段。
    Case {
        name: "run_tests_empty",
        src: r#"IMPORT("wlwl:std.test", ["RUN_TESTS"]); RUN_TESTS()"#,
        expect: "[]",
    },
    Case {
        name: "run_tests_arity",
        src: r#"IMPORT("wlwl:std.test", ["RUN_TESTS"]); RUN_TESTS(1)"#,
        expect: "!E0022 RUN_TESTS: function expects 0 argument(s), got 1",
    },
    Case {
        name: "run_tests_passing",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "ASSERT_EQ", "RUN_TESTS"]); TEST("a", FUN((), ASSERT_EQ(1, 1))); LET(rs, RUN_TESTS()); LEN(rs)"#,
        expect: "1",
    },
    Case {
        name: "run_tests_passing_name",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "RUN_TESTS"]); TEST("a", FUN((), TRUE)); LET(rs, RUN_TESTS()); rs[0]["name"]"#,
        expect: "a",
    },
    Case {
        name: "run_tests_passing_flag",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "RUN_TESTS"]); TEST("a", FUN((), TRUE)); LET(rs, RUN_TESTS()); rs[0]["passed"]"#,
        expect: "TRUE",
    },
    Case {
        name: "run_tests_record_has_required_keys",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "RUN_TESTS"]); TEST("a", FUN((), TRUE)); LET(rs, RUN_TESTS()); >=(LEN(KEYS(INDEX_GET(rs, 0))), 3)"#,
        expect: "TRUE",
    },
    Case {
        name: "run_tests_duration_is_integer",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "RUN_TESTS"]); TEST("a", FUN((), TRUE)); LET(rs, RUN_TESTS()); TYPE(rs[0]["duration_ms"])"#,
        expect: "INTEGER",
    },
    Case {
        name: "run_tests_failing_flag",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "ASSERT", "RUN_TESTS"]); TEST("a", FUN((), ASSERT(FALSE))); LET(rs, RUN_TESTS()); rs[0]["passed"]"#,
        expect: "FALSE",
    },
    Case {
        name: "run_tests_failing_has_error_key",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "ASSERT", "RUN_TESTS"]); TEST("a", FUN((), ASSERT(FALSE))); LET(rs, RUN_TESTS()); HAS(rs[0], "error")"#,
        expect: "TRUE",
    },
    Case {
        name: "run_tests_failing_error_payload_code",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "ASSERT", "RUN_TESTS"]); TEST("a", FUN((), ASSERT(FALSE))); LET(rs, RUN_TESTS()); rs[0]["error"]["code"]"#,
        expect: "E0046",
    },
    Case {
        name: "run_tests_uncaught_err_in_body_fails",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "RUN_TESTS"]); TEST("a", FUN((), ERR("escaped"))); LET(rs, RUN_TESTS()); rs[0]["passed"]"#,
        expect: "FALSE",
    },
    Case {
        name: "run_tests_uncaught_err_has_error_key",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "RUN_TESTS"]); TEST("a", FUN((), ERR("escaped"))); LET(rs, RUN_TESTS()); HAS(rs[0], "error")"#,
        expect: "TRUE",
    },
    Case {
        name: "run_tests_order_is_registration_order",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "RUN_TESTS"]); TEST("first", FUN((), TRUE)); TEST("second", FUN((), TRUE)); LET(rs, RUN_TESTS()); +(+(INDEX_GET(INDEX_GET(rs, 0), "name"), "/"), INDEX_GET(INDEX_GET(rs, 1), "name"))"#,
        expect: "first/second",
    },
    Case {
        name: "run_tests_drains_registry",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "RUN_TESTS"]); TEST("a", FUN((), TRUE)); LEN(RUN_TESTS())"#,
        expect: "1",
    },
    Case {
        name: "run_tests_second_call_is_empty",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "RUN_TESTS"]); TEST("a", FUN((), TRUE)); RUN_TESTS(); LEN(RUN_TESTS())"#,
        expect: "0",
    },
    // ── [G-1] ASSERT_MSG(cond, message) ──
    //
    // 载荷与 `ASSERT` **同形**(`code: E0046` / `kind: test_assertion_failed`)
    // **加** `msg` 键 ⇒ **不新增诊断码**(规范 N-3)。
    Case {
        name: "assert_msg_passing_is_ok",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_MSG"]); ASSERT_MSG(TRUE, "m")"#,
        expect: "OK(TRUE)",
    },
    Case {
        name: "assert_msg_failure_payload_carries_the_message",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_MSG"]); LET(r, ASSERT_MSG(FALSE, "boom")); ERR_PAYLOAD(r)"#,
        expect: "[code: E0046, kind: test_assertion_failed, msg: boom]",
    },
    Case {
        name: "assert_msg_failure_keeps_the_cond_key",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_MSG"]); LET(r, ASSERT_MSG(0, "boom")); ERR_PAYLOAD(r)"#,
        expect: "[code: E0046, kind: test_assertion_failed, cond: 0, msg: boom]",
    },
    // 消息是**必需**的:缺一个就是元数错(措辞照抄 R2 的 `arity()` 口径)。
    Case {
        name: "assert_msg_requires_a_message",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_MSG"]); ASSERT_MSG(TRUE)"#,
        expect: "!E0022 ASSERT_MSG: function expects 2 argument(s), got 1",
    },
    Case {
        name: "assert_msg_arity_zero",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_MSG"]); ASSERT_MSG()"#,
        expect: "!E0022 ASSERT_MSG: function expects 2 argument(s), got 0",
    },
    // 消息必须是**字符串**(CI 要能直接读它)。
    Case {
        name: "assert_msg_message_must_be_a_string",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_MSG"]); ASSERT_MSG(TRUE, 5)"#,
        expect: "!E0030 ASSERT_MSG: expected string, got integer",
    },
    // ── [G-1] ASSERT_TRUE(value) ──
    //
    // ⚠ 与 `ASSERT` 的真值口径**不同**:ASSERT 用 §2.3 的八假值(0 / "" / []
    // 都是假值 ⇒ **失败**);ASSERT_TRUE 用**宽松口径** —— **只有 `FALSE` 与
    // `NULL` 为假**,其余一切为真 ⇒ 0 / "" / [] 都**通过**。
    // 这一组是这个差异的探测器(需求清单 §4 W-31 点名要解决的那三件事)。
    Case {
        name: "assert_true_accepts_true",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_TRUE"]); ASSERT_TRUE(TRUE)"#,
        expect: "OK(TRUE)",
    },
    // 宽松口径:`0` / `0.0` / `""` / 空数组 / 空字典 / NaN 都是**真**。
    Case {
        name: "assert_true_accepts_zero",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_TRUE"]); ASSERT_TRUE(0)"#,
        expect: "OK(TRUE)",
    },
    Case {
        name: "assert_true_accepts_an_empty_string",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_TRUE"]); ASSERT_TRUE("")"#,
        expect: "OK(TRUE)",
    },
    Case {
        name: "assert_true_accepts_an_empty_array",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_TRUE"]); ASSERT_TRUE([])"#,
        expect: "OK(TRUE)",
    },
    // 只有 `FALSE` 与 `NULL` 为假。
    Case {
        name: "assert_true_rejects_false",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_TRUE"]); LET(r, ASSERT_TRUE(FALSE)); ERR_PAYLOAD(r)"#,
        expect: "[code: E0046, kind: test_assertion_failed]",
    },
    Case {
        name: "assert_true_rejects_null",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_TRUE"]); LET(r, ASSERT_TRUE(NULL)); ERR_PAYLOAD(r)"#,
        expect: "[code: E0046, kind: test_assertion_failed]",
    },
    // ⚠ **反向**:同一个实参,`ASSERT` 判失败而 `ASSERT_TRUE` 判通过 ——
    //   这条钉住「§2.3 口径没被顺手改掉」(计划 §4 W-31 的硬约束之一)。
    Case {
        name: "assert_zero_is_still_falsy_under_section_2_3",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); LET(r, ASSERT(0)); ERR_PAYLOAD(r)"#,
        expect: "[code: E0046, kind: test_assertion_failed, cond: 0]",
    },
    Case {
        name: "assert_true_arity_zero",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_TRUE"]); ASSERT_TRUE()"#,
        expect: "!E0022 ASSERT_TRUE: function expects 1 argument(s), got 0",
    },
    // ── [G-1] EXPECT_ERR_CODE(fn, code) ──
    //
    // 捕获**原生诊断码**(宿主级错误)。⚠ 与 `EXPECT_ERR` 捕的不是同一种东西:
    // `EXPECT_ERR` 只吃 `ERR` **值**;原生诊断(`E1003` 除零)是宿主级错误,
    // 捕不到它就会一路逃逸成进程级失败(仓里 `H4_expect_err_cannot_catch_native`
    // 探针钉的就是这个缺口)。
    Case {
        name: "expect_err_code_catches_the_native_code",
        src: r#"IMPORT("wlwl:std.test", ["EXPECT_ERR_CODE"]); INDEX_GET(UNWRAP(EXPECT_ERR_CODE(FUN((), /(1, 0)), "E1003")), "code")"#,
        expect: "E1003",
    },
    // ⚠ **原生码不逃逸**:命中时结果是 `OK(...)` 值,不是一个逃出去的诊断。
    //   若诊断逃逸,这一行会变成 `!E1003 /: division by zero` ⇒ 变红。
    Case {
        name: "expect_err_code_native_code_does_not_escape",
        src: r#"IMPORT("wlwl:std.test", ["EXPECT_ERR_CODE"]); IS_OK(EXPECT_ERR_CODE(FUN((), /(1, 0)), "E1003"))"#,
        expect: "TRUE",
    },
    // 没抛诊断 ⇒ 不匹配(失败走**既有**码 E0049 ⇒ 不新增诊断码)。
    Case {
        name: "expect_err_code_without_a_diagnostic_fails",
        src: r#"IMPORT("wlwl:std.test", ["EXPECT_ERR_CODE"]); LET(r, EXPECT_ERR_CODE(FUN((), TRUE), "E1003")); ERR_PAYLOAD(r)"#,
        expect: "[code: E0049, kind: test_expect_err_code_failed, expected: E1003]",
    },
    // 抛了**别的**码 ⇒ 载荷带 `actual`(实际拿到的码)。
    Case {
        name: "expect_err_code_reports_the_actual_code",
        src: r#"IMPORT("wlwl:std.test", ["EXPECT_ERR_CODE"]); INDEX_GET(ERR_PAYLOAD(EXPECT_ERR_CODE(FUN((), /(1, 0)), "E0030")), "actual")"#,
        expect: "E1003",
    },
    Case {
        name: "expect_err_code_arity_one",
        src: r#"IMPORT("wlwl:std.test", ["EXPECT_ERR_CODE"]); EXPECT_ERR_CODE(FUN((), TRUE))"#,
        expect: "!E0022 EXPECT_ERR_CODE: function expects 2 argument(s), got 1",
    },
    Case {
        name: "expect_err_code_code_must_be_a_string",
        src: r#"IMPORT("wlwl:std.test", ["EXPECT_ERR_CODE"]); EXPECT_ERR_CODE(FUN((), TRUE), 5)"#,
        expect: "!E0030 EXPECT_ERR_CODE: expected string, got integer",
    },
    Case {
        name: "expect_err_code_body_must_be_callable",
        src: r#"IMPORT("wlwl:std.test", ["EXPECT_ERR_CODE"]); EXPECT_ERR_CODE(5, "E1003")"#,
        expect: "!E0030 EXPECT_ERR_CODE: expected function, got integer",
    },
    // ── [G-1] SKIP(reason) ──
    //
    // 跳过必须**机器可区分**:跳过记录多出 `skipped` / `reason` 两个键。
    Case {
        name: "skip_marks_the_record_as_skipped",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "SKIP", "RUN_TESTS"]); TEST("s", FUN((), SKIP("no net"))); LET(rs, RUN_TESTS()); rs[0]["skipped"]"#,
        expect: "TRUE",
    },
    // ⚠ 跳过**不是**通过:`passed` 取 FALSE ⇒ CI 必须先看 `skipped`。
    Case {
        name: "skip_is_not_recorded_as_passed",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "SKIP", "RUN_TESTS"]); TEST("s", FUN((), SKIP("no net"))); LET(rs, RUN_TESTS()); rs[0]["passed"]"#,
        expect: "FALSE",
    },
    Case {
        name: "skip_records_the_reason",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "SKIP", "RUN_TESTS"]); TEST("s", FUN((), SKIP("no net"))); LET(rs, RUN_TESTS()); rs[0]["reason"]"#,
        expect: "no net",
    },
    // ⚠ **其它记录的键集逐字未变**(通过记录**没有** `skipped` 键)—— 这条
    //   钉住「新增能力只落在新增记录上,不改既有记录的形状」。
    Case {
        name: "passing_records_have_no_skipped_key",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "RUN_TESTS"]); TEST("a", FUN((), TRUE)); LET(rs, RUN_TESTS()); HAS(rs[0], "skipped")"#,
        expect: "FALSE",
    },
    Case {
        name: "failing_records_have_no_skipped_key",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "ASSERT", "RUN_TESTS"]); TEST("a", FUN((), ASSERT(FALSE))); LET(rs, RUN_TESTS()); HAS(rs[0], "skipped")"#,
        expect: "FALSE",
    },
    Case {
        name: "skip_reason_must_be_a_string",
        src: r#"IMPORT("wlwl:std.test", ["SKIP"]); SKIP(5)"#,
        expect: "!E0030 SKIP: expected string, got integer",
    },
    // ── [G-1] TEST_CASES(name, cases, body) ──
    Case {
        name: "test_cases_registers_one_test_per_case",
        src: r#"IMPORT("wlwl:std.test", ["TEST_CASES", "ASSERT_EQ", "RUN_TESTS"]); TEST_CASES("add", [[1, 1], [2, 3]], FUN((a, b), ASSERT_EQ(a, b))); LET(rs, RUN_TESTS()); LEN(rs)"#,
        expect: "2",
    },
    Case {
        name: "test_cases_names_carry_the_case_index",
        src: r#"IMPORT("wlwl:std.test", ["TEST_CASES", "ASSERT_EQ", "RUN_TESTS"]); TEST_CASES("add", [[1, 1], [2, 3]], FUN((a, b), ASSERT_EQ(a, b))); LET(rs, RUN_TESTS()); rs[0]["name"]"#,
        expect: "add[0]",
    },
    Case {
        name: "test_cases_passes_when_the_arguments_match",
        src: r#"IMPORT("wlwl:std.test", ["TEST_CASES", "ASSERT_EQ", "RUN_TESTS"]); TEST_CASES("add", [[1, 1], [2, 3]], FUN((a, b), ASSERT_EQ(a, b))); LET(rs, RUN_TESTS()); rs[0]["passed"]"#,
        expect: "TRUE",
    },
    Case {
        name: "test_cases_records_the_failing_case",
        src: r#"IMPORT("wlwl:std.test", ["TEST_CASES", "ASSERT_EQ", "RUN_TESTS"]); TEST_CASES("add", [[1, 1], [2, 3]], FUN((a, b), ASSERT_EQ(a, b))); LET(rs, RUN_TESTS()); rs[1]["passed"]"#,
        expect: "FALSE",
    },
    Case {
        name: "test_cases_failure_carries_the_error_payload",
        src: r#"IMPORT("wlwl:std.test", ["TEST_CASES", "ASSERT_EQ", "RUN_TESTS"]); TEST_CASES("add", [[1, 1], [2, 3]], FUN((a, b), ASSERT_EQ(a, b))); LET(rs, RUN_TESTS()); rs[1]["error"]["code"]"#,
        expect: "E0047",
    },
    Case {
        name: "test_cases_arity",
        src: r#"IMPORT("wlwl:std.test", ["TEST_CASES"]); TEST_CASES("add")"#,
        expect: "!E0022 TEST_CASES: function expects 3 argument(s), got 1",
    },
    // ── [G-1] RUN_TESTS_ASYNC() ──
    //
    // 异步跑一轮:测试体里 `SPAWN` + `AWAIT` 等一个异步任务 ⇒ 外层必须**等到**
    // 它 ⇒ 记录里有它的终值。
    Case {
        name: "run_tests_async_waits_for_the_spawned_task",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "RUN_TESTS_ASYNC"]); TEST("a", FUN((), (LET(h, SPAWN(FUN((), 42))); AWAIT(h)))); LET(rs, RUN_TESTS_ASYNC()); rs[0]["return_value"]"#,
        expect: "42",
    },
    Case {
        name: "run_tests_async_records_the_pass",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "RUN_TESTS_ASYNC"]); TEST("a", FUN((), (LET(h, SPAWN(FUN((), 42))); AWAIT(h)))); LET(rs, RUN_TESTS_ASYNC()); rs[0]["passed"]"#,
        expect: "TRUE",
    },
    // 异步模式下的跳过与同步模式**同形**(同一套识别口径)。
    Case {
        name: "run_tests_async_marks_skips_the_same_way",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "SKIP", "RUN_TESTS_ASYNC"]); TEST("s", FUN((), SKIP("no net"))); LET(rs, RUN_TESTS_ASYNC()); rs[0]["skipped"]"#,
        expect: "TRUE",
    },
    Case {
        name: "run_tests_async_arity",
        src: r#"IMPORT("wlwl:std.test", ["RUN_TESTS_ASYNC"]); RUN_TESTS_ASYNC(1)"#,
        expect: "!E0022 RUN_TESTS_ASYNC: function expects 0 argument(s), got 1",
    },
    // ── 成员面(§8 全表)──
    Case {
        name: "all_section8_members_import",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "ASSERT", "ASSERT_EQ", "ASSERT_NEQ", "EXPECT_ERR", "RUN_TESTS"]); LEN([TEST, ASSERT, ASSERT_EQ, ASSERT_NEQ, EXPECT_ERR, RUN_TESTS])"#,
        expect: "6",
    },
    // [G-1] 既有 6 个成员**一个没动**,新增 7 个 ⇒ 导出面 13 个。
    Case {
        name: "all_g1_members_import",
        src: r#"IMPORT("wlwl:std.test", ["TEST", "ASSERT", "ASSERT_EQ", "ASSERT_NEQ", "EXPECT_ERR", "RUN_TESTS", "ASSERT_MSG", "ASSERT_TRUE", "EXPECT_ERR_CODE", "SKIP", "TEST_CASES", "TEMP_DIR", "RUN_TESTS_ASYNC"]); LEN([TEST, ASSERT, ASSERT_EQ, ASSERT_NEQ, EXPECT_ERR, RUN_TESTS, ASSERT_MSG, ASSERT_TRUE, EXPECT_ERR_CODE, SKIP, TEST_CASES, TEMP_DIR, RUN_TESTS_ASYNC])"#,
        expect: "13",
    },
    Case {
        name: "test_is_not_a_global_builtin",
        src: r#"PRINT(TEST("t", FUN((), NULL)));"#,
        expect: "!E0020 undefined name `TEST`",
    },
    Case {
        name: "unknown_member_is_e0023",
        src: r#"IMPORT("wlwl:std.test", ["NOPE"]); NULL"#,
        expect: "!E0023 'NOPE' is not exported by module 'wlwl:std.test'",
    },
    // ── 混合形态特有:门面成员是闭包,内核透传成员是 native ──
    // 这两条锁 §0.2「混合模块以门面(R1 侧)为对外契约层」:调用面完全一致,
    // 实现语言不可感知(规范 §0.1 / ADR-0021)。
    Case {
        name: "facade_member_is_usable_without_import_prefix",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); TYPE(ASSERT)"#,
        expect: "FUNCTION",
    },
];

fn scratch_dir() -> PathBuf {
    common::scratch("wlwl_test_contract")
}

fn actual(case: &Case) -> String {
    // 解析错也算「实际结果」而不是 panic —— 引导器要能跑完整张表,
    // 半路 panic 会让人以为后面那些用例不存在。
    let ast = match wlwl_parser::parse(case.src, "t.wll") {
        Ok(a) => a,
        Err(e) => return format!("!PARSE {}", e.diagnostic().message),
    };
    let mut ev = Evaluator::new().with_base_dir(scratch_dir());
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => format!(
            "!{} {}",
            e.diagnostic().code.as_str(),
            e.diagnostic().message
        ),
    }
}

#[test]
fn test_module_matches_the_frozen_r2_contract() {
    let bad: Vec<String> = CASES
        .iter()
        .filter_map(|c| {
            let got = actual(c);
            if c.expect == UNFROZEN {
                Some(format!(
                    "{name}: NOT FROZEN yet (actual = {got})",
                    name = c.name
                ))
            } else if c.expect != got {
                Some(format!(
                    "{name}:\n      frozen: {want}\n      actual: {got}",
                    name = c.name,
                    want = c.expect
                ))
            } else {
                None
            }
        })
        .collect();
    assert!(
        bad.is_empty(),
        "{} / {} case(s) diverged from the frozen contract:\n  - {}",
        bad.len(),
        CASES.len(),
        bad.join("\n  - ")
    );
}

/// 引导器:打印全部用例的实际结果,用来冻结 `expect`。
#[test]
fn dump_actuals() {
    for c in CASES {
        println!(
            "CASE|{name}|{expect}|{actual}",
            name = c.name,
            expect = c.expect,
            actual = actual(c)
        );
    }
}

/// [D11-019] 测试体里出现 `YIELD()` **不得**产生假通过。
///
/// 这条不是契约表的一行,因为要钉的是「结果里**不出现**某个东西」——
/// 表格只能比一个值。历史缺陷:测试体里的 `YIELD()` 在
/// `wlwl_std::call_callable` 里被吞掉,回调在挂起点被截断,
/// `RUN_TESTS` 拿到挂起点的 `NULL`,按 §8「非 `ERR` 即通过」记成
/// `passed = TRUE`,体里 `YIELD()` 之后的断言**一次都没跑**。
/// 挂起信号必须原样交回调度器(见 `wlwl_value::StdFn` 契约)。
#[test]
fn a_yielding_test_body_is_never_recorded_as_passed() {
    // `YIELD` 只能出现在调度驱动的上下文(任务体)里,故整段包在
    // SCOPE/SPAWN 内,并用 `AWAIT` 取回任务终值(`SCOPE` 自己返回的是
    // `SPAWN` 的句柄,不是子任务的值)。
    let src = r#"IMPORT("wlwl:std.test", ["TEST", "ASSERT", "RUN_TESTS"]);
LET(r, SCOPE(FUN(() , AWAIT(SPAWN(FUN(() ,
    TEST("must-fail", FUN(() , YIELD(); ASSERT(FALSE)));
    RUN_TESTS()
))))));
r"#;
    let got = actual(&Case {
        name: "unused",
        src,
        expect: UNFROZEN,
    });
    assert!(
        !got.contains("passed: TRUE"),
        "a test body that suspended must not be recorded as passed: {got}"
    );
    assert!(
        !got.contains("must-fail"),
        "a suspended test body must not produce a record at all: {got}"
    );
}

/// 反向守卫:上面那条以「结果里没有 `passed: TRUE`」为断言,若整段程序
/// 因为任何原因没跑起来(解析错 / 立即失败),断言会**空洞地**通过。
/// 这条把「同一段程序在测试体不挂起时确实产出 `passed: FALSE` 记录」
/// 钉住 —— 即上面的守卫不是靠「什么都没发生」满足的。
#[test]
fn the_guarding_case_above_would_have_caught_the_false_pass() {
    let src = r#"IMPORT("wlwl:std.test", ["TEST", "ASSERT", "RUN_TESTS"]);
LET(r, SCOPE(FUN(() , AWAIT(SPAWN(FUN(() ,
    TEST("must-fail", FUN(() , ASSERT(FALSE)));
    RUN_TESTS()
))))));
r"#;
    let got = actual(&Case {
        name: "unused",
        src,
        expect: UNFROZEN,
    });
    assert!(
        got.contains("must-fail") && got.contains("passed: FALSE"),
        "the non-suspending twin must fail loudly, else the guard above is vacuous: {got}"
    );
}

/// 反向守卫:表本身不能塌掉。
#[test]
fn the_contract_table_is_not_empty() {
    let frozen = CASES.iter().filter(|c| c.expect != UNFROZEN).count();
    assert_eq!(
        frozen,
        CASES.len(),
        "{}/{} rows are unfrozen — run `dump_actuals` and fill `expect` in",
        CASES.len() - frozen,
        CASES.len()
    );
    let mut names: Vec<&str> = CASES.iter().map(|c| c.name).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len(), "duplicate case name");
}

/// ── [G-1] `TEMP_DIR()` 必须返回**绝对路径** ───────────────────────────
///
/// ⚠⚠ **为什么这条不能靠表格那一行 display 钉住**:`TEMP_DIR()` 的返回值
/// **每次都不同**(唯一名)⇒ 表格只能比「它长什么样」,比不了「它是不是绝对
/// 路径」。而「是不是绝对路径」恰恰是这条要求的**全部** —— `std.fs` 成员的
/// 相对路径按**进程 CWD** 解析 ⇒ 相对路径会把测试文件写进仓库里
/// (`fs_atomic_lock_meta_contract.rs` 踩过的既有教训)。
///
/// ⇒ 判据放在 Rust 侧:`Path::is_absolute()` + 「不在当前工作目录底下」。
#[test]
fn temp_dir_returns_an_absolute_path_outside_the_working_directory() {
    let src = r#"IMPORT("wlwl:std.test", ["TEMP_DIR"]); TEMP_DIR()"#;
    let got = actual(&Case {
        name: "unused",
        src,
        expect: UNFROZEN,
    });
    assert!(
        !got.starts_with('!'),
        "TEMP_DIR() must not raise a diagnostic: {got}"
    );
    let path = PathBuf::from(&got);
    assert!(
        path.is_absolute(),
        "TEMP_DIR() must return an ABSOLUTE path (std.fs resolves relative paths \
         against the process CWD ⇒ a relative path writes test files into the repo): {got}"
    );
    // 不许落在当前工作目录底下(那正是「写进仓库」的那一档)。
    if let Ok(cwd) = std::env::current_dir() {
        assert!(
            !path.starts_with(&cwd),
            "TEMP_DIR() must not hand out a path inside the working directory \
             (cwd = {}): {got}",
            cwd.display()
        );
    }
}

/// ── [G-1] `ASSERT_TRUE` 与 `ASSERT` 的真值口径**不同** ────────────────
///
/// `ASSERT` 用规范 §2.3 的**八假值**(FALSE / NULL / 0 / 0.0 / "" / 空数组 /
/// 空字典 / NaN)⇒ `ASSERT(0)` **失败**。
/// `ASSERT_TRUE` 用**宽松口径**(**只有 `FALSE` 与 `NULL` 为假**)⇒
/// `ASSERT_TRUE(0)` **通过**、`ASSERT_TRUE(FALSE)` **失败**。
///
/// ⇒ 两个成员**必须能区分开**:同一份实参,一个判失败、一个判通过。
/// ⚠ 这条**不是**表格的一行:表格只能比一个值,而这里要钉的是**两个成员的
///   口径差**(两个不同的源程序)。
#[test]
fn assert_true_uses_a_looser_truthiness_than_assert() {
    let assert_zero = actual(&Case {
        name: "unused",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT"]); LET(r, ASSERT(0)); ERR_PAYLOAD(r)"#,
        expect: UNFROZEN,
    });
    assert_eq!(
        assert_zero, "[code: E0046, kind: test_assertion_failed, cond: 0]",
        "ASSERT keeps the §2.3 truthiness口径: 0 is one of the eight falsy values ⇒ it fails"
    );
    let assert_true_zero = actual(&Case {
        name: "unused",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_TRUE"]); ASSERT_TRUE(0)"#,
        expect: UNFROZEN,
    });
    assert_eq!(
        assert_true_zero, "OK(TRUE)",
        "ASSERT_TRUE uses the loose口径 (only FALSE / NULL are falsy) ⇒ 0 must pass"
    );
    let assert_true_false = actual(&Case {
        name: "unused",
        src: r#"IMPORT("wlwl:std.test", ["ASSERT_TRUE"]); LET(r, ASSERT_TRUE(FALSE)); ERR_PAYLOAD(r)"#,
        expect: UNFROZEN,
    });
    assert_eq!(
        assert_true_false, "[code: E0046, kind: test_assertion_failed]",
        "ASSERT_TRUE must still reject FALSE"
    );
}

// ── 与标准库规范 §8 表格的外部对照 ─────────────────────────────────

fn spec_text() -> String {
    let p: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("wlwl-spec-reference.md");
    std::fs::read_to_string(&p).expect("stdlib spec is readable")
}

fn section_8_members() -> Vec<String> {
    let text = spec_text();
    // ⚠ 结束锚点是**具体**的下一章号(§9)⇒ 用 `chapter_body_until`:
    // 规范将来在 §8 与 §9 之间插入新章时,它会被**包进来** ⇒ 成员表对拍失败
    // ⇒ 变红(「设计变了就一起改测试」那道闸)。用通用锚点则会**静默截断**。
    let body = common::chapter_body_until(&text, "## 标准库规范 · 8 ", "\n## 标准库规范 · 9 ");
    let mut out: Vec<String> = Vec::new();
    for line in body.lines() {
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

fn exports_of(path: &str) -> Vec<String> {
    let backend = wlwl_std::resolve(path).unwrap_or_else(|| panic!("{path} resolves"));
    match backend {
        wlwl_std::StdBackend::Lang(src) => wlwl_std::lang_exports(src.source),
        wlwl_std::StdBackend::Native(spec) => {
            spec.functions.iter().map(|(n, _)| n.to_string()).collect()
        }
    }
}

/// [G-1 / v0.12.2] 需求清单 G-1 新增的 7 个成员。
///
/// ⚠ **临时登记**:规范 §8 表格与附录 A 抄件**尚未**收录这些成员(规范仓的
///   注册镜像册是**冻结快照**,改它要走规范修订流程 —— 一致性册 §5.5 的
///   「实现侧扩展台账」是给**规范未认可**的扩展的合法落点)。
///   ⇒ 本测试先把它们登记成「实现侧扩展面」,让 §8 对拍**继续有效**
///   (而不是因为少 7 个成员就整条红掉)。
/// ⚠ 规范侧收录(附录 A 正式修订**或**台账登记)之后:把本常量**清空**即可 ——
///   那时 §8 表格 == 导出面,这条对拍回到「逐字相等」。
const G1_EXTENSIONS: &[&str] = &[
    "ASSERT_MSG",
    "ASSERT_TRUE",
    "EXPECT_ERR_CODE",
    "SKIP",
    "TEST_CASES",
    "TEMP_DIR",
    "RUN_TESTS_ASYNC",
];

#[test]
fn member_set_matches_the_spec_section_8_table() {
    let spec = section_8_members();
    let impls = exports_of("wlwl:std.test");
    let mut expected: Vec<String> = spec.clone();
    for m in G1_EXTENSIONS {
        assert!(
            !spec.iter().any(|s| s == m),
            "G1_EXTENSIONS 的 `{m}` 已经在 §8 表格里了 ⇒ 它该从本常量里删掉"
        );
        expected.push((*m).to_string());
    }
    let mut a = expected.clone();
    let mut b = impls.clone();
    a.sort();
    b.sort();
    assert_eq!(
        a, b,
        "§8 table (+ the G-1 extension list) and the export list disagree.\n  spec: {spec:?}\n  impl: {impls:?}"
    );
    assert_eq!(
        impls.len(),
        spec.len() + G1_EXTENSIONS.len(),
        "§8 declares {} member(s) and G-1 adds {} ⇒ the export list must hold {}",
        spec.len(),
        G1_EXTENSIONS.len(),
        spec.len() + G1_EXTENSIONS.len()
    );
}

/// 反向守卫:上面那条以「§8 解析结果」为输入,§8 表格形状一变就可能解析出
/// 空集。这条把「至少解析出 6 个且逐个点名」钉住。
#[test]
fn the_spec_table_extractor_actually_finds_the_members() {
    let spec = section_8_members();
    assert_eq!(
        spec.len(),
        6,
        "§8 extractor found {} member(s): {spec:?}",
        spec.len()
    );
    for n in [
        "TEST",
        "ASSERT",
        "ASSERT_EQ",
        "ASSERT_NEQ",
        "EXPECT_ERR",
        "RUN_TESTS",
    ] {
        assert!(spec.iter().any(|m| m == n), "§8 extractor missed `{n}`");
    }
}
